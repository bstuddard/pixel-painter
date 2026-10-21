// SPDX-License-Identifier: GPL-3.0-only
#![cfg(feature = "cli")]
use painter::{Pixel, storage};
use serde_json::{Value, json};
use std::{
    ffi::OsStr,
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_painter"))
        .arg("--json")
        .args(args)
        .output()
        .unwrap()
}
fn ok(args: &[&OsStr]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn s(value: &str) -> &OsStr {
    OsStr::new(value)
}
fn new_project(path: &Path) {
    ok(&[
        s("project"),
        s("new"),
        path.as_os_str(),
        s("--width"),
        s("8"),
        s("--height"),
        s("8"),
    ]);
    ok(&[
        s("--file"),
        path.as_os_str(),
        s("draw"),
        s("clear"),
        s("--color"),
        s("#C8281E"),
    ]);
}
#[test]
fn cli_channels_export_and_legacy_png_keep_color_separate() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project.json");
    new_project(&project);
    for (name, channel, color) in [
        ("glow", "emissive", "#20C08080"),
        ("normals", "normal", "#8080FF"),
    ] {
        ok(&[
            s("project"),
            s("layer"),
            project.as_os_str(),
            s("add"),
            s(name),
            s("--channel"),
            s(channel),
        ]);
        ok(&[
            s("--file"),
            project.as_os_str(),
            s("draw"),
            s("clear"),
            s("--color"),
            s(color),
        ]);
    }
    let output = root.path().join("export");
    ok(&[
        s("project"),
        s("export"),
        project.as_os_str(),
        s("--output"),
        output.as_os_str(),
        s("--maps"),
        s("emissive,normal"),
        s("--scale"),
        s("2"),
    ]);
    let metadata: Value =
        serde_json::from_slice(&fs::read(output.join("metadata.json")).unwrap()).unwrap();
    assert_eq!(metadata["maps"]["normal"]["positive_y"], "up");
    assert_eq!(metadata["maps"]["emissive"]["encoding"], "rgba8_emission");
    for name in ["normal/frames/0000.png", "emissive/frames/0000.png"] {
        assert!(output.join(name).is_file());
    }
    let image = root.path().join("legacy.png");
    ok(&[
        s("--file"),
        project.as_os_str(),
        s("export"),
        s("--output"),
        image.as_os_str(),
    ]);
    assert!(
        storage::import_png(&image, false)
            .unwrap()
            .pixels()
            .iter()
            .all(|pixel| *pixel == Pixel::rgba(200, 40, 30, 255))
    );
    let before = fs::read(output.join("metadata.json")).unwrap();
    assert!(
        !run(&[
            s("project"),
            s("export"),
            project.as_os_str(),
            s("--output"),
            output.as_os_str(),
            s("--maps"),
            s("normal")
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(output.join("metadata.json")).unwrap(), before);
}
#[test]
fn invalid_map_inputs_and_bad_normal_paint_preserve_files() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project.json");
    new_project(&project);
    let before = fs::read(&project).unwrap();
    for request in [
        "color",
        "normal,normal",
        "normal/../../private",
        "\u{1b}[31m",
    ] {
        let output = root.path().join("no-output");
        let result = run(&[
            s("project"),
            s("export"),
            project.as_os_str(),
            s("--output"),
            output.as_os_str(),
            s("--maps"),
            s(request),
        ]);
        assert!(!result.status.success());
        assert!(!output.exists());
        assert!(!result.stdout.contains(&0x1b) && !result.stderr.contains(&0x1b));
        let _: Value = serde_json::from_slice(&result.stderr).unwrap();
    }
    assert_eq!(fs::read(&project).unwrap(), before);
    ok(&[
        s("project"),
        s("layer"),
        project.as_os_str(),
        s("add"),
        s("normal"),
        s("--channel"),
        s("normal"),
    ]);
    let before = fs::read(&project).unwrap();
    assert!(
        !run(&[
            s("--file"),
            project.as_os_str(),
            s("draw"),
            s("clear"),
            s("--color"),
            s("#808080")
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(&project).unwrap(), before);
}
#[test]
fn cli_geometry_populates_aligned_normal_cels_atomically() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project.json");
    new_project(&project);
    ok(&[
        s("project"),
        s("layer"),
        project.as_os_str(),
        s("add"),
        s("normals"),
        s("--channel"),
        s("normal"),
    ]);
    ok(&[
        s("project"),
        s("layer"),
        project.as_os_str(),
        s("select"),
        s("base"),
    ]);
    let shape = root.path().join("shape.json");
    fs::write(&shape,serde_json::to_vec(&json!({"shape":"prism","x":1,"y":2,"width":3,"height":3,"depth":1,"faces":{"top":"#FFFFFF","front":"#C8281E","side":"#1E32BE"}})).unwrap()).unwrap();
    ok(&[
        s("project"),
        s("geometry"),
        project.as_os_str(),
        s("--input"),
        shape.as_os_str(),
        s("--normal-layer"),
        s("normals"),
    ]);
    let document = storage::load_project(&project).unwrap();
    assert_eq!(
        document
            .render_channel(0, painter::maps::Channel::Normal)
            .unwrap()
            .pixel(4, 2),
        Some(Pixel::rgba(218, 128, 218, 255))
    );
    let before = fs::read(&project).unwrap();
    assert!(
        !run(&[
            s("project"),
            s("geometry"),
            project.as_os_str(),
            s("--input"),
            shape.as_os_str(),
            s("--normal-layer"),
            s("base")
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(&project).unwrap(), before);
}
