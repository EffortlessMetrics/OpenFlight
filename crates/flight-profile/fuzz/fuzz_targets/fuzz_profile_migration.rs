// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for profile version migration paths (v1→v2→v3).
//!
//! Feeds arbitrary JSON into `MigrationRegistry::migrate` to exercise all
//! built-in migration functions with unexpected schemas. Must never panic.
//!
//! Run with: `cargo +nightly fuzz run fuzz_profile_migration`

#![no_main]

use flight_profile::profile_migration::MigrationRegistry;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };

    let registry = MigrationRegistry::new();

    // Exercise every built-in migration path — none may panic
    let _ = registry.migrate(value.clone(), "v1", "v2");
    let _ = registry.migrate(value.clone(), "v2", "v3");
    let _ = registry.migrate(value.clone(), "v1", "v3");

    // Unknown versions must return an error, not panic
    let _ = registry.migrate(value.clone(), "v0", "v3");
    let _ = registry.migrate(value, "v1", "v99");
});
