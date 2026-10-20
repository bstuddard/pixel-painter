// SPDX-License-Identifier: GPL-3.0-only
//! End-to-end machine output, persistence, report exits, and no-overwrite contracts.
#![cfg(feature = "cli")]

use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_painter"))
        .current_dir(root)
        .arg("--json")
        .args(args)
        .output()
        .unwrap()
}
fn success(root: &Path, args: &[&str]) -> Value {
    let output = run(root, args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}
fn write(root: &Path, path: &str, value: Value) {
    fs::write(root.join(path), serde_json::to_vec(&value).unwrap()).unwrap();
}

#[test]
fn generate_uses_one_schema_and_one_configuration_byte_limit() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let mut oversized =
        serde_json::to_vec(&json!({"version":2,"width":1,"height":1,"operations":[]})).unwrap();
    oversized.resize(painter::assets::MAX_CONFIG_BYTES as usize + 1, b' ');
    for (input, code) in [
        (
            serde_json::to_vec(&json!({"version":1,"width":1,"height":1,"operations":[]})).unwrap(),
            "unsupported_version",
        ),
        (
            serde_json::to_vec(
                &json!({"version":1,"width":1,"height":1,"batch":{"version":1,"operations":[]}}),
            )
            .unwrap(),
            "invalid_json",
        ),
        (oversized, "resource_limit"),
    ] {
        fs::write(root.join("recipe.json"), input).unwrap();
        let output = run(root, &["generate", "--input", "recipe.json"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], code);
        assert!(!root.join("canvas.json").exists());
    }
    write(
        root,
        "recipe.json",
        json!({"version":2,"width":1,"height":1,"seed":91,"operations":[{"op":"clear","color":"#C8281E"}]}),
    );
    assert_eq!(
        success(root, &["generate", "--input", "recipe.json"])["seed"],
        91
    );
    let canvas = painter::storage::load(&root.join("canvas.json")).unwrap();
    assert_eq!(canvas.pixels(), &[painter::Pixel::rgba(200, 40, 30, 255)]);
}

#[test]
fn authoring_commands_work_together_and_emit_one_json_value() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write(
        root,
        "profile.json",
        json!({"version":1,"name":"test","palette":{"version":1,"colors":{"outline":"#000000","top":"#FFFFFF","front":"#FF0000","side":"#0000FF","highlight":"#00FF00","shadow":"#00000080"}},"roles":{"outline":"outline","top":"top","front":"front","side":"side","highlight":"highlight","shadow":"shadow"}}),
    );
    write(
        root,
        "recipe.json",
        json!({"version":2,"width":8,"height":8,"parameters":{"ink":{"type":"color","default":"front"}},"operations":[{"op":"fill_rect","x":1,"y":1,"width":3,"height":3,"color":{"param":"ink"}}]}),
    );
    success(
        root,
        &[
            "--file",
            "canvas.json",
            "generate",
            "--input",
            "recipe.json",
            "--profile",
            "profile.json",
        ],
    );
    success(
        root,
        &[
            "style",
            "reference",
            "--profile",
            "profile.json",
            "--output",
            "reference.png",
        ],
    );
    assert_eq!(
        success(
            root,
            &[
                "style",
                "check",
                "--profile",
                "profile.json",
                "--input",
                "canvas.json"
            ]
        )["valid"],
        true
    );
    write(
        root,
        "shape.json",
        json!({"shape":"prism","x":1,"y":2,"width":3,"height":3,"depth":1,"faces":{"top":"#FFFFFF","front":"#FF0000","side":"#0000FF"}}),
    );
    success(root, &["geometry", "--input", "shape.json"]);
    success(root, &["project", "migrate", "canvas.json", "project.json"]);
    success(
        root,
        &[
            "animation",
            "check",
            "--input",
            "project.json",
            "--profile",
            "profile.json",
        ],
    );
    success(
        root,
        &[
            "animation",
            "contact-sheet",
            "--input",
            "project.json",
            "--output",
            "animation.png",
        ],
    );
    write(
        root,
        "scene.json",
        json!({"version":1,"width":16,"height":16,"assets":[{"id":"sprite","input":"project.json","position":{"x":8,"y":8}}]}),
    );
    success(
        root,
        &[
            "scene",
            "render",
            "--input",
            "scene.json",
            "--output",
            "scene.png",
        ],
    );
    success(
        root,
        &[
            "scene",
            "contact-sheet",
            "--input",
            "scene.json",
            "--output",
            "contact.png",
        ],
    );
    write(
        root,
        "shadow.json",
        json!({"width":16,"height":16,"origin":{"x":0,"y":0},"step":{"x":1,"y":1},"steps":2,"squash_y":2,"color":"#00000080"}),
    );
    success(
        root,
        &[
            "shadow",
            "--input",
            "project.json",
            "--options",
            "shadow.json",
            "--output",
            "shadow.png",
        ],
    );
    write(
        root,
        "pack.json",
        json!({"version":1,"profile":"profile.json","assets":[{"id":"sprite","source":{"kind":"artwork","input":"project.json"}}]}),
    );
    success(
        root,
        &[
            "pack",
            "generate",
            "--input",
            "pack.json",
            "--output",
            "pack",
        ],
    );
    assert_eq!(
        success(root, &["pack", "check", "--input", "pack"])["valid"],
        true
    );
    assert!(
        !run(
            root,
            &[
                "pack",
                "generate",
                "--input",
                "pack.json",
                "--output",
                "pack"
            ]
        )
        .status
        .success()
    );
    let original = fs::read(root.join("scene.png")).unwrap();
    assert!(
        !run(
            root,
            &[
                "scene",
                "render",
                "--input",
                "scene.json",
                "--output",
                "scene.png"
            ]
        )
        .status
        .success()
    );
    assert_eq!(fs::read(root.join("scene.png")).unwrap(), original);

    // A measurable violation is a report on stdout with exit 1; malformed input
    // remains the established structured error on stderr.
    success(
        root,
        &[
            "set-pixel",
            "--x",
            "0",
            "--y",
            "0",
            "--r",
            "12",
            "--g",
            "13",
            "--b",
            "14",
        ],
    );
    let output = run(
        root,
        &[
            "style",
            "check",
            "--profile",
            "profile.json",
            "--input",
            "canvas.json",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["valid"],
        false
    );
    write(
        root,
        "scene.json",
        json!({"version":1,"width":8,"height":8,"assets":[{"id":"bad","input":"../secret.png","position":{"x":0,"y":0}}]}),
    );
    let output = run(
        root,
        &[
            "scene",
            "render",
            "--input",
            "scene.json",
            "--output",
            "blocked.png",
        ],
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(serde_json::from_slice::<Value>(&output.stderr).is_ok());
    assert!(!root.join("blocked.png").exists());
}

#[test]
fn untrusted_pack_paths_cannot_emit_raw_terminal_controls_in_default_reports() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    success(root, &["new", "--width", "1", "--height", "1"]);
    success(root, &["project", "migrate", "canvas.json", "source.json"]);
    write(
        root,
        "recipe.json",
        json!({"version":1,"assets":[{"id":"sprite","source":{"kind":"artwork","input":"source.json"}}]}),
    );
    success(
        root,
        &[
            "pack",
            "generate",
            "--input",
            "recipe.json",
            "--output",
            "pack",
        ],
    );
    let name = "asset-\u{009b}2J-\u{007f}";
    fs::rename(
        root.join("pack/assets/0000"),
        root.join("pack/assets").join(name),
    )
    .unwrap();
    let manifest_path = root.join("pack/pack.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["assets"][0]["directory"] = json!(format!("assets/{name}"));
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::write(
        root.join("pack/assets")
            .join(name)
            .join("export/frames/0000.png"),
        b"corrupt",
    )
    .unwrap();
    for flags in [&[][..], &["--json"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_painter"))
            .current_dir(root)
            .args(flags)
            .args(["pack", "check", "--input", "pack"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        let encoded = String::from_utf8(output.stdout).unwrap();
        assert!(encoded.chars().all(|c| !c.is_control() || c == '\n'));
        let report: Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(report["valid"], false);
        assert!(
            report["issues"][0]["message"]
                .as_str()
                .unwrap()
                .contains(name)
        );
    }
}
