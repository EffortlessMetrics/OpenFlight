// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for flight-profile YAML deserialization and validation.
//!
//! Dedicated YAML corpus allows the fuzzer to explore YAML-specific edge cases
//! (anchors, aliases, multi-document, block scalars) separately from JSON.
//!
//! Run with: `cargo +nightly fuzz run fuzz_profile_yaml`

#![no_main]

use flight_profile::{CapabilityContext, CapabilityMode, Profile};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz YAML deserialization — must never panic
    if let Ok(profile) = serde_yaml::from_str::<Profile>(input) {
        let _ = profile.validate();
        let _ = profile.canonicalize();
        let _ = profile.effective_hash();

        // validate_with_capabilities must not panic for any mode
        for mode in [CapabilityMode::Full, CapabilityMode::Demo, CapabilityMode::Kid] {
            let ctx = CapabilityContext::for_mode(mode);
            let _ = profile.validate_with_capabilities(&ctx);
        }
    }
});
