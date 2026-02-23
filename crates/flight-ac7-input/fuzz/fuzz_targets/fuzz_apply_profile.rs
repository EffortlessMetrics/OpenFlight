#![no_main]

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team
//
// Fuzz target: apply a profile to arbitrary existing ini content.
// Must never panic.

use flight_ac7_input::{Ac7InputProfile, apply_profile_to_existing};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|existing: &str| {
    let profile = Ac7InputProfile::default();
    let _ = apply_profile_to_existing(existing, &profile);
});
