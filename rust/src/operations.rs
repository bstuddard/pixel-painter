// SPDX-License-Identifier: GPL-3.0-only
//! Versioned typed edits with rollback, bounded work, and local named clipboards.

use crate::{
    Canvas, Error, ErrorCode, Pixel, Result,
    drawing::{self, Axis, Point, Rect},
    maps::{self, Channel},
    mask::{Mask, Selection},
    material::{self, Ramp},
    palette::{Color, Palette, Policy},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub version: u32,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub palette: Option<Palette>,
    #[serde(default)]
    pub policy: Policy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<Mask>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub alpha_lock: bool,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 10_000>")]
    pub operations: Vec<Operation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Move visible colors within explicit material ramps, preserving alpha.
    Shade {
        #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 64>")]
        ramps: Vec<Ramp>,
        steps: i32,
    },
    SetPixel {
        x: i32,
        y: i32,
        color: Color,
        #[serde(default)]
        over: bool,
    },
    Line {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: Color,
        #[serde(default)]
        over: bool,
    },
    Rect {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        color: Color,
        #[serde(default)]
        fill: bool,
        #[serde(default)]
        over: bool,
    },
    FillRect {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        color: Color,
        #[serde(default)]
        over: bool,
    },
    Ellipse {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        color: Color,
        #[serde(default)]
        fill: bool,
        #[serde(default)]
        over: bool,
    },
    Polygon {
        #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 256>")]
        points: Vec<Point>,
        color: Color,
        #[serde(default)]
        fill: bool,
        #[serde(default)]
        over: bool,
    },
    Erase {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
    Clear {
        #[serde(default)]
        color: Option<Color>,
    },
    Fill {
        x: i32,
        y: i32,
        color: Color,
    },
    Copy {
        name: String,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
    Paste {
        name: String,
        x: i32,
        y: i32,
        #[serde(default)]
        over: bool,
    },
    Stamp {
        canvas: Canvas,
        x: i32,
        y: i32,
        #[serde(default)]
        over: bool,
    },
    Flip {
        axis: Axis,
    },
    Rotate {
        turns: i32,
    },
    Move {
        dx: i32,
        dy: i32,
    },
    Crop {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
    Resize {
        width: u32,
        height: u32,
    },
    Noise {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 256>")]
        colors: Vec<Color>,
        #[serde(default = "full_density")]
        density: u8,
    },
}
fn full_density() -> u8 {
    255
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub version: u32,
    pub operations: usize,
    pub changed_pixels: usize,
    pub width: u32,
    pub height: u32,
}

impl Batch {
    pub fn new(operations: Vec<Operation>) -> Self {
        Self {
            version: 1,
            seed: 0,
            palette: None,
            policy: Policy::Unrestricted,
            mask: None,
            alpha_lock: false,
            operations,
        }
    }

    /// Compute on a clone and commit once. A failed operation never changes input.
    pub fn apply(&self, canvas: &mut Canvas) -> Result<Summary> {
        self.apply_with_seed(canvas, self.seed)
    }

    /// Override the procedural stream without copying embedded canvas components.
    pub fn apply_with_seed(&self, canvas: &mut Canvas, seed: u64) -> Result<Summary> {
        let mut remaining = drawing::MAX_WORK;
        self.apply_budgeted(canvas, seed, &mut remaining)
    }

    /// Share rasterization work across recipes while preserving canvas rollback.
    pub(crate) fn apply_budgeted(
        &self,
        canvas: &mut Canvas,
        seed: u64,
        remaining: &mut u64,
    ) -> Result<Summary> {
        let (edited, summary) = self.render_budgeted(canvas, seed, remaining)?;
        *canvas = edited;
        Ok(summary)
    }

    /// Compute an independent result without changing the source. Useful for
    /// document-level invariants that must be checked before committing a cel.
    pub fn render(&self, canvas: &Canvas) -> Result<(Canvas, Summary)> {
        let mut remaining = drawing::MAX_WORK;
        self.render_budgeted(canvas, self.seed, &mut remaining)
    }

    pub(crate) fn render_channel(
        &self,
        canvas: &Canvas,
        channel: Channel,
        remaining: &mut u64,
    ) -> Result<(Canvas, Summary)> {
        self.render_budgeted_channel(canvas, self.seed, remaining, channel)
    }

    fn render_budgeted(
        &self,
        canvas: &Canvas,
        seed: u64,
        remaining: &mut u64,
    ) -> Result<(Canvas, Summary)> {
        self.render_budgeted_channel(canvas, seed, remaining, Channel::Color)
    }
    fn render_budgeted_channel(
        &self,
        canvas: &Canvas,
        seed: u64,
        remaining: &mut u64,
        channel: Channel,
    ) -> Result<(Canvas, Summary)> {
        if self.version != 1 {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "batch version must be 1",
            ));
        }
        if self.operations.len() > 10_000 {
            return Err(Error::limit("batch exceeds 10,000 operations"));
        }
        let embedded: u64 = self
            .operations
            .iter()
            .filter_map(|op| match op {
                Operation::Stamp {
                    canvas,
                    ..
                } => Some(canvas.pixels().len() as u64),
                _ => None,
            })
            .sum::<u64>()
            + match &self.mask {
                Some(Mask::Canvas {
                    canvas,
                }) => canvas.pixels().len() as u64,
                _ => 0,
            };
        if embedded > crate::canvas::MAX_PIXELS {
            return Err(Error::limit(
                "embedded components exceed aggregate pixel limit",
            ));
        }
        let selection = Selection::capture(canvas, self.mask.as_ref(), self.alpha_lock)?;
        if selection.is_some() {
            *remaining = remaining
                .checked_sub(canvas.pixels().len() as u64)
                .ok_or_else(|| Error::limit("mask capture exceeds rasterization budget"))?;
        }
        if let Some(palette) = &self.palette {
            palette.validate()?;
        }
        if self.palette.is_none() && !matches!(self.policy, Policy::Unrestricted) {
            return Err(Error::invalid("palette policy requires a palette"));
        }
        if channel != Channel::Color && !matches!(self.policy, Policy::Unrestricted) {
            return Err(Error::invalid(
                "companion map layers require unrestricted palette policy",
            ));
        }
        let mut edited = canvas.clone();
        let mut clipboard = BTreeMap::<String, Canvas>::new();
        let mut random = Random(seed);
        for (index, operation) in self.operations.iter().enumerate() {
            if channel != Channel::Color && matches!(operation, Operation::Shade { .. }) {
                return Err(Error::invalid("material shading requires a color layer").at(index));
            }
            if selection.is_some() && operation.transforms_canvas() {
                return Err(Error::invalid(
                    "masked batches cannot transform canvas coordinates or dimensions",
                )
                .at(index));
            }
            if channel == Channel::Normal && operation.blends_paint() {
                return Err(Error::invalid(
                    "normal edits replace vectors; alpha paint blending is not supported",
                )
                .at(index));
            }
            let transform = channel == Channel::Normal
                && matches!(operation, Operation::Flip { .. } | Operation::Rotate { .. });
            let cost = operation.cost(&edited)
                + if self.alpha_lock {
                    edited.pixels().len() as u64
                } else {
                    0
                }
                + if selection.is_some() {
                    edited.pixels().len() as u64
                } else {
                    0
                }
                + if transform {
                    edited.pixels().len() as u64
                } else {
                    0
                };
            *remaining = remaining
                .checked_sub(cost)
                .ok_or_else(|| Error::limit("batch exceeds rasterization budget").at(index))?;
            let previous = self.alpha_lock.then(|| edited.clone());
            if let (
                Some(selection),
                Operation::Fill {
                    x,
                    y,
                    color,
                },
            ) = (&selection, operation)
            {
                selection
                    .fill(
                        &mut edited,
                        Point {
                            x: *x,
                            y: *y,
                        },
                        color
                            .resolve(self.palette.as_ref())
                            .map_err(|error| error.at(index))?,
                    )
                    .map_err(|error| error.at(index))?;
            } else {
                operation
                    .apply(
                        &mut edited,
                        &mut clipboard,
                        self.palette.as_ref(),
                        &mut random,
                    )
                    .map_err(|e| e.at(index))?;
            }
            if let Some(selection) = &selection {
                selection.constrain(&mut edited, canvas, previous.as_ref());
            }
            if transform {
                for pixel in edited.pixels_mut() {
                    match operation {
                        Operation::Flip {
                            axis,
                        } => maps::flip_normal(pixel, *axis),
                        Operation::Rotate {
                            turns,
                        } => maps::rotate_normal(pixel, *turns),
                        _ => unreachable!(),
                    }
                }
            }
        }
        if let Some(palette) = &self.palette {
            if self.alpha_lock {
                *remaining = remaining
                    .checked_sub(edited.pixels().len() as u64)
                    .ok_or_else(|| Error::limit("alpha lock exceeds rasterization budget"))?;
            }
            let previous = self.alpha_lock.then(|| edited.clone());
            palette.constrain_budgeted(&mut edited, self.policy, remaining)?;
            if let Some(selection) = &selection {
                *remaining = remaining
                    .checked_sub(edited.pixels().len() as u64)
                    .ok_or_else(|| Error::limit("mask validation exceeds rasterization budget"))?;
                selection.constrain(&mut edited, canvas, previous.as_ref());
            }
        }
        if channel == Channel::Normal {
            *remaining = remaining
                .checked_sub(edited.pixels().len() as u64)
                .ok_or_else(|| Error::limit("normal validation exceeds rasterization budget"))?;
            maps::validate_normals(&edited)?;
        }
        let changed_pixels =
            if (canvas.width(), canvas.height()) == (edited.width(), edited.height()) {
                canvas
                    .pixels()
                    .iter()
                    .zip(edited.pixels())
                    .filter(|(a, b)| a != b)
                    .count()
            } else {
                edited.pixels().len()
            };
        let summary = Summary {
            version: 1,
            operations: self.operations.len(),
            changed_pixels,
            width: edited.width(),
            height: edited.height(),
        };
        Ok((edited, summary))
    }
}

impl Operation {
    fn transforms_canvas(&self) -> bool {
        matches!(
            self,
            Self::Flip { .. }
                | Self::Rotate { .. }
                | Self::Move { .. }
                | Self::Crop { .. }
                | Self::Resize { .. }
        )
    }
    fn blends_paint(&self) -> bool {
        match self {
            Self::SetPixel {
                over,
                ..
            }
            | Self::Line {
                over,
                ..
            }
            | Self::Rect {
                over,
                ..
            }
            | Self::FillRect {
                over,
                ..
            }
            | Self::Ellipse {
                over,
                ..
            }
            | Self::Polygon {
                over,
                ..
            }
            | Self::Paste {
                over,
                ..
            }
            | Self::Stamp {
                over,
                ..
            } => *over,
            _ => false,
        }
    }
    fn cost(&self, canvas: &Canvas) -> u64 {
        let rect_cost = |x, y, width, height| {
            Rect {
                x,
                y,
                width,
                height,
            }
            .visible_area(canvas)
        };
        match self {
            Self::Shade {
                ramps,
                ..
            } => {
                canvas.pixels().len() as u64
                    + ramps
                        .iter()
                        .map(|ramp| ramp.colors.len() as u64)
                        .sum::<u64>()
            }
            Self::SetPixel {
                ..
            } => 1,
            Self::Line {
                x1,
                y1,
                x2,
                y2,
                ..
            } => {
                (i64::from(*x2) - i64::from(*x1))
                    .unsigned_abs()
                    .max((i64::from(*y2) - i64::from(*y1)).unsigned_abs())
                    + 1
            }
            Self::Rect {
                x,
                y,
                width,
                height,
                ..
            }
            | Self::FillRect {
                x,
                y,
                width,
                height,
                ..
            }
            | Self::Ellipse {
                x,
                y,
                width,
                height,
                ..
            }
            | Self::Erase {
                x,
                y,
                width,
                height,
            }
            | Self::Noise {
                x,
                y,
                width,
                height,
                ..
            } => rect_cost(*x, *y, *width, *height),
            Self::Copy {
                width,
                height,
                ..
            }
            | Self::Crop {
                width,
                height,
                ..
            }
            | Self::Resize {
                width,
                height,
            } => u64::from(*width) * u64::from(*height),
            Self::Polygon {
                points,
                ..
            } => drawing::polygon_work(canvas, points),
            _ => canvas.pixels().len() as u64,
        }
    }

    fn apply(
        &self,
        canvas: &mut Canvas,
        clipboard: &mut BTreeMap<String, Canvas>,
        palette: Option<&Palette>,
        random: &mut Random,
    ) -> Result<()> {
        let resolve = |color: &Color| color.resolve(palette);
        match self {
            Self::Shade {
                ramps,
                steps,
            } => material::shade(canvas, ramps, *steps, palette)?,
            Self::SetPixel {
                x,
                y,
                color,
                over,
            } => {
                if *x < 0 || *y < 0 || canvas.pixel(*x as u32, *y as u32).is_none() {
                    return Err(Error::new(
                        ErrorCode::OutOfBounds,
                        "set_pixel origin is outside canvas",
                    ));
                }
                canvas.put(i64::from(*x), i64::from(*y), resolve(color)?, *over);
            }
            Self::Line {
                x1,
                y1,
                x2,
                y2,
                color,
                over,
            } => drawing::line(
                canvas,
                Point {
                    x: *x1,
                    y: *y1,
                },
                Point {
                    x: *x2,
                    y: *y2,
                },
                resolve(color)?,
                *over,
            )?,
            Self::Rect {
                x,
                y,
                width,
                height,
                color,
                fill,
                over,
            } => drawing::rectangle(
                canvas,
                Rect {
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *height,
                },
                resolve(color)?,
                *fill,
                *over,
            )?,
            Self::FillRect {
                x,
                y,
                width,
                height,
                color,
                over,
            } => drawing::rectangle(
                canvas,
                Rect {
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *height,
                },
                resolve(color)?,
                true,
                *over,
            )?,
            Self::Ellipse {
                x,
                y,
                width,
                height,
                color,
                fill,
                over,
            } => drawing::ellipse(
                canvas,
                Rect {
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *height,
                },
                resolve(color)?,
                *fill,
                *over,
            )?,
            Self::Polygon {
                points,
                color,
                fill,
                over,
            } => drawing::polygon(canvas, points, resolve(color)?, *fill, *over)?,
            Self::Erase {
                x,
                y,
                width,
                height,
            } => drawing::rectangle(
                canvas,
                Rect {
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *height,
                },
                Pixel::TRANSPARENT,
                true,
                false,
            )?,
            Self::Clear {
                color,
            } => canvas
                .pixels_mut()
                .fill(color.as_ref().map(resolve).transpose()?.unwrap_or_default()),
            Self::Fill {
                x,
                y,
                color,
            } => drawing::flood_fill(
                canvas,
                Point {
                    x: *x,
                    y: *y,
                },
                resolve(color)?,
            )?,
            Self::Copy {
                name,
                x,
                y,
                width,
                height,
            } => {
                if name.is_empty() || name.len() > 128 {
                    return Err(Error::invalid("clipboard name must contain 1..=128 bytes"));
                }
                let retained: u64 = clipboard
                    .iter()
                    .filter(|(key, _)| *key != name)
                    .map(|(_, c)| c.pixels().len() as u64)
                    .sum();
                if retained + u64::from(*width) * u64::from(*height) > crate::canvas::MAX_PIXELS {
                    return Err(Error::limit("clipboard exceeds aggregate pixel limit"));
                }
                clipboard.insert(
                    name.clone(),
                    drawing::crop(
                        canvas,
                        Rect {
                            x: *x,
                            y: *y,
                            width: *width,
                            height: *height,
                        },
                    )?,
                );
            }
            Self::Paste {
                name,
                x,
                y,
                over,
            } => drawing::paste(
                canvas,
                clipboard
                    .get(name)
                    .ok_or_else(|| Error::invalid(format!("unknown clipboard {name:?}")))?,
                *x,
                *y,
                *over,
            )?,
            Self::Stamp {
                canvas: source,
                x,
                y,
                over,
            } => drawing::paste(canvas, source, *x, *y, *over)?,
            Self::Flip {
                axis,
            } => *canvas = drawing::flip(canvas, *axis)?,
            Self::Rotate {
                turns,
            } => *canvas = drawing::rotate(canvas, *turns)?,
            Self::Move {
                dx,
                dy,
            } => *canvas = drawing::translate(canvas, *dx, *dy)?,
            Self::Crop {
                x,
                y,
                width,
                height,
            } => {
                *canvas = drawing::crop(
                    canvas,
                    Rect {
                        x: *x,
                        y: *y,
                        width: *width,
                        height: *height,
                    },
                )?
            }
            Self::Resize {
                width,
                height,
            } => *canvas = drawing::resize(canvas, *width, *height)?,
            Self::Noise {
                x,
                y,
                width,
                height,
                colors,
                density,
            } => {
                let rect = Rect {
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *height,
                };
                rect.validate()?;
                if colors.is_empty() || colors.len() > 256 {
                    return Err(Error::invalid("noise requires 1..=256 colors"));
                }
                let colors: Vec<_> = colors.iter().map(resolve).collect::<Result<_>>()?;
                let (x0, y0, x1, y1) = rect.clipped(canvas);
                for y in y0..y1 {
                    for x in x0..x1 {
                        let sample = random.next();
                        if sample % 255 < u64::from(*density) {
                            canvas.put(
                                x,
                                y,
                                colors[(random.next() % colors.len() as u64) as usize],
                                false,
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Specified SplitMix64 stream; independent of platform and Rust's random libraries.
struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::Random;
    #[test]
    fn splitmix64_matches_published_reference_vector() {
        let mut random = Random(0);
        assert_eq!(random.next(), 0xE220A8397B1DCDAF);
        assert_eq!(random.next(), 0x6E789E6AA1B965F4);
        assert_eq!(random.next(), 0x06C45D188009454F);
    }

    #[test]
    fn shared_work_budget_keeps_canvas_rollback_without_resetting() {
        use super::{Batch, Operation};
        use crate::{Canvas, ErrorCode, Pixel, palette::Color};
        let red = Pixel::rgba(255, 0, 0, 255);
        let blue = Pixel::rgba(0, 0, 255, 255);
        let mut canvas = Canvas::new(2, 1).unwrap();
        let mut remaining = 3;
        Batch::new(vec![Operation::Clear {
            color: Some(Color::Rgba(red)),
        }])
        .apply_budgeted(&mut canvas, 0, &mut remaining)
        .unwrap();
        assert_eq!(remaining, 1);
        let error = Batch::new(vec![Operation::Clear {
            color: Some(Color::Rgba(blue)),
        }])
        .apply_budgeted(&mut canvas, 0, &mut remaining)
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(canvas.pixels(), &[red, red]);
    }

    #[test]
    fn mask_capture_constraining_and_alpha_snapshots_share_the_batch_budget() {
        use super::{Batch, Operation};
        use crate::{Canvas, ErrorCode, Pixel, mask::Mask, palette::Color};
        for alpha_lock in [false, true] {
            let mut canvas = Canvas::from_pixels(2, 1, vec![Pixel::rgba(1, 2, 3, 255); 2]).unwrap();
            let before = canvas.clone();
            let mut batch = Batch::new(vec![Operation::Clear {
                color: Some(Color::Rgba(Pixel::rgba(255, 255, 255, 255))),
            }]);
            batch.mask = Some(Mask::Alpha {});
            batch.alpha_lock = alpha_lock;
            let mut remaining = if alpha_lock { 7 } else { 5 };
            assert_eq!(
                batch
                    .apply_budgeted(&mut canvas, 0, &mut remaining)
                    .unwrap_err()
                    .code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(canvas, before);
            let mut enough = if alpha_lock { 8 } else { 6 };
            batch.apply_budgeted(&mut canvas, 0, &mut enough).unwrap();
            assert_eq!(enough, 0);
        }
    }
}
