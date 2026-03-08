// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for blackbox binary format decoding.
//!
//! Feeds arbitrary bytes through the postcard deserialization paths used by
//! `BlackboxReader` to decode headers, records, footers, and index entries.
//! Must never panic on malformed input.
//!
//! Run with: `cargo +nightly fuzz run fuzz_blackbox_decode`

#![no_main]

use flight_blackbox::{BlackboxFooter, BlackboxHeader, BlackboxRecord, IndexEntry};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz postcard deserialization of each blackbox wire type — must never panic
    let _ = postcard::from_bytes::<BlackboxHeader>(data);
    let _ = postcard::from_bytes::<BlackboxRecord>(data);
    let _ = postcard::from_bytes::<BlackboxFooter>(data);
    let _ = postcard::from_bytes::<IndexEntry>(data);
});
