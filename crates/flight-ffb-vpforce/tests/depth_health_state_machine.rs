// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for VPforce Rhino health monitor state machine — covers all
//! transitions, ghost-rate computation, custom thresholds, and edge cases.

use flight_ffb_vpforce::health::{RhinoHealthMonitor, RhinoHealthStatus};

// ── Initial state ───────────────────────────────────────────────────────

#[test]
fn fresh_monitor_is_healthy_and_connected() {
    let m = RhinoHealthMonitor::new();
    let s = m.status();
    assert!(s.is_healthy());
    assert!(s.connected);
    assert_eq!(s.consecutive_failures, 0);
    assert_eq!(s.ghost_rate, 0.0);
    assert!(s.last_success.is_none());
}

#[test]
fn default_trait_matches_new() {
    let a = RhinoHealthMonitor::new();
    let b = RhinoHealthMonitor::default();
    let sa = a.status();
    let sb = b.status();
    assert_eq!(sa.connected, sb.connected);
    assert_eq!(sa.consecutive_failures, sb.consecutive_failures);
    assert!((sa.ghost_rate - sb.ghost_rate).abs() < 1e-12);
}

#[test]
fn default_failure_threshold_is_three() {
    assert_eq!(RhinoHealthMonitor::DEFAULT_FAILURE_THRESHOLD, 3);
}

// ── Failure threshold transitions ───────────────────────────────────────

#[test]
fn one_failure_still_online() {
    let mut m = RhinoHealthMonitor::new();
    m.record_failure();
    assert!(!m.is_offline());
    assert!(m.status().connected);
}

#[test]
fn two_failures_still_online() {
    let mut m = RhinoHealthMonitor::new();
    m.record_failure();
    m.record_failure();
    assert!(!m.is_offline());
}

#[test]
fn three_failures_goes_offline() {
    let mut m = RhinoHealthMonitor::new();
    for _ in 0..3 {
        m.record_failure();
    }
    assert!(m.is_offline());
    assert!(!m.status().connected);
    assert!(!m.status().is_healthy());
}

#[test]
fn many_failures_stays_offline() {
    let mut m = RhinoHealthMonitor::new();
    for _ in 0..100 {
        m.record_failure();
    }
    assert!(m.is_offline());
    assert_eq!(m.status().consecutive_failures, 100);
}

// ── Custom failure threshold ────────────────────────────────────────────

#[test]
fn custom_threshold_of_five() {
    let mut m = RhinoHealthMonitor::new().with_failure_threshold(5);
    for _ in 0..4 {
        m.record_failure();
    }
    assert!(!m.is_offline(), "4 failures should be below threshold of 5");
    m.record_failure();
    assert!(m.is_offline(), "5 failures should trigger offline");
}

#[test]
fn custom_threshold_of_one() {
    let mut m = RhinoHealthMonitor::new().with_failure_threshold(1);
    assert!(!m.is_offline());
    m.record_failure();
    assert!(m.is_offline(), "single failure should trigger offline at threshold 1");
}

#[test]
fn custom_threshold_chaining() {
    let m = RhinoHealthMonitor::new().with_failure_threshold(10);
    // Just verify it doesn't panic and is connected
    assert!(!m.is_offline());
}

// ── Recovery transitions ────────────────────────────────────────────────

#[test]
fn success_after_one_failure_resets() {
    let mut m = RhinoHealthMonitor::new();
    m.record_failure();
    m.record_success(false);
    assert_eq!(m.status().consecutive_failures, 0);
}

#[test]
fn success_after_offline_recovers() {
    let mut m = RhinoHealthMonitor::new();
    for _ in 0..5 {
        m.record_failure();
    }
    assert!(m.is_offline());
    m.record_success(false);
    assert!(!m.is_offline());
}

#[test]
fn success_records_timestamp() {
    let mut m = RhinoHealthMonitor::new();
    assert!(m.status().last_success.is_none());
    m.record_success(false);
    assert!(m.status().last_success.is_some());
}

#[test]
fn time_since_last_success_available_after_success() {
    let mut m = RhinoHealthMonitor::new();
    assert!(m.time_since_last_success().is_none());
    m.record_success(false);
    let elapsed = m.time_since_last_success();
    assert!(elapsed.is_some());
    assert!(elapsed.unwrap().as_secs() < 1);
}

// ── Ghost rate computation ──────────────────────────────────────────────

#[test]
fn no_reports_zero_ghost_rate() {
    let m = RhinoHealthMonitor::new();
    assert_eq!(m.status().ghost_rate, 0.0);
}

#[test]
fn all_normal_reports_zero_ghost_rate() {
    let mut m = RhinoHealthMonitor::new();
    for _ in 0..100 {
        m.record_success(false);
    }
    assert_eq!(m.status().ghost_rate, 0.0);
}

#[test]
fn all_ghost_reports_full_ghost_rate() {
    let mut m = RhinoHealthMonitor::new();
    for _ in 0..100 {
        m.record_success(true);
    }
    assert!((m.status().ghost_rate - 1.0).abs() < 1e-6);
}

#[test]
fn half_ghost_reports() {
    let mut m = RhinoHealthMonitor::new();
    for i in 0..100 {
        m.record_success(i % 2 == 0);
    }
    assert!((m.status().ghost_rate - 0.5).abs() < 0.01);
}

#[test]
fn ten_percent_ghost_rate() {
    let mut m = RhinoHealthMonitor::new();
    for i in 0..100 {
        m.record_success(i < 10);
    }
    let rate = m.status().ghost_rate;
    assert!((rate - 0.1).abs() < 0.01, "expected ≈10%, got {rate}");
}

#[test]
fn ghost_rate_includes_failures_in_total() {
    let mut m = RhinoHealthMonitor::new();
    m.record_success(true); // 1 ghost, 1 total
    m.record_failure();      // 1 ghost, 2 total
    let rate = m.status().ghost_rate;
    assert!((rate - 0.5).abs() < 1e-6, "ghost_rate should be 0.5, got {rate}");
}

// ── Ghost rate health threshold ─────────────────────────────────────────

#[test]
fn ghost_rate_below_threshold_healthy() {
    let mut m = RhinoHealthMonitor::new();
    // 9 ghost out of 100 = 9% < 10% threshold
    for i in 0..100 {
        m.record_success(i < 9);
    }
    assert!(m.status().is_healthy(), "9% ghost rate should be healthy");
}

#[test]
fn ghost_rate_at_threshold_unhealthy() {
    let mut m = RhinoHealthMonitor::new();
    // 10 ghost out of 100 = 10% = threshold
    for i in 0..100 {
        m.record_success(i < 10);
    }
    assert!(!m.status().is_healthy(), "10% ghost rate should be unhealthy");
}

#[test]
fn ghost_rate_above_threshold_unhealthy() {
    let mut m = RhinoHealthMonitor::new();
    for _ in 0..20 {
        m.record_success(true);
    }
    assert!(!m.status().is_healthy());
}

// ── Combined conditions ─────────────────────────────────────────────────

#[test]
fn offline_plus_high_ghost_rate() {
    let mut m = RhinoHealthMonitor::new();
    for _ in 0..10 {
        m.record_success(true);
    }
    for _ in 0..3 {
        m.record_failure();
    }
    let s = m.status();
    assert!(!s.connected);
    assert!(s.ghost_rate > 0.5);
    assert!(!s.is_healthy());
}

#[test]
fn recovering_from_offline_but_still_high_ghost_rate() {
    let mut m = RhinoHealthMonitor::new();
    for _ in 0..3 {
        m.record_failure();
    }
    assert!(m.is_offline());
    // Recover with ghost reports
    for _ in 0..5 {
        m.record_success(true);
    }
    let s = m.status();
    assert!(s.connected, "should be connected after successes");
    assert!(s.ghost_rate > 0.5, "ghost rate should still be high");
    assert!(!s.is_healthy(), "high ghost rate still makes unhealthy");
}

// ── Interleaved sequences ───────────────────────────────────────────────

#[test]
fn alternating_success_failure_stays_online() {
    let mut m = RhinoHealthMonitor::new();
    for _ in 0..50 {
        m.record_failure();
        m.record_success(false);
    }
    assert!(!m.is_offline());
    assert_eq!(m.status().consecutive_failures, 0);
}

#[test]
fn two_failures_then_success_then_two_failures_stays_online() {
    let mut m = RhinoHealthMonitor::new();
    m.record_failure();
    m.record_failure();
    m.record_success(false);
    m.record_failure();
    m.record_failure();
    assert!(!m.is_offline());
    assert_eq!(m.status().consecutive_failures, 2);
}

// ── RhinoHealthStatus direct tests ──────────────────────────────────────

#[test]
fn health_status_requires_all_conditions() {
    // All good → healthy
    let healthy = RhinoHealthStatus {
        connected: true,
        consecutive_failures: 0,
        last_success: None,
        ghost_rate: 0.0,
    };
    assert!(healthy.is_healthy());

    // Not connected → unhealthy
    let disconnected = RhinoHealthStatus {
        connected: false,
        consecutive_failures: 0,
        last_success: None,
        ghost_rate: 0.0,
    };
    assert!(!disconnected.is_healthy());

    // 3+ failures → unhealthy
    let too_many = RhinoHealthStatus {
        connected: true,
        consecutive_failures: 3,
        last_success: None,
        ghost_rate: 0.0,
    };
    assert!(!too_many.is_healthy());

    // High ghost rate → unhealthy
    let ghosted = RhinoHealthStatus {
        connected: true,
        consecutive_failures: 0,
        last_success: None,
        ghost_rate: 0.15,
    };
    assert!(!ghosted.is_healthy());
}

#[test]
fn health_status_two_failures_still_healthy() {
    let s = RhinoHealthStatus {
        connected: true,
        consecutive_failures: 2,
        last_success: None,
        ghost_rate: 0.05,
    };
    assert!(s.is_healthy());
}

#[test]
fn health_status_ghost_rate_just_below_threshold() {
    let s = RhinoHealthStatus {
        connected: true,
        consecutive_failures: 0,
        last_success: None,
        ghost_rate: 0.099,
    };
    assert!(s.is_healthy());
}
