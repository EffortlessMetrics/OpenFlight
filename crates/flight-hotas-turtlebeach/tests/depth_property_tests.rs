// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based depth tests for Turtle Beach VelocityOne HID report parsing.
//!
//! Covers invariants across the full input domain: axis bounds, finiteness,
//! short-report rejection, and button-mask consistency.

use flight_hotas_turtlebeach::{
    FLIGHTDECK_MIN_REPORT_BYTES, FLIGHT_MIN_REPORT_BYTES, FLIGHTSTICK_MIN_REPORT_BYTES,
    RUDDER_MIN_REPORT_BYTES, parse_flight_report, parse_flightdeck_report,
    parse_flightstick_report, parse_rudder_report,
};
use proptest::prelude::*;

// ── Report builders (shared) ─────────────────────────────────────────────────

mod common;
use common::{FlightInput, build_flight, make_flightstick, make_flightdeck, make_rudder};

// ═══════════════════════════════════════════════════════════════════════════════
// VelocityOne Flight (yoke) property tests
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    /// All Flight bipolar axes stay within [-1.0, 1.0].
    #[test]
    fn prop_flight_bipolar_axes_bounded(
        roll in 0u16..=4095u16,
        pitch in 0u16..=4095u16,
        rudder in 0u16..=4095u16,
        trim in 0u16..=4095u16,
    ) {
        let r = parse_flight_report(&build_flight(&FlightInput { roll, pitch, rudder, trim, ..Default::default() })).unwrap();
        prop_assert!((-1.0..=1.0).contains(&r.roll), "roll={}", r.roll);
        prop_assert!((-1.0..=1.0).contains(&r.pitch), "pitch={}", r.pitch);
        prop_assert!((-1.0..=1.0).contains(&r.rudder_twist), "rudder_twist={}", r.rudder_twist);
        prop_assert!((-1.0..=1.0).contains(&r.trim_wheel), "trim_wheel={}", r.trim_wheel);
    }

    /// Flight throttle (unipolar) axes stay within [0.0, 1.0].
    #[test]
    fn prop_flight_throttle_bounded(tl in 0u8..=255u8, tr in 0u8..=255u8) {
        let r = parse_flight_report(&build_flight(&FlightInput { tl, tr, ..Default::default() })).unwrap();
        prop_assert!((0.0..=1.0).contains(&r.throttle_left), "throttle_left={}", r.throttle_left);
        prop_assert!((0.0..=1.0).contains(&r.throttle_right), "throttle_right={}", r.throttle_right);
    }

    /// Flight axes are always finite (no NaN / Inf).
    #[test]
    fn prop_flight_axes_finite(
        roll in 0u16..=u16::MAX,
        pitch in 0u16..=u16::MAX,
        rudder in 0u16..=u16::MAX,
        tl in 0u8..=255u8,
        tr in 0u8..=255u8,
    ) {
        let r = parse_flight_report(&build_flight(&FlightInput { roll, pitch, rudder, tl, tr, ..Default::default() })).unwrap();
        prop_assert!(r.roll.is_finite(), "roll not finite: {}", r.roll);
        prop_assert!(r.pitch.is_finite(), "pitch not finite: {}", r.pitch);
        prop_assert!(r.rudder_twist.is_finite(), "rudder_twist not finite: {}", r.rudder_twist);
        prop_assert!(r.throttle_left.is_finite(), "throttle_left not finite: {}", r.throttle_left);
        prop_assert!(r.throttle_right.is_finite(), "throttle_right not finite: {}", r.throttle_right);
        prop_assert!(r.trim_wheel.is_finite(), "trim_wheel not finite: {}", r.trim_wheel);
    }

    /// Reports shorter than FLIGHT_MIN_REPORT_BYTES always fail.
    #[test]
    fn prop_flight_short_report_errors(
        data in proptest::collection::vec(any::<u8>(), 0..FLIGHT_MIN_REPORT_BYTES),
    ) {
        prop_assert!(parse_flight_report(&data).is_err());
    }

    /// Flight toggle switch mask is always ≤ 0x7F (only 7 bits used).
    #[test]
    fn prop_flight_toggle_mask_bounded(toggles in 0u8..=255u8) {
        let r = parse_flight_report(&build_flight(&FlightInput { toggles, ..Default::default() })).unwrap();
        prop_assert!(r.toggle_switches <= 0x7F,
            "toggle_switches should be masked to 7 bits, got 0x{:02X}", r.toggle_switches);
    }

    /// Flight hat switch is always 0–8.
    #[test]
    fn prop_flight_hat_bounded(hat in 0u8..=255u8) {
        let r = parse_flight_report(&build_flight(&FlightInput { hat, ..Default::default() })).unwrap();
        prop_assert!(r.hat <= 8, "hat should be 0–8, got {}", r.hat);
    }

    /// Arbitrary valid-length reports must not panic.
    #[test]
    fn prop_flight_no_panic(
        data in proptest::collection::vec(any::<u8>(), FLIGHT_MIN_REPORT_BYTES..64),
    ) {
        let _ = parse_flight_report(&data);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// VelocityOne Flightstick property tests
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    /// All Flightstick bipolar axes stay within [-1.0, 1.0].
    #[test]
    fn prop_flightstick_axes_bounded(
        x in 0u16..=4095u16,
        y in 0u16..=4095u16,
        twist in 0u16..=4095u16,
    ) {
        let r = parse_flightstick_report(&make_flightstick(x, y, twist, 0, 0, 15)).unwrap();
        prop_assert!((-1.0..=1.0).contains(&r.x), "x={}", r.x);
        prop_assert!((-1.0..=1.0).contains(&r.y), "y={}", r.y);
        prop_assert!((-1.0..=1.0).contains(&r.twist), "twist={}", r.twist);
    }

    /// Flightstick throttle slider stays within [0.0, 1.0].
    #[test]
    fn prop_flightstick_throttle_bounded(throttle in 0u8..=255u8) {
        let r = parse_flightstick_report(&make_flightstick(2048, 2048, 2048, throttle, 0, 15)).unwrap();
        prop_assert!((0.0..=1.0).contains(&r.throttle), "throttle={}", r.throttle);
    }

    /// Flightstick axes are always finite.
    #[test]
    fn prop_flightstick_axes_finite(
        x in 0u16..=u16::MAX,
        y in 0u16..=u16::MAX,
        twist in 0u16..=u16::MAX,
        throttle in 0u8..=255u8,
    ) {
        let r = parse_flightstick_report(&make_flightstick(x, y, twist, throttle, 0, 15)).unwrap();
        prop_assert!(r.x.is_finite(), "x not finite: {}", r.x);
        prop_assert!(r.y.is_finite(), "y not finite: {}", r.y);
        prop_assert!(r.twist.is_finite(), "twist not finite: {}", r.twist);
        prop_assert!(r.throttle.is_finite(), "throttle not finite: {}", r.throttle);
    }

    /// Reports shorter than FLIGHTSTICK_MIN_REPORT_BYTES always fail.
    #[test]
    fn prop_flightstick_short_report_errors(
        data in proptest::collection::vec(any::<u8>(), 0..FLIGHTSTICK_MIN_REPORT_BYTES),
    ) {
        prop_assert!(parse_flightstick_report(&data).is_err());
    }

    /// Flightstick hat switch is always 0–8.
    #[test]
    fn prop_flightstick_hat_bounded(hat in 0u8..=255u8) {
        let r = parse_flightstick_report(&make_flightstick(2048, 2048, 2048, 0, 0, hat)).unwrap();
        prop_assert!(r.hat <= 8, "hat should be 0–8, got {}", r.hat);
    }

    /// Button mask is always passed through unmodified (16-bit).
    #[test]
    fn prop_flightstick_buttons_roundtrip(buttons in any::<u16>()) {
        let r = parse_flightstick_report(&make_flightstick(2048, 2048, 2048, 0, buttons, 15)).unwrap();
        prop_assert_eq!(r.buttons, buttons, "button mask should round-trip");
    }

    /// Arbitrary valid-length reports must not panic.
    #[test]
    fn prop_flightstick_no_panic(
        data in proptest::collection::vec(any::<u8>(), FLIGHTSTICK_MIN_REPORT_BYTES..32),
    ) {
        let _ = parse_flightstick_report(&data);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Legacy Flightdeck (VID 0x1432) property tests
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    /// Legacy Flightdeck bipolar axes stay within [-1.0, 1.0].
    #[test]
    fn prop_flightdeck_axes_bounded(
        roll in 0u16..=u16::MAX,
        pitch in 0u16..=u16::MAX,
    ) {
        let r = parse_flightdeck_report(&make_flightdeck(roll, pitch, 0, 0, 0)).unwrap();
        prop_assert!((-1.0..=1.0).contains(&r.roll), "roll={}", r.roll);
        prop_assert!((-1.0..=1.0).contains(&r.pitch), "pitch={}", r.pitch);
    }

    /// Legacy Flightdeck throttle axes stay within [0.0, 1.0].
    #[test]
    fn prop_flightdeck_throttle_bounded(tl in 0u8..=255u8, tr in 0u8..=255u8) {
        let r = parse_flightdeck_report(&make_flightdeck(32767, 32767, tl, tr, 0)).unwrap();
        prop_assert!((0.0..=1.0).contains(&r.throttle_left), "throttle_left={}", r.throttle_left);
        prop_assert!((0.0..=1.0).contains(&r.throttle_right), "throttle_right={}", r.throttle_right);
    }

    /// Legacy Flightdeck reports shorter than minimum always fail.
    #[test]
    fn prop_flightdeck_short_errors(
        data in proptest::collection::vec(any::<u8>(), 0..FLIGHTDECK_MIN_REPORT_BYTES),
    ) {
        prop_assert!(parse_flightdeck_report(&data).is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Legacy Rudder (VID 0x1432) property tests
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    /// Legacy Rudder axis stays within [-1.0, 1.0].
    #[test]
    fn prop_rudder_axis_bounded(rudder in 0u16..=u16::MAX) {
        let r = parse_rudder_report(&make_rudder(rudder, 0, 0)).unwrap();
        prop_assert!((-1.0..=1.0).contains(&r.rudder), "rudder={}", r.rudder);
    }

    /// Legacy Rudder brake axes stay within [0.0, 1.0].
    #[test]
    fn prop_rudder_brakes_bounded(bl in 0u8..=255u8, br in 0u8..=255u8) {
        let r = parse_rudder_report(&make_rudder(32767, bl, br)).unwrap();
        prop_assert!((0.0..=1.0).contains(&r.brake_left), "brake_left={}", r.brake_left);
        prop_assert!((0.0..=1.0).contains(&r.brake_right), "brake_right={}", r.brake_right);
    }

    /// Legacy Rudder axes are always finite.
    #[test]
    fn prop_rudder_axes_finite(rudder in 0u16..=u16::MAX, bl in 0u8..=255u8, br in 0u8..=255u8) {
        let r = parse_rudder_report(&make_rudder(rudder, bl, br)).unwrap();
        prop_assert!(r.rudder.is_finite());
        prop_assert!(r.brake_left.is_finite());
        prop_assert!(r.brake_right.is_finite());
    }

    /// Legacy Rudder reports shorter than minimum always fail.
    #[test]
    fn prop_rudder_short_errors(
        data in proptest::collection::vec(any::<u8>(), 0..RUDDER_MIN_REPORT_BYTES),
    ) {
        prop_assert!(parse_rudder_report(&data).is_err());
    }

    /// Arbitrary valid-length rudder reports must not panic.
    #[test]
    fn prop_rudder_no_panic(
        data in proptest::collection::vec(any::<u8>(), RUDDER_MIN_REPORT_BYTES..32),
    ) {
        let _ = parse_rudder_report(&data);
    }
}
