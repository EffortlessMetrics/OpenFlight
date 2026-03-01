// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for metrics export formatting.
//!
//! Constructs a `PrometheusRegistry` from arbitrary metric names, help text,
//! labels, and values, then exercises both Prometheus and JSON export to
//! ensure no panics on edge-case inputs (NaN, Inf, empty strings, etc.).
//!
//! Run with: `cargo +nightly fuzz run fuzz_metrics_export`

#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::BTreeMap;

fuzz_target!(|data: &[u8]| {
    if data.len() < 10 {
        return;
    }

    let mut registry = flight_metrics::prometheus_export::PrometheusRegistry::new();

    // Split data into chunks to derive metric parameters.
    let name = lossy_str(&data[..data.len().min(32)]);
    let help = lossy_str(&data[data.len().min(32)..data.len().min(64)]);

    // Derive an f64 value from the first 8 bytes (or fewer).
    let value = if data.len() >= 8 {
        f64::from_be_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ])
    } else {
        0.0
    };

    // Build a label map from remaining data.
    let mut labels = BTreeMap::new();
    if data.len() > 64 {
        let label_key = lossy_str(&data[64..data.len().min(80)]);
        let label_val = lossy_str(&data[data.len().min(80)..data.len().min(96)]);
        if !label_key.is_empty() {
            labels.insert(label_key, label_val);
        }
    }

    registry.register_counter(&name, &help, labels.clone(), value);
    registry.register_gauge(&name, &help, labels, value);

    // Exercise both export paths — must never panic.
    let _ = registry.export_prometheus();
    let _ = registry.export_json();
});

fn lossy_str(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
