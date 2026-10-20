// SPDX-License-Identifier: GPL-3.0-only
//! Measurable shared style constraints; visual conventions remain human guidance.

use crate::{
    Canvas, Error, ErrorCode, Pixel, Result, drawing, palette::Palette, validation::Report,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Range {
    pub min: u32,
    pub max: u32,
}
impl Range {
    fn validate(self) -> Result<()> {
        if self.min == 0 || self.min > self.max || u64::from(self.max) > crate::canvas::MAX_PIXELS {
            return Err(Error::invalid(
                "size ranges require 1 <= min <= max <= MAX_PIXELS",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Perspective {
    #[default]
    ThreeQuarter,
    TopDown,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Light {
    #[default]
    Northwest,
    Northeast,
    Southwest,
    Southeast,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlphaPolicy {
    #[default]
    Any,
    Binary,
    Opaque,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationRules {
    #[serde(default, deserialize_with = "crate::limits::sequence::<'de, _, _, 32>")]
    pub required_tags: Vec<String>,
    #[serde(default)]
    pub baseline: Option<i32>,
    #[serde(default)]
    pub margin: u32,
    #[serde(default)]
    pub max_size_delta: Option<u32>,
    #[serde(
        default,
        deserialize_with = "crate::limits::sequence::<'de, _, _, 256>"
    )]
    pub movement_exceptions: Vec<String>,
    #[serde(
        default,
        deserialize_with = "crate::limits::sequence::<'de, _, _, 256>"
    )]
    pub clipping_exceptions: Vec<String>,
}
fn max_colors() -> u16 {
    256
}
fn grid() -> u32 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u32,
    pub name: String,
    pub palette: Palette,
    #[serde(deserialize_with = "crate::limits::map::<'de, _, _, 32>")]
    pub roles: BTreeMap<String, String>,
    #[serde(default)]
    pub perspective: Perspective,
    #[serde(default)]
    pub light: Light,
    #[serde(default)]
    pub width: Option<Range>,
    #[serde(default)]
    pub height: Option<Range>,
    #[serde(default = "grid")]
    pub grid: u32,
    #[serde(default = "max_colors")]
    pub max_colors: u16,
    #[serde(default)]
    pub alpha: AlphaPolicy,
    /// Zero disables outline checking; 1..=4 checks opaque boundary rings.
    #[serde(default)]
    pub outline_thickness: u8,
    #[serde(default)]
    pub animation: AnimationRules,
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "style version must be 1",
            ));
        }
        crate::document::name_valid(&self.name)?;
        self.palette.validate()?;
        if self.roles.len() > 32
            || self.max_colors == 0
            || self.max_colors > 256
            || self.outline_thickness > 4
            || self.grid == 0
            || self.grid > 1024
        {
            return Err(Error::invalid(
                "invalid style role, color, outline, or grid limits",
            ));
        }
        for role in ["outline", "top", "front", "side", "highlight", "shadow"] {
            self.role(role)?;
        }
        for (role, color) in &self.roles {
            crate::document::name_valid(role)?;
            crate::document::name_valid(color)?;
            self.role(role)?;
        }
        for name in self.palette.colors.keys() {
            crate::document::name_valid(name)?;
        }
        if let Some(range) = self.width {
            range.validate()?;
        }
        if let Some(range) = self.height {
            range.validate()?;
        }
        if self.animation.margin > 1024
            || self.animation.required_tags.len() > 32
            || self.animation.movement_exceptions.len() > 256
            || self.animation.clipping_exceptions.len() > 256
        {
            return Err(Error::invalid("animation style limits exceeded"));
        }
        if let Some(y) = self.animation.baseline {
            drawing::Point {
                x: 0,
                y,
            }
            .validate()?;
        }
        for names in [
            &self.animation.required_tags,
            &self.animation.movement_exceptions,
            &self.animation.clipping_exceptions,
        ] {
            let mut unique = BTreeSet::new();
            for name in names {
                crate::document::name_valid(name)?;
                if !unique.insert(name) {
                    return Err(Error::invalid("duplicate animation rule name"));
                }
            }
        }
        Ok(())
    }
    pub fn role(&self, role: &str) -> Result<Pixel> {
        self.roles
            .get(role)
            .and_then(|name| self.palette.colors.get(name))
            .copied()
            .ok_or_else(|| {
                Error::invalid(format!(
                    "style role {role:?} requires a named palette color"
                ))
            })
    }
    /// Check every flattened frame using one aggregate scan budget.
    pub fn check_document(&self, document: &crate::Document) -> Result<Report> {
        self.validate()?;
        self.check_work(
            u64::from(document.width())
                * u64::from(document.height())
                * document.frames().len() as u64,
        )?;
        let mut report = Report::default();
        for (index, frame) in document.frames().iter().enumerate() {
            report.extend(self.check(&document.flatten(index)?, &frame.name)?);
        }
        Ok(report)
    }
    pub fn check(&self, canvas: &Canvas, asset: &str) -> Result<Report> {
        self.validate()?;
        let mut report = Report {
            checked: 1,
            ..Report::default()
        };
        for (name, dimension, range) in [
            ("width", canvas.width(), self.width),
            ("height", canvas.height(), self.height),
        ] {
            if range.is_some_and(|range| dimension < range.min || dimension > range.max) {
                report.error(
                    "dimensions",
                    asset,
                    format!("{name} {dimension} is outside the profile range"),
                );
            }
            if dimension % self.grid != 0 {
                report.error(
                    "grid",
                    asset,
                    format!("{name} must be a multiple of {}", self.grid),
                );
            }
        }
        self.check_work(canvas.pixels().len() as u64)?;
        let allowed: BTreeSet<_> = self.palette.colors.values().copied().collect();
        // Retain at most 257 colors, even for a large true-color input.
        let mut colors = BTreeSet::new();
        let (mut off_palette, mut partial, mut transparent) = (0usize, 0usize, 0usize);
        for pixel in canvas.pixels() {
            if pixel.a == 0 {
                transparent += 1;
            } else {
                if colors.len() <= usize::from(self.max_colors) {
                    colors.insert(*pixel);
                }
                off_palette += usize::from(!allowed.contains(pixel));
                partial += usize::from(pixel.a != 255);
            }
        }
        if colors.len() > usize::from(self.max_colors) {
            report.error(
                "color_count",
                asset,
                format!("more than {} visible colors", self.max_colors),
            );
        }
        if off_palette != 0 {
            report.error(
                "palette",
                asset,
                format!("{off_palette} visible pixels are outside the palette"),
            );
        }
        if matches!(self.alpha, AlphaPolicy::Binary | AlphaPolicy::Opaque) && partial != 0 {
            report.error("partial_alpha", asset, "partial alpha is disallowed");
        }
        if matches!(self.alpha, AlphaPolicy::Opaque) && transparent != 0 {
            report.error("transparency", asset, "transparent pixels are disallowed");
        }
        if transparent == canvas.pixels().len() {
            report.warning("empty_image", asset, "image has no visible pixels");
        }
        if self.outline_thickness != 0 {
            let outline = self.role("outline")?;
            let radius = i64::from(self.outline_thickness);
            let mut wrong = 0;
            for (index, pixel) in canvas
                .pixels()
                .iter()
                .enumerate()
                .filter(|(_, p)| p.a == 255)
            {
                let x = (index % canvas.width() as usize) as i64;
                let y = (index / canvas.width() as usize) as i64;
                let boundary = (1..=radius).any(|d| {
                    [(x - d, y), (x + d, y), (x, y - d), (x, y + d)]
                        .into_iter()
                        .any(|(x, y)| {
                            x < 0
                                || y < 0
                                || canvas.pixel(x as u32, y as u32).is_none_or(|p| p.a != 255)
                        })
                });
                if boundary && *pixel != outline {
                    wrong += 1;
                }
            }
            if wrong != 0 {
                report.error(
                    "outline",
                    asset,
                    format!("{wrong} opaque boundary pixels differ from the outline role"),
                );
            }
        }
        Ok(report)
    }
    /// Conservative shared scan budget, including animation bounds and outline neighbors.
    pub(crate) fn check_work(&self, pixels: u64) -> Result<()> {
        if pixels.saturating_mul(2 + 4 * u64::from(self.outline_thickness)) > drawing::MAX_WORK {
            return Err(Error::limit("style validation exceeds work budget"));
        }
        Ok(())
    }
    /// Role swatches and an integer light-direction marker, without subjective scoring.
    pub fn reference(&self) -> Result<Canvas> {
        self.validate()?;
        let mut canvas = Canvas::new(128, 64)?;
        for (index, role) in ["outline", "top", "front", "side", "highlight", "shadow"]
            .iter()
            .enumerate()
        {
            drawing::rectangle(
                &mut canvas,
                drawing::Rect {
                    x: 4 + index as i32 * 20,
                    y: 4,
                    width: 16,
                    height: 24,
                },
                self.role(role)?,
                true,
                false,
            )?;
        }
        let (x, y) = match self.light {
            Light::Northwest => (44, 36),
            Light::Northeast => (84, 36),
            Light::Southwest => (44, 60),
            Light::Southeast => (84, 60),
        };
        drawing::line(
            &mut canvas,
            drawing::Point {
                x,
                y,
            },
            drawing::Point {
                x: 64,
                y: 48,
            },
            self.role("highlight")?,
            false,
        )?;
        Ok(canvas)
    }
}
