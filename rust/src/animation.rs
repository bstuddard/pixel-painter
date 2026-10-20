// SPDX-License-Identifier: GPL-3.0-only
//! Animation data shared by editable documents and engine-independent exports.

use crate::{Canvas, drawing::Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub name: String,
    pub duration_ms: u32,
    #[serde(default)]
    pub pivot: Option<Point>,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 128>")]
    pub cels: Vec<Canvas>,
}

/// Ordered frame indices; direction is a user-defined convention, not an engine type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tag {
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 256>")]
    pub frames: Vec<usize>,
    #[serde(default)]
    pub direction: Option<String>,
}
