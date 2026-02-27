// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fuzz target for the TrackIR / OpenTrack UDP packet parser and normaliser.
//!
//! Run with: `cargo +nightly fuzz run fuzz_trackir_packet`

#![no_main]

use flight_trackir::{normalize_pose, parse_packet};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(raw) = parse_packet(data) {
        let pose = normalize_pose(raw);
        assert!(
            (-1.0..=1.0).contains(&pose.x),
            "pose.x out of range: {}",
            pose.x
        );
        assert!(
            (-1.0..=1.0).contains(&pose.y),
            "pose.y out of range: {}",
            pose.y
        );
        assert!(
            (-1.0..=1.0).contains(&pose.z),
            "pose.z out of range: {}",
            pose.z
        );
        assert!(
            (-1.0..=1.0).contains(&pose.yaw),
            "pose.yaw out of range: {}",
            pose.yaw
        );
        assert!(
            (-1.0..=1.0).contains(&pose.pitch),
            "pose.pitch out of range: {}",
            pose.pitch
        );
        assert!(
            (-1.0..=1.0).contains(&pose.roll),
            "pose.roll out of range: {}",
            pose.roll
        );
    }
});
