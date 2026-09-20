// SPDX-License-Identifier: GPL-3.0-only

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct Sandbox {
    directory: PathBuf,
}

impl Sandbox {
    /// Create an isolated temporary directory for one test.
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("painter-test-{}-{stamp}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        Self {
            directory,
        }
    }

    /// Run the CLI in this test's directory and capture its output.
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_painter"))
            .current_dir(&self.directory)
            .args(args)
            .output()
            .unwrap()
    }

    /// Require a successful command and return its printed text.
    fn success(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}

impl Drop for Sandbox {
    /// Remove temporary files when the test releases its sandbox.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

/// Verify edits persist between processes and appear at the right row and column.
#[test]
fn commands_share_pixels_and_print_correct_rows() {
    let sandbox = Sandbox::new();
    sandbox.success(&["--file", "art.json", "new", "--width", "3", "--height", "2"]);
    sandbox.success(&[
        "set-pixel",
        "--file",
        "art.json",
        "--x",
        "2",
        "--y",
        "1",
        "--r",
        "255",
        "--g",
        "12",
        "--b",
        "34",
    ]);

    let saved: Value =
        serde_json::from_slice(&fs::read(sandbox.directory.join("art.json")).unwrap()).unwrap();
    let pixels = saved["pixels"].as_array().unwrap();
    assert_eq!(pixels.len(), 6);
    assert!(pixels[..5].iter().all(|pixel| pixel["a"] == 0));
    assert_eq!(
        pixels[5],
        serde_json::json!({ "r": 255, "g": 12, "b": 34, "a": 255 })
    );

    let printed = sandbox.success(&["print", "--file", "art.json"]);
    let rows: Vec<_> = printed.lines().collect();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], "3x2");
    assert!(rows[1].starts_with("row 0: "));
    assert!(rows[2].starts_with("row 1: "));
    assert_eq!(rows[1].matches("Pixel {").count(), 3);
    assert_eq!(rows[2].matches("Pixel {").count(), 3);
    assert!(rows[2].ends_with("Pixel { r: 255, g: 12, b: 34, a: 255 } "));
}

/// Verify rejected edits and duplicate creation leave saved pixels unchanged.
#[test]
fn bad_edits_and_recreation_preserve_saved_canvas() {
    let sandbox = Sandbox::new();
    sandbox.success(&["new", "--width", "2", "--height", "2"]);
    let path = sandbox.directory.join("canvas.json");
    let original = fs::read(&path).unwrap();

    for args in [
        vec![
            "set-pixel",
            "--x",
            "2",
            "--y",
            "0",
            "--r",
            "255",
            "--g",
            "0",
            "--b",
            "0",
        ],
        vec![
            "set-pixel",
            "--x",
            "0",
            "--y",
            "2",
            "--r",
            "255",
            "--g",
            "0",
            "--b",
            "0",
        ],
        vec![
            "set-pixel",
            "--x",
            "0",
            "--y",
            "0",
            "--r",
            "256",
            "--g",
            "0",
            "--b",
            "0",
        ],
        vec!["new"],
    ] {
        let output = sandbox.run(&args);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("error:"));
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}

/// Verify help output and graceful rejection of missing or invalid canvases.
#[test]
fn help_and_validation_work_without_panics() {
    let sandbox = Sandbox::new();
    for args in [
        vec!["--help"],
        vec!["new", "--help"],
        vec!["set-pixel", "--help"],
        vec!["print", "--help"],
        vec!["view", "--help"],
        vec!["export", "--help"],
    ] {
        assert!(sandbox.success(&args).contains("Usage:"));
    }
    for args in [
        vec!["print"],
        vec!["new", "--width", "0"],
        vec!["new", "--width", "4294967295", "--height", "4294967295"],
    ] {
        let output = sandbox.run(&args);
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
    let path = sandbox.directory.join("canvas.json");
    assert!(!path.exists());
    for bad_canvas in [
        "not json",
        r#"{"width":2,"height":2,"pixels":[]}"#,
        r#"{"width":0,"height":0,"pixels":[]}"#,
    ] {
        fs::write(&path, bad_canvas).unwrap();
        assert!(!sandbox.run(&["print"]).status.success());
    }
}

/// Decode an exported image to verify dimensions, pixel order, and all alpha values.
#[test]
fn export_preserves_rgba_and_source_canvas() {
    let sandbox = Sandbox::new();
    let source = sandbox.directory.join("canvas.json");
    let canvas = r#"{
        "width": 3,
        "height": 2,
        "pixels": [
            {"r":255,"g":0,"b":0,"a":255},
            {"r":1,"g":2,"b":3,"a":0},
            {"r":0,"g":0,"b":255,"a":128},
            {"r":0,"g":255,"b":0,"a":255},
            {"r":255,"g":255,"b":255,"a":64},
            {"r":0,"g":0,"b":0,"a":255}
        ]
    }"#;
    fs::write(&source, canvas).unwrap();
    sandbox.success(&["export", "--output", "my art.png"]);

    let file = fs::File::open(sandbox.directory.join("my art.png")).unwrap();
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    assert_eq!((info.width, info.height), (3, 2));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    assert_eq!(
        &bytes[..info.buffer_size()],
        &[
            255, 0, 0, 255, 1, 2, 3, 0, 0, 0, 255, 128, 0, 255, 0, 255, 255, 255, 255, 64, 0, 0, 0,
            255,
        ]
    );
    assert_eq!(fs::read_to_string(source).unwrap(), canvas);
}

/// Export must not replace existing images, the input JSON, or leave temporary files.
#[test]
fn export_refuses_existing_destinations() {
    let sandbox = Sandbox::new();
    sandbox.success(&["new", "--width", "1", "--height", "1"]);
    sandbox.success(&["export", "-o", "art.png"]);
    for destination in ["art.png", "canvas.json"] {
        let path = sandbox.directory.join(destination);
        let original = fs::read(&path).unwrap();
        assert!(
            !sandbox
                .run(&["export", "--output", destination])
                .status
                .success()
        );
        assert_eq!(fs::read(path).unwrap(), original);
    }
    assert_eq!(fs::read_dir(&sandbox.directory).unwrap().count(), 2);
}

/// Missing or invalid input and invalid destinations must fail without partial output.
#[test]
fn failed_export_leaves_no_output() {
    let sandbox = Sandbox::new();
    assert!(
        !sandbox
            .run(&["export", "--output", "art.png"])
            .status
            .success()
    );
    fs::write(
        sandbox.directory.join("canvas.json"),
        r#"{"width":1,"height":1,"pixels":[]}"#,
    )
    .unwrap();
    assert!(
        !sandbox
            .run(&["export", "--output", "art.png"])
            .status
            .success()
    );
    assert!(!sandbox.directory.join("art.png").exists());
    sandbox.success(&[
        "--file",
        "valid.json",
        "new",
        "--width",
        "1",
        "--height",
        "1",
    ]);
    assert!(
        !sandbox
            .run(&[
                "--file",
                "valid.json",
                "export",
                "--output",
                "missing/art.png"
            ])
            .status
            .success()
    );
    assert!(
        !sandbox
            .run(&["--file", "valid.json", "export", "--output", "."])
            .status
            .success()
    );
    assert_eq!(fs::read_dir(&sandbox.directory).unwrap().count(), 2);
}

/// Verify terminal colors, transparency blending, row order, and color resets.
#[test]
fn view_renders_rgba_without_changing_the_canvas() {
    let sandbox = Sandbox::new();
    let path = sandbox.directory.join("canvas.json");
    let canvas = r#"{
        "width": 2,
        "height": 2,
        "pixels": [
            {"r":255,"g":0,"b":0,"a":255},
            {"r":0,"g":255,"b":0,"a":0},
            {"r":0,"g":0,"b":255,"a":128},
            {"r":0,"g":0,"b":0,"a":255}
        ]
    }"#;
    fs::write(&path, canvas).unwrap();

    let output = sandbox.success(&["view"]);
    assert_eq!(
        output,
        concat!(
            "2x2\n",
            "\x1b[48;2;255;0;0m  \x1b[48;2;128;128;128m  \x1b[0m\n",
            "\x1b[48;2;64;64;192m  \x1b[48;2;0;0;0m  \x1b[0m\n",
        )
    );
    assert_eq!(fs::read_to_string(path).unwrap(), canvas);
}
