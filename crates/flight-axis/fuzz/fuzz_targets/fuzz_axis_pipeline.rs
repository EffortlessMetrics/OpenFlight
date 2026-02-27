// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the RT axis pipeline processing path.
//!
//! Exercises: arbitrary f32 input + u64 timestamp → default pipeline → output bounds check.
//! Ensures the RT pipeline never panics or produces non-finite output.
//!
//! Run with: `cargo +nightly fuzz run fuzz_axis_pipeline`

#![no_main]

use flight_axis::{AxisFrame, PipelineBuilder};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 12 {
        return;
    }

    let input = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let ts = u64::from_le_bytes([
        data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
    ]);

    // Build a representative default pipeline (deadzone + curve)
    let Ok(pipeline) = PipelineBuilder::new()
        .deadzone(0.03)
        .curve(0.3)
        .unwrap()
        .compile()
    else {
        return;
    };

    let mut state = pipeline.create_state();
    let mut frame = AxisFrame::new(input, ts);
    frame.out = frame.in_raw;
    pipeline.process(&mut frame, &mut state);

    // RT pipeline must never produce non-finite output
    assert!(!frame.out.is_nan(), "pipeline produced NaN for input {input}");
    assert!(!frame.out.is_infinite(), "pipeline produced Inf for input {input}");
});
