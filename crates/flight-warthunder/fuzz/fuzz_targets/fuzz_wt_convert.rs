// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fuzz target for War Thunder telemetry conversion logic.
//!
//! Parses arbitrary JSON as `WtIndicators`, then exercises the full
//! `convert_indicators` pipeline — unit conversions, validated types,
//! validity flag derivation — with no known-safe inputs.
//!
//! Run with: `cargo +nightly fuzz run fuzz_wt_convert`

#![no_main]

use flight_warthunder::{WarThunderAdapter, WarThunderConfig, protocol::WtIndicators};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Only enter the conversion path when deserialization succeeds so the
    // fuzzer can focus corpus entries on inputs that exercise both paths.
    if let Ok(indicators) = serde_json::from_str::<WtIndicators>(input) {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        let _ = adapter.convert_indicators(&indicators);
    }
});
