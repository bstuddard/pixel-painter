// SPDX-License-Identifier: GPL-3.0-only
//! Exact-pixel, rollback, round-trip, and adversarial library regressions.

use painter::{
    Canvas, Document, Pixel,
    animation::Tag,
    atlas::{Atlas, Options},
    drawing::{self, Axis, Point, Rect},
    inspect,
    operations::Batch,
    palette::{Palette, Policy},
    recipe::{Parameters, Recipe},
    storage,
    tileset::{Connection, Edge, Rules, Tileset},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

const RED: Pixel = Pixel::rgba(255, 0, 0, 255);
fn point(x: i32, y: i32) -> Point {
    Point {
        x,
        y,
    }
}
fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
fn batch(operations: Value) -> Batch {
    serde_json::from_value(json!({"version":1,"operations":operations})).unwrap()
}
fn apply(canvas: &mut Canvas, operations: Value) {
    batch(operations).apply(canvas).unwrap();
}
fn sample(width: u32, height: u32) -> Canvas {
    Canvas::from_pixels(
        width,
        height,
        (0..width * height)
            .map(|i| Pixel::rgba(i as u8, (i * 17) as u8, (i * 37) as u8, (i * 59) as u8))
            .collect(),
    )
    .unwrap()
}

#[test]
fn golden_batch_matches_hand_authored_pixels() {
    let operations: Batch = serde_json::from_str(include_str!("fixtures/gem.ops.json")).unwrap();
    let mut canvas = Canvas::new(8, 8).unwrap();
    let summary = operations.apply(&mut canvas).unwrap();
    assert_eq!(summary.changed_pixels, 35);
    let expected: Vec<_> = include_str!("fixtures/gem.txt")
        .lines()
        .flat_map(str::chars)
        .map(|c| match c {
            '.' => Pixel::TRANSPARENT,
            'D' => "#283048".parse().unwrap(),
            'P' => "#C05070".parse().unwrap(),
            'W' => "#FFFFFF".parse().unwrap(),
            _ => panic!("unknown fixture color"),
        })
        .collect();
    assert_eq!(canvas.pixels(), expected);
}

#[test]
fn malformed_canvases_cannot_bypass_public_invariants() {
    for data in [
        json!({"width":1,"height":1,"pixels":[]}),
        json!({"width":0,"height":1,"pixels":[]}),
        json!({"width":u32::MAX,"height":u32::MAX,"pixels":[]}),
        json!({"width":1,"height":1,"pixels":[RED,RED]}),
        json!({"width":1,"height":1,"pixels":[RED],"typo":true}),
    ] {
        assert!(serde_json::from_value::<Canvas>(data).is_err());
    }
    assert!(Canvas::from_pixels(1, 1, vec![]).is_err());
    assert!(Canvas::new(16_777_217, 1).is_err());
}

#[test]
fn library_export_preserves_every_rgba_byte() {
    let root = tempfile::tempdir().unwrap();
    let canvas = sample(19, 11);
    storage::save(&root.path().join("art.json"), &canvas, true).unwrap();
    let loaded = storage::load(&root.path().join("art.json")).unwrap();
    storage::export_png(&root.path().join("art.png"), &loaded).unwrap();
    assert_eq!(
        storage::import_png(&root.path().join("art.png"), false).unwrap(),
        canvas
    );
}

#[test]
fn hex_colors_are_strict_and_alpha_defaults_to_opaque() {
    for text in ["#12345", "#123456789", "123456", "#GG0000", "#é12345"] {
        assert!(text.parse::<Pixel>().is_err());
    }
    assert_eq!(
        "#abcdef".parse::<Pixel>().unwrap(),
        Pixel::rgba(171, 205, 239, 255)
    );
    assert_eq!(
        "#01020304".parse::<Pixel>().unwrap().to_string(),
        "#01020304"
    );
    assert_eq!(
        serde_json::from_value::<Pixel>(json!({"r":1,"g":2,"b":3}))
            .unwrap()
            .a,
        255
    );
    assert!(serde_json::from_value::<Pixel>(json!({"r":256,"g":0,"b":0})).is_err());
}

#[test]
fn source_over_uses_straight_alpha_with_expected_rounding() {
    assert_eq!(
        Pixel::rgba(255, 0, 0, 128).over(Pixel::rgba(0, 0, 255, 255), 255),
        Pixel::rgba(128, 0, 127, 255)
    );
    assert_eq!(
        Pixel::rgba(255, 0, 0, 128).over(Pixel::rgba(0, 0, 255, 128), 255),
        Pixel::rgba(170, 0, 85, 192)
    );
    assert_eq!(
        RED.over(Pixel::TRANSPARENT, 128),
        Pixel::rgba(255, 0, 0, 128)
    );
    assert_eq!(Pixel::rgba(9, 8, 7, 0).over(RED, 255), RED);
    assert_eq!(
        RED.over(Pixel::rgba(1, 2, 3, 0), 0),
        Pixel::rgba(1, 2, 3, 0)
    );
}

#[test]
fn lines_cover_all_octants_and_reverse_identically() {
    for x in 0..9 {
        for y in 0..9 {
            let mut a = Canvas::new(9, 9).unwrap();
            let mut b = a.clone();
            drawing::line(&mut a, point(4, 4), point(x, y), RED, false).unwrap();
            drawing::line(&mut b, point(x, y), point(4, 4), RED, false).unwrap();
            assert_eq!(a, b);
            assert_eq!(a.pixel(x as u32, y as u32), Some(RED));
            assert_eq!(
                a.pixels().iter().filter(|p| **p == RED).count(),
                (x - 4).abs().max((y - 4).abs()) as usize + 1
            );
        }
    }
}

#[test]
fn clipped_line_keeps_original_rasterization() {
    let mut small = Canvas::new(5, 5).unwrap();
    let mut large = Canvas::new(15, 15).unwrap();
    drawing::line(&mut small, point(-3, -2), point(7, 6), RED, false).unwrap();
    drawing::line(&mut large, point(2, 3), point(12, 11), RED, false).unwrap();
    assert_eq!(small, drawing::crop(&large, rect(5, 5, 5, 5)).unwrap());
}

#[test]
fn rectangle_edges_do_not_double_blend_corners_or_invent_clipped_edges() {
    let color = Pixel::rgba(255, 0, 0, 128);
    let mut canvas = Canvas::new(3, 3).unwrap();
    drawing::rectangle(&mut canvas, rect(-1, -1, 3, 3), color, false, true).unwrap();
    assert_eq!(canvas.pixel(0, 0), Some(Pixel::TRANSPARENT));
    assert_eq!(canvas.pixel(1, 1), Some(color));
    assert_eq!(canvas.pixels().iter().filter(|p| p.a != 0).count(), 3);
}

#[test]
fn degenerate_shapes_and_ellipse_symmetry() {
    for width in 1..=10 {
        for height in 1..=10 {
            let mut ellipse = Canvas::new(width, height).unwrap();
            drawing::ellipse(&mut ellipse, rect(0, 0, width, height), RED, true, false).unwrap();
            assert_eq!(ellipse, drawing::flip(&ellipse, Axis::Horizontal).unwrap());
            assert_eq!(ellipse, drawing::flip(&ellipse, Axis::Vertical).unwrap());
            assert!(ellipse.pixels().contains(&RED));
            let mut outline = Canvas::new(width, height).unwrap();
            drawing::rectangle(&mut outline, rect(0, 0, width, height), RED, false, false).unwrap();
            let expected = if width == 1 || height == 1 {
                width * height
            } else {
                2 * width + 2 * height - 4
            };
            assert_eq!(
                outline.pixels().iter().filter(|p| p.a != 0).count(),
                expected as usize
            );
        }
    }
    assert!(
        drawing::rectangle(
            &mut Canvas::new(1, 1).unwrap(),
            rect(0, 0, 0, 1),
            RED,
            true,
            false
        )
        .is_err()
    );
}

#[test]
fn polygon_fill_is_winding_independent_and_blends_once() {
    let mut canvas = Canvas::new(8, 8).unwrap();
    let mut reverse = canvas.clone();
    let points = [point(1, 1), point(6, 1), point(4, 6)];
    let color = Pixel::rgba(40, 80, 120, 128);
    drawing::polygon(&mut canvas, &points, color, true, true).unwrap();
    drawing::polygon(
        &mut reverse,
        &points.into_iter().rev().collect::<Vec<_>>(),
        color,
        true,
        true,
    )
    .unwrap();
    assert_eq!(canvas, reverse);
    assert!(canvas.pixels().iter().all(|p| p.a == 0 || p.a == 128));
    assert_eq!(canvas.pixel(4, 3), Some(color));
    assert_eq!(canvas.pixel(0, 0), Some(Pixel::TRANSPARENT));
    assert!(drawing::polygon(&mut canvas, &[point(0, 0)], RED, true, false).is_err());
}

#[test]
fn flood_fill_uses_four_neighbors_and_exact_hidden_rgba() {
    let mut canvas =
        Canvas::from_pixels(2, 2, vec![Pixel::TRANSPARENT, RED, RED, Pixel::TRANSPARENT]).unwrap();
    drawing::flood_fill(&mut canvas, point(0, 0), Pixel::rgba(0, 1, 0, 255)).unwrap();
    assert_eq!(canvas.pixel(1, 1), Some(Pixel::TRANSPARENT));
    let mut canvas =
        Canvas::from_pixels(2, 1, vec![Pixel::TRANSPARENT, Pixel::rgba(4, 5, 6, 0)]).unwrap();
    drawing::flood_fill(&mut canvas, point(0, 0), RED).unwrap();
    assert_eq!(canvas.pixel(1, 0), Some(Pixel::rgba(4, 5, 6, 0)));
    assert!(drawing::flood_fill(&mut canvas, point(-1, 0), RED).is_err());
}

#[test]
fn flood_fill_handles_large_connected_regions_without_recursion() {
    let mut canvas = Canvas::new(512, 512).unwrap();
    drawing::flood_fill(&mut canvas, point(0, 0), RED).unwrap();
    assert!(canvas.pixels().iter().all(|p| *p == RED));
}

#[test]
fn region_copy_is_a_snapshot_for_overlapping_paste() {
    let mut canvas = sample(5, 1);
    let before = canvas.clone();
    apply(
        &mut canvas,
        json!([{"op":"copy","name":"part","x":0,"y":0,"width":4,"height":1},{"op":"paste","name":"part","x":1,"y":0}]),
    );
    assert_eq!(&canvas.pixels()[1..], &before.pixels()[..4]);
    let crop = drawing::crop(&before, rect(-1, 0, 3, 2)).unwrap();
    assert_eq!(crop.pixel(0, 0), Some(Pixel::TRANSPARENT));
    assert_eq!(crop.pixel(1, 0), before.pixel(0, 0));
    assert!(crop.pixels()[3..].iter().all(|p| *p == Pixel::TRANSPARENT));
}

#[test]
fn transforms_preserve_rgba_and_inverse_operations_restore_pixels() {
    for width in 1..8 {
        for height in 1..8 {
            let canvas = sample(width, height);
            assert_eq!(
                drawing::rotate(&drawing::rotate(&canvas, 1).unwrap(), -1).unwrap(),
                canvas
            );
            assert_eq!(drawing::rotate(&canvas, 4).unwrap(), canvas);
            for axis in [Axis::Horizontal, Axis::Vertical] {
                assert_eq!(
                    drawing::flip(&drawing::flip(&canvas, axis).unwrap(), axis).unwrap(),
                    canvas
                );
            }
            assert_eq!(
                drawing::resize(
                    &drawing::resize(&canvas, width * 3, height * 3).unwrap(),
                    width,
                    height
                )
                .unwrap(),
                canvas
            );
        }
    }
    let rotated = drawing::rotate(&sample(2, 3), 1).unwrap();
    assert_eq!(rotated.pixel(0, 0), sample(2, 3).pixel(0, 2));
}

#[test]
fn translate_clears_exposed_pixels_and_clips() {
    let source = sample(4, 3);
    let moved = drawing::translate(&source, 1, -1).unwrap();
    assert_eq!(moved.pixel(0, 0), Some(Pixel::TRANSPARENT));
    assert_eq!(moved.pixel(1, 0), source.pixel(0, 1));
    assert!(moved.pixels()[8..].iter().all(|p| *p == Pixel::TRANSPARENT));
}

#[test]
fn failing_batch_rolls_back_with_exact_operation_index() {
    let mut canvas = sample(4, 4);
    let original = canvas.clone();
    let error = batch(
        json!([{"op":"clear","color":"#FFFFFF"},{"op":"set_pixel","x":99,"y":0,"color":"#FF0000"}]),
    )
    .apply(&mut canvas)
    .unwrap_err();
    assert_eq!(error.operation, Some(1));
    assert_eq!(error.code.as_str(), "out_of_bounds");
    assert_eq!(canvas, original);
}

#[test]
fn batch_rejects_versions_typos_missing_clipboards_and_huge_work() {
    for value in [
        json!({"version":1,"operations":[{"op":"unknown"}]}),
        json!({"version":1,"operations":[{"op":"clear","colour":"#FF0000"}]}),
        json!({"operations":[]}),
        json!({"version":1,"operations":[],"typo":true}),
    ] {
        assert!(serde_json::from_value::<Batch>(value).is_err());
    }
    let mut canvas = sample(2, 2);
    let original = canvas.clone();
    for value in [
        json!([{"op":"paste","name":"missing","x":0,"y":0}]),
        json!([{"op":"resize","width":u32::MAX,"height":u32::MAX}]),
        json!([{"op":"line","x1":i32::MIN,"y1":0,"x2":i32::MAX,"y2":0,"color":"#000000"}]),
        json!([{"op":"fill_rect","x":0,"y":0,"width":0,"height":1,"color":"#000000"}]),
    ] {
        assert!(batch(value).apply(&mut canvas).is_err());
        assert_eq!(canvas, original);
    }
    let mut unsupported = Batch::new(vec![]);
    unsupported.version = 99;
    assert_eq!(
        unsupported.apply(&mut canvas).unwrap_err().code.as_str(),
        "unsupported_version"
    );
}

fn test_palette() -> Palette {
    Palette {
        version: 1,
        colors: BTreeMap::from([
            ("a".into(), Pixel::rgba(0, 0, 0, 255)),
            ("z".into(), Pixel::rgba(2, 0, 0, 255)),
        ]),
    }
}

#[test]
fn strict_palette_validation_is_transactional_and_nearest_ties_are_stable() {
    let mut canvas = sample(2, 2);
    let original = canvas.clone();
    let mut operations = batch(json!([{"op":"clear","color":"#010000"}]));
    operations.palette = Some(test_palette());
    operations.policy = Policy::Strict;
    assert_eq!(
        operations.apply(&mut canvas).unwrap_err().code.as_str(),
        "palette_violation"
    );
    assert_eq!(canvas, original);
    operations.policy = Policy::Nearest;
    operations.apply(&mut canvas).unwrap();
    assert!(
        canvas
            .pixels()
            .iter()
            .all(|p| *p == Pixel::rgba(0, 0, 0, 255))
    );
    let mut hidden = Canvas::from_pixels(1, 1, vec![Pixel::rgba(1, 2, 3, 0)]).unwrap();
    test_palette()
        .constrain(&mut hidden, Policy::Nearest)
        .unwrap();
    assert_eq!(hidden.pixel(0, 0), Some(Pixel::rgba(1, 2, 3, 0)));
}

#[test]
fn named_colors_and_palette_reports_are_consistent() {
    let mut operations = batch(json!([{"op":"clear","color":"z"}]));
    operations.palette = Some(test_palette());
    let mut canvas = Canvas::new(2, 3).unwrap();
    operations.apply(&mut canvas).unwrap();
    let report = inspect::inspect(&canvas, operations.palette.as_ref());
    assert_eq!(report.palette_usage["z"], 6);
    assert_eq!(report.off_palette_pixels, 0);
    assert!(
        batch(json!([{"op":"clear","color":"missing"}]))
            .apply(&mut canvas)
            .is_err()
    );
}

#[test]
fn inspection_reports_bounds_and_each_alpha_category() {
    let mut canvas = Canvas::new(4, 3).unwrap();
    canvas.set_pixel(2, 1, RED).unwrap();
    canvas.set_pixel(1, 2, Pixel::rgba(2, 3, 4, 128)).unwrap();
    let report = inspect::inspect(&canvas, None);
    assert_eq!(report.occupied_bounds, Some(rect(1, 1, 2, 2)));
    assert_eq!(report.transparent_pixels, 10);
    assert_eq!(report.partial_alpha_pixels, 1);
    assert_eq!(report.opaque_pixels, 1);
    assert_eq!(report.unique_colors, 3);
    assert_eq!(
        inspect::inspect(&Canvas::new(1, 1).unwrap(), None).warnings,
        vec!["empty_image"]
    );
}

#[test]
fn seeded_templates_are_reproducible_and_support_components() {
    let value = json!({"version":2,"width":16,"height":16,"seed":42,"operations":[{"op":"noise","x":0,"y":0,"width":16,"height":16,"colors":["#112233","#445566"],"density":128},{"op":"stamp","canvas":{"width":1,"height":1,"pixels":[RED]},"x":2,"y":3}]});
    let template: Recipe = serde_json::from_value(value).unwrap();
    let a = template
        .render(&Parameters::new(), None, None, None)
        .unwrap();
    assert_eq!(
        a,
        template
            .render(&Parameters::new(), None, Some(42), None)
            .unwrap()
    );
    assert_ne!(
        a,
        template
            .render(&Parameters::new(), None, Some(43), None)
            .unwrap()
    );
    assert_eq!(a.pixel(2, 3), Some(RED));
}

#[test]
fn zero_density_noise_is_a_noop_and_full_density_covers_every_pixel() {
    let mut canvas = Canvas::new(8, 8).unwrap();
    apply(
        &mut canvas,
        json!([{"op":"noise","x":0,"y":0,"width":8,"height":8,"colors":["#FF0000"],"density":0}]),
    );
    assert!(canvas.pixels().iter().all(|p| p.a == 0));
    apply(
        &mut canvas,
        json!([{"op":"noise","x":0,"y":0,"width":8,"height":8,"colors":["#FF0000"]}]),
    );
    assert!(canvas.pixels().iter().all(|p| *p == RED));
}

#[test]
fn project_layers_compose_visibility_and_opacity() {
    let mut project = Document::from_canvas(
        Canvas::from_pixels(1, 1, vec![Pixel::rgba(0, 0, 255, 255)]).unwrap(),
    )
    .unwrap();
    project.add_layer("top".into()).unwrap();
    project
        .edit(&batch(json!([{"op":"clear","color":"#FF000080"}])))
        .unwrap();
    assert_eq!(
        project.flatten(0).unwrap().pixel(0, 0),
        Some(Pixel::rgba(128, 0, 127, 255))
    );
    project.configure_layer("top", Some(false), None).unwrap();
    assert_eq!(
        project.flatten(0).unwrap().pixel(0, 0),
        Some(Pixel::rgba(0, 0, 255, 255))
    );
    project
        .configure_layer("top", Some(true), Some(128))
        .unwrap();
    assert_eq!(
        project.flatten(0).unwrap().pixel(0, 0),
        Some(Pixel::rgba(64, 0, 191, 255))
    );
}

#[test]
fn project_round_trip_preserves_layers_frames_tags_and_selection() {
    let mut project = Document::from_canvas(sample(3, 2)).unwrap();
    project.add_layer("hat".into()).unwrap();
    project
        .edit(&batch(
            json!([{"op":"set_pixel","x":1,"y":0,"color":"#AABBCC80"}]),
        ))
        .unwrap();
    project.add_frame("walk".into(), Some(0), 123).unwrap();
    project.configure_frame(1, None, Some(point(1, 2))).unwrap();
    project
        .set_tag(
            "walk-east".into(),
            Tag {
                frames: vec![0, 1],
                direction: Some("east".into()),
            },
        )
        .unwrap();
    let restored: Document =
        serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
    assert_eq!(project, restored);
}

#[test]
fn invalid_project_structures_are_rejected() {
    let project = Document::new(2, 2).unwrap();
    let original = serde_json::to_value(project).unwrap();
    for (field, value) in [
        ("version", json!(2)),
        ("active_layer", json!(99)),
        ("frames", json!([])),
        ("layers", json!([])),
        ("width", json!(3)),
        ("tags", json!({"bad":{"frames":[99]}})),
    ] {
        let mut data = original.clone();
        data[field] = value;
        assert!(
            serde_json::from_value::<Document>(data).is_err(),
            "accepted {field}"
        );
    }
    let mut data = original;
    data["frames"][0]["duration_ms"] = json!(0);
    assert!(serde_json::from_value::<Document>(data).is_err());
}

#[test]
fn layer_duplicate_reorder_delete_and_merge_preserve_expected_pixels() {
    let mut project = Document::from_canvas(sample(2, 2)).unwrap();
    project.add_layer("top".into()).unwrap();
    project
        .edit(&batch(json!([{"op":"clear","color":"#44556680"}])))
        .unwrap();
    let before = project.flatten(0).unwrap();
    project.merge_layer("top").unwrap();
    assert_eq!(project.flatten(0).unwrap(), before);
    project.duplicate_layer("base", "copy".into()).unwrap();
    project.configure_layer("copy", Some(false), None).unwrap();
    project.reorder_layer("copy", 0).unwrap();
    assert_eq!(project.layers()[project.active_layer()].name, "copy");
    project.delete_layer("copy").unwrap();
    assert_eq!(project.flatten(0).unwrap(), before);
    assert!(project.delete_layer("base").is_err());
    assert!(project.merge_layer("base").is_err());
}

#[test]
fn frame_reordering_and_deletion_remap_tags_by_identity() {
    let mut project = Document::new(1, 1).unwrap();
    project.add_frame("b".into(), Some(0), 100).unwrap();
    project.add_frame("c".into(), None, 200).unwrap();
    project
        .set_tag(
            "walk".into(),
            Tag {
                frames: vec![0, 2, 1],
                direction: None,
            },
        )
        .unwrap();
    project.reorder_frame(2, 0).unwrap();
    assert_eq!(project.tags()["walk"].frames, vec![1, 0, 2]);
    assert_eq!(project.active_frame(), 0);
    project.delete_frame(0).unwrap();
    assert_eq!(project.tags()["walk"].frames, vec![0, 1]);
    project.delete_frame(1).unwrap();
    assert!(project.delete_frame(0).is_err());
}

#[test]
fn rejected_project_edits_do_not_change_selection_or_cels() {
    let mut project = Document::new(2, 3).unwrap();
    let before = project.clone();
    assert!(
        project
            .edit(&batch(json!([{"op":"resize","width":4,"height":4}])))
            .is_err()
    );
    assert_eq!(project, before);
    assert!(project.add_frame("bad".into(), None, 0).is_err());
    assert_eq!(project, before);
    assert!(project.add_layer("base".into()).is_err());
    assert_eq!(project, before);
    assert!(project.select_frame(9).is_err());
    assert_eq!(project, before);
    assert!(project.configure_frame(0, Some(60_001), None).is_err());
    assert_eq!(project, before);
}

#[test]
fn project_resize_updates_all_cels_and_checks_aggregate_limit() {
    let mut project = Document::new(2, 3).unwrap();
    project.add_layer("top".into()).unwrap();
    project.add_frame("b".into(), None, 100).unwrap();
    project.resize(4, 6).unwrap();
    project.validate().unwrap();
    assert_eq!(project.frames()[1].cels[1].width(), 4);
    let before = project.clone();
    assert!(project.resize(4096, 4096).is_err());
    assert_eq!(project, before);
}

#[test]
fn atlas_rectangles_decode_to_composites_and_bundles_refuse_overwrites() {
    let root = tempfile::tempdir().unwrap();
    let mut project = Document::from_canvas(sample(3, 2)).unwrap();
    project.add_frame("b".into(), Some(0), 150).unwrap();
    let atlas = Atlas::from_document(
        &project,
        Options {
            columns: Some(2),
            padding: 2,
            margin: 1,
        },
    )
    .unwrap();
    for (index, frame) in atlas.metadata().frames.iter().enumerate() {
        assert_eq!(
            drawing::crop(atlas.sheet(), frame.rect).unwrap(),
            project.flatten(index).unwrap()
        );
    }
    let output = root.path().join("bundle");
    atlas.export(&output, 3).unwrap();
    assert_eq!(
        storage::import_png(&output.join("sheet.png"), false).unwrap(),
        *atlas.sheet()
    );
    let bytes = fs::read(output.join("metadata.json")).unwrap();
    assert!(atlas.export(&output, 3).is_err());
    assert_eq!(fs::read(output.join("metadata.json")).unwrap(), bytes);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn animated_png_preserves_each_frame_pixels_and_timing() {
    let root = tempfile::tempdir().unwrap();
    let mut project = Document::from_canvas(sample(3, 2)).unwrap();
    project.add_frame("b".into(), None, 123).unwrap();
    let output = root.path().join("bundle");
    Atlas::from_document(&project, Options::default())
        .unwrap()
        .export(&output, 1)
        .unwrap();
    let mut reader = png::Decoder::new(std::io::BufReader::new(
        fs::File::open(output.join("animation.png")).unwrap(),
    ))
    .read_info()
    .unwrap();
    assert_eq!(reader.info().animation_control.unwrap().num_frames, 2);
    for index in 0..2 {
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        let expected: Vec<_> = project
            .flatten(index)
            .unwrap()
            .pixels()
            .iter()
            .flat_map(|p| [p.r, p.g, p.b, p.a])
            .collect();
        assert_eq!(&bytes[..info.buffer_size()], expected);
        let control = reader.info().frame_control.unwrap();
        assert_eq!(
            u32::from(control.delay_num) * 1000 / u32::from(control.delay_den),
            project.frames()[index].duration_ms
        );
    }
    assert!(storage::import_png(&output.join("animation.png"), false).is_err());
}

#[test]
fn impossible_atlas_and_preview_sizes_fail_without_output() {
    let root = tempfile::tempdir().unwrap();
    let project = Document::new(1, 1).unwrap();
    for columns in [0, 2, u32::MAX] {
        assert!(
            Atlas::from_document(
                &project,
                Options {
                    columns: Some(columns),
                    ..Options::default()
                }
            )
            .is_err()
        );
    }
    let atlas = Atlas::from_document(&project, Options::default()).unwrap();
    let output = root.path().join("bundle");
    assert!(atlas.export(&output, 0).is_err());
    assert!(!output.exists());
}

#[test]
fn tiles_preserve_ids_padding_and_explicit_connectivity() {
    let mut canvas = Canvas::new(4, 2).unwrap();
    canvas.set_pixel(1, 0, RED).unwrap();
    canvas.set_pixel(2, 0, RED).unwrap();
    let tiles = Tileset::slice(&canvas, 2, 2, 0, 0).unwrap();
    let rules = Rules {
        version: 1,
        connections: vec![Connection {
            from: 0,
            to: 1,
            edge: Edge::Right,
        }],
    };
    assert_eq!(tiles.seams(&rules).unwrap()[0].mismatched_pixels, 0);
    let atlas = tiles
        .atlas(
            Options {
                padding: 2,
                margin: 1,
                columns: Some(2),
            },
            Some(&rules),
        )
        .unwrap();
    assert_eq!(atlas.metadata().tileset.as_ref().unwrap().ids, vec![0, 1]);
    assert!(atlas.metadata().animation.is_none());
    let bad = Rules {
        version: 1,
        connections: vec![Connection {
            from: 1,
            to: 0,
            edge: Edge::Right,
        }],
    };
    assert_eq!(tiles.seams(&bad).unwrap()[0].mismatched_pixels, 0);
    let invalid = Rules {
        version: 1,
        connections: vec![Connection {
            from: 99,
            to: 0,
            edge: Edge::Down,
        }],
    };
    assert!(tiles.seams(&invalid).is_err());
}

#[test]
fn tiles_reject_partial_grids_and_detect_mismatched_edges() {
    assert!(Tileset::slice(&Canvas::new(5, 4).unwrap(), 2, 2, 0, 0).is_err());
    assert!(Tileset::slice(&Canvas::new(4, 4).unwrap(), 0, 2, 0, 0).is_err());
    let mut canvas = Canvas::new(4, 2).unwrap();
    canvas.set_pixel(1, 1, RED).unwrap();
    let tiles = Tileset::slice(&canvas, 2, 2, 0, 0).unwrap();
    let rules = Rules {
        version: 1,
        connections: vec![Connection {
            from: 0,
            to: 1,
            edge: Edge::Right,
        }],
    };
    assert_eq!(tiles.seams(&rules).unwrap()[0].mismatched_pixels, 1);
    let spaced = Tileset::slice(&Canvas::new(7, 4).unwrap(), 2, 2, 1, 1).unwrap();
    assert_eq!(spaced.len(), 2);
}

fn encode_png(
    path: &std::path::Path,
    color: png::ColorType,
    depth: png::BitDepth,
    bytes: &[u8],
    indexed: bool,
) {
    let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), 2, 1);
    encoder.set_color(color);
    encoder.set_depth(depth);
    if indexed {
        encoder.set_palette(vec![255, 0, 0, 0, 255, 0]);
        encoder.set_trns(vec![255, 64]);
    }
    encoder
        .write_header()
        .unwrap()
        .write_image_data(bytes)
        .unwrap();
}

#[test]
fn png_import_expands_gray_rgb_indexed_and_transparency() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("input.png");
    for (color, depth, bytes, indexed, expected) in [
        (
            png::ColorType::Rgb,
            png::BitDepth::Eight,
            vec![1, 2, 3, 4, 5, 6],
            false,
            vec![Pixel::rgba(1, 2, 3, 255), Pixel::rgba(4, 5, 6, 255)],
        ),
        (
            png::ColorType::GrayscaleAlpha,
            png::BitDepth::Eight,
            vec![20, 128, 30, 0],
            false,
            vec![Pixel::rgba(20, 20, 20, 128), Pixel::rgba(30, 30, 30, 0)],
        ),
        (
            png::ColorType::Grayscale,
            png::BitDepth::One,
            vec![0b0100_0000],
            false,
            vec![Pixel::rgba(0, 0, 0, 255), Pixel::rgba(255, 255, 255, 255)],
        ),
        (
            png::ColorType::Indexed,
            png::BitDepth::Eight,
            vec![0, 1],
            true,
            vec![RED, Pixel::rgba(0, 255, 0, 64)],
        ),
    ] {
        encode_png(&path, color, depth, &bytes, indexed);
        assert_eq!(
            storage::import_png(&path, false).unwrap().pixels(),
            expected
        );
    }
}

#[test]
fn png_16_bit_reduction_is_explicit() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("16.png");
    encode_png(
        &path,
        png::ColorType::Grayscale,
        png::BitDepth::Sixteen,
        &[0x12, 0x34, 0xab, 0xcd],
        false,
    );
    assert!(storage::import_png(&path, false).is_err());
    assert_eq!(
        storage::import_png(&path, true).unwrap().pixels(),
        vec![
            Pixel::rgba(0x12, 0x12, 0x12, 255),
            Pixel::rgba(0xab, 0xab, 0xab, 255)
        ]
    );
}

#[test]
fn terminal_and_text_writers_propagate_io_errors() {
    struct Failure;
    impl std::io::Write for Failure {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let canvas = Canvas::new(1, 1).unwrap();
    assert!(canvas.write_terminal(Failure).is_err());
    assert!(canvas.write_pixels(Failure).is_err());
    assert!(canvas.write_png(Failure).is_err());
}

#[test]
fn merge_rejects_byte_rounding_changes_without_mutating_the_document() {
    let mut project = Document::from_canvas(
        Canvas::from_pixels(1, 1, vec![Pixel::rgba(11, 33, 79, 255)]).unwrap(),
    )
    .unwrap();
    project.add_layer("middle".into()).unwrap();
    project
        .edit(&batch(json!([{"op":"clear","color":"#FF000001"}])))
        .unwrap();
    project.add_layer("top".into()).unwrap();
    project
        .edit(&batch(json!([{"op":"clear","color":"#00FF0001"}])))
        .unwrap();
    let original = project.clone();
    assert_eq!(
        project.merge_layer("top").unwrap_err().code.as_str(),
        "merge_changes_composite"
    );
    assert_eq!(project, original);
}

#[test]
fn polygon_edge_work_is_bounded_even_on_a_tiny_canvas() {
    let mut canvas = Canvas::new(1, 1).unwrap();
    let points: Vec<_> = (0..100)
        .map(|i| point(if i % 2 == 0 { -1_000_000 } else { 1_000_000 }, i))
        .collect();
    assert_eq!(
        drawing::polygon(&mut canvas, &points, RED, false, false)
            .unwrap_err()
            .code
            .as_str(),
        "resource_limit"
    );
    assert_eq!(canvas.pixel(0, 0), Some(Pixel::TRANSPARENT));
}

#[test]
fn bounded_input_sequences_reject_oversized_lists_and_duplicate_palette_keys() {
    assert!(
        serde_json::from_str::<Palette>(
            r##"{"version":1,"colors":{"ink":"#000000","ink":"#FFFFFF"}}"##
        )
        .is_err()
    );
    assert!(serde_json::from_value::<Tag>(json!({"frames":vec![0;257]})).is_err());
    assert!(serde_json::from_value::<Batch>(json!({"version":1,"operations":[{"op":"polygon","points":vec![json!({"x":0,"y":0});257],"color":"#000000"}]})).is_err());
    assert!(serde_json::from_value::<Batch>(json!({"version":1,"operations":[{"op":"noise","x":0,"y":0,"width":1,"height":1,"colors":vec!["#000000";257]}]})).is_err());
}

#[test]
fn polygons_share_the_batch_budget_and_roll_back_on_the_second_shape() {
    use painter::{ErrorCode, operations::Operation, palette::Color};
    let mut canvas = Canvas::new(512, 512).unwrap();
    let original = canvas.clone();
    // Each outline charges 512*512*128 pixel/edge tests plus 128 boundary
    // samples: individually allowed, but two exceed the 67,108,864 batch cap.
    let points = vec![point(0, 0); 128];
    let mut direct = canvas.clone();
    drawing::polygon(&mut direct, &points, RED, false, false).unwrap();
    assert_eq!(direct.pixel(0, 0), Some(RED));
    let shape = Operation::Polygon {
        points,
        color: Color::Rgba(RED),
        fill: false,
        over: false,
    };
    let error = Batch::new(vec![shape.clone(), shape])
        .apply(&mut canvas)
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(error.operation, Some(1));
    assert_eq!(canvas, original);
}

#[test]
fn alpha_compositing_preserves_all_nonzero_source_alpha_over_transparency() {
    for alpha in 1..=255 {
        for channel in [0, 1, 63, 127, 128, 200, 254, 255] {
            let pixel = Pixel::rgba(channel, 255 - channel, channel / 2, alpha);
            assert_eq!(pixel.over(Pixel::TRANSPARENT, 255), pixel);
        }
    }
}

#[test]
fn zero_operation_batch_is_idempotent_and_reports_no_changes() {
    let mut canvas = sample(3, 2);
    let before = canvas.clone();
    let summary = Batch::new(vec![]).apply(&mut canvas).unwrap();
    assert_eq!(canvas, before);
    assert_eq!(summary.changed_pixels, 0);
    assert_eq!(summary.operations, 0);
}

#[test]
fn malformed_png_never_becomes_a_canvas() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("bad.png");
    for bytes in [b"not a PNG".as_slice(), b"\x89PNG\r\n\x1a\n".as_slice()] {
        fs::write(&path, bytes).unwrap();
        assert_eq!(
            storage::import_png(&path, false).unwrap_err().code.as_str(),
            "invalid_png"
        );
    }
}

#[test]
fn atlas_metadata_is_stable_across_independent_exports() {
    let root = tempfile::tempdir().unwrap();
    let mut project = Document::from_canvas(sample(3, 2)).unwrap();
    project.add_frame("b".into(), Some(0), 123).unwrap();
    let atlas = Atlas::from_document(&project, Options::default()).unwrap();
    for path in ["one", "two"] {
        atlas.export(&root.path().join(path), 2).unwrap();
    }
    for file in [
        "metadata.json",
        "sheet.png",
        "preview.png",
        "animation.png",
        "frames/0000.png",
        "frames/0001.png",
    ] {
        assert_eq!(
            fs::read(root.path().join("one").join(file)).unwrap(),
            fs::read(root.path().join("two").join(file)).unwrap()
        );
    }
}

#[test]
fn legacy_pixel_operation_accepts_valid_coordinates_beyond_shape_limits() {
    let mut canvas = Canvas::new(1_000_002, 1).unwrap();
    apply(
        &mut canvas,
        json!([{"op":"set_pixel","x":1_000_001,"y":0,"color":"#FF0000"}]),
    );
    assert_eq!(canvas.pixel(1_000_001, 0), Some(RED));
}
