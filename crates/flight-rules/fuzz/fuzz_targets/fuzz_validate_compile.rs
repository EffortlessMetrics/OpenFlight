// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target asserting validate/compile agreement.
//!
//! For any arbitrary condition+action pair, if `validate()` succeeds then
//! `compile()` must also succeed — and vice-versa. A divergence is a bug.
//!
//! Run with: `cargo +nightly fuzz run fuzz_validate_compile`

#![no_main]

use flight_rules::{Rule, RulesSchema};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    let mid = input.floor_char_boundary(input.len() / 2);
    let (cond, action) = input.split_at(mid);

    let schema = RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![Rule {
            when: cond.to_string(),
            action: action.to_string(),
            do_action: action.to_string(),
        }],
        defaults: None,
    };

    let v = schema.validate();
    let c = schema.compile();

    // validate() and compile() must agree: both Ok or both Err.
    assert_eq!(
        v.is_ok(),
        c.is_ok(),
        "validate/compile disagreement for when={:?} action={:?}: validate={:?} compile={:?}",
        cond,
        action,
        v,
        c,
    );
});
