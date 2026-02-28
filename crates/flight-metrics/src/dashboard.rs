// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Dashboard snapshot — a typed, human-readable view of a metrics registry.
//!
//! Call [`MetricsDashboard::from_snapshot`] with the output of
//! [`MetricsRegistry::snapshot`] to obtain a fully-typed [`DashboardSnapshot`]
//! you can serialize to JSON or display in a terminal.

use crate::common;
use crate::types::{HistogramSummary, Metric};

/// Typed snapshot of simulator-integration metrics.
#[derive(Debug, Clone, Default)]
pub struct SimMetrics {
    /// Total frames received from any simulator
    pub frames_total: u64,
    /// Total frames that failed to parse or were dropped
    pub errors_total: u64,
    /// Simulator connection state: `true` = connected
    pub connected: bool,
    /// Effective data rate in Hz (0.0 if unknown)
    pub data_rate_hz: f64,
    /// Age of the most recently received packet in milliseconds
    pub last_packet_age_ms: f64,
    /// Frame latency histogram summary, if samples exist
    pub frame_latency_ms: Option<HistogramSummary>,
    /// Profile switches triggered by aircraft detection
    pub profile_switches_total: u64,
}

/// Typed snapshot of force-feedback engine metrics.
#[derive(Debug, Clone, Default)]
pub struct FfbMetrics {
    /// Total FFB effect updates applied to hardware
    pub effects_applied_total: u64,
    /// Total hardware faults detected
    pub fault_count_total: u64,
    /// Times the safety envelope clamped an effect
    pub envelope_clamp_total: u64,
    /// Times an emergency-stop was triggered
    pub emergency_stop_total: u64,
    /// Configured maximum torque in N·m
    pub max_torque_nm: f64,
    /// Current output torque in N·m
    pub current_torque_nm: f64,
    /// Effect write latency histogram summary, if samples exist
    pub effect_latency_ms: Option<HistogramSummary>,
}

/// Typed snapshot of real-time scheduler metrics.
#[derive(Debug, Clone, Default)]
pub struct RtMetrics {
    /// Total 250 Hz ticks processed since start
    pub ticks_total: u64,
    /// Ticks that missed their deadline
    pub missed_deadlines_total: u64,
    /// Tick jitter histogram summary, if samples exist
    pub jitter_us: Option<HistogramSummary>,
}

/// Typed snapshot of axis processing metrics.
#[derive(Debug, Clone, Default)]
pub struct AxisMetrics {
    /// Axis processing latency histogram, if samples exist
    pub processing_latency_us: Option<HistogramSummary>,
}

/// Typed snapshot of bus metrics.
#[derive(Debug, Clone, Default)]
pub struct BusMetrics {
    /// Events dispatched per second
    pub events_per_second: f64,
    /// Total bus events dispatched
    pub events_total: u64,
}

/// Typed snapshot of device inventory metrics.
#[derive(Debug, Clone, Default)]
pub struct DeviceMetrics {
    /// Number of currently connected devices
    pub connected_count: f64,
}

/// Typed snapshot of watchdog metrics.
#[derive(Debug, Clone, Default)]
pub struct WatchdogMetrics {
    /// Dead-man's switch triggers
    pub dms_triggers_total: u64,
    /// Hardware watchdog timeouts
    pub hw_timeouts_total: u64,
}

/// Aggregated dashboard snapshot built from a raw metrics snapshot.
///
/// Construct with [`MetricsDashboard::from_snapshot`].
#[derive(Debug, Clone, Default)]
pub struct DashboardSnapshot {
    /// Simulator integration metrics
    pub sim: SimMetrics,
    /// Force-feedback engine metrics
    pub ffb: FfbMetrics,
    /// Real-time scheduler metrics
    pub rt: RtMetrics,
    /// Axis processing metrics
    pub axis: AxisMetrics,
    /// Event bus metrics
    pub bus: BusMetrics,
    /// Device inventory metrics
    pub devices: DeviceMetrics,
    /// Watchdog metrics
    pub watchdog: WatchdogMetrics,
}

/// Builder that converts a raw [`Vec<Metric>`] snapshot into a
/// typed [`DashboardSnapshot`].
pub struct MetricsDashboard;

impl MetricsDashboard {
    /// Convert a raw metrics snapshot into a typed dashboard view.
    ///
    /// Unknown metric names are silently ignored, so the function is safe
    /// to call even when some subsystems have not yet recorded any data.
    pub fn from_snapshot(metrics: &[Metric]) -> DashboardSnapshot {
        let mut snap = DashboardSnapshot::default();

        for metric in metrics {
            match metric {
                Metric::Counter { name, value } => match name.as_str() {
                    common::SIM_FRAMES_TOTAL => snap.sim.frames_total = *value,
                    common::SIM_ERRORS_TOTAL => snap.sim.errors_total = *value,
                    common::SIM_PROFILE_SWITCHES_TOTAL => snap.sim.profile_switches_total = *value,
                    common::FFB_EFFECTS_APPLIED_TOTAL => snap.ffb.effects_applied_total = *value,
                    common::FFB_FAULT_COUNT_TOTAL => snap.ffb.fault_count_total = *value,
                    common::FFB_ENVELOPE_CLAMP_TOTAL => snap.ffb.envelope_clamp_total = *value,
                    common::FFB_EMERGENCY_STOP_TOTAL => snap.ffb.emergency_stop_total = *value,
                    common::RT_TICKS_TOTAL => snap.rt.ticks_total = *value,
                    common::RT_MISSED_DEADLINES_TOTAL => snap.rt.missed_deadlines_total = *value,
                    common::BUS_EVENTS_TOTAL => snap.bus.events_total = *value,
                    common::WATCHDOG_DMS_TRIGGERS_TOTAL => {
                        snap.watchdog.dms_triggers_total = *value
                    }
                    common::WATCHDOG_HW_TIMEOUTS_TOTAL => snap.watchdog.hw_timeouts_total = *value,
                    _ => {}
                },
                Metric::Gauge { name, value } => match name.as_str() {
                    common::SIM_CONNECTION_STATE => snap.sim.connected = *value >= 0.5,
                    common::SIM_DATA_RATE_HZ => snap.sim.data_rate_hz = *value,
                    common::SIM_LAST_PACKET_AGE_MS => snap.sim.last_packet_age_ms = *value,
                    common::FFB_MAX_TORQUE_NM => snap.ffb.max_torque_nm = *value,
                    common::FFB_CURRENT_TORQUE_NM => snap.ffb.current_torque_nm = *value,
                    common::BUS_EVENTS_PER_SECOND => snap.bus.events_per_second = *value,
                    common::DEVICES_CONNECTED_COUNT => snap.devices.connected_count = *value,
                    _ => {}
                },
                Metric::Histogram { name, summary } => match name.as_str() {
                    common::SIM_FRAME_LATENCY_MS => {
                        snap.sim.frame_latency_ms = Some(summary.clone())
                    }
                    common::FFB_EFFECT_LATENCY_MS => {
                        snap.ffb.effect_latency_ms = Some(summary.clone())
                    }
                    common::RT_JITTER_US => snap.rt.jitter_us = Some(summary.clone()),
                    common::AXIS_PROCESSING_LATENCY_US => {
                        snap.axis.processing_latency_us = Some(summary.clone())
                    }
                    _ => {}
                },
            }
        }

        snap
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MetricsRegistry, common::*};

    fn build_registry() -> MetricsRegistry {
        let reg = MetricsRegistry::new();
        reg.inc_counter(SIM_FRAMES_TOTAL, 1000);
        reg.inc_counter(SIM_ERRORS_TOTAL, 3);
        reg.set_gauge(SIM_CONNECTION_STATE, 1.0);
        reg.set_gauge(SIM_DATA_RATE_HZ, 60.0);
        reg.set_gauge(SIM_LAST_PACKET_AGE_MS, 5.2);
        reg.observe(SIM_FRAME_LATENCY_MS, 2.1);
        reg.observe(SIM_FRAME_LATENCY_MS, 3.4);
        reg.inc_counter(SIM_PROFILE_SWITCHES_TOTAL, 2);

        reg.inc_counter(FFB_EFFECTS_APPLIED_TOTAL, 500);
        reg.inc_counter(FFB_FAULT_COUNT_TOTAL, 0);
        reg.inc_counter(FFB_ENVELOPE_CLAMP_TOTAL, 7);
        reg.inc_counter(FFB_EMERGENCY_STOP_TOTAL, 0);
        reg.set_gauge(FFB_MAX_TORQUE_NM, 4.0);
        reg.set_gauge(FFB_CURRENT_TORQUE_NM, 1.2);
        reg.observe(FFB_EFFECT_LATENCY_MS, 0.8);

        reg.inc_counter(RT_TICKS_TOTAL, 250_000);
        reg.inc_counter(RT_MISSED_DEADLINES_TOTAL, 0);
        reg.observe(RT_JITTER_US, 120.0);
        reg.observe(RT_JITTER_US, 140.0);
        reg.observe(RT_JITTER_US, 310.0);
        reg
    }

    #[test]
    fn dashboard_sim_fields() {
        let reg = build_registry();
        let snap = MetricsDashboard::from_snapshot(&reg.snapshot());
        assert_eq!(snap.sim.frames_total, 1000);
        assert_eq!(snap.sim.errors_total, 3);
        assert!(snap.sim.connected);
        assert_eq!(snap.sim.data_rate_hz, 60.0);
        assert_eq!(snap.sim.profile_switches_total, 2);
        let lat = snap.sim.frame_latency_ms.expect("frame latency missing");
        assert_eq!(lat.count, 2);
    }

    #[test]
    fn dashboard_ffb_fields() {
        let reg = build_registry();
        let snap = MetricsDashboard::from_snapshot(&reg.snapshot());
        assert_eq!(snap.ffb.effects_applied_total, 500);
        assert_eq!(snap.ffb.envelope_clamp_total, 7);
        assert_eq!(snap.ffb.max_torque_nm, 4.0);
        assert_eq!(snap.ffb.current_torque_nm, 1.2);
        assert!(snap.ffb.effect_latency_ms.is_some());
    }

    #[test]
    fn dashboard_rt_fields() {
        let reg = build_registry();
        let snap = MetricsDashboard::from_snapshot(&reg.snapshot());
        assert_eq!(snap.rt.ticks_total, 250_000);
        assert_eq!(snap.rt.missed_deadlines_total, 0);
        let jitter = snap.rt.jitter_us.expect("jitter histogram missing");
        assert_eq!(jitter.count, 3);
        assert_eq!(jitter.max, 310.0);
    }

    #[test]
    fn empty_snapshot_yields_defaults() {
        let snap = MetricsDashboard::from_snapshot(&[]);
        assert!(!snap.sim.connected);
        assert_eq!(snap.sim.frames_total, 0);
        assert_eq!(snap.ffb.fault_count_total, 0);
        assert_eq!(snap.rt.ticks_total, 0);
        assert!(snap.rt.jitter_us.is_none());
    }

    #[test]
    fn disconnected_gauge_zero() {
        let reg = MetricsRegistry::new();
        reg.set_gauge(SIM_CONNECTION_STATE, 0.0);
        let snap = MetricsDashboard::from_snapshot(&reg.snapshot());
        assert!(!snap.sim.connected);
    }

    // ── Connection-state threshold boundary ──────────────────────────────────

    #[test]
    fn connection_state_exactly_at_threshold_is_connected() {
        let reg = MetricsRegistry::new();
        reg.set_gauge(SIM_CONNECTION_STATE, 0.5);
        let snap = MetricsDashboard::from_snapshot(&reg.snapshot());
        assert!(snap.sim.connected);
    }

    #[test]
    fn connection_state_just_below_threshold_is_disconnected() {
        let reg = MetricsRegistry::new();
        reg.set_gauge(SIM_CONNECTION_STATE, 0.49);
        let snap = MetricsDashboard::from_snapshot(&reg.snapshot());
        assert!(!snap.sim.connected);
    }

    // ── Unknown metric names are silently ignored ─────────────────────────────

    #[test]
    fn unknown_metrics_do_not_panic_and_leave_defaults() {
        let metrics = vec![
            Metric::Counter {
                name: "unknown.counter".to_string(),
                value: 99,
            },
            Metric::Gauge {
                name: "unknown.gauge".to_string(),
                value: 3.14,
            },
            Metric::Histogram {
                name: "unknown.hist".to_string(),
                summary: HistogramSummary {
                    count: 1,
                    min: 1.0,
                    max: 1.0,
                    mean: 1.0,
                    p50: 1.0,
                    p95: 1.0,
                    p99: 1.0,
                },
            },
        ];
        let snap = MetricsDashboard::from_snapshot(&metrics);
        assert_eq!(snap.sim.frames_total, 0);
        assert_eq!(snap.ffb.effects_applied_total, 0);
        assert_eq!(snap.rt.ticks_total, 0);
        assert!(snap.rt.jitter_us.is_none());
    }

    // ── Remaining sim fields ──────────────────────────────────────────────────

    #[test]
    fn sim_last_packet_age_ms_populated() {
        let reg = MetricsRegistry::new();
        reg.set_gauge(SIM_LAST_PACKET_AGE_MS, 42.0);
        let snap = MetricsDashboard::from_snapshot(&reg.snapshot());
        assert_eq!(snap.sim.last_packet_age_ms, 42.0);
    }

    // ── Remaining FFB fields ──────────────────────────────────────────────────

    #[test]
    fn ffb_emergency_stop_and_fault_count_populated() {
        let reg = MetricsRegistry::new();
        reg.inc_counter(FFB_EMERGENCY_STOP_TOTAL, 3);
        reg.inc_counter(FFB_FAULT_COUNT_TOTAL, 1);
        let snap = MetricsDashboard::from_snapshot(&reg.snapshot());
        assert_eq!(snap.ffb.emergency_stop_total, 3);
        assert_eq!(snap.ffb.fault_count_total, 1);
    }

    // ── Histogram statistics ──────────────────────────────────────────────────

    #[test]
    fn rt_jitter_histogram_mean_and_bounds() {
        let reg = MetricsRegistry::new();
        for v in [100.0_f64, 200.0, 300.0, 400.0, 500.0] {
            reg.observe(RT_JITTER_US, v);
        }
        let snap = MetricsDashboard::from_snapshot(&reg.snapshot());
        let jitter = snap.rt.jitter_us.expect("jitter histogram present");
        assert_eq!(jitter.count, 5);
        assert_eq!(jitter.min, 100.0);
        assert_eq!(jitter.max, 500.0);
        assert_eq!(jitter.mean, 300.0);
    }

    #[test]
    fn histogram_absent_when_no_samples_observed() {
        let reg = MetricsRegistry::new();
        // No calls to observe for any histogram key
        let snap = MetricsDashboard::from_snapshot(&reg.snapshot());
        assert!(snap.sim.frame_latency_ms.is_none());
        assert!(snap.ffb.effect_latency_ms.is_none());
        assert!(snap.rt.jitter_us.is_none());
    }

    // ── Duplicate metric name in raw slice (last value wins) ─────────────────

    #[test]
    fn duplicate_counter_last_value_wins() {
        let metrics = vec![
            Metric::Counter {
                name: RT_TICKS_TOTAL.to_string(),
                value: 1,
            },
            Metric::Counter {
                name: RT_TICKS_TOTAL.to_string(),
                value: 9_999,
            },
        ];
        let snap = MetricsDashboard::from_snapshot(&metrics);
        assert_eq!(snap.rt.ticks_total, 9_999);
    }

    // ── Large / edge counter values ───────────────────────────────────────────

    #[test]
    fn large_counter_value_stored_correctly() {
        let reg = MetricsRegistry::new();
        reg.inc_counter(RT_TICKS_TOTAL, u64::MAX / 2);
        let snap = MetricsDashboard::from_snapshot(&reg.snapshot());
        assert_eq!(snap.rt.ticks_total, u64::MAX / 2);
    }
}
