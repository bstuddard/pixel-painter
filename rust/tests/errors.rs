// SPDX-License-Identifier: GPL-3.0-only
//! Typed library errors retain the CLI's stable JSON contract and native causes.

use painter::{Error, ErrorCode};
use serde_json::json;
use std::{error::Error as _, io};

#[test]
fn error_categories_keep_their_existing_json_spellings() {
    for (code, name) in [
        (ErrorCode::InvalidArguments, "invalid_arguments"),
        (ErrorCode::InvalidInput, "invalid_input"),
        (ErrorCode::InvalidJson, "invalid_json"),
        (ErrorCode::InvalidPng, "invalid_png"),
        (ErrorCode::UnsupportedVersion, "unsupported_version"),
        (ErrorCode::OutOfBounds, "out_of_bounds"),
        (ErrorCode::ResourceLimit, "resource_limit"),
        (ErrorCode::PaletteViolation, "palette_violation"),
        (ErrorCode::NotFound, "not_found"),
        (ErrorCode::AlreadyExists, "already_exists"),
        (ErrorCode::PermissionDenied, "permission_denied"),
        (ErrorCode::IoError, "io_error"),
        (ErrorCode::PngError, "png_error"),
        (ErrorCode::SeamMismatch, "seam_mismatch"),
        (ErrorCode::MergeChangesComposite, "merge_changes_composite"),
        (ErrorCode::IncompleteExport, "incomplete_export"),
    ] {
        let mut error = Error::new(code, "explanation");
        assert_eq!(code.as_str(), name);
        assert_eq!(code.to_string(), name);
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            json!({"code":name,"message":"explanation"})
        );
        error.operation = Some(3);
        assert_eq!(
            serde_json::to_value(error).unwrap(),
            json!({"code":name,"message":"explanation","operation":3})
        );
    }
}

#[test]
fn converted_errors_and_their_clones_retain_native_causes() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Error>();
    for (kind, code) in [
        (io::ErrorKind::NotFound, ErrorCode::NotFound),
        (io::ErrorKind::AlreadyExists, ErrorCode::AlreadyExists),
        (io::ErrorKind::PermissionDenied, ErrorCode::PermissionDenied),
        (io::ErrorKind::Other, ErrorCode::IoError),
    ] {
        let error = Error::from(io::Error::new(kind, "original cause"));
        for error in [error.clone(), error] {
            assert_eq!(error.code, code);
            let cause = error.source().unwrap().downcast_ref::<io::Error>().unwrap();
            assert_eq!(cause.kind(), kind);
            assert_eq!(cause.to_string(), "original cause");
            assert_eq!(
                serde_json::to_value(error).unwrap(),
                json!({"code":code,"message":"original cause"})
            );
        }
    }
    let native = serde_json::from_str::<serde_json::Value>("{\n broken").unwrap_err();
    let location = (native.line(), native.column());
    let error = Error::from(native);
    assert_eq!(error.code, ErrorCode::InvalidJson);
    let cause = error
        .source()
        .unwrap()
        .downcast_ref::<serde_json::Error>()
        .unwrap();
    assert_eq!((cause.line(), cause.column()), location);
    assert!(
        !serde_json::to_value(error)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("source")
    );
}

#[test]
fn png_failures_retain_codec_causes_without_changing_json_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid.png");
    std::fs::write(&path, b"invalid PNG signature").unwrap();
    let error = painter::storage::import_png(&path, false).unwrap_err();
    for error in [error.clone(), error] {
        assert_eq!(error.code, ErrorCode::InvalidPng);
        let cause = error
            .source()
            .unwrap()
            .downcast_ref::<png::DecodingError>()
            .unwrap();
        assert_eq!(error.message, cause.to_string());
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            json!({"code":"invalid_png","message":error.message})
        );
    }

    struct Unwritable;
    impl io::Write for Unwritable {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "write denied",
            ))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let native = png::Encoder::new(Unwritable, 1, 1)
        .write_header()
        .err()
        .unwrap();
    let error = Error::from(native);
    for error in [error.clone(), error] {
        assert_eq!(error.code, ErrorCode::PngError);
        let cause = error
            .source()
            .unwrap()
            .downcast_ref::<png::EncodingError>()
            .unwrap();
        let png::EncodingError::IoError(native) = cause else {
            panic!("expected original PNG writer failure");
        };
        assert_eq!(native.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(native.to_string(), "write denied");
        assert_eq!(
            serde_json::to_value(error).unwrap(),
            json!({"code":"png_error","message":"write denied"})
        );
    }
}
