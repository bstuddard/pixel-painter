// SPDX-License-Identifier: GPL-3.0-only
//! Stream large legacy/project JSON with a metadata-only preflight. File size is
//! independent of raster memory: no large encoded file or pixel tree is buffered.

use crate::{
    Canvas, Error, ErrorCode, Result,
    storage::{Artwork, MAX_ARTWORK_BYTES},
};
use serde::{
    Deserialize, Deserializer,
    de::{IgnoredAny, SeqAccess, Visitor},
};
use std::{
    fmt,
    fs::File,
    io::{self, BufReader, Read, Seek},
    path::Path,
};

struct PixelCount(usize);
impl<'de> Deserialize<'de> for PixelCount {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct Counter;
        impl<'de> Visitor<'de> for Counter {
            type Value = PixelCount;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a bounded pixel/layer array")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<PixelCount, A::Error> {
                let mut count = 0;
                while seq.next_element::<IgnoredAny>()?.is_some() {
                    count += 1;
                    if count as u64 > crate::canvas::MAX_PIXELS {
                        return Err(serde::de::Error::custom("pixel count exceeds canvas limit"));
                    }
                }
                Ok(PixelCount(count))
            }
        }
        deserializer.deserialize_seq(Counter)
    }
}

#[derive(Deserialize)]
struct CelProbe {
    width: u32,
    height: u32,
    pixels: PixelCount,
}
#[derive(Deserialize)]
struct FrameProbe {
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 128>")]
    cels: Vec<CelProbe>,
}
#[derive(Deserialize)]
#[serde(transparent)]
struct FramesProbe(
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 256>")] Vec<FrameProbe>,
);
#[derive(Deserialize)]
struct Probe {
    width: u32,
    height: u32,
    version: Option<u32>,
    pixels: Option<PixelCount>,
    layers: Option<PixelCount>,
    frames: Option<FramesProbe>,
}
impl Probe {
    fn validate(&self) -> Result<bool> {
        let expected = Canvas::pixel_count(self.width, self.height)?;
        if self.version.is_none() && self.frames.is_none() {
            if self.pixels.as_ref().map(|count| count.0) != Some(expected) {
                return Err(Error::invalid("canvas dimensions do not match pixel count"));
            }
            return Ok(false);
        }
        if self.version != Some(1) {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "project version must be 1",
            ));
        }
        let layers = self.layers.as_ref().map_or(0, |count| count.0);
        let frames = self
            .frames
            .as_ref()
            .ok_or_else(|| Error::invalid("project requires frames"))?;
        if !(1..=128).contains(&layers) || frames.0.is_empty() {
            return Err(Error::invalid(
                "project requires 1..=128 layers and 1..=256 frames",
            ));
        }
        let mut aggregate = 0u64;
        for frame in &frames.0 {
            if frame.cels.len() != layers {
                return Err(Error::invalid(
                    "project requires one cel per layer in each frame",
                ));
            }
            for cel in &frame.cels {
                if cel.width != self.width || cel.height != self.height || cel.pixels.0 != expected
                {
                    return Err(Error::invalid(
                        "project cel dimensions or pixel count do not match",
                    ));
                }
                aggregate += cel.pixels.0 as u64;
                if aggregate > crate::canvas::MAX_PIXELS {
                    return Err(Error::limit("project exceeds aggregate pixel limit"));
                }
            }
        }
        Ok(true)
    }
}

/// Limit bytes, literal length, and nesting before serde can allocate strings or
/// build its ignore-value stack. Escaped quotes/braces are handled across reads.
struct Guard<R> {
    inner: R,
    remaining: u64,
    in_string: bool,
    escaped: bool,
    string_bytes: usize,
    depth: usize,
}
impl<R> Guard<R> {
    fn new(inner: R, limit: u64) -> Self {
        Self {
            inner,
            remaining: limit,
            in_string: false,
            escaped: false,
            string_bytes: 0,
            depth: 0,
        }
    }
}
impl<R: Read> Read for Guard<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return if self.inner.read(&mut [0])? == 0 {
                Ok(0)
            } else {
                Err(io::Error::other(Error::limit(
                    "artwork exceeds JSON byte limit",
                )))
            };
        }
        let length = buffer
            .len()
            .min(self.remaining.min(usize::MAX as u64) as usize);
        let count = self.inner.read(&mut buffer[..length])?;
        self.remaining -= count as u64;
        for byte in &buffer[..count] {
            if self.in_string {
                self.string_bytes += 1;
                if self.string_bytes > 4096 {
                    return Err(io::Error::other(Error::limit(
                        "artwork JSON string exceeds 4096 encoded bytes",
                    )));
                }
                if self.escaped {
                    self.escaped = false;
                } else if *byte == b'\\' {
                    self.escaped = true;
                } else if *byte == b'"' {
                    self.in_string = false;
                }
            } else {
                match byte {
                    b'"' => {
                        self.in_string = true;
                        self.string_bytes = 1;
                    }
                    b'[' | b'{' => {
                        self.depth += 1;
                        if self.depth > 128 {
                            return Err(io::Error::other(Error::limit(
                                "artwork JSON nesting exceeds 128",
                            )));
                        }
                    }
                    b']' | b'}' => self.depth = self.depth.saturating_sub(1),
                    _ => {}
                }
            }
        }
        Ok(count)
    }
}

pub(crate) fn load(path: &Path) -> Result<Artwork> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() > MAX_ARTWORK_BYTES {
        return Err(Error::limit("artwork JSON exceeds 2 GiB"));
    }
    let probe: Probe =
        serde_json::from_reader(BufReader::new(Guard::new(&mut file, MAX_ARTWORK_BYTES)))?;
    let project = probe.validate()?;
    // Reuse the same open file so an atomic replacement between passes cannot
    // substitute a different artifact after its preflight succeeds.
    file.rewind()?;
    let reader = BufReader::new(Guard::new(&mut file, MAX_ARTWORK_BYTES));
    if project {
        Ok(Artwork::Project(serde_json::from_reader(reader)?))
    } else {
        Ok(Artwork::Canvas(serde_json::from_reader(reader)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guard_enforces_exact_byte_boundary_and_small_read_chunks() {
        let mut exact = Guard::new(&b"1234"[..], 4);
        let mut data = Vec::new();
        exact.read_to_end(&mut data).unwrap();
        assert_eq!(data, b"1234");
        let mut extra = Guard::new(&b"12345"[..], 4);
        assert!(extra.read_to_end(&mut Vec::new()).is_err());
        let mut escaped = Guard::new(&b"[\"a\\\"{b\\\\c\"]"[..], 100);
        let mut byte = [0];
        while escaped.read(&mut byte).unwrap() != 0 {}
        assert!(!escaped.in_string);
        assert_eq!(escaped.depth, 0);
    }
    #[test]
    fn guard_rejects_large_literals_and_excessive_nesting() {
        let text = format!("\"{}\"", "x".repeat(4097));
        assert!(
            Guard::new(text.as_bytes(), 10000)
                .read_to_end(&mut Vec::new())
                .is_err()
        );
        assert!(
            Guard::new("[".repeat(129).as_bytes(), 10000)
                .read_to_end(&mut Vec::new())
                .is_err()
        );
    }

    #[test]
    fn preflight_rejects_aggregate_pixels_without_constructing_any_rasters() {
        let cel = || CelProbe {
            width: 4096,
            height: 4096,
            pixels: PixelCount(16_777_216),
        };
        let probe = Probe {
            width: 4096,
            height: 4096,
            version: Some(1),
            pixels: None,
            layers: Some(PixelCount(1)),
            frames: Some(FramesProbe(vec![
                FrameProbe {
                    cels: vec![cel()],
                },
                FrameProbe {
                    cels: vec![cel()],
                },
            ])),
        };
        assert_eq!(
            probe.validate().unwrap_err().code.as_str(),
            "resource_limit"
        );
    }

    #[test]
    fn guard_resource_error_survives_serde_and_io_wrapping() {
        let text = format!(
            "{{\"width\":1,\"height\":1,\"pixels\":[\"{}\"]}}",
            "x".repeat(4097)
        );
        let error =
            serde_json::from_reader::<_, Probe>(BufReader::new(Guard::new(text.as_bytes(), 10000)))
                .err()
                .unwrap();
        assert_eq!(Error::from(error).code.as_str(), "resource_limit");
    }

    #[test]
    fn guard_preserves_json_escapes_unicode_and_encoded_string_boundaries() {
        let values = [
            serde_json::json!({"text":"quotes: \" and \\ and []{}\n\t\u{0000}","unicode":"éclair 🌱"}),
            serde_json::json!([null,true,false,0,-12,1.25,{"nested":["\\\"[]{}",[]]}]),
        ];
        for value in values {
            let bytes = serde_json::to_vec(&value).unwrap();
            for chunk_size in 1..=17 {
                let mut guard = Guard::new(bytes.as_slice(), bytes.len() as u64);
                let mut output = Vec::new();
                let mut buffer = vec![0; chunk_size];
                loop {
                    let count = guard.read(&mut buffer).unwrap();
                    if count == 0 {
                        break;
                    }
                    output.extend_from_slice(&buffer[..count]);
                }
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&output).unwrap(),
                    value
                );
            }
        }
        for (contents, accepted) in [(4094, true), (4095, false)] {
            let text = format!("\"{}\"", "x".repeat(contents));
            assert_eq!(
                Guard::new(text.as_bytes(), 10000)
                    .read_to_end(&mut Vec::new())
                    .is_ok(),
                accepted
            );
        }
    }
}
