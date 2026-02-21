// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Step definitions for REQ-15: Thrustmaster T.Flight HOTAS 4 Support

use crate::FlightWorld;
use cucumber::{given, then, when};
use flight_hotas_thrustmaster::{
    TFlightInputHandler, TFlightYawPolicy, TFlightYawSource,
    TFlightModel,
};

// ---------------------------------------------------------------------------
// Fixture bytes (deterministic — no hardware required)
// ---------------------------------------------------------------------------

/// Merged-mode report at rest: centered stick, mid throttle, centered twist, no buttons, no hat.
const FIXTURE_MERGED_CENTERED: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00];

/// Separate-mode report at rest: centered stick, mid throttle, centered twist/rocker, no buttons, no hat.
const FIXTURE_SEPARATE_CENTERED: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x80, 0x00, 0x00];

/// Separate-mode report where aux (rocker) is at maximum (+1.0) and twist is at minimum (-1.0).
const FIXTURE_SEPARATE_AUX_DOMINANT: &[u8] = &[0x00, 0x80, 0x00, 0x80, 0x80, 0x00, 0xFF, 0x00, 0x00];

fn fixture_bytes(name: &str) -> Vec<u8> {
    match name {
        "merged_centered" => FIXTURE_MERGED_CENTERED.to_vec(),
        "separate_centered" => FIXTURE_SEPARATE_CENTERED.to_vec(),
        "separate_aux_dominant" => FIXTURE_SEPARATE_AUX_DOMINANT.to_vec(),
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
    world.hotas4_handler = Some(
        TFlightInputHandler::new(TFlightModel::Hotas4).with_yaw_policy(yaw_policy),
    );
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
    assert_eq!(
        state.buttons.hat, expected,
        "hat value mismatch"
    );
}

#[then(expr = "button mask SHALL equal {int}")]
async fn then_button_mask_equals(world: &mut FlightWorld, expected: u16) {
    let state = world.hotas4_parsed_state.as_ref().expect("state not set");
    assert_eq!(
        state.buttons.buttons, expected,
        "button mask mismatch"
    );
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
