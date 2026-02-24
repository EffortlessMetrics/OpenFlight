// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for Elite Dangerous journal and status JSON parsing.
//!
//! Run with: `cargo +nightly fuzz run fuzz_elite_journal`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_elite::protocol::{JournalEvent, StatusJson};

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz journal line parsing — must never panic
    let _ = flight_elite::protocol::parse_journal_line(input);

    // Fuzz StatusJson deserialization — must never panic
    let _ = serde_json::from_str::<StatusJson>(input);

    // Fuzz JournalEvent deserialization — must never panic
    let _ = serde_json::from_str::<JournalEvent>(input);
});
