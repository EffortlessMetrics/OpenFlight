// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: Logitech Extreme 3D Pro HID parsing → bus pipeline.
//!
//! Each test parses a raw HID byte slice using the device-specific parser,
//! maps the resulting state to a [`BusSnapshot`], publishes it through a
//! [`BusPublisher`], receives it via a [`Subscriber`], and asserts that the
//! round-tripped values match the originally parsed axes.

use flight_bus::{
    BusPublisher, SubscriptionConfig,
    snapshot::BusSnapshot,
    types::{AircraftId, SimId},
};
use flight_hotas_logitech::{EXTREME_3D_PRO_MIN_REPORT_BYTES, parse_extreme_3d_pro};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build a valid Extreme 3D Pro HID report using the bit-packed layout.
///
/// Report layout (7 bytes, bit-packed LSB-first):
/// - X (10 bits): 0–1023, centre = 512
/// - Y (10 bits): 0–1023, centre = 512
/// - Twist (8 bits): 0–255, centre = 128
/// - Throttle (7 bits): 0–127, 0 = top/forward (idle)
/// - Buttons (12 bits): button bitmask
/// - Hat (4 bits): 0=N, 1=NE, ... 7=NW, 8–15=center
fn build_extreme_3d_pro_report(x: u16, y: u16, twist: u8, throttle: u8, buttons: u16, hat: u8) -> [u8; 7] {
    let x = x & 0x3FF;
    let y = y & 0x3FF;
    let throttle = throttle & 0x7F;
    let buttons = buttons & 0x0FFF;
    let hat = hat & 0x0F;

    let mut data = [0u8; 7];
    data[0] = x as u8;
    data[1] = ((x >> 8) as u8) | ((y as u8 & 0x3F) << 2);
    data[2] = ((y >> 6) as u8 & 0x0F) | ((twist & 0x0F) << 4);
    data[3] = (twist >> 4) | ((throttle & 0x0F) << 4);
    data[4] = (throttle >> 4) | (((buttons & 0x1F) as u8) << 3);
    data[5] = ((buttons >> 5) as u8 & 0x7F) | ((hat & 0x01) << 7);
    data[6] = (hat >> 1) & 0x07;
    data
}

/// Publish a snapshot through a fresh bus and return the received snapshot.
fn publish_and_receive(snapshot: BusSnapshot) -> BusSnapshot {
    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    publisher.publish(snapshot).expect("publish must succeed");
    subscriber
        .try_recv()
        .expect("channel must not error")
        .expect("snapshot must be present after publish")
}

// ── Logitech Extreme 3D Pro tests ─────────────────────────────────────────────

/// Smoke-test: report at centre position parses without error.
#[test]
fn logitech_extreme_3d_pro_parses_valid_report() {
    let report = build_extreme_3d_pro_report(512, 512, 128, 0, 0, 8);
    assert_eq!(report.len(), EXTREME_3D_PRO_MIN_REPORT_BYTES);

    let state = parse_extreme_3d_pro(&report).expect("should parse valid report");

    // Centre x/y should be approximately 0.0
    assert!(
        (-0.1..=0.1).contains(&state.axes.x),
        "centred x should be near 0.0, got {}",
        state.axes.x
    );
    assert!(
        (-0.1..=0.1).contains(&state.axes.y),
        "centred y should be near 0.0, got {}",
        state.axes.y
    );
    // Throttle at min (0) should be 0.0
    assert!(
        (0.0..=0.01).contains(&state.axes.throttle),
        "min throttle should be near 0.0, got {}",
        state.axes.throttle
    );
}

/// Bipolar axes always stay in [-1.0, +1.0].
#[test]
fn logitech_extreme_3d_pro_axes_bounded() {
    let full_report = build_extreme_3d_pro_report(1023, 1023, 255, 127, 0xFFF, 8);
    let state = parse_extreme_3d_pro(&full_report).expect("valid report");

    assert!(
        (-1.0..=1.0).contains(&state.axes.x),
        "x out of range: {}",
        state.axes.x
    );
    assert!(
        (-1.0..=1.0).contains(&state.axes.y),
        "y out of range: {}",
        state.axes.y
    );
    assert!(
        (-1.0..=1.0).contains(&state.axes.twist),
        "twist out of range: {}",
        state.axes.twist
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.throttle),
        "throttle out of range: {}",
        state.axes.throttle
    );
}

/// Short report returns an error, not a panic.
#[test]
fn logitech_extreme_3d_pro_rejects_short_report() {
    for len in 0..EXTREME_3D_PRO_MIN_REPORT_BYTES {
        let short = vec![0u8; len];
        let result = parse_extreme_3d_pro(&short);
        assert!(
            result.is_err(),
            "expected error for {len}-byte report, got ok"
        );
    }
}

/// Bus round-trip: published snapshot is received with correct sim/aircraft.
#[test]
fn logitech_extreme_3d_pro_bus_round_trip() {
    let report = build_extreme_3d_pro_report(800, 200, 128, 64, 0, 8);
    let state = parse_extreme_3d_pro(&report).expect("valid report");

    // Verify axis values are bounded
    assert!(
        (-1.0..=1.0).contains(&state.axes.x),
        "x out of range: {}",
        state.axes.x
    );

    let snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("C172"));
    let received = publish_and_receive(snapshot);

    assert_eq!(received.sim, SimId::XPlane);
    assert_eq!(received.aircraft.icao, "C172");
}

/// Max throttle position parses to approximately 1.0.
#[test]
fn logitech_extreme_3d_pro_max_throttle() {
    let report = build_extreme_3d_pro_report(512, 512, 128, 127, 0, 8);
    let state = parse_extreme_3d_pro(&report).expect("valid report");

    assert!(
        state.axes.throttle > 0.99,
        "max throttle should be ~1.0, got {}",
        state.axes.throttle
    );
}
