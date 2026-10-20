// SPDX-License-Identifier: GPL-3.0-only
//! Semantic layer channels and linear, screen-space normal data.

use crate::{Canvas, Error, Pixel, Result, drawing::Axis};
use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    #[default]
    Color,
    Emissive,
    Normal,
}
impl Channel {
    pub fn is_color(&self) -> bool {
        *self == Self::Color
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Emissive => "emissive",
            Self::Normal => "normal",
        }
    }
    pub(crate) fn composite(self, source: Pixel, target: Pixel, opacity: u8) -> Pixel {
        if self != Self::Normal {
            return source.over(target, opacity);
        }
        let alpha = ((u16::from(source.a) * u16::from(opacity) + 127) / 255) as u8;
        if alpha == 0 {
            target
        } else {
            Pixel {
                a: alpha,
                ..source
            }
        }
    }
}
impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl FromStr for Channel {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "color" => Ok(Self::Color),
            "emissive" => Ok(Self::Emissive),
            "normal" => Ok(Self::Normal),
            _ => Err(Error::invalid("channel must be color, emissive, or normal")),
        }
    }
}

/// Requested companions; color is always exported. JSON uses a bounded name list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(into = "Vec<Channel>")]
pub struct MapRequest {
    pub emissive: bool,
    pub normal: bool,
}
impl MapRequest {
    pub fn is_empty(&self) -> bool {
        !self.emissive && !self.normal
    }
    pub fn channels(self) -> impl Iterator<Item = Channel> {
        [Channel::Emissive, Channel::Normal]
            .into_iter()
            .filter(move |channel| match channel {
                Channel::Emissive => self.emissive,
                Channel::Normal => self.normal,
                Channel::Color => false,
            })
    }
    fn from_channels(channels: impl IntoIterator<Item = Channel>) -> Result<Self> {
        let mut request = Self::default();
        for channel in channels {
            request.insert(channel)?;
        }
        Ok(request)
    }
    fn insert(&mut self, channel: Channel) -> Result<()> {
        let selected = match channel {
            Channel::Emissive => &mut self.emissive,
            Channel::Normal => &mut self.normal,
            Channel::Color => {
                return Err(Error::invalid(
                    "color is required; companion maps are emissive and normal",
                ));
            }
        };
        if *selected {
            return Err(Error::invalid("duplicate companion map"));
        }
        *selected = true;
        Ok(())
    }
}
impl From<MapRequest> for Vec<Channel> {
    fn from(value: MapRequest) -> Self {
        value.channels().collect()
    }
}
impl FromStr for MapRequest {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self> {
        if value.is_empty() {
            return Ok(Self::default());
        }
        let mut request = Self::default();
        for name in value.split(',') {
            request.insert(name.parse()?)?;
        }
        Ok(request)
    }
}
impl<'de> Deserialize<'de> for MapRequest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let channels = crate::limits::sequence::<'de, D, Channel, 2>(deserializer)?;
        Self::from_channels(channels).map_err(serde::de::Error::custom)
    }
}

pub const FLAT_NORMAL: Pixel = Pixel::rgba(128, 128, 255, 255);

/// Encode a finite, nonzero direction as XYZ UNORM8; +X right, +Y up, +Z viewer.
pub fn encode_normal(x: f64, y: f64, z: f64, alpha: u8) -> Result<Pixel> {
    let largest = x.abs().max(y.abs()).max(z.abs());
    if largest == 0.0 || ![x, y, z].into_iter().all(f64::is_finite) {
        return Err(Error::invalid(
            "normal direction must be finite and nonzero",
        ));
    }
    // Scale first so extreme finite magnitudes cannot overflow or underflow.
    let [x, y, z] = [x, y, z].map(|component| component / largest);
    let length = x.hypot(y).hypot(z);
    let encode = |value: f64| ((value / length + 1.0) * 127.5).round() as u8;
    Ok(Pixel::rgba(encode(x), encode(y), encode(z), alpha))
}

/// A 3% squared-length tolerance admits UNORM8 quantization, not arbitrary paint.
pub fn validate_normals(canvas: &Canvas) -> Result<()> {
    for pixel in canvas.pixels().iter().filter(|pixel| pixel.a != 0) {
        let component = |value: u8| 2 * i32::from(value) - 255;
        let square = [pixel.r, pixel.g, pixel.b]
            .map(component)
            .into_iter()
            .map(|v| v * v)
            .sum::<i32>();
        if square.abs_diff(65_025) > 1_951 {
            return Err(Error::invalid(
                "normal RGB must encode a unit direction (XYZ UNORM8)",
            ));
        }
    }
    Ok(())
}

pub(crate) fn flip_normal(pixel: &mut Pixel, axis: Axis) {
    if pixel.a == 0 {
        return;
    }
    match axis {
        Axis::Horizontal => pixel.r = 255 - pixel.r,
        Axis::Vertical => pixel.g = 255 - pixel.g,
    }
}
pub(crate) fn rotate_normal(pixel: &mut Pixel, turns: i32) {
    if pixel.a == 0 {
        return;
    }
    // Positive image turns are clockwise. Screen-space Y points upward.
    (pixel.r, pixel.g) = match turns.rem_euclid(4) {
        0 => (pixel.r, pixel.g),
        1 => (pixel.g, 255 - pixel.r),
        2 => (255 - pixel.r, 255 - pixel.g),
        _ => (255 - pixel.g, pixel.r),
    };
}
