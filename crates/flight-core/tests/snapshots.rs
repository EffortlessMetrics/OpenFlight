// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for capability modes and config format examples.
//!
//! Run `cargo insta review` to accept new or changed snapshots.

use flight_core::calibration_store::{AxisCalibration, CalibrationStore};
use flight_core::profile::{CapabilityContext, CapabilityMode};

// ── Capability mode context snapshots ────────────────────────────────────────

#[test]
fn snapshot_capability_context_full_mode() {
    let ctx = CapabilityContext::for_mode(CapabilityMode::Full);
    insta::assert_json_snapshot!("capability_context_full", ctx);
}

#[test]
fn snapshot_capability_context_demo_mode() {
    let ctx = CapabilityContext::for_mode(CapabilityMode::Demo);
    insta::assert_json_snapshot!("capability_context_demo", ctx);
}

#[test]
fn snapshot_capability_context_kid_mode() {
    let ctx = CapabilityContext::for_mode(CapabilityMode::Kid);
    insta::assert_json_snapshot!("capability_context_kid", ctx);
}

// ── Config format example snapshots ──────────────────────────────────────────

#[test]
fn snapshot_calibration_store_toml_format() {
    let mut store = CalibrationStore::new();
    store.set(
        0x044F,
        0xB10A,
        vec![
            AxisCalibration::new(0, 0, 65535, 32768),
            AxisCalibration::new(1, 0, 65535, 32768),
        ],
    );
    let toml_str = toml::to_string_pretty(&store).unwrap();
    insta::assert_snapshot!("calibration_store_toml_format", toml_str);
}
