// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for flight-profile JSON deserialization and validation.
//!
//! Run with: `cargo +nightly fuzz run fuzz_profile_parse`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_profile::Profile;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz JSON deserialization — must never panic
    if let Ok(profile) = serde_json::from_str::<Profile>(input) {
        // If deserialization succeeds, validation must also never panic
        let _ = profile.validate();
        let _ = profile.canonicalize();
        let _ = profile.effective_hash();
    }

    // Fuzz YAML deserialization — must never panic
    if let Ok(profile) = serde_yaml::from_str::<Profile>(input) {
        let _ = profile.validate();
        let _ = profile.canonicalize();
        let _ = profile.effective_hash();
    }
});
