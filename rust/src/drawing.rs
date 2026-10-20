// SPDX-License-Identifier: GPL-3.0-only
//! Integer rasterization and exact, nearest-neighbor image transformations.

use crate::{Canvas, Error, ErrorCode, Pixel, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const MAX_WORK: u64 = 67_108_864;
const MAX_COORDINATE: i64 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn validate(self) -> Result<()> {
        if i64::from(self.x).abs() > MAX_COORDINATE || i64::from(self.y).abs() > MAX_COORDINATE {
            return Err(Error::limit(
                "drawing coordinates must be within +/-1,000,000",
            ));
        }
        Ok(())
    }
}

/// Half-open rectangle [x, x+width) × [y, y+height), with signed origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn validate(self) -> Result<()> {
        Point {
            x: self.x,
            y: self.y,
        }
        .validate()?;
        if self.width == 0 || self.height == 0 {
            return Err(Error::invalid("rectangle dimensions must be positive"));
        }
        if self.width > 2_000_000 || self.height > 2_000_000 {
            return Err(Error::limit("drawing extents must not exceed 2,000,000"));
        }
        Ok(())
    }

    pub(crate) fn clipped(self, canvas: &Canvas) -> (i64, i64, i64, i64) {
        let x = i64::from(self.x);
        let y = i64::from(self.y);
        let bounds = (
            x.max(0),
            y.max(0),
            (x + i64::from(self.width)).min(i64::from(canvas.width())),
            (y + i64::from(self.height)).min(i64::from(canvas.height())),
        );
        // Empty intersections must have no rows either: nested raster loops
        // otherwise scan the full height even though their charged area is zero.
        if bounds.0 >= bounds.2 || bounds.1 >= bounds.3 {
            (0, 0, 0, 0)
        } else {
            bounds
        }
    }

    pub fn visible_area(self, canvas: &Canvas) -> u64 {
        let (x0, y0, x1, y1) = self.clipped(canvas);
        ((x1 - x0).max(0) * (y1 - y0).max(0)) as u64
    }
}

/// Inclusive Bresenham line. Endpoints are canonicalized for reversal symmetry.
pub fn line(canvas: &mut Canvas, start: Point, end: Point, color: Pixel, over: bool) -> Result<()> {
    start.validate()?;
    end.validate()?;
    let (start, end) = if (start.x, start.y) <= (end.x, end.y) {
        (start, end)
    } else {
        (end, start)
    };
    let (mut x, mut y) = (i64::from(start.x), i64::from(start.y));
    let (x1, y1) = (i64::from(end.x), i64::from(end.y));
    let dx = (x1 - x).abs();
    let dy = -(y1 - y).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let sy = if y < y1 { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        canvas.put(x, y, color, over);
        if x == x1 && y == y1 {
            break;
        }
        let twice = error * 2;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
    Ok(())
}

pub fn rectangle(
    canvas: &mut Canvas,
    rect: Rect,
    color: Pixel,
    fill: bool,
    over: bool,
) -> Result<()> {
    rect.validate()?;
    let (x0, y0, x1, y1) = rect.clipped(canvas);
    for y in y0..y1 {
        for x in x0..x1 {
            if fill
                || x == i64::from(rect.x)
                || y == i64::from(rect.y)
                || x == i64::from(rect.x) + i64::from(rect.width) - 1
                || y == i64::from(rect.y) + i64::from(rect.height) - 1
            {
                canvas.put(x, y, color, over);
            }
        }
    }
    Ok(())
}

/// Pixel-center ellipse; outlines contain inside pixels with an outside 4-neighbor.
pub fn ellipse(
    canvas: &mut Canvas,
    rect: Rect,
    color: Pixel,
    fill: bool,
    over: bool,
) -> Result<()> {
    rect.validate()?;
    let inside = |x: i64, y: i64| {
        let w = i128::from(rect.width);
        let h = i128::from(rect.height);
        let dx = 2 * (i128::from(x) - i128::from(rect.x)) + 1 - w;
        let dy = 2 * (i128::from(y) - i128::from(rect.y)) + 1 - h;
        dx * dx * h * h + dy * dy * w * w <= w * w * h * h
    };
    let (x0, y0, x1, y1) = rect.clipped(canvas);
    for y in y0..y1 {
        for x in x0..x1 {
            if inside(x, y)
                && (fill
                    || !inside(x - 1, y)
                    || !inside(x + 1, y)
                    || !inside(x, y - 1)
                    || !inside(x, y + 1))
            {
                canvas.put(x, y, color, over);
            }
        }
    }
    Ok(())
}

/// One estimate for both direct drawing and shared batch-budget accounting.
pub(crate) fn polygon_work(canvas: &Canvas, points: &[Point]) -> u64 {
    let edges = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| {
            (i64::from(a.x) - i64::from(b.x))
                .unsigned_abs()
                .max((i64::from(a.y) - i64::from(b.y)).unsigned_abs())
                + 1
        })
        .fold(0u64, u64::saturating_add);
    (canvas.pixels().len() as u64)
        .saturating_mul(points.len() as u64)
        .saturating_add(edges)
}

/// Even-odd fill at pixel centers plus inclusive Bresenham boundary pixels.
pub fn polygon(
    canvas: &mut Canvas,
    points: &[Point],
    color: Pixel,
    fill: bool,
    over: bool,
) -> Result<()> {
    if !(3..=256).contains(&points.len()) {
        return Err(Error::invalid("polygons require 3..=256 points"));
    }
    for point in points {
        point.validate()?;
    }
    if polygon_work(canvas, points) > MAX_WORK {
        return Err(Error::limit("polygon exceeds rasterization budget"));
    }
    // Render into a mask so shared vertices and fill/boundary overlap blend once.
    let mut mask = Canvas::new(canvas.width(), canvas.height())?;
    let mark = Pixel::rgba(0, 0, 0, 255);
    if fill {
        let min_x = points.iter().map(|p| p.x).min().unwrap().max(0) as i64;
        let min_y = points.iter().map(|p| p.y).min().unwrap().max(0) as i64;
        let max_x =
            i64::from(points.iter().map(|p| p.x).max().unwrap()).min(i64::from(canvas.width()) - 1);
        let max_y = i64::from(points.iter().map(|p| p.y).max().unwrap())
            .min(i64::from(canvas.height()) - 1);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let (px, py) = (i128::from(x) * 2 + 1, i128::from(y) * 2 + 1);
                let mut inside = false;
                for (a, b) in points
                    .iter()
                    .zip(points.iter().cycle().skip(1))
                    .take(points.len())
                {
                    let (ax, ay, bx, by) = (
                        i128::from(a.x) * 2,
                        i128::from(a.y) * 2,
                        i128::from(b.x) * 2,
                        i128::from(b.y) * 2,
                    );
                    if (ay > py) != (by > py) {
                        let lhs = (px - ax) * (by - ay);
                        let rhs = (bx - ax) * (py - ay);
                        if if by > ay { lhs < rhs } else { lhs > rhs } {
                            inside = !inside;
                        }
                    }
                }
                if inside {
                    mask.put(x, y, mark, false);
                }
            }
        }
    }
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        line(&mut mask, *a, *b, mark, false)?;
    }
    for (target, masked) in canvas.pixels_mut().iter_mut().zip(mask.pixels()) {
        if masked.a != 0 {
            *target = if over {
                color.over(*target, 255)
            } else {
                color
            };
        }
    }
    Ok(())
}

/// Exact RGBA equality and four-connected neighbors; no tolerance or diagonal fill.
pub fn flood_fill(canvas: &mut Canvas, start: Point, color: Pixel) -> Result<()> {
    start.validate()?;
    let (Ok(x), Ok(y)) = (u32::try_from(start.x), u32::try_from(start.y)) else {
        return Err(Error::new(
            ErrorCode::OutOfBounds,
            "fill origin is outside canvas",
        ));
    };
    let old = canvas
        .pixel(x, y)
        .ok_or_else(|| Error::new(ErrorCode::OutOfBounds, "fill origin is outside canvas"))?;
    if old == color {
        return Ok(());
    }
    let mut queue = VecDeque::from([(x, y)]);
    canvas.set_pixel(x, y, color)?;
    while let Some((x, y)) = queue.pop_front() {
        for (nx, ny) in [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ] {
            if canvas.pixel(nx, ny) == Some(old) {
                canvas.set_pixel(nx, ny, color)?;
                queue.push_back((nx, ny));
            }
        }
    }
    Ok(())
}

/// Extract exact dimensions; off-canvas samples become transparent.
pub fn crop(canvas: &Canvas, rect: Rect) -> Result<Canvas> {
    rect.validate()?;
    let mut result = Canvas::new(rect.width, rect.height)?;
    for y in 0..rect.height {
        for x in 0..rect.width {
            let (sx, sy) = (
                i64::from(rect.x) + i64::from(x),
                i64::from(rect.y) + i64::from(y),
            );
            if sx >= 0
                && sy >= 0
                && let Some(pixel) = canvas.pixel(sx as u32, sy as u32)
            {
                result.set_pixel(x, y, pixel)?;
            }
        }
    }
    Ok(result)
}

pub fn paste(canvas: &mut Canvas, source: &Canvas, x: i32, y: i32, over: bool) -> Result<()> {
    Point {
        x,
        y,
    }
    .validate()?;
    let rect = Rect {
        x,
        y,
        width: source.width(),
        height: source.height(),
    };
    let (x0, y0, x1, y1) = rect.clipped(canvas);
    for dy in y0..y1 {
        for dx in x0..x1 {
            let pixel = source
                .pixel((dx - i64::from(x)) as u32, (dy - i64::from(y)) as u32)
                .unwrap();
            canvas.put(dx, dy, pixel, over);
        }
    }
    Ok(())
}

pub fn resize(canvas: &Canvas, width: u32, height: u32) -> Result<Canvas> {
    let mut result = Canvas::new(width, height)?;
    for y in 0..height {
        for x in 0..width {
            let sx = (u64::from(x) * u64::from(canvas.width()) / u64::from(width)) as u32;
            let sy = (u64::from(y) * u64::from(canvas.height()) / u64::from(height)) as u32;
            result.set_pixel(x, y, canvas.pixel(sx, sy).unwrap())?;
        }
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Horizontal,
    Vertical,
}

pub fn flip(canvas: &Canvas, axis: Axis) -> Result<Canvas> {
    let mut result = Canvas::new(canvas.width(), canvas.height())?;
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let (sx, sy) = match axis {
                Axis::Horizontal => (canvas.width() - 1 - x, y),
                Axis::Vertical => (x, canvas.height() - 1 - y),
            };
            result.set_pixel(x, y, canvas.pixel(sx, sy).unwrap())?;
        }
    }
    Ok(result)
}

/// Clockwise quarter-turns, normalized modulo four (including negative turns).
pub fn rotate(canvas: &Canvas, turns: i32) -> Result<Canvas> {
    let turns = turns.rem_euclid(4);
    if turns == 0 {
        return Ok(canvas.clone());
    }
    let (w, h) = if turns % 2 == 0 {
        (canvas.width(), canvas.height())
    } else {
        (canvas.height(), canvas.width())
    };
    let mut result = Canvas::new(w, h)?;
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let (dx, dy) = match turns {
                1 => (canvas.height() - 1 - y, x),
                2 => (canvas.width() - 1 - x, canvas.height() - 1 - y),
                _ => (y, canvas.width() - 1 - x),
            };
            result.set_pixel(dx, dy, canvas.pixel(x, y).unwrap())?;
        }
    }
    Ok(result)
}

pub fn translate(canvas: &Canvas, dx: i32, dy: i32) -> Result<Canvas> {
    let mut result = Canvas::new(canvas.width(), canvas.height())?;
    paste(&mut result, canvas, dx, dy, false)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fully_clipped_rectangles_have_no_rows_to_scan() {
        let canvas = Canvas::new(1, 200_000).unwrap();
        for (x, y, width, height) in [
            (-1, 0, 1, 200_000),
            (1, 0, 1, 200_000),
            (0, -1, 1, 1),
            (0, 200_000, 1, 1),
        ] {
            let rect = Rect {
                x,
                y,
                width,
                height,
            };
            assert_eq!(rect.clipped(&canvas), (0, 0, 0, 0));
            assert_eq!(rect.visible_area(&canvas), 0);
        }
    }
}
