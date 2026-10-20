// SPDX-License-Identifier: GPL-3.0-only
//! Engine-neutral anchors and visual parts, separate from editable raster documents.

use crate::{Canvas, Document, Error, ErrorCode, Result, drawing::Point, storage};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anchors {
    #[serde(default)]
    pub ground: Option<Point>,
    #[serde(default, deserialize_with = "crate::limits::map::<'de, _, _, 32>")]
    pub attachments: BTreeMap<String, Point>,
    #[serde(default, deserialize_with = "crate::limits::sequence::<'de, _, _, 64>")]
    pub footprint: Vec<Point>,
}
impl Anchors {
    pub fn validate(&self) -> Result<()> {
        if self.attachments.len() > 32
            || self.footprint.len() > 64
            || matches!(self.footprint.len(), 1 | 2)
        {
            return Err(Error::invalid(
                "anchors allow 32 attachments and a 3..=64-point footprint",
            ));
        }
        for name in self.attachments.keys() {
            crate::document::name_valid(name)?;
        }
        for point in self
            .ground
            .iter()
            .chain(self.attachments.values())
            .chain(&self.footprint)
        {
            point.validate()?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub id: String,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 128>")]
    pub layers: Vec<String>,
    #[serde(default)]
    pub order: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub version: u32,
    #[serde(default)]
    pub anchors: Anchors,
    #[serde(default, deserialize_with = "crate::limits::map::<'de, _, _, 256>")]
    pub frames: BTreeMap<String, Anchors>,
    #[serde(default, deserialize_with = "crate::limits::sequence::<'de, _, _, 32>")]
    pub parts: Vec<Part>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FramePlacement {
    pub name: String,
    pub anchors: Anchors,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportPart {
    pub id: String,
    pub order: i32,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 256>")]
    pub files: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 256>")]
    pub frames: Vec<FramePlacement>,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 32>")]
    pub parts: Vec<ExportPart>,
}
impl Placement {
    pub fn validate(&self, document: &Document) -> Result<()> {
        if self.version != 1 {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "placement version must be 1",
            ));
        }
        self.anchors.validate()?;
        if self.frames.len() > 256 || self.parts.len() > 32 {
            return Err(Error::limit("placement limits exceeded"));
        }
        for (name, anchors) in &self.frames {
            let index = document
                .frames()
                .iter()
                .position(|f| &f.name == name)
                .ok_or_else(|| Error::invalid(format!("unknown placement frame {name:?}")))?;
            anchors.validate()?;
            self.frame_anchors(document, index)?;
        }
        let mut ids = BTreeSet::new();
        let mut layers = BTreeSet::new();
        for part in &self.parts {
            crate::document::name_valid(&part.id)?;
            if !ids.insert(&part.id) || part.layers.is_empty() || part.layers.len() > 128 {
                return Err(Error::invalid(
                    "parts require unique IDs and nonempty layer lists",
                ));
            }
            for name in &part.layers {
                document.layer_index(name)?;
                if !layers.insert(name) {
                    return Err(Error::invalid(
                        "split parts require disjoint layer selections",
                    ));
                }
            }
        }
        Ok(())
    }
    pub fn frame_anchors(&self, document: &Document, index: usize) -> Result<Anchors> {
        let frame = document
            .frames()
            .get(index)
            .ok_or_else(|| Error::invalid("placement frame index out of range"))?;
        let mut anchors = self.anchors.clone();
        if let Some(frame) = self.frames.get(&frame.name) {
            if frame.ground.is_some() {
                anchors.ground = frame.ground;
            }
            anchors.attachments.extend(frame.attachments.clone());
            if !frame.footprint.is_empty() {
                anchors.footprint = frame.footprint.clone();
            }
        }
        if anchors.ground.is_none() {
            anchors.ground = frame.pivot;
        }
        anchors.validate()?;
        Ok(anchors)
    }
    pub(crate) fn write(&self, document: &Document, destination: &Path) -> Result<()> {
        self.validate(document)?;
        let mut parts = Vec::new();
        if !self.parts.is_empty() {
            fs::create_dir(destination.join("parts"))?;
        }
        for (index, part) in self.parts.iter().enumerate() {
            fs::create_dir(destination.join(format!("parts/{index:04}")))?;
            let indices = part
                .layers
                .iter()
                .map(|name| document.layer_index(name))
                .collect::<Result<Vec<_>>>()?;
            let mut files = Vec::new();
            for (frame_index, frame) in document.frames().iter().enumerate() {
                let mut canvas = Canvas::new(document.width(), document.height())?;
                for (layer, cel) in document
                    .layers()
                    .iter()
                    .zip(&frame.cels)
                    .enumerate()
                    .filter_map(|(i, (layer, cel))| indices.contains(&i).then_some((layer, cel)))
                {
                    if layer.visible {
                        for (target, source) in canvas.pixels_mut().iter_mut().zip(cel.pixels()) {
                            *target = source.over(*target, layer.opacity);
                        }
                    }
                }
                let file = format!("parts/{index:04}/{frame_index:04}.png");
                storage::export_png(&destination.join(&file), &canvas)?;
                files.push(file);
            }
            parts.push(ExportPart {
                id: part.id.clone(),
                order: part.order,
                files,
            });
        }
        let frames = document
            .frames()
            .iter()
            .enumerate()
            .map(|(index, frame)| {
                Ok(FramePlacement {
                    name: frame.name.clone(),
                    anchors: self.frame_anchors(document, index)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        storage::write_json(
            &destination.join("placement.json"),
            &Metadata {
                version: 1,
                width: document.width(),
                height: document.height(),
                frames,
                parts,
            },
            true,
        )
    }
}

impl Metadata {
    /// Validate the sidecar before resolving any referenced part files.
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "placement metadata version must be 1",
            ));
        }
        if self.width == 0
            || self.height == 0
            || u64::from(self.width) * u64::from(self.height) > crate::canvas::MAX_PIXELS
        {
            return Err(Error::limit("invalid placement dimensions"));
        }
        if self.frames.is_empty() || self.frames.len() > 256 || self.parts.len() > 32 {
            return Err(Error::limit(
                "placement metadata frame or part limit exceeded",
            ));
        }
        let mut names = BTreeSet::new();
        for frame in &self.frames {
            crate::document::name_valid(&frame.name)?;
            frame.anchors.validate()?;
            if !names.insert(&frame.name) {
                return Err(Error::invalid("duplicate placement frame"));
            }
        }
        names.clear();
        for part in &self.parts {
            crate::document::name_valid(&part.id)?;
            if !names.insert(&part.id) || part.files.len() != self.frames.len() {
                return Err(Error::invalid(
                    "parts require unique IDs and one file per frame",
                ));
            }
            let mut files = BTreeSet::new();
            for file in &part.files {
                if file.is_empty() || !files.insert(file) {
                    return Err(Error::invalid("part files must be nonempty and unique"));
                }
            }
        }
        Ok(())
    }
}
