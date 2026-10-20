// SPDX-License-Identifier: GPL-3.0-only
//! Exercise the public CLI as an agent would, including its failure contracts.

#![cfg(feature = "cli")]

use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

struct Session(tempfile::TempDir);
impl Session {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }
    fn path(&self, name: &str) -> std::path::PathBuf {
        self.0.path().join(name)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_painter"))
            .current_dir(self.0.path())
            .arg("--json")
            .args(args)
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> Value {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn error(&self, args: &[&str]) -> Value {
        let output = self.run(args);
        assert!(!output.status.success(), "unexpected success: {args:?}");
        assert!(output.stdout.is_empty(), "failure polluted stdout");
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["ok"], false);
        assert_eq!(error["version"], 1);
        error
    }
    fn write(&self, name: &str, value: Value) {
        fs::write(self.path(name), serde_json::to_vec(&value).unwrap()).unwrap();
    }
    fn canvas(&self) {
        self.ok(&["new", "--width", "8", "--height", "8"]);
    }
}

#[test]
fn dry_run_matches_commit_and_preserves_original_file_bytes() {
    let session = Session::new();
    session.canvas();
    let original = fs::read(session.path("canvas.json")).unwrap();
    fs::write(
        session.path("ops.json"),
        include_str!("fixtures/gem.ops.json"),
    )
    .unwrap();
    let dry = session.ok(&["batch", "--input", "ops.json", "--dry-run"]);
    assert_eq!(fs::read(session.path("canvas.json")).unwrap(), original);
    let committed = session.ok(&["batch", "--input", "ops.json"]);
    assert_eq!(dry["summary"], committed["summary"]);
    assert_eq!(committed["summary"]["changed_pixels"], 35);
    session.ok(&["export", "--output", "gem.png"]);
    session.ok(&["render", "--output", "preview.png", "--scale", "4"]);
    let report = session.ok(&["inspect"]);
    assert_eq!(report["unique_colors"], 4);
    assert_eq!(report["opaque_pixels"], 35);
}

#[test]
fn failed_final_edit_reports_index_and_preserves_disk() {
    let session = Session::new();
    session.canvas();
    let original = fs::read(session.path("canvas.json")).unwrap();
    session.write("ops.json",json!({"version":1,"operations":[{"op":"clear","color":"#FF0000"},{"op":"set_pixel","x":99,"y":0,"color":"#000000"}]}));
    let error = session.error(&["batch", "--input", "ops.json"]);
    assert_eq!(error["error"]["operation"], 1);
    assert_eq!(error["error"]["code"], "out_of_bounds");
    assert_eq!(fs::read(session.path("canvas.json")).unwrap(), original);
    assert_eq!(fs::read_dir(session.0.path()).unwrap().count(), 2);
}

#[test]
fn parser_and_schema_errors_are_structured_and_non_destructive() {
    let session = Session::new();
    session.canvas();
    let original = fs::read(session.path("canvas.json")).unwrap();
    for operations in [
        json!({"operations":[]}),
        json!({"version":2,"operations":[]}),
        json!({"version":1,"operations":[{"op":"rect","x":0,"y":0,"width":1,"height":1,"color":"#FFFFFF","typo":1}]}),
        json!({"version":1,"operations":[{"op":"unknown"}]}),
    ] {
        session.write("ops.json", operations);
        session.error(&["batch", "--input", "ops.json"]);
        assert_eq!(fs::read(session.path("canvas.json")).unwrap(), original);
    }
    assert_eq!(
        session.error(&["set-pixel", "--x", "abc"])["error"]["code"],
        "invalid_arguments"
    );
    fs::write(session.path("ops.json"), "{broken").unwrap();
    session.error(&["batch", "--input", "ops.json"]);
}

#[test]
fn direct_drawing_and_transform_commands_work_with_negative_coordinates() {
    let session = Session::new();
    session.canvas();
    session.ok(&[
        "draw", "rect", "--x", "-1", "--y", "-1", "--width", "4", "--height", "4", "--color",
        "#FF0000", "--fill",
    ]);
    session.ok(&[
        "draw", "line", "--x1", "-2", "--y1", "0", "--x2", "7", "--y2", "7", "--color", "#00FF00",
    ]);
    session.ok(&[
        "draw",
        "ellipse",
        "--x",
        "2",
        "--y",
        "2",
        "--width",
        "4",
        "--height",
        "3",
        "--color",
        "#0000FF80",
        "--fill",
        "--over",
    ]);
    session.ok(&[
        "draw",
        "polygon",
        "--points",
        "1,1,5,1,3,5",
        "--color",
        "#FF00FF",
        "--fill",
    ]);
    session.ok(&["transform", "rotate", "-1"]);
    session.ok(&["transform", "flip", "vertical"]);
    session.ok(&["transform", "move", "--dx", "-1", "--dy", "2"]);
    session.ok(&[
        "transform",
        "crop",
        "--x",
        "-1",
        "--y",
        "-1",
        "--width",
        "5",
        "--height",
        "6",
    ]);
    session.ok(&["transform", "resize", "--width", "10", "--height", "12"]);
    assert_eq!(session.ok(&["inspect"])["height"], 12);
}

#[test]
fn preview_replacement_is_explicit_and_cannot_replace_input() {
    let session = Session::new();
    session.canvas();
    session.ok(&[
        "render",
        "--output",
        "preview.png",
        "--scale",
        "2",
        "--replace",
    ]);
    let original = fs::read(session.path("preview.png")).unwrap();
    session.error(&["render", "--output", "preview.png", "--scale", "3"]);
    assert_eq!(fs::read(session.path("preview.png")).unwrap(), original);
    session.ok(&[
        "render",
        "--output",
        "preview.png",
        "--scale",
        "3",
        "--replace",
    ]);
    assert_ne!(fs::read(session.path("preview.png")).unwrap(), original);
    let source = fs::read(session.path("canvas.json")).unwrap();
    session.error(&["render", "--output", "canvas.json", "--replace"]);
    assert_eq!(fs::read(session.path("canvas.json")).unwrap(), source);
}

#[cfg(unix)]
#[test]
fn preview_replacement_refuses_symlink_to_source() {
    let session = Session::new();
    session.canvas();
    std::os::unix::fs::symlink("canvas.json", session.path("alias.json")).unwrap();
    session.error(&["render", "--output", "alias.json", "--replace"]);
    assert!(session.path("alias.json").is_symlink());
    assert_eq!(session.ok(&["inspect"])["width"], 8);
}

#[test]
fn migration_and_checkpoints_never_overwrite_legacy_source() {
    let session = Session::new();
    session.canvas();
    let original = fs::read(session.path("canvas.json")).unwrap();
    session.ok(&["snapshot", "--output", "checkpoint.json"]);
    assert_eq!(fs::read(session.path("checkpoint.json")).unwrap(), original);
    session.error(&["snapshot", "--output", "checkpoint.json"]);
    session.ok(&["project", "migrate", "canvas.json", "project.json"]);
    assert_eq!(fs::read(session.path("canvas.json")).unwrap(), original);
    session.error(&["project", "migrate", "canvas.json", "canvas.json"]);
    assert_eq!(fs::read(session.path("canvas.json")).unwrap(), original);
    assert_eq!(
        session.ok(&["project", "show", "project.json"])["version"],
        1
    );
}

#[test]
fn layers_frames_tags_and_export_work_end_to_end() {
    let session = Session::new();
    session.ok(&[
        "project",
        "new",
        "hero.json",
        "--width",
        "8",
        "--height",
        "8",
    ]);
    session.ok(&["project", "layer", "hero.json", "add", "hat"]);
    session.ok(&[
        "--file",
        "hero.json",
        "draw",
        "rect",
        "--x",
        "2",
        "--y",
        "2",
        "--width",
        "4",
        "--height",
        "4",
        "--color",
        "#44AAFF",
        "--fill",
    ]);
    session.ok(&[
        "project",
        "frame",
        "hero.json",
        "add",
        "walk",
        "--duplicate",
        "0",
        "--duration-ms",
        "125",
    ]);
    session.ok(&[
        "project",
        "frame",
        "hero.json",
        "set",
        "1",
        "--pivot-x",
        "4",
        "--pivot-y",
        "7",
    ]);
    session.ok(&[
        "--file",
        "hero.json",
        "--frame",
        "1",
        "--layer",
        "hat",
        "transform",
        "move",
        "--dx",
        "1",
        "--dy",
        "0",
    ]);
    session.ok(&[
        "project",
        "tag",
        "hero.json",
        "set",
        "walk-east",
        "--frames",
        "0,1",
        "--direction",
        "east",
    ]);
    session.ok(&[
        "project",
        "export",
        "hero.json",
        "--output",
        "bundle",
        "--columns",
        "2",
        "--padding",
        "2",
        "--scale",
        "2",
    ]);
    assert!(session.path("bundle/animation.png").is_file());
    let metadata: Value =
        serde_json::from_slice(&fs::read(session.path("bundle/metadata.json")).unwrap()).unwrap();
    assert_eq!(metadata["frames"][1]["duration_ms"], 125);
    assert_eq!(metadata["tags"]["walk-east"]["direction"], "east");
    session.error(&["project", "export", "hero.json", "--output", "bundle"]);
    session.ok(&[
        "project",
        "layer",
        "hero.json",
        "duplicate",
        "hat",
        "backup",
    ]);
    session.ok(&[
        "project",
        "layer",
        "hero.json",
        "set",
        "backup",
        "--visible",
        "false",
        "--opacity",
        "100",
    ]);
    session.ok(&["project", "layer", "hero.json", "reorder", "backup", "0"]);
    session.ok(&["project", "layer", "hero.json", "delete", "backup"]);
    session.ok(&["project", "frame", "hero.json", "reorder", "1", "0"]);
    session.ok(&["project", "frame", "hero.json", "delete", "1"]);
    session.ok(&["project", "tag", "hero.json", "delete", "walk-east"]);
    session.ok(&[
        "project",
        "resize",
        "hero.json",
        "--width",
        "16",
        "--height",
        "16",
    ]);
    session.ok(&["project", "layer", "hero.json", "merge", "hat"]);
}

#[test]
fn invalid_project_version_and_geometry_report_failure_without_rewrite() {
    let session = Session::new();
    session.ok(&["project", "new", "bad.json"]);
    let mut data: Value =
        serde_json::from_slice(&fs::read(session.path("bad.json")).unwrap()).unwrap();
    data["version"] = json!(99);
    session.write("bad.json", data);
    assert_eq!(
        session.error(&["--file", "bad.json", "inspect"])["error"]["code"],
        "unsupported_version"
    );
    let before = fs::read(session.path("bad.json")).unwrap();
    session.error(&["project", "layer", "bad.json", "add", "new"]);
    assert_eq!(fs::read(session.path("bad.json")).unwrap(), before);
}

#[test]
fn project_batch_rejects_geometry_change_and_dry_run_leaves_selection_unchanged() {
    let session = Session::new();
    session.ok(&[
        "project",
        "new",
        "hero.json",
        "--width",
        "8",
        "--height",
        "8",
    ]);
    session.ok(&["project", "layer", "hero.json", "add", "hat"]);
    let before = fs::read(session.path("hero.json")).unwrap();
    session.write(
        "ops.json",
        json!({"version":1,"operations":[{"op":"clear","color":"#FF0000"}]}),
    );
    session.ok(&[
        "--file",
        "hero.json",
        "--layer",
        "base",
        "batch",
        "--input",
        "ops.json",
        "--dry-run",
    ]);
    assert_eq!(fs::read(session.path("hero.json")).unwrap(), before);
    session.write(
        "ops.json",
        json!({"version":1,"operations":[{"op":"resize","width":4,"height":4}]}),
    );
    session.error(&["--file", "hero.json", "batch", "--input", "ops.json"]);
    assert_eq!(fs::read(session.path("hero.json")).unwrap(), before);
}

#[test]
fn template_generate_extract_stamp_and_png_import_round_trip() {
    let session = Session::new();
    session.write("recipe.json",json!({"version":2,"width":8,"height":8,"operations":[{"op":"fill_rect","x":1,"y":1,"width":6,"height":6,"color":"#22446680"}]}));
    session.ok(&["generate", "--input", "recipe.json", "--seed", "42"]);
    session.ok(&[
        "extract",
        "--x",
        "1",
        "--y",
        "1",
        "--width",
        "6",
        "--height",
        "6",
        "--output",
        "component.json",
    ]);
    session.ok(&[
        "stamp",
        "--input",
        "component.json",
        "--x",
        "0",
        "--y",
        "0",
        "--over",
    ]);
    session.ok(&["export", "--output", "art.png"]);
    session.ok(&["--file", "imported.json", "import", "--input", "art.png"]);
    assert_eq!(
        session.ok(&["print"]),
        session.ok(&["--file", "imported.json", "print"])
    );
    session.error(&["import", "--input", "art.png"]);
}

#[test]
fn palette_cli_overrides_are_explicit_and_invalid_colors_roll_back() {
    let session = Session::new();
    session.canvas();
    session.write(
        "palette.json",
        json!({"version":1,"colors":{"ink":"#112233"}}),
    );
    session.ok(&[
        "draw",
        "--palette",
        "palette.json",
        "clear",
        "--color",
        "ink",
    ]);
    assert_eq!(
        session.ok(&["inspect", "--palette", "palette.json"])["palette_usage"]["ink"],
        64
    );
    let before = fs::read(session.path("canvas.json")).unwrap();
    session.write(
        "ops.json",
        json!({"version":1,"operations":[{"op":"clear","color":"#445566"}]}),
    );
    assert_eq!(
        session.error(&[
            "batch",
            "--input",
            "ops.json",
            "--palette",
            "palette.json",
            "--policy",
            "strict"
        ])["error"]["code"],
        "palette_violation"
    );
    assert_eq!(fs::read(session.path("canvas.json")).unwrap(), before);
    session.ok(&[
        "batch",
        "--input",
        "ops.json",
        "--palette",
        "palette.json",
        "--policy",
        "nearest",
    ]);
    assert_eq!(fs::read(session.path("canvas.json")).unwrap(), before);
}

#[test]
fn tile_cli_exports_ids_and_checks_seams() {
    let session = Session::new();
    session.canvas();
    session.write(
        "rules.json",
        json!({"version":1,"connections":[{"from":0,"to":1,"edge":"right"}]}),
    );
    session.ok(&[
        "tiles",
        "--tile-width",
        "4",
        "--tile-height",
        "4",
        "seams",
        "--rules",
        "rules.json",
    ]);
    session.ok(&[
        "tiles",
        "--tile-width",
        "4",
        "--tile-height",
        "4",
        "export",
        "--output",
        "tiles",
        "--rules",
        "rules.json",
    ]);
    let metadata: Value =
        serde_json::from_slice(&fs::read(session.path("tiles/metadata.json")).unwrap()).unwrap();
    assert_eq!(metadata["tileset"]["ids"], json!([0, 1, 2, 3]));
    assert!(!session.path("tiles/animation.png").exists());
    session.ok(&[
        "set-pixel",
        "--x",
        "3",
        "--y",
        "0",
        "--r",
        "255",
        "--g",
        "0",
        "--b",
        "0",
    ]);
    let output = session.run(&[
        "tiles",
        "--tile-width",
        "4",
        "--tile-height",
        "4",
        "seams",
        "--rules",
        "rules.json",
    ]);
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["valid"], false);
}

#[test]
fn oversized_sparse_input_is_rejected_before_json_parsing() {
    let session = Session::new();
    fs::File::create(session.path("huge.json"))
        .unwrap()
        .set_len(painter::storage::MAX_FILE_BYTES + 1)
        .unwrap();
    assert_eq!(
        session.error(&["batch", "--input", "huge.json"])["error"]["code"],
        "resource_limit"
    );
    assert_eq!(
        fs::metadata(session.path("huge.json")).unwrap().len(),
        painter::storage::MAX_FILE_BYTES + 1
    );
}

#[test]
fn export_bundle_refuses_even_empty_destinations() {
    let session = Session::new();
    session.ok(&["project", "new", "hero.json"]);
    fs::create_dir(session.path("bundle")).unwrap();
    session.error(&["project", "export", "hero.json", "--output", "bundle"]);
    assert_eq!(fs::read_dir(session.path("bundle")).unwrap().count(), 0);
}

#[test]
fn batch_clipboard_and_operation_limits_are_enforced_without_writes() {
    let session = Session::new();
    session.canvas();
    let before = fs::read(session.path("canvas.json")).unwrap();
    session.write(
        "ops.json",
        json!({"version":1,"operations":vec![json!({"op":"clear"});10_001]}),
    );
    session.error(&["batch", "--input", "ops.json"]);
    assert_eq!(fs::read(session.path("canvas.json")).unwrap(), before);
    session.write("ops.json",json!({"version":1,"operations":[{"op":"copy","name":"huge","x":0,"y":0,"width":4097,"height":4097}]}));
    assert_eq!(
        session.error(&["batch", "--input", "ops.json"])["error"]["code"],
        "resource_limit"
    );
    assert_eq!(fs::read(session.path("canvas.json")).unwrap(), before);
}

#[test]
fn global_json_flag_after_subcommands_suppresses_ansi_and_legacy_text() {
    let session = Session::new();
    session.canvas();
    let output = Command::new(env!("CARGO_BIN_EXE_painter"))
        .current_dir(session.0.path())
        .args(["view", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!output.stdout.contains(&0x1b));
    let _: Value = serde_json::from_slice(&output.stdout).unwrap();
    let bad = Command::new(env!("CARGO_BIN_EXE_painter"))
        .current_dir(session.0.path())
        .args(["draw", "rect", "--json"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    let _: Value = serde_json::from_slice(&bad.stderr).unwrap();
}

#[test]
fn temporary_files_do_not_escape_sandbox() {
    let session = Session::new();
    session.canvas();
    session.error(&["export", "--output", "missing/output.png"]);
    assert!(!Path::new(&session.path("missing")).exists());
    assert_eq!(fs::read_dir(session.0.path()).unwrap().count(), 1);
}

#[test]
fn a_positional_json_filename_does_not_enable_json_diagnostics() {
    let root = tempfile::tempdir().unwrap();
    for json in [false, true] {
        for extra in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_painter"));
            command.current_dir(root.path());
            if json {
                command.arg("--json");
            }
            command.args(["project", "show", "--", "--json"]);
            if extra {
                command.arg("extra");
            }
            let result = command.output().unwrap();
            assert_eq!(result.status.code(), Some(if extra { 2 } else { 1 }));
            assert!(result.stdout.is_empty());
            if json {
                let error: Value = serde_json::from_slice(&result.stderr).unwrap();
                assert_eq!(error["ok"], false);
            } else {
                assert!(
                    String::from_utf8(result.stderr)
                        .unwrap()
                        .starts_with("error: ")
                );
            }
        }
    }
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}
