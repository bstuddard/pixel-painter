// SPDX-License-Identifier: GPL-3.0-only
//! Named colors and deterministic palette constraints.

use crate::{Canvas, Error, ErrorCode, Pixel, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    pub version: u32,
    #[serde(deserialize_with = "crate::limits::map::<'de, _, _, 256>")]
    pub colors: BTreeMap<String, Pixel>,
}

impl Palette {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "palette version must be 1",
            ));
        }
        if self.colors.is_empty() || self.colors.len() > 256 {
            return Err(Error::invalid("palettes require 1..=256 named colors"));
        }
        if self
            .colors
            .keys()
            .any(|name| name.is_empty() || name.len() > 128 || name.starts_with('#'))
        {
            return Err(Error::invalid(
                "palette names must contain 1..=128 bytes and cannot start with #",
            ));
        }
        Ok(())
    }

    /// Transparent pixels retain hidden RGB; nearest mapping uses squared RGBA distance.
    /// Equidistant colors resolve by lexicographically sorted color name.
    /// Work is bounded, repeated colors are cached, and errors preserve the input.
    pub fn constrain(&self, canvas: &mut Canvas, policy: Policy) -> Result<()> {
        let mut remaining = crate::drawing::MAX_WORK;
        if matches!(policy, Policy::Nearest) {
            let mut edited = canvas.clone();
            self.constrain_budgeted(&mut edited, policy, &mut remaining)?;
            *canvas = edited;
            Ok(())
        } else {
            self.constrain_budgeted(canvas, policy, &mut remaining)
        }
    }

    /// The caller owns the transaction: a budget failure can leave this working
    /// copy partly mapped. A bounded lookup cache never affects candidate order.
    pub(crate) fn constrain_budgeted(
        &self,
        canvas: &mut Canvas,
        policy: Policy,
        remaining: &mut u64,
    ) -> Result<()> {
        self.validate()?;
        if matches!(policy, Policy::Unrestricted) {
            return Ok(());
        }
        let charge = |remaining: &mut u64, cost: u64| -> Result<()> {
            *remaining = remaining
                .checked_sub(cost)
                .ok_or_else(|| Error::limit("palette exceeds remaining work budget"))?;
            Ok(())
        };
        charge(remaining, canvas.pixels().len() as u64)?;
        let candidates: Vec<_> = self.colors.values().copied().collect();
        let mut cache = HashMap::<Pixel, Pixel>::new();
        for pixel in canvas.pixels_mut().iter_mut().filter(|p| p.a != 0) {
            if let Some(mapped) = cache.get(pixel) {
                *pixel = *mapped;
                continue;
            }
            charge(remaining, candidates.len() as u64)?;
            let mapped = if matches!(policy, Policy::Strict) {
                if !candidates.contains(pixel) {
                    return Err(Error::new(
                        ErrorCode::PaletteViolation,
                        format!("color {pixel} is outside the palette"),
                    ));
                }
                *pixel
            } else {
                *candidates
                    .iter()
                    .min_by_key(|color| {
                        [
                            (pixel.r, color.r),
                            (pixel.g, color.g),
                            (pixel.b, color.b),
                            (pixel.a, color.a),
                        ]
                        .iter()
                        .map(|(a, b)| (i32::from(*a) - i32::from(*b)).pow(2) as u32)
                        .sum::<u32>()
                    })
                    .expect("validated nonempty palette")
            };
            // A high-color input must not turn a CPU safeguard into an unbounded
            // allocation. Beyond this limit, charge and recompute cache misses.
            if cache.len() < 65_536 {
                cache.insert(*pixel, mapped);
            }
            *pixel = mapped;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    #[default]
    Unrestricted,
    Strict,
    Nearest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Color {
    Rgba(Pixel),
    Named(String),
}

impl Color {
    pub fn resolve(&self, palette: Option<&Palette>) -> Result<Pixel> {
        match self {
            Self::Rgba(pixel) => Ok(*pixel),
            Self::Named(name) if name.starts_with('#') => name.parse(),
            Self::Named(name) => palette
                .and_then(|p| p.colors.get(name))
                .copied()
                .ok_or_else(|| Error::invalid(format!("unknown palette color {name:?}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_palette_step_uses_the_remaining_shared_budget() {
        let palette = Palette {
            version: 1,
            colors: BTreeMap::from([("ink".into(), Pixel::rgba(0, 0, 0, 255))]),
        };
        let mut canvas =
            Canvas::from_pixels(2, 1, vec![Pixel::rgba(255, 255, 255, 255); 2]).unwrap();
        let mut remaining = 2;
        assert_eq!(
            palette
                .constrain_budgeted(&mut canvas, Policy::Nearest, &mut remaining)
                .unwrap_err()
                .code
                .as_str(),
            "resource_limit"
        );
    }
}
