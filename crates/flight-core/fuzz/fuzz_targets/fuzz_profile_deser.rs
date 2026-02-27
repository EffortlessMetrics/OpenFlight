// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for JSON deserialization of Profile via flight-core re-export.
//!
//! Run with: `cargo +nightly fuzz run fuzz_profile_deser`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_core::profile::Profile;

fuzz_target!(|data: &[u8]| {
    // Fuzz JSON deserialization of Profile — must never panic
    if let Ok(profile) = serde_json::from_slice::<Profile>(data) {
        // If deserialization succeeds, validate and hash — must never panic
        let _ = profile.validate();
        let _ = profile.effective_hash();
        let _ = profile.canonicalize();
    }
});
