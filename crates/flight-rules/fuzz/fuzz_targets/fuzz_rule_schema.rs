// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for multi-rule schemas with hysteresis defaults.
//!
//! Exercises paths not covered by the single-rule fuzz targets:
//! - Multi-rule schemas (two rules compiled together)
//! - Hysteresis lookup during condition compilation
//!
//! Run with: `cargo +nightly fuzz run fuzz_rule_schema`

#![no_main]

use flight_rules::{Rule, RuleDefaults, RulesSchema};
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Split input in half: first half → condition, second half → action.
    let mid = input.len() / 2;
    let (cond_str, action_str) = input.split_at(mid);

    // Fixed hysteresis entry so the compiler exercises the hysteresis path.
    let mut hysteresis = HashMap::new();
    hysteresis.insert("speed".to_string(), 5.0_f32);

    // Multi-rule schema with hysteresis defaults — must never panic.
    let schema = RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![
            Rule {
                when: cond_str.to_string(),
                action: action_str.to_string(),
                do_action: action_str.to_string(),
            },
            Rule {
                when: action_str.to_string(),
                action: cond_str.to_string(),
                do_action: cond_str.to_string(),
            },
        ],
        defaults: Some(RuleDefaults {
            hysteresis: Some(hysteresis),
        }),
    };
    let _ = schema.validate();
    let _ = schema.compile();
});
