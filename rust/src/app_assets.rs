// SPDX-License-Identifier: GPL-3.0-only
//! CLI adapters for bounded authoring, composition, and validation.

use crate::{
    app::{emit, path_value},
    cli::{AnimationCommand, PackCommand, SceneCommand, StyleCommand},
};
use painter::{
    Result,
    assets::{self, AssetRoot},
    storage,
    style::Profile,
    validation::Report,
};
use std::process::ExitCode;

fn report(report: Report) -> Result<ExitCode> {
    emit(&report)?;
    Ok(if report.valid {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
pub(crate) fn style(command: StyleCommand) -> Result<ExitCode> {
    match command {
        StyleCommand::Reference {
            profile,
            output,
        } => {
            let profile: Profile = assets::read_config(&profile)?;
            storage::export_png(&output, &profile.reference()?)?;
            saved(&output)
        }
        StyleCommand::Check {
            profile,
            input,
        } => {
            let profile: Profile = assets::read_config(&profile)?;
            let mut result = Report::default();
            if input
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png"))
            {
                result.extend(profile.check(&storage::import_png(&input, false)?, "image")?);
            } else {
                match storage::load_artwork(&input)? {
                    storage::Artwork::Canvas(canvas) => {
                        result.extend(profile.check(&canvas, "image")?)
                    }
                    storage::Artwork::Project(document) => {
                        result.extend(profile.check_document(&document)?);
                    }
                }
            }
            report(result)
        }
    }
}
pub(crate) fn scene(command: SceneCommand) -> Result<ExitCode> {
    let (input, output, scale, contact) = match command {
        SceneCommand::Render {
            input,
            output,
            scale,
        } => (input, output, scale, None),
        SceneCommand::ContactSheet {
            input,
            output,
            columns,
            padding,
            scale,
        } => (input, output, scale, Some((columns, padding))),
    };
    let root = AssetRoot::for_config(&input)?;
    let scene: painter::scene::Scene = assets::read_config(&input)?;
    let canvas = if let Some((columns, padding)) = contact {
        scene.contact_sheet(&root, columns, padding, scale)?
    } else {
        scene.render(&root, scale)?
    };
    storage::export_png(&output, &canvas)?;
    saved(&output)
}
pub(crate) fn animation(command: AnimationCommand) -> Result<ExitCode> {
    match command {
        AnimationCommand::Check {
            input,
            profile,
            placement,
        } => {
            let document = storage::load_project(&input)?;
            let profile = assets::read_config(&profile)?;
            let placement = placement.as_deref().map(assets::read_config).transpose()?;
            report(painter::animation_check::check(
                &document,
                &profile,
                placement.as_ref(),
            )?)
        }
        AnimationCommand::ContactSheet {
            input,
            output,
            scale,
        } => {
            let document = storage::load_project(&input)?;
            storage::export_png(
                &output,
                &painter::animation_check::contact_sheet(&document, scale)?,
            )?;
            saved(&output)
        }
    }
}
pub(crate) fn pack(command: PackCommand) -> Result<ExitCode> {
    match command {
        PackCommand::Generate {
            input,
            output,
        } => {
            let recipe: painter::pack::Recipe = assets::read_config(&input)?;
            let root = AssetRoot::for_config(&input)?;
            report(recipe.generate(&root, &output)?)
        }
        PackCommand::Check {
            input,
            profile,
        } => {
            let profile = profile.as_deref().map(assets::read_config).transpose()?;
            report(painter::pack::check(&input, profile.as_ref())?)
        }
    }
}

fn saved(output: &std::path::Path) -> Result<ExitCode> {
    emit(&serde_json::json!({"version":1,"ok":true,"output":path_value(output)}))?;
    Ok(ExitCode::SUCCESS)
}
