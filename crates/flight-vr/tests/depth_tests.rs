// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the `flight-vr` crate.
//!
//! Covers: head-pose parsing/normalization, 6DOF data handling, controller
//! input mapping via `MockVrBackend`, runtime detection, error paths,
//! clone/debug round-trips, and property-based tests with `proptest`.

use flight_vr::adapter::VrBackend;
use flight_vr::mock::make_snapshot;
use flight_vr::{HeadPose, MockVrBackend, TrackingQuality, VrAdapter, VrError, VrSnapshot};
use proptest::prelude::*;

// ── Helpers ──────────────────────────────────────────────────────────────

fn full_pose(x: f32, y: f32, z: f32, yaw: f32, pitch: f32, roll: f32) -> HeadPose {
    HeadPose {
        x,
        y,
        z,
        yaw,
        pitch,
        roll,
    }
}

fn snapshot_with_pose(pose: HeadPose, quality: TrackingQuality, is_worn: bool) -> VrSnapshot {
    VrSnapshot {
        pose,
        quality,
        is_worn,
    }
}

// ── HeadPose::zero ───────────────────────────────────────────────────────

#[test]
fn zero_pose_has_all_fields_zero() {
    let p = HeadPose::zero();
    assert_eq!((p.x, p.y, p.z, p.yaw, p.pitch, p.roll), (0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
}

#[test]
fn zero_pose_normalize_is_identity() {
    let p = HeadPose::zero();
    let n = p.normalize();
    assert_eq!(n.yaw, 0.0);
    assert_eq!(n.pitch, 0.0);
    assert_eq!(n.roll, 0.0);
}

// ── HeadPose::normalize — specific angles ────────────────────────────────

#[test]
fn normalize_positive_overflow() {
    let p = full_pose(1.0, 2.0, 3.0, 270.0, 360.0, 540.0);
    let n = p.normalize();
    assert!((n.yaw - (-90.0)).abs() < 1e-4);
    assert!(n.pitch.abs() < 1e-4); // 360 → 0
    assert!((n.roll - 180.0).abs() < 1e-4); // 540 → 180
}

#[test]
fn normalize_negative_overflow() {
    let p = full_pose(0.0, 0.0, 0.0, -270.0, -360.0, -540.0);
    let n = p.normalize();
    assert!((n.yaw - 90.0).abs() < 1e-4);
    assert!(n.pitch.abs() < 1e-4);
    // -540 % 360 = -180, which wraps to +180 via the +360 branch
    assert!((n.roll - 180.0).abs() < 1e-4);
}

#[test]
fn normalize_preserves_position() {
    let p = full_pose(10.0, -5.5, 3.14, 400.0, -400.0, 720.0);
    let n = p.normalize();
    assert_eq!(n.x, 10.0);
    assert_eq!(n.y, -5.5);
    assert_eq!(n.z, 3.14);
}

#[test]
fn normalize_in_range_is_identity() {
    let p = full_pose(0.0, 0.0, 0.0, 45.0, -30.0, 179.0);
    let n = p.normalize();
    assert!((n.yaw - 45.0).abs() < 1e-6);
    assert!((n.pitch - (-30.0)).abs() < 1e-6);
    assert!((n.roll - 179.0).abs() < 1e-6);
}

#[test]
fn normalize_exactly_180() {
    let p = full_pose(0.0, 0.0, 0.0, 180.0, -180.0, 180.0);
    let n = p.normalize();
    // 180 stays 180; -180 wraps to 180
    assert!((n.yaw - 180.0).abs() < 1e-4);
    assert!((n.pitch - 180.0).abs() < 1e-4);
    assert!((n.roll - 180.0).abs() < 1e-4);
}

#[test]
fn normalize_large_multiples() {
    let p = full_pose(0.0, 0.0, 0.0, 3600.0, -3600.0, 1080.0);
    let n = p.normalize();
    assert!(n.yaw.abs() < 1e-3);
    assert!(n.pitch.abs() < 1e-3);
    assert!(n.roll.abs() < 1e-3);
}

#[test]
fn normalize_small_negative_angle() {
    let p = full_pose(0.0, 0.0, 0.0, -1.0, -0.5, -179.0);
    let n = p.normalize();
    assert!((n.yaw - (-1.0)).abs() < 1e-6);
    assert!((n.pitch - (-0.5)).abs() < 1e-6);
    assert!((n.roll - (-179.0)).abs() < 1e-6);
}

// ── 6DOF data handling ───────────────────────────────────────────────────

#[test]
fn six_dof_all_axes_independent() {
    let p = full_pose(1.0, 2.0, 3.0, 90.0, -45.0, 15.0);
    assert_eq!(p.x, 1.0);
    assert_eq!(p.y, 2.0);
    assert_eq!(p.z, 3.0);
    assert_eq!(p.yaw, 90.0);
    assert_eq!(p.pitch, -45.0);
    assert_eq!(p.roll, 15.0);
}

#[test]
fn six_dof_extreme_positions() {
    let p = full_pose(f32::MAX, f32::MIN, 0.0, 0.0, 0.0, 0.0);
    assert_eq!(p.x, f32::MAX);
    assert_eq!(p.y, f32::MIN);
}

#[test]
fn six_dof_negative_position() {
    let p = full_pose(-1.5, -2.5, -3.5, 0.0, 0.0, 0.0);
    assert_eq!(p.x, -1.5);
    assert_eq!(p.y, -2.5);
    assert_eq!(p.z, -3.5);
}

// ── TrackingQuality ──────────────────────────────────────────────────────

#[test]
fn tracking_quality_good_equality() {
    assert_eq!(TrackingQuality::Good, TrackingQuality::Good);
}

#[test]
fn tracking_quality_degraded_equality() {
    assert_eq!(TrackingQuality::Degraded, TrackingQuality::Degraded);
}

#[test]
fn tracking_quality_lost_equality() {
    assert_eq!(TrackingQuality::Lost, TrackingQuality::Lost);
}

#[test]
fn tracking_quality_variants_differ() {
    assert_ne!(TrackingQuality::Good, TrackingQuality::Degraded);
    assert_ne!(TrackingQuality::Good, TrackingQuality::Lost);
    assert_ne!(TrackingQuality::Degraded, TrackingQuality::Lost);
}

// ── VrSnapshot construction & cloning ────────────────────────────────────

#[test]
fn snapshot_clone_preserves_fields() {
    let snap = snapshot_with_pose(full_pose(1.0, 2.0, 3.0, 45.0, 0.0, 0.0), TrackingQuality::Good, true);
    let cloned = snap.clone();
    assert_eq!(cloned.pose, snap.pose);
    assert_eq!(cloned.quality, snap.quality);
    assert_eq!(cloned.is_worn, snap.is_worn);
}

#[test]
fn snapshot_debug_format_contains_fields() {
    let snap = snapshot_with_pose(HeadPose::zero(), TrackingQuality::Lost, false);
    let dbg = format!("{snap:?}");
    assert!(dbg.contains("Lost"));
    assert!(dbg.contains("is_worn: false"));
}

#[test]
fn head_pose_debug_format_contains_fields() {
    let p = full_pose(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
    let dbg = format!("{p:?}");
    assert!(dbg.contains("yaw: 4.0"));
    assert!(dbg.contains("pitch: 5.0"));
    assert!(dbg.contains("roll: 6.0"));
}

#[test]
fn head_pose_copy_semantics() {
    let p = full_pose(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
    let p2 = p; // Copy
    assert_eq!(p.x, p2.x);
    assert_eq!(p.yaw, p2.yaw);
}

// ── make_snapshot helper ─────────────────────────────────────────────────

#[test]
fn make_snapshot_sets_yaw_only() {
    let snap = make_snapshot(90.0, TrackingQuality::Good, true);
    assert_eq!(snap.pose.yaw, 90.0);
    assert_eq!(snap.pose.pitch, 0.0);
    assert_eq!(snap.pose.roll, 0.0);
    assert_eq!(snap.pose.x, 0.0);
    assert_eq!(snap.pose.y, 0.0);
    assert_eq!(snap.pose.z, 0.0);
    assert!(snap.is_worn);
}

#[test]
fn make_snapshot_not_worn() {
    let snap = make_snapshot(0.0, TrackingQuality::Degraded, false);
    assert!(!snap.is_worn);
    assert_eq!(snap.quality, TrackingQuality::Degraded);
}

// ── MockVrBackend — connected ────────────────────────────────────────────

#[test]
fn mock_connected_backend_reports_connected() {
    let backend = MockVrBackend::new_connected(vec![make_snapshot(0.0, TrackingQuality::Good, true)]);
    assert!(backend.is_connected());
}

#[test]
fn mock_connected_backend_name() {
    let backend = MockVrBackend::new_connected(vec![make_snapshot(0.0, TrackingQuality::Good, true)]);
    assert_eq!(backend.backend_name(), "MockVrBackend");
}

#[test]
fn mock_connected_cycles_through_sequence() {
    let snaps = vec![
        make_snapshot(10.0, TrackingQuality::Good, true),
        make_snapshot(20.0, TrackingQuality::Degraded, true),
    ];
    let mut backend = MockVrBackend::new_connected(snaps);
    assert_eq!(backend.poll().unwrap().pose.yaw, 10.0);
    assert_eq!(backend.poll().unwrap().pose.yaw, 20.0);
    // Wraps back to start
    assert_eq!(backend.poll().unwrap().pose.yaw, 10.0);
}

#[test]
fn mock_connected_empty_sequence_returns_error() {
    let mut backend = MockVrBackend::new_connected(vec![]);
    let err = backend.poll().unwrap_err();
    assert!(matches!(err, VrError::PollFailed(_)));
}

#[test]
fn mock_connected_single_element_repeats() {
    let snaps = vec![make_snapshot(42.0, TrackingQuality::Good, true)];
    let mut backend = MockVrBackend::new_connected(snaps);
    for _ in 0..10 {
        assert_eq!(backend.poll().unwrap().pose.yaw, 42.0);
    }
}

// ── MockVrBackend — disconnected ─────────────────────────────────────────

#[test]
fn mock_disconnected_reports_not_connected() {
    let backend = MockVrBackend::new_disconnected();
    assert!(!backend.is_connected());
}

#[test]
fn mock_disconnected_backend_name() {
    let backend = MockVrBackend::new_disconnected();
    assert_eq!(backend.backend_name(), "MockVrBackend(disconnected)");
}

#[test]
fn mock_disconnected_poll_returns_error() {
    let mut backend = MockVrBackend::new_disconnected();
    // Disconnected mock has empty sequence → PollFailed
    assert!(backend.poll().is_err());
}

// ── VrAdapter — update & caching ─────────────────────────────────────────

#[test]
fn adapter_initial_state_no_snapshot() {
    let adapter = VrAdapter::new(MockVrBackend::new_connected(vec![
        make_snapshot(0.0, TrackingQuality::Good, true),
    ]));
    assert!(adapter.last_snapshot().is_none());
    assert!(!adapter.is_active());
}

#[test]
fn adapter_update_caches_snapshot() {
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(vec![
        make_snapshot(55.0, TrackingQuality::Good, true),
    ]));
    adapter.update().unwrap();
    let cached = adapter.last_snapshot().unwrap();
    assert_eq!(cached.pose.yaw, 55.0);
}

#[test]
fn adapter_update_replaces_cached_snapshot() {
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(vec![
        make_snapshot(10.0, TrackingQuality::Good, true),
        make_snapshot(20.0, TrackingQuality::Good, true),
    ]));
    adapter.update().unwrap();
    assert_eq!(adapter.last_snapshot().unwrap().pose.yaw, 10.0);
    adapter.update().unwrap();
    assert_eq!(adapter.last_snapshot().unwrap().pose.yaw, 20.0);
}

#[test]
fn adapter_disconnected_returns_not_connected() {
    let mut adapter = VrAdapter::new(MockVrBackend::new_disconnected());
    let err = adapter.update().unwrap_err();
    assert!(matches!(err, VrError::NotConnected));
}

#[test]
fn adapter_disconnected_does_not_cache() {
    let mut adapter = VrAdapter::new(MockVrBackend::new_disconnected());
    let _ = adapter.update();
    assert!(adapter.last_snapshot().is_none());
}

#[test]
fn adapter_is_active_after_successful_poll() {
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(vec![
        make_snapshot(0.0, TrackingQuality::Good, true),
    ]));
    adapter.update().unwrap();
    assert!(adapter.is_active());
}

#[test]
fn adapter_handles_poll_failure_on_empty_seq() {
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(vec![]));
    let err = adapter.update().unwrap_err();
    assert!(matches!(err, VrError::PollFailed(_)));
}

// ── VrError — display & matching ─────────────────────────────────────────

#[test]
fn vr_error_not_connected_display() {
    let err = VrError::NotConnected;
    assert_eq!(format!("{err}"), "Backend not connected");
}

#[test]
fn vr_error_poll_failed_display() {
    let err = VrError::PollFailed("timeout".to_owned());
    assert_eq!(format!("{err}"), "Poll failed: timeout");
}

#[test]
fn vr_error_invalid_pose_display() {
    let err = VrError::InvalidPose;
    assert_eq!(format!("{err}"), "Invalid pose data");
}

#[test]
fn vr_error_debug_format() {
    let err = VrError::NotConnected;
    let dbg = format!("{err:?}");
    assert!(dbg.contains("NotConnected"));
}

#[test]
fn vr_error_poll_failed_contains_reason() {
    let err = VrError::PollFailed("device lost".to_owned());
    let display = format!("{err}");
    assert!(display.contains("device lost"));
}

// ── Adapter with varying quality/worn states ─────────────────────────────

#[test]
fn adapter_degraded_quality_snapshot() {
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(vec![
        make_snapshot(0.0, TrackingQuality::Degraded, true),
    ]));
    let snap = adapter.update().unwrap();
    assert_eq!(snap.quality, TrackingQuality::Degraded);
}

#[test]
fn adapter_lost_quality_snapshot() {
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(vec![
        make_snapshot(0.0, TrackingQuality::Lost, false),
    ]));
    let snap = adapter.update().unwrap();
    assert_eq!(snap.quality, TrackingQuality::Lost);
    assert!(!snap.is_worn);
}

#[test]
fn adapter_quality_transitions_through_sequence() {
    let snaps = vec![
        make_snapshot(0.0, TrackingQuality::Good, true),
        make_snapshot(0.0, TrackingQuality::Degraded, true),
        make_snapshot(0.0, TrackingQuality::Lost, false),
    ];
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(snaps));
    assert_eq!(adapter.update().unwrap().quality, TrackingQuality::Good);
    assert_eq!(adapter.update().unwrap().quality, TrackingQuality::Degraded);
    assert_eq!(adapter.update().unwrap().quality, TrackingQuality::Lost);
}

// ── Full 6DOF snapshot through adapter ───────────────────────────────────

#[test]
fn adapter_full_6dof_snapshot() {
    let pose = full_pose(0.5, -1.2, 3.0, 45.0, -10.0, 5.0);
    let snap = snapshot_with_pose(pose, TrackingQuality::Good, true);
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(vec![snap]));
    let result = adapter.update().unwrap();
    assert_eq!(result.pose.x, 0.5);
    assert_eq!(result.pose.y, -1.2);
    assert_eq!(result.pose.z, 3.0);
    assert_eq!(result.pose.yaw, 45.0);
    assert_eq!(result.pose.pitch, -10.0);
    assert_eq!(result.pose.roll, 5.0);
}

#[test]
fn adapter_6dof_with_normalization() {
    let pose = full_pose(1.0, 2.0, 3.0, 400.0, -400.0, 900.0);
    let snap = snapshot_with_pose(pose, TrackingQuality::Good, true);
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(vec![snap]));
    let result = adapter.update().unwrap();
    let normalized = result.pose.normalize();
    assert!((normalized.yaw - 40.0).abs() < 1e-3);
    assert!((normalized.pitch - (-40.0)).abs() < 1e-3);
    assert!((normalized.roll - 180.0).abs() < 1e-3);
    // Position unchanged
    assert_eq!(normalized.x, 1.0);
    assert_eq!(normalized.y, 2.0);
    assert_eq!(normalized.z, 3.0);
}

// ── Multiple updates stress ──────────────────────────────────────────────

#[test]
fn adapter_many_updates_cycles_correctly() {
    let snaps = vec![
        make_snapshot(0.0, TrackingQuality::Good, true),
        make_snapshot(90.0, TrackingQuality::Good, true),
        make_snapshot(180.0, TrackingQuality::Good, true),
    ];
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(snaps));
    let expected = [0.0, 90.0, 180.0];
    for cycle in 0..5 {
        for (i, &exp_yaw) in expected.iter().enumerate() {
            let snap = adapter.update().unwrap();
            assert_eq!(
                snap.pose.yaw, exp_yaw,
                "cycle {cycle}, step {i}: expected yaw {exp_yaw}, got {}",
                snap.pose.yaw
            );
        }
    }
}

// ── VrBackend trait via MockVrBackend (controller input mapping proxy) ────

#[test]
fn backend_trait_poll_returns_snapshot() {
    let mut backend = MockVrBackend::new_connected(vec![
        make_snapshot(33.0, TrackingQuality::Good, true),
    ]);
    let snap = backend.poll().unwrap();
    assert_eq!(snap.pose.yaw, 33.0);
}

#[test]
fn backend_trait_is_connected() {
    let conn = MockVrBackend::new_connected(vec![make_snapshot(0.0, TrackingQuality::Good, true)]);
    let disc = MockVrBackend::new_disconnected();
    assert!(conn.is_connected());
    assert!(!disc.is_connected());
}

#[test]
fn backend_trait_backend_name() {
    let backend = MockVrBackend::new_connected(vec![make_snapshot(0.0, TrackingQuality::Good, true)]);
    assert!(!backend.backend_name().is_empty());
}

// ── HeadPose equality ────────────────────────────────────────────────────

#[test]
fn head_pose_partial_eq() {
    let a = full_pose(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
    let b = full_pose(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
    assert_eq!(a, b);
}

#[test]
fn head_pose_partial_ne() {
    let a = full_pose(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
    let b = full_pose(1.0, 2.0, 3.0, 4.0, 5.0, 7.0);
    assert_ne!(a, b);
}

// ── Property-based tests ─────────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_normalize_yaw_in_range(yaw in -10000.0_f32..10000.0) {
        let p = full_pose(0.0, 0.0, 0.0, yaw, 0.0, 0.0);
        let n = p.normalize();
        prop_assert!(n.yaw > -180.0 && n.yaw <= 180.0,
            "yaw {} normalized to {} which is out of (-180, 180]", yaw, n.yaw);
    }

    #[test]
    fn prop_normalize_pitch_in_range(pitch in -10000.0_f32..10000.0) {
        let p = full_pose(0.0, 0.0, 0.0, 0.0, pitch, 0.0);
        let n = p.normalize();
        prop_assert!(n.pitch > -180.0 && n.pitch <= 180.0,
            "pitch {} normalized to {} which is out of (-180, 180]", pitch, n.pitch);
    }

    #[test]
    fn prop_normalize_roll_in_range(roll in -10000.0_f32..10000.0) {
        let p = full_pose(0.0, 0.0, 0.0, 0.0, 0.0, roll);
        let n = p.normalize();
        prop_assert!(n.roll > -180.0 && n.roll <= 180.0,
            "roll {} normalized to {} which is out of (-180, 180]", roll, n.roll);
    }

    #[test]
    fn prop_normalize_preserves_position(
        x in -1000.0_f32..1000.0,
        y in -1000.0_f32..1000.0,
        z in -1000.0_f32..1000.0,
        yaw in -10000.0_f32..10000.0,
    ) {
        let p = full_pose(x, y, z, yaw, 0.0, 0.0);
        let n = p.normalize();
        prop_assert_eq!(n.x, x);
        prop_assert_eq!(n.y, y);
        prop_assert_eq!(n.z, z);
    }

    #[test]
    fn prop_double_normalize_is_idempotent(
        yaw in -10000.0_f32..10000.0,
        pitch in -10000.0_f32..10000.0,
        roll in -10000.0_f32..10000.0,
    ) {
        let p = full_pose(0.0, 0.0, 0.0, yaw, pitch, roll);
        let n1 = p.normalize();
        let n2 = n1.normalize();
        prop_assert!((n1.yaw - n2.yaw).abs() < 1e-4,
            "double normalize: yaw {} → {} → {}", yaw, n1.yaw, n2.yaw);
        prop_assert!((n1.pitch - n2.pitch).abs() < 1e-4,
            "double normalize: pitch {} → {} → {}", pitch, n1.pitch, n2.pitch);
        prop_assert!((n1.roll - n2.roll).abs() < 1e-4,
            "double normalize: roll {} → {} → {}", roll, n1.roll, n2.roll);
    }

    #[test]
    fn prop_zero_pose_is_fixed_point(
        yaw in Just(0.0_f32),
        pitch in Just(0.0_f32),
        roll in Just(0.0_f32),
    ) {
        let p = full_pose(0.0, 0.0, 0.0, yaw, pitch, roll);
        let n = p.normalize();
        prop_assert_eq!(n.yaw, 0.0);
        prop_assert_eq!(n.pitch, 0.0);
        prop_assert_eq!(n.roll, 0.0);
    }

    #[test]
    fn prop_clone_equals_original(
        x in -100.0_f32..100.0,
        y in -100.0_f32..100.0,
        z in -100.0_f32..100.0,
        yaw in -180.0_f32..180.0,
        pitch in -90.0_f32..90.0,
        roll in -180.0_f32..180.0,
    ) {
        let p = full_pose(x, y, z, yaw, pitch, roll);
        let p2 = p;
        prop_assert_eq!(p.x, p2.x);
        prop_assert_eq!(p.y, p2.y);
        prop_assert_eq!(p.z, p2.z);
        prop_assert_eq!(p.yaw, p2.yaw);
        prop_assert_eq!(p.pitch, p2.pitch);
        prop_assert_eq!(p.roll, p2.roll);
    }

    #[test]
    fn prop_snapshot_clone_matches(yaw in -180.0_f32..180.0) {
        let snap = make_snapshot(yaw, TrackingQuality::Good, true);
        let cloned = snap.clone();
        prop_assert_eq!(cloned.pose.yaw, snap.pose.yaw);
        prop_assert_eq!(cloned.quality, snap.quality);
        prop_assert_eq!(cloned.is_worn, snap.is_worn);
    }

    #[test]
    fn prop_adapter_always_caches_on_success(yaw in -180.0_f32..180.0) {
        let snap = make_snapshot(yaw, TrackingQuality::Good, true);
        let mut adapter = VrAdapter::new(MockVrBackend::new_connected(vec![snap]));
        adapter.update().unwrap();
        let cached = adapter.last_snapshot().unwrap();
        prop_assert!((cached.pose.yaw - yaw).abs() < 1e-6);
        prop_assert!(adapter.is_active());
    }
}
