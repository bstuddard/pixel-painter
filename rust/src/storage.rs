// SPDX-License-Identifier: GPL-3.0-only
//! Bounded input, atomic single-file persistence, and validated image import.

use crate::{Canvas, Document, Error, Pixel, Result};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File},
    io::{self, BufReader, BufWriter, Read, Write},
    path::Path,
};
use tempfile::NamedTempFile;

pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Enough for every permitted legacy/project raster, including pretty JSON.
/// Artwork reads stream with pixel, aggregate, string, depth, and byte limits.
pub const MAX_ARTWORK_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum Artwork {
    Canvas(Canvas),
    Project(Document),
}

impl Artwork {
    pub fn composite(&self) -> Result<Canvas> {
        match self {
            Self::Canvas(canvas) => Ok(canvas.clone()),
            Self::Project(project) => project.flatten(project.active_frame()),
        }
    }
    pub fn edit(&mut self, batch: &crate::operations::Batch) -> Result<crate::operations::Summary> {
        match self {
            Self::Canvas(canvas) => batch.apply(canvas),
            Self::Project(project) => project.edit(batch),
        }
    }
}

/// Decode configuration/metadata JSON under the 64 MiB limit. Use the artwork
/// loaders for canvases/projects, which preflight raster memory while streaming.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_slice(&read_bytes(path)?)?)
}

fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_FILE_BYTES {
        return Err(Error::limit("JSON input exceeds 64 MiB"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(Error::limit("JSON input exceeds 64 MiB"));
    }
    Ok(bytes)
}

pub fn load(path: &Path) -> Result<Canvas> {
    match load_artwork(path)? {
        Artwork::Canvas(canvas) => Ok(canvas),
        Artwork::Project(_) => Err(Error::invalid("expected a flat canvas")),
    }
}

/// Stream a legacy canvas or versioned project after validating its pixel counts.
pub fn load_artwork(path: &Path) -> Result<Artwork> {
    crate::artwork_io::load(path)
}

pub fn load_project(path: &Path) -> Result<Document> {
    match load_artwork(path)? {
        Artwork::Project(project) => Ok(project),
        Artwork::Canvas(_) => Err(Error::invalid(
            "use project migrate to convert a legacy canvas explicitly",
        )),
    }
}

/// Complete a sibling temporary file before replacing the destination.
/// New files use no-clobber persistence; edits preserve existing file permissions.
pub fn atomic_write(
    path: &Path,
    create_new: bool,
    encode: impl FnOnce(&mut dyn Write) -> Result<()>,
) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = NamedTempFile::new_in(parent)?;
    if !create_new {
        temporary
            .as_file()
            .set_permissions(fs::metadata(path)?.permissions())?;
    }
    {
        let mut writer = BufWriter::new(&mut temporary);
        encode(&mut writer)?;
        writer.flush()?;
    }
    temporary.as_file().sync_all()?;
    if create_new {
        temporary.persist_noclobber(path).map_err(|e| e.error)?;
    } else {
        temporary.persist(path).map_err(|e| e.error)?;
    }
    Ok(())
}

struct LimitedWriter<'a> {
    inner: &'a mut dyn Write,
    remaining: u64,
}
impl Write for LimitedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.remaining {
            return Err(io::Error::other(Error::limit(
                "JSON output exceeds byte limit",
            )));
        }
        let count = self.inner.write(bytes)?;
        self.remaining -= count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Atomically save configuration/metadata JSON under the 64 MiB limit.
pub fn write_json<T: Serialize>(path: &Path, value: &T, create_new: bool) -> Result<()> {
    write_json_limited(path, value, create_new, MAX_FILE_BYTES)
}

fn write_json_limited<T: Serialize>(
    path: &Path,
    value: &T,
    create_new: bool,
    limit: u64,
) -> Result<()> {
    atomic_write(path, create_new, |writer| {
        let mut limited = LimitedWriter {
            inner: writer,
            remaining: limit,
        };
        serde_json::to_writer_pretty(&mut limited, value)?;
        writeln!(limited)?;
        Ok(())
    })
}

/// Atomically save legacy canvas JSON with enough encoded space for MAX_PIXELS.
pub fn save(path: &Path, canvas: &Canvas, create_new: bool) -> Result<()> {
    write_json_limited(path, canvas, create_new, MAX_ARTWORK_BYTES)
}

/// Preserve the artwork's existing format, using the larger streamed-artifact limit.
pub fn save_artwork(path: &Path, artwork: &Artwork, create_new: bool) -> Result<()> {
    write_json_limited(path, artwork, create_new, MAX_ARTWORK_BYTES)
}
/// Atomically save a validated native project using the artwork byte limit.
pub fn save_project(path: &Path, project: &Document, create_new: bool) -> Result<()> {
    write_json_limited(path, project, create_new, MAX_ARTWORK_BYTES)
}

pub fn export_png(path: &Path, canvas: &Canvas) -> Result<()> {
    atomic_write(path, true, |writer| {
        canvas.write_png(writer)?;
        Ok(())
    })
}

/// Expand indexed/gray PNGs to RGBA8 without color correction or alpha flattening.
/// Sixteen-bit samples require opt-in truncation to their high byte. APNG is rejected.
pub fn import_png(path: &Path, reduce_16: bool) -> Result<Canvas> {
    if fs::metadata(path)?.len() > MAX_FILE_BYTES {
        return Err(Error::limit("PNG input exceeds 64 MiB"));
    }
    let mut decoder = png::Decoder::new(BufReader::new(File::open(path)?));
    decoder.set_limits(png::Limits {
        bytes: MAX_FILE_BYTES as usize,
    });
    decoder.set_transformations(if reduce_16 {
        png::Transformations::normalize_to_color8()
    } else {
        png::Transformations::EXPAND
    });
    let mut reader = decoder.read_info()?;
    let info = reader.info();
    Canvas::pixel_count(info.width, info.height)?;
    if info.animation_control.is_some() {
        return Err(Error::invalid(
            "animated PNG import is unsupported; import individual frames",
        ));
    }
    if info.bit_depth == png::BitDepth::Sixteen && !reduce_16 {
        return Err(Error::invalid("16-bit PNG requires --reduce-16"));
    }
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| Error::limit("PNG buffer overflow"))?;
    if size as u64 > MAX_FILE_BYTES {
        return Err(Error::limit("decoded PNG exceeds 64 MiB"));
    }
    let mut bytes = vec![0; size];
    let info = reader.next_frame(&mut bytes)?;
    // next_frame stops after image data; IEND and trailing ancillary chunks still
    // need validation before an import can commit any artwork.
    reader.finish()?;
    let channels = info.color_type.samples();
    let pixels = bytes[..info.buffer_size()]
        .chunks_exact(channels)
        .map(|p| match info.color_type {
            png::ColorType::Rgba => Pixel::rgba(p[0], p[1], p[2], p[3]),
            png::ColorType::Rgb => Pixel::rgba(p[0], p[1], p[2], 255),
            png::ColorType::GrayscaleAlpha => Pixel::rgba(p[0], p[0], p[0], p[1]),
            png::ColorType::Grayscale => Pixel::rgba(p[0], p[0], p[0], 255),
            png::ColorType::Indexed => unreachable!("EXPAND removes indexed output"),
        })
        .collect();
    Canvas::from_pixels(info.width, info.height, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_encoding_keeps_old_file_and_cleans_temporary() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("art.json");
        fs::write(&path, "old").unwrap();
        let result = atomic_write(&path, false, |writer| {
            writer.write_all(b"partial")?;
            Err(Error::invalid("injected encoder failure"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "old");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_replacement_cleans_up_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("art.json");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep.txt"), "keep").unwrap();
        assert!(save(&target, &Canvas::new(1, 1).unwrap(), false).is_err());
        assert_eq!(fs::read_to_string(target.join("keep.txt")).unwrap(), "keep");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn output_budget_failure_is_classified_and_preserves_old_artwork() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("art.json");
        fs::write(&path, b"original").unwrap();
        let error = write_json_limited(&path, &Canvas::new(1, 1).unwrap(), false, 16).unwrap_err();
        assert_eq!(error.code.as_str(), "resource_limit");
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn artwork_byte_budget_covers_worst_case_pixel_encoding_with_metadata_headroom() {
        let make = |width| {
            let mut doc = Document::new(width, 1).unwrap();
            let batch = crate::operations::Batch::new(vec![crate::operations::Operation::Clear {
                color: Some(crate::palette::Color::Rgba(Pixel::rgba(255, 255, 255, 255))),
            }]);
            doc.edit(&batch).unwrap();
            serde_json::to_vec_pretty(&doc).unwrap().len() as u64
        };
        let pixel_bytes = make(2) - make(1);
        assert_eq!(pixel_bytes, 124);
        assert!(MAX_ARTWORK_BYTES - crate::canvas::MAX_PIXELS * pixel_bytes >= MAX_FILE_BYTES);
    }
}
