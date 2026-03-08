// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for Moza health monitor state machine — covers all transitions,
//! combined fault modes, and edge cases.

use flight_ffb_moza::health::{MozaHealthMonitor, MozaHealthStatus};

// ── Initial state ───────────────────────────────────────────────────────

#[test]
fn fresh_monitor_is_healthy_and_connected() {
    let m = MozaHealthMonitor::new();
    let s = m.status();
    assert!(s.is_healthy());
    assert!(s.connected);
    assert_eq!(s.consecutive_failures, 0);
    assert!(!s.torque_fault);
    assert!(s.last_success.is_none());
}

#[test]
fn default_trait_matches_new() {
    let a = MozaHealthMonitor::new();
    let b = MozaHealthMonitor::default();
    let sa = a.status();
    let sb = b.status();
    assert_eq!(sa.connected, sb.connected);
    assert_eq!(sa.consecutive_failures, sb.consecutive_failures);
    assert_eq!(sa.torque_fault, sb.torque_fault);
}

// ── Failure threshold transitions ───────────────────────────────────────

#[test]
fn one_failure_still_online() {
    let mut m = MozaHealthMonitor::new();
    m.record_failure();
    assert!(!m.is_offline());
    assert!(m.status().is_healthy());
}

#[test]
fn two_failures_still_online() {
    let mut m = MozaHealthMonitor::new();
    m.record_failure();
    m.record_failure();
    assert!(!m.is_offline());
    assert!(m.status().is_healthy());
}

#[test]
fn three_failures_goes_offline() {
    let mut m = MozaHealthMonitor::new();
    for _ in 0..3 {
        m.record_failure();
    }
    assert!(m.is_offline());
    assert!(!m.status().connected);
    assert!(!m.status().is_healthy());
}

#[test]
fn many_failures_stays_offline() {
    let mut m = MozaHealthMonitor::new();
    for _ in 0..100 {
        m.record_failure();
    }
    assert!(m.is_offline());
    assert_eq!(m.status().consecutive_failures, 100);
}

#[test]
fn default_failure_threshold_is_three() {
    assert_eq!(MozaHealthMonitor::DEFAULT_FAILURE_THRESHOLD, 3);
}

// ── Recovery transitions ────────────────────────────────────────────────

#[test]
fn success_after_one_failure_resets() {
    let mut m = MozaHealthMonitor::new();
    m.record_failure();
    m.record_success();
    assert_eq!(m.status().consecutive_failures, 0);
    assert!(m.status().is_healthy());
}

#[test]
fn success_after_offline_recovers() {
    let mut m = MozaHealthMonitor::new();
    for _ in 0..5 {
        m.record_failure();
    }
    assert!(m.is_offline());
    m.record_success();
    assert!(!m.is_offline());
    assert!(m.status().is_healthy());
}

#[test]
fn success_records_timestamp() {
    let mut m = MozaHealthMonitor::new();
    assert!(m.status().last_success.is_none());
    m.record_success();
    assert!(m.status().last_success.is_some());
}

#[test]
fn time_since_last_success_available_after_success() {
    let mut m = MozaHealthMonitor::new();
    assert!(m.time_since_last_success().is_none());
    m.record_success();
    let elapsed = m.time_since_last_success();
    assert!(elapsed.is_some());
    // Should be very recent
    assert!(elapsed.unwrap().as_secs() < 1);
}

// ── Torque fault transitions ────────────────────────────────────────────

#[test]
fn torque_fault_makes_unhealthy_while_connected() {
    let mut m = MozaHealthMonitor::new();
    m.set_torque_fault(true);
    let s = m.status();
    assert!(s.connected, "should still be connected");
    assert!(s.torque_fault);
    assert!(!s.is_healthy(), "torque fault should make unhealthy");
}

#[test]
fn clearing_torque_fault_restores_health() {
    let mut m = MozaHealthMonitor::new();
    m.set_torque_fault(true);
    assert!(!m.status().is_healthy());
    m.set_torque_fault(false);
    assert!(m.status().is_healthy());
}

#[test]
fn torque_fault_toggle_multiple_times() {
    let mut m = MozaHealthMonitor::new();
    for _ in 0..10 {
        m.set_torque_fault(true);
        assert!(!m.status().is_healthy());
        m.set_torque_fault(false);
        assert!(m.status().is_healthy());
    }
}

// ── Combined fault modes ────────────────────────────────────────────────

#[test]
fn offline_plus_torque_fault_both_unhealthy() {
    let mut m = MozaHealthMonitor::new();
    for _ in 0..3 {
        m.record_failure();
    }
    m.set_torque_fault(true);
    let s = m.status();
    assert!(!s.connected);
    assert!(s.torque_fault);
    assert!(!s.is_healthy());
}

#[test]
fn recovering_from_offline_but_still_torque_fault() {
    let mut m = MozaHealthMonitor::new();
    for _ in 0..3 {
        m.record_failure();
    }
    m.set_torque_fault(true);
    m.record_success(); // recover from offline
    let s = m.status();
    assert!(s.connected);
    assert!(s.torque_fault);
    assert!(!s.is_healthy(), "torque fault still present");
}

#[test]
fn clearing_all_faults_restores_full_health() {
    let mut m = MozaHealthMonitor::new();
    for _ in 0..3 {
        m.record_failure();
    }
    m.set_torque_fault(true);
    m.record_success();
    m.set_torque_fault(false);
    assert!(m.status().is_healthy());
}

// ── Interleaved success/failure sequences ───────────────────────────────

#[test]
fn alternating_success_failure_stays_online() {
    let mut m = MozaHealthMonitor::new();
    for _ in 0..50 {
        m.record_failure();
        m.record_success();
    }
    assert!(!m.is_offline());
    assert_eq!(m.status().consecutive_failures, 0);
}

#[test]
fn two_failures_then_success_then_two_failures_stays_online() {
    let mut m = MozaHealthMonitor::new();
    m.record_failure();
    m.record_failure();
    m.record_success();
    m.record_failure();
    m.record_failure();
    assert!(!m.is_offline());
    assert_eq!(m.status().consecutive_failures, 2);
}

// ── MozaHealthStatus direct tests ───────────────────────────────────────

#[test]
fn health_status_is_healthy_requires_all_conditions() {
    // Connected, no failures, no fault → healthy
    let healthy = MozaHealthStatus {
        connected: true,
        consecutive_failures: 0,
        last_success: None,
        torque_fault: false,
    };
    assert!(healthy.is_healthy());

    // Not connected → unhealthy
    let disconnected = MozaHealthStatus {
        connected: false,
        ..healthy.clone()
    };
    assert!(!disconnected.is_healthy());

    // 3+ failures → unhealthy
    let too_many_failures = MozaHealthStatus {
        consecutive_failures: 3,
        ..healthy.clone()
    };
    assert!(!too_many_failures.is_healthy());

    // Torque fault → unhealthy
    let faulted = MozaHealthStatus {
        torque_fault: true,
        ..healthy
    };
    assert!(!faulted.is_healthy());
}

#[test]
fn health_status_two_failures_still_healthy() {
    let s = MozaHealthStatus {
        connected: true,
        consecutive_failures: 2,
        last_success: None,
        torque_fault: false,
    };
    assert!(s.is_healthy());
}
