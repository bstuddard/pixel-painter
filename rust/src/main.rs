// SPDX-License-Identifier: GPL-3.0-only
//! Parse arguments, dispatch once, and map errors to process exit codes.

#![forbid(unsafe_code)]

mod app;
mod app_assets;
mod cli;
mod json_output;
mod terminal;

use clap::Parser;
use cli::Cli;
use painter::{Error, ErrorCode};
use serde_json::json;
use std::{
    io::{self, Write},
    process::ExitCode,
};

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().collect();
    // Before parsing, honor the option terminator and exclude the process name.
    let json_output = args
        .iter()
        .skip(1)
        .take_while(|argument| *argument != "--")
        .any(|argument| argument == "--json");
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(mut error) => {
            if error.exit_code() != 0 && !json_output {
                escape_parser_values(&mut error);
            }
            if error.exit_code() == 0 || !json_output {
                let _ = error.print();
            } else {
                report_error(
                    &Error::new(ErrorCode::InvalidArguments, error.to_string()),
                    true,
                );
            }
            return ExitCode::from(error.exit_code() as u8);
        }
    };
    // Once parsing succeeds, Clap is authoritative about flags versus values.
    let json_output = cli.json;
    match app::run(cli) {
        Ok(status) => status,
        Err(error) => {
            report_error(&error, json_output);
            ExitCode::FAILURE
        }
    }
}

/// Escape argument data before Clap adds its trusted layout and ANSI styling.
/// JSON diagnostics retain their original strings; help/version are unchanged.
fn escape_parser_values(error: &mut clap::Error) {
    use clap::error::{ContextKind, ContextValue};
    let escape = |text: &str| terminal::Escaped(text).to_string();
    let contexts: Vec<_> = error
        .context()
        .filter_map(|(kind, value)| {
            if kind == ContextKind::Usage {
                return None;
            }
            let value = match value {
                ContextValue::String(value) => ContextValue::String(escape(value)),
                ContextValue::Strings(values) => {
                    ContextValue::Strings(values.iter().map(|value| escape(value)).collect())
                }
                ContextValue::StyledStr(value) => {
                    ContextValue::StyledStr(escape(&value.to_string()).into())
                }
                ContextValue::StyledStrs(values) => ContextValue::StyledStrs(
                    values
                        .iter()
                        .map(|value| escape(&value.to_string()).into())
                        .collect(),
                ),
                _ => return None,
            };
            Some((kind, value))
        })
        .collect();
    for (kind, value) in contexts {
        error.insert(kind, value);
    }
}

fn report_error(error: &Error, json_output: bool) {
    let mut stderr = io::stderr().lock();
    if json_output {
        let _ = json_output::write(
            &mut stderr,
            &json!({"version":1,"ok":false,"error":error}),
            false,
        );
        let _ = writeln!(stderr);
    } else {
        let _ = writeln!(stderr, "error: {error}");
    }
}
