// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Step definitions for REQ-15 and REQ-16: Thrustmaster T.Flight HOTAS 4 Support

use crate::FlightWorld;
use cucumber::{given, then, when};
use flight_hotas_thrustmaster::{
    AxisMode, TFlightInputHandler, TFlightModel, TFlightYawPolicy, TFlightYawSource,
};

// ---------------------------------------------------------------------------
// Fixture bytes (deterministic — no hardware required)
// ---------------------------------------------------------------------------

/// Merged-mode report at rest: centered stick, mid throttle, centered twist, no buttons, no hat.
const FIXTURE_MERGED_CENTERED: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00];

/// Separate-mode report at rest: centered stick, mid throttle, centered twist/rocker, no buttons, no hat.
const FIXTURE_SEPARATE_CENTERED: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00];

/// Separate-mode report where aux (rocker) is at maximum (+1.0) and twist is at minimum (-1.0).
const FIXTURE_SEPARATE_AUX_DOMINANT: &[u8] =
    &[0x00, 0x80, 0x00, 0x80, 0x80, 0x00, 0xFF, 0x00, 0x00];

/// Report-ID prefixed merged payload: 0x01 + merged_centered.
/// Scaffold — replace first byte with actual Report ID from hardware receipt.
const FIXTURE_REPORT_ID_MERGED_CENTERED: &[u8] =
    &[0x01, 0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00];

/// Report-ID prefixed separate payload: 0x01 + separate_centered.
/// Scaffold — replace first byte with actual Report ID from hardware receipt.
const FIXTURE_REPORT_ID_SEPARATE_CENTERED: &[u8] =
    &[0x01, 0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00];

/// Merged report with HAT nibble = 0xA (10 — out of range), should clamp to 0.
const FIXTURE_HAT_OUT_OF_RANGE: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0xA0];

/// Merged report with HAT nibble = 0x8 (8 — last valid direction).
const FIXTURE_HAT_MAX_VALID: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x80];

/// Merged report with throttle byte = 0x00 (raw minimum).
const FIXTURE_THROTTLE_MIN: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x00];

fn fixture_bytes(name: &str) -> Vec<u8> {
    match name {
        "merged_centered" => FIXTURE_MERGED_CENTERED.to_vec(),
        "separate_centered" => FIXTURE_SEPARATE_CENTERED.to_vec(),
        "separate_aux_dominant" => FIXTURE_SEPARATE_AUX_DOMINANT.to_vec(),
        "report_id_merged_centered" => FIXTURE_REPORT_ID_MERGED_CENTERED.to_vec(),
        "report_id_separate_centered" => FIXTURE_REPORT_ID_SEPARATE_CENTERED.to_vec(),
        "hat_out_of_range" => FIXTURE_HAT_OUT_OF_RANGE.to_vec(),
        "hat_max_valid" => FIXTURE_HAT_MAX_VALID.to_vec(),
        "throttle_min" => FIXTURE_THROTTLE_MIN.to_vec(),
        other => panic!("unknown HOTAS 4 fixture: {other}"),
    }
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a HOTAS 4 input handler")]
async fn given_hotas4_handler(world: &mut FlightWorld) {
    world.hotas4_handler = Some(TFlightInputHandler::new(TFlightModel::Hotas4));
}

#[given(expr = "a HOTAS 4 input handler with yaw policy {string}")]
async fn given_hotas4_handler_with_policy(world: &mut FlightWorld, policy: String) {
    let yaw_policy = match policy.as_str() {
        "Auto" => TFlightYawPolicy::Auto,
        "Twist" => TFlightYawPolicy::Twist,
        "Aux" => TFlightYawPolicy::Aux,
        other => panic!("unknown yaw policy: {other}"),
    };
    world.hotas4_handler =
        Some(TFlightInputHandler::new(TFlightModel::Hotas4).with_yaw_policy(yaw_policy));
}

#[given("a HOTAS 4 input handler with report ID enabled")]
async fn given_hotas4_handler_with_report_id(world: &mut FlightWorld) {
    world.hotas4_handler =
        Some(TFlightInputHandler::new(TFlightModel::Hotas4).with_report_id(true));
}

#[given("a HOTAS 4 input handler with throttle inversion enabled")]
async fn given_hotas4_handler_with_throttle_inversion(world: &mut FlightWorld) {
    world.hotas4_handler =
        Some(TFlightInputHandler::new(TFlightModel::Hotas4).with_throttle_inversion(true));
}

#[given(expr = "a merged-mode report fixture {string}")]
async fn given_merged_fixture(world: &mut FlightWorld, name: String) {
    world.hotas4_report = Some(fixture_bytes(&name));
}

#[given(expr = "a separate-mode report fixture {string}")]
async fn given_separate_fixture(world: &mut FlightWorld, name: String) {
    world.hotas4_report = Some(fixture_bytes(&name));
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("I parse the report")]
async fn when_parse_report(world: &mut FlightWorld) {
    let handler = world
        .hotas4_handler
        .as_mut()
        .expect("handler must be initialised");
    let report = world
        .hotas4_report
        .as_deref()
        .expect("report fixture must be set");

    let state = handler
        .try_parse_report(report)
        .expect("parse must succeed for valid fixture");

    world.hotas4_yaw_resolution = Some(handler.resolve_yaw(&state));
    world.hotas4_parsed_state = Some(state);
}

/// Parse a named fixture in-place — sets `hotas4_report` and parses it.
/// Enables AC-15.4 mode-switch scenario and REQ-16 auto-detection scenarios.
#[when(expr = "I parse fixture {string}")]
async fn when_parse_fixture(world: &mut FlightWorld, name: String) {
    world.hotas4_report = Some(fixture_bytes(&name));
    when_parse_report(world).await;
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("rocker SHALL be absent")]
async fn then_rocker_absent(world: &mut FlightWorld) {
    let state = world.hotas4_parsed_state.as_ref().expect("state not set");
    assert!(
        state.axes.rocker.is_none(),
        "expected rocker to be absent in merged mode"
    );
}

#[then("rocker SHALL be present")]
async fn then_rocker_present(world: &mut FlightWorld) {
    let state = world.hotas4_parsed_state.as_ref().expect("state not set");
    assert!(
        state.axes.rocker.is_some(),
        "expected rocker to be present in separate mode"
    );
}

#[then(expr = "hat SHALL equal {int}")]
async fn then_hat_equals(world: &mut FlightWorld, expected: u8) {
    let state = world.hotas4_parsed_state.as_ref().expect("state not set");
    assert_eq!(state.buttons.hat, expected, "hat value mismatch");
}

#[then(expr = "button mask SHALL equal {int}")]
async fn then_button_mask_equals(world: &mut FlightWorld, expected: u16) {
    let state = world.hotas4_parsed_state.as_ref().expect("state not set");
    assert_eq!(state.buttons.buttons, expected, "button mask mismatch");
}

#[then(expr = "resolved yaw source SHALL equal {string}")]
async fn then_yaw_source_equals(world: &mut FlightWorld, expected: String) {
    let resolution = world
        .hotas4_yaw_resolution
        .as_ref()
        .expect("yaw resolution not set");
    let expected_source = match expected.as_str() {
        "Combined" => TFlightYawSource::Combined,
        "Twist" => TFlightYawSource::Twist,
        "Aux" => TFlightYawSource::Aux,
        other => panic!("unknown yaw source: {other}"),
    };
    assert_eq!(
        resolution.source, expected_source,
        "yaw source mismatch: expected {expected_source:?}, got {:?}",
        resolution.source
    );
}

#[then(expr = "axis mode SHALL equal {string}")]
async fn then_axis_mode_equals(world: &mut FlightWorld, expected: String) {
    let state = world.hotas4_parsed_state.as_ref().expect("state not set");
    let expected_mode = match expected.as_str() {
        "Merged" => AxisMode::Merged,
        "Separate" => AxisMode::Separate,
        "Unknown" => AxisMode::Unknown,
        other => panic!("unknown axis mode: {other}"),
    };
    assert_eq!(
        state.axis_mode, expected_mode,
        "axis mode mismatch: expected {expected_mode:?}, got {:?}",
        state.axis_mode
    );
}

#[then(expr = "HAT SHALL equal {int}")]
async fn then_hat_value_equals(world: &mut FlightWorld, expected: u8) {
    let state = world.hotas4_parsed_state.as_ref().expect("state not set");
    assert_eq!(state.buttons.hat, expected, "HAT value mismatch");
}

#[then(expr = "throttle SHALL be approximately {float}")]
async fn then_throttle_approximately(world: &mut FlightWorld, expected: f32) {
    let state = world.hotas4_parsed_state.as_ref().expect("state not set");
    assert!(
        (state.axes.throttle - expected).abs() < 0.05,
        "throttle mismatch: expected ~{expected}, got {}",
        state.axes.throttle
    );
}
