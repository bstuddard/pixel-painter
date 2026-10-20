// SPDX-License-Identifier: GPL-3.0-only
//! CLI dispatch: library calls, explicit persistence, and human/JSON presentation.

use crate::cli::{
    Bounds, Cli, Command, Draw, FrameAction, LayerAction, Packing, ProjectCommand, TagAction,
    TileCommand, Transform,
};
use painter::{
    Canvas, Document, Error, ErrorCode, Pixel, Result,
    animation::Tag,
    atlas::{Atlas, Options},
    drawing::{self, Point, Rect},
    inspect,
    operations::{Batch, Operation},
    palette::{Color, Palette, Policy},
    storage::{self, Artwork},
    template::Template,
    tileset::{Rules, Tileset},
};
use serde::Serialize;
use serde_json::json;
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

fn emit(value: &impl Serialize) -> Result<()> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, value)?;
    writeln!(stdout)?;
    Ok(())
}

/// Preserve ordinary JSON strings and encode otherwise unrepresentable OS paths
/// losslessly. PathBuf's serializer rejects these paths, which json! would panic on.
fn path_value(path: &Path) -> serde_json::Value {
    if let Some(path) = path.to_str() {
        return serde_json::Value::String(path.into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        json!({"display":path.to_string_lossy(),"encoding":"unix_bytes","bytes":path.as_os_str().as_bytes()})
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        json!({"display":path.to_string_lossy(),"encoding":"windows_wide","units":path.as_os_str().encode_wide().collect::<Vec<_>>()})
    }
    #[cfg(not(any(unix, windows)))]
    {
        json!({"display":path.to_string_lossy(),"encoding":"platform_bytes","bytes":path.as_os_str().as_encoded_bytes()})
    }
}
fn message(json_output: bool, text: String, value: serde_json::Value) -> Result<()> {
    if json_output {
        emit(&value)
    } else {
        writeln!(io::stdout().lock(), "{}", crate::terminal::Escaped(&text))?;
        Ok(())
    }
}
struct Context {
    file: PathBuf,
    json: bool,
    frame: Option<usize>,
    layer: Option<String>,
}
impl Context {
    fn load(&self) -> Result<Artwork> {
        let mut artwork = storage::load_artwork(&self.file)?;
        match &mut artwork {
            Artwork::Project(project) => {
                if let Some(frame) = self.frame {
                    project.select_frame(frame)?;
                }
                if let Some(layer) = &self.layer {
                    project.select_layer(layer)?;
                }
            }
            Artwork::Canvas(_) if self.frame.is_some() || self.layer.is_some() => {
                return Err(Error::invalid("frame/layer selection requires a project"));
            }
            Artwork::Canvas(_) => {}
        }
        Ok(artwork)
    }
    fn edit(&self, batch: Batch, dry_run: bool) -> Result<()> {
        let mut artwork = self.load()?;
        let summary = artwork.edit(&batch)?;
        if !dry_run {
            storage::save_artwork(&self.file, &artwork, false)?;
        }
        if self.json {
            emit(&json!({"version":1,"ok":true,"dry_run":dry_run,"summary":summary}))
        } else {
            writeln!(
                io::stdout().lock(),
                "{} {} operations; {} changed pixels ({}x{})",
                if dry_run { "Validated" } else { "Applied" },
                summary.operations,
                summary.changed_pixels,
                summary.width,
                summary.height
            )?;
            Ok(())
        }
    }
}
fn palette(path: Option<&Path>) -> Result<Option<Palette>> {
    path.map(|path| {
        let palette: Palette = storage::read_json(path)?;
        palette.validate()?;
        Ok(palette)
    })
    .transpose()
}
fn constrain(canvas: &mut Canvas, palette: Option<&Palette>, policy: Policy) -> Result<()> {
    if let Some(palette) = palette {
        palette.constrain(canvas, policy)
    } else if matches!(policy, Policy::Unrestricted) {
        Ok(())
    } else {
        Err(Error::invalid("palette policy requires --palette"))
    }
}
impl From<Bounds> for Rect {
    fn from(b: Bounds) -> Self {
        Self {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        }
    }
}
fn options(packing: &Packing) -> Options {
    Options {
        columns: packing.columns,
        padding: packing.padding,
        margin: packing.margin,
    }
}

pub(crate) fn run(cli: Cli) -> Result<()> {
    let context = Context {
        file: cli.file,
        json: cli.json,
        frame: cli.frame,
        layer: cli.layer,
    };
    match cli.command {
        Command::New {
            width,
            height,
        } => {
            let canvas = Canvas::new(width, height)?;
            storage::save(&context.file, &canvas, true)?;
            message(
                context.json,
                format!("Created {canvas} at {}", context.file.display()),
                json!({"version":1,"ok":true,"width":width,"height":height,"file":path_value(&context.file)}),
            )?;
            if !context.json {
                canvas.print_pixels()?;
            }
            Ok(())
        }
        Command::SetPixel {
            x,
            y,
            r,
            g,
            b,
            a,
        } => {
            let mut artwork = context.load()?;
            // Legacy coordinates are u32; validate before converting to signed drawing input.
            let x = i32::try_from(x)
                .map_err(|_| Error::new(ErrorCode::OutOfBounds, "x is outside canvas"))?;
            let y = i32::try_from(y)
                .map_err(|_| Error::new(ErrorCode::OutOfBounds, "y is outside canvas"))?;
            artwork.edit(&Batch::new(vec![Operation::SetPixel {
                x,
                y,
                color: Color::Rgba(Pixel::rgba(r, g, b, a)),
                over: false,
            }]))?;
            storage::save_artwork(&context.file, &artwork, false)?;
            message(
                context.json,
                format!("Updated ({x}, {y}) in {}", context.file.display()),
                json!({"version":1,"ok":true,"x":x,"y":y}),
            )
        }
        Command::Print => {
            let canvas = context.load()?.composite()?;
            if context.json {
                emit(&canvas)
            } else {
                writeln!(io::stdout().lock(), "{canvas}")?;
                canvas.print_pixels()?;
                Ok(())
            }
        }
        Command::View => {
            let canvas = context.load()?.composite()?;
            if context.json {
                emit(&inspect::inspect(&canvas, None))
            } else {
                writeln!(io::stdout().lock(), "{canvas}")?;
                canvas.view()?;
                Ok(())
            }
        }
        Command::Export {
            output,
        } => {
            let canvas = context.load()?.composite()?;
            storage::export_png(&output, &canvas)?;
            message(
                context.json,
                format!("Exported {canvas} to {}", output.display()),
                json!({"version":1,"ok":true,"output":path_value(&output)}),
            )
        }
        Command::Render {
            output,
            scale,
            replace,
        } => {
            if scale == 0 || scale > 64 {
                return Err(Error::invalid("scale must be 1..=64"));
            }
            // canonicalize also rejects symlink aliases to the source before replacement.
            if replace
                && output.exists()
                && std::fs::canonicalize(&output)? == std::fs::canonicalize(&context.file)?
            {
                return Err(Error::invalid("preview cannot replace input artwork"));
            }
            let canvas = context.load()?.composite()?;
            let scaled = drawing::resize(
                &canvas,
                canvas
                    .width()
                    .checked_mul(scale)
                    .ok_or_else(|| Error::limit("preview width overflow"))?,
                canvas
                    .height()
                    .checked_mul(scale)
                    .ok_or_else(|| Error::limit("preview height overflow"))?,
            )?;
            storage::atomic_write(&output, !(replace && output.try_exists()?), |writer| {
                scaled.write_png(writer)?;
                Ok(())
            })?;
            message(
                context.json,
                format!("Rendered {}", output.display()),
                json!({"version":1,"ok":true,"output":path_value(&output),"width":scaled.width(),"height":scaled.height()}),
            )
        }
        Command::Inspect {
            palette: path,
        } => {
            let palette = palette(path.as_deref())?;
            emit(&inspect::inspect(
                &context.load()?.composite()?,
                palette.as_ref(),
            ))
        }
        Command::Batch {
            input,
            dry_run,
            palette: path,
            policy,
        } => {
            let mut batch: Batch = storage::read_json(&input)?;
            if let Some(path) = path {
                batch.palette = palette(Some(&path))?;
            }
            if let Some(policy) = policy {
                batch.policy = policy.into();
            }
            context.edit(batch, dry_run)
        }
        Command::Draw {
            palette: path,
            shape,
        } => {
            let operation = draw_operation(shape)?;
            let mut batch = Batch::new(vec![operation]);
            batch.palette = palette(path.as_deref())?;
            context.edit(batch, false)
        }
        Command::Transform {
            transform,
        } => context.edit(
            Batch::new(vec![match transform {
                Transform::Flip {
                    axis,
                } => Operation::Flip {
                    axis: axis.into(),
                },
                Transform::Rotate {
                    turns,
                } => Operation::Rotate {
                    turns,
                },
                Transform::Move {
                    dx,
                    dy,
                } => Operation::Move {
                    dx,
                    dy,
                },
                Transform::Crop {
                    bounds: b,
                } => Operation::Crop {
                    x: b.x,
                    y: b.y,
                    width: b.width,
                    height: b.height,
                },
                Transform::Resize {
                    width,
                    height,
                } => Operation::Resize {
                    width,
                    height,
                },
            }]),
            false,
        ),
        Command::Extract {
            bounds,
            output,
        } => {
            let canvas = drawing::crop(&context.load()?.composite()?, bounds.into())?;
            storage::save(&output, &canvas, true)?;
            message(
                context.json,
                format!("Extracted {}", output.display()),
                json!({"version":1,"ok":true,"output":path_value(&output)}),
            )
        }
        Command::Stamp {
            input,
            x,
            y,
            over,
        } => context.edit(
            Batch::new(vec![Operation::Stamp {
                canvas: storage::load(&input)?,
                x,
                y,
                over,
            }]),
            false,
        ),
        Command::Import {
            input,
            reduce_16,
            palette: path,
            policy,
        } => {
            let mut canvas = storage::import_png(&input, reduce_16)?;
            let palette = palette(path.as_deref())?;
            constrain(&mut canvas, palette.as_ref(), policy.into())?;
            storage::save(&context.file, &canvas, true)?;
            message(
                context.json,
                format!("Imported {}", input.display()),
                json!({"version":1,"ok":true,"file":path_value(&context.file)}),
            )
        }
        Command::Generate {
            input,
            seed,
        } => {
            let template: Template = storage::read_json(&input)?;
            let canvas = template.render(seed)?;
            storage::save(&context.file, &canvas, true)?;
            message(
                context.json,
                format!("Generated {}", context.file.display()),
                json!({"version":1,"ok":true,"file":path_value(&context.file),"seed":seed.unwrap_or(template.batch.seed)}),
            )
        }
        Command::Snapshot {
            output,
        } => {
            storage::save_artwork(&output, &context.load()?, true)?;
            message(
                context.json,
                format!("Checkpoint {}", output.display()),
                json!({"version":1,"ok":true,"output":path_value(&output)}),
            )
        }
        Command::Project {
            command,
        } => project_command(command, context.json),
        Command::Tiles {
            tile_width,
            tile_height,
            source_margin,
            source_spacing,
            command,
        } => {
            let tiles = Tileset::slice(
                &context.load()?.composite()?,
                tile_width,
                tile_height,
                source_margin,
                source_spacing,
            )?;
            match command {
                TileCommand::Export {
                    packing,
                    rules,
                } => {
                    let rules = rules
                        .as_deref()
                        .map(storage::read_json::<Rules>)
                        .transpose()?;
                    tiles
                        .atlas(options(&packing), rules.as_ref())?
                        .export(&packing.output, packing.scale)?;
                    message(
                        context.json,
                        format!(
                            "Exported {} tiles to {}",
                            tiles.len(),
                            packing.output.display()
                        ),
                        json!({"version":1,"ok":true,"tiles":tiles.len(),"output":path_value(&packing.output)}),
                    )
                }
                TileCommand::Seams {
                    rules,
                } => {
                    let rules: Rules = storage::read_json(&rules)?;
                    let seams = tiles.seams(&rules)?;
                    let valid = seams.iter().all(|s| s.mismatched_pixels == 0);
                    emit(&json!({"version":1,"valid":valid,"seams":seams}))?;
                    if valid {
                        Ok(())
                    } else {
                        Err(Error::new(
                            ErrorCode::SeamMismatch,
                            "tile edges do not match",
                        ))
                    }
                }
            }
        }
    }
}

fn draw_operation(shape: Draw) -> Result<Operation> {
    let color = |s: String| Color::Named(s);
    Ok(match shape {
        Draw::Line {
            x1,
            y1,
            x2,
            y2,
            color: c,
            over,
        } => Operation::Line {
            x1,
            y1,
            x2,
            y2,
            color: color(c),
            over,
        },
        Draw::Rect {
            bounds: b,
            color: c,
            fill,
            over,
        } => Operation::Rect {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
            color: color(c),
            fill,
            over,
        },
        Draw::Ellipse {
            bounds: b,
            color: c,
            fill,
            over,
        } => Operation::Ellipse {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
            color: color(c),
            fill,
            over,
        },
        Draw::Polygon {
            points,
            color: c,
            fill,
            over,
        } => {
            if points.len() % 2 != 0 {
                return Err(Error::invalid("polygon points must be x,y pairs"));
            }
            Operation::Polygon {
                points: points
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| Point {
                        x: p[0],
                        y: p[1],
                    })
                    .collect(),
                color: color(c),
                fill,
                over,
            }
        }
        Draw::Fill {
            x,
            y,
            color: c,
        } => Operation::Fill {
            x,
            y,
            color: color(c),
        },
        Draw::Erase {
            bounds: b,
        } => Operation::Erase {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        },
        Draw::Clear {
            color: c,
        } => Operation::Clear {
            color: c.map(color),
        },
    })
}

fn project_command(command: ProjectCommand, json_output: bool) -> Result<()> {
    let (path, project, create_new) = match command {
        ProjectCommand::New {
            path,
            width,
            height,
        } => (path, Document::new(width, height)?, true),
        ProjectCommand::Migrate {
            input,
            output,
        } => (output, Document::from_canvas(storage::load(&input)?)?, true),
        ProjectCommand::Show {
            path,
        } => return emit(&storage::load_project(&path)?),
        ProjectCommand::Export {
            path,
            packing,
        } => {
            Atlas::from_document(&storage::load_project(&path)?, options(&packing))?
                .export(&packing.output, packing.scale)?;
            return message(
                json_output,
                format!("Exported {}", packing.output.display()),
                json!({"version":1,"ok":true,"output":path_value(&packing.output)}),
            );
        }
        ProjectCommand::Resize {
            path,
            width,
            height,
        } => {
            let mut project = storage::load_project(&path)?;
            project.resize(width, height)?;
            (path, project, false)
        }
        ProjectCommand::Layer {
            path,
            action,
        } => {
            let mut project = storage::load_project(&path)?;
            match action {
                LayerAction::Add {
                    name,
                } => project.add_layer(name)?,
                LayerAction::Select {
                    name,
                } => project.select_layer(&name)?,
                LayerAction::Duplicate {
                    name,
                    new_name,
                } => project.duplicate_layer(&name, new_name)?,
                LayerAction::Set {
                    name,
                    visible,
                    opacity,
                } => project.configure_layer(&name, visible, opacity)?,
                LayerAction::Reorder {
                    name,
                    index,
                } => project.reorder_layer(&name, index)?,
                LayerAction::Merge {
                    name,
                } => project.merge_layer(&name)?,
                LayerAction::Delete {
                    name,
                } => project.delete_layer(&name)?,
            }
            (path, project, false)
        }
        ProjectCommand::Frame {
            path,
            action,
        } => {
            let mut project = storage::load_project(&path)?;
            match action {
                FrameAction::Add {
                    name,
                    duplicate,
                    duration_ms,
                } => project.add_frame(name, duplicate, duration_ms)?,
                FrameAction::Select {
                    index,
                } => project.select_frame(index)?,
                FrameAction::Set {
                    index,
                    duration_ms,
                    pivot_x,
                    pivot_y,
                } => project.configure_frame(
                    index,
                    duration_ms,
                    pivot_x.zip(pivot_y).map(|(x, y)| Point {
                        x,
                        y,
                    }),
                )?,
                FrameAction::Reorder {
                    index,
                    to,
                } => project.reorder_frame(index, to)?,
                FrameAction::Delete {
                    index,
                } => project.delete_frame(index)?,
            }
            (path, project, false)
        }
        ProjectCommand::Tag {
            path,
            action,
        } => {
            let mut project = storage::load_project(&path)?;
            match action {
                TagAction::Set {
                    name,
                    frames,
                    direction,
                } => project.set_tag(
                    name,
                    Tag {
                        frames,
                        direction,
                    },
                )?,
                TagAction::Delete {
                    name,
                } => project.delete_tag(&name)?,
            }
            (path, project, false)
        }
    };
    project.validate()?;
    storage::save_project(&path, &project, create_new)?;
    message(
        json_output,
        format!("Saved project {}", path.display()),
        json!({"version":1,"ok":true,"file":path_value(&path)}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf8_path_response_contract_stays_a_json_string() {
        assert_eq!(
            path_value(Path::new("art/éclair.png")),
            json!("art/éclair.png")
        );
    }
    #[cfg(windows)]
    #[test]
    fn unpaired_windows_surrogate_is_reported_losslessly() {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};
        let units = [0x61, 0xD800, 0x2e, 0x70, 0x6e, 0x67];
        let path = PathBuf::from(OsString::from_wide(&units));
        let value = path_value(&path);
        assert_eq!(value["encoding"], "windows_wide");
        assert_eq!(value["units"], json!(units));
    }
}
