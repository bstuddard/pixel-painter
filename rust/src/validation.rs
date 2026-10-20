// SPDX-License-Identifier: GPL-3.0-only
//! Deterministic validation reports shared by style, animation, and pack checks.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Serialize)]
pub struct Issue {
    pub code: &'static str,
    pub severity: Severity,
    pub asset: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub version: u32,
    pub valid: bool,
    pub checked: usize,
    pub issues: Vec<Issue>,
    pub omitted: usize,
}
impl Default for Report {
    fn default() -> Self {
        Self {
            version: 1,
            valid: true,
            checked: 0,
            issues: Vec::new(),
            omitted: 0,
        }
    }
}
impl Report {
    pub fn issue(
        &mut self,
        severity: Severity,
        code: &'static str,
        asset: &str,
        message: impl Into<String>,
    ) {
        if severity == Severity::Error {
            self.valid = false;
        }
        // Aggregate checks report counts rather than allocating one issue per pixel.
        if self.issues.len() < 4096 {
            self.issues.push(Issue {
                code,
                severity,
                asset: asset.into(),
                message: message.into(),
            });
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
    pub fn error(&mut self, code: &'static str, asset: &str, message: impl Into<String>) {
        self.issue(Severity::Error, code, asset, message);
    }
    pub fn warning(&mut self, code: &'static str, asset: &str, message: impl Into<String>) {
        self.issue(Severity::Warning, code, asset, message);
    }
    pub fn extend(&mut self, other: Self) {
        self.valid &= other.valid;
        self.checked += other.checked;
        let available = 4096usize.saturating_sub(self.issues.len());
        self.omitted = self
            .omitted
            .saturating_add(other.omitted)
            .saturating_add(other.issues.len().saturating_sub(available));
        self.issues.extend(
            other
                .issues
                .into_iter()
                .take(4096usize.saturating_sub(self.issues.len())),
        );
    }
}
