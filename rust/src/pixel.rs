// SPDX-License-Identifier: GPL-3.0-only

use serde::{Deserialize, Serialize};

/// RGBA channels, each ranging from 0 to 255.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// 0 is transparent; 255 is opaque.
    pub a: u8,
}
