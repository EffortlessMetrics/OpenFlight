// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for flight-profile merge and capability-enforcement paths.
//!
//! Run with: `cargo +nightly fuzz run fuzz_profile_merge`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_profile::{CapabilityContext, CapabilityMode, Profile};

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Split input at the midpoint to obtain two independent JSON fragments.
    let mid = input.len() / 2;
    let (left, right) = input.split_at(mid);

    let Ok(base) = serde_json::from_str::<Profile>(left) else {
        return;
    };
    let Ok(override_profile) = serde_json::from_str::<Profile>(right) else {
        return;
    };

    // merge_with must never panic regardless of profile contents
    if let Ok(merged) = base.merge_with(&override_profile) {
        // Downstream operations on a successfully merged profile must not panic
        let _ = merged.effective_hash();
        let _ = merged.canonicalize();
    }

    // validate_with_capabilities must never panic for any capability mode
    for mode in [CapabilityMode::Full, CapabilityMode::Demo, CapabilityMode::Kid] {
        let ctx = CapabilityContext::for_mode(mode);
        let _ = base.validate_with_capabilities(&ctx);
        let _ = override_profile.validate_with_capabilities(&ctx);
    }
});
