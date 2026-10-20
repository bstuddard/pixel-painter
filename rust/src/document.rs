// SPDX-License-Identifier: GPL-3.0-only
//! Versioned layered documents. All frames share layer definitions and dimensions.

use crate::{
    Canvas, Error, ErrorCode, Result,
    animation::{Frame, Tag},
    canvas::MAX_PIXELS,
    drawing::{self, Point},
    operations::{Batch, Summary},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub opacity: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DocumentData")]
pub struct Document {
    version: u32,
    width: u32,
    height: u32,
    layers: Vec<Layer>,
    frames: Vec<Frame>,
    tags: BTreeMap<String, Tag>,
    active_layer: usize,
    active_frame: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentData {
    version: u32,
    width: u32,
    height: u32,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 128>")]
    layers: Vec<Layer>,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 256>")]
    frames: Vec<Frame>,
    #[serde(default, deserialize_with = "crate::limits::map::<'de, _, _, 256>")]
    tags: BTreeMap<String, Tag>,
    #[serde(default)]
    active_layer: usize,
    #[serde(default)]
    active_frame: usize,
}
impl TryFrom<DocumentData> for Document {
    type Error = Error;
    fn try_from(data: DocumentData) -> Result<Self> {
        let document = Self {
            version: data.version,
            width: data.width,
            height: data.height,
            layers: data.layers,
            frames: data.frames,
            tags: data.tags,
            active_layer: data.active_layer,
            active_frame: data.active_frame,
        };
        document.validate()?;
        Ok(document)
    }
}

pub(crate) fn name_valid(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err(Error::invalid(
            "names require 1..=128 bytes without control characters",
        ));
    }
    Ok(())
}

impl Document {
    pub fn new(width: u32, height: u32) -> Result<Self> {
        Self::from_canvas(Canvas::new(width, height)?)
    }

    /// Explicitly migrate legacy artwork without modifying its source file.
    pub fn from_canvas(canvas: Canvas) -> Result<Self> {
        Ok(Self {
            version: 1,
            width: canvas.width(),
            height: canvas.height(),
            layers: vec![Layer {
                name: "base".into(),
                visible: true,
                opacity: 255,
            }],
            frames: vec![Frame {
                name: "frame-0".into(),
                duration_ms: 100,
                pivot: None,
                cels: vec![canvas],
            }],
            tags: BTreeMap::new(),
            active_layer: 0,
            active_frame: 0,
        })
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }
    pub fn tags(&self) -> &BTreeMap<String, Tag> {
        &self.tags
    }
    pub fn active_layer(&self) -> usize {
        self.active_layer
    }
    pub fn active_frame(&self) -> usize {
        self.active_frame
    }

    fn check_capacity(&self, layers: usize, frames: usize) -> Result<()> {
        if layers == 0 || layers > 128 || frames == 0 || frames > 256 {
            return Err(Error::limit(
                "projects require 1..=128 layers and 1..=256 frames",
            ));
        }
        let cells = (layers as u64)
            .checked_mul(frames as u64)
            .and_then(|n| n.checked_mul(u64::from(self.width) * u64::from(self.height)))
            .ok_or_else(|| Error::limit("project pixel count overflow"))?;
        if cells > MAX_PIXELS {
            return Err(Error::limit(
                "project exceeds 16,777,216 aggregate cel pixels",
            ));
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "project version must be 1",
            ));
        }
        Canvas::pixel_count(self.width, self.height)?;
        self.check_capacity(self.layers.len(), self.frames.len())?;
        if self.active_layer >= self.layers.len() || self.active_frame >= self.frames.len() {
            return Err(Error::invalid("invalid active layer or frame"));
        }
        let mut names = BTreeSet::new();
        for layer in &self.layers {
            name_valid(&layer.name)?;
            if !names.insert(&layer.name) {
                return Err(Error::invalid("duplicate layer name"));
            }
        }
        names.clear();
        for frame in &self.frames {
            name_valid(&frame.name)?;
            if !names.insert(&frame.name) {
                return Err(Error::invalid("duplicate frame name"));
            }
            if !(1..=60_000).contains(&frame.duration_ms) {
                return Err(Error::invalid("frame duration must be 1..=60,000 ms"));
            }
            if let Some(pivot) = frame.pivot {
                pivot.validate()?;
            }
            if frame.cels.len() != self.layers.len()
                || frame
                    .cels
                    .iter()
                    .any(|c| c.width() != self.width || c.height() != self.height)
            {
                return Err(Error::invalid(
                    "all frames require one equal-sized cel per layer",
                ));
            }
        }
        if self.tags.len() > 256 {
            return Err(Error::limit("project exceeds 256 tags"));
        }
        for (name, tag) in &self.tags {
            name_valid(name)?;
            self.validate_tag(tag)?;
        }
        Ok(())
    }

    fn validate_tag(&self, tag: &Tag) -> Result<()> {
        if tag.frames.is_empty()
            || tag.frames.len() > 256
            || tag.frames.iter().any(|i| *i >= self.frames.len())
        {
            return Err(Error::invalid("tags require 1..=256 valid frame indices"));
        }
        if let Some(direction) = &tag.direction {
            name_valid(direction)?;
        }
        Ok(())
    }

    pub fn layer_index(&self, name: &str) -> Result<usize> {
        self.layers
            .iter()
            .position(|l| l.name == name)
            .ok_or_else(|| Error::new(ErrorCode::NotFound, format!("unknown layer {name:?}")))
    }
    pub fn select_layer(&mut self, name: &str) -> Result<()> {
        self.active_layer = self.layer_index(name)?;
        Ok(())
    }
    pub fn select_frame(&mut self, index: usize) -> Result<()> {
        if index >= self.frames.len() {
            return Err(Error::new(
                ErrorCode::OutOfBounds,
                "frame index is outside project",
            ));
        }
        self.active_frame = index;
        Ok(())
    }

    pub fn edit(&mut self, batch: &Batch) -> Result<Summary> {
        let (cel, summary) =
            batch.render(&self.frames[self.active_frame].cels[self.active_layer])?;
        if cel.width() != self.width || cel.height() != self.height {
            return Err(Error::invalid(
                "cel edits cannot change project dimensions; transform flat canvases or use project resize",
            ));
        }
        self.frames[self.active_frame].cels[self.active_layer] = cel;
        Ok(summary)
    }

    /// Composite bottom to top over transparent pixels using each layer's opacity.
    pub fn flatten(&self, index: usize) -> Result<Canvas> {
        let frame = self
            .frames
            .get(index)
            .ok_or_else(|| Error::new(ErrorCode::OutOfBounds, "frame index is outside project"))?;
        let mut result = Canvas::new(self.width, self.height)?;
        for (layer, cel) in self
            .layers
            .iter()
            .zip(&frame.cels)
            .filter(|(l, _)| l.visible)
        {
            for (target, source) in result.pixels_mut().iter_mut().zip(cel.pixels()) {
                *target = source.over(*target, layer.opacity);
            }
        }
        Ok(result)
    }

    pub fn add_layer(&mut self, name: String) -> Result<()> {
        name_valid(&name)?;
        if self.layers.iter().any(|l| l.name == name) {
            return Err(Error::invalid("duplicate layer name"));
        }
        self.check_capacity(self.layers.len() + 1, self.frames.len())?;
        let blank = Canvas::new(self.width, self.height)?;
        for frame in &mut self.frames {
            frame.cels.push(blank.clone());
        }
        self.layers.push(Layer {
            name,
            visible: true,
            opacity: 255,
        });
        self.active_layer = self.layers.len() - 1;
        Ok(())
    }

    pub fn duplicate_layer(&mut self, name: &str, new_name: String) -> Result<()> {
        let source = self.layer_index(name)?;
        self.add_layer(new_name)?;
        let target = self.active_layer;
        self.layers[target].visible = self.layers[source].visible;
        self.layers[target].opacity = self.layers[source].opacity;
        for frame in &mut self.frames {
            frame.cels[target] = frame.cels[source].clone();
        }
        Ok(())
    }
    pub fn configure_layer(
        &mut self,
        name: &str,
        visible: Option<bool>,
        opacity: Option<u8>,
    ) -> Result<()> {
        let index = self.layer_index(name)?;
        if let Some(value) = visible {
            self.layers[index].visible = value;
        }
        if let Some(value) = opacity {
            self.layers[index].opacity = value;
        }
        Ok(())
    }
    pub fn reorder_layer(&mut self, name: &str, index: usize) -> Result<()> {
        if index >= self.layers.len() {
            return Err(Error::new(
                ErrorCode::OutOfBounds,
                "layer index is outside project",
            ));
        }
        let source = self.layer_index(name)?;
        let active = self.layers[self.active_layer].name.clone();
        let layer = self.layers.remove(source);
        self.layers.insert(index, layer);
        for frame in &mut self.frames {
            let cel = frame.cels.remove(source);
            frame.cels.insert(index, cel);
        }
        self.active_layer = self.layer_index(&active)?;
        Ok(())
    }
    pub fn delete_layer(&mut self, name: &str) -> Result<()> {
        if self.layers.len() == 1 {
            return Err(Error::invalid("cannot delete the final layer"));
        }
        let index = self.layer_index(name)?;
        self.layers.remove(index);
        for frame in &mut self.frames {
            frame.cels.remove(index);
        }
        if self.active_layer >= index {
            self.active_layer = self.active_layer.saturating_sub(1);
        }
        Ok(())
    }
    pub fn merge_layer(&mut self, name: &str) -> Result<()> {
        let mut merged = self.clone();
        merged.merge_layer_inner(name)?;
        // Integer source-over is not perfectly associative. Never silently change
        // the visible pixels when an isolated pair is placed over lower layers.
        for index in 0..self.frames.len() {
            if self.flatten(index)? != merged.flatten(index)? {
                return Err(Error::new(
                    ErrorCode::MergeChangesComposite,
                    "byte rounding would change the composite over lower layers; keep these layers separate",
                ));
            }
        }
        *self = merged;
        Ok(())
    }

    fn merge_layer_inner(&mut self, name: &str) -> Result<()> {
        let index = self.layer_index(name)?;
        if index == 0 {
            return Err(Error::invalid(
                "bottom layer has no layer beneath it to merge into",
            ));
        }
        let lower = self.layers[index - 1].clone();
        let upper = self.layers[index].clone();
        for frame in &mut self.frames {
            let mut merged = Canvas::new(self.width, self.height)?;
            for (layer, cel) in [
                (&lower, &frame.cels[index - 1]),
                (&upper, &frame.cels[index]),
            ] {
                if layer.visible {
                    for (target, source) in merged.pixels_mut().iter_mut().zip(cel.pixels()) {
                        *target = source.over(*target, layer.opacity);
                    }
                }
            }
            frame.cels[index - 1] = merged;
        }
        self.layers[index - 1].visible = true;
        self.layers[index - 1].opacity = 255;
        self.delete_layer(name)?;
        self.active_layer = index - 1;
        Ok(())
    }

    pub fn add_frame(
        &mut self,
        name: String,
        duplicate: Option<usize>,
        duration_ms: u32,
    ) -> Result<()> {
        name_valid(&name)?;
        if self.frames.iter().any(|f| f.name == name) {
            return Err(Error::invalid("duplicate frame name"));
        }
        if !(1..=60_000).contains(&duration_ms) {
            return Err(Error::invalid("frame duration must be 1..=60,000 ms"));
        }
        self.check_capacity(self.layers.len(), self.frames.len() + 1)?;
        let (cels, pivot) = match duplicate {
            Some(index) => {
                let frame = self.frames.get(index).ok_or_else(|| {
                    Error::new(ErrorCode::OutOfBounds, "frame index is outside project")
                })?;
                (frame.cels.clone(), frame.pivot)
            }
            None => (
                vec![Canvas::new(self.width, self.height)?; self.layers.len()],
                None,
            ),
        };
        self.frames.push(Frame {
            name,
            duration_ms,
            pivot,
            cels,
        });
        self.active_frame = self.frames.len() - 1;
        Ok(())
    }
    pub fn configure_frame(
        &mut self,
        index: usize,
        duration: Option<u32>,
        pivot: Option<Point>,
    ) -> Result<()> {
        if let Some(value) = duration
            && !(1..=60_000).contains(&value)
        {
            return Err(Error::invalid("frame duration must be 1..=60,000 ms"));
        }
        if let Some(value) = pivot {
            value.validate()?;
        }
        let frame = self
            .frames
            .get_mut(index)
            .ok_or_else(|| Error::new(ErrorCode::OutOfBounds, "frame index is outside project"))?;
        if let Some(value) = duration {
            frame.duration_ms = value;
        }
        if let Some(value) = pivot {
            frame.pivot = Some(value);
        }
        Ok(())
    }
    fn remap_tags(&mut self, old_names: &[String]) {
        for tag in self.tags.values_mut() {
            tag.frames = tag
                .frames
                .iter()
                .filter_map(|i| self.frames.iter().position(|f| f.name == old_names[*i]))
                .collect();
        }
        self.tags.retain(|_, tag| !tag.frames.is_empty());
    }
    pub fn reorder_frame(&mut self, source: usize, target: usize) -> Result<()> {
        if source >= self.frames.len() || target >= self.frames.len() {
            return Err(Error::new(
                ErrorCode::OutOfBounds,
                "frame index is outside project",
            ));
        }
        let old: Vec<_> = self.frames.iter().map(|f| f.name.clone()).collect();
        let active = self.frames[self.active_frame].name.clone();
        let frame = self.frames.remove(source);
        self.frames.insert(target, frame);
        self.active_frame = self.frames.iter().position(|f| f.name == active).unwrap();
        self.remap_tags(&old);
        Ok(())
    }
    pub fn delete_frame(&mut self, index: usize) -> Result<()> {
        if self.frames.len() == 1 {
            return Err(Error::invalid("cannot delete the final frame"));
        }
        if index >= self.frames.len() {
            return Err(Error::new(
                ErrorCode::OutOfBounds,
                "frame index is outside project",
            ));
        }
        let old: Vec<_> = self.frames.iter().map(|f| f.name.clone()).collect();
        self.frames.remove(index);
        if self.active_frame >= index {
            self.active_frame = self.active_frame.saturating_sub(1);
        }
        self.remap_tags(&old);
        Ok(())
    }
    pub fn set_tag(&mut self, name: String, tag: Tag) -> Result<()> {
        name_valid(&name)?;
        self.validate_tag(&tag)?;
        if self.tags.len() >= 256 && !self.tags.contains_key(&name) {
            return Err(Error::limit("project exceeds 256 tags"));
        }
        self.tags.insert(name, tag);
        Ok(())
    }
    pub fn delete_tag(&mut self, name: &str) -> Result<()> {
        self.tags
            .remove(name)
            .ok_or_else(|| Error::new(ErrorCode::NotFound, "unknown tag"))?;
        Ok(())
    }

    /// Resize every cel together, preserving the frame/layer dimension invariant.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        let count = Canvas::pixel_count(width, height)? as u64;
        if count * self.frames.len() as u64 * self.layers.len() as u64 > MAX_PIXELS {
            return Err(Error::limit(
                "resized project exceeds aggregate pixel limit",
            ));
        }
        let mut edited = self.clone();
        for frame in &mut edited.frames {
            for cel in &mut frame.cels {
                *cel = drawing::resize(cel, width, height)?;
            }
        }
        edited.width = width;
        edited.height = height;
        *self = edited;
        Ok(())
    }
}
