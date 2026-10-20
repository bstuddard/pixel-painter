// SPDX-License-Identifier: GPL-3.0-only
//! CLI argument definitions. Editing and persistence stay in the library.

use clap::{Args, Parser, Subcommand};
use painter::{drawing::Axis, palette::Policy};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    bin_name = "painter",
    about = "Deterministic pixel-art drawing, layered animation, and asset exports"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = "canvas.json")]
    pub file: PathBuf,
    /// Emit versioned JSON on stdout and structured errors on stderr.
    #[arg(long, global = true)]
    pub json: bool,
    /// Override the selected project frame (zero-based).
    #[arg(long, global = true)]
    pub frame: Option<usize>,
    /// Override the selected project layer by name.
    #[arg(long, global = true)]
    pub layer: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a transparent legacy-compatible canvas, refusing overwrites.
    New {
        #[arg(long, default_value_t = 32)]
        width: u32,
        #[arg(long, default_value_t = 32)]
        height: u32,
    },
    /// Replace one exact RGBA pixel and save.
    SetPixel {
        #[arg(long)]
        x: u32,
        #[arg(long)]
        y: u32,
        #[arg(long)]
        r: u8,
        #[arg(long)]
        g: u8,
        #[arg(long)]
        b: u8,
        #[arg(long, default_value_t = 255)]
        a: u8,
    },
    Print,
    View,
    /// Export a native-size PNG, refusing overwrites.
    Export {
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Write an enlarged nearest-neighbor preview.
    Render {
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value_t = 8)]
        scale: u32,
        /// Atomically replace an existing preview (never the input artwork).
        #[arg(long)]
        replace: bool,
    },
    /// Report dimensions, colors, transparency, and occupied bounds.
    Inspect {
        #[arg(long)]
        palette: Option<PathBuf>,
    },
    /// Apply a version-1 operation file as one transaction.
    Batch {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        palette: Option<PathBuf>,
        #[arg(long, value_enum)]
        policy: Option<CliPolicy>,
    },
    Draw {
        #[arg(long)]
        palette: Option<PathBuf>,
        #[command(subcommand)]
        shape: Draw,
    },
    Transform {
        #[command(subcommand)]
        transform: Transform,
    },
    /// Copy a region to a new reusable canvas component.
    Extract {
        #[command(flatten)]
        bounds: Bounds,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Paste a reusable canvas component into the selected canvas/cel.
    Stamp {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        x: i32,
        #[arg(long, allow_hyphen_values = true)]
        y: i32,
        #[arg(long)]
        over: bool,
    },
    /// Import PNG to a new flat canvas. Existing artwork is never overwritten.
    Import {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        reduce_16: bool,
        #[arg(long)]
        palette: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "unrestricted")]
        policy: CliPolicy,
    },
    /// Render a data-driven template into a new flat canvas.
    Generate {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Save an independent checkpoint without changing the working artwork.
    Snapshot {
        #[arg(short, long)]
        output: PathBuf,
    },
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    Tiles {
        #[arg(long)]
        tile_width: u32,
        #[arg(long)]
        tile_height: u32,
        #[arg(long, default_value_t = 0)]
        source_margin: u32,
        #[arg(long, default_value_t = 0)]
        source_spacing: u32,
        #[command(subcommand)]
        command: TileCommand,
    },
}

#[derive(Clone, Copy, Args)]
pub struct Bounds {
    #[arg(long, allow_hyphen_values = true)]
    pub x: i32,
    #[arg(long, allow_hyphen_values = true)]
    pub y: i32,
    #[arg(long)]
    pub width: u32,
    #[arg(long)]
    pub height: u32,
}

#[derive(Subcommand)]
pub enum Draw {
    Line {
        #[arg(long, allow_hyphen_values = true)]
        x1: i32,
        #[arg(long, allow_hyphen_values = true)]
        y1: i32,
        #[arg(long, allow_hyphen_values = true)]
        x2: i32,
        #[arg(long, allow_hyphen_values = true)]
        y2: i32,
        #[arg(long)]
        color: String,
        #[arg(long)]
        over: bool,
    },
    Rect {
        #[command(flatten)]
        bounds: Bounds,
        #[arg(long)]
        color: String,
        #[arg(long)]
        fill: bool,
        #[arg(long)]
        over: bool,
    },
    Ellipse {
        #[command(flatten)]
        bounds: Bounds,
        #[arg(long)]
        color: String,
        #[arg(long)]
        fill: bool,
        #[arg(long)]
        over: bool,
    },
    /// Supply comma-separated x,y pairs, e.g. --points 1,1,8,1,4,8.
    Polygon {
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        points: Vec<i32>,
        #[arg(long)]
        color: String,
        #[arg(long)]
        fill: bool,
        #[arg(long)]
        over: bool,
    },
    Fill {
        #[arg(long)]
        x: i32,
        #[arg(long)]
        y: i32,
        #[arg(long)]
        color: String,
    },
    Erase {
        #[command(flatten)]
        bounds: Bounds,
    },
    Clear {
        #[arg(long)]
        color: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum Transform {
    Flip {
        #[arg(value_enum)]
        axis: CliAxis,
    },
    Rotate {
        #[arg(allow_hyphen_values = true)]
        turns: i32,
    },
    Move {
        #[arg(long, allow_hyphen_values = true)]
        dx: i32,
        #[arg(long, allow_hyphen_values = true)]
        dy: i32,
    },
    Crop {
        #[command(flatten)]
        bounds: Bounds,
    },
    Resize {
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
    },
}
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum CliAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum CliPolicy {
    Unrestricted,
    Strict,
    Nearest,
}
impl From<CliPolicy> for Policy {
    fn from(value: CliPolicy) -> Self {
        match value {
            CliPolicy::Unrestricted => Self::Unrestricted,
            CliPolicy::Strict => Self::Strict,
            CliPolicy::Nearest => Self::Nearest,
        }
    }
}
impl From<CliAxis> for Axis {
    fn from(value: CliAxis) -> Self {
        match value {
            CliAxis::Horizontal => Self::Horizontal,
            CliAxis::Vertical => Self::Vertical,
        }
    }
}

#[derive(Args)]
pub struct Packing {
    /// New output directory; metadata.json marks a complete bundle.
    #[arg(short, long)]
    pub output: PathBuf,
    #[arg(long)]
    pub columns: Option<u32>,
    #[arg(long, default_value_t = 0)]
    pub padding: u32,
    #[arg(long, default_value_t = 0)]
    pub margin: u32,
    #[arg(long, default_value_t = 4)]
    pub scale: u32,
}

#[derive(Subcommand)]
pub enum ProjectCommand {
    New {
        path: PathBuf,
        #[arg(long, default_value_t = 32)]
        width: u32,
        #[arg(long, default_value_t = 32)]
        height: u32,
    },
    Migrate {
        input: PathBuf,
        output: PathBuf,
    },
    Show {
        path: PathBuf,
    },
    Export {
        path: PathBuf,
        #[command(flatten)]
        packing: Packing,
    },
    Resize {
        path: PathBuf,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
    },
    Layer {
        path: PathBuf,
        #[command(subcommand)]
        action: LayerAction,
    },
    Frame {
        path: PathBuf,
        #[command(subcommand)]
        action: FrameAction,
    },
    Tag {
        path: PathBuf,
        #[command(subcommand)]
        action: TagAction,
    },
}

#[derive(Subcommand)]
pub enum LayerAction {
    #[command(alias = "create")]
    Add {
        name: String,
    },
    Select {
        name: String,
    },
    Duplicate {
        name: String,
        new_name: String,
    },
    Set {
        name: String,
        #[arg(long)]
        visible: Option<bool>,
        #[arg(long)]
        opacity: Option<u8>,
    },
    Reorder {
        name: String,
        index: usize,
    },
    Merge {
        name: String,
    },
    Delete {
        name: String,
    },
}
#[derive(Subcommand)]
pub enum FrameAction {
    #[command(alias = "create")]
    Add {
        name: String,
        #[arg(long)]
        duplicate: Option<usize>,
        #[arg(long, default_value_t = 100)]
        duration_ms: u32,
    },
    Select {
        index: usize,
    },
    Set {
        index: usize,
        #[arg(long)]
        duration_ms: Option<u32>,
        #[arg(long, requires = "pivot_y", allow_hyphen_values = true)]
        pivot_x: Option<i32>,
        #[arg(long, requires = "pivot_x", allow_hyphen_values = true)]
        pivot_y: Option<i32>,
    },
    Reorder {
        index: usize,
        to: usize,
    },
    Delete {
        index: usize,
    },
}
#[derive(Subcommand)]
pub enum TagAction {
    Set {
        name: String,
        #[arg(long, value_delimiter = ',', required = true)]
        frames: Vec<usize>,
        #[arg(long)]
        direction: Option<String>,
    },
    Delete {
        name: String,
    },
}
#[derive(Subcommand)]
pub enum TileCommand {
    Export {
        #[command(flatten)]
        packing: Packing,
        #[arg(long)]
        rules: Option<PathBuf>,
    },
    Seams {
        #[arg(long)]
        rules: PathBuf,
    },
}
