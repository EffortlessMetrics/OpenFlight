// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fuzz target for the OpenTrack UDP packet parser and normalisation helpers.
//!
//! Run with: `cargo +nightly fuzz run fuzz_opentrack_packet`

#![no_main]

use flight_opentrack::{parse_packet, pitch_to_normalized, yaw_to_normalized};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // parse_packet must never panic on any input
    if let Ok(pos) = parse_packet(data) {
        // All fields must be finite (parser rejects non-finite)
        assert!(pos.x_mm.is_finite());
        assert!(pos.y_mm.is_finite());
        assert!(pos.z_mm.is_finite());
        assert!(pos.yaw_deg.is_finite());
        assert!(pos.pitch_deg.is_finite());
        assert!(pos.roll_deg.is_finite());

        // Normalisation helpers must not panic
        let _ = yaw_to_normalized(pos.yaw_deg);
        let _ = pitch_to_normalized(pos.pitch_deg);
    }
});
