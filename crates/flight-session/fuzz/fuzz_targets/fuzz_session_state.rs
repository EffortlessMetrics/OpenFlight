// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for session state JSON deserialization.
//!
//! Exercises deserialization of `SessionState` and its component types from
//! arbitrary bytes to ensure no panics on malformed persisted state files.
//!
//! Run with: `cargo +nightly fuzz run fuzz_session_state`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz the top-level session state type.
    let _ = serde_json::from_slice::<flight_session::store::SessionState>(data);

    // Fuzz individual component types.
    let _ = serde_json::from_slice::<flight_session::store::WindowPosition>(data);
    let _ = serde_json::from_slice::<flight_session::store::CalibrationData>(data);
    let _ = serde_json::from_slice::<flight_session::store::ShutdownInfo>(data);

    // Fuzz the AircraftId and TelemetrySnapshot types from the session crate.
    let _ = serde_json::from_slice::<flight_session::AircraftId>(data);
    let _ = serde_json::from_slice::<flight_session::TelemetrySnapshot>(data);
});
