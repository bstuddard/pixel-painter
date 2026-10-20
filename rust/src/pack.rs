// SPDX-License-Identifier: GPL-3.0-only
//! Deterministic, transactional public asset families and complete-export checks.

use crate::{
    Document, Error, ErrorCode, Pixel, Result,
    assets::{AssetRoot, Budget},
    atlas::{Atlas, Options},
    placement::Placement,
    recipe::{Parameters, Recipe as AssetRecipe},
    storage,
    style::Profile,
    validation::Report,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Recipe {
        input: PathBuf,
        #[serde(default)]
        parameters: Parameters,
        #[serde(default)]
        variant: Option<String>,
        #[serde(default)]
        seed: Option<u64>,
    },
    Artwork {
        input: PathBuf,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub id: String,
    pub source: Source,
    #[serde(default)]
    pub placement: Option<PathBuf>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub version: u32,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub profile: Option<PathBuf>,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 64>")]
    pub assets: Vec<Asset>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportAsset {
    pub id: String,
    pub directory: PathBuf,
    pub width: u32,
    pub height: u32,
    pub frames: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<PathBuf>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 64>")]
    pub assets: Vec<ExportAsset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<PathBuf>,
}
impl Recipe {
    pub fn generate(&self, root: &AssetRoot, destination: &Path) -> Result<Report> {
        let mut work = crate::drawing::MAX_WORK;
        self.generate_budgeted(root, destination, &mut work)
    }
    fn generate_budgeted(
        &self,
        root: &AssetRoot,
        destination: &Path,
        work: &mut u64,
    ) -> Result<Report> {
        if self.version != 1 {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "pack version must be 1",
            ));
        }
        if self.assets.is_empty() || self.assets.len() > 64 {
            return Err(Error::invalid("packs require 1..=64 assets"));
        }
        let mut budget = Budget::default();
        let profile = self
            .profile
            .as_ref()
            .map(|path| budget.config::<Profile>(&root.resolve(path)?))
            .transpose()?;
        if let Some(profile) = &profile {
            profile.validate()?;
        }
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let stage = tempfile::tempdir_in(parent)?;
        fs::create_dir(stage.path().join("assets"))?;
        let mut report = Report::default();
        let mut ids = BTreeSet::new();
        let mut previews = Vec::new();
        let mut exported = Vec::new();
        for (index, asset) in self.assets.iter().enumerate() {
            crate::document::name_valid(&asset.id)?;
            if !ids.insert(&asset.id) {
                return Err(Error::invalid("duplicate pack asset ID"));
            }
            let document = match &asset.source {
                Source::Recipe {
                    input,
                    parameters,
                    variant,
                    seed,
                } => {
                    let recipe: AssetRecipe = budget.config(&root.resolve(input)?)?;
                    let document = Document::from_canvas(recipe.render_budgeted(
                        parameters,
                        variant.as_deref(),
                        Some(seed.unwrap_or(self.seed.wrapping_add(index as u64))),
                        profile.as_ref(),
                        work,
                    )?)?;
                    budget.document(&document)?;
                    document
                }
                Source::Artwork {
                    input,
                } => {
                    let path = root.resolve(input)?;
                    match budget.artwork(&path)? {
                        storage::Artwork::Canvas(canvas) => Document::from_canvas(canvas)?,
                        storage::Artwork::Project(document) => document,
                    }
                }
            };
            let placement = asset
                .placement
                .as_ref()
                .map(|path| budget.config::<Placement>(&root.resolve(path)?))
                .transpose()?;
            if let Some(profile) = &profile {
                profile.check_work(budget.used_pixels())?;
                if document.frames().len() > 1 {
                    report.extend(crate::animation_check::check(
                        &document,
                        profile,
                        placement.as_ref(),
                    )?);
                } else {
                    report.extend(profile.check(&document.flatten(0)?, &asset.id)?);
                }
            } else {
                report.checked += 1;
            }
            let directory = PathBuf::from(format!("assets/{index:04}"));
            fs::create_dir(stage.path().join(&directory))?;
            storage::save_project(
                &stage.path().join(&directory).join("source.painter.json"),
                &document,
                true,
            )?;
            let placement_file = placement
                .as_ref()
                .map(|_| directory.join("placement.source.json"));
            if let (Some(placement), Some(path)) = (&placement, &placement_file) {
                storage::write_json(&stage.path().join(path), placement, true)?;
            }
            let atlas = Atlas::from_document(&document, Options::default())?;
            let output = stage.path().join(&directory).join("export");
            if let Some(placement) = &placement {
                atlas.export_with_placement(&output, 1, &document, placement)?;
            } else {
                atlas.export(&output, 1)?;
            }
            previews.push(document.flatten(0)?);
            exported.push(ExportAsset {
                id: asset.id.clone(),
                directory,
                width: document.width(),
                height: document.height(),
                frames: document.frames().len(),
                placement: placement_file,
            });
        }
        if !report.valid {
            return Err(Error::invalid(
                "pack assets violate the shared profile; check source assets before export",
            ));
        }
        let preview =
            crate::scene::contact_sheet(&previews, None, 4, Pixel::rgba(33, 43, 52, 255), 4)?;
        storage::export_png(&stage.path().join("contact-sheet.png"), &preview)?;
        storage::write_json(&stage.path().join("report.json"), &report, true)?;
        if let Some(profile) = &profile {
            storage::write_json(&stage.path().join("profile.json"), profile, true)?;
        }
        storage::write_json(
            &stage.path().join("pack.json"),
            &Manifest {
                version: 1,
                assets: exported,
                profile: profile.as_ref().map(|_| PathBuf::from("profile.json")),
            },
            true,
        )?;
        storage::publish_bundle(stage.path(), destination, "pack.json")?;
        Ok(report)
    }
}

/// Verify every generated export byte against its editable source and stored placement.
/// Additional unrelated files are ignored. Generated files are immutable pack artifacts.
pub fn check(directory: &Path, profile: Option<&Profile>) -> Result<Report> {
    let root = AssetRoot::new(directory)?;
    let mut budget = Budget::default();
    let manifest: Manifest = budget.config(&root.resolve(Path::new("pack.json"))?)?;
    if manifest.version != 1 {
        return Err(Error::new(
            ErrorCode::UnsupportedVersion,
            "pack manifest version must be 1",
        ));
    }
    if manifest.assets.is_empty() || manifest.assets.len() > 64 {
        return Err(Error::invalid("pack manifest requires 1..=64 assets"));
    }
    let stored_profile = manifest
        .profile
        .as_ref()
        .map(|path| budget.config::<Profile>(&root.resolve(path)?))
        .transpose()?;
    let profile = profile.or(stored_profile.as_ref());
    if let Some(profile) = profile {
        profile.validate()?;
    }
    let mut report = Report::default();
    let mut ids = BTreeSet::new();
    let mut directories = BTreeSet::new();
    let mut previews = Vec::new();
    let stage = tempfile::tempdir()?;
    for (index, asset) in manifest.assets.into_iter().enumerate() {
        crate::document::name_valid(&asset.id)?;
        if !ids.insert(asset.id.clone()) || !directories.insert(asset.directory.clone()) {
            report.error(
                "duplicate_asset",
                &asset.id,
                "asset IDs and directories must be unique",
            );
        }
        let result = (|| -> Result<()> {
            let source = root.resolve(&asset.directory.join("source.painter.json"))?;
            let storage::Artwork::Project(document) = budget.artwork(&source)? else {
                return Err(Error::invalid("pack sources must be projects"));
            };
            if let Some(profile) = profile {
                profile.check_work(budget.used_pixels())?;
            }
            if asset.frames != document.frames().len()
                || document.width() != asset.width
                || document.height() != asset.height
            {
                return Err(Error::invalid("pack dimensions or frame counts disagree"));
            }
            let placement = asset
                .placement
                .as_ref()
                .map(|path| budget.config::<Placement>(&root.resolve(path)?))
                .transpose()?;
            let atlas = Atlas::from_document(&document, Options::default())?;
            let expected = stage.path().join(format!("{index:04}"));
            if let Some(placement) = &placement {
                atlas.export_with_placement(&expected, 1, &document, placement)?;
            } else {
                atlas.export(&expected, 1)?;
            }
            compare_bundle(&expected, &root, &asset.directory.join("export"))?;
            previews.push(document.flatten(0)?);
            if let Some(profile) = profile {
                if document.frames().len() > 1 {
                    report.extend(crate::animation_check::check(
                        &document,
                        profile,
                        placement.as_ref(),
                    )?);
                } else {
                    report.extend(profile.check(&document.flatten(0)?, &asset.id)?);
                }
            } else {
                report.checked += 1;
            }
            Ok(())
        })();
        if let Err(error) = result {
            if error.code == ErrorCode::ResourceLimit {
                return Err(error);
            }
            report.error("incomplete_asset", &asset.id, error.message);
        }
    }
    // Pack-level files are also part of the completeness contract.
    let summary: serde_json::Value = budget.config(&root.resolve(Path::new("report.json"))?)?;
    if summary.get("version").and_then(serde_json::Value::as_u64) != Some(1)
        || summary.get("valid").and_then(serde_json::Value::as_bool) != Some(true)
    {
        report.error("pack_report", "pack", "generation report is invalid");
    }
    if report.valid {
        let preview =
            crate::scene::contact_sheet(&previews, None, 4, Pixel::rgba(33, 43, 52, 255), 4)?;
        let expected = stage.path().join("contact-sheet.png");
        storage::export_png(&expected, &preview)?;
        if !same_file(&expected, &root.resolve(Path::new("contact-sheet.png"))?)? {
            report.error(
                "contact_sheet",
                "pack",
                "contact sheet differs from source assets",
            );
        }
    }
    Ok(report)
}
fn compare_bundle(expected: &Path, actual: &AssetRoot, relative: &Path) -> Result<()> {
    for entry in fs::read_dir(expected)? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            compare_bundle(&entry.path(), actual, &path)?;
        } else if !same_file(&entry.path(), &actual.resolve(&path)?)? {
            return Err(Error::invalid(format!(
                "export differs from source: {}",
                path.display()
            )));
        }
    }
    Ok(())
}
fn same_file(expected: &Path, actual: &Path) -> Result<bool> {
    use std::io::Read;
    let mut expected = fs::File::open(expected)?;
    let mut actual = fs::File::open(actual)?;
    if expected.metadata()?.len() != actual.metadata()?.len() {
        return Ok(false);
    }
    let (mut a, mut b) = ([0u8; 8192], [0u8; 8192]);
    loop {
        let count = expected.read(&mut a)?;
        if count == 0 {
            return Ok(true);
        }
        actual.read_exact(&mut b[..count])?;
        if a[..count] != b[..count] {
            return Ok(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Recipe;
    use crate::{ErrorCode, assets::AssetRoot, storage};
    use serde_json::json;

    #[test]
    fn pack_recipes_share_work_and_clean_up_on_failure() {
        let directory = tempfile::tempdir().unwrap();
        let operation = json!({"op":"clear","color":"#FF0000"});
        for name in ["first.json", "second.json"] {
            storage::write_json(
                &directory.path().join(name),
                &json!({"version":2,"width":2,"height":1,"operations":[operation]}),
                true,
            )
            .unwrap();
        }
        let recipe:Recipe=serde_json::from_value(json!({"version":1,"assets":[{"id":"first","source":{"kind":"recipe","input":"first.json"}},{"id":"second","source":{"kind":"recipe","input":"second.json"}}]})).unwrap();
        let root = AssetRoot::new(directory.path()).unwrap();
        let output = directory.path().join("output");
        let mut work = 3;
        assert_eq!(
            recipe
                .generate_budgeted(&root, &output, &mut work)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert!(!output.exists());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
        let mut work = 4;
        assert!(
            recipe
                .generate_budgeted(&root, &output, &mut work)
                .unwrap()
                .valid
        );
        assert_eq!(work, 0);
        assert!(super::check(&output, None).unwrap().valid);
    }
}
