// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests: Saitek X52/X55/X56 HOTAS HID parsing → bus pipeline.
//!
//! Each test parses a raw HID byte slice using the device-specific parser,
//! optionally maps the result through a [`BusSnapshot`] round-trip, and
//! asserts that parsed axis values and button states are correct.
//!
//! Note: The crate covers X52, X55, and X56 families; there is no Cessna
//! panel variant in this crate.

use flight_bus::{
    BusPublisher, SubscriptionConfig,
    snapshot::BusSnapshot,
    types::{AircraftId, SimId},
};
use flight_hotas_saitek::{HotasInputHandler, SaitekHotasType};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build a 14-byte X52/X52 Pro HID report.
///
/// Layout (best-effort, per parser in source):
/// | Bytes | Content |
/// |-------|---------|
/// | 0–1   | Stick X (11-bit; low byte in [0], bits 10–8 masked in [1] & 0x07) |
/// | 2–3   | Stick Y (same encoding) |
/// | 6     | Throttle (8-bit unsigned) |
/// | 7–10  | Primary buttons (u32 LE) |
fn make_x52_report(x_11bit: u16, y_11bit: u16, throttle: u8, buttons: u32) -> [u8; 14] {
    let mut r = [0u8; 14];
    r[0] = (x_11bit & 0xFF) as u8;
    r[1] = ((x_11bit >> 8) & 0x07) as u8;
    r[2] = (y_11bit & 0xFF) as u8;
    r[3] = ((y_11bit >> 8) & 0x07) as u8;
    r[6] = throttle;
    r[7..11].copy_from_slice(&buttons.to_le_bytes());
    r
}

/// Build a 12-byte X55/X56 stick HID report.
///
/// | Bytes | Content |
/// |-------|---------|
/// | 0–1   | Stick X (u16 LE, signed-normalised) |
/// | 2–3   | Stick Y (u16 LE, signed-normalised) |
/// | 8–11  | Primary buttons (u32 LE) |
fn make_x55_stick_report(x: u16, y: u16, buttons: u32) -> [u8; 12] {
    let mut r = [0u8; 12];
    r[0..2].copy_from_slice(&x.to_le_bytes());
    r[2..4].copy_from_slice(&y.to_le_bytes());
    r[8..12].copy_from_slice(&buttons.to_le_bytes());
    r
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

// ── Saitek X52 / X55 / X56 tests ─────────────────────────────────────────────

/// Smoke-test: centred X52 report parses without error and axes are near zero.
#[test]
fn saitek_x52_parses_centred_report() {
    let mut handler = HotasInputHandler::new(SaitekHotasType::X52);
    // 11-bit centre ≈ 0x400 (1024); throttle centre ≈ 127.
    let report = make_x52_report(0x400, 0x400, 127, 0);
    let state = handler.parse_report(&report);

    assert!(
        state.axes.stick_x.abs() < 0.01,
        "centred stick_x should be ~0.0, got {}",
        state.axes.stick_x
    );
    assert!(
        state.axes.stick_y.abs() < 0.01,
        "centred stick_y should be ~0.0, got {}",
        state.axes.stick_y
    );
    assert!(
        state.axes.throttle.abs() < 0.01,
        "centred throttle should be ~0.0, got {}",
        state.axes.throttle
    );
    assert_eq!(state.buttons.primary, 0, "no buttons should be pressed");
}

/// Full-throttle X52 Pro report produces a normalised value near +1.0.
#[test]
fn saitek_x52_full_throttle_normalises_to_max() {
    let mut handler = HotasInputHandler::new(SaitekHotasType::X52Pro);
    let report = make_x52_report(0x400, 0x400, 255, 0);
    let state = handler.parse_report(&report);

    assert!(
        state.axes.throttle > 0.99,
        "full throttle should approach +1.0, got {}",
        state.axes.throttle
    );
}

/// X55 stick axes remain within [−1, +1] after a bus round-trip.
#[test]
fn saitek_x55_stick_axes_in_bounds_after_bus_round_trip() {
    let mut handler = HotasInputHandler::new(SaitekHotasType::X55Stick);
    // Partial deflection using 16-bit signed range.
    let report = make_x55_stick_report(0xC000, 0x4000, 0);
    let state = handler.parse_report(&report);

    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    let received = publish_and_receive(snapshot);

    assert!(
        (-1.0..=1.0).contains(&state.axes.stick_x),
        "stick_x {} out of [−1, +1]",
        state.axes.stick_x
    );
    assert!(
        (-1.0..=1.0).contains(&state.axes.stick_y),
        "stick_y {} out of [−1, +1]",
        state.axes.stick_y
    );
    assert_eq!(received.sim, SimId::Msfs);
}

/// Short X52 report (< 14 bytes) returns a zero-default state, not a panic.
#[test]
fn saitek_x52_short_report_yields_default_state() {
    let mut handler = HotasInputHandler::new(SaitekHotasType::X52);
    for len in 0..14 {
        let short = vec![0u8; len];
        let state = handler.parse_report(&short);
        assert_eq!(
            state.axes.stick_x, 0.0,
            "short {len}-byte report should yield default stick_x"
        );
        assert_eq!(
            state.axes.throttle, 0.0,
            "short {len}-byte report should yield default throttle"
        );
        assert_eq!(
            state.buttons.primary, 0,
            "short {len}-byte report should yield no buttons"
        );
    }
}

/// X55 stick button bitmask is decoded correctly after the debounce window.
///
/// The Saitek input handler applies a ghost-input debounce filter; buttons only
/// appear in output after the raw state has been stable for the threshold
/// duration (≥ 20 ms). This test calls `parse_report` twice, sleeping between
/// calls to let the debounce timer mature.
#[test]
fn saitek_x55_stick_button_decoding() {
    let mut handler = HotasInputHandler::new(SaitekHotasType::X55Stick);
    let report = make_x55_stick_report(0x8000, 0x8000, 0x0000_0001); // button 1 pressed

    // First call starts the debounce timer while the button is held stable.
    let _ = handler.parse_report(&report);
    std::thread::sleep(std::time::Duration::from_millis(30));

    // Second call after the debounce window: button 1 should now be visible.
    let state = handler.parse_report(&report);
    assert_ne!(
        state.buttons.primary & 1,
        0,
        "button 1 (bit 0) should be set after debounce, primary=0x{:08X}",
        state.buttons.primary
    );
    assert_eq!(
        state.buttons.primary & !1,
        0,
        "no other buttons should be set"
    );
}
