// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth / integration tests for `flight-trackir`.
//!
//! These tests exercise cross-cutting behaviour that spans multiple components
//! (parsing → normalisation → filtering → adapter) and verify invariants that
//! the unit-level tests in `lib.rs` do not cover.

use flight_trackir::{
    normalize_pose, parse_packet, HeadPose, PoseFilter, TrackIrAdapter, TrackIrError,
    TrackIrPacket, PACKET_SIZE, TRACKIR_PORT,
};
use std::thread;
use std::time::Duration;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn build_packet(x: f64, y: f64, z: f64, yaw: f64, pitch: f64, roll: f64) -> Vec<u8> {
    let mut buf = vec![0u8; PACKET_SIZE];
    buf[0..8].copy_from_slice(&x.to_le_bytes());
    buf[8..16].copy_from_slice(&y.to_le_bytes());
    buf[16..24].copy_from_slice(&z.to_le_bytes());
    buf[24..32].copy_from_slice(&yaw.to_le_bytes());
    buf[32..40].copy_from_slice(&pitch.to_le_bytes());
    buf[40..48].copy_from_slice(&roll.to_le_bytes());
    buf
}

fn assert_f32_eq(a: f32, b: f32, eps: f32) {
    assert!((a - b).abs() < eps, "expected {a} ≈ {b} (eps = {eps})");
}

/// Build a zero-pose packet.
fn zero_packet() -> Vec<u8> {
    build_packet(0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
}

// =============================================================================
// § 1  Packet parsing — field-isolation & byte-level fidelity
// =============================================================================

#[test]
fn parse_isolates_each_field_to_correct_offset() {
    let data = build_packet(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
    let pkt = parse_packet(&data).unwrap();
    assert_eq!(pkt.x, 1.0);
    assert_eq!(pkt.y, 2.0);
    assert_eq!(pkt.z, 3.0);
    assert_eq!(pkt.yaw, 4.0);
    assert_eq!(pkt.pitch, 5.0);
    assert_eq!(pkt.roll, 6.0);
}

#[test]
fn parse_preserves_sign_bit_for_all_fields() {
    let data = build_packet(-1.0, -2.0, -3.0, -4.0, -5.0, -6.0);
    let pkt = parse_packet(&data).unwrap();
    assert!(pkt.x < 0.0);
    assert!(pkt.y < 0.0);
    assert!(pkt.z < 0.0);
    assert!(pkt.yaw < 0.0);
    assert!(pkt.pitch < 0.0);
    assert!(pkt.roll < 0.0);
}

#[test]
fn parse_handles_mixed_positive_negative() {
    let data = build_packet(10.0, -20.0, 30.0, -40.0, 50.0, -60.0);
    let pkt = parse_packet(&data).unwrap();
    assert!(pkt.x > 0.0);
    assert!(pkt.y < 0.0);
    assert!(pkt.z > 0.0);
    assert!(pkt.yaw < 0.0);
    assert!(pkt.pitch > 0.0);
    assert!(pkt.roll < 0.0);
}

#[test]
fn parse_every_short_length_rejected() {
    for len in 0..PACKET_SIZE {
        let data = vec![0u8; len];
        assert_eq!(
            parse_packet(&data),
            Err(TrackIrError::PacketTooShort { actual: len }),
            "expected PacketTooShort for len={len}"
        );
    }
}

#[test]
fn parse_accepts_lengths_ge_packet_size() {
    for extra in 0..=16 {
        let mut data = build_packet(1.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        data.extend(vec![0xFFu8; extra]);
        assert!(
            parse_packet(&data).is_ok(),
            "should accept len={}",
            PACKET_SIZE + extra
        );
    }
}

#[test]
fn parse_infinity_in_every_field_rejected() {
    let fields = [
        (f64::INFINITY, 0.0, 0.0, 0.0, 0.0, 0.0),
        (0.0, f64::INFINITY, 0.0, 0.0, 0.0, 0.0),
        (0.0, 0.0, f64::INFINITY, 0.0, 0.0, 0.0),
        (0.0, 0.0, 0.0, f64::INFINITY, 0.0, 0.0),
        (0.0, 0.0, 0.0, 0.0, f64::INFINITY, 0.0),
        (0.0, 0.0, 0.0, 0.0, 0.0, f64::INFINITY),
    ];
    for (i, &(x, y, z, yaw, pitch, roll)) in fields.iter().enumerate() {
        let data = build_packet(x, y, z, yaw, pitch, roll);
        assert_eq!(
            parse_packet(&data),
            Err(TrackIrError::NonFiniteValue),
            "infinity in field {i} should be rejected"
        );
    }
}

#[test]
fn parse_neg_infinity_in_every_field_rejected() {
    let fields = [
        (f64::NEG_INFINITY, 0.0, 0.0, 0.0, 0.0, 0.0),
        (0.0, f64::NEG_INFINITY, 0.0, 0.0, 0.0, 0.0),
        (0.0, 0.0, f64::NEG_INFINITY, 0.0, 0.0, 0.0),
        (0.0, 0.0, 0.0, f64::NEG_INFINITY, 0.0, 0.0),
        (0.0, 0.0, 0.0, 0.0, f64::NEG_INFINITY, 0.0),
        (0.0, 0.0, 0.0, 0.0, 0.0, f64::NEG_INFINITY),
    ];
    for (i, &(x, y, z, yaw, pitch, roll)) in fields.iter().enumerate() {
        let data = build_packet(x, y, z, yaw, pitch, roll);
        assert_eq!(
            parse_packet(&data),
            Err(TrackIrError::NonFiniteValue),
            "neg-infinity in field {i} should be rejected"
        );
    }
}

#[test]
fn parse_f64_max_is_finite_and_accepted() {
    let data = build_packet(f64::MAX, f64::MIN, f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    let pkt = parse_packet(&data).unwrap();
    assert_eq!(pkt.x, f64::MAX);
    assert_eq!(pkt.y, f64::MIN);
}

// =============================================================================
// § 2  Normalisation — boundary precision & clamping invariants
// =============================================================================

#[test]
fn normalize_boundary_values_exact() {
    let pose = normalize_pose(TrackIrPacket {
        x: 100.0,
        y: -100.0,
        z: 0.0,
        yaw: 180.0,
        pitch: -90.0,
        roll: 0.0,
    });
    assert_eq!(pose.x, 1.0);
    assert_eq!(pose.y, -1.0);
    assert_eq!(pose.z, 0.0);
    assert_eq!(pose.yaw, 1.0);
    assert_eq!(pose.pitch, -1.0);
    assert_eq!(pose.roll, 0.0);
}

#[test]
fn normalize_half_boundary_values() {
    let pose = normalize_pose(TrackIrPacket {
        x: 50.0,
        y: -50.0,
        z: 25.0,
        yaw: 90.0,
        pitch: -45.0,
        roll: 90.0,
    });
    assert_f32_eq(pose.x, 0.5, 1e-6);
    assert_f32_eq(pose.y, -0.5, 1e-6);
    assert_f32_eq(pose.z, 0.25, 1e-6);
    assert_f32_eq(pose.yaw, 0.5, 1e-6);
    assert_f32_eq(pose.pitch, -0.5, 1e-6);
    assert_f32_eq(pose.roll, 0.5, 1e-6);
}

#[test]
fn normalize_clamps_all_translations_symmetrically() {
    for mm in [200.0, 500.0, 1000.0, f64::MAX / 2.0] {
        let pos = normalize_pose(TrackIrPacket {
            x: mm,
            y: mm,
            z: mm,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        });
        let neg = normalize_pose(TrackIrPacket {
            x: -mm,
            y: -mm,
            z: -mm,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        });
        assert_eq!(pos.x, 1.0, "positive clamp at {mm}");
        assert_eq!(neg.x, -1.0, "negative clamp at {mm}");
        assert_eq!(pos.y, 1.0);
        assert_eq!(neg.z, -1.0);
    }
}

#[test]
fn normalize_clamps_all_rotations_symmetrically() {
    for deg in [360.0, 720.0, 10_000.0] {
        let pos = normalize_pose(TrackIrPacket {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: deg,
            pitch: deg,
            roll: deg,
        });
        let neg = normalize_pose(TrackIrPacket {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: -deg,
            pitch: -deg,
            roll: -deg,
        });
        assert_eq!(pos.yaw, 1.0, "yaw positive clamp at {deg}");
        assert_eq!(neg.yaw, -1.0, "yaw negative clamp at {deg}");
        assert_eq!(pos.pitch, 1.0);
        assert_eq!(neg.roll, -1.0);
    }
}

#[test]
fn normalize_tiny_values_near_zero() {
    let tiny = 1e-10;
    let pose = normalize_pose(TrackIrPacket {
        x: tiny,
        y: -tiny,
        z: tiny,
        yaw: tiny,
        pitch: -tiny,
        roll: tiny,
    });
    assert!(pose.x.abs() < 1e-6);
    assert!(pose.y.abs() < 1e-6);
    assert!(pose.yaw.abs() < 1e-6);
}

#[test]
fn normalize_negative_zero_produces_zero() {
    let pose = normalize_pose(TrackIrPacket {
        x: -0.0,
        y: -0.0,
        z: -0.0,
        yaw: -0.0,
        pitch: -0.0,
        roll: -0.0,
    });
    assert_eq!(pose.x, 0.0);
    assert_eq!(pose.yaw, 0.0);
}

#[test]
fn normalize_output_always_in_unit_range() {
    let values = [-1e6, -500.0, -100.0, -1.0, 0.0, 1.0, 100.0, 500.0, 1e6];
    for &x in &values {
        for &yaw in &values {
            let pose = normalize_pose(TrackIrPacket {
                x,
                y: 0.0,
                z: 0.0,
                yaw,
                pitch: 0.0,
                roll: 0.0,
            });
            assert!((-1.0..=1.0).contains(&pose.x), "x={x} → {}", pose.x);
            assert!(
                (-1.0..=1.0).contains(&pose.yaw),
                "yaw={yaw} → {}",
                pose.yaw
            );
        }
    }
}

#[test]
fn normalize_is_monotonic_within_range() {
    let mut prev_x = -2.0_f32;
    for i in -100..=100 {
        let mm = i as f64;
        let pose = normalize_pose(TrackIrPacket {
            x: mm,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        });
        assert!(pose.x >= prev_x, "monotonicity violated at x={mm}");
        prev_x = pose.x;
    }
}

// =============================================================================
// § 3  EMA filter — mathematical properties & edge cases
// =============================================================================

#[test]
fn filter_nan_alpha_defaults_to_passthrough() {
    let f = PoseFilter::new(f32::NAN);
    assert_eq!(f.alpha(), 1.0);
}

#[test]
fn filter_inf_alpha_defaults_to_passthrough() {
    let f = PoseFilter::new(f32::INFINITY);
    assert_eq!(f.alpha(), 1.0);
}

#[test]
fn filter_neg_inf_alpha_defaults_to_passthrough() {
    let f = PoseFilter::new(f32::NEG_INFINITY);
    assert_eq!(f.alpha(), 1.0);
}

#[test]
fn filter_impulse_response_decays() {
    let mut filter = PoseFilter::new(0.3);
    let impulse = HeadPose {
        x: 1.0,
        ..HeadPose::default()
    };
    let zero = HeadPose::default();

    let _ = filter.apply(impulse);
    let mut prev = 1.0_f32;
    for _ in 0..20 {
        let out = filter.apply(zero);
        assert!(out.x <= prev, "impulse response should decay");
        assert!(out.x >= 0.0, "no undershoot expected");
        prev = out.x;
    }
    assert!(prev < 0.01, "should converge near zero, got {prev}");
}

#[test]
fn filter_ema_exact_formula_two_steps() {
    let alpha = 0.4_f32;
    let mut filter = PoseFilter::new(alpha);

    let p0 = HeadPose {
        x: 0.0,
        ..HeadPose::default()
    };
    let p1 = HeadPose {
        x: 1.0,
        ..HeadPose::default()
    };
    let p2 = HeadPose {
        x: 0.5,
        ..HeadPose::default()
    };

    let out0 = filter.apply(p0);
    assert_f32_eq(out0.x, 0.0, 1e-6);

    let out1 = filter.apply(p1);
    // α*1.0 + (1-α)*0.0 = 0.4
    assert_f32_eq(out1.x, 0.4, 1e-6);

    let out2 = filter.apply(p2);
    // α*0.5 + (1-α)*0.4 = 0.2 + 0.24 = 0.44
    assert_f32_eq(out2.x, 0.44, 1e-5);
}

#[test]
fn filter_multiple_resets_are_idempotent() {
    let mut filter = PoseFilter::new(0.5);
    let p = HeadPose {
        x: 1.0,
        ..HeadPose::default()
    };
    let _ = filter.apply(p);

    filter.reset();
    filter.reset();
    filter.reset();

    let out = filter.apply(HeadPose {
        x: 0.7,
        ..HeadPose::default()
    });
    assert_f32_eq(out.x, 0.7, 1e-6);
}

#[test]
fn filter_alpha_one_always_equals_input() {
    let mut filter = PoseFilter::new(1.0);
    let samples = [0.1, 0.5, -0.3, 0.9, -1.0];
    for &val in &samples {
        let p = HeadPose {
            x: val,
            ..HeadPose::default()
        };
        let out = filter.apply(p);
        assert_f32_eq(out.x, val, 1e-6);
    }
}

#[test]
fn filter_alpha_zero_always_equals_first() {
    let mut filter = PoseFilter::new(0.0);
    let first = HeadPose {
        x: 0.42,
        ..HeadPose::default()
    };
    let _ = filter.apply(first);

    for val in [0.0, 1.0, -1.0, 0.5] {
        let p = HeadPose {
            x: val,
            ..HeadPose::default()
        };
        let out = filter.apply(p);
        assert_f32_eq(out.x, 0.42, 1e-6);
    }
}

#[test]
fn filter_convergence_rate_matches_theory() {
    let alpha = 0.2_f32;
    let mut filter = PoseFilter::new(alpha);
    let zero = HeadPose::default();
    let target = HeadPose {
        x: 1.0,
        ..HeadPose::default()
    };

    let _ = filter.apply(zero);

    let n = 10;
    let mut out = zero;
    for _ in 0..n {
        out = filter.apply(target);
    }

    let expected = 1.0 - (1.0 - alpha).powi(n);
    assert_f32_eq(out.x, expected, 1e-5);
}

#[test]
fn filter_output_bounded_between_inputs() {
    let mut filter = PoseFilter::new(0.6);
    let p1 = HeadPose {
        x: 0.2,
        y: -0.8,
        z: 0.5,
        yaw: -0.1,
        pitch: 0.9,
        roll: -0.3,
    };
    let p2 = HeadPose {
        x: 0.8,
        y: -0.2,
        z: 0.1,
        yaw: -0.9,
        pitch: 0.1,
        roll: -0.7,
    };

    let _ = filter.apply(p1);
    let out = filter.apply(p2);

    let check = |a: f32, b: f32, o: f32, name: &str| {
        let lo = a.min(b) - 1e-6;
        let hi = a.max(b) + 1e-6;
        assert!(o >= lo && o <= hi, "{name}: {o} not in [{lo}, {hi}]");
    };
    check(p1.x, p2.x, out.x, "x");
    check(p1.y, p2.y, out.y, "y");
    check(p1.z, p2.z, out.z, "z");
    check(p1.yaw, p2.yaw, out.yaw, "yaw");
    check(p1.pitch, p2.pitch, out.pitch, "pitch");
    check(p1.roll, p2.roll, out.roll, "roll");
}

// =============================================================================
// § 4  Adapter — full pipeline integration
// =============================================================================

#[test]
fn adapter_end_to_end_parse_normalize_smooth() {
    let mut adapter = TrackIrAdapter::with_smoothing(0.5);

    let d1 = zero_packet();
    let p1 = adapter.process_packet(&d1).unwrap();
    assert_eq!(p1, HeadPose::default());

    let d2 = build_packet(0.0, 0.0, 0.0, 180.0, 0.0, 0.0);
    let p2 = adapter.process_packet(&d2).unwrap();
    assert_f32_eq(p2.yaw, 0.5, 1e-6);

    let p3 = adapter.process_packet(&d2).unwrap();
    assert_f32_eq(p3.yaw, 0.75, 1e-6);
}

#[test]
fn adapter_error_does_not_update_timestamp() {
    let mut adapter = TrackIrAdapter::new();
    let valid = zero_packet();
    adapter.process_packet(&valid).unwrap();

    assert!(!adapter.is_stale(60_000));

    let _ = adapter.process_packet(&[0u8; 2]);
    assert!(!adapter.is_stale(60_000));
}

#[test]
fn adapter_multiple_errors_preserve_last_good_pose() {
    let mut adapter = TrackIrAdapter::new();
    let valid = build_packet(50.0, 0.0, 0.0, 90.0, 0.0, 0.0);
    let good = adapter.process_packet(&valid).unwrap();

    let _ = adapter.process_packet(&[]);
    let _ = adapter.process_packet(&[0u8; 10]);
    let _ = adapter.process_packet(&build_packet(f64::NAN, 0.0, 0.0, 0.0, 0.0, 0.0));
    let _ = adapter.process_packet(&build_packet(0.0, f64::INFINITY, 0.0, 0.0, 0.0, 0.0));

    assert_eq!(adapter.last_pose(), Some(good));
}

#[test]
fn adapter_staleness_lifecycle() {
    let mut adapter = TrackIrAdapter::new();

    // Phase 1: no data → stale.
    assert!(adapter.is_stale(1_000));

    // Phase 2: receive data → fresh.
    adapter.process_packet(&zero_packet()).unwrap();
    assert!(!adapter.is_stale(5_000));

    // Phase 3: wait → stale again.
    thread::sleep(Duration::from_millis(40));
    assert!(adapter.is_stale(10));

    // Phase 4: new data → fresh again.
    adapter.process_packet(&zero_packet()).unwrap();
    assert!(!adapter.is_stale(5_000));
}

#[test]
fn adapter_with_smoothing_first_packet_is_passthrough() {
    let mut adapter = TrackIrAdapter::with_smoothing(0.1);
    let data = build_packet(100.0, -100.0, 50.0, 180.0, -90.0, 180.0);
    let pose = adapter.process_packet(&data).unwrap();

    assert_f32_eq(pose.x, 1.0, 1e-6);
    assert_f32_eq(pose.y, -1.0, 1e-6);
    assert_f32_eq(pose.z, 0.5, 1e-6);
    assert_f32_eq(pose.yaw, 1.0, 1e-6);
    assert_f32_eq(pose.pitch, -1.0, 1e-6);
    assert_f32_eq(pose.roll, 1.0, 1e-6);
}

#[test]
fn adapter_rapid_update_convergence() {
    let mut adapter = TrackIrAdapter::with_smoothing(0.3);

    adapter.process_packet(&zero_packet()).unwrap();

    let target = build_packet(0.0, 0.0, 0.0, 180.0, 0.0, 0.0);
    let mut last = HeadPose::default();
    for _ in 0..50 {
        last = adapter.process_packet(&target).unwrap();
    }

    assert_f32_eq(last.yaw, 1.0, 1e-3);
}

#[test]
fn adapter_no_smoothing_gives_raw_normalised() {
    let mut adapter = TrackIrAdapter::new();
    let data = build_packet(33.0, -66.0, 99.0, 60.0, -30.0, 120.0);
    let pose = adapter.process_packet(&data).unwrap();

    assert_f32_eq(pose.x, 0.33, 1e-3);
    assert_f32_eq(pose.y, -0.66, 1e-3);
    assert_f32_eq(pose.z, 0.99, 1e-3);
    assert_f32_eq(pose.yaw, 60.0 / 180.0, 1e-5);
    assert_f32_eq(pose.pitch, -30.0 / 90.0, 1e-5);
    assert_f32_eq(pose.roll, 120.0 / 180.0, 1e-5);
}

#[test]
fn adapter_last_pose_none_before_first_packet() {
    let adapter = TrackIrAdapter::with_smoothing(0.5);
    assert!(adapter.last_pose().is_none());
}

#[test]
fn adapter_default_trait_produces_no_smoothing() {
    let mut def = TrackIrAdapter::default();
    let mut plain = TrackIrAdapter::new();

    let d1 = zero_packet();
    let d2 = build_packet(100.0, 0.0, 0.0, 180.0, 0.0, 0.0);

    let _ = def.process_packet(&d1).unwrap();
    let _ = plain.process_packet(&d1).unwrap();

    let p_def = def.process_packet(&d2).unwrap();
    let p_plain = plain.process_packet(&d2).unwrap();
    assert_eq!(p_def, p_plain);
}

// =============================================================================
// § 5  Error type properties
// =============================================================================

#[test]
fn error_partial_eq_same_variant() {
    let a = TrackIrError::PacketTooShort { actual: 5 };
    let b = TrackIrError::PacketTooShort { actual: 5 };
    assert_eq!(a, b);
}

#[test]
fn error_partial_eq_different_actual() {
    let a = TrackIrError::PacketTooShort { actual: 5 };
    let b = TrackIrError::PacketTooShort { actual: 10 };
    assert_ne!(a, b);
}

#[test]
fn error_partial_eq_different_variants() {
    let a = TrackIrError::PacketTooShort { actual: 0 };
    let b = TrackIrError::NonFiniteValue;
    assert_ne!(a, b);
}

#[test]
fn error_debug_contains_variant_name() {
    let err = TrackIrError::PacketTooShort { actual: 7 };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("PacketTooShort"), "got: {dbg}");
    assert!(dbg.contains("7"), "got: {dbg}");
}

#[test]
fn error_display_non_finite_is_stable() {
    assert_eq!(
        TrackIrError::NonFiniteValue.to_string(),
        "non-finite value in TrackIR packet"
    );
}

// =============================================================================
// § 6  HeadPose & TrackIrPacket type properties
// =============================================================================

#[test]
fn head_pose_copy_semantics() {
    let a = HeadPose {
        x: 0.1,
        y: 0.2,
        z: 0.3,
        yaw: 0.4,
        pitch: 0.5,
        roll: 0.6,
    };
    let b = a; // Copy
    let c = a; // Still valid after copy
    assert_eq!(b, c);
}

#[test]
fn head_pose_debug_includes_all_fields() {
    let pose = HeadPose {
        x: 0.1,
        y: 0.2,
        z: 0.3,
        yaw: 0.4,
        pitch: 0.5,
        roll: 0.6,
    };
    let dbg = format!("{pose:?}");
    for field in ["x:", "y:", "z:", "yaw:", "pitch:", "roll:"] {
        assert!(dbg.contains(field), "missing {field} in debug: {dbg}");
    }
}

#[test]
fn trackir_packet_partial_eq_field_sensitivity() {
    let base = TrackIrPacket {
        x: 1.0,
        y: 2.0,
        z: 3.0,
        yaw: 4.0,
        pitch: 5.0,
        roll: 6.0,
    };
    let same = base.clone();
    assert_eq!(base, same);

    let diff = TrackIrPacket {
        x: 1.0,
        y: 2.0,
        z: 3.0,
        yaw: 4.0,
        pitch: 5.0,
        roll: 6.001,
    };
    assert_ne!(base, diff);
}

// =============================================================================
// § 7  Constants
// =============================================================================

#[test]
fn trackir_port_value() {
    assert_eq!(TRACKIR_PORT, 4242);
}

#[test]
fn packet_size_is_six_f64s() {
    assert_eq!(PACKET_SIZE, 6 * std::mem::size_of::<f64>());
}

// =============================================================================
// § 8  Full round-trip integration (parse → normalize → filter → adapter)
// =============================================================================

#[test]
fn full_pipeline_clamped_extremes() {
    let mut adapter = TrackIrAdapter::new();
    let data = build_packet(999.0, -999.0, 999.0, 999.0, -999.0, 999.0);
    let pose = adapter.process_packet(&data).unwrap();

    assert_eq!(pose.x, 1.0);
    assert_eq!(pose.y, -1.0);
    assert_eq!(pose.z, 1.0);
    assert_eq!(pose.yaw, 1.0);
    assert_eq!(pose.pitch, -1.0);
    assert_eq!(pose.roll, 1.0);
}

#[test]
fn full_pipeline_smoothed_step_response_all_axes() {
    let alpha = 0.5_f32;
    let mut adapter = TrackIrAdapter::with_smoothing(alpha);

    adapter.process_packet(&zero_packet()).unwrap();

    let full = build_packet(100.0, -100.0, 100.0, 180.0, -90.0, 180.0);
    let pose = adapter.process_packet(&full).unwrap();

    assert_f32_eq(pose.x, 0.5, 1e-6);
    assert_f32_eq(pose.y, -0.5, 1e-6);
    assert_f32_eq(pose.z, 0.5, 1e-6);
    assert_f32_eq(pose.yaw, 0.5, 1e-6);
    assert_f32_eq(pose.pitch, -0.5, 1e-6);
    assert_f32_eq(pose.roll, 0.5, 1e-6);
}

#[test]
fn full_pipeline_oscillating_input_stays_bounded() {
    let mut adapter = TrackIrAdapter::with_smoothing(0.4);

    for i in 0..100 {
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        let data = build_packet(
            sign * 100.0,
            sign * 100.0,
            sign * 100.0,
            sign * 180.0,
            sign * 90.0,
            sign * 180.0,
        );
        let pose = adapter.process_packet(&data).unwrap();
        assert!((-1.0..=1.0).contains(&pose.x));
        assert!((-1.0..=1.0).contains(&pose.y));
        assert!((-1.0..=1.0).contains(&pose.z));
        assert!((-1.0..=1.0).contains(&pose.yaw));
        assert!((-1.0..=1.0).contains(&pose.pitch));
        assert!((-1.0..=1.0).contains(&pose.roll));
    }
}

#[test]
fn full_pipeline_many_sequential_packets_last_pose_correct() {
    let mut adapter = TrackIrAdapter::new();
    let mut expected = HeadPose::default();

    for i in 0..20 {
        let mm = (i as f64) * 5.0;
        let deg = (i as f64) * 9.0;
        let data = build_packet(mm, -mm, mm, deg, -deg / 2.0, deg);
        expected = adapter.process_packet(&data).unwrap();
    }

    assert_eq!(adapter.last_pose(), Some(expected));
}
