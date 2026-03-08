// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fuzz target for the PSX TCP message parser.
//!
//! Exercises `parse_psx_line` and `PsxTelemetry::apply` with arbitrary
//! byte sequences interpreted as UTF-8 text.
//!
//! Run with: `cargo +nightly fuzz run fuzz_psx_line`

#![no_main]

use flight_psx::{parse_psx_line, PsxTelemetry, PsxVariable};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // PSX protocol is text-based; skip invalid UTF-8 gracefully
    let Ok(line) = std::str::from_utf8(data) else {
        return;
    };

    // parse_psx_line must never panic on any string input
    if let Ok((var, val)) = parse_psx_line(line) {
        // Value must be a finite f64 (parsed from text)
        assert!(val.is_finite(), "parsed non-finite value: {val}");

        // PsxVariable::from_id round-trip for known variables
        if let Some(id) = var.id() {
            assert_eq!(PsxVariable::from_id(id), var);
        }

        // PsxTelemetry::apply must never panic
        let mut telemetry = PsxTelemetry::default();
        telemetry.apply(&var, val);
    }
});
