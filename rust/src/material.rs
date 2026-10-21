// SPDX-License-Identifier: GPL-3.0-only
//! Explicit dark-to-light material ramps. Shading never crosses material boundaries.

use crate::{
    Canvas, Error, Pixel, Result,
    palette::{Color, Palette},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A named material with unique, opaque colors ordered from shadow to highlight.
/// The same RGB cannot belong to two ramps in one shading operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ramp {
    pub name: String,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 256>")]
    pub colors: Vec<Color>,
}

pub(crate) fn shade(
    canvas: &mut Canvas,
    ramps: &[Ramp],
    steps: i32,
    palette: Option<&Palette>,
) -> Result<()> {
    if ramps.is_empty() || ramps.len() > 64 {
        return Err(Error::invalid("shading requires 1..=64 material ramps"));
    }
    let mut names = BTreeSet::new();
    let mut replacements = BTreeMap::new();
    for ramp in ramps {
        crate::document::name_valid(&ramp.name)?;
        if !names.insert(&ramp.name) || !(2..=256).contains(&ramp.colors.len()) {
            return Err(Error::invalid(
                "material names must be unique and ramps require 2..=256 colors",
            ));
        }
        let colors: Vec<_> = ramp
            .colors
            .iter()
            .map(|color| color.resolve(palette))
            .collect::<Result<_>>()?;
        for (index, color) in colors.iter().enumerate() {
            if color.a != 255 {
                return Err(Error::invalid(
                    "material ramp colors must be opaque; shading preserves pixel alpha",
                ));
            }
            let target =
                (index as i64 + i64::from(steps)).clamp(0, colors.len() as i64 - 1) as usize;
            if replacements
                .insert((color.r, color.g, color.b), colors[target])
                .is_some()
            {
                return Err(Error::invalid(
                    "material ramp RGB colors must be unique across all ramps",
                ));
            }
        }
    }
    for pixel in canvas.pixels_mut().iter_mut().filter(|pixel| pixel.a != 0) {
        if let Some(replacement) = replacements.get(&(pixel.r, pixel.g, pixel.b)) {
            *pixel = Pixel {
                a: pixel.a,
                ..*replacement
            };
        }
    }
    Ok(())
}
