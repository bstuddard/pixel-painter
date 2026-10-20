// SPDX-License-Identifier: GPL-3.0-only
//! Fixed-grid tiles, explicit adjacency rules, and exact seam validation.

use crate::{
    Canvas, Error, ErrorCode, Result,
    atlas::{Atlas, Options, RenderedFrame},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    Right,
    Down,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub from: u32,
    pub to: u32,
    pub edge: Edge,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub version: u32,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 16_384>")]
    pub connections: Vec<Connection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileMetadata {
    pub tile_width: u32,
    pub tile_height: u32,
    pub ids: Vec<u32>,
    pub connections: Vec<Connection>,
}

#[derive(Debug, Serialize)]
pub struct Seam {
    pub from: u32,
    pub to: u32,
    pub edge: Edge,
    pub mismatched_pixels: usize,
}

pub struct Tileset {
    tiles: Vec<Canvas>,
    width: u32,
    height: u32,
}

impl Tileset {
    /// Read a grid with explicit source margin/spacing; partial tiles are rejected.
    pub fn slice(
        source: &Canvas,
        width: u32,
        height: u32,
        margin: u32,
        spacing: u32,
    ) -> Result<Self> {
        if width == 0 || height == 0 || margin > 1024 || spacing > 1024 {
            return Err(Error::invalid(
                "tiles need positive dimensions and margin/spacing <=1024",
            ));
        }
        let extent = |size: u32, tile: u32| -> Result<u32> {
            let inner = u64::from(size)
                .checked_sub(2 * u64::from(margin))
                .ok_or_else(|| Error::invalid("tile margins exceed source dimensions"))?;
            if inner == 0
                || (inner + u64::from(spacing)) % (u64::from(tile) + u64::from(spacing)) != 0
            {
                return Err(Error::invalid(
                    "source dimensions must contain complete tiles",
                ));
            }
            Ok(((inner + u64::from(spacing)) / (u64::from(tile) + u64::from(spacing))) as u32)
        };
        let columns = extent(source.width(), width)?;
        let rows = extent(source.height(), height)?;
        if u64::from(columns) * u64::from(rows) > 4096 {
            return Err(Error::limit("tileset exceeds 4096 tiles"));
        }
        let mut tiles = Vec::new();
        for row in 0..rows {
            for column in 0..columns {
                tiles.push(source.region(
                    margin + column * (width + spacing),
                    margin + row * (height + spacing),
                    width,
                    height,
                )?);
            }
        }
        Ok(Self {
            tiles,
            width,
            height,
        })
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn seams(&self, rules: &Rules) -> Result<Vec<Seam>> {
        if rules.version != 1 {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "tile rules version must be 1",
            ));
        }
        if rules.connections.len() > 16384 {
            return Err(Error::limit("tile rules exceed 16384 connections"));
        }
        // Bound edge comparisons as well as rule count. A narrow, tall tile can
        // otherwise request billions of comparisons using a small rules file.
        let mut work = 0u64;
        for rule in &rules.connections {
            if rule.from as usize >= self.tiles.len() || rule.to as usize >= self.tiles.len() {
                return Err(Error::invalid("connection references an unknown tile"));
            }
            work += u64::from(match rule.edge {
                Edge::Right => self.height,
                Edge::Down => self.width,
            });
        }
        if work > crate::drawing::MAX_WORK {
            return Err(Error::limit("tile seams exceed comparison budget"));
        }
        let mut seams = Vec::with_capacity(rules.connections.len());
        for rule in &rules.connections {
            // Every reference was validated before any edge comparisons begin.
            let from = &self.tiles[rule.from as usize];
            let to = &self.tiles[rule.to as usize];
            let mismatched_pixels = match rule.edge {
                Edge::Right => (0..self.height)
                    .filter(|y| from.pixel(self.width - 1, *y) != to.pixel(0, *y))
                    .count(),
                Edge::Down => (0..self.width)
                    .filter(|x| from.pixel(*x, self.height - 1) != to.pixel(*x, 0))
                    .count(),
            };
            seams.push(Seam {
                from: rule.from,
                to: rule.to,
                edge: rule.edge,
                mismatched_pixels,
            });
        }
        Ok(seams)
    }

    pub fn atlas(&self, options: Options, rules: Option<&Rules>) -> Result<Atlas> {
        if let Some(rules) = rules {
            self.seams(rules)?;
        }
        let frames = self
            .tiles
            .iter()
            .enumerate()
            .map(|(index, canvas)| RenderedFrame {
                name: format!("tile-{index}"),
                canvas: canvas.clone(),
                duration_ms: 100,
                pivot: None,
            })
            .collect();
        let mut atlas = Atlas::pack(frames, options)?;
        // Tile sequences are collections, not temporal animations.
        atlas.set_tileset(TileMetadata {
            tile_width: self.width,
            tile_height: self.height,
            ids: (0..self.tiles.len() as u32).collect(),
            connections: rules.map(|r| r.connections.clone()).unwrap_or_default(),
        });
        Ok(atlas)
    }
}
