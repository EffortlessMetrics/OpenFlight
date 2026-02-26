// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Integration tests: VKB Gladiator NXT EVO HID parsing → bus pipeline.
//!
//! Each test parses a raw HID byte slice using the Gladiator parser,
//! optionally maps the resulting state through a [`BusSnapshot`] round-trip,
//! and asserts that axis and button values are correct.

use flight_bus::{
    BusPublisher, SubscriptionConfig,
    snapshot::BusSnapshot,
    types::{AircraftId, SimId},
};
use flight_hotas_vkb::{GladiatorInputHandler, VkbGladiatorVariant};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build a 21-byte Gladiator NXT EVO HID report (no report-ID prefix).
///
/// Layout:
/// | Bytes | Content |
/// |-------|---------|
/// | 0–11  | Six u16 LE axes: roll, pitch, yaw, mini_x, mini_y, throttle |
/// | 12–15 | Button bits 0–31 (u32 LE) |
/// | 16–19 | Button bits 32–63 (u32 LE) |
/// | 20    | Hat nibbles: low = hat0, high = hat1; 0xF = centred |
fn make_gladiator_report(axes: [u16; 6], btn_lo: u32, btn_hi: u32, hat_byte: u8) -> [u8; 21] {
    let mut r = [0u8; 21];
    for (i, &v) in axes.iter().enumerate() {
        let le = v.to_le_bytes();
        r[i * 2] = le[0];
        r[i * 2 + 1] = le[1];
    }
    r[12..16].copy_from_slice(&btn_lo.to_le_bytes());
    r[16..20].copy_from_slice(&btn_hi.to_le_bytes());
    r[20] = hat_byte;
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

// ── VKB Gladiator NXT EVO tests ───────────────────────────────────────────────

/// Smoke-test: centred report parses without error and stick axes are near zero.
#[test]
fn vkb_gladiator_nxt_parses_centred_report() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    // Signed axes centre at 0x8000; throttle wheel idles at 0x0000.
    let report =
        make_gladiator_report([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000], 0, 0, 0xFF);
    let state = handler.parse_report(&report).expect("valid centred report");

    assert!(
        state.axes.roll.abs() < 0.01,
        "centred roll should be ~0.0, got {}",
        state.axes.roll
    );
    assert!(
        state.axes.pitch.abs() < 0.01,
        "centred pitch should be ~0.0, got {}",
        state.axes.pitch
    );
    assert!(
        state.axes.yaw.abs() < 0.01,
        "centred yaw should be ~0.0, got {}",
        state.axes.yaw
    );
    assert_eq!(state.axes.throttle, 0.0, "idle throttle should be 0.0");
    assert!(
        state.pressed_buttons().is_empty(),
        "no buttons should be pressed at centre"
    );
}

/// Full-positive deflection: stick axes at 0xFFFF produce approximately +1.0.
#[test]
fn vkb_gladiator_nxt_full_deflection_positive() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    let report =
        make_gladiator_report([0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF], 0, 0, 0xFF);
    let state = handler
        .parse_report(&report)
        .expect("valid full-deflection report");

    assert!(
        (0.99..=1.01).contains(&state.axes.roll),
        "full-right roll should approach +1.0, got {}",
        state.axes.roll
    );
    assert!(
        (0.99..=1.01).contains(&state.axes.pitch),
        "full-aft pitch should approach +1.0, got {}",
        state.axes.pitch
    );
    assert!(
        (0.99..=1.01).contains(&state.axes.throttle),
        "full throttle should approach +1.0, got {}",
        state.axes.throttle
    );
}

/// Parsed axis values remain within expected bounds after a bus round-trip.
#[test]
fn vkb_gladiator_nxt_axes_in_bounds_after_bus_round_trip() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoLeft);
    // Partial deflection: roll ≈ +50%, pitch ≈ −50%, throttle ≈ 50%.
    let report = make_gladiator_report(
        [0xC000, 0x4000, 0x8000, 0x8000, 0x8000, 0x8000],
        0b0000_0101,
        0,
        0xFF,
    );
    let state = handler
        .parse_report(&report)
        .expect("valid partial-deflection report");

    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("A320"));
    let received = publish_and_receive(snapshot);

    assert!(
        (-1.0..=1.0).contains(&state.axes.roll),
        "roll {} out of [−1, +1]",
        state.axes.roll
    );
    assert!(
        (-1.0..=1.0).contains(&state.axes.pitch),
        "pitch {} out of [−1, +1]",
        state.axes.pitch
    );
    assert!(
        (0.0..=1.0).contains(&state.axes.throttle),
        "throttle {} out of [0, +1]",
        state.axes.throttle
    );
    assert_eq!(received.sim, SimId::Msfs);
}

/// Reports shorter than 12 bytes are rejected with an appropriate error.
#[test]
fn vkb_gladiator_nxt_rejects_short_report() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    for len in 0..12 {
        let short = vec![0u8; len];
        assert!(
            handler.parse_report(&short).is_err(),
            "expected error for {len}-byte report, got ok"
        );
    }
}

/// Button bitmask is decoded correctly across both the lo and hi u32 words.
#[test]
fn vkb_gladiator_nxt_button_decoding() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    let centre_axes = [0x8000u16, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000];

    // Buttons 1 (bit 0) and 32 (bit 31) in lo word.
    let report = make_gladiator_report(centre_axes, 0x8000_0001, 0, 0xFF);
    let state = handler.parse_report(&report).expect("valid report");
    assert!(state.buttons[0], "button 1 should be pressed");
    assert!(state.buttons[31], "button 32 should be pressed");
    assert!(!state.buttons[32], "button 33 should not be pressed");

    // Button 33 (bit 0 of hi word) only.
    let report2 = make_gladiator_report(centre_axes, 0, 0x0000_0001, 0xFF);
    let state2 = handler.parse_report(&report2).expect("valid report");
    assert!(state2.buttons[32], "button 33 should be pressed");
    assert_eq!(state2.pressed_buttons(), vec![33]);
}
