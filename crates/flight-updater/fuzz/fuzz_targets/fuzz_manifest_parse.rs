// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for update manifest JSON parsing and SemVer parsing.
//!
//! Exercises `parse_manifest` (JSON deserialization of `UpdateManifest`) and
//! `SemVer::parse` with arbitrary input. Must never panic.
//!
//! Run with: `cargo +nightly fuzz run fuzz_manifest_parse`

#![no_main]

use flight_updater::{SemVer, parse_manifest};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz JSON deserialization of UpdateManifest — must never panic
    if let Ok(manifest) = parse_manifest(data) {
        // If parsing succeeds, downstream operations must not panic
        let _ = manifest.canonical_bytes();
    }

    // Also fuzz SemVer::parse with the input interpreted as a string
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = SemVer::parse(s);
    }
});
