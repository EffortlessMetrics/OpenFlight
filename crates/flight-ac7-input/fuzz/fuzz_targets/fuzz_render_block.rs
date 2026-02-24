#![no_main]

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team
//
// Fuzz target: render a managed block from arbitrary profile name bytes.
// Must never panic.

use flight_ac7_input::{Ac7InputProfile, render_managed_block};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|name: &str| {
    let mut profile = Ac7InputProfile::default();
    profile.name = name.to_string();
    // validate + render must never panic (may return errors)
    let _ = render_managed_block(&profile);
});
