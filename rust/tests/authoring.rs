// SPDX-License-Identifier: GPL-3.0-only
//! Cross-feature authoring contracts and hostile-input boundaries.

use painter::{
    Canvas, Document, Pixel,
    assets::{self, AssetRoot},
    atlas::{Atlas, Options},
    drawing::Point,
    geometry::{Faces, Shape},
    operations::Batch,
    palette::Color,
    placement::{Anchors, Part, Placement},
    recipe::{Parameters, Recipe},
    scene::{Entry, Scene},
    shadow, storage,
    style::Profile,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const RED: Pixel = Pixel::rgba(200, 40, 30, 255);
const BLUE: Pixel = Pixel::rgba(30, 50, 190, 255);
const GREEN: Pixel = Pixel::rgba(30, 150, 50, 255);
fn profile() -> Profile {
    serde_json::from_value(json!({"version":1,"name":"generic","palette":{"version":1,"colors":{"outline":"#000000","top":"#FFFFFF","front":"#C8281E","side":"#1E32BE","highlight":"#1E9632","shadow":"#00000080"}},"roles":{"outline":"outline","top":"top","front":"front","side":"side","highlight":"highlight","shadow":"shadow"}})).unwrap()
}
fn recipe_value() -> Value {
    json!({"version":2,"width":8,"height":8,"parameters":{"width":{"type":"integer","default":3,"min":2,"max":5},"ink":{"type":"color","default":"#C8281E"}},"operations":[{"op":"fill_rect","x":1,"y":1,"width":{"param":"width"},"height":2,"color":{"param":"ink"}}]})
}
fn render_recipe(
    value: Value,
    parameters: &Parameters,
    variant: Option<&str>,
    seed: Option<u64>,
    style: Option<&Profile>,
) -> painter::Result<Canvas> {
    serde_json::from_value::<Recipe>(value)?.render(parameters, variant, seed, style)
}
fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
fn shared_style_reports_measurable_violations_and_preserves_inputs() {
    let mut style = profile();
    let image = Canvas::from_pixels(2, 1, vec![RED, Pixel::rgba(12, 13, 14, 128)]).unwrap();
    style.max_colors = 1;
    style.alpha = painter::style::AlphaPolicy::Binary;
    style.width = Some(painter::style::Range {
        min: 3,
        max: 8,
    });
    let report = style.check(&image, "sprite").unwrap();
    assert!(!report.valid);
    for code in ["dimensions", "palette", "color_count", "partial_alpha"] {
        assert!(
            report.issues.iter().any(|issue| issue.code == code),
            "{code}"
        );
    }
    assert_eq!(image.pixel(0, 0), Some(RED));
    assert_eq!(style.reference().unwrap().width(), 128);
    style.roles.remove("top");
    assert!(style.validate().is_err());
}
#[test]
fn outlines_check_boundary_rings_without_rejecting_interior_colors() {
    let mut style = profile();
    style.outline_thickness = 1;
    let black = Pixel::rgba(0, 0, 0, 255);
    let mut image = Canvas::from_pixels(3, 3, vec![black; 9]).unwrap();
    image.set_pixel(1, 1, RED).unwrap();
    assert!(style.check(&image, "outlined").unwrap().valid);
    image.set_pixel(0, 1, RED).unwrap();
    let report = style.check(&image, "damaged").unwrap();
    assert!(report.issues.iter().any(|issue| issue.code == "outline"));
}
#[test]
fn face_assemblies_have_exact_top_front_and_side_pixels() {
    let shape = Shape::Prism {
        x: 1,
        y: 2,
        width: 3,
        height: 3,
        depth: 1,
        faces: Faces {
            top: Color::Rgba(GREEN),
            front: Color::Rgba(RED),
            side: Color::Rgba(BLUE),
            outline: None,
        },
    };
    let mut image = Canvas::new(8, 8).unwrap();
    Batch::new(shape.operations().unwrap())
        .apply(&mut image)
        .unwrap();
    assert_eq!(image.pixel(2, 1), Some(GREEN));
    assert_eq!(image.pixel(1, 3), Some(RED));
    assert_eq!(image.pixel(4, 2), Some(BLUE));
    let bad = Shape::Steps {
        x: 0,
        y: 0,
        width: 3,
        height: 3,
        depth: 1,
        count: 33,
        faces: Faces {
            top: Color::Rgba(GREEN),
            front: Color::Rgba(RED),
            side: Color::Rgba(BLUE),
            outline: None,
        },
    };
    assert!(bad.operations().is_err());
}
#[test]
fn parameters_are_bounded_typed_and_deterministic() {
    let value = recipe_value();
    let original = render_recipe(value.clone(), &Parameters::new(), None, None, None).unwrap();
    let overrides = BTreeMap::from([("width".into(), json!(5)), ("ink".into(), json!("#1E32BE"))]);
    let large = render_recipe(value.clone(), &overrides, None, Some(8), None).unwrap();
    assert_eq!(original.pixel(4, 1), Some(Pixel::TRANSPARENT));
    assert_eq!(large.pixel(5, 1), Some(BLUE));
    assert_eq!(
        large,
        render_recipe(value.clone(), &overrides, None, Some(8), None).unwrap()
    );
    for parameters in [
        BTreeMap::from([("width".into(), json!(6))]),
        BTreeMap::from([("width".into(), json!("3"))]),
        BTreeMap::from([("unknown".into(), json!(3))]),
    ] {
        assert!(render_recipe(value.clone(), &parameters, None, None, None).is_err());
    }
    assert!(render_recipe(value, &Parameters::new(), Some("missing"), None, None).is_err());
}
#[test]
fn recipes_enforce_palette_policy_after_parameter_expansion() {
    let mut value = json!({"version":2,"width":1,"height":1,"palette":{"version":1,"colors":{"ink":"#C8281E"}},"parameters":{"ink":{"type":"color","default":"#C9281E"}},"operations":[{"op":"clear","color":{"param":"ink"}}]});
    let parameters = Parameters::new();
    assert_eq!(
        render_recipe(value.clone(), &parameters, None, None, None)
            .unwrap()
            .pixel(0, 0),
        Some(Pixel::rgba(201, 40, 30, 255))
    );
    value["policy"] = json!("strict");
    assert_eq!(
        render_recipe(value.clone(), &parameters, None, None, None)
            .unwrap_err()
            .code,
        painter::ErrorCode::PaletteViolation
    );
    value["policy"] = json!("nearest");
    assert_eq!(
        render_recipe(value.clone(), &parameters, None, None, None)
            .unwrap()
            .pixels(),
        &[RED]
    );
    value["palette"] = Value::Null;
    assert!(render_recipe(value.clone(), &parameters, None, None, None).is_err());
    assert_eq!(
        render_recipe(value, &parameters, None, None, Some(&profile()))
            .unwrap()
            .pixels(),
        &[RED]
    );
}
#[test]
fn reusable_parts_flip_and_substitute_without_cascading() {
    let part = Canvas::from_pixels(2, 1, vec![RED, GREEN]).unwrap();
    let value = json!({"version":2,"width":3,"height":2,"parts":{"panel":part},"operations":[{"op":"component","name":"panel","x":0,"y":0,"flip_x":true,"substitutions":[{"from":"#1E9632","to":"#1E32BE"},{"from":"#1E32BE","to":"#FFFFFF"}]}]});
    let image = render_recipe(value, &Parameters::new(), None, None, None).unwrap();
    assert_eq!(image.pixel(0, 0), Some(BLUE));
    assert_eq!(image.pixel(1, 0), Some(RED));
    assert_eq!(part.pixels(), &[RED, GREEN]);
}
#[test]
fn cast_shadow_combines_coverage_once_and_ignores_hidden_rgb() {
    let source = Canvas::from_pixels(2, 1, vec![RED, Pixel::rgba(250, 40, 80, 0)]).unwrap();
    let options = shadow::Options {
        width: 4,
        height: 2,
        origin: Point {
            x: 1,
            y: 0,
        },
        step: Point {
            x: 0,
            y: 0,
        },
        steps: 5,
        squash_y: 1,
        color: Pixel::rgba(0, 0, 0, 128),
    };
    let image = shadow::project(&source, options).unwrap();
    assert_eq!(image.pixel(1, 0), Some(Pixel::rgba(0, 0, 0, 128)));
    assert_eq!(image.pixel(2, 0), Some(Pixel::TRANSPARENT));
    assert!(
        shadow::project(
            &source,
            shadow::Options {
                steps: 65,
                ..options
            }
        )
        .is_err()
    );
}
fn entry(id: &str, input: &str, order: i32) -> Entry {
    Entry {
        id: id.into(),
        input: input.into(),
        position: Point {
            x: 2,
            y: 2,
        },
        scale: 1,
        order,
        anchor: Some(Point {
            x: 0,
            y: 0,
        }),
        frame: 0,
        placement: None,
        part: None,
    }
}
#[test]
fn offline_scene_order_anchors_and_transparency_are_exact() {
    let root = tempfile::tempdir().unwrap();
    storage::export_png(
        &root.path().join("red.png"),
        &Canvas::from_pixels(1, 1, vec![RED]).unwrap(),
    )
    .unwrap();
    storage::export_png(
        &root.path().join("blue.png"),
        &Canvas::from_pixels(1, 1, vec![BLUE]).unwrap(),
    )
    .unwrap();
    let assets = AssetRoot::new(root.path()).unwrap();
    let mut scene = Scene {
        version: 1,
        width: 4,
        height: 4,
        background: Pixel::TRANSPARENT,
        tile: None,
        y_sort: true,
        assets: vec![entry("red", "red.png", 0), entry("blue", "blue.png", 0)],
    };
    assert_eq!(scene.render(&assets, 1).unwrap().pixel(2, 2), Some(BLUE));
    scene.assets[0].order = 1;
    assert_eq!(scene.render(&assets, 1).unwrap().pixel(2, 2), Some(RED));
    scene.assets[0].scale = 2;
    scene.assets[0].anchor = Some(Point {
        x: 1,
        y: 1,
    });
    let image = scene.render(&assets, 1).unwrap();
    assert_eq!(image.pixel(0, 0), Some(RED));
    assert_eq!(
        scene
            .contact_sheet(&assets, Some(2), 1, 2)
            .unwrap()
            .height(),
        8
    );
}
#[test]
fn asset_references_reject_traversal_absolute_paths_and_platform_prefixes() {
    let directory = tempfile::tempdir().unwrap();
    let root = AssetRoot::new(directory.path()).unwrap();
    for path in [
        "../private.png",
        "/tmp/private.png",
        "C:/private.png",
        "..\\private.png",
        "./image.png",
        "",
    ] {
        assert!(root.resolve(Path::new(path)).is_err(), "{path}");
    }
}
#[cfg(unix)]
#[test]
fn asset_references_cannot_follow_symlinks_outside_the_root() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), directory.path().join("link.json")).unwrap();
    assert!(
        AssetRoot::new(directory.path())
            .unwrap()
            .resolve(Path::new("link.json"))
            .is_err()
    );
}
#[test]
fn split_exports_publish_flattened_frames_and_real_parts() {
    let root = tempfile::tempdir().unwrap();
    let mut document = Document::new(2, 2).unwrap();
    document
        .edit(&Batch::new(vec![painter::operations::Operation::Clear {
            color: Some(Color::Rgba(RED)),
        }]))
        .unwrap();
    document.add_layer("canopy".into()).unwrap();
    document
        .edit(&Batch::new(vec![painter::operations::Operation::Clear {
            color: Some(Color::Rgba(BLUE)),
        }]))
        .unwrap();
    let placement = Placement {
        version: 1,
        anchors: Anchors {
            ground: Some(Point {
                x: 1,
                y: 1,
            }),
            ..Anchors::default()
        },
        frames: BTreeMap::new(),
        parts: vec![
            Part {
                id: "trunk".into(),
                layers: vec!["base".into()],
                order: 0,
            },
            Part {
                id: "canopy".into(),
                layers: vec!["canopy".into()],
                order: 1,
            },
        ],
    };
    let atlas = Atlas::from_document(&document, Options::default()).unwrap();
    let split = root.path().join("split");
    atlas
        .export_with_placement(&split, 1, &document, &placement)
        .unwrap();
    assert_eq!(
        storage::import_png(&split.join("frames/0000.png"), false).unwrap(),
        document.flatten(0).unwrap()
    );
    assert_eq!(
        storage::import_png(&split.join("parts/0000/0000.png"), false)
            .unwrap()
            .pixel(0, 0),
        Some(RED)
    );
    assert_eq!(
        storage::import_png(&split.join("parts/0001/0000.png"), false)
            .unwrap()
            .pixel(0, 0),
        Some(BLUE)
    );
    let metadata: painter::placement::Metadata =
        assets::read_config(&split.join("placement.json")).unwrap();
    assert_eq!(
        metadata.frames[0].anchors.ground,
        Some(Point {
            x: 1,
            y: 1
        })
    );
    assert!(
        atlas
            .export_with_placement(&split, 1, &document, &placement)
            .is_err()
    );
}
#[test]
fn animation_validation_detects_drift_and_honors_named_exceptions() {
    let mut document = Document::new(8, 8).unwrap();
    document
        .configure_frame(
            0,
            None,
            Some(Point {
                x: 4,
                y: 6,
            }),
        )
        .unwrap();
    document.add_frame("jump".into(), Some(0), 100).unwrap();
    document
        .configure_frame(
            1,
            None,
            Some(Point {
                x: 4,
                y: 2,
            }),
        )
        .unwrap();
    let mut style = profile();
    style.animation.required_tags = vec!["walk-south".into()];
    let report = painter::animation_check::check(&document, &style, None).unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "ground_drift")
    );
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "missing_tag")
    );
    style.animation.required_tags.clear();
    style.animation.movement_exceptions = vec!["jump".into()];
    assert!(
        painter::animation_check::check(&document, &style, None)
            .unwrap()
            .valid
    );
}
#[test]
fn animation_motion_exceptions_do_not_set_the_proportion_reference() {
    let mut document =
        Document::from_canvas(Canvas::from_pixels(8, 8, vec![RED; 64]).unwrap()).unwrap();
    let mut style = profile();
    style.animation.max_size_delta = Some(0);
    style.animation.movement_exceptions = vec![document.frames()[0].name.clone()];
    document.add_frame("idle".into(), None, 100).unwrap();
    document
        .edit(&Batch::new(vec![painter::operations::Operation::Stamp {
            canvas: Canvas::from_pixels(1, 1, vec![RED]).unwrap(),
            x: 2,
            y: 2,
            over: false,
        }]))
        .unwrap();
    document.add_frame("idle-2".into(), Some(1), 100).unwrap();
    let report = painter::animation_check::check(&document, &style, None).unwrap();
    assert!(
        report
            .issues
            .iter()
            .all(|issue| issue.code != "proportion_drift")
    );
    document
        .edit(&Batch::new(vec![painter::operations::Operation::Stamp {
            canvas: Canvas::from_pixels(2, 1, vec![RED; 2]).unwrap(),
            x: 2,
            y: 2,
            over: false,
        }]))
        .unwrap();
    let report = painter::animation_check::check(&document, &style, None).unwrap();
    let drift: Vec<_> = report
        .issues
        .iter()
        .filter(|issue| issue.code == "proportion_drift")
        .collect();
    assert_eq!(drift.len(), 1);
    assert_eq!(drift[0].asset, "idle-2");
}

#[test]
fn placement_limits_apply_to_resolved_attachment_maps() {
    let document = Document::new(2, 2).unwrap();
    let frame_name = document.frames()[0].name.clone();
    let mut placement = Placement {
        version: 1,
        anchors: Anchors {
            attachments: (0..32)
                .map(|i| {
                    (
                        format!("anchor-{i}"),
                        Point {
                            x: i,
                            y: 0,
                        },
                    )
                })
                .collect(),
            ..Anchors::default()
        },
        frames: BTreeMap::from([(
            frame_name.clone(),
            Anchors {
                attachments: BTreeMap::from([(
                    "extra".into(),
                    Point {
                        x: 1,
                        y: 1,
                    },
                )]),
                ..Anchors::default()
            },
        )]),
        parts: Vec::new(),
    };
    placement.anchors.validate().unwrap();
    placement.frames[&frame_name].validate().unwrap();
    assert!(placement.validate(&document).is_err());
    assert!(placement.frame_anchors(&document, 0).is_err());
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("export");
    let atlas = Atlas::from_document(&document, Options::default()).unwrap();
    assert!(
        atlas
            .export_with_placement(&output, 1, &document, &placement)
            .is_err()
    );
    assert!(!output.exists());

    placement.frames.get_mut(&frame_name).unwrap().attachments = BTreeMap::from([(
        "anchor-0".into(),
        Point {
            x: 1,
            y: 1,
        },
    )]);
    placement.validate(&document).unwrap();
    let anchors = placement.frame_anchors(&document, 0).unwrap();
    assert_eq!(anchors.attachments.len(), 32);
    assert_eq!(
        anchors.attachments["anchor-0"],
        Point {
            x: 1,
            y: 1
        }
    );
    atlas
        .export_with_placement(&output, 1, &document, &placement)
        .unwrap();
    let metadata: painter::placement::Metadata =
        assets::read_config(&output.join("placement.json")).unwrap();
    metadata.validate().unwrap();
}

#[test]
fn pack_generation_is_repeatable_and_detects_missing_exports() {
    let root = tempfile::tempdir().unwrap();
    write(&root.path().join("crate.json"), &recipe_value());
    let recipe:painter::pack::Recipe=serde_json::from_value(json!({"version":1,"assets":[{"id":"crate","source":{"kind":"recipe","input":"crate.json"}}]})).unwrap();
    let assets = AssetRoot::new(root.path()).unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    assert!(recipe.generate(&assets, &first).unwrap().valid);
    assert!(recipe.generate(&assets, &second).unwrap().valid);
    for name in [
        "pack.json",
        "report.json",
        "contact-sheet.png",
        "assets/0000/source.painter.json",
        "assets/0000/export/sheet.png",
    ] {
        assert_eq!(
            fs::read(first.join(name)).unwrap(),
            fs::read(second.join(name)).unwrap(),
            "{name}"
        );
    }
    assert!(painter::pack::check(&first, None).unwrap().valid);
    fs::remove_file(first.join("assets/0000/export/frames/0000.png")).unwrap();
    assert!(!painter::pack::check(&first, None).unwrap().valid);
    assert!(recipe.generate(&assets, &first).is_err());
}
#[test]
fn authoring_configs_are_bounded_and_unknown_fields_fail() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("huge.json");
    fs::File::create(&path)
        .unwrap()
        .set_len(assets::MAX_CONFIG_BYTES + 1)
        .unwrap();
    assert_eq!(
        assets::read_config::<Value>(&path).unwrap_err().code,
        painter::ErrorCode::ResourceLimit
    );
    let mut bad = recipe_value();
    bad["typo"] = json!(true);
    assert!(serde_json::from_value::<painter::recipe::Recipe>(bad).is_err());
    let mut bad = serde_json::to_value(profile()).unwrap();
    bad["version"] = json!(7);
    assert!(
        serde_json::from_value::<Profile>(bad)
            .unwrap()
            .validate()
            .is_err()
    );
}

#[test]
fn placement_indices_stale_sources_and_duplicate_metadata_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let mut document = Document::new(2, 2).unwrap();
    let placement: Placement = serde_json::from_value(json!({"version":1})).unwrap();
    assert!(placement.frame_anchors(&document, usize::MAX).is_err());
    let atlas = Atlas::from_document(&document, Options::default()).unwrap();
    document
        .edit(&Batch::new(vec![painter::operations::Operation::Clear {
            color: Some(Color::Rgba(RED)),
        }]))
        .unwrap();
    let output = directory.path().join("stale");
    assert!(
        atlas
            .export_with_placement(&output, 1, &document, &placement)
            .is_err()
    );
    assert!(!output.exists());
    let metadata: painter::placement::Metadata = serde_json::from_value(json!({"version":1,"width":2,"height":2,"frames":[{"name":"same","anchors":{}},{"name":"same","anchors":{}}],"parts":[]})).unwrap();
    assert!(metadata.validate().is_err());
}

#[test]
fn scene_uses_project_frame_and_exported_part_order() {
    let directory = tempfile::tempdir().unwrap();
    let mut document =
        Document::from_canvas(Canvas::from_pixels(2, 2, vec![RED; 4]).unwrap()).unwrap();
    document.add_frame("blue".into(), None, 100).unwrap();
    document
        .edit(&Batch::new(vec![painter::operations::Operation::Clear {
            color: Some(Color::Rgba(BLUE)),
        }]))
        .unwrap();
    storage::save_project(&directory.path().join("source.json"), &document, true).unwrap();
    let placement: Placement = serde_json::from_value(
        json!({"version":1,"parts":[{"id":"all","layers":["base"],"order":2}]}),
    )
    .unwrap();
    Atlas::from_document(&document, Options::default())
        .unwrap()
        .export_with_placement(&directory.path().join("export"), 1, &document, &placement)
        .unwrap();
    let root = AssetRoot::new(directory.path()).unwrap();
    let mut sprite = entry("sprite", "source.json", 0);
    sprite.frame = 1;
    sprite.placement = Some("export/placement.json".into());
    let mut scene = Scene {
        version: 1,
        width: 4,
        height: 4,
        background: Pixel::TRANSPARENT,
        tile: None,
        y_sort: false,
        assets: vec![sprite.clone()],
    };
    assert_eq!(scene.render(&root, 1).unwrap().pixel(2, 2), Some(BLUE));
    scene.assets[0].input = "export/frames/0001.png".into();
    assert_eq!(scene.render(&root, 1).unwrap().pixel(2, 2), Some(BLUE));
    scene.assets[0].input = "source.json".into();
    sprite.id = "part".into();
    sprite.part = Some("all".into());
    scene.assets.insert(0, sprite);
    scene.assets[1].frame = 0;
    scene.assets[1].order = 1;
    assert_eq!(scene.render(&root, 1).unwrap().pixel(2, 2), Some(BLUE));
}

#[test]
fn invalid_unselected_variants_and_deep_expansion_fail() {
    let mut value = recipe_value();
    value["variants"] = json!({"broken":{"width":999}});
    assert!(render_recipe(value, &Parameters::new(), None, None, None).is_err());
    let mut value = recipe_value();
    let mut nested = json!(0);
    for _ in 0..40 {
        nested = json!([nested]);
    }
    value["operations"] = json!([nested]);
    assert_eq!(
        render_recipe(value, &Parameters::new(), None, None, None)
            .unwrap_err()
            .code,
        painter::ErrorCode::ResourceLimit
    );
}

#[test]
fn large_outline_checks_fail_before_expensive_scanning() {
    let mut profile = profile();
    profile.outline_thickness = 4;
    let image = Canvas::new(2000, 2000).unwrap();
    assert_eq!(
        profile.check(&image, "large").unwrap_err().code,
        painter::ErrorCode::ResourceLimit
    );
}

#[test]
fn inferred_baselines_also_check_earlier_unanchored_frames() {
    let mut document = Document::new(8, 8).unwrap();
    document.add_frame("anchored".into(), None, 100).unwrap();
    document
        .configure_frame(
            1,
            None,
            Some(Point {
                x: 4,
                y: 6,
            }),
        )
        .unwrap();
    let report = painter::animation_check::check(&document, &profile(), None).unwrap();
    assert!(!report.valid);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "missing_ground" && issue.asset == "frame-0")
    );
}

#[test]
fn reports_cap_details_without_hiding_errors_or_omissions() {
    let mut report = painter::validation::Report::default();
    for _ in 0..4096 {
        report.warning("detail", "sprite", "warning");
    }
    report.error("failure", "sprite", "error beyond detail cap");
    assert!(!report.valid);
    assert_eq!(report.issues.len(), 4096);
    assert_eq!(report.omitted, 1);
    let mut combined = painter::validation::Report::default();
    combined.extend(report);
    assert!(!combined.valid);
    assert_eq!(combined.omitted, 1);
}

#[test]
fn failed_pack_profile_validation_never_publishes_a_partial_bundle() {
    let directory = tempfile::tempdir().unwrap();
    storage::write_json(&directory.path().join("profile.json"), &profile(), true).unwrap();
    let mut value = recipe_value();
    value["parameters"]["ink"]["default"] = json!("#010203");
    write(&directory.path().join("bad.json"), &value);
    let recipe:painter::pack::Recipe=serde_json::from_value(json!({"version":1,"profile":"profile.json","assets":[{"id":"bad","source":{"kind":"recipe","input":"bad.json"}}]})).unwrap();
    let output = directory.path().join("output");
    assert!(
        recipe
            .generate(&AssetRoot::new(directory.path()).unwrap(), &output)
            .is_err()
    );
    assert!(!output.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[test]
fn seeded_recipe_operations_reproduce_and_change_with_explicit_seed() {
    let value = json!({"version":2,"width":8,"height":8,"seed":47,"operations":[{"op":"noise","x":0,"y":0,"width":8,"height":8,"colors":["#FF0000","#0000FF"]}]});
    let render = |seed| render_recipe(value.clone(), &Parameters::new(), None, seed, None).unwrap();
    assert_eq!(render(None), render(Some(47)));
    assert_ne!(render(Some(47)), render(Some(48)));
}

#[test]
fn pack_checks_timing_animation_previews_placement_and_completion_files() {
    let directory = tempfile::tempdir().unwrap();
    let mut document =
        Document::from_canvas(Canvas::from_pixels(2, 2, vec![RED; 4]).unwrap()).unwrap();
    document.add_frame("second".into(), Some(0), 160).unwrap();
    storage::save_project(&directory.path().join("source.json"), &document, true).unwrap();
    write(
        &directory.path().join("placement.json"),
        &json!({"version":1,"parts":[{"id":"body","layers":["base"]}]}),
    );
    let recipe:painter::pack::Recipe=serde_json::from_value(json!({"version":1,"assets":[{"id":"sprite","source":{"kind":"artwork","input":"source.json"},"placement":"placement.json"}]})).unwrap();
    let root = AssetRoot::new(directory.path()).unwrap();
    let output = directory.path().join("pack");
    recipe.generate(&root, &output).unwrap();
    for file in [
        "assets/0000/export/parts/0000/0000.png",
        "assets/0000/export/preview.png",
        "assets/0000/export/animation.png",
        "assets/0000/export/metadata.json",
        "assets/0000/export/placement.json",
    ] {
        let path = output.join(file);
        let original = fs::read(&path).unwrap();
        fs::write(&path, b"corrupt").unwrap();
        assert!(
            !painter::pack::check(&output, None).unwrap().valid,
            "{file}"
        );
        fs::write(&path, original).unwrap();
    }
    let path = output.join("assets/0000/export/metadata.json");
    let original = fs::read(&path).unwrap();
    let mut metadata: Value = serde_json::from_slice(&original).unwrap();
    metadata["frames"][1]["duration_ms"] = json!(161);
    write(&path, &metadata);
    assert!(!painter::pack::check(&output, None).unwrap().valid);
    fs::write(&path, original).unwrap();
    fs::remove_file(output.join("contact-sheet.png")).unwrap();
    assert!(painter::pack::check(&output, None).is_err());
}
