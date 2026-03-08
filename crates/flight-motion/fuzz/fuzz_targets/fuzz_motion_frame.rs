// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fuzz target for `MotionFrame` operations.
//!
//! Exercises frame construction from arbitrary f32 values, clamping,
//! scaling, SimTools string generation, and array conversion.
//!
//! Run with: `cargo +nightly fuzz run fuzz_motion_frame`

#![no_main]

use flight_motion::MotionFrame;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Need at least 28 bytes: 6 × f32 (24) + 1 × f32 scale factor (4)
    if data.len() < 28 {
        return;
    }

    let f = |offset: usize| -> f32 {
        let bytes: [u8; 4] = data[offset..offset + 4].try_into().unwrap();
        f32::from_le_bytes(bytes)
    };

    let frame = MotionFrame {
        surge: f(0),
        sway: f(4),
        heave: f(8),
        roll: f(12),
        pitch: f(16),
        yaw: f(20),
    };
    let scale = f(24);

    // to_simtools_string must not panic
    let _ = frame.to_simtools_string();

    // to_array must not panic
    let arr = frame.to_array();
    assert_eq!(arr.len(), 6);

    // is_neutral must not panic
    let _ = frame.is_neutral();

    // Display must not panic
    let _ = format!("{frame}");

    // clamped must produce values in [-1, 1]
    let clamped = frame.clamped();
    for &v in &clamped.to_array() {
        if v.is_finite() {
            assert!(
                (-1.0..=1.0).contains(&v),
                "clamped value out of range: {v}"
            );
        }
    }

    // scaled must not panic (even with NaN/Inf scale)
    let _ = frame.scaled(scale);
});
