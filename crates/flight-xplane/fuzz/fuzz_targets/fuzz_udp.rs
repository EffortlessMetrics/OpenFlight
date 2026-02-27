// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for the X-Plane UDP binary packet parser.
//!
//! Exercises `handle_response` against arbitrary bytes, covering both the
//! RREF DataRef-response path and the DATA output path.
//!
//! Run with: `cargo +nightly fuzz run fuzz_udp`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Must never panic on any byte sequence — errors are expected and ignored.
    let _ = flight_xplane::udp::parse_udp_packet(data);
});
