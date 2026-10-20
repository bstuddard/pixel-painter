// SPDX-License-Identifier: GPL-3.0-only
//! Regressions for defects found during implementation reviews.

use painter::{
    Canvas, Pixel,
    atlas::{Atlas, Options, RenderedFrame},
    drawing,
    operations::Batch,
    palette::{Palette, Policy},
    storage,
};
use std::{collections::BTreeMap, fs};
#[cfg(feature = "cli")]
use std::{io::Write, process::Command};

#[cfg(all(unix, feature = "cli"))]
#[test]
fn non_utf8_paths_follow_native_filesystem_rules_without_panicking() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    for json_mode in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(OsString::from_vec(vec![
            b'a',
            if json_mode { b'j' } else { b'h' },
            0xff,
        ]));
        // Some Unix filesystems reject non-UTF-8 names. Establish the native
        // behavior independently so rejection must still produce a clean error.
        let rejection = match fs::File::create_new(&path) {
            Ok(file) => {
                drop(file);
                fs::remove_file(&path).unwrap();
                None
            }
            Err(error) => Some(error),
        };
        let mut command = Command::new(env!("CARGO_BIN_EXE_painter"));
        if json_mode {
            command.arg("--json");
        }
        let result = command
            .arg("--file")
            .arg(&path)
            .args(["new", "--width", "1", "--height", "1"])
            .output()
            .unwrap();
        if let Some(error) = rejection {
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            if json_mode {
                let value: serde_json::Value = serde_json::from_slice(&result.stderr).unwrap();
                assert_eq!(value["error"]["code"], "io_error");
                assert_eq!(value["error"]["message"], error.to_string());
            } else {
                assert_eq!(
                    String::from_utf8(result.stderr).unwrap(),
                    format!("error: {error}\n")
                );
            }
            assert!(fs::read_dir(root.path()).unwrap().next().is_none());
            continue;
        }
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(storage::load(&path).unwrap(), Canvas::new(1, 1).unwrap());
        if json_mode {
            let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(value["file"]["encoding"], "unix_bytes");
            use std::os::unix::ffi::OsStrExt;
            assert_eq!(
                value["file"]["bytes"],
                serde_json::json!(path.as_os_str().as_bytes())
            );
        }
    }
}

#[test]
#[cfg(feature = "cli")]
fn compact_1024_canvas_can_be_edited_saved_and_reopened() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("large.json");
    let mut file = std::io::BufWriter::new(fs::File::create(&path).unwrap());
    file.write_all(b"{\"width\":1024,\"height\":1024,\"pixels\":[")
        .unwrap();
    for index in 0..1024 * 1024 {
        if index != 0 {
            file.write_all(b",").unwrap();
        }
        file.write_all(b"{\"r\":0,\"g\":0,\"b\":0,\"a\":0}")
            .unwrap();
    }
    file.write_all(b"]}").unwrap();
    file.flush().unwrap();
    drop(file);
    assert!(fs::metadata(&path).unwrap().len() < storage::MAX_FILE_BYTES);
    let result = Command::new(env!("CARGO_BIN_EXE_painter"))
        .arg("--json")
        .arg("--file")
        .arg(&path)
        .args([
            "set-pixel",
            "--x",
            "1023",
            "--y",
            "1023",
            "--r",
            "255",
            "--g",
            "1",
            "--b",
            "2",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let canvas = storage::load(&path).unwrap();
    assert_eq!(canvas.pixel(1023, 1023), Some(Pixel::rgba(255, 1, 2, 255)));
    assert_eq!(canvas.pixel(0, 0), Some(Pixel::TRANSPARENT));
}

#[test]
fn new_1024_canvas_can_be_persisted() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("new.json");
    let canvas = Canvas::new(1024, 1024).unwrap();
    storage::save(&path, &canvas, true).unwrap();
    assert_eq!(storage::load(&path).unwrap(), canvas);
}

#[test]
#[cfg(feature = "cli")]
fn png_import_checks_trailing_chunks_before_creating_artwork() {
    let root = tempfile::tempdir().unwrap();
    let mut good = Vec::new();
    Canvas::new(2, 2).unwrap().write_png(&mut good).unwrap();
    let mut bad_crc = good.clone();
    *bad_crc.last_mut().unwrap() ^= 1;
    let mut truncated_text = good[..good.len() - 12].to_vec();
    truncated_text.extend_from_slice(b"\0\0\0\x09tEXt");
    for (name, bytes) in [
        ("bad-crc", bad_crc),
        ("missing-crc", good[..good.len() - 4].to_vec()),
        ("truncated-text", truncated_text),
    ] {
        let input = root.path().join(format!("{name}.png"));
        let output = root.path().join(format!("{name}.json"));
        fs::write(&input, bytes).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_painter"))
            .arg("--json")
            .arg("--file")
            .arg(&output)
            .arg("import")
            .arg("--input")
            .arg(&input)
            .output()
            .unwrap();
        assert!(!result.status.success(), "accepted {name}");
        assert!(!output.exists());
        let error: serde_json::Value = serde_json::from_slice(&result.stderr).unwrap();
        assert_eq!(error["error"]["code"], "invalid_png");
    }
}

#[test]
fn wide_and_tall_atlases_use_canvas_bounds_for_internal_placement() {
    for vertical in [false, true] {
        let (width, height) = if vertical { (1, 500_001) } else { (500_001, 1) };
        let frames = (0..3)
            .map(|i| RenderedFrame {
                name: format!("f{i}"),
                canvas: Canvas::from_pixels(
                    width,
                    height,
                    vec![Pixel::rgba(i + 1, 0, 0, 255); 500_001],
                )
                .unwrap(),
                duration_ms: 100,
                pivot: None,
            })
            .collect();
        let atlas = Atlas::pack(
            frames,
            Options {
                columns: Some(if vertical { 1 } else { 3 }),
                padding: 2,
                margin: 1,
            },
        )
        .unwrap();
        for (index, frame) in atlas.metadata().frames.iter().enumerate() {
            assert_eq!(
                atlas
                    .sheet()
                    .pixel(frame.rect.x as u32, frame.rect.y as u32),
                Some(Pixel::rgba(index as u8 + 1, 0, 0, 255))
            );
        }
        assert_eq!(atlas.sheet().pixel(0, 0), Some(Pixel::TRANSPARENT));
    }
}

fn palette() -> Palette {
    Palette {
        version: 1,
        colors: (0..256)
            .map(|n| (format!("c{n:03}"), Pixel::rgba(n as u8, 0, 0, 255)))
            .collect::<BTreeMap<_, _>>(),
    }
}

#[test]
fn palette_work_cannot_exceed_batch_budget_and_failure_rolls_back() {
    let mut canvas = Canvas::from_pixels(
        512,
        1024,
        (0..512 * 1024)
            .map(|i| Pixel::rgba(i as u8, (i >> 8) as u8, (i >> 16) as u8, 255))
            .collect(),
    )
    .unwrap();
    let before = canvas.clone();
    let mut batch = Batch::new(vec![]);
    batch.palette = Some(palette());
    batch.policy = Policy::Nearest;
    let error = batch.apply(&mut canvas).unwrap_err();
    assert_eq!(error.code.as_str(), "resource_limit");
    assert_eq!(canvas, before);
    assert!(canvas.pixels().len() as u64 * 256 > drawing::MAX_WORK);
}

#[test]
fn repeated_palette_colors_are_cached_without_changing_tie_breaking() {
    let mut canvas =
        Canvas::from_pixels(512, 1024, vec![Pixel::rgba(255, 255, 255, 255); 512 * 1024]).unwrap();
    let mut batch = Batch::new(vec![]);
    batch.palette = Some(palette());
    batch.policy = Policy::Nearest;
    batch.apply(&mut canvas).unwrap();
    assert!(
        canvas
            .pixels()
            .iter()
            .all(|p| *p == Pixel::rgba(255, 0, 0, 255))
    );
}

#[test]
fn valid_png_metadata_after_image_data_is_accepted() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("metadata.png");
    let mut encoder = png::Encoder::new(fs::File::create(&path).unwrap(), 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&[1, 2, 3, 128]).unwrap();
    writer
        .write_text_chunk(&png::text_metadata::TEXtChunk::new(
            "note",
            "valid trailing metadata",
        ))
        .unwrap();
    writer.finish().unwrap();
    assert_eq!(
        storage::import_png(&path, false).unwrap().pixel(0, 0),
        Some(Pixel::rgba(1, 2, 3, 128))
    );
}

#[test]
fn standalone_palette_mapping_rolls_back_on_budget_failure() {
    let mut canvas = Canvas::from_pixels(
        512,
        1024,
        (0..512 * 1024)
            .map(|i| Pixel::rgba(i as u8, (i >> 8) as u8, (i >> 16) as u8, 255))
            .collect(),
    )
    .unwrap();
    let before = canvas.clone();
    assert_eq!(
        palette()
            .constrain(&mut canvas, Policy::Nearest)
            .unwrap_err()
            .code
            .as_str(),
        "resource_limit"
    );
    assert_eq!(canvas, before);
}

#[test]
fn batch_render_computes_without_mutating_source() {
    let canvas = Canvas::new(2, 2).unwrap();
    let before = canvas.clone();
    let batch = Batch::new(vec![painter::operations::Operation::Clear {
        color: Some(painter::palette::Color::Rgba(Pixel::rgba(255, 0, 0, 255))),
    }]);
    let (rendered, summary) = batch.render(&canvas).unwrap();
    assert_eq!(canvas, before);
    assert!(
        rendered
            .pixels()
            .iter()
            .all(|p| *p == Pixel::rgba(255, 0, 0, 255))
    );
    assert_eq!(summary.changed_pixels, 4);
}

#[test]
fn tiles_slicing_uses_internal_canvas_coordinates_and_preserves_pixels() {
    let mut pixels = vec![Pixel::rgba(1, 2, 3, 255); 1_500_003];
    pixels[1_000_002] = Pixel::rgba(9, 8, 7, 255);
    let canvas = Canvas::from_pixels(1_500_003, 1, pixels).unwrap();
    let tiles = painter::tileset::Tileset::slice(&canvas, 500_001, 1, 0, 0).unwrap();
    let atlas = tiles
        .atlas(
            Options {
                columns: Some(3),
                padding: 0,
                margin: 0,
            },
            None,
        )
        .unwrap();
    assert_eq!(atlas.sheet(), &canvas);
}

#[test]
fn artwork_preflight_rejects_invalid_counts_and_oversized_strings() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("bad.json");
    fs::write(&path, b"{\"width\":1,\"height\":1,\"pixels\":[]}").unwrap();
    assert!(storage::load_artwork(&path).is_err());
    let value = serde_json::json!({"width":1,"height":1,"pixels":[{"r":0,"g":0,"b":0,"a":0}],"extra":"x".repeat(4097)});
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        storage::load_artwork(&path).unwrap_err().code.as_str(),
        "resource_limit"
    );
}

#[test]
fn fully_clipped_shapes_are_noops_and_do_not_advance_the_noise_stream() {
    use painter::{operations::Operation, palette::Color};
    let canvas = Canvas::new(1, 200_000).unwrap();
    let color = Color::Rgba(Pixel::rgba(1, 2, 3, 255));
    let mut operations = Vec::new();
    for x in [-1, 1] {
        for _ in 0..100 {
            operations.extend([
                Operation::FillRect {
                    x,
                    y: 0,
                    width: 1,
                    height: 200_000,
                    color: color.clone(),
                    over: true,
                },
                Operation::Ellipse {
                    x,
                    y: 0,
                    width: 1,
                    height: 200_000,
                    color: color.clone(),
                    fill: true,
                    over: true,
                },
                Operation::Noise {
                    x,
                    y: 0,
                    width: 1,
                    height: 200_000,
                    colors: vec![color.clone()],
                    density: 255,
                },
            ]);
        }
    }
    let (unchanged, summary) = Batch::new(operations.clone()).render(&canvas).unwrap();
    assert_eq!(unchanged, canvas);
    assert_eq!(summary.changed_pixels, 0);

    let noise = Operation::Noise {
        x: 0,
        y: 0,
        width: 1,
        height: 8,
        colors: vec![color, Color::Rgba(Pixel::rgba(9, 8, 7, 255))],
        density: 128,
    };
    operations.push(noise.clone());
    let expected = Batch::new(vec![noise]).render(&canvas).unwrap().0;
    assert_eq!(Batch::new(operations).render(&canvas).unwrap().0, expected);
}

#[test]
fn inspection_counts_repeated_rgba_colors_palette_aliases_and_hidden_rgb() {
    let opaque = Pixel::rgba(1, 2, 3, 255);
    let partial = Pixel::rgba(1, 2, 3, 128);
    let hidden = Pixel::rgba(9, 8, 7, 0);
    let canvas = Canvas::from_pixels(
        8,
        1,
        vec![
            opaque,
            opaque,
            opaque,
            partial,
            partial,
            hidden,
            hidden,
            Pixel::TRANSPARENT,
        ],
    )
    .unwrap();
    let palette = Palette {
        version: 1,
        colors: BTreeMap::from([
            ("ink".into(), opaque),
            ("alias".into(), opaque),
            ("hidden".into(), hidden),
        ]),
    };
    let report = painter::inspect::inspect(&canvas, Some(&palette));
    assert_eq!(
        report.colors,
        BTreeMap::from([
            ("#010203FF".into(), 3),
            ("#01020380".into(), 2),
            ("#09080700".into(), 2),
            ("#00000000".into(), 1),
        ])
    );
    assert_eq!(report.unique_colors, 4);
    assert_eq!(
        report.palette_usage,
        BTreeMap::from([("ink".into(), 3), ("alias".into(), 3), ("hidden".into(), 2),])
    );
    assert_eq!(report.off_palette_pixels, 2);
    assert_eq!(report.transparent_pixels, 3);
    assert_eq!(report.partial_alpha_pixels, 2);
    assert_eq!(report.opaque_pixels, 3);
    assert_eq!(report.warnings, ["off_palette_colors"]);
}

#[test]
fn tile_seams_bound_total_edge_work_and_charge_the_correct_axis() {
    use painter::tileset::{Connection, Edge, Rules, Tileset};
    let rules = |edge| Rules {
        version: 1,
        connections: vec![
            Connection {
                from: 0,
                to: 0,
                edge
            };
            16_384
        ],
    };
    // Exactly 16,384 × 4,096 comparisons are permitted.
    let canvas = Canvas::new(1, 4096).unwrap();
    let tiles = Tileset::slice(&canvas, 1, 4096, 0, 0).unwrap();
    let seams = tiles.seams(&rules(Edge::Right)).unwrap();
    assert_eq!(seams.len(), 16_384);
    assert!(seams.iter().all(|s| s.mismatched_pixels == 0));

    for (width, height, costly_edge, cheap_edge) in [
        (1, 4097, Edge::Right, Edge::Down),
        (4097, 1, Edge::Down, Edge::Right),
    ] {
        let canvas = Canvas::new(width, height).unwrap();
        let tiles = Tileset::slice(&canvas, width, height, 0, 0).unwrap();
        assert_eq!(
            tiles.seams(&rules(costly_edge)).unwrap_err().code.as_str(),
            "resource_limit"
        );
        assert_eq!(
            tiles
                .atlas(Options::default(), Some(&rules(costly_edge)))
                .err()
                .unwrap()
                .code
                .as_str(),
            "resource_limit"
        );
        assert_eq!(tiles.seams(&rules(cheap_edge)).unwrap().len(), 16_384);
    }
}

#[test]
#[cfg(feature = "cli")]
fn excessive_seam_work_fails_without_a_report_or_source_changes() {
    use painter::tileset::{Connection, Edge, Rules};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("canvas.json");
    let original = serde_json::to_vec(&Canvas::new(1, 4097).unwrap()).unwrap();
    fs::write(&source, &original).unwrap();
    let rules = Rules {
        version: 1,
        connections: vec![
            Connection {
                from: 0,
                to: 0,
                edge: Edge::Right
            };
            16_384
        ],
    };
    fs::write(
        root.path().join("rules.json"),
        serde_json::to_vec(&rules).unwrap(),
    )
    .unwrap();
    for action in [
        vec!["seams", "--rules", "rules.json"],
        vec![
            "export",
            "--rules",
            "rules.json",
            "--output",
            "bundle",
            "--scale",
            "1",
        ],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_painter"))
            .current_dir(root.path())
            .args([
                "--json",
                "tiles",
                "--tile-width",
                "1",
                "--tile-height",
                "4097",
            ])
            .args(action)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&result.stderr).unwrap();
        assert_eq!(error["error"]["code"], "resource_limit");
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    }
}
