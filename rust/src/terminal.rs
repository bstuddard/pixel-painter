// SPDX-License-Identifier: GPL-3.0-only
//! Escape untrusted text in status lines and diagnostics, preserving readable Unicode.
//! Shared privately by the library and binary; serialized data is never changed.

use std::fmt;

pub(crate) struct Escaped<'a>(pub(crate) &'a str);

impl fmt::Display for Escaped<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for character in self.0.chars() {
            if character.is_control() {
                write!(f, "{}", character.escape_default())?;
            } else {
                write!(f, "{character}")?;
            }
        }
        Ok(())
    }
}
