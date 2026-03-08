// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Scripting mode for the CLI (REQ-882).
//!
//! Reads commands line-by-line from a file or stdin, parses them into
//! [`BatchOp`]s, and executes them. Supports comments (`#`), blank lines,
//! and a `--dry-run` mode that validates without executing.

use crate::batch::{self, BatchOp, BatchResult, OpResult};
use std::io::BufRead;

/// Exit codes returned by [`run_script`].
pub mod exit_code {
    /// Every command succeeded.
    pub const SUCCESS: i32 = 0;
    /// At least one command failed during execution.
    pub const FAILURE: i32 = 1;
    /// A line could not be parsed into a valid command.
    pub const PARSE_ERROR: i32 = 2;
}

/// Outcome of running a script.
#[derive(Debug, Clone)]
pub struct ScriptResult {
    /// Exit code suitable for `std::process::exit`.
    pub exit_code: i32,
    /// Per-operation results (empty when a parse error occurs before execution).
    pub batch_result: Option<BatchResult>,
    /// Parse errors collected during parsing, if any.
    pub parse_errors: Vec<ParseError>,
}

/// A parse error on a specific line.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// 1-based line number.
    pub line: usize,
    /// The problematic input.
    pub text: String,
    /// Human-readable reason.
    pub reason: String,
}

/// Reads commands from `reader`, parses, and optionally executes them.
///
/// When `dry_run` is `true` the commands are parsed and validated but not
/// executed; the returned [`ScriptResult`] will contain an empty
/// `batch_result`.
pub fn run_script(reader: impl BufRead, dry_run: bool) -> ScriptResult {
    let mut ops: Vec<BatchOp> = Vec::new();
    let mut parse_errors: Vec<ParseError> = Vec::new();

    for (idx, line) in reader.lines().enumerate() {
        let line_no = idx + 1;
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                parse_errors.push(ParseError {
                    line: line_no,
                    text: String::new(),
                    reason: format!("I/O error: {e}"),
                });
                continue;
            }
        };

        let trimmed = line.trim();

        // Skip blank lines and comments.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        match parse_command(trimmed) {
            Ok(op) => ops.push(op),
            Err(reason) => {
                parse_errors.push(ParseError {
                    line: line_no,
                    text: trimmed.to_owned(),
                    reason,
                });
            }
        }
    }

    // If there were parse errors, report immediately without executing.
    if !parse_errors.is_empty() {
        return ScriptResult {
            exit_code: exit_code::PARSE_ERROR,
            batch_result: None,
            parse_errors,
        };
    }

    if dry_run {
        return ScriptResult {
            exit_code: exit_code::SUCCESS,
            batch_result: None,
            parse_errors,
        };
    }

    let batch_result = batch::execute_batch(&ops);
    let exit_code = if batch_result.all_succeeded() {
        exit_code::SUCCESS
    } else {
        exit_code::FAILURE
    };

    ScriptResult {
        exit_code,
        batch_result: Some(batch_result),
        parse_errors,
    }
}

/// Parse a single non-blank, non-comment line into a [`BatchOp`].
fn parse_command(line: &str) -> Result<BatchOp, String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Err("empty command".into());
    }

    match parts[0] {
        "set-deadzone" => {
            if parts.len() != 3 {
                return Err("usage: set-deadzone <axis> <value>".into());
            }
            let value: f64 = parts[2]
                .parse()
                .map_err(|_| format!("invalid number: {}", parts[2]))?;
            Ok(BatchOp::SetDeadzone {
                axis: parts[1].into(),
                value,
            })
        }
        "set-expo" => {
            if parts.len() != 3 {
                return Err("usage: set-expo <axis> <value>".into());
            }
            let value: f64 = parts[2]
                .parse()
                .map_err(|_| format!("invalid number: {}", parts[2]))?;
            Ok(BatchOp::SetExpo {
                axis: parts[1].into(),
                value,
            })
        }
        "set-curve" => {
            if parts.len() != 3 {
                return Err("usage: set-curve <axis> <curve_name>".into());
            }
            Ok(BatchOp::SetCurve {
                axis: parts[1].into(),
                curve: parts[2].into(),
            })
        }
        "enable-axis" => {
            if parts.len() != 2 {
                return Err("usage: enable-axis <axis>".into());
            }
            Ok(BatchOp::EnableAxis {
                axis: parts[1].into(),
            })
        }
        "disable-axis" => {
            if parts.len() != 2 {
                return Err("usage: disable-axis <axis>".into());
            }
            Ok(BatchOp::DisableAxis {
                axis: parts[1].into(),
            })
        }
        other => Err(format!("unknown command: {other}")),
    }
}

/// Reads commands from `reader` and executes them via [`ScriptRunner`].
pub struct ScriptRunner {
    pub dry_run: bool,
}

impl ScriptRunner {
    pub fn new(dry_run: bool) -> Self {
        Self { dry_run }
    }

    /// Run the script from the given reader.
    pub fn run(&self, reader: impl BufRead) -> ScriptResult {
        run_script(reader, self.dry_run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parse_comment_lines_ignored() {
        let input = "# this is a comment\n# another comment\nenable-axis roll\n";
        let result = run_script(Cursor::new(input), false);
        assert_eq!(result.exit_code, exit_code::SUCCESS);
        let batch = result.batch_result.unwrap();
        assert_eq!(batch.results.len(), 1);
    }

    #[test]
    fn parse_blank_lines_ignored() {
        let input = "\n\nenable-axis roll\n\n";
        let result = run_script(Cursor::new(input), false);
        assert_eq!(result.exit_code, exit_code::SUCCESS);
        let batch = result.batch_result.unwrap();
        assert_eq!(batch.results.len(), 1);
    }

    #[test]
    fn parse_valid_commands() {
        let input = "set-deadzone roll 0.05\nset-expo pitch 0.3\nenable-axis yaw\n";
        let result = run_script(Cursor::new(input), false);
        assert_eq!(result.exit_code, exit_code::SUCCESS);
        let batch = result.batch_result.unwrap();
        assert_eq!(batch.results.len(), 3);
        assert!(batch.all_succeeded());
    }

    #[test]
    fn invalid_command_returns_parse_error() {
        let input = "not-a-command foo\n";
        let result = run_script(Cursor::new(input), false);
        assert_eq!(result.exit_code, exit_code::PARSE_ERROR);
        assert!(result.batch_result.is_none());
        assert_eq!(result.parse_errors.len(), 1);
        assert_eq!(result.parse_errors[0].line, 1);
    }

    #[test]
    fn dry_run_does_not_execute() {
        let input = "set-deadzone roll 0.05\nenable-axis pitch\n";
        let result = run_script(Cursor::new(input), true);
        assert_eq!(result.exit_code, exit_code::SUCCESS);
        // No batch result when dry-running.
        assert!(result.batch_result.is_none());
        assert!(result.parse_errors.is_empty());
    }

    #[test]
    fn snapshot_script_result_success() {
        let input = "set-deadzone roll 0.05\nset-expo pitch 0.3\nenable-axis yaw\n";
        let result = run_script(Cursor::new(input), false);
        insta::assert_snapshot!("script_result_success", format!("{:#?}", result));
    }

    #[test]
    fn snapshot_script_parse_errors() {
        let input = "not-a-command foo\nset-deadzone roll bad_number\n";
        let result = run_script(Cursor::new(input), false);
        insta::assert_snapshot!("script_parse_errors", format!("{:#?}", result));
    }
}
