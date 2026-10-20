// SPDX-License-Identifier: GPL-3.0-only
//! Stable machine-readable errors shared by the library and CLI.

use serde::{Serialize, Serializer};
use std::{fmt, sync::Arc};

/// Stable error categories. JSON uses the strings returned by [`Self::as_str`].
/// Match with a fallback arm to allow future categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
    InvalidArguments,
    InvalidInput,
    InvalidJson,
    InvalidPng,
    UnsupportedVersion,
    OutOfBounds,
    ResourceLimit,
    PaletteViolation,
    NotFound,
    AlreadyExists,
    PermissionDenied,
    IoError,
    PngError,
    SeamMismatch,
    MergeChangesComposite,
    IncompleteExport,
}

impl ErrorCode {
    /// Machine-readable spelling, independent of Rust variant names.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidArguments => "invalid_arguments",
            Self::InvalidInput => "invalid_input",
            Self::InvalidJson => "invalid_json",
            Self::InvalidPng => "invalid_png",
            Self::UnsupportedVersion => "unsupported_version",
            Self::OutOfBounds => "out_of_bounds",
            Self::ResourceLimit => "resource_limit",
            Self::PaletteViolation => "palette_violation",
            Self::NotFound => "not_found",
            Self::AlreadyExists => "already_exists",
            Self::PermissionDenied => "permission_denied",
            Self::IoError => "io_error",
            Self::PngError => "png_error",
            Self::SeamMismatch => "seam_mismatch",
            Self::MergeChangesComposite => "merge_changes_composite",
            Self::IncompleteExport => "incomplete_export",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// A stable category, readable explanation, and optional zero-based batch index.
#[derive(Clone, Debug, Serialize)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<usize>,
    // Share native causes so cloning and IO wrapping preserve the diagnostic chain.
    #[serde(skip)]
    source: Option<Arc<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            operation: None,
            source: None,
        }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }
    pub fn limit(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ResourceLimit, message)
    }

    fn from_source(
        code: ErrorCode,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            code,
            message: source.to_string(),
            operation: None,
            source: Some(Arc::new(source)),
        }
    }
    pub(crate) fn at(mut self, index: usize) -> Self {
        self.operation = Some(index);
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(index) = self.operation {
            write!(f, "operation {index}: ")?;
        }
        write!(f, "{}", crate::terminal::Escaped(&self.message))
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref().map(|source| source as _)
    }
}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        if let Some(error) = error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<Self>())
        {
            return error.clone();
        }
        let code = match error.kind() {
            std::io::ErrorKind::NotFound => ErrorCode::NotFound,
            std::io::ErrorKind::AlreadyExists => ErrorCode::AlreadyExists,
            std::io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
            _ => ErrorCode::IoError,
        };
        Self::from_source(code, error)
    }
}
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        if error.is_io() {
            return std::io::Error::from(error).into();
        }
        Self::from_source(ErrorCode::InvalidJson, error)
    }
}
impl From<png::DecodingError> for Error {
    fn from(error: png::DecodingError) -> Self {
        Self::from_source(ErrorCode::InvalidPng, error)
    }
}
impl From<png::EncodingError> for Error {
    fn from(error: png::EncodingError) -> Self {
        Self::from_source(ErrorCode::PngError, error)
    }
}
pub type Result<T> = std::result::Result<T, Error>;
