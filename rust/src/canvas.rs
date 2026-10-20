// SPDX-License-Identifier: GPL-3.0-only

use crate::pixel::Pixel;
use crate::{Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

pub const MAX_PIXELS: u64 = 16_777_216;

/// Pixel dimensions and RGBA storage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "CanvasData")]
pub struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<Pixel>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CanvasData {
    width: u32,
    height: u32,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 16_777_216>")]
    pixels: Vec<Pixel>,
}

impl TryFrom<CanvasData> for Canvas {
    type Error = Error;
    fn try_from(data: CanvasData) -> Result<Self> {
        Self::from_pixels(data.width, data.height, data.pixels)
    }
}

impl std::fmt::Display for Canvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl Canvas {
    /// Create transparent pixels; reject zero dimensions or too many pixels.
    pub fn new(width: u32, height: u32) -> Result<Self> {
        let count = Self::pixel_count(width, height)?;
        Ok(Canvas {
            width,
            height,
            pixels: vec![Pixel::TRANSPARENT; count],
        })
    }

    /// Replace one pixel; return an error for coordinates outside the canvas.
    pub fn set_pixel(&mut self, x: u32, y: u32, pixel: Pixel) -> Result<()> {
        if x >= self.width || y >= self.height {
            return Err(Error::new(
                ErrorCode::OutOfBounds,
                format!("pixel ({x}, {y}) is outside canvas {self}"),
            ));
        }
        let index = y as usize * self.width as usize + x as usize;
        self.pixels[index] = pixel;
        Ok(())
    }

    /// Check that dimensions are allowed and match the stored pixel count.
    pub fn validate(&self) -> Result<()> {
        let expected = Self::pixel_count(self.width, self.height)?;
        if self.pixels.len() != expected {
            return Err(Error::invalid(format!(
                "expected {expected} pixels, found {}",
                self.pixels.len()
            )));
        }
        Ok(())
    }

    /// Calculate storage length, rejecting empty or oversized canvases.
    pub(crate) fn pixel_count(width: u32, height: u32) -> Result<usize> {
        let count = u64::from(width) * u64::from(height);
        if count == 0 || count > MAX_PIXELS {
            return Err(Error::limit(format!(
                "canvas must contain between 1 and {MAX_PIXELS} pixels"
            )));
        }
        Ok(count as usize)
    }

    /// Construct from exact row-major bytes, enforcing the same invariants as new().
    pub fn from_pixels(width: u32, height: u32, pixels: Vec<Pixel>) -> Result<Self> {
        let expected = Self::pixel_count(width, height)?;
        if pixels.len() != expected {
            return Err(Error::invalid(format!(
                "expected {expected} pixels, found {}",
                pixels.len()
            )));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn pixels(&self) -> &[Pixel] {
        &self.pixels
    }
    pub(crate) fn pixels_mut(&mut self) -> &mut [Pixel] {
        &mut self.pixels
    }

    /// Copy an entirely in-bounds region without user drawing coordinate limits.
    pub(crate) fn copy_from(&mut self, source: &Canvas, x: u32, y: u32) -> Result<()> {
        if x.checked_add(source.width)
            .is_none_or(|end| end > self.width)
            || y.checked_add(source.height)
                .is_none_or(|end| end > self.height)
        {
            return Err(Error::new(
                ErrorCode::OutOfBounds,
                "source region does not fit in destination",
            ));
        }
        let stride = self.width as usize;
        for (row, pixels) in source.pixels.chunks(source.width as usize).enumerate() {
            let start = (y as usize + row) * stride + x as usize;
            self.pixels[start..start + pixels.len()].copy_from_slice(pixels);
        }
        Ok(())
    }

    /// Extract a checked internal region; all valid canvas coordinates are allowed.
    pub(crate) fn region(&self, x: u32, y: u32, width: u32, height: u32) -> Result<Self> {
        let count = Self::pixel_count(width, height)?;
        if x.checked_add(width).is_none_or(|end| end > self.width)
            || y.checked_add(height).is_none_or(|end| end > self.height)
        {
            return Err(Error::new(
                ErrorCode::OutOfBounds,
                "region is outside canvas",
            ));
        }
        let mut pixels = Vec::with_capacity(count);
        for row in y..y + height {
            let start = row as usize * self.width as usize + x as usize;
            pixels.extend_from_slice(&self.pixels[start..start + width as usize]);
        }
        Self::from_pixels(width, height, pixels)
    }
    pub fn pixel(&self, x: u32, y: u32) -> Option<Pixel> {
        if x >= self.width || y >= self.height {
            None
        } else {
            Some(self.pixels[y as usize * self.width as usize + x as usize])
        }
    }

    /// Replace a signed coordinate if visible; off-canvas drawing is a no-op.
    pub(crate) fn put(&mut self, x: i64, y: i64, pixel: Pixel, over: bool) {
        if x >= 0 && y >= 0 && x < i64::from(self.width) && y < i64::from(self.height) {
            let target = &mut self.pixels[y as usize * self.width as usize + x as usize];
            *target = if over {
                pixel.over(*target, 255)
            } else {
                pixel
            };
        }
    }

    /// Encode original RGBA values as an eight-bit PNG without the preview checkerboard.
    pub fn write_png(&self, output: impl Write) -> io::Result<()> {
        self.validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let mut encoder = png::Encoder::new(output, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        // PNG expects consecutive red, green, blue, and alpha bytes for each pixel.
        let bytes: Vec<u8> = self
            .pixels
            .iter()
            .flat_map(|pixel| [pixel.r, pixel.g, pixel.b, pixel.a])
            .collect();
        let mut writer = encoder.write_header().map_err(io::Error::other)?;
        writer.write_image_data(&bytes).map_err(io::Error::other)?;
        writer.finish().map_err(io::Error::other)
    }

    /// Render two terminal cells per pixel, blending alpha over a gray checkerboard.
    pub fn view(&self) -> io::Result<()> {
        self.write_terminal(io::stdout().lock())
    }

    /// Write the terminal preview to a caller-provided sink, propagating IO errors.
    pub fn write_terminal(&self, mut output: impl Write) -> io::Result<()> {
        for (y, row) in self.pixels.chunks(self.width as usize).enumerate() {
            for (x, pixel) in row.iter().enumerate() {
                let background = if (x + y) % 2 == 0 { 192 } else { 128 };
                let alpha = u32::from(pixel.a);
                // Alpha weights the pixel against the background; +127 rounds to the nearest byte.
                let blend = |channel: u8| {
                    (u32::from(channel) * alpha + background * (255 - alpha) + 127) / 255
                };

                // ANSI background color; two spaces approximate a square pixel.
                write!(
                    output,
                    "\x1b[48;2;{};{};{}m  ",
                    blend(pixel.r),
                    blend(pixel.g),
                    blend(pixel.b),
                )?;
            }
            // Restore terminal colors before ending the row.
            writeln!(output, "\x1b[0m")?;
        }
        output.flush()
    }

    /// Print RGBA fields left to right, labeling each row from zero.
    pub fn print_pixels(&self) -> io::Result<()> {
        self.write_pixels(io::stdout().lock())
    }

    pub fn write_pixels(&self, mut output: impl Write) -> io::Result<()> {
        for (y, row) in self.pixels.chunks(self.width as usize).enumerate() {
            write!(output, "row {y}: ")?;
            for pixel in row {
                write!(output, "{pixel:?} ")?;
            }
            writeln!(output)?;
        }
        output.flush()
    }
}
