// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for update manifest JSON parsing.
//!
//! Feeds arbitrary bytes into the manifest parser and semver parser to ensure
//! no panics on malformed JSON or version strings.
//!
//! Run with: `cargo +nightly fuzz run fuzz_manifest_parse`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Primary: exercise the manifest JSON parser.
    let _ = flight_updater::parse_manifest(data);

    // Also try direct serde deserialization of component types.
    let _ = serde_json::from_slice::<flight_updater::SignedUpdateManifest>(data);
    let _ = serde_json::from_slice::<flight_updater::FileUpdate>(data);
    let _ = serde_json::from_slice::<flight_updater::SemVer>(data);

    // Fuzz the string-based semver parser.
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = flight_updater::SemVer::parse(s);
    }
});
