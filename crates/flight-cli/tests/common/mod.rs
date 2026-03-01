// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Shared test helpers for flight-cli integration tests

use serde_json::Value;

/// Build an `assert_cmd::Command` pointing at the `flightctl` binary.
pub fn cli() -> assert_cmd::Command {
    assert_cmd::Command::new(assert_cmd::cargo_bin!("flightctl"))
}

/// Find the first JSON object line in `text` and parse it, or panic.
pub fn parse_json_from(text: &str) -> Value {
    text.lines()
        .find(|l| l.trim().starts_with('{'))
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_else(|| panic!("No valid JSON line found in:\n{}", text))
}

/// Try to find and parse the first JSON object line in `text`.
#[allow(dead_code)]
pub fn try_parse_json_from(text: &str) -> Option<Value> {
    text.lines()
        .find(|l| l.trim().starts_with('{'))
        .and_then(|l| serde_json::from_str(l).ok())
}
