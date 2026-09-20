// SPDX-License-Identifier: GPL-3.0-only

use crate::canvas::Canvas;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::Path;
use tempfile::NamedTempFile;

/// Read a JSON canvas and validate its dimensions and pixel count.
pub fn load(path: &Path) -> io::Result<Canvas> {
    let file = File::open(path)?;
    let canvas: Canvas = serde_json::from_reader(BufReader::new(file))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    // Deserialization bypasses new(), so check the dimensions and pixel count.
    canvas
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(canvas)
}

/// Write a complete temporary file before replacing artwork; create_new refuses overwrites.
pub fn save(path: &Path, canvas: &Canvas, create_new: bool) -> io::Result<()> {
    canvas
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut temporary = sibling_temporary(path)?;
    if !create_new {
        temporary
            .as_file()
            .set_permissions(fs::metadata(path)?.permissions())?;
    }
    {
        let mut writer = BufWriter::new(&mut temporary);
        serde_json::to_writer_pretty(&mut writer, canvas)?;
        writeln!(writer)?;
        writer.flush()?;
    }
    temporary.as_file().sync_all()?;
    if create_new {
        temporary
            .persist_noclobber(path)
            .map_err(|error| error.error)?;
    } else {
        temporary.persist(path).map_err(|error| error.error)?;
    }
    Ok(())
}

/// Export a complete PNG without overwriting existing files or modifying the JSON canvas.
pub fn export_png(path: &Path, canvas: &Canvas) -> io::Result<()> {
    let mut temporary = sibling_temporary(path)?;
    {
        let mut writer = BufWriter::new(&mut temporary);
        canvas.write_png(&mut writer)?;
        writer.flush()?;
    }
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| error.error)?;
    Ok(())
}

/// Create a sibling temporary file so persistence stays on the same filesystem.
fn sibling_temporary(path: &Path) -> io::Result<NamedTempFile> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    NamedTempFile::new_in(parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Invalid canvas data must not overwrite previously saved artwork.
    #[test]
    fn invalid_save_preserves_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("art.json");
        save(&path, &Canvas::new(2, 2).unwrap(), true).unwrap();
        let original = fs::read(&path).unwrap();
        let invalid: Canvas =
            serde_json::from_str(r#"{"width":2,"height":2,"pixels":[]}"#).unwrap();
        assert!(save(&path, &invalid, false).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    /// A failed replacement leaves the target directory and its contents untouched.
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
}
