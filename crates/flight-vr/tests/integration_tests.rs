// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

use flight_vr::{HeadPose, MockVrBackend, TrackingQuality, VrAdapter, VrError, VrSnapshot};

fn make_snapshot(yaw: f32, quality: TrackingQuality) -> VrSnapshot {
    VrSnapshot {
        pose: HeadPose {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw,
            pitch: 0.0,
            roll: 0.0,
        },
        quality,
        is_worn: true,
    }
}

#[test]
fn test_mock_adapter_polls_sequence() {
    let snapshots = vec![
        make_snapshot(10.0, TrackingQuality::Good),
        make_snapshot(20.0, TrackingQuality::Good),
        make_snapshot(30.0, TrackingQuality::Good),
    ];
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(snapshots));
    assert_eq!(adapter.update().unwrap().pose.yaw, 10.0);
    assert_eq!(adapter.update().unwrap().pose.yaw, 20.0);
    assert_eq!(adapter.update().unwrap().pose.yaw, 30.0);
}

#[test]
fn test_mock_adapter_disconnected_returns_error() {
    let mut adapter = VrAdapter::new(MockVrBackend::new_disconnected());
    assert!(matches!(adapter.update(), Err(VrError::NotConnected)));
}

#[test]
fn test_adapter_caches_last_snapshot() {
    let snapshots = vec![make_snapshot(45.0, TrackingQuality::Good)];
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(snapshots));
    assert!(adapter.last_snapshot().is_none());
    adapter.update().unwrap();
    let cached = adapter.last_snapshot().unwrap();
    assert_eq!(cached.pose.yaw, 45.0);
}

#[test]
fn test_head_pose_normalize() {
    // 270° wraps to -90°; -270° wraps to 90°.
    let pose = HeadPose {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        yaw: 270.0,
        pitch: -270.0,
        roll: 0.0,
    };
    let n = pose.normalize();
    assert!(
        (n.yaw - (-90.0)).abs() < 1e-4,
        "expected yaw -90, got {}",
        n.yaw
    );
    assert!(
        (n.pitch - 90.0).abs() < 1e-4,
        "expected pitch 90, got {}",
        n.pitch
    );
    // Values already in range are unchanged.
    let pose2 = HeadPose {
        x: 1.0,
        y: 2.0,
        z: -3.0,
        yaw: 45.0,
        pitch: -30.0,
        roll: 10.0,
    };
    let n2 = pose2.normalize();
    assert_eq!(n2.x, 1.0);
    assert_eq!(n2.yaw, 45.0);
}

#[test]
fn test_tracking_quality_good() {
    let snap = make_snapshot(0.0, TrackingQuality::Good);
    assert_eq!(snap.quality, TrackingQuality::Good);
}

#[test]
fn test_tracking_quality_lost_on_disconnect() {
    let snapshots = vec![VrSnapshot {
        pose: HeadPose::zero(),
        quality: TrackingQuality::Lost,
        is_worn: false,
    }];
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(snapshots));
    let snap = adapter.update().unwrap();
    assert_eq!(snap.quality, TrackingQuality::Lost);
    assert!(!snap.is_worn);
}

#[test]
fn test_adapter_is_active() {
    let snapshots = vec![make_snapshot(0.0, TrackingQuality::Good)];
    let mut adapter = VrAdapter::new(MockVrBackend::new_connected(snapshots));
    assert!(!adapter.is_active(), "should be inactive before first poll");
    adapter.update().unwrap();
    assert!(
        adapter.is_active(),
        "should be active after successful poll"
    );
}

#[test]
fn test_head_pose_zero() {
    let pose = HeadPose::zero();
    assert_eq!(pose.x, 0.0);
    assert_eq!(pose.y, 0.0);
    assert_eq!(pose.z, 0.0);
    assert_eq!(pose.yaw, 0.0);
    assert_eq!(pose.pitch, 0.0);
    assert_eq!(pose.roll, 0.0);
}
