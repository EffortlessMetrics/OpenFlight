// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the Rules DSL parser (conditions, actions, full schema).
//!
//! Run with: `cargo +nightly fuzz run fuzz_rules_parser`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_rules::{Rule, RulesSchema};

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz condition validation path — must never panic
    let schema = RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![Rule {
            when: input.to_string(),
            action: "led.panel('GEAR').on()".to_string(),
            do_action: "led.panel('GEAR').on()".to_string(),
        }],
        defaults: None,
    };
    let _ = schema.validate();

    // Fuzz action validation path — must never panic
    let schema2 = RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![Rule {
            when: "gear_down".to_string(),
            action: input.to_string(),
            do_action: input.to_string(),
        }],
        defaults: None,
    };
    let _ = schema2.validate();

    // Fuzz full compile path — must never panic
    let schema3 = RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![Rule {
            when: input.to_string(),
            action: input.to_string(),
            do_action: input.to_string(),
        }],
        defaults: None,
    };
    let _ = schema3.compile();
});
