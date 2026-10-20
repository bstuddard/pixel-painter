// SPDX-License-Identifier: GPL-3.0-only
//! Deterministic pixel-art editing, layered animation, and portable asset exports.
//!
//! Edits operate in memory. Persistence is explicit; batches commit to their input
//! only after every operation and palette check succeeds.
//!
//! ```
//! use painter::{Canvas, Pixel};
//! let mut canvas = Canvas::new(16, 16)?;
//! canvas.set_pixel(4, 2, Pixel::rgba(255, 128, 0, 255))?;
//! let mut png = Vec::new();
//! canvas.write_png(&mut png)?;
//! assert!(png.starts_with(b"\x89PNG"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]

pub mod animation;
pub mod animation_check;
mod artwork_io;
pub mod assets;
pub mod atlas;
pub mod canvas;
pub mod document;
pub mod drawing;
pub mod error;
pub mod geometry;
pub mod inspect;
mod limits;
pub mod maps;
pub mod operations;
pub mod pack;
pub mod palette;
pub mod pixel;
pub mod placement;
pub mod recipe;
pub mod scene;
pub mod shadow;
pub mod storage;
pub mod style;
mod terminal;
pub mod tileset;
pub mod validation;

pub use canvas::Canvas;
pub use document::Document;
pub use error::{Error, ErrorCode, Result};
pub use pixel::Pixel;
