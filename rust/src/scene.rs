// SPDX-License-Identifier: GPL-3.0-only
//! Bounded offline compositions and contact sheets, with deterministic ordering.

use crate::{
    Canvas, Error, ErrorCode, Pixel, Result,
    assets::{AssetRoot, Budget},
    drawing::{self, Point},
    placement,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn one() -> u32 {
    1
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub input: PathBuf,
    pub position: Point,
    #[serde(default = "one")]
    pub scale: u32,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub anchor: Option<Point>,
    #[serde(default)]
    pub frame: usize,
    #[serde(default)]
    pub placement: Option<PathBuf>,
    #[serde(default)]
    pub part: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub background: Pixel,
    #[serde(default)]
    pub tile: Option<PathBuf>,
    #[serde(default)]
    pub y_sort: bool,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 256>")]
    pub assets: Vec<Entry>,
}
struct Loaded {
    canvas: Canvas,
    anchor: Point,
    position: Point,
    order: i32,
    index: usize,
}
impl Scene {
    fn load(&self, root: &AssetRoot, budget: &mut Budget) -> Result<Vec<Loaded>> {
        if self.version != 1 {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "scene version must be 1",
            ));
        }
        if self.assets.is_empty() || self.assets.len() > 256 {
            return Err(Error::invalid("scenes require 1..=256 assets"));
        }
        let mut names = std::collections::BTreeSet::new();
        let mut pixels = 0u64;
        let mut loaded = Vec::new();
        for (index, entry) in self.assets.iter().enumerate() {
            crate::document::name_valid(&entry.id)?;
            if !names.insert(&entry.id) {
                return Err(Error::invalid("duplicate scene asset ID"));
            }
            entry.position.validate()?;
            if entry.scale == 0 || entry.scale > 64 {
                return Err(Error::invalid("asset scale must be 1..=64"));
            }
            let mut input = root.resolve(&entry.input)?;
            let mut anchor = entry.anchor;
            let mut part_order = 0;
            let mut dimensions = None;
            if let Some(path) = &entry.placement {
                let path = root.resolve(path)?;
                let metadata: placement::Metadata = budget.config(&path)?;
                metadata.validate()?;
                dimensions = Some((metadata.width, metadata.height));
                let frame = metadata
                    .frames
                    .get(entry.frame)
                    .ok_or_else(|| Error::invalid("placement frame is missing"))?;
                frame.anchors.validate()?;
                anchor = anchor.or(frame.anchors.ground);
                if let Some(id) = &entry.part {
                    let part = metadata
                        .parts
                        .iter()
                        .find(|part| &part.id == id)
                        .ok_or_else(|| Error::invalid("unknown scene part"))?;
                    part_order = part.order;
                    let file = part
                        .files
                        .get(entry.frame)
                        .ok_or_else(|| Error::invalid("part frame is missing"))?;
                    let local = AssetRoot::new(
                        path.parent()
                            .ok_or_else(|| Error::invalid("placement directory missing"))?,
                    )?;
                    input = local.resolve(std::path::Path::new(file))?;
                }
            } else if entry.part.is_some() {
                return Err(Error::invalid("scene parts require placement metadata"));
            }
            // Exported PNGs select their image through the path; the metadata
            // frame index selects anchors and part files rather than PNG frames.
            let flat_export = entry.placement.is_some()
                && input
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("png"));
            let mut canvas = budget.image(&input, if flat_export { 0 } else { entry.frame })?;
            if dimensions.is_some_and(|size| size != (canvas.width(), canvas.height())) {
                return Err(Error::invalid(
                    "placement dimensions disagree with scene image",
                ));
            }
            let anchor = anchor.unwrap_or(Point {
                x: canvas.width() as i32 / 2,
                y: canvas.height() as i32 - 1,
            });
            anchor.validate()?;
            let width = canvas
                .width()
                .checked_mul(entry.scale)
                .ok_or_else(|| Error::limit("scaled asset width overflow"))?;
            let height = canvas
                .height()
                .checked_mul(entry.scale)
                .ok_or_else(|| Error::limit("scaled asset height overflow"))?;
            pixels = pixels
                .checked_add(u64::from(width) * u64::from(height))
                .ok_or_else(|| Error::limit("scene pixel overflow"))?;
            if pixels > crate::canvas::MAX_PIXELS {
                return Err(Error::limit("scene exceeds aggregate asset pixel limit"));
            }
            if entry.scale != 1 {
                canvas = drawing::resize(&canvas, width, height)?;
            }
            let anchor = Point {
                x: anchor
                    .x
                    .checked_mul(entry.scale as i32)
                    .ok_or_else(|| Error::limit("anchor overflow"))?,
                y: anchor
                    .y
                    .checked_mul(entry.scale as i32)
                    .ok_or_else(|| Error::limit("anchor overflow"))?,
            };
            loaded.push(Loaded {
                canvas,
                anchor,
                position: entry.position,
                order: entry
                    .order
                    .checked_add(part_order)
                    .ok_or_else(|| Error::limit("scene order overflow"))?,
                index,
            });
        }
        Ok(loaded)
    }
    pub fn render(&self, root: &AssetRoot, scale: u32) -> Result<Canvas> {
        let mut canvas = Canvas::new(self.width, self.height)?;
        let mut budget = Budget::default();
        let mut assets = self.load(root, &mut budget)?;
        canvas.pixels_mut().fill(self.background);
        if let Some(tile) = &self.tile {
            let tile = budget.image(&root.resolve(tile)?, 0)?;
            for y in 0..canvas.height() {
                for x in 0..canvas.width() {
                    let source = tile.pixel(x % tile.width(), y % tile.height()).unwrap();
                    canvas.put(i64::from(x), i64::from(y), source, true);
                }
            }
        }
        assets.sort_by_key(|asset| {
            (
                asset.order,
                if self.y_sort { asset.position.y } else { 0 },
                asset.index,
            )
        });
        for asset in assets {
            let x = asset
                .position
                .x
                .checked_sub(asset.anchor.x)
                .ok_or_else(|| Error::limit("scene x overflow"))?;
            let y = asset
                .position
                .y
                .checked_sub(asset.anchor.y)
                .ok_or_else(|| Error::limit("scene y overflow"))?;
            drawing::paste(&mut canvas, &asset.canvas, x, y, true)?;
        }
        scaled(&canvas, scale)
    }
    pub fn contact_sheet(
        &self,
        root: &AssetRoot,
        columns: Option<u32>,
        padding: u32,
        scale: u32,
    ) -> Result<Canvas> {
        let assets = self.load(root, &mut Budget::default())?;
        contact_sheet(
            &assets.into_iter().map(|a| a.canvas).collect::<Vec<_>>(),
            columns,
            padding,
            self.background,
            scale,
        )
    }
}
pub fn scaled(canvas: &Canvas, scale: u32) -> Result<Canvas> {
    if scale == 0 || scale > 64 {
        return Err(Error::invalid("preview scale must be 1..=64"));
    }
    if scale == 1 {
        return Ok(canvas.clone());
    }
    drawing::resize(
        canvas,
        canvas
            .width()
            .checked_mul(scale)
            .ok_or_else(|| Error::limit("preview width overflow"))?,
        canvas
            .height()
            .checked_mul(scale)
            .ok_or_else(|| Error::limit("preview height overflow"))?,
    )
}
pub fn contact_sheet(
    images: &[Canvas],
    columns: Option<u32>,
    padding: u32,
    background: Pixel,
    scale: u32,
) -> Result<Canvas> {
    if images.is_empty() || images.len() > 256 || padding > 1024 {
        return Err(Error::invalid(
            "contact sheets require 1..=256 images and padding <=1024",
        ));
    }
    let columns =
        columns.unwrap_or_else(|| (1..=16).find(|n| n * n >= images.len() as u32).unwrap());
    if columns == 0 || columns > images.len() as u32 {
        return Err(Error::invalid("invalid contact sheet columns"));
    }
    let width = images.iter().map(Canvas::width).max().unwrap();
    let height = images.iter().map(Canvas::height).max().unwrap();
    let rows = (images.len() as u32).div_ceil(columns);
    let extent = |count: u32, size: u32| {
        u32::try_from(
            u64::from(count) * (u64::from(size) + u64::from(padding)) + u64::from(padding),
        )
        .map_err(|_| Error::limit("contact sheet dimensions overflow"))
    };
    let mut output = Canvas::new(extent(columns, width)?, extent(rows, height)?)?;
    output.pixels_mut().fill(background);
    for (index, image) in images.iter().enumerate() {
        let x = padding + (index as u32 % columns) * (width + padding);
        let y = padding + (index as u32 / columns) * (height + padding);
        for row in 0..image.height() {
            for column in 0..image.width() {
                output.put(
                    i64::from(x + column),
                    i64::from(y + row),
                    image.pixel(column, row).unwrap(),
                    true,
                );
            }
        }
    }
    scaled(&output, scale)
}
