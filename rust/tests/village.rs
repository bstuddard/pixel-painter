// SPDX-License-Identifier: GPL-3.0-only
//! Validate real review artifacts through the same public API used by the CLI.

use painter::{
    assets::{self, AssetRoot},
    pack,
    scene::Scene,
    storage,
    style::Profile,
};
use std::path::PathBuf;

fn village() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/village")
}

#[test]
fn checked_in_village_pack_is_complete_and_its_scene_reproduces() {
    let directory = village();
    let report = pack::check(&directory.join("export"), None).unwrap();
    assert!(report.valid, "{:?}", report.issues);
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let scene: Scene = assets::read_config(&directory.join("scene.json")).unwrap();
    assert_eq!(
        scene
            .render(&AssetRoot::new(&directory).unwrap(), 4)
            .unwrap(),
        storage::import_png(&directory.join("scene.png"), false).unwrap()
    );
}

#[test]
fn directional_character_has_idle_and_walk_frames_and_passes_qa() {
    let directory = village();
    let document =
        storage::load_project(&directory.join("character/project.painter.json")).unwrap();
    let profile: Profile = assets::read_config(&directory.join("profile.json")).unwrap();
    assert_eq!(document.frames().len(), 12);
    for direction in ["south", "east", "north", "west"] {
        assert_eq!(
            document.tags()[&format!("idle-{direction}")].frames.len(),
            1
        );
        assert_eq!(
            document.tags()[&format!("walk-{direction}")].frames.len(),
            2
        );
    }
    let report = painter::animation_check::check(&document, &profile, None).unwrap();
    assert!(report.valid);
    assert!(report.issues.is_empty(), "{:?}", report.issues);
}
