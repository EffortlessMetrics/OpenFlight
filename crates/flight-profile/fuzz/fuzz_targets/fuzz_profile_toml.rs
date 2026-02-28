// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for flight-profile TOML deserialization and validation.
//!
//! Dedicated TOML corpus allows the fuzzer to explore TOML-specific edge cases
//! (inline tables, dotted keys, multi-line strings, date-times) separately from
//! JSON and YAML.
//!
//! Run with: `cargo +nightly fuzz run fuzz_profile_toml`

#![no_main]

use flight_profile::{CapabilityContext, CapabilityMode, Profile};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz TOML deserialization — must never panic
    if let Ok(profile) = toml::from_str::<Profile>(input) {
        let _ = profile.validate();
        let _ = profile.canonicalize();
        let _ = profile.effective_hash();

        // TOML re-serialization roundtrip — must not panic
        let _ = toml::to_string(&profile);

        // validate_with_capabilities must not panic for any mode
        for mode in [CapabilityMode::Full, CapabilityMode::Demo, CapabilityMode::Kid] {
            let ctx = CapabilityContext::for_mode(mode);
            let _ = profile.validate_with_capabilities(&ctx);
        }

        // merge_with on TOML-derived profiles must not panic
        if let Ok(merged) = profile.merge_with(&profile) {
            let _ = merged.effective_hash();
            let _ = merged.canonicalize();
        }
    }
});
