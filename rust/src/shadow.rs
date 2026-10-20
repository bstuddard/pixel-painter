// SPDX-License-Identifier: GPL-3.0-only
//! Offline pixel-snapped silhouette projection; no blur or repeated alpha blending.

use crate::{Canvas, Error, Pixel, Result, drawing::Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub width: u32,
    pub height: u32,
    pub origin: Point,
    pub step: Point,
    pub steps: u32,
    pub squash_y: u32,
    pub color: Pixel,
}
pub fn project(source: &Canvas, options: Options) -> Result<Canvas> {
    options.origin.validate()?;
    options.step.validate()?;
    if options.steps > 64 || options.squash_y == 0 || options.squash_y > 64 {
        return Err(Error::invalid(
            "shadows require 0..=64 steps and squash_y 1..=64",
        ));
    }
    if source.pixels().len() as u64 * (u64::from(options.steps) + 1) > crate::drawing::MAX_WORK {
        return Err(Error::limit("shadow projection exceeds work budget"));
    }
    let mut output = Canvas::new(options.width, options.height)?;
    for (index, pixel) in source.pixels().iter().enumerate().filter(|(_, p)| p.a != 0) {
        let x = (index % source.width() as usize) as i64 + i64::from(options.origin.x);
        let y = (index / source.width() as usize) as i64 / i64::from(options.squash_y)
            + i64::from(options.origin.y);
        let alpha = ((u32::from(pixel.a) * u32::from(options.color.a) + 127) / 255) as u8;
        for step in 0..=options.steps {
            let dx = x + i64::from(step) * i64::from(options.step.x);
            let dy = y + i64::from(step) * i64::from(options.step.y);
            if dx >= 0
                && dy >= 0
                && dx < i64::from(output.width())
                && dy < i64::from(output.height())
            {
                let target =
                    &mut output.pixels_mut()[dy as usize * options.width as usize + dx as usize];
                if alpha > target.a {
                    *target = Pixel::rgba(options.color.r, options.color.g, options.color.b, alpha);
                }
            }
        }
    }
    Ok(output)
}
