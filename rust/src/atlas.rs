// SPDX-License-Identifier: GPL-3.0-only
//! Deterministic grid packing and manifest-last export bundles.

use crate::{
    Canvas, Document, Error, Result,
    animation::Tag,
    drawing::{self, Point, Rect},
    maps::{Channel, MapRequest},
    storage,
};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::BTreeMap, fs, path::Path};

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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub maps: BTreeMap<Channel, MapMetadata>,
}

/// Companion textures reuse the color frame rectangles and index order.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapMetadata {
    pub sheet: String,
    pub frames: Vec<String>,
    pub encoding: MapEncoding,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub positive_y: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapEncoding {
    Rgba8Emission,
    XyzUnorm8,
}
struct MapImages {
    channel: Channel,
    sheet: Canvas,
    frames: Vec<Canvas>,
}

/// Compute packing once, before raster allocation, and reuse it for every map.
struct Layout {
    width: u32,
    height: u32,
    rects: Vec<Rect>,
}
impl Layout {
    fn new(
        width: u32,
        height: u32,
        count: usize,
        options: Options,
        channels: usize,
    ) -> Result<Self> {
        if count == 0 || count > 4096 {
            return Err(Error::limit("atlas requires 1..=4096 frames"));
        }
        let columns = options
            .columns
            .unwrap_or_else(|| (1..=64).find(|n| n * n >= count as u32).unwrap());
        if columns == 0 || columns > count as u32 {
            return Err(Error::invalid(
                "atlas columns must be between 1 and frame count",
            ));
        }
        if options.padding > 1024 || options.margin > 1024 {
            return Err(Error::limit(
                "atlas margin and padding must not exceed 1024",
            ));
        }
        let extent = |cells: u32, size: u32| {
            u32::try_from(
                2 * u64::from(options.margin)
                    + u64::from(cells) * u64::from(size)
                    + u64::from(cells - 1) * u64::from(options.padding),
            )
            .map_err(|_| Error::limit("atlas dimensions overflow"))
        };
        let sheet_width = extent(columns, width)?;
        let sheet_height = extent((count as u32).div_ceil(columns), height)?;
        let pixels = Canvas::pixel_count(sheet_width, sheet_height)? as u64;
        let frames = u64::from(width) * u64::from(height) * count as u64;
        if pixels * channels as u64 > crate::canvas::MAX_PIXELS
            || frames * channels as u64 > crate::canvas::MAX_PIXELS
        {
            return Err(Error::limit(
                "atlas exceeds aggregate channel/frame pixel limit",
            ));
        }
        let rects = (0..count)
            .map(|index| Rect {
                x: (options.margin + (index as u32 % columns) * (width + options.padding)) as i32,
                y: (options.margin + (index as u32 / columns) * (height + options.padding)) as i32,
                width,
                height,
            })
            .collect();
        Ok(Self {
            width: sheet_width,
            height: sheet_height,
            rects,
        })
    }
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
    maps: Vec<MapImages>,
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
        Self::from_document_with_maps(document, options, MapRequest::default())
    }

    /// Render selected companions using the color layout and visible coverage.
    pub fn from_document_with_maps(
        document: &Document,
        options: Options,
        maps: MapRequest,
    ) -> Result<Self> {
        let layout = Layout::new(
            document.width(),
            document.height(),
            document.frames().len(),
            options,
            1 + maps.channels().count(),
        )?;
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
        let mut atlas = Self::pack_layout(frames, layout)?;
        atlas.metadata.tags = document.tags().clone();
        for channel in maps.channels() {
            let mut sheet = Canvas::new(atlas.sheet.width(), atlas.sheet.height())?;
            let mut frames = Vec::with_capacity(document.frames().len());
            for (index, color) in atlas.frames.iter().enumerate() {
                let image = document.render_map(index, channel, color)?;
                let rect = atlas.metadata.frames[index].rect;
                sheet.copy_from(&image, rect.x as u32, rect.y as u32)?;
                frames.push(image);
            }
            let name = channel.as_str();
            let normal = channel == Channel::Normal;
            atlas.metadata.maps.insert(
                channel,
                MapMetadata {
                    sheet: format!("{name}/sheet.png"),
                    frames: (0..frames.len())
                        .map(|index| format!("{name}/frames/{index:04}.png"))
                        .collect(),
                    encoding: if normal {
                        MapEncoding::XyzUnorm8
                    } else {
                        MapEncoding::Rgba8Emission
                    },
                    space: normal.then(|| "screen".into()),
                    positive_y: normal.then(|| "up".into()),
                    default: normal.then(|| "flat".into()),
                },
            );
            atlas.maps.push(MapImages {
                channel,
                sheet,
                frames,
            });
        }
        Ok(atlas)
    }

    /// Fixed-size cells, stable input order, transparent padding, and no trimming.
    pub fn pack(frames: Vec<RenderedFrame>, options: Options) -> Result<Self> {
        let first = frames
            .first()
            .ok_or_else(|| Error::limit("atlas requires 1..=4096 frames"))?;
        let layout = Layout::new(
            first.canvas.width(),
            first.canvas.height(),
            frames.len(),
            options,
            1,
        )?;
        Self::pack_layout(frames, layout)
    }

    fn pack_layout(frames: Vec<RenderedFrame>, layout: Layout) -> Result<Self> {
        let mut names = std::collections::BTreeSet::new();
        let first = &frames[0].canvas;
        for frame in &frames {
            crate::document::name_valid(&frame.name)?;
            if !names.insert(&frame.name) {
                return Err(Error::invalid("duplicate atlas frame name"));
            }
            if let Some(pivot) = frame.pivot {
                pivot.validate()?;
            }
            if frame.canvas.width() != first.width() || frame.canvas.height() != first.height() {
                return Err(Error::invalid("atlas frames must have equal dimensions"));
            }
            if !(1..=60_000).contains(&frame.duration_ms) {
                return Err(Error::invalid("atlas durations must be 1..=60,000 ms"));
            }
        }
        let mut sheet = Canvas::new(layout.width, layout.height)?;
        let mut metadata = Metadata {
            version: 1,
            image: "sheet.png".into(),
            preview: "preview.png".into(),
            animation: (frames.len() > 1).then(|| "animation.png".into()),
            width: sheet.width(),
            height: sheet.height(),
            frames: Vec::with_capacity(frames.len()),
            tags: BTreeMap::new(),
            tileset: None,
            maps: BTreeMap::new(),
        };
        let mut images = Vec::with_capacity(frames.len());
        for (index, (frame, rect)) in frames.into_iter().zip(layout.rects).enumerate() {
            sheet.copy_from(&frame.canvas, rect.x as u32, rect.y as u32)?;
            metadata.frames.push(FrameMetadata {
                name: frame.name,
                file: format!("frames/{index:04}.png"),
                rect,
                duration_ms: frame.duration_ms,
                pivot: frame.pivot,
            });
            images.push(frame.canvas);
        }
        Ok(Self {
            sheet,
            metadata,
            frames: images,
            maps: Vec::new(),
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
        for map in &self.maps {
            for (index, image) in map.frames.iter().enumerate() {
                if document.render_map(index, map.channel, &self.frames[index])? != *image {
                    return Err(Error::invalid(
                        "placement document maps do not match the atlas",
                    ));
                }
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
        let preview = if scale == 1 {
            Cow::Borrowed(&self.sheet)
        } else {
            Cow::Owned(drawing::resize(
                &self.sheet,
                self.sheet
                    .width()
                    .checked_mul(scale)
                    .ok_or_else(|| Error::limit("preview width overflow"))?,
                self.sheet
                    .height()
                    .checked_mul(scale)
                    .ok_or_else(|| Error::limit("preview height overflow"))?,
            )?)
        };
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
        for map in &self.maps {
            let directory = staged.path().join(map.channel.as_str());
            fs::create_dir(&directory)?;
            fs::create_dir(directory.join("frames"))?;
            storage::export_png(&directory.join("sheet.png"), &map.sheet)?;
            for (index, frame) in map.frames.iter().enumerate() {
                storage::export_png(&directory.join(format!("frames/{index:04}.png")), frame)?;
            }
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
