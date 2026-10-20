// SPDX-License-Identifier: GPL-3.0-only
//! Scoped, bounded inputs for recipes, previews, and public asset packs.

use crate::{Canvas, Error, Result, storage};
use serde::de::DeserializeOwned;
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

/// Authoring configurations are deliberately smaller than raster documents.
pub const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

pub fn read_config<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > MAX_CONFIG_BYTES {
        return Err(Error::limit("authoring configuration exceeds 4 MiB"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(Error::limit("authoring configuration exceeds 4 MiB"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

/// References are relative to one explicit root, including after symlink resolution.
pub struct AssetRoot(PathBuf);
impl AssetRoot {
    pub fn new(directory: &Path) -> Result<Self> {
        let root = directory.canonicalize()?;
        if !root.is_dir() {
            return Err(Error::invalid("asset root must be a directory"));
        }
        Ok(Self(root))
    }
    pub fn for_config(path: &Path) -> Result<Self> {
        Self::new(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
    }
    pub fn resolve(&self, relative: &Path) -> Result<PathBuf> {
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
            || relative.to_string_lossy().contains(['\\', ':'])
        {
            return Err(Error::invalid(
                "asset references require relative paths without traversal",
            ));
        }
        let path = self.0.join(relative).canonicalize()?;
        if !path.starts_with(&self.0) || !path.is_file() {
            return Err(Error::invalid(
                "asset reference leaves its root or is not a file",
            ));
        }
        Ok(path)
    }
    pub fn image(&self, relative: &Path, frame: usize) -> Result<Canvas> {
        let path = self.resolve(relative)?;
        load_image(&path, frame)
    }
}

pub fn load_image(path: &Path, frame: usize) -> Result<Canvas> {
    Budget::default().image(path, frame)
}

/// Join reference components with forward slashes on every host OS.
/// Native `Path::join` is for resolved filesystem paths, not serialized references.
/// Untrusted references still require validation through `AssetRoot::resolve`.
pub(crate) fn join_reference(base: &Path, suffix: impl AsRef<Path>) -> PathBuf {
    let mut reference = base.as_os_str().to_os_string();
    reference.push("/");
    reference.push(suffix.as_ref().as_os_str());
    PathBuf::from(reference)
}

/// Account for every read, including malformed files and nondisplayed cels.
/// Metadata charges happen before parsing; decoded raster charges precede flattening.
pub(crate) struct Budget {
    remaining_bytes: u64,
    remaining_pixels: u64,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            remaining_bytes: storage::MAX_ARTWORK_BYTES,
            remaining_pixels: crate::canvas::MAX_PIXELS,
        }
    }
}
impl Budget {
    fn file(&mut self, path: &Path) -> Result<()> {
        self.remaining_bytes = self
            .remaining_bytes
            .checked_sub(fs::metadata(path)?.len())
            .ok_or_else(|| Error::limit("asset inputs exceed aggregate byte limit"))?;
        Ok(())
    }
    pub(crate) fn config<T: DeserializeOwned>(&mut self, path: &Path) -> Result<T> {
        self.file(path)?;
        read_config(path)
    }
    fn pixels(&mut self, count: u64) -> Result<()> {
        self.remaining_pixels = self
            .remaining_pixels
            .checked_sub(count)
            .ok_or_else(|| Error::limit("asset inputs exceed aggregate source pixel limit"))?;
        Ok(())
    }
    pub(crate) fn document(&mut self, document: &crate::Document) -> Result<()> {
        self.pixels(
            u64::from(document.width())
                * u64::from(document.height())
                * document.frames().len() as u64
                * document.layers().len() as u64,
        )
    }
    pub(crate) fn used_pixels(&self) -> u64 {
        crate::canvas::MAX_PIXELS - self.remaining_pixels
    }
    pub(crate) fn artwork(&mut self, path: &Path) -> Result<storage::Artwork> {
        self.file(path)?;
        let artwork = if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        {
            storage::Artwork::Canvas(storage::import_png(path, false)?)
        } else {
            storage::load_artwork(path)?
        };
        match &artwork {
            storage::Artwork::Canvas(canvas) => self.pixels(canvas.pixels().len() as u64)?,
            storage::Artwork::Project(document) => self.document(document)?,
        }
        Ok(artwork)
    }
    pub(crate) fn image(&mut self, path: &Path, frame: usize) -> Result<Canvas> {
        if frame != 0
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        {
            return Err(Error::invalid("flat images only have frame 0"));
        }
        match self.artwork(path)? {
            storage::Artwork::Canvas(canvas) => {
                if frame != 0 {
                    return Err(Error::invalid("flat images only have frame 0"));
                }
                Ok(canvas)
            }
            storage::Artwork::Project(document) => document.flatten(frame),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Budget;
    use crate::{Document, ErrorCode, storage};
    use std::fs;

    #[test]
    fn image_budget_includes_other_frames_and_layers_before_flattening() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("project.json");
        let mut document = Document::new(1, 1).unwrap();
        document.add_layer("hidden".into()).unwrap();
        document.add_frame("other".into(), None, 100).unwrap();
        storage::save_project(&path, &document, true).unwrap();
        let mut budget = Budget {
            remaining_pixels: 3,
            ..Budget::default()
        };
        assert_eq!(
            budget.image(&path, 0).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        let mut budget = Budget {
            remaining_pixels: 4,
            ..Budget::default()
        };
        assert_eq!(budget.image(&path, 0).unwrap().pixels().len(), 1);
        assert_eq!(budget.remaining_pixels, 0);
        assert_eq!(
            budget.image(&path, 0).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }

    #[test]
    fn malformed_artwork_consumes_the_shared_byte_budget() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bad.json");
        fs::write(&path, b"not json").unwrap();
        let mut budget = Budget {
            remaining_bytes: 15,
            ..Budget::default()
        };
        assert_eq!(
            budget.artwork(&path).unwrap_err().code,
            ErrorCode::InvalidJson
        );
        assert_eq!(budget.remaining_bytes, 7);
        let error = budget.artwork(&path).unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert!(error.message.contains("aggregate byte"));
    }

    #[test]
    fn configs_share_the_byte_budget_even_when_decoding_fails() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bad.json");
        fs::write(&path, b"not json").unwrap();
        let mut budget = Budget {
            remaining_bytes: 15,
            ..Budget::default()
        };
        assert_eq!(
            budget.config::<serde_json::Value>(&path).unwrap_err().code,
            ErrorCode::InvalidJson
        );
        assert_eq!(
            budget.config::<serde_json::Value>(&path).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }

    #[cfg(unix)]
    #[test]
    fn scoped_references_reject_special_files_before_opening_them() {
        let root = super::AssetRoot::new(std::path::Path::new("/dev")).unwrap();
        assert!(root.resolve(std::path::Path::new("null")).is_err());
    }
}
