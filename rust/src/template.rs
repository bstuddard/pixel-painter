// SPDX-License-Identifier: GPL-3.0-only
//! Reusable data-driven recipes. Stamp operations assemble generic canvas parts.

use crate::{Canvas, Error, ErrorCode, Result, operations::Batch};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub batch: Batch,
}

impl Template {
    pub fn render(&self, seed: Option<u64>) -> Result<Canvas> {
        if self.version != 1 {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "template version must be 1",
            ));
        }
        let mut canvas = Canvas::new(self.width, self.height)?;
        self.batch
            .apply_with_seed(&mut canvas, seed.unwrap_or(self.batch.seed))?;
        Ok(canvas)
    }
}
