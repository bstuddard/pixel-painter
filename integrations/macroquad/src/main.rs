// SPDX-License-Identifier: GPL-3.0-only
//! A consumer of exported data, deliberately independent of the painter library.

use macroquad::prelude::*;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct SourceRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}
#[derive(Deserialize)]
struct Frame {
    rect: SourceRect,
    duration_ms: u32,
}
#[derive(Deserialize)]
struct Tag {
    frames: Vec<usize>,
}
#[derive(Deserialize)]
struct Manifest {
    version: u32,
    width: u32,
    height: u32,
    frames: Vec<Frame>,
    tags: BTreeMap<String, Tag>,
}

fn manifest() -> Manifest {
    let manifest: Manifest = serde_json::from_str(include_str!(
        "../../../rust/examples/gallery/01-crystal-keeper/export/metadata.json"
    ))
    .expect("valid generated manifest");
    assert_eq!(manifest.version, 1);
    assert!(!manifest.frames.is_empty());
    for frame in &manifest.frames {
        assert!((1..=60_000).contains(&frame.duration_ms));
        assert!(frame.rect.x >= 0 && frame.rect.y >= 0);
        assert!(frame.rect.x as u64 + u64::from(frame.rect.width) <= u64::from(manifest.width));
        assert!(frame.rect.y as u64 + u64::from(frame.rect.height) <= u64::from(manifest.height));
    }
    for tag in manifest.tags.values() {
        assert!(!tag.frames.is_empty());
        assert!(tag.frames.iter().all(|i| *i < manifest.frames.len()));
    }
    manifest
}

/// Choose from the tag's ordered frames using exported durations, not a fixed FPS.
fn frame_at(manifest: &Manifest, indices: &[usize], elapsed_ms: u64) -> usize {
    let total: u64 = indices
        .iter()
        .map(|i| u64::from(manifest.frames[*i].duration_ms))
        .sum();
    let mut remaining = elapsed_ms % total;
    for index in indices {
        let duration = u64::from(manifest.frames[*index].duration_ms);
        if remaining < duration {
            return *index;
        }
        remaining -= duration;
    }
    unreachable!("positive durations cover the complete cycle")
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Pixel Painter — exported animation".into(),
        window_width: 1000,
        window_height: 420,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let manifest = manifest();
    let texture = Texture2D::from_file_with_format(
        include_bytes!("../../../rust/examples/gallery/01-crystal-keeper/export/sheet.png"),
        Some(ImageFormat::Png),
    );
    texture.set_filter(FilterMode::Nearest);
    assert_eq!(texture.width() as u32, manifest.width);
    assert_eq!(texture.height() as u32, manifest.height);
    let smoke = std::env::args().any(|arg| arg == "--smoke-test");
    let mut drawn = 0;
    loop {
        clear_background(Color::from_rgba(24, 37, 53, 255));
        let elapsed_ms = (get_time() * 1000.0) as u64;
        for (column, direction) in ["south", "east", "north", "west"].iter().enumerate() {
            let tag = &manifest.tags[&format!("walk-{direction}")];
            let frame = &manifest.frames[frame_at(&manifest, &tag.frames, elapsed_ms)];
            let rect = &frame.rect;
            let x = 20.0 + column as f32 * 245.0;
            draw_texture_ex(
                &texture,
                x,
                60.0,
                WHITE,
                DrawTextureParams {
                    source: Some(Rect::new(
                        rect.x as f32,
                        rect.y as f32,
                        rect.width as f32,
                        rect.height as f32,
                    )),
                    dest_size: Some(vec2(rect.width as f32 * 4.0, rect.height as f32 * 4.0)),
                    ..Default::default()
                },
            );
            draw_text(direction, x + 55.0, 310.0, 24.0, LIGHTGRAY);
        }
        draw_text(
            "Rectangles, timing and directions come from metadata.json. Esc exits.",
            20.0,
            380.0,
            20.0,
            LIGHTGRAY,
        );
        next_frame().await;
        drawn += 1;
        if is_key_pressed(KeyCode::Escape) || (smoke && drawn >= 3) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_manifest_loads_and_animation_wraps_at_duration_boundaries() {
        let manifest = manifest();
        for direction in ["south", "east", "north", "west"] {
            let indices = &manifest.tags[&format!("walk-{direction}")].frames;
            assert_eq!(indices.len(), 2);
            let first = u64::from(manifest.frames[indices[0]].duration_ms);
            let cycle = first + u64::from(manifest.frames[indices[1]].duration_ms);
            assert_eq!(frame_at(&manifest, indices, 0), indices[0]);
            assert_eq!(frame_at(&manifest, indices, first - 1), indices[0]);
            assert_eq!(frame_at(&manifest, indices, first), indices[1]);
            assert_eq!(frame_at(&manifest, indices, cycle), indices[0]);
        }
    }
}
