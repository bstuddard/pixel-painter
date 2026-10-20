// SPDX-License-Identifier: GPL-3.0-only
//! Verify that review artifacts remain loadable, reproducible, and internally consistent.

use painter::{
    atlas::Metadata,
    drawing,
    recipe::{Parameters, Recipe},
    storage,
    tileset::{Rules, Tileset},
};
use std::path::PathBuf;

fn gallery() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/gallery")
}

#[test]
fn checked_in_layered_assets_match_exported_frames_and_metadata() {
    for (folder, filename) in [
        ("01-crystal-keeper", "project.painter.json"),
        ("02-moss-cottage", "project.painter.json"),
        ("04-seeded-slimes", "variants.painter.json"),
    ] {
        let root = gallery().join(folder);
        let project = storage::load_project(&root.join(filename)).unwrap();
        let metadata: Metadata = storage::read_json(&root.join("export/metadata.json")).unwrap();
        assert_eq!(metadata.version, 1);
        assert_eq!(metadata.frames.len(), project.frames().len());
        assert_eq!(&metadata.tags, project.tags());
        let sheet = storage::import_png(&root.join("export").join(&metadata.image), false).unwrap();
        assert_eq!(
            (sheet.width(), sheet.height()),
            (metadata.width, metadata.height)
        );
        for (index, frame) in metadata.frames.iter().enumerate() {
            let expected = project.flatten(index).unwrap();
            assert_eq!(
                drawing::crop(&sheet, frame.rect).unwrap(),
                expected,
                "{folder} frame {index}"
            );
            assert_eq!(
                storage::import_png(&root.join("export").join(&frame.file), false).unwrap(),
                expected
            );
            assert_eq!(frame.duration_ms, project.frames()[index].duration_ms);
            assert_eq!(frame.pivot, project.frames()[index].pivot);
        }
        assert_eq!(
            storage::import_png(&root.join("native.png"), false).unwrap(),
            project.flatten(0).unwrap()
        );
    }
}

#[test]
fn checked_in_templates_reproduce_every_seeded_canvas() {
    let root = gallery().join("04-seeded-slimes");
    let template: Recipe = painter::assets::read_config(&root.join("slime.template.json")).unwrap();
    for seed in [10, 21, 42, 73, 99, 128] {
        assert_eq!(
            template
                .render(&Parameters::new(), None, Some(seed), None)
                .unwrap(),
            storage::load(&root.join(format!("slime-{seed}.json"))).unwrap()
        );
    }
    let terrain = gallery().join("03-forest-tiles");
    let template: Recipe =
        painter::assets::read_config(&terrain.join("terrain.template.json")).unwrap();
    assert_eq!(
        template
            .render(&Parameters::new(), None, None, None)
            .unwrap(),
        storage::load(&terrain.join("terrain.json")).unwrap()
    );
}

#[test]
fn checked_in_terrain_connections_all_have_exact_matching_edges() {
    let root = gallery().join("03-forest-tiles");
    let canvas = storage::load(&root.join("terrain.json")).unwrap();
    let rules: Rules = storage::read_json(&root.join("connections.json")).unwrap();
    let tiles = Tileset::slice(&canvas, 16, 16, 0, 0).unwrap();
    assert_eq!(tiles.len(), 8);
    let seams = tiles.seams(&rules).unwrap();
    assert_eq!(seams.len(), 16);
    assert!(seams.iter().all(|s| s.mismatched_pixels == 0));
}
