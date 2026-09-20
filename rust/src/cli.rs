// SPDX-License-Identifier: GPL-3.0-only

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Create, edit, and inspect an RGBA pixel canvas.
#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// Canvas file shared between commands.
    #[arg(long, global = true, default_value = "canvas.json")]
    pub file: PathBuf,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create and print a transparent canvas (up to 16,777,216 pixels).
    New {
        /// Width in pixels.
        #[arg(long, default_value_t = 32)]
        width: u32,

        /// Height in pixels.
        #[arg(long, default_value_t = 32)]
        height: u32,
    },
    /// Change one pixel and save the canvas.
    SetPixel {
        /// Column, starting at zero on the left.
        #[arg(long)]
        x: u32,

        /// Row, starting at zero at the top.
        #[arg(long)]
        y: u32,

        /// Red channel (0-255).
        #[arg(long)]
        r: u8,

        /// Green channel (0-255).
        #[arg(long)]
        g: u8,

        /// Blue channel (0-255).
        #[arg(long)]
        b: u8,

        /// Alpha: 0 is transparent; 255 is opaque.
        #[arg(long, default_value_t = 255)]
        a: u8,
    },
    /// Print dimensions and RGBA values in numbered rows.
    Print,
    /// Show colored pixels over a transparency checkerboard in a true-color terminal.
    View,
    /// Export the canvas as a PNG with its original size and transparency.
    Export {
        /// Destination PNG path; must not already exist.
        #[arg(short, long)]
        output: PathBuf,
    },
}
