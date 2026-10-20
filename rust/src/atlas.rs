// SPDX-License-Identifier: GPL-3.0-only
//! Deterministic grid packing and manifest-last export bundles.

use crate::{
    Canvas, Document, Error, Result,
    animation::Tag,
    drawing::{self, Point, Rect},
    storage,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    pub columns: Option<u32>,
    pub padding: u32,
    pub margin: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameMetadata {
    pub name: String,
    pub file: String,
    pub rect: Rect,
    pub duration_ms: u32,
    pub pivot: Option<Point>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub version: u32,
    pub image: String,
    pub preview: String,
    pub animation: Option<String>,
    pub width: u32,
    pub height: u32,
    pub frames: Vec<FrameMetadata>,
    pub tags: BTreeMap<String, Tag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tileset: Option<crate::tileset::TileMetadata>,
}

pub struct RenderedFrame {
    pub name: String,
    pub canvas: Canvas,
    pub duration_ms: u32,
    pub pivot: Option<Point>,
}
pub struct Atlas {
    sheet: Canvas,
    metadata: Metadata,
    frames: Vec<Canvas>,
}

impl Atlas {
    pub fn sheet(&self) -> &Canvas {
        &self.sheet
    }
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
    pub(crate) fn set_tileset(&mut self, tileset: crate::tileset::TileMetadata) {
        self.metadata.animation = None;
        self.metadata.tileset = Some(tileset);
    }
    pub fn from_document(document: &Document, options: Options) -> Result<Self> {
        let frames = document
            .frames()
            .iter()
            .enumerate()
            .map(|(index, frame)| {
                Ok(RenderedFrame {
                    name: frame.name.clone(),
                    canvas: document.flatten(index)?,
                    duration_ms: frame.duration_ms,
                    pivot: frame.pivot,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut atlas = Self::pack(frames, options)?;
        atlas.metadata.tags = document.tags().clone();
        Ok(atlas)
    }

    /// Fixed-size cells, stable input order, transparent padding, and no trimming.
    pub fn pack(frames: Vec<RenderedFrame>, options: Options) -> Result<Self> {
        if frames.is_empty() || frames.len() > 4096 {
            return Err(Error::limit("atlas requires 1..=4096 frames"));
        }
        let mut names = std::collections::BTreeSet::new();
        for frame in &frames {
            crate::document::name_valid(&frame.name)?;
            if !names.insert(&frame.name) {
                return Err(Error::invalid("duplicate atlas frame name"));
            }
            if let Some(pivot) = frame.pivot {
                pivot.validate()?;
            }
        }
        let width = frames[0].canvas.width();
        let height = frames[0].canvas.height();
        if frames
            .iter()
            .any(|f| f.canvas.width() != width || f.canvas.height() != height)
        {
            return Err(Error::invalid("atlas frames must have equal dimensions"));
        }
        if frames
            .iter()
            .any(|f| !(1..=60_000).contains(&f.duration_ms))
        {
            return Err(Error::invalid("atlas durations must be 1..=60,000 ms"));
        }
        if u64::from(width) * u64::from(height) * frames.len() as u64 > crate::canvas::MAX_PIXELS {
            return Err(Error::limit("atlas exceeds aggregate frame pixel limit"));
        }
        let columns = options.columns.unwrap_or_else(|| {
            let mut n = 1;
            while n * n < frames.len() as u32 {
                n += 1;
            }
            n
        });
        if columns == 0 || columns > frames.len() as u32 {
            return Err(Error::invalid(
                "atlas columns must be between 1 and frame count",
            ));
        }
        if options.padding > 1024 || options.margin > 1024 {
            return Err(Error::limit(
                "atlas margin and padding must not exceed 1024",
            ));
        }
        let rows = (frames.len() as u32).div_ceil(columns);
        let extent = |cells: u32, size: u32| -> Result<u32> {
            u32::try_from(
                2 * u64::from(options.margin)
                    + u64::from(cells) * u64::from(size)
                    + u64::from(cells - 1) * u64::from(options.padding),
            )
            .map_err(|_| Error::limit("atlas dimensions overflow"))
        };
        let mut sheet = Canvas::new(extent(columns, width)?, extent(rows, height)?)?;
        let mut metadata = Metadata {
            version: 1,
            image: "sheet.png".into(),
            preview: "preview.png".into(),
            animation: if frames.len() > 1 {
                Some("animation.png".into())
            } else {
                None
            },
            width: sheet.width(),
            height: sheet.height(),
            frames: Vec::new(),
            tags: BTreeMap::new(),
            tileset: None,
        };
        let mut images = Vec::new();
        for (index, frame) in frames.into_iter().enumerate() {
            let x = options.margin + (index as u32 % columns) * (width + options.padding);
            let y = options.margin + (index as u32 / columns) * (height + options.padding);
            sheet.copy_from(&frame.canvas, x, y)?;
            metadata.frames.push(FrameMetadata {
                name: frame.name,
                file: format!("frames/{index:04}.png"),
                rect: Rect {
                    x: x as i32,
                    y: y as i32,
                    width,
                    height,
                },
                duration_ms: frame.duration_ms,
                pivot: frame.pivot,
            });
            images.push(frame.canvas);
        }
        Ok(Self {
            sheet,
            metadata,
            frames: images,
        })
    }

    /// Export to a new directory. metadata.json is the completion marker.
    /// Existing directories are never replaced, even when empty.
    pub fn export(&self, destination: &Path, scale: u32) -> Result<()> {
        self.export_inner(destination, scale, |_| Ok(()))
    }
    /// Export resolved anchors and split-part images alongside the flattened atlas.
    pub fn export_with_placement(
        &self,
        destination: &Path,
        scale: u32,
        document: &Document,
        placement: &crate::placement::Placement,
    ) -> Result<()> {
        placement.validate(document)?;
        if document.width() != self.frames[0].width()
            || document.height() != self.frames[0].height()
            || document.frames().len() != self.frames.len()
            || document
                .frames()
                .iter()
                .zip(&self.metadata.frames)
                .any(|(a, b)| {
                    a.name != b.name || a.duration_ms != b.duration_ms || a.pivot != b.pivot
                })
        {
            return Err(Error::invalid(
                "placement document does not match the atlas",
            ));
        }
        if document.tags() != &self.metadata.tags {
            return Err(Error::invalid(
                "placement document tags do not match the atlas",
            ));
        }
        for (index, image) in self.frames.iter().enumerate() {
            if document.flatten(index)? != *image {
                return Err(Error::invalid(
                    "placement document pixels do not match the atlas",
                ));
            }
        }
        self.export_inner(destination, scale, |stage| placement.write(document, stage))
    }
    fn export_inner(
        &self,
        destination: &Path,
        scale: u32,
        extra: impl FnOnce(&Path) -> Result<()>,
    ) -> Result<()> {
        if scale == 0 || scale > 64 {
            return Err(Error::invalid("preview scale must be 1..=64"));
        }
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let staged = tempfile::tempdir_in(parent)?;
        let preview = drawing::resize(
            &self.sheet,
            self.sheet
                .width()
                .checked_mul(scale)
                .ok_or_else(|| Error::limit("preview width overflow"))?,
            self.sheet
                .height()
                .checked_mul(scale)
                .ok_or_else(|| Error::limit("preview height overflow"))?,
        )?;
        fs::create_dir(staged.path().join("frames"))?;
        storage::export_png(&staged.path().join("sheet.png"), &self.sheet)?;
        storage::export_png(&staged.path().join("preview.png"), &preview)?;
        for (index, canvas) in self.frames.iter().enumerate() {
            storage::export_png(
                &staged.path().join(format!("frames/{index:04}.png")),
                canvas,
            )?;
        }
        if self.metadata.animation.is_some() {
            self.write_animation(&staged.path().join("animation.png"))?;
        }
        extra(staged.path())?;
        storage::write_json(&staged.path().join("metadata.json"), &self.metadata, true)?;
        publish(staged.path(), destination)
    }

    fn write_animation(&self, path: &Path) -> Result<()> {
        storage::atomic_write(path, true, |output| {
            let first = &self.frames[0];
            let mut encoder = png::Encoder::new(output, first.width(), first.height());
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_animated(self.frames.len() as u32, 0)?;
            let mut writer = encoder.write_header()?;
            for (canvas, metadata) in self.frames.iter().zip(&self.metadata.frames) {
                writer.set_frame_delay(metadata.duration_ms as u16, 1000)?;
                writer.set_blend_op(png::BlendOp::Source)?;
                let bytes: Vec<_> = canvas
                    .pixels()
                    .iter()
                    .flat_map(|p| [p.r, p.g, p.b, p.a])
                    .collect();
                writer.write_image_data(&bytes)?;
            }
            writer.finish()?;
            Ok(())
        })
    }
}

fn publish(staged: &Path, destination: &Path) -> Result<()> {
    storage::publish_bundle(staged, destination, "metadata.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_publish_failure_removes_partial_bundle() {
        let root = tempfile::tempdir().unwrap();
        let staged = root.path().join("stage");
        fs::create_dir(&staged).unwrap();
        fs::write(staged.join("sheet.png"), b"image").unwrap();
        let target = root.path().join("bundle");
        assert!(publish(&staged, &target).is_err());
        assert!(!target.exists());
    }
    #[test]
    fn existing_empty_directory_is_preserved() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("bundle");
        fs::create_dir(&target).unwrap();
        assert!(publish(root.path(), &target).is_err());
        assert!(target.is_dir());
    }
}
