// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for X-Plane plugin message deserialization.
//!
//! Run with: `cargo +nightly fuzz run fuzz_xplane_plugin_msg`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_xplane::PluginMessage;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz JSON deserialization of plugin messages — must never panic
    let _ = serde_json::from_str::<PluginMessage>(input);
});
