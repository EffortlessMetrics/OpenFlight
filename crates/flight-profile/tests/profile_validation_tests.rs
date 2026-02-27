// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Additional validation and capability tests for `flight-profile`.
//!
//! These tests exercise validation edge-cases and capability enforcement that
//! complement the snapshot tests in `snapshot_tests.rs`.

use flight_profile::{
    AircraftId, AxisConfig, CapabilityContext, CapabilityMode, PROFILE_SCHEMA_VERSION, Profile,
};
use std::collections::HashMap;

// ── helpers ──────────────────────────────────────────────────────────────────

fn profile_with_pitch(expo: Option<f32>, slew_rate: Option<f32>) -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo,
            slew_rate,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    }
}

// ── Schema version validation ─────────────────────────────────────────────────

/// An old or unrecognised schema version must fail validation.
/// This mirrors what a "profile migration" gate would enforce: older profiles
/// are rejected until they are migrated to the current schema.
#[test]
fn old_schema_version_fails_validation() {
    let profile = Profile {
        schema: "flight.profile/0".to_string(),
        sim: None,
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes: HashMap::new(),
        pof_overrides: None,
    };
    let err = profile.validate().unwrap_err();
    assert!(
        err.to_string().contains("Unsupported schema version"),
        "expected schema-version error, got: {err}"
    );
}

// ── Extreme / out-of-range values ─────────────────────────────────────────────

/// `expo = 2.0` exceeds `MAX_EXPO` (1.0) and must be rejected in Full mode.
/// The API rejects out-of-range values; it does not silently clamp them.
#[test]
fn expo_above_max_rejected_in_full_mode() {
    let profile = profile_with_pitch(Some(2.0), None);
    let err = profile.validate().unwrap_err();
    assert!(
        err.to_string().contains("expo"),
        "expected expo error, got: {err}"
    );
}

// ── Capability enforcement ────────────────────────────────────────────────────

/// Demo mode allows `expo` up to 0.6.  A value exactly at the limit must pass;
/// a value just above it must fail.
#[test]
fn demo_mode_enforces_max_expo() {
    let demo_ctx = CapabilityContext::for_mode(CapabilityMode::Demo);

    // At the limit (0.6) — should pass.
    let at_limit = profile_with_pitch(Some(0.6), None);
    assert!(
        at_limit.validate_with_capabilities(&demo_ctx).is_ok(),
        "expo at Demo limit should pass"
    );

    // One step above the limit — should fail.
    let above_limit = profile_with_pitch(Some(0.7), None);
    let err = above_limit
        .validate_with_capabilities(&demo_ctx)
        .unwrap_err();
    assert!(
        err.to_string().contains("Demo"),
        "error should mention the mode, got: {err}"
    );
}

/// Kid mode allows `slew_rate` up to 20.0.  A value at the limit must pass;
/// a value above must fail.
#[test]
fn kid_mode_enforces_max_slew_rate() {
    let kid_ctx = CapabilityContext::for_mode(CapabilityMode::Kid);

    // At the limit (20.0) — should pass.
    let at_limit = profile_with_pitch(None, Some(20.0));
    assert!(
        at_limit.validate_with_capabilities(&kid_ctx).is_ok(),
        "slew_rate at Kid limit should pass"
    );

    // Above the limit — should fail.
    let above_limit = profile_with_pitch(None, Some(25.0));
    let err = above_limit
        .validate_with_capabilities(&kid_ctx)
        .unwrap_err();
    assert!(
        err.to_string().contains("slew_rate"),
        "error should mention slew_rate, got: {err}"
    );
}

// ── Merge / conflict resolution ───────────────────────────────────────────────

/// When two profiles both configure the same axis, `merge_with` uses
/// last-writer-wins semantics: the overriding profile's non-None scalar fields
/// take precedence over the base profile's values.
#[test]
fn merge_same_axis_last_writer_wins() {
    // Base: pitch expo = 0.3
    let base = profile_with_pitch(Some(0.3), Some(10.0));

    // Override: pitch expo = 0.5, slew_rate left as None (base value kept).
    let mut override_axes = HashMap::new();
    override_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: Some(0.5),
            slew_rate: None, // intentionally absent — base slew_rate should be kept
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let override_profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: override_axes,
        pof_overrides: None,
    };

    let merged = base.merge_with(&override_profile).unwrap();

    let pitch = merged.axes.get("pitch").expect("pitch axis must exist");
    assert_eq!(
        pitch.expo,
        Some(0.5),
        "override expo should win over base expo"
    );
    assert_eq!(
        pitch.slew_rate,
        Some(10.0),
        "base slew_rate should be kept when override is None"
    );
}
