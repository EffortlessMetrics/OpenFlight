// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the `flight-openxr` crate.
//!
//! Covers pose data, session state machine, controller input mapping,
//! frame timing / prediction, error handling, serialization round-trips,
//! and property-based invariants.

use std::f32::consts::PI;

use approx::assert_relative_eq;
use flight_openxr::{HeadPose, MockRuntime, OpenXrAdapter, OpenXrError, OpenXrRuntime, SessionState};
use proptest::prelude::*;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn pose(x: f32, y: f32, z: f32, yaw: f32, pitch: f32, roll: f32) -> HeadPose {
    HeadPose {
        x,
        y,
        z,
        yaw,
        pitch,
        roll,
    }
}

fn simple_pose(x: f32, yaw: f32) -> HeadPose {
    pose(x, 0.0, 0.0, yaw, 0.0, 0.0)
}

fn make_adapter(poses: Vec<HeadPose>) -> OpenXrAdapter<MockRuntime> {
    OpenXrAdapter::new(MockRuntime::new(poses))
}

fn make_running_adapter(poses: Vec<HeadPose>) -> OpenXrAdapter<MockRuntime> {
    let mut a = make_adapter(poses);
    a.initialize().unwrap();
    a
}

// ── 1. HeadPose construction & defaults ──────────────────────────────────────

#[test]
fn zero_pose_has_all_fields_zero() {
    let p = HeadPose::zero();
    assert_eq!(p.x, 0.0);
    assert_eq!(p.y, 0.0);
    assert_eq!(p.z, 0.0);
    assert_eq!(p.yaw, 0.0);
    assert_eq!(p.pitch, 0.0);
    assert_eq!(p.roll, 0.0);
}

#[test]
fn zero_pose_is_finite() {
    assert!(HeadPose::zero().is_finite());
}

#[test]
fn custom_pose_preserves_all_fields() {
    let p = pose(1.0, 2.0, 3.0, 0.5, -0.3, 0.1);
    assert_relative_eq!(p.x, 1.0);
    assert_relative_eq!(p.y, 2.0);
    assert_relative_eq!(p.z, 3.0);
    assert_relative_eq!(p.yaw, 0.5);
    assert_relative_eq!(p.pitch, -0.3);
    assert_relative_eq!(p.roll, 0.1);
}

// ── 2. HeadPose::is_finite – NaN / Inf injection per field ──────────────────

#[test]
fn nan_x_is_not_finite() {
    assert!(!HeadPose { x: f32::NAN, ..HeadPose::zero() }.is_finite());
}

#[test]
fn nan_y_is_not_finite() {
    assert!(!HeadPose { y: f32::NAN, ..HeadPose::zero() }.is_finite());
}

#[test]
fn nan_z_is_not_finite() {
    assert!(!HeadPose { z: f32::NAN, ..HeadPose::zero() }.is_finite());
}

#[test]
fn nan_yaw_is_not_finite() {
    assert!(!HeadPose { yaw: f32::NAN, ..HeadPose::zero() }.is_finite());
}

#[test]
fn nan_pitch_is_not_finite() {
    assert!(!HeadPose { pitch: f32::NAN, ..HeadPose::zero() }.is_finite());
}

#[test]
fn nan_roll_is_not_finite() {
    assert!(!HeadPose { roll: f32::NAN, ..HeadPose::zero() }.is_finite());
}

#[test]
fn inf_x_is_not_finite() {
    assert!(!HeadPose { x: f32::INFINITY, ..HeadPose::zero() }.is_finite());
}

#[test]
fn neg_inf_y_is_not_finite() {
    assert!(!HeadPose { y: f32::NEG_INFINITY, ..HeadPose::zero() }.is_finite());
}

#[test]
fn inf_z_is_not_finite() {
    assert!(!HeadPose { z: f32::INFINITY, ..HeadPose::zero() }.is_finite());
}

#[test]
fn inf_yaw_is_not_finite() {
    assert!(!HeadPose { yaw: f32::INFINITY, ..HeadPose::zero() }.is_finite());
}

// ── 3. HeadPose equality & cloning ──────────────────────────────────────────

#[test]
fn pose_equality() {
    let a = pose(1.0, 2.0, 3.0, 0.1, 0.2, 0.3);
    let b = pose(1.0, 2.0, 3.0, 0.1, 0.2, 0.3);
    assert_eq!(a, b);
}

#[test]
fn pose_inequality_on_any_field() {
    let base = pose(1.0, 2.0, 3.0, 0.1, 0.2, 0.3);
    assert_ne!(base, HeadPose { x: 9.0, ..base });
    assert_ne!(base, HeadPose { y: 9.0, ..base });
    assert_ne!(base, HeadPose { z: 9.0, ..base });
    assert_ne!(base, HeadPose { yaw: 9.0, ..base });
    assert_ne!(base, HeadPose { pitch: 9.0, ..base });
    assert_ne!(base, HeadPose { roll: 9.0, ..base });
}

#[test]
fn pose_clone_is_independent() {
    let a = pose(1.0, 2.0, 3.0, 0.1, 0.2, 0.3);
    let mut b = a;
    b.x = 99.0;
    assert_relative_eq!(a.x, 1.0);
    assert_relative_eq!(b.x, 99.0);
}

#[test]
fn pose_debug_format_contains_field_values() {
    let p = HeadPose::zero();
    let dbg = format!("{p:?}");
    assert!(dbg.contains("HeadPose"));
    assert!(dbg.contains("0.0"));
}

// ── 4. Session state machine ────────────────────────────────────────────────

#[test]
fn new_adapter_is_uninitialized() {
    let adapter = make_adapter(vec![HeadPose::zero()]);
    assert_eq!(adapter.state(), SessionState::Uninitialized);
}

#[test]
fn initialize_transitions_to_running() {
    let mut adapter = make_adapter(vec![HeadPose::zero()]);
    adapter.initialize().unwrap();
    assert_eq!(adapter.state(), SessionState::Running);
}

#[test]
fn shutdown_transitions_to_stopping() {
    let mut adapter = make_running_adapter(vec![HeadPose::zero()]);
    adapter.shutdown();
    assert_eq!(adapter.state(), SessionState::Stopping);
}

#[test]
fn poll_after_shutdown_returns_last_pose() {
    let p = pose(1.0, 2.0, 3.0, 0.1, 0.2, 0.3);
    let mut adapter = make_running_adapter(vec![p]);
    let got = adapter.poll();
    assert_eq!(got, p);
    adapter.shutdown();
    let after = adapter.poll();
    assert_eq!(after, p);
}

#[test]
fn error_state_from_session_lost() {
    let mut rt = MockRuntime::new(vec![HeadPose::zero()]);
    rt.next_error = Some(OpenXrError::SessionLost);
    let mut adapter = OpenXrAdapter::new(rt);
    adapter.initialize().unwrap();
    adapter.poll();
    assert_eq!(adapter.state(), SessionState::Error);
}

#[test]
fn error_state_from_pose_unavailable() {
    let mut rt = MockRuntime::new(vec![HeadPose::zero()]);
    rt.next_error = Some(OpenXrError::PoseUnavailable);
    let mut adapter = OpenXrAdapter::new(rt);
    adapter.initialize().unwrap();
    adapter.poll();
    assert_eq!(adapter.state(), SessionState::Error);
}

#[test]
fn full_lifecycle_uninitialized_running_stopping() {
    let mut adapter = make_adapter(vec![HeadPose::zero()]);
    assert_eq!(adapter.state(), SessionState::Uninitialized);
    adapter.initialize().unwrap();
    assert_eq!(adapter.state(), SessionState::Running);
    adapter.poll();
    assert_eq!(adapter.state(), SessionState::Running);
    adapter.shutdown();
    assert_eq!(adapter.state(), SessionState::Stopping);
}

// ── 5. Polling behaviour ────────────────────────────────────────────────────

#[test]
fn poll_before_init_returns_zero() {
    let mut adapter = make_adapter(vec![simple_pose(5.0, 1.0)]);
    assert_eq!(adapter.poll(), HeadPose::zero());
}

#[test]
fn poll_count_zero_before_any_polls() {
    let adapter = make_adapter(vec![HeadPose::zero()]);
    assert_eq!(adapter.poll_count(), 0);
}

#[test]
fn poll_count_increments_only_when_running() {
    let mut adapter = make_adapter(vec![HeadPose::zero()]);
    adapter.poll(); // not running — should NOT count
    assert_eq!(adapter.poll_count(), 0);
    adapter.initialize().unwrap();
    adapter.poll();
    adapter.poll();
    assert_eq!(adapter.poll_count(), 2);
}

#[test]
fn poll_count_increments_even_on_error() {
    let mut rt = MockRuntime::new(vec![HeadPose::zero()]);
    rt.next_error = Some(OpenXrError::SessionLost);
    let mut adapter = OpenXrAdapter::new(rt);
    adapter.initialize().unwrap();
    adapter.poll();
    assert_eq!(adapter.poll_count(), 1);
}

#[test]
fn poll_does_not_increment_after_error_state() {
    let mut rt = MockRuntime::new(vec![HeadPose::zero()]);
    rt.next_error = Some(OpenXrError::SessionLost);
    let mut adapter = OpenXrAdapter::new(rt);
    adapter.initialize().unwrap();
    adapter.poll(); // transitions to Error, count = 1
    adapter.poll(); // state is Error, should not increment
    assert_eq!(adapter.poll_count(), 1);
}

#[test]
fn sequential_polls_return_poses_in_order() {
    let poses = (0..5).map(|i| simple_pose(i as f32, 0.0)).collect();
    let mut adapter = make_running_adapter(poses);
    for i in 0..5 {
        let p = adapter.poll();
        assert_relative_eq!(p.x, i as f32);
    }
}

#[test]
fn mock_runtime_wraps_around() {
    let poses = vec![simple_pose(10.0, 0.0), simple_pose(20.0, 0.0)];
    let mut adapter = make_running_adapter(poses);
    let _ = adapter.poll(); // 10
    let _ = adapter.poll(); // 20
    let p = adapter.poll(); // wraps → 10
    assert_relative_eq!(p.x, 10.0);
}

// ── 6. Error handling depth ─────────────────────────────────────────────────

#[test]
fn session_lost_preserves_last_good_pose() {
    let good = pose(1.5, 0.0, -0.3, 0.7, 0.0, 0.0);
    let rt = MockRuntime::new(vec![good]);
    let mut adapter = OpenXrAdapter::new(rt);
    adapter.initialize().unwrap();
    let first = adapter.poll(); // primes last_pose to `good`
    assert_eq!(first, good);
}

#[test]
fn pose_unavailable_error_transitions_to_error() {
    let mut rt = MockRuntime::new(vec![HeadPose::zero()]);
    rt.next_error = Some(OpenXrError::PoseUnavailable);
    let mut adapter = OpenXrAdapter::new(rt);
    adapter.initialize().unwrap();
    adapter.poll();
    assert_eq!(adapter.state(), SessionState::Error);
}

#[test]
fn error_display_runtime_not_available() {
    let err = OpenXrError::RuntimeNotAvailable("test reason".into());
    let s = format!("{err}");
    assert!(s.contains("Runtime not available"));
    assert!(s.contains("test reason"));
}

#[test]
fn error_display_session_lost() {
    let err = OpenXrError::SessionLost;
    assert_eq!(format!("{err}"), "Session lost");
}

#[test]
fn error_display_pose_unavailable() {
    let err = OpenXrError::PoseUnavailable;
    assert_eq!(format!("{err}"), "Pose unavailable");
}

#[test]
fn error_equality() {
    assert_eq!(OpenXrError::SessionLost, OpenXrError::SessionLost);
    assert_eq!(OpenXrError::PoseUnavailable, OpenXrError::PoseUnavailable);
    assert_ne!(OpenXrError::SessionLost, OpenXrError::PoseUnavailable);
}

#[test]
fn error_clone() {
    let err = OpenXrError::RuntimeNotAvailable("hw gone".into());
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn error_debug_format() {
    let err = OpenXrError::SessionLost;
    let dbg = format!("{err:?}");
    assert!(dbg.contains("SessionLost"));
}

#[test]
fn empty_runtime_returns_pose_unavailable() {
    let mut adapter = make_running_adapter(vec![]);
    let p = adapter.poll();
    assert_eq!(p, HeadPose::zero());
    assert_eq!(adapter.state(), SessionState::Error);
}

// ── 7. MockRuntime specifics ────────────────────────────────────────────────

#[test]
fn mock_runtime_initialize_sets_initialized() {
    let mut rt = MockRuntime::new(vec![]);
    assert!(!rt.initialized);
    OpenXrRuntime::initialize(&mut rt).unwrap();
    assert!(rt.initialized);
}

#[test]
fn mock_runtime_shutdown_clears_initialized() {
    let mut rt = MockRuntime::new(vec![]);
    OpenXrRuntime::initialize(&mut rt).unwrap();
    OpenXrRuntime::shutdown(&mut rt);
    assert!(!rt.initialized);
}

#[test]
fn mock_runtime_next_error_consumed_once() {
    let mut rt = MockRuntime::new(vec![HeadPose::zero()]);
    rt.next_error = Some(OpenXrError::SessionLost);
    let result1 = rt.poll_pose();
    assert!(result1.is_err());
    let result2 = rt.poll_pose();
    assert!(result2.is_ok());
}

#[test]
fn mock_runtime_index_advances() {
    let mut rt = MockRuntime::new(vec![simple_pose(1.0, 0.0), simple_pose(2.0, 0.0)]);
    assert_eq!(rt.index, 0);
    let _ = rt.poll_pose();
    assert_eq!(rt.index, 1);
    let _ = rt.poll_pose();
    assert_eq!(rt.index, 2);
}

// ── 8. Pose normalization and boundary values ───────────────────────────────

#[test]
fn extreme_positive_position_is_finite() {
    let p = pose(f32::MAX / 2.0, f32::MAX / 2.0, f32::MAX / 2.0, 0.0, 0.0, 0.0);
    assert!(p.is_finite());
}

#[test]
fn extreme_negative_position_is_finite() {
    let p = pose(f32::MIN / 2.0, f32::MIN / 2.0, f32::MIN / 2.0, 0.0, 0.0, 0.0);
    assert!(p.is_finite());
}

#[test]
fn subnormal_values_are_finite() {
    let tiny = f32::MIN_POSITIVE / 2.0;
    let p = pose(tiny, tiny, tiny, tiny, tiny, tiny);
    assert!(p.is_finite());
}

#[test]
fn full_rotation_angles_are_finite() {
    let p = pose(0.0, 0.0, 0.0, 2.0 * PI, -2.0 * PI, PI);
    assert!(p.is_finite());
}

#[test]
fn negative_zero_is_finite() {
    let p = pose(-0.0, -0.0, -0.0, -0.0, -0.0, -0.0);
    assert!(p.is_finite());
}

// ── 9. Controller input mapping (axis → pose) ──────────────────────────────

#[test]
fn controller_pitch_up_maps_to_negative_pitch() {
    let p = pose(0.0, 0.0, 0.0, 0.0, -0.3, 0.0);
    assert!(p.pitch < 0.0);
    assert!(p.is_finite());
}

#[test]
fn controller_yaw_left_maps_to_positive_yaw() {
    let p = pose(0.0, 0.0, 0.0, 0.5, 0.0, 0.0);
    assert!(p.yaw > 0.0);
}

#[test]
fn controller_roll_right_maps_to_positive_roll() {
    let p = pose(0.0, 0.0, 0.0, 0.0, 0.0, 0.4);
    assert!(p.roll > 0.0);
}

#[test]
fn mixed_controller_axes_all_finite() {
    let p = pose(0.1, -0.2, 0.3, 1.5, -0.8, 0.6);
    assert!(p.is_finite());
}

// ── 10. Frame timing / prediction (simulated via poll cadence) ──────────────

#[test]
fn rapid_polling_returns_sequential_poses() {
    let poses: Vec<_> = (0..100).map(|i| simple_pose(i as f32 * 0.01, 0.0)).collect();
    let mut adapter = make_running_adapter(poses);
    let mut last_x = -1.0f32;
    for _ in 0..100 {
        let p = adapter.poll();
        assert!(p.x > last_x || (p.x - last_x).abs() < 1e-9);
        last_x = p.x;
    }
}

#[test]
fn poll_count_matches_frame_count() {
    let poses: Vec<_> = (0..250).map(|_| HeadPose::zero()).collect();
    let mut adapter = make_running_adapter(poses);
    for _ in 0..250 {
        adapter.poll();
    }
    assert_eq!(adapter.poll_count(), 250);
}

#[test]
fn wrap_around_cycle_is_deterministic() {
    let poses = vec![
        simple_pose(1.0, 0.0),
        simple_pose(2.0, 0.0),
        simple_pose(3.0, 0.0),
    ];
    let mut adapter = make_running_adapter(poses);
    let mut cycle1 = Vec::new();
    let mut cycle2 = Vec::new();
    for _ in 0..3 {
        cycle1.push(adapter.poll().x);
    }
    for _ in 0..3 {
        cycle2.push(adapter.poll().x);
    }
    assert_eq!(cycle1, cycle2);
}

// ── 11. SessionState enum coverage ──────────────────────────────────────────

#[test]
fn session_state_debug() {
    assert_eq!(format!("{:?}", SessionState::Uninitialized), "Uninitialized");
    assert_eq!(format!("{:?}", SessionState::Initializing), "Initializing");
    assert_eq!(format!("{:?}", SessionState::Ready), "Ready");
    assert_eq!(format!("{:?}", SessionState::Running), "Running");
    assert_eq!(format!("{:?}", SessionState::Stopping), "Stopping");
    assert_eq!(format!("{:?}", SessionState::Error), "Error");
}

#[test]
fn session_state_equality() {
    assert_eq!(SessionState::Running, SessionState::Running);
    assert_ne!(SessionState::Running, SessionState::Stopping);
}

#[test]
fn session_state_clone() {
    let s = SessionState::Running;
    let c = s;
    assert_eq!(s, c);
}

// ── 12. Serialization round-trip (via Debug + PartialEq) ────────────────────

#[test]
fn pose_round_trip_through_fields() {
    let original = pose(1.234, -5.678, 9.012, 0.345, -0.678, 0.901);
    let reconstructed = HeadPose {
        x: original.x,
        y: original.y,
        z: original.z,
        yaw: original.yaw,
        pitch: original.pitch,
        roll: original.roll,
    };
    assert_eq!(original, reconstructed);
}

#[test]
fn error_round_trip() {
    let original = OpenXrError::RuntimeNotAvailable("loader missing".into());
    let cloned = original.clone();
    assert_eq!(original, cloned);
    assert_eq!(format!("{original}"), format!("{cloned}"));
}

// ── 13. Property-based tests (proptest) ─────────────────────────────────────

proptest! {
    #[test]
    fn prop_finite_pose_reports_is_finite(
        x in -1000.0f32..1000.0,
        y in -1000.0f32..1000.0,
        z in -1000.0f32..1000.0,
        yaw in -PI..PI,
        pitch in -PI..PI,
        roll in -PI..PI,
    ) {
        let p = pose(x, y, z, yaw, pitch, roll);
        prop_assert!(p.is_finite());
    }

    #[test]
    fn prop_zero_like_poses_are_finite(
        x in -f32::MIN_POSITIVE..f32::MIN_POSITIVE,
        y in -f32::MIN_POSITIVE..f32::MIN_POSITIVE,
        z in -f32::MIN_POSITIVE..f32::MIN_POSITIVE,
    ) {
        let p = pose(x, y, z, 0.0, 0.0, 0.0);
        prop_assert!(p.is_finite());
    }

    #[test]
    fn prop_pose_equality_is_reflexive(
        x in -100.0f32..100.0,
        y in -100.0f32..100.0,
        z in -100.0f32..100.0,
        yaw in -PI..PI,
        pitch in -PI..PI,
        roll in -PI..PI,
    ) {
        let p = pose(x, y, z, yaw, pitch, roll);
        prop_assert_eq!(p, p);
    }

    #[test]
    fn prop_adapter_poll_returns_finite_pose(
        x in -100.0f32..100.0,
        yaw in -PI..PI,
    ) {
        let p = simple_pose(x, yaw);
        let mut adapter = make_running_adapter(vec![p]);
        let got = adapter.poll();
        prop_assert!(got.is_finite());
        prop_assert!((got.x - x).abs() < 1e-6);
    }

    #[test]
    fn prop_mock_runtime_wraps_deterministically(
        len in 1usize..20,
    ) {
        let poses: Vec<_> = (0..len).map(|i| simple_pose(i as f32, 0.0)).collect();
        let mut adapter = make_running_adapter(poses);
        for i in 0..(len * 2) {
            let p = adapter.poll();
            let expected_x = (i % len) as f32;
            prop_assert!((p.x - expected_x).abs() < 1e-6,
                "At index {i}, expected x={expected_x}, got x={}", p.x);
        }
    }

    #[test]
    fn prop_poll_count_equals_running_polls(n in 1u64..100) {
        let poses: Vec<_> = (0..n).map(|_| HeadPose::zero()).collect();
        let mut adapter = make_running_adapter(poses);
        for _ in 0..n {
            adapter.poll();
        }
        prop_assert_eq!(adapter.poll_count(), n);
    }

    #[test]
    fn prop_clone_preserves_all_fields(
        x in -1000.0f32..1000.0,
        y in -1000.0f32..1000.0,
        z in -1000.0f32..1000.0,
        yaw in -PI..PI,
        pitch in -PI..PI,
        roll in -PI..PI,
    ) {
        let original = pose(x, y, z, yaw, pitch, roll);
        let cloned = original;
        prop_assert_eq!(original, cloned);
    }

    #[test]
    fn prop_bounded_angles_are_valid(
        yaw in -2.0 * PI..2.0 * PI,
        pitch in -PI / 2.0..PI / 2.0,
        roll in -PI..PI,
    ) {
        let p = pose(0.0, 0.0, 0.0, yaw, pitch, roll);
        prop_assert!(p.is_finite());
        prop_assert!(p.yaw.abs() <= 2.0 * PI + 1e-6);
        prop_assert!(p.pitch.abs() <= PI / 2.0 + 1e-6);
        prop_assert!(p.roll.abs() <= PI + 1e-6);
    }

    #[test]
    fn prop_position_bounded_by_cockpit_range(
        x in -2.0f32..2.0,
        y in -1.0f32..2.0,
        z in -2.0f32..2.0,
    ) {
        let p = pose(x, y, z, 0.0, 0.0, 0.0);
        prop_assert!(p.is_finite());
        prop_assert!(p.x.abs() <= 2.0 + 1e-6);
        prop_assert!(p.y >= -1.0 - 1e-6 && p.y <= 2.0 + 1e-6);
        prop_assert!(p.z.abs() <= 2.0 + 1e-6);
    }
}

// ── 14. Adapter re-init / double shutdown ───────────────────────────────────

#[test]
fn double_shutdown_does_not_panic() {
    let mut adapter = make_running_adapter(vec![HeadPose::zero()]);
    adapter.shutdown();
    adapter.shutdown();
    assert_eq!(adapter.state(), SessionState::Stopping);
}

#[test]
fn poll_in_stopping_state_returns_last_pose() {
    let p = simple_pose(42.0, 0.0);
    let mut adapter = make_running_adapter(vec![p]);
    let _ = adapter.poll();
    adapter.shutdown();
    let after = adapter.poll();
    assert_relative_eq!(after.x, 42.0);
}

#[test]
fn poll_in_error_state_returns_last_good_pose() {
    let good = simple_pose(7.7, 0.0);
    let mut rt = MockRuntime::new(vec![good]);
    rt.next_error = Some(OpenXrError::SessionLost);
    let mut adapter = OpenXrAdapter::new(rt);
    adapter.initialize().unwrap();
    // Error fires on first poll, last_pose is still zero
    let after_err = adapter.poll();
    assert_eq!(after_err, HeadPose::zero());
    // Subsequent polls in error state also return zero
    let again = adapter.poll();
    assert_eq!(again, HeadPose::zero());
}
