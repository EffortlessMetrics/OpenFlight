// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for blackbox codec deserialization.
//!
//! Exercises postcard deserialization of the core .fbb record types and
//! serde_json deserialization of the export DTO types to ensure no panics
//! on malformed input.
//!
//! Run with: `cargo +nightly fuzz run fuzz_blackbox_codec`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Postcard decode of core binary types used in .fbb files.
    let _ = postcard::from_bytes::<flight_blackbox::BlackboxHeader>(data);
    let _ = postcard::from_bytes::<flight_blackbox::BlackboxRecord>(data);
    let _ = postcard::from_bytes::<flight_blackbox::BlackboxFooter>(data);
    let _ = postcard::from_bytes::<flight_blackbox::IndexEntry>(data);

    // JSON deserialization of export DTO types.
    let _ = serde_json::from_slice::<flight_blackbox::ExportDoc>(data);
    let _ = serde_json::from_slice::<flight_blackbox::ExportHeader>(data);
    let _ = serde_json::from_slice::<flight_blackbox::ExportRecord>(data);
    let _ = serde_json::from_slice::<flight_blackbox::ExportSummary>(data);
    let _ = serde_json::from_slice::<flight_blackbox::export::AxisRecordDto>(data);
    let _ = serde_json::from_slice::<flight_blackbox::export::EventRecordDto>(data);
    let _ = serde_json::from_slice::<flight_blackbox::export::TelemetryRecordDto>(data);
    let _ = serde_json::from_slice::<flight_blackbox::export::FfbRecordDto>(data);
});
