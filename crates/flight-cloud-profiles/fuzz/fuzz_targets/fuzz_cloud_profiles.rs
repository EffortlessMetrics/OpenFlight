// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for flight-cloud-profiles JSON deserialization, sanitization,
//! and validation.
//!
//! This crate parses untrusted JSON from the network, so it is a high-value
//! fuzz target.
//!
//! Run with: `cargo +nightly fuzz run fuzz_cloud_profiles`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_cloud_profiles::{
    models::{CloudProfile, ProfileListing, ProfileSortOrder, VoteDirection, VoteResult},
    sanitize_for_upload,
    sanitize::validate_for_publish,
};

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz ProfileListing deserialization (returned by GET /profiles)
    if let Ok(listing) = serde_json::from_str::<ProfileListing>(input) {
        let _ = listing.score();
        // Re-serialize must not panic
        let _ = serde_json::to_string(&listing);
    }

    // Fuzz CloudProfile deserialization (returned by GET /profiles/{id})
    // This also exercises embedded Profile deserialization.
    if let Ok(cloud) = serde_json::from_str::<CloudProfile>(input) {
        let _ = cloud.score();
        // Sanitize the embedded profile — must never panic
        let sanitized = sanitize_for_upload(&cloud.profile);
        let _ = validate_for_publish(&sanitized, &cloud.title);
    }

    // Fuzz sort order and vote direction deserialization
    let _ = serde_json::from_str::<ProfileSortOrder>(input);
    let _ = serde_json::from_str::<VoteDirection>(input);
    let _ = serde_json::from_str::<VoteResult>(input);
});
