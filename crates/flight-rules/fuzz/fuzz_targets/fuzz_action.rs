// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the Rules DSL action parser.
//!
//! Run with: `cargo +nightly fuzz run fuzz_action`

#![no_main]

use flight_rules::{Rule, RulesSchema};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz only the action parsing path — condition is a known-good constant
    let schema = RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![Rule {
            when: "gear_down".to_string(),
            action: input.to_string(),
            do_action: input.to_string(),
        }],
        defaults: None,
    };
    let _ = schema.validate();
    let _ = schema.compile();
});
