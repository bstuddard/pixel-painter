// SPDX-License-Identifier: GPL-3.0-only

mod canvas;
mod cli;
mod pixel;
mod storage;

use canvas::Canvas;
use clap::Parser;
use cli::{Cli, Commands};
use pixel::Pixel;
use std::error::Error;
use std::process::ExitCode;

/// Parse arguments and report command errors with a failure exit code.
fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Dispatch the selected command, loading or saving its canvas as needed.
fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Commands::New {
            width,
            height,
        } => {
            let canvas = Canvas::new(width, height)?;
            storage::save(&cli.file, &canvas, true)?;
            println!("Created {canvas} at {}", cli.file.display());
            canvas.print_pixels();
        }
        Commands::SetPixel {
            x,
            y,
            r,
            g,
            b,
            a,
        } => {
            let mut canvas = storage::load(&cli.file)?;
            canvas.set_pixel(
                x,
                y,
                Pixel {
                    r,
                    g,
                    b,
                    a,
                },
            )?;
            storage::save(&cli.file, &canvas, false)?;
            println!("Updated ({x}, {y}) in {}", cli.file.display());
        }
        Commands::Print => {
            let canvas = storage::load(&cli.file)?;
            println!("{canvas}");
            canvas.print_pixels();
        }
        Commands::View => {
            let canvas = storage::load(&cli.file)?;
            println!("{canvas}");
            canvas.view()?;
        }
        Commands::Export {
            output,
        } => {
            let canvas = storage::load(&cli.file)?;
            storage::export_png(&output, &canvas)?;
            println!("Exported {canvas} to {}", output.display());
        }
    }
    Ok(())
}
