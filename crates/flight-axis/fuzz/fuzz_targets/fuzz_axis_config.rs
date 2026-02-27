// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the axis config parsing path.
//!
//! Exercises: JSON bytes → EngineConfig deserialization → AxisEngine creation → frame processing.
//! Must never panic regardless of input.
//!
//! Run with: `cargo +nightly fuzz run fuzz_axis_config`

#![no_main]

use flight_axis::{AxisEngine, AxisFrame, EngineConfig};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz the config parsing path: JSON → EngineConfig → AxisEngine → process
    if let Ok(config) = serde_json::from_str::<EngineConfig>(s) {
        let engine = AxisEngine::with_config("fuzz".to_string(), config);
        let mut frame = AxisFrame::new(0.5, 1_000_000);
        let _ = engine.process(&mut frame);
    }
});
