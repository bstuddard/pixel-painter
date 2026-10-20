// SPDX-License-Identifier: GPL-3.0-only
//! Security boundaries: terminal text, no-clobber writes, and data-only asset names.

use painter::{
    Canvas, Document, Error,
    atlas::{Atlas, Options},
    storage,
};
use std::fs;

#[test]
fn error_display_escapes_controls_without_changing_serialized_messages() {
    let message = "éclair 'quoted' \\ path\x1b]52;c;dGVzdA==\x07\n\r\t\u{009b}";
    let error = Error::invalid(message);
    let display = error.to_string();
    assert_eq!(
        display,
        "éclair 'quoted' \\ path\\u{1b}]52;c;dGVzdA==\\u{7}\\n\\r\\t\\u{9b}"
    );
    assert!(display.chars().all(|character| !character.is_control()));
    assert_eq!(serde_json::to_value(&error).unwrap()["message"], message);
}

#[test]
#[cfg(feature = "cli")]
fn parser_diagnostics_escape_argument_controls_and_keep_json_data_and_layout() {
    use std::process::Command;
    let value = "éclair\r\u{009b}2J\nspoof\t";
    for json in [false, true] {
        for arguments in [
            vec!["new", "--width", value, "--height", "1"],
            vec![value],
            vec!["new", value],
        ] {
            let root = tempfile::tempdir().unwrap();
            let mut command = Command::new(env!("CARGO_BIN_EXE_painter"));
            command.current_dir(root.path());
            if json {
                command.arg("--json");
            }
            let result = command.args(&arguments).output().unwrap();
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
            let stderr = String::from_utf8(result.stderr).unwrap();
            if json {
                let error: serde_json::Value = serde_json::from_str(&stderr).unwrap();
                assert_eq!(error["error"]["code"], "invalid_arguments");
                assert!(error["error"]["message"].as_str().unwrap().contains(value));
            } else {
                assert!(stderr.starts_with("error: "));
                assert!(
                    stderr.contains("éclair\\r\\u{9b}2J\\nspoof\\t"),
                    "{stderr:?}"
                );
                assert!(stderr.contains("\n\n"));
                assert!(
                    stderr
                        .chars()
                        .all(|character| character == '\n' || !character.is_control())
                );
            }
        }
    }
}

#[cfg(all(unix, feature = "cli"))]
#[test]
fn parser_usage_does_not_echo_a_control_character_in_argv_zero() {
    use std::{os::unix::process::CommandExt, process::Command};
    for argument in ["--help", "unknown"] {
        for process_name in ["bad\r\u{009b}2J", "--json"] {
            let result = Command::new(env!("CARGO_BIN_EXE_painter"))
                .arg0(process_name)
                .arg(argument)
                .output()
                .unwrap();
            let success = result.status.success();
            let output = if success {
                result.stdout
            } else {
                result.stderr
            };
            let output = String::from_utf8(output).unwrap();
            if !success {
                assert!(output.starts_with("error: "));
            }
            assert!(output.contains("Usage: painter"), "{output:?}");
            assert!(
                output
                    .chars()
                    .all(|character| character == '\n' || !character.is_control())
            );
        }
    }
}

#[cfg(all(unix, feature = "cli"))]
#[test]
fn filenames_cannot_inject_terminal_commands_and_json_preserves_the_path() {
    use std::process::Command;
    for json in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let name = "éclair 'quoted' \\ \x1b]52;c;dGVzdA==\x07\n\r\t\u{009b}.json";
        let mut command = Command::new(env!("CARGO_BIN_EXE_painter"));
        command.current_dir(root.path());
        if json {
            command.arg("--json");
        }
        let result = command
            .args(["--file", name, "new", "--width", "1", "--height", "1"])
            .output()
            .unwrap();
        assert!(result.status.success(), "{:?}", result.stderr);
        assert!(result.stderr.is_empty());
        let output = String::from_utf8(result.stdout).unwrap();
        if json {
            // JSON escapes ASCII controls and preserves the original Unicode
            // value. C1 Unicode controls are legal JSON string contents.
            assert!(!output.contains(['\x1b', '\x07', '\r', '\t']));
            let value: serde_json::Value = serde_json::from_str(&output).unwrap();
            assert_eq!(value["file"], name);
        } else {
            assert!(
                output
                    .chars()
                    .all(|character| character == '\n' || !character.is_control())
            );
            assert!(output.starts_with("Created 1x1 at éclair 'quoted' \\ "));
            assert!(output.contains("\\u{1b}]52;c;dGVzdA==\\u{7}\\n\\r\\t\\u{9b}.json"));
        }
        assert_eq!(
            storage::load(&root.path().join(name)).unwrap(),
            Canvas::new(1, 1).unwrap()
        );
    }
}

#[cfg(unix)]
#[test]
fn exports_refuse_live_and_dangling_symlinks_without_modifying_their_targets() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    fs::write(&target, b"keep").unwrap();
    for (name, destination) in [
        ("live", target.clone()),
        ("dangling", root.path().join("missing")),
    ] {
        let output = root.path().join(name);
        symlink(&destination, &output).unwrap();
        assert_eq!(
            storage::export_png(&output, &Canvas::new(1, 1).unwrap())
                .unwrap_err()
                .code
                .as_str(),
            "already_exists"
        );
        assert_eq!(fs::read_link(&output).unwrap(), destination);
        assert_eq!(fs::read(&target).unwrap(), b"keep");
        assert!(!root.path().join("missing").exists());
    }
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 3);
}

#[test]
fn frame_names_cannot_escape_the_export_directory() {
    let root = tempfile::tempdir().unwrap();
    let outside = root.path().join("escape.png");
    fs::write(&outside, b"keep").unwrap();
    let mut document = Document::new(1, 1).unwrap();
    document
        .add_frame("../escape.png".into(), None, 100)
        .unwrap();
    let atlas = Atlas::from_document(&document, Options::default()).unwrap();
    let output = root.path().join("bundle");
    atlas.export(&output, 1).unwrap();
    assert_eq!(atlas.metadata().frames[1].name, "../escape.png");
    assert_eq!(atlas.metadata().frames[1].file, "frames/0001.png");
    assert!(output.join("frames/0001.png").is_file());
    assert_eq!(fs::read(&outside).unwrap(), b"keep");
}

#[cfg(unix)]
#[test]
fn new_artwork_is_private_and_edits_preserve_explicit_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("art.json");
    let canvas = Canvas::new(1, 1).unwrap();
    storage::save(&output, &canvas, true).unwrap();
    assert_eq!(
        fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::set_permissions(&output, fs::Permissions::from_mode(0o640)).unwrap();
    storage::save(&output, &canvas, false).unwrap();
    assert_eq!(
        fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o640
    );
}
