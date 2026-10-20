// SPDX-License-Identifier: GPL-3.0-only

use crate::{Error, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt, str::FromStr};

/// Straight RGBA bytes; serialization retains the original canvas JSON structure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// 0 is transparent; 255 is opaque.
    pub a: u8,
}

impl Pixel {
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r,
            g,
            b,
            a,
        }
    }

    /// Source-over in byte sRGB, using integer rounding and straight alpha.
    pub fn over(self, destination: Self, opacity: u8) -> Self {
        let sa = (u32::from(self.a) * u32::from(opacity) + 127) / 255;
        if sa == 0 {
            return destination;
        }
        let da = u32::from(destination.a);
        let alpha = sa * 255 + da * (255 - sa);
        let channel = |src: u8, dst: u8| {
            ((u32::from(src) * sa * 255 + u32::from(dst) * da * (255 - sa) + alpha / 2) / alpha)
                as u8
        };
        Self::rgba(
            channel(self.r, destination.r),
            channel(self.g, destination.g),
            channel(self.b, destination.b),
            ((alpha + 127) / 255) as u8,
        )
    }
}

impl FromStr for Pixel {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self> {
        let hex = text
            .strip_prefix('#')
            .ok_or_else(|| Error::invalid("color must be #RRGGBB or #RRGGBBAA"))?;
        if !matches!(hex.len(), 6 | 8) || !hex.is_ascii() {
            return Err(Error::invalid("color must be #RRGGBB or #RRGGBBAA"));
        }
        let byte = |start| {
            u8::from_str_radix(&hex[start..start + 2], 16)
                .map_err(|_| Error::invalid("invalid hex color"))
        };
        Ok(Self::rgba(
            byte(0)?,
            byte(2)?,
            byte(4)?,
            if hex.len() == 8 { byte(6)? } else { 255 },
        ))
    }
}
impl fmt::Display for Pixel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{:02X}{:02X}{:02X}{:02X}",
            self.r, self.g, self.b, self.a
        )
    }
}
impl<'de> Deserialize<'de> for Pixel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Bytes {
            r: u8,
            g: u8,
            b: u8,
            #[serde(default = "opaque")]
            a: u8,
        }
        fn opaque() -> u8 {
            255
        }
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Pixel;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an RGBA object or hex color")
            }
            fn visit_str<E: serde::de::Error>(self, text: &str) -> std::result::Result<Pixel, E> {
                text.parse().map_err(E::custom)
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> std::result::Result<Pixel, A::Error> {
                let p = Bytes::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(Pixel::rgba(p.r, p.g, p.b, p.a))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}
