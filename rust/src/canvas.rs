// SPDX-License-Identifier: GPL-3.0-only

use crate::pixel::Pixel;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

const MAX_PIXELS: u64 = 16_777_216;

/// Pixel dimensions and RGBA storage.
#[derive(Serialize, Deserialize)]
pub struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<Pixel>,
}

impl std::fmt::Display for Canvas {
    /// Format dimensions as width x height.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl Canvas {
    /// Create transparent pixels; reject zero dimensions or too many pixels.
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let count = Self::pixel_count(width, height)?;
        Ok(Canvas {
            width,
            height,
            pixels: vec![
                Pixel {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                };
                count
            ],
        })
    }

    /// Replace one pixel; return an error for coordinates outside the canvas.
    pub fn set_pixel(&mut self, x: u32, y: u32, pixel: Pixel) -> Result<(), String> {
        if x >= self.width || y >= self.height {
            return Err(format!("pixel ({x}, {y}) is outside canvas {self}"));
        }
        let index = y as usize * self.width as usize + x as usize;
        self.pixels[index] = pixel;
        Ok(())
    }

    /// Check that dimensions are allowed and match the stored pixel count.
    pub fn validate(&self) -> Result<(), String> {
        let expected = Self::pixel_count(self.width, self.height)?;
        if self.pixels.len() != expected {
            return Err(format!(
                "expected {expected} pixels, found {}",
                self.pixels.len()
            ));
        }
        Ok(())
    }

    /// Calculate storage length, rejecting empty or oversized canvases.
    fn pixel_count(width: u32, height: u32) -> Result<usize, String> {
        let count = u64::from(width) * u64::from(height);
        if count == 0 || count > MAX_PIXELS {
            return Err(format!(
                "canvas must contain between 1 and {MAX_PIXELS} pixels"
            ));
        }
        Ok(count as usize)
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
        if self.width == 0 {
            return Ok(());
        }

        let mut output = io::stdout().lock();
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
    pub fn print_pixels(&self) {
        if self.width == 0 {
            return;
        }

        // chunks groups rows; enumerate supplies each row's zero-based y coordinate.
        for (y, row) in self.pixels.chunks(self.width as usize).enumerate() {
            print!("row {y}: ");
            for pixel in row {
                print!("{pixel:?} ");
            }
            println!();
        }
    }
}
