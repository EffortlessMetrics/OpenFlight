// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the calibration → EMA smoothing → rate-limit → trim pipeline.
//!
//! Exercises: arbitrary raw u16 + parameter bytes → full pre-pipeline processing →
//! output bounds check. Ensures this chain never panics and always produces output
//! in `[-1.0, 1.0]`.
//!
//! Run with: `cargo +nightly fuzz run fuzz_cal_smooth_trim`

#![no_main]

use flight_axis::{
    calibration::AxisCalibration, rate_limit::AxisRateLimiter, smoothing::EmaFilter, trim::AxisTrim,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 6 {
        return;
    }

    let raw_u16 = u16::from_le_bytes([data[0], data[1]]);
    // Clamp alpha to [0.01, 1.0] — EmaFilter panics outside [0.0, 1.0].
    let alpha = (data[2] as f32 / 255.0).max(0.01);
    // max_rate: 0.0 (unlimited) .. 1.0 (full range per tick)
    let max_rate = data[3] as f32 / 255.0;
    // trim_offset: approximately [-1.0, 1.0]; AxisTrim::set_offset clamps to ±max_range.
    let trim_offset = (data[4] as f32 / 127.5) - 1.0;

    // Calibration: full 16-bit range.
    let cal = AxisCalibration::default_full_range();
    let normalized = cal.normalize(raw_u16);

    // EMA smoothing.
    let mut ema = EmaFilter::new(alpha);
    let smoothed = ema.apply(normalized);

    // Rate limiter.
    let mut rate = AxisRateLimiter::new(max_rate);
    let rate_limited = rate.apply(smoothed);

    // Trim.
    let mut trim = AxisTrim::default();
    trim.set_offset(trim_offset);
    let output = trim.apply(rate_limited);

    // Invariant: output must always be in [-1.0, 1.0].
    assert!(
        output >= -1.0 && output <= 1.0,
        "Pipeline output {output} out of bounds for raw={raw_u16} alpha={alpha} max_rate={max_rate} trim={trim_offset}"
    );
});
