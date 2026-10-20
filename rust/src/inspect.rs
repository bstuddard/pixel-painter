// SPDX-License-Identifier: GPL-3.0-only
//! Stable image metrics without terminal-output parsing.

use crate::{Canvas, drawing::Rect, palette::Palette};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
pub struct Report {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub pixels: usize,
    pub transparent_pixels: usize,
    pub partial_alpha_pixels: usize,
    pub opaque_pixels: usize,
    pub transparency_coverage: f64,
    pub occupied_bounds: Option<Rect>,
    pub unique_colors: usize,
    pub colors: BTreeMap<String, usize>,
    pub palette_usage: BTreeMap<String, usize>,
    pub off_palette_pixels: usize,
    pub warnings: Vec<String>,
}

pub fn inspect(canvas: &Canvas, palette: Option<&Palette>) -> Report {
    let mut colors = BTreeMap::new();
    let mut transparent = 0;
    let mut partial = 0;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (canvas.width(), canvas.height(), 0, 0);
    for (index, pixel) in canvas.pixels().iter().enumerate() {
        *colors.entry(*pixel).or_insert(0) += 1;
        if pixel.a == 0 {
            transparent += 1;
        } else {
            if pixel.a != 255 {
                partial += 1;
            }
            let x = index as u32 % canvas.width();
            let y = index as u32 / canvas.width();
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    let pixels = canvas.pixels().len();
    let mut palette_usage = BTreeMap::new();
    let mut off_palette_pixels = 0;
    if let Some(palette) = palette {
        for (name, color) in &palette.colors {
            palette_usage.insert(name.clone(), *colors.get(color).unwrap_or(&0));
        }
        // Count each distinct RGBA value once. Repeated pixels and palette
        // aliases must not multiply the cost of checking palette membership.
        let allowed: BTreeSet<_> = palette.colors.values().collect();
        off_palette_pixels = colors
            .iter()
            .filter(|(pixel, _)| pixel.a != 0 && !allowed.contains(pixel))
            .map(|(_, count)| *count)
            .sum();
    }
    let mut warnings = Vec::new();
    if transparent == pixels {
        warnings.push("empty_image".into());
    }
    if off_palette_pixels != 0 {
        warnings.push("off_palette_colors".into());
    }
    Report {
        version: 1,
        width: canvas.width(),
        height: canvas.height(),
        pixels,
        transparent_pixels: transparent,
        partial_alpha_pixels: partial,
        opaque_pixels: pixels - transparent - partial,
        transparency_coverage: transparent as f64 / pixels as f64,
        occupied_bounds: if transparent == pixels {
            None
        } else {
            Some(Rect {
                x: min_x as i32,
                y: min_y as i32,
                width: max_x - min_x + 1,
                height: max_y - min_y + 1,
            })
        },
        unique_colors: colors.len(),
        colors: colors
            .into_iter()
            .map(|(pixel, count)| (pixel.to_string(), count))
            .collect(),
        palette_usage,
        off_palette_pixels,
        warnings,
    }
}

/// Visible bounds without retaining a color histogram.
pub fn occupied_bounds(canvas: &Canvas) -> Option<Rect> {
    let (mut left, mut top, mut right, mut bottom) = (canvas.width(), canvas.height(), 0, 0);
    for (index, _) in canvas.pixels().iter().enumerate().filter(|(_, p)| p.a != 0) {
        let (x, y) = (index as u32 % canvas.width(), index as u32 / canvas.width());
        left = left.min(x);
        top = top.min(y);
        right = right.max(x);
        bottom = bottom.max(y);
    }
    (left < canvas.width()).then(|| Rect {
        x: left as i32,
        y: top as i32,
        width: right - left + 1,
        height: bottom - top + 1,
    })
}
