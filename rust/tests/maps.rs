// SPDX-License-Identifier: GPL-3.0-only
//! Shared texture layout, channel isolation, vector semantics, and export safety.
use painter::{
    Canvas, Document, ErrorCode, Pixel,
    assets::AssetRoot,
    atlas::{Atlas, Options},
    drawing::Axis,
    geometry::{Faces, Shape},
    maps::{self, Channel, MapRequest},
    operations::{Batch, Operation},
    palette::{Color, Policy},
    placement::Placement,
    storage,
};
use serde_json::json;
use std::{fs, path::Path};

const RED: Pixel = Pixel::rgba(200, 40, 30, 255);
const GREEN: Pixel = Pixel::rgba(20, 210, 80, 128);
const RIGHT: Pixel = Pixel::rgba(255, 128, 128, 255);
const BOTH: MapRequest = MapRequest {
    emissive: true,
    normal: true,
};
fn clear(document: &mut Document, pixel: Pixel) {
    document
        .edit(&Batch::new(vec![Operation::Clear {
            color: Some(Color::Rgba(pixel)),
        }]))
        .unwrap();
}
fn mapped() -> Document {
    let mut document = Document::from_canvas(
        Canvas::from_pixels(
            2,
            2,
            vec![
                Pixel {
                    a: 128,
                    ..RED
                },
                RED,
                Pixel::TRANSPARENT,
                RED,
            ],
        )
        .unwrap(),
    )
    .unwrap();
    document
        .add_layer_with_channel("glow".into(), Channel::Emissive)
        .unwrap();
    clear(&mut document, GREEN);
    document
        .add_layer_with_channel("normals".into(), Channel::Normal)
        .unwrap();
    document
        .edit(&Batch::new(vec![Operation::SetPixel {
            x: 1,
            y: 0,
            color: Color::Rgba(RIGHT),
            over: false,
        }]))
        .unwrap();
    document
}
#[test]
fn channel_coverage_fallbacks_and_color_isolation_are_exact() {
    let document = mapped();
    let color = document.flatten(0).unwrap();
    assert_eq!(
        color.pixel(0, 0),
        Some(Pixel {
            a: 128,
            ..RED
        })
    );
    let glow = document.render_channel(0, Channel::Emissive).unwrap();
    assert_eq!(
        glow.pixel(0, 0),
        Some(Pixel {
            a: 64,
            ..GREEN
        })
    );
    assert_eq!(glow.pixel(1, 0), Some(GREEN));
    assert_eq!(glow.pixel(0, 1), Some(Pixel::TRANSPARENT));
    let normal = document.render_channel(0, Channel::Normal).unwrap();
    assert_eq!(
        normal.pixel(0, 0),
        Some(Pixel {
            a: 128,
            ..maps::FLAT_NORMAL
        })
    );
    assert_eq!(normal.pixel(1, 0), Some(RIGHT));
    assert_eq!(normal.pixel(0, 1), Some(Pixel::TRANSPARENT));
    assert_eq!(document.render_channel(0, Channel::Color).unwrap(), color);
}
#[test]
fn normal_layers_choose_the_topmost_vector_without_rgb_blending() {
    let mut document = mapped();
    document
        .add_layer_with_channel("top-normal".into(), Channel::Normal)
        .unwrap();
    clear(
        &mut document,
        Pixel {
            a: 128,
            ..maps::FLAT_NORMAL
        },
    );
    document
        .configure_layer("top-normal", None, Some(127))
        .unwrap();
    assert_eq!(
        document
            .render_channel(0, Channel::Normal)
            .unwrap()
            .pixel(1, 0),
        Some(maps::FLAT_NORMAL)
    );
    document
        .configure_layer("top-normal", Some(false), None)
        .unwrap();
    assert_eq!(
        document
            .render_channel(0, Channel::Normal)
            .unwrap()
            .pixel(1, 0),
        Some(RIGHT)
    );
}
#[test]
fn invalid_normals_paint_blending_and_palette_quantization_roll_back() {
    let mut document = mapped();
    let original = document.clone();
    for operation in [
        Operation::Clear {
            color: Some(Color::Rgba(Pixel::rgba(128, 128, 128, 255))),
        },
        Operation::SetPixel {
            x: 0,
            y: 0,
            color: Color::Rgba(RIGHT),
            over: true,
        },
    ] {
        assert_eq!(
            document
                .edit(&Batch::new(vec![operation]))
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
        assert_eq!(document, original);
    }
    let mut batch = Batch::new(Vec::new());
    batch.palette =
        Some(serde_json::from_value(json!({"version":1,"colors":{"red":"#C8281E"}})).unwrap());
    batch.policy = Policy::Nearest;
    assert!(document.edit(&batch).is_err());
    assert_eq!(document, original);
    document.select_layer("glow").unwrap();
    let original = document.clone();
    assert!(document.edit(&batch).is_err());
    assert_eq!(document, original);
}
#[test]
fn normal_spatial_transforms_also_transform_encoded_directions() {
    let mut document = Document::new(2, 2).unwrap();
    clear(&mut document, RED);
    document
        .add_layer_with_channel("normal".into(), Channel::Normal)
        .unwrap();
    document
        .edit(&Batch::new(vec![Operation::SetPixel {
            x: 0,
            y: 1,
            color: Color::Rgba(RIGHT),
            over: false,
        }]))
        .unwrap();
    let original = document.clone();
    document
        .edit(&Batch::new(vec![Operation::Flip {
            axis: Axis::Horizontal,
        }]))
        .unwrap();
    assert_eq!(
        document
            .render_channel(0, Channel::Normal)
            .unwrap()
            .pixel(1, 1),
        Some(Pixel::rgba(0, 128, 128, 255))
    );
    document
        .edit(&Batch::new(vec![Operation::Flip {
            axis: Axis::Horizontal,
        }]))
        .unwrap();
    assert_eq!(document, original);
    for (turns, x, y, expected) in [
        (1, 0, 0, Pixel::rgba(128, 0, 128, 255)),
        (-1, 1, 1, Pixel::rgba(127, 255, 128, 255)),
        (2, 1, 0, Pixel::rgba(0, 127, 128, 255)),
    ] {
        let mut transformed = original.clone();
        transformed
            .edit(&Batch::new(vec![Operation::Rotate {
                turns,
            }]))
            .unwrap();
        assert_eq!(
            transformed
                .render_channel(0, Channel::Normal)
                .unwrap()
                .pixel(x, y),
            Some(expected)
        );
    }
}
#[test]
fn layer_operations_preserve_channels_and_refuse_cross_channel_merges() {
    let mut document = mapped();
    document.duplicate_layer("normals", "copy".into()).unwrap();
    assert_eq!(document.layers().last().unwrap().channel, Channel::Normal);
    let before = document.render_channel(0, Channel::Normal).unwrap();
    document.merge_layer("copy").unwrap();
    assert_eq!(document.render_channel(0, Channel::Normal).unwrap(), before);
    let original = document.clone();
    assert!(document.merge_layer("normals").is_err());
    assert_eq!(document, original);
}
#[test]
fn schemas_default_color_reject_unknown_channels_and_validate_all_normal_cels() {
    let document = Document::new(1, 1).unwrap();
    let legacy = serde_json::to_value(&document).unwrap();
    assert!(legacy["layers"][0].get("channel").is_none());
    assert_eq!(
        serde_json::from_value::<Document>(legacy.clone()).unwrap(),
        document
    );
    let mut invalid = legacy.clone();
    invalid["layers"][0]["channel"] = json!("depth");
    assert!(serde_json::from_value::<Document>(invalid).is_err());
    let mut invalid = legacy;
    invalid["layers"][0]["channel"] = json!("normal");
    invalid["layers"][0]["visible"] = json!(false);
    invalid["frames"][0]["cels"][0]["pixels"][0] = json!({"r":128,"g":128,"b":128,"a":255});
    assert!(serde_json::from_value::<Document>(invalid).is_err());
    for value in ["color", "normal,normal", "unknown", "normal,,emissive"] {
        assert!(value.parse::<MapRequest>().is_err());
    }
    for value in [
        json!(["color"]),
        json!(["normal", "normal"]),
        json!(["emissive", "normal", "emissive"]),
    ] {
        assert!(serde_json::from_value::<MapRequest>(value).is_err());
    }
}
#[test]
fn normal_encoding_accepts_unit_quantization_and_rejects_nonfinite_or_zero_vectors() {
    for (x, y, z) in [(1.0, 1.0, 1.0), (-1.0, 0.0, 0.0), (0.0, 1.0, 1.0)] {
        let pixel = maps::encode_normal(x, y, z, 255).unwrap();
        maps::validate_normals(&Canvas::from_pixels(1, 1, vec![pixel]).unwrap()).unwrap();
    }
    for direction in [
        (0.0, 0.0, 0.0),
        (f64::INFINITY, 0.0, 1.0),
        (f64::NAN, 1.0, 1.0),
    ] {
        assert!(maps::encode_normal(direction.0, direction.1, direction.2, 255).is_err());
    }
}
fn bytes(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap()
}
#[test]
fn animated_maps_share_rectangles_native_resolution_and_deterministic_bytes() {
    let mut document = mapped();
    document.add_frame("second".into(), Some(0), 230).unwrap();
    clear(&mut document, maps::FLAT_NORMAL);
    document.select_layer("glow").unwrap();
    clear(&mut document, Pixel::rgba(190, 60, 220, 255));
    let root = tempfile::tempdir().unwrap();
    let options = Options {
        columns: Some(2),
        padding: 2,
        margin: 1,
    };
    let atlas = Atlas::from_document_with_maps(&document, options, BOTH).unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    let old = root.path().join("old");
    atlas.export(&a, 3).unwrap();
    atlas.export(&b, 3).unwrap();
    Atlas::from_document(&document, options)
        .unwrap()
        .export(&old, 3)
        .unwrap();
    for name in [
        "sheet.png",
        "preview.png",
        "animation.png",
        "frames/0000.png",
        "frames/0001.png",
    ] {
        assert_eq!(bytes(&a.join(name)), bytes(&old.join(name)), "{name}");
    }
    for channel in ["emissive", "normal"] {
        let map = &atlas.metadata().maps[&channel.parse().unwrap()];
        assert_eq!(
            map.frames,
            [
                format!("{channel}/frames/0000.png"),
                format!("{channel}/frames/0001.png")
            ]
        );
        let sheet = storage::import_png(&a.join(&map.sheet), false).unwrap();
        assert_eq!(
            (sheet.width(), sheet.height()),
            (atlas.sheet().width(), atlas.sheet().height())
        );
        assert_eq!(sheet.pixel(0, 0), Some(Pixel::TRANSPARENT));
        for (index, frame) in atlas.metadata().frames.iter().enumerate() {
            let image = storage::import_png(&a.join(&map.frames[index]), false).unwrap();
            assert_eq!(
                image,
                document
                    .render_channel(index, channel.parse().unwrap())
                    .unwrap()
            );
            assert_eq!((image.width(), image.height()), (2, 2));
            assert_eq!(
                sheet.pixel(frame.rect.x as u32 + 1, frame.rect.y as u32),
                image.pixel(1, 0)
            );
            assert_eq!(
                bytes(&a.join(&map.frames[index])),
                bytes(&b.join(&map.frames[index]))
            );
        }
        assert_eq!(bytes(&a.join(&map.sheet)), bytes(&b.join(&map.sheet)));
    }
    assert_eq!(
        bytes(&a.join("metadata.json")),
        bytes(&b.join("metadata.json"))
    );
    assert!(
        !serde_json::to_value(Atlas::from_document(&document, options).unwrap().metadata())
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("maps")
    );
    assert!(!old.join("normal").exists());
    assert!(atlas.export(&a, 1).is_err());
    assert_eq!(
        bytes(&a.join("metadata.json")),
        bytes(&b.join("metadata.json"))
    );
}
#[test]
fn unpainted_maps_fall_back_and_combined_channel_limits_precede_allocations() {
    let mut document = Document::new(1, 1).unwrap();
    clear(&mut document, RED);
    assert_eq!(
        document
            .render_channel(0, Channel::Emissive)
            .unwrap()
            .pixels(),
        &[Pixel::TRANSPARENT]
    );
    assert_eq!(
        document
            .render_channel(0, Channel::Normal)
            .unwrap()
            .pixels(),
        &[maps::FLAT_NORMAL]
    );
    document.add_frame("second".into(), Some(0), 100).unwrap();
    let error = Atlas::from_document_with_maps(
        &document,
        Options {
            columns: Some(2),
            padding: 1024,
            margin: 1024,
        },
        BOTH,
    )
    .err()
    .unwrap();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
}
#[test]
fn geometry_uses_identical_face_masks_and_expected_orientations() {
    let faces = Faces {
        top: Color::Rgba(RED),
        front: Color::Rgba(RED),
        side: Color::Rgba(RED),
        outline: Some(Color::Rgba(RED)),
    };
    let prism = Shape::Prism {
        x: 1,
        y: 2,
        width: 3,
        height: 3,
        depth: 1,
        faces: faces.clone(),
    };
    for shape in [
        prism,
        Shape::Roof {
            x: 1,
            y: 5,
            width: 5,
            height: 3,
            depth: 1,
            faces: faces.clone(),
        },
        Shape::Cottage {
            x: 1,
            y: 8,
            width: 5,
            height: 4,
            depth: 1,
            roof_height: 3,
            faces: faces.clone(),
            roof: faces.clone(),
        },
        Shape::Steps {
            x: 1,
            y: 7,
            width: 3,
            height: 2,
            depth: 1,
            count: 3,
            faces,
        },
    ] {
        let mut document = Document::new(12, 12).unwrap();
        document
            .add_layer_with_channel("normals".into(), Channel::Normal)
            .unwrap();
        document.select_layer("base").unwrap();
        document.draw_shape(&shape, Some("normals"), None).unwrap();
        let color = document.flatten(0).unwrap();
        let normal = document.render_channel(0, Channel::Normal).unwrap();
        maps::validate_normals(&normal).unwrap();
        for (color, normal) in color.pixels().iter().zip(normal.pixels()) {
            assert_eq!(color.a, normal.a);
        }
        let mut reference = Canvas::new(12, 12).unwrap();
        Batch::new(shape.operations().unwrap())
            .apply(&mut reference)
            .unwrap();
        assert_eq!(color, reference);
        let original = document.clone();
        assert!(document.draw_shape(&shape, Some("base"), None).is_err());
        assert_eq!(document, original);
    }
}

#[test]
fn generated_prism_normals_point_top_front_and_right() {
    let mut document = Document::new(8, 8).unwrap();
    document
        .add_layer_with_channel("normal".into(), Channel::Normal)
        .unwrap();
    document.select_layer("base").unwrap();
    let shape = Shape::Prism {
        x: 1,
        y: 2,
        width: 3,
        height: 3,
        depth: 1,
        faces: Faces {
            top: Color::Rgba(RED),
            front: Color::Rgba(RED),
            side: Color::Rgba(RED),
            outline: None,
        },
    };
    document.draw_shape(&shape, Some("normal"), None).unwrap();
    let normal = document.render_channel(0, Channel::Normal).unwrap();
    assert_eq!(normal.pixel(2, 1), Some(Pixel::rgba(128, 218, 218, 255)));
    assert_eq!(normal.pixel(1, 3), Some(maps::FLAT_NORMAL));
    assert_eq!(normal.pixel(4, 2), Some(Pixel::rgba(218, 128, 218, 255)));
}

#[test]
fn geometry_and_normals_share_work_limits_and_roll_back_both_cels() {
    let mut document = Document::new(384, 256).unwrap();
    document
        .add_layer_with_channel("normal".into(), Channel::Normal)
        .unwrap();
    document.select_layer("base").unwrap();
    let shape = Shape::Steps {
        x: 1,
        y: 70,
        width: 3,
        height: 2,
        depth: 1,
        count: 32,
        faces: Faces {
            top: Color::Rgba(RED),
            front: Color::Rgba(RED),
            side: Color::Rgba(RED),
            outline: None,
        },
    };
    let original = document.clone();
    // Color alone fits, but a second independently reset budget would allow too much work.
    let mut color_only = document.clone();
    color_only.draw_shape(&shape, None, None).unwrap();
    assert!(
        color_only
            .flatten(0)
            .unwrap()
            .pixels()
            .iter()
            .any(|pixel| pixel.a != 0)
    );
    assert_eq!(
        document
            .draw_shape(&shape, Some("normal"), None)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(document, original);
}
#[test]
fn map_aware_pack_checks_detect_tampered_companions() {
    let root = tempfile::tempdir().unwrap();
    storage::save_project(&root.path().join("source.json"), &mapped(), true).unwrap();
    let recipe:painter::pack::Recipe=serde_json::from_value(json!({"version":1,"assets":[{"id":"sign","maps":["emissive","normal"],"source":{"kind":"artwork","input":"source.json"}}]})).unwrap();
    let out = root.path().join("pack");
    assert!(
        recipe
            .generate(&AssetRoot::new(root.path()).unwrap(), &out)
            .unwrap()
            .valid
    );
    assert!(painter::pack::check(&out, None).unwrap().valid);
    fs::remove_file(out.join("assets/0000/export/normal/frames/0000.png")).unwrap();
    assert!(!painter::pack::check(&out, None).unwrap().valid);
}
#[test]
fn stale_map_placement_and_noncolor_parts_fail_without_publishing() {
    let mut document = mapped();
    let atlas = Atlas::from_document_with_maps(&document, Options::default(), BOTH).unwrap();
    clear(&mut document, maps::FLAT_NORMAL);
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("out");
    let placement: Placement = serde_json::from_value(json!({"version":1})).unwrap();
    assert!(
        atlas
            .export_with_placement(&output, 1, &document, &placement)
            .is_err()
    );
    assert!(!output.exists());
    let placement: Placement = serde_json::from_value(
        json!({"version":1,"parts":[{"id":"bad","layers":["glow"],"order":0}]}),
    )
    .unwrap();
    assert!(placement.validate(&document).is_err());
}
