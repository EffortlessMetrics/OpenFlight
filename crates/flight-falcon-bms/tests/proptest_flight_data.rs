// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Property-based tests for `FlightData` invariants using proptest.
//!
//! Covers:
//! - Normalised outputs always within expected bounds (ignoring NaN)
//! - Byte round-trip identity for arbitrary valid structs
//! - Normalisation is idempotent on clamped values
//! - Throttle normalisation monotonicity on valid inputs

use bytemuck::{bytes_of, try_from_bytes, Zeroable};
use flight_falcon_bms::FlightData;
use proptest::prelude::*;

// ── Strategies ──────────────────────────────────────────────────────────────

/// Generate a finite (non-NaN, non-Inf) f32 in a wide range.
fn finite_f32() -> impl Strategy<Value = f32> {
    prop_oneof![
        prop::num::f32::NORMAL,
        Just(0.0f32),
        Just(-0.0f32),
        Just(f32::MIN_POSITIVE),
    ]
}

/// Generate a FlightData with all finite float fields.
fn arb_flight_data() -> impl Strategy<Value = FlightData> {
    // proptest tuples max at 12 elements, so we split into two groups
    let group_a = (
        finite_f32(), // x
        finite_f32(), // y
        finite_f32(), // z
        finite_f32(), // x_dot
        finite_f32(), // y_dot
        finite_f32(), // z_dot
        finite_f32(), // alpha
        finite_f32(), // beta
        finite_f32(), // gamma
    );
    let group_b = (
        finite_f32(), // pitch
        finite_f32(), // roll
        finite_f32(), // yaw
        finite_f32(), // mach
        finite_f32(), // cas
        finite_f32(), // alt
        finite_f32(), // throttle
        finite_f32(), // rpm
    );
    (group_a, group_b).prop_map(
        |((x, y, z, x_dot, y_dot, z_dot, alpha, beta, gamma), (pitch, roll, yaw, mach, cas, alt, throttle, rpm))| {
            let mut fd = FlightData::zeroed();
            fd.x = x;
            fd.y = y;
            fd.z = z;
            fd.x_dot = x_dot;
            fd.y_dot = y_dot;
            fd.z_dot = z_dot;
            fd.alpha = alpha;
            fd.beta = beta;
            fd.gamma = gamma;
            fd.pitch = pitch;
            fd.roll = roll;
            fd.yaw = yaw;
            fd.mach = mach;
            fd.cas = cas;
            fd.alt = alt;
            fd.throttle = throttle;
            fd.rpm = rpm;
            fd
        },
    )
}

// ── Normalisation bounds ────────────────────────────────────────────────────

proptest! {
    #[test]
    fn pitch_normalized_in_bounds(fd in arb_flight_data()) {
        let n = fd.pitch_normalized();
        if !n.is_nan() {
            prop_assert!((-1.0..=1.0).contains(&n), "pitch_normalized={n} out of [-1,1]");
        }
    }

    #[test]
    fn roll_normalized_in_bounds(fd in arb_flight_data()) {
        let n = fd.roll_normalized();
        if !n.is_nan() {
            prop_assert!((-1.0..=1.0).contains(&n), "roll_normalized={n} out of [-1,1]");
        }
    }

    #[test]
    fn yaw_normalized_in_bounds(fd in arb_flight_data()) {
        let n = fd.yaw_normalized();
        if !n.is_nan() {
            prop_assert!((-1.0..=1.0).contains(&n), "yaw_normalized={n} out of [-1,1]");
        }
    }

    #[test]
    fn throttle_normalized_in_bounds(fd in arb_flight_data()) {
        let n = fd.throttle_normalized();
        if !n.is_nan() {
            prop_assert!((0.0..=1.0).contains(&n), "throttle_normalized={n} out of [0,1]");
        }
    }
}

// ── Byte round-trip identity ────────────────────────────────────────────────

proptest! {
    #[test]
    fn byte_round_trip_preserves_fields(fd in arb_flight_data()) {
        let bytes = bytes_of(&fd);
        let restored: &FlightData = try_from_bytes(bytes).expect("round-trip must succeed");
        prop_assert_eq!(fd.x.to_bits(), restored.x.to_bits());
        prop_assert_eq!(fd.y.to_bits(), restored.y.to_bits());
        prop_assert_eq!(fd.z.to_bits(), restored.z.to_bits());
        prop_assert_eq!(fd.pitch.to_bits(), restored.pitch.to_bits());
        prop_assert_eq!(fd.roll.to_bits(), restored.roll.to_bits());
        prop_assert_eq!(fd.yaw.to_bits(), restored.yaw.to_bits());
        prop_assert_eq!(fd.throttle.to_bits(), restored.throttle.to_bits());
        prop_assert_eq!(fd.mach.to_bits(), restored.mach.to_bits());
        prop_assert_eq!(fd.cas.to_bits(), restored.cas.to_bits());
        prop_assert_eq!(fd.alt.to_bits(), restored.alt.to_bits());
        prop_assert_eq!(fd.rpm.to_bits(), restored.rpm.to_bits());
    }
}

// ── Normalisation idempotency ───────────────────────────────────────────────

proptest! {
    /// If we take the normalised output and feed it back (scaled to the raw
    /// domain), the result should be the same normalised value.
    #[test]
    fn pitch_normalised_then_renormalised_is_same(raw_pitch in -10.0f32..10.0) {
        let mut fd = FlightData::zeroed();
        fd.pitch = raw_pitch;
        let n1 = fd.pitch_normalized();

        // Re-encode: normalised value * PI back into radians
        fd.pitch = n1 * std::f32::consts::PI;
        let n2 = fd.pitch_normalized();

        if !n1.is_nan() && !n2.is_nan() {
            prop_assert!((n1 - n2).abs() < 1e-5, "n1={n1}, n2={n2}");
        }
    }

    #[test]
    fn throttle_normalised_then_renormalised_is_same(raw_throttle in -5.0f32..5.0) {
        let mut fd = FlightData::zeroed();
        fd.throttle = raw_throttle;
        let n1 = fd.throttle_normalized();

        fd.throttle = n1;
        let n2 = fd.throttle_normalized();

        if !n1.is_nan() && !n2.is_nan() {
            prop_assert!((n1 - n2).abs() < 1e-5, "n1={n1}, n2={n2}");
        }
    }
}

// ── Throttle monotonicity ───────────────────────────────────────────────────

proptest! {
    #[test]
    fn throttle_normalised_is_monotone(a in -2.0f32..2.0, b in -2.0f32..2.0) {
        let mut fd_a = FlightData::zeroed();
        fd_a.throttle = a;
        let mut fd_b = FlightData::zeroed();
        fd_b.throttle = b;

        let na = fd_a.throttle_normalized();
        let nb = fd_b.throttle_normalized();

        if a <= b {
            prop_assert!(na <= nb, "monotonicity violated: {a} <= {b} but {na} > {nb}");
        } else {
            prop_assert!(na >= nb, "monotonicity violated: {a} > {b} but {na} < {nb}");
        }
    }
}

// ── Arbitrary bytes → FlightData acceptance ─────────────────────────────────

proptest! {
    /// Any 768-byte slice should be parseable as FlightData (bytemuck Pod).
    #[test]
    fn any_exact_size_bytes_parse(bytes in prop::collection::vec(any::<u8>(), 768)) {
        let result = try_from_bytes::<FlightData>(&bytes);
        prop_assert!(result.is_ok(), "any 768-byte slice must parse as FlightData");
    }

    /// Wrong-size slices must be rejected.
    #[test]
    fn wrong_size_bytes_rejected(len in 0usize..2000) {
        prop_assume!(len != 768);
        let bytes = vec![0u8; len];
        let result = try_from_bytes::<FlightData>(&bytes);
        prop_assert!(result.is_err(), "size {len} must be rejected");
    }
}
