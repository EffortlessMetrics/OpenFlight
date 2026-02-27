// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for flight-cloud-profiles JSON deserialization.
//!
//! Run with: `cargo +nightly fuzz run fuzz_cloud_profile_parse`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_cloud_profiles::models::{CloudProfile, ProfileListing};

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz generic JSON parse — must never panic
    let _ = serde_json::from_str::<serde_json::Value>(input);

    // Fuzz CloudProfile deserialization — must never panic
    if let Ok(cloud) = serde_json::from_str::<CloudProfile>(input) {
        let _ = cloud.score();
        let _ = serde_json::to_string(&cloud);
    }

    // Fuzz ProfileListing deserialization — must never panic
    if let Ok(listing) = serde_json::from_str::<ProfileListing>(input) {
        let _ = listing.score();
        let _ = serde_json::to_string(&listing);
    }
});
