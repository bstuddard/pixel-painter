// SPDX-License-Identifier: GPL-3.0-only
//! Binary selections captured before a batch; mask alpha is a selection, not coverage.

use crate::{Canvas, Error, Pixel, Result, drawing::Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mask {
    /// Select pixels whose destination alpha was nonzero before the batch.
    // A struct variant makes Serde reject extra fields; unit variants ignore them.
    Alpha {},
    /// Select nonzero alpha in a same-size canvas; RGB and partial alpha are ignored.
    Canvas {
        canvas: Canvas,
    },
}

pub(crate) struct Selection {
    selected: Vec<bool>,
    lock_alpha: bool,
}
impl Selection {
    pub(crate) fn capture(
        canvas: &Canvas,
        mask: Option<&Mask>,
        lock_alpha: bool,
    ) -> Result<Option<Self>> {
        if mask.is_none() && !lock_alpha {
            return Ok(None);
        }
        let source = match mask {
            Some(Mask::Canvas {
                canvas: mask,
            }) => {
                if (mask.width(), mask.height()) != (canvas.width(), canvas.height()) {
                    return Err(Error::invalid(
                        "mask dimensions must match the edited canvas",
                    ));
                }
                Some(mask)
            }
            Some(Mask::Alpha {}) => Some(canvas),
            None => None,
        };
        let selected = canvas
            .pixels()
            .iter()
            .enumerate()
            .map(|(index, pixel)| {
                source.is_none_or(|source| source.pixels()[index].a != 0)
                    && (!lock_alpha || pixel.a != 0)
            })
            .collect();
        Ok(Some(Self {
            selected,
            lock_alpha,
        }))
    }

    /// Reapply the captured selection after every edit, before the next operation reads it.
    pub(crate) fn constrain(
        &self,
        edited: &mut Canvas,
        original: &Canvas,
        previous: Option<&Canvas>,
    ) {
        for (index, ((pixel, original), selected)) in edited
            .pixels_mut()
            .iter_mut()
            .zip(original.pixels())
            .zip(&self.selected)
            .enumerate()
        {
            if !selected {
                *pixel = *original;
            } else if self.lock_alpha {
                // Transparent brush texels and erase operations have no paint
                // to contribute while alpha is locked. Retain the prior edit.
                if pixel.a == 0 {
                    *pixel = previous
                        .expect("alpha lock captures previous pixels")
                        .pixels()[index];
                }
                pixel.a = original.a;
            }
        }
    }

    /// A masked flood cannot traverse an unselected pixel to reach another island.
    pub(crate) fn fill(&self, canvas: &mut Canvas, point: Point, color: Pixel) -> Result<()> {
        crate::drawing::flood_fill_masked(canvas, point, color, Some(&self.selected))
    }
}
