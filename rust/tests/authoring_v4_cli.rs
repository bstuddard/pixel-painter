// SPDX-License-Identifier: GPL-3.0-only
#![cfg(feature = "cli")]
use painter::{Canvas, Pixel, storage};
use serde_json::{Value, json};
use std::{
    ffi::OsStr,
    fs,
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

#[test]
fn cli_mask_png_and_json_alpha_lock_dry_run_and_errors_preserve_artwork() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("canvas.json");
    let canvas = Canvas::from_pixels(
        3,
        1,
        vec![
            Pixel::rgba(10, 20, 30, 128),
            Pixel::rgba(40, 50, 60, 0),
            Pixel::rgba(70, 80, 90, 255),
        ],
    )
    .unwrap();
    storage::save(&path, &canvas, true).unwrap();
    let mask = Canvas::from_pixels(
        3,
        1,
        vec![
            Pixel::rgba(0, 0, 0, 1),
            Pixel::TRANSPARENT,
            Pixel::TRANSPARENT,
        ],
    )
    .unwrap();
    for extension in ["png", "json"] {
        let mask_path = root.path().join(format!("mask.{extension}"));
        if extension == "png" {
            storage::export_png(&mask_path, &mask).unwrap();
        } else {
            storage::save(&mask_path, &mask, true).unwrap();
        }
        ok(&[
            s("--file"),
            path.as_os_str(),
            s("draw"),
            s("--mask"),
            mask_path.as_os_str(),
            s("--alpha-lock"),
            s("clear"),
            s("--color"),
            s("#FFFFFF"),
        ]);
        let result = storage::load(&path).unwrap();
        assert_eq!(result.pixels()[0], Pixel::rgba(255, 255, 255, 128));
        assert_eq!(&result.pixels()[1..], &canvas.pixels()[1..]);
    }
    let before = fs::read(&path).unwrap();
    let operations = root.path().join("batch.json");
    storage::write_json(
        &operations,
        &json!({"version":1,"operations":[{"op":"clear","color":"#000000"}]}),
        true,
    )
    .unwrap();
    ok(&[
        s("--file"),
        path.as_os_str(),
        s("batch"),
        s("--input"),
        operations.as_os_str(),
        s("--alpha-lock"),
        s("--dry-run"),
    ]);
    assert_eq!(fs::read(&path).unwrap(), before);
    let bad = root.path().join("bad.png");
    storage::export_png(&bad, &Canvas::new(1, 1).unwrap()).unwrap();
    let result = run(&[
        s("--file"),
        path.as_os_str(),
        s("draw"),
        s("--mask"),
        bad.as_os_str(),
        s("clear"),
    ]);
    assert!(!result.status.success());
    let _: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn cli_solo_raw_channels_and_review_are_read_only_and_have_explicit_arguments() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("project.json");
    ok(&[
        s("project"),
        s("new"),
        path.as_os_str(),
        s("--width"),
        s("2"),
        s("--height"),
        s("1"),
    ]);
    ok(&[
        s("--file"),
        path.as_os_str(),
        s("draw"),
        s("clear"),
        s("--color"),
        s("#204080"),
    ]);
    ok(&[
        s("project"),
        s("layer"),
        path.as_os_str(),
        s("set"),
        s("base"),
        s("--visible"),
        s("false"),
        s("--opacity"),
        s("128"),
    ]);
    let before = fs::read(&path).unwrap();
    for raw in [false, true] {
        let output = root.path().join(if raw { "raw.png" } else { "solo.png" });
        let mut args = vec![
            s("--file"),
            path.as_os_str(),
            s("render"),
            s("--output"),
            output.as_os_str(),
            s("--scale"),
            s("1"),
            s("--solo-layer"),
            s("base"),
        ];
        if raw {
            args.push(s("--raw"));
        }
        ok(&args);
        assert_eq!(
            storage::import_png(&output, false).unwrap().pixels()[0],
            Pixel::rgba(32, 64, 128, if raw { 255 } else { 128 })
        );
    }
    let output = root.path().join("review");
    let value = ok(&[
        s("--file"),
        path.as_os_str(),
        s("preview"),
        s("--output"),
        output.as_os_str(),
        s("--all-frames"),
        s("--scale"),
        s("1"),
    ]);
    assert_eq!(value["review"]["frames"][0]["color_pixels"], 0);
    let diagnostic = fs::read(output.join("review.json")).unwrap();
    assert!(
        !run(&[
            s("--file"),
            path.as_os_str(),
            s("preview"),
            s("--output"),
            output.as_os_str()
        ])
        .status
        .success()
    );
    assert_eq!(fs::read(output.join("review.json")).unwrap(), diagnostic);
    let absent = root.path().join("absent.png");
    for flags in [
        vec![s("--raw")],
        vec![s("--solo-layer"), s("base"), s("--channel"), s("normal")],
        vec![s("--solo-layer"), s("missing")],
        vec![s("--channel"), s("../normal")],
    ] {
        let mut args = vec![
            s("--file"),
            path.as_os_str(),
            s("render"),
            s("--output"),
            absent.as_os_str(),
        ];
        args.extend(flags);
        assert!(!run(&args).status.success());
        assert!(!absent.exists());
    }
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn readme_quick_start_draws_exports_and_reviews_a_sprite() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("gem.json");
    ok(&[
        s("--file"),
        path.as_os_str(),
        s("new"),
        s("--width"),
        s("32"),
        s("--height"),
        s("32"),
    ]);
    ok(&[
        s("--file"),
        path.as_os_str(),
        s("draw"),
        s("polygon"),
        s("--points"),
        s("16,3,28,14,16,29,4,14"),
        s("--color"),
        s("#253347"),
        s("--fill"),
    ]);
    ok(&[
        s("--file"),
        path.as_os_str(),
        s("draw"),
        s("polygon"),
        s("--points"),
        s("16,5,25,14,16,25,7,14"),
        s("--color"),
        s("#51B7B0"),
        s("--fill"),
    ]);
    ok(&[
        s("--file"),
        path.as_os_str(),
        s("draw"),
        s("--alpha-lock"),
        s("line"),
        s("--x1"),
        s("8"),
        s("--y1"),
        s("13"),
        s("--x2"),
        s("16"),
        s("--y2"),
        s("6"),
        s("--color"),
        s("#B7ECD4"),
    ]);
    let output = root.path().join("gem.png");
    ok(&[
        s("--file"),
        path.as_os_str(),
        s("export"),
        s("--output"),
        output.as_os_str(),
    ]);
    let preview = root.path().join("gem-review");
    ok(&[
        s("--file"),
        path.as_os_str(),
        s("preview"),
        s("--output"),
        preview.as_os_str(),
        s("--scale"),
        s("4"),
    ]);
    let image = storage::import_png(&output, false).unwrap();
    assert_eq!(image.pixel(0, 0), Some(Pixel::TRANSPARENT));
    assert!(preview.join("contact-sheet.png").is_file());
}
