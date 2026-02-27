// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for aircraft type (ICAO) string parsing and Profile validation.
//!
//! Run with: `cargo +nightly fuzz run fuzz_aircraft_type`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_core::profile::{AircraftId, Profile};
use std::collections::HashMap;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    // Fuzz Profile::validate with an arbitrary ICAO aircraft type — must never panic
    let profile = Profile {
        schema: "flight.profile/1".to_string(),
        sim: None,
        aircraft: Some(AircraftId { icao: input.to_string() }),
        axes: HashMap::new(),
        pof_overrides: None,
    };
    let _ = profile.validate();

    // Also fuzz with the input as the sim string — must never panic
    let profile2 = Profile {
        schema: "flight.profile/1".to_string(),
        sim: Some(input.to_string()),
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    };
    let _ = profile2.validate();

    // Fuzz schema field — must never panic
    let profile3 = Profile {
        schema: input.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::new(),
        pof_overrides: None,
    };
    let _ = profile3.validate();
});
