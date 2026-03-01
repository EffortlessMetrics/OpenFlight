// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the metrics and telemetry subsystem.
//!
//! Covers counter overflow/wrapping, histogram bucket boundaries, gauge
//! concurrency, export format fidelity, collection lifecycle, and
//! integration patterns for axis/scheduler/FFB/bus/service-health metrics.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::SystemTime;

use flight_metrics::export::{FlightMetrics, MetricsCollector};
use flight_metrics::prometheus_export::{MetricType, PrometheusRegistry};
use flight_metrics::{
    DashboardSnapshot, HistogramSummary, Metric, MetricsDashboard, MetricsRegistry,
};

// ═══════════════════════════════════════════════════════════════════════════
//  1. Counter metrics (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Counter wraps on u64 overflow (fetch_add wraps by default).
#[test]
fn counter_u64_overflow_wraps() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("wrap", u64::MAX);
    reg.inc_counter("wrap", 2);
    // AtomicU64::fetch_add wraps: u64::MAX + 2 wraps to 1
    let val = counter_value(&reg, "wrap");
    assert_eq!(val, 1, "counter must wrap on overflow");
}

/// Multiple named counters remain independent after many increments.
#[test]
fn counter_named_group_independence() {
    let reg = MetricsRegistry::new();
    let names = ["alpha", "beta", "gamma", "delta"];
    for (i, name) in names.iter().enumerate() {
        for _ in 0..=(i * 10) {
            reg.inc_counter(name, 1);
        }
    }
    assert_eq!(counter_value(&reg, "alpha"), 1);
    assert_eq!(counter_value(&reg, "beta"), 11);
    assert_eq!(counter_value(&reg, "gamma"), 21);
    assert_eq!(counter_value(&reg, "delta"), 31);
}

/// CounterHandle from export module: increment and increment_by are additive.
#[test]
fn export_counter_increment_and_increment_by_additive() {
    let collector = MetricsCollector::new();
    let c = collector.register_counter("ops", "operations");
    c.increment();
    c.increment_by(9);
    c.increment();
    assert_eq!(c.value(), 11);
}

/// CounterHandle: cloned handles reflect the same underlying state.
#[test]
fn export_counter_cloned_handle_shares_state() {
    let collector = MetricsCollector::new();
    let c = collector.register_counter("shared", "shared counter");
    let c2 = c.clone();
    c.increment_by(5);
    c2.increment_by(3);
    assert_eq!(c.value(), 8);
    assert_eq!(c2.value(), 8);
}

/// Thread-safe counter increments across many threads yield exact sum.
#[test]
fn counter_threadsafe_many_threads() {
    let reg = Arc::new(MetricsRegistry::new());
    let threads = 16;
    let per_thread = 500u64;
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let r = Arc::clone(&reg);
            std::thread::spawn(move || {
                for _ in 0..per_thread {
                    r.inc_counter("concurrent", 1);
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(counter_value(&reg, "concurrent"), threads * per_thread);
}

/// CounterHandle: thread-safe increments via export handle.
#[test]
fn export_counter_handle_threadsafe() {
    let collector = Arc::new(MetricsCollector::new());
    let c = collector.register_counter("ts", "threadsafe");
    let threads = 8;
    let per_thread = 1000u64;
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let ch = c.clone();
            std::thread::spawn(move || {
                for _ in 0..per_thread {
                    ch.increment();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(c.value(), threads * per_thread);
}

// ═══════════════════════════════════════════════════════════════════════════
//  2. Histogram metrics (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Histogram percentile values (p50/p90/p99/max) for a 1..=100 distribution.
#[test]
fn histogram_percentile_accuracy_100_samples() {
    let reg = MetricsRegistry::new();
    for i in 1..=100 {
        reg.observe("dist", i as f64);
    }
    let metrics = reg.snapshot();
    let s = find_histogram(&metrics, "dist").unwrap();
    assert_eq!(s.count, 100);
    assert_eq!(s.min, 1.0);
    assert_eq!(s.max, 100.0);
    // p50 = sorted[round(99*0.50)] = sorted[50] = 51
    assert!((s.p50 - 51.0).abs() < f64::EPSILON, "p50={}", s.p50);
    // p95 = sorted[round(99*0.95)] = sorted[94] = 95
    assert!((s.p95 - 95.0).abs() < f64::EPSILON, "p95={}", s.p95);
    // p99 = sorted[round(99*0.99)] = sorted[98] = 99
    assert!((s.p99 - 99.0).abs() < f64::EPSILON, "p99={}", s.p99);
}

/// Histogram bucket boundaries in Prometheus export are correctly populated.
#[test]
fn export_histogram_bucket_boundaries() {
    let collector = MetricsCollector::new();
    let h = collector.register_histogram("lat", "latency", &[1.0, 5.0, 10.0, 50.0]);
    h.observe(0.5);
    h.observe(3.0);
    h.observe(7.0);
    h.observe(25.0);
    h.observe(100.0);
    let text = collector.snapshot().to_prometheus_text();
    assert!(text.contains("lat_bucket{le=\"1.0\"} 1"));
    assert!(text.contains("lat_bucket{le=\"5.0\"} 2"));
    assert!(text.contains("lat_bucket{le=\"10.0\"} 3"));
    assert!(text.contains("lat_bucket{le=\"50.0\"} 4"));
    assert!(text.contains("lat_bucket{le=\"+Inf\"} 5"));
}

/// Histogram reset: after MetricsRegistry::reset, histogram is gone.
#[test]
fn histogram_reset_clears_samples() {
    let reg = MetricsRegistry::new();
    for i in 0..50 {
        reg.observe("h", i as f64);
    }
    assert!(find_histogram(&reg.snapshot(), "h").is_some());
    reg.reset();
    assert!(find_histogram(&reg.snapshot(), "h").is_none());
}

/// Histogram overflow: samples beyond capacity are dropped (ring buffer).
#[test]
fn histogram_capacity_overflow_drops_oldest() {
    let cap = 8;
    let reg = MetricsRegistry::with_histogram_capacity(cap);
    // Insert 0..15 → oldest samples (0..7) should be evicted
    for i in 0..16 {
        reg.observe("ring", i as f64);
    }
    let snap = reg.snapshot();
    let s = find_histogram(&snap, "ring").unwrap();
    assert_eq!(s.count, cap, "count must be capped at capacity");
    // Oldest evicted, so min should be 8.0 (values 8..15 remain)
    assert!(s.min >= 8.0, "min={}, expected >=8 after eviction", s.min);
    assert_eq!(s.max, 15.0);
}

/// Empty histogram produces None in snapshot.
#[test]
fn histogram_empty_returns_none_in_snapshot() {
    let reg = MetricsRegistry::new();
    // Don't observe anything for "empty_h"
    let metrics = reg.snapshot();
    assert!(find_histogram(&metrics, "empty_h").is_none());
}

/// Histogram with negative values records them correctly.
#[test]
fn histogram_negative_values() {
    let reg = MetricsRegistry::new();
    reg.observe("neg", -10.0);
    reg.observe("neg", -5.0);
    reg.observe("neg", 0.0);
    reg.observe("neg", 5.0);
    let snap = reg.snapshot();
    let s = find_histogram(&snap, "neg").unwrap();
    assert_eq!(s.min, -10.0);
    assert_eq!(s.max, 5.0);
    assert_eq!(s.count, 4);
    assert!((s.mean - (-2.5)).abs() < f64::EPSILON, "mean={}", s.mean);
}

// ═══════════════════════════════════════════════════════════════════════════
//  3. Gauge metrics (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Gauge set then read back gives exact bit-for-bit value.
#[test]
fn gauge_set_get_bit_exact() {
    let reg = MetricsRegistry::new();
    let v = std::f64::consts::PI;
    reg.set_gauge("pi", v);
    let got = reg.gauge_value("pi").unwrap();
    assert_eq!(got.to_bits(), v.to_bits());
}

/// GaugeHandle increment/decrement CAS loop works under light contention.
#[test]
fn export_gauge_increment_decrement_sequence() {
    let collector = MetricsCollector::new();
    let g = collector.register_gauge("g", "gauge");
    g.set(100.0);
    for _ in 0..50 {
        g.increment();
    }
    for _ in 0..30 {
        g.decrement();
    }
    assert!((g.value() - 120.0).abs() < f64::EPSILON);
}

/// Concurrent gauge updates from multiple threads — final value is
/// deterministic when all threads set the same value.
#[test]
fn gauge_concurrent_set_same_value() {
    let reg = Arc::new(MetricsRegistry::new());
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let r = Arc::clone(&reg);
            std::thread::spawn(move || {
                r.set_gauge("shared", 42.0);
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let val = reg.gauge_value("shared").unwrap();
    assert!((val - 42.0).abs() < f64::EPSILON);
}

/// Gauge supports extreme f64 values (MAX, MIN_POSITIVE, subnormals).
#[test]
fn gauge_extreme_f64_values() {
    let reg = MetricsRegistry::new();
    reg.set_gauge("max", f64::MAX);
    assert_eq!(reg.gauge_value("max").unwrap(), f64::MAX);

    reg.set_gauge("min_pos", f64::MIN_POSITIVE);
    assert_eq!(reg.gauge_value("min_pos").unwrap(), f64::MIN_POSITIVE);

    reg.set_gauge("neg_max", f64::MIN);
    assert_eq!(reg.gauge_value("neg_max").unwrap(), f64::MIN);

    // Subnormal
    let subnormal = 5e-324_f64;
    reg.set_gauge("sub", subnormal);
    assert_eq!(reg.gauge_value("sub").unwrap().to_bits(), subnormal.to_bits());
}

/// GaugeHandle concurrent increment/decrement under contention converges.
#[test]
fn export_gauge_concurrent_increment_decrement() {
    let collector = Arc::new(MetricsCollector::new());
    let g = collector.register_gauge("contended", "contended gauge");
    g.set(0.0);
    let threads = 8;
    let per_thread = 500;

    let inc_handles: Vec<_> = (0..threads)
        .map(|_| {
            let gh = g.clone();
            std::thread::spawn(move || {
                for _ in 0..per_thread {
                    gh.increment();
                }
            })
        })
        .collect();
    let dec_handles: Vec<_> = (0..threads)
        .map(|_| {
            let gh = g.clone();
            std::thread::spawn(move || {
                for _ in 0..per_thread {
                    gh.decrement();
                }
            })
        })
        .collect();
    for h in inc_handles.into_iter().chain(dec_handles) {
        h.join().unwrap();
    }
    // Equal increments and decrements → should net to 0.0
    assert!(
        g.value().abs() < f64::EPSILON,
        "net should be 0.0, got {}",
        g.value()
    );
}

// ═══════════════════════════════════════════════════════════════════════════
//  4. Export format tests (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Prometheus text format includes HELP, TYPE, and value lines for each metric.
#[test]
fn prometheus_text_format_completeness() {
    let collector = MetricsCollector::new();
    let c = collector.register_counter("http_requests_total", "Total HTTP requests");
    c.increment_by(42);
    let g = collector.register_gauge("cpu_usage", "CPU usage percent");
    g.set(73.5);
    let h = collector.register_histogram("req_duration_seconds", "Request duration", &[0.1, 0.5]);
    h.observe(0.05);
    h.observe(0.3);

    let text = collector.snapshot().to_prometheus_text();
    // Counter
    assert!(text.contains("# HELP http_requests_total Total HTTP requests"));
    assert!(text.contains("# TYPE http_requests_total counter"));
    assert!(text.contains("http_requests_total 42"));
    // Gauge
    assert!(text.contains("# HELP cpu_usage CPU usage percent"));
    assert!(text.contains("# TYPE cpu_usage gauge"));
    assert!(text.contains("cpu_usage 73.5"));
    // Histogram
    assert!(text.contains("# HELP req_duration_seconds Request duration"));
    assert!(text.contains("# TYPE req_duration_seconds histogram"));
    assert!(text.contains("req_duration_seconds_count 2"));
}

/// JSON export contains timestamp_ms, metrics array, and correct types.
#[test]
fn json_export_structure_and_timestamp() {
    let before = SystemTime::now();
    let collector = MetricsCollector::new();
    let c = collector.register_counter("events", "Events total");
    c.increment_by(5);
    let snap = collector.snapshot();
    let after = SystemTime::now();

    let json = snap.to_json();
    assert!(json.starts_with('{'));
    assert!(json.ends_with('}'));
    assert!(json.contains("\"timestamp_ms\":"));
    assert!(json.contains("\"metrics\":["));
    assert!(json.contains("\"name\":\"events\""));
    assert!(json.contains("\"type\":\"counter\""));

    // Verify timestamp is within bounds
    let before_ms = before
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let after_ms = after
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    // Extract timestamp_ms value
    let ts_start = json.find("\"timestamp_ms\":").unwrap() + "\"timestamp_ms\":".len();
    let ts_end = json[ts_start..].find(',').unwrap() + ts_start;
    let ts: u128 = json[ts_start..ts_end].parse().unwrap();
    assert!(ts >= before_ms && ts <= after_ms, "timestamp out of range");
}

/// PrometheusRegistry labels appear in correct order in export.
#[test]
fn prometheus_registry_labels_in_output() {
    let mut reg = PrometheusRegistry::new();
    let mut labels = BTreeMap::new();
    labels.insert("instance".to_string(), "host1".to_string());
    labels.insert("job".to_string(), "flightd".to_string());
    labels.insert("env".to_string(), "prod".to_string());
    reg.register_counter("requests", "Total requests", labels, 100.0);

    let output = reg.export_prometheus();
    // BTreeMap sorts keys alphabetically: env, instance, job
    assert!(output.contains("requests{env=\"prod\",instance=\"host1\",job=\"flightd\"} 100.0"));
}

/// Snapshot timestamp is monotonically non-decreasing across snapshots.
#[test]
fn snapshot_timestamp_monotonic() {
    let collector = MetricsCollector::new();
    collector.register_counter("c", "c");
    let snap1 = collector.snapshot();
    let snap2 = collector.snapshot();
    let snap3 = collector.snapshot();
    assert!(snap2.timestamp >= snap1.timestamp);
    assert!(snap3.timestamp >= snap2.timestamp);
}

/// PrometheusRegistry supports multiple metric families (counter + gauge)
/// exported together.
#[test]
fn prometheus_metric_families_exported_together() {
    let mut reg = PrometheusRegistry::new();
    reg.register_counter("req_total", "Total requests", BTreeMap::new(), 50.0);
    reg.register_gauge("active_conns", "Active connections", BTreeMap::new(), 12.0);
    reg.register_counter("errors_total", "Total errors", BTreeMap::new(), 3.0);

    let output = reg.export_prometheus();
    assert!(output.contains("# TYPE req_total counter"));
    assert!(output.contains("# TYPE active_conns gauge"));
    assert!(output.contains("# TYPE errors_total counter"));
    assert_eq!(reg.metric_count(), 3);

    let json = reg.export_json();
    assert!(json.contains("\"name\":\"req_total\""));
    assert!(json.contains("\"name\":\"active_conns\""));
    assert!(json.contains("\"name\":\"errors_total\""));
}

// ═══════════════════════════════════════════════════════════════════════════
//  5. Collection lifecycle tests (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// MetricsCollector registration creates independent handles for same-name metrics.
#[test]
fn collector_register_same_name_creates_separate_entries() {
    let collector = MetricsCollector::new();
    let c1 = collector.register_counter("x", "first");
    let c2 = collector.register_counter("x", "second");
    c1.increment_by(10);
    c2.increment_by(20);
    // Both are separate entries in the snapshot
    let snap = collector.snapshot();
    assert_eq!(snap.len(), 2);
}

/// MetricsRegistry snapshot returns consistent view across metric types.
#[test]
fn registry_snapshot_consistency() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("c", 5);
    reg.set_gauge("g", 3.14);
    reg.observe("h", 42.0);

    let snap = reg.snapshot();
    assert_eq!(snap.len(), 3);

    let has_c = snap
        .iter()
        .any(|m| matches!(m, Metric::Counter { name, value } if name == "c" && *value == 5));
    let has_g = snap.iter().any(
        |m| matches!(m, Metric::Gauge { name, value } if name == "g" && (*value - 3.14).abs() < f64::EPSILON),
    );
    let has_h = snap
        .iter()
        .any(|m| matches!(m, Metric::Histogram { name, .. } if name == "h"));
    assert!(has_c && has_g && has_h);
}

/// Filtering snapshots by metric type yields correct subsets.
#[test]
fn snapshot_filter_by_metric_type() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("c1", 1);
    reg.inc_counter("c2", 2);
    reg.set_gauge("g1", 1.0);
    reg.observe("h1", 1.0);
    reg.observe("h2", 2.0);

    let snap = reg.snapshot();
    let counters: Vec<_> = snap
        .iter()
        .filter(|m| matches!(m, Metric::Counter { .. }))
        .collect();
    let gauges: Vec<_> = snap
        .iter()
        .filter(|m| matches!(m, Metric::Gauge { .. }))
        .collect();
    let histograms: Vec<_> = snap
        .iter()
        .filter(|m| matches!(m, Metric::Histogram { .. }))
        .collect();

    assert_eq!(counters.len(), 2);
    assert_eq!(gauges.len(), 1);
    assert_eq!(histograms.len(), 2);
}

/// Multiple sequential snapshots reflect mutations between them.
#[test]
fn sequential_snapshots_reflect_mutations() {
    let collector = MetricsCollector::new();
    let c = collector.register_counter("seq", "sequential");
    c.increment_by(1);
    let s1 = collector.snapshot();
    c.increment_by(4);
    let s2 = collector.snapshot();
    c.increment_by(5);
    let s3 = collector.snapshot();

    let t1 = s1.to_prometheus_text();
    let t2 = s2.to_prometheus_text();
    let t3 = s3.to_prometheus_text();

    assert!(t1.contains("seq 1"));
    assert!(t2.contains("seq 5"));
    assert!(t3.contains("seq 10"));
}

/// MetricsCollector implements Default and is usable immediately.
#[test]
fn collector_default_trait_usable() {
    let c = MetricsCollector::default();
    let counter = c.register_counter("a", "a");
    counter.increment();
    assert_eq!(counter.value(), 1);
    assert_eq!(c.snapshot().len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════
//  6. Integration: subsystem → metrics (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Axis engine → metrics: processing latency histogram via FlightMetrics.
#[test]
fn integration_axis_engine_latency_metrics() {
    let collector = MetricsCollector::new();
    let fm = FlightMetrics::register(&collector);

    // Simulate 250Hz tick latencies (in µs)
    let latencies = [80.0, 120.0, 95.0, 200.0, 350.0, 90.0, 110.0, 4500.0];
    for &lat in &latencies {
        fm.axis_processing_latency_us.observe(lat);
    }

    assert_eq!(fm.axis_processing_latency_us.count(), latencies.len() as u64);
    let sum: f64 = latencies.iter().sum();
    assert!((fm.axis_processing_latency_us.sum() - sum).abs() < f64::EPSILON);

    let text = collector.snapshot().to_prometheus_text();
    assert!(text.contains("axis_processing_latency_us_count 8"));
    // 80, 90, 95 ≤ 100 → 3 in the 100 bucket
    assert!(text.contains("axis_processing_latency_us_bucket{le=\"100.0\"} 3"));
}

/// Scheduler → metrics: tick counter and deadline misses via dashboard.
#[test]
fn integration_scheduler_tick_metrics() {
    let reg = MetricsRegistry::new();
    // Simulate 1 second of 250Hz ticks
    reg.inc_counter(flight_metrics::common::RT_TICKS_TOTAL, 250);
    reg.inc_counter(flight_metrics::common::RT_MISSED_DEADLINES_TOTAL, 2);
    // Simulate jitter samples
    for &jitter in &[80.0, 120.0, 150.0, 200.0, 450.0] {
        reg.observe(flight_metrics::common::RT_JITTER_US, jitter);
    }

    let snap = MetricsDashboard::from_snapshot(&reg.snapshot());
    assert_eq!(snap.rt.ticks_total, 250);
    assert_eq!(snap.rt.missed_deadlines_total, 2);
    let jitter = snap.rt.jitter_us.unwrap();
    assert_eq!(jitter.count, 5);
    assert_eq!(jitter.min, 80.0);
    assert_eq!(jitter.max, 450.0);
}

/// FFB → metrics: force magnitude and safety envelope via FlightMetrics.
#[test]
fn integration_ffb_force_metrics() {
    let collector = MetricsCollector::new();
    let fm = FlightMetrics::register(&collector);

    // Simulate FFB force outputs
    let forces = [0.1, 0.3, 0.5, 0.8, 1.5, 3.0, 7.0];
    for &f in &forces {
        fm.ffb_force_magnitude.observe(f);
    }
    assert_eq!(fm.ffb_force_magnitude.count(), forces.len() as u64);

    let text = collector.snapshot().to_prometheus_text();
    assert!(text.contains("ffb_force_magnitude_count 7"));
    // 0.1 ≤ 0.1 → 1
    assert!(text.contains("ffb_force_magnitude_bucket{le=\"0.1\"} 1"));
    // 0.1, 0.3, 0.5 ≤ 0.5 → 3
    assert!(text.contains("ffb_force_magnitude_bucket{le=\"0.5\"} 3"));

    // Also test dashboard FFB counters
    let reg = MetricsRegistry::new();
    reg.inc_counter(flight_metrics::common::FFB_EFFECTS_APPLIED_TOTAL, 1000);
    reg.inc_counter(flight_metrics::common::FFB_ENVELOPE_CLAMP_TOTAL, 15);
    reg.inc_counter(flight_metrics::common::FFB_EMERGENCY_STOP_TOTAL, 1);
    reg.set_gauge(flight_metrics::common::FFB_CURRENT_TORQUE_NM, 2.5);

    let dash = MetricsDashboard::from_snapshot(&reg.snapshot());
    assert_eq!(dash.ffb.effects_applied_total, 1000);
    assert_eq!(dash.ffb.envelope_clamp_total, 15);
    assert_eq!(dash.ffb.emergency_stop_total, 1);
    assert!((dash.ffb.current_torque_nm - 2.5).abs() < f64::EPSILON);
}

/// Service health → metrics: device count, connection state, memory usage.
#[test]
fn integration_service_health_metrics() {
    let collector = MetricsCollector::new();
    let fm = FlightMetrics::register(&collector);

    // Simulate service health state
    fm.device_count.set(4.0);
    fm.memory_usage_bytes.set(52_428_800.0); // 50 MB
    fm.adapter_reconnections_total.increment_by(3);
    fm.profile_switches_total.increment_by(7);

    assert!((fm.device_count.value() - 4.0).abs() < f64::EPSILON);
    assert!((fm.memory_usage_bytes.value() - 52_428_800.0).abs() < f64::EPSILON);
    assert_eq!(fm.adapter_reconnections_total.value(), 3);
    assert_eq!(fm.profile_switches_total.value(), 7);

    let text = collector.snapshot().to_prometheus_text();
    assert!(text.contains("device_count 4.0"));
    assert!(text.contains("adapter_reconnections_total 3"));
    assert!(text.contains("profile_switches_total 7"));

    // Also verify dashboard path for device count + sim connection
    let reg = MetricsRegistry::new();
    reg.set_gauge(flight_metrics::common::DEVICES_CONNECTED_COUNT, 4.0);
    reg.set_gauge(flight_metrics::common::SIM_CONNECTION_STATE, 1.0);
    reg.set_gauge(flight_metrics::common::SIM_DATA_RATE_HZ, 60.0);

    let dash = MetricsDashboard::from_snapshot(&reg.snapshot());
    assert!((dash.devices.connected_count - 4.0).abs() < f64::EPSILON);
    assert!(dash.sim.connected);
    assert!((dash.sim.data_rate_hz - 60.0).abs() < f64::EPSILON);
}

/// Bus → metrics: event counter and events-per-second gauge via dashboard.
#[test]
fn integration_bus_event_metrics() {
    let collector = MetricsCollector::new();
    let fm = FlightMetrics::register(&collector);

    // Simulate bus events
    fm.bus_events_total.increment_by(5000);
    assert_eq!(fm.bus_events_total.value(), 5000);

    let text = collector.snapshot().to_prometheus_text();
    assert!(text.contains("bus_events_total 5000"));

    // Dashboard integration for bus metrics
    let reg = MetricsRegistry::new();
    reg.inc_counter(flight_metrics::common::BUS_EVENTS_TOTAL, 12_000);
    reg.set_gauge(flight_metrics::common::BUS_EVENTS_PER_SECOND, 250.0);

    let dash = MetricsDashboard::from_snapshot(&reg.snapshot());
    assert_eq!(dash.bus.events_total, 12_000);
    assert!((dash.bus.events_per_second - 250.0).abs() < f64::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════
//  Additional depth tests (bonus)
// ═══════════════════════════════════════════════════════════════════════════

/// PrometheusRegistry: increment_counter returns false for missing metric.
#[test]
fn prometheus_registry_increment_missing_counter() {
    let mut reg = PrometheusRegistry::new();
    assert!(!reg.increment_counter("nonexistent", 1.0));
}

/// PrometheusRegistry: set_gauge returns false for missing metric.
#[test]
fn prometheus_registry_set_missing_gauge() {
    let mut reg = PrometheusRegistry::new();
    assert!(!reg.set_gauge("nonexistent", 1.0));
}

/// PrometheusRegistry: clear removes all metrics and export is empty.
#[test]
fn prometheus_registry_clear_then_export() {
    let mut reg = PrometheusRegistry::new();
    reg.register_counter("a", "a", BTreeMap::new(), 1.0);
    reg.register_gauge("b", "b", BTreeMap::new(), 2.0);
    assert_eq!(reg.metric_count(), 2);
    reg.clear();
    assert_eq!(reg.metric_count(), 0);
    assert!(reg.export_prometheus().is_empty());
    assert_eq!(reg.export_json(), "[]");
}

/// MetricType Display formatting is correct for all variants.
#[test]
fn metric_type_display_format() {
    assert_eq!(format!("{}", MetricType::Counter), "counter");
    assert_eq!(format!("{}", MetricType::Gauge), "gauge");
    assert_eq!(format!("{}", MetricType::Histogram), "histogram");
    assert_eq!(format!("{}", MetricType::Summary), "summary");
}

/// DashboardSnapshot defaults are all zero/false/None.
#[test]
fn dashboard_snapshot_default_values() {
    let snap = DashboardSnapshot::default();
    assert_eq!(snap.sim.frames_total, 0);
    assert!(!snap.sim.connected);
    assert_eq!(snap.ffb.effects_applied_total, 0);
    assert_eq!(snap.rt.ticks_total, 0);
    assert!(snap.rt.jitter_us.is_none());
    assert!(snap.axis.processing_latency_us.is_none());
    assert_eq!(snap.bus.events_total, 0);
    assert_eq!(snap.watchdog.dms_triggers_total, 0);
}

/// HistogramSummary Clone and PartialEq work correctly.
#[test]
fn histogram_summary_clone_and_eq() {
    let s1 = HistogramSummary {
        count: 10,
        min: 1.0,
        max: 100.0,
        mean: 50.0,
        p50: 50.0,
        p95: 95.0,
        p99: 99.0,
    };
    let s2 = s1.clone();
    assert_eq!(s1, s2);
}

/// Metric enum variants implement Clone and PartialEq.
#[test]
fn metric_enum_clone_and_eq() {
    let c = Metric::Counter {
        name: "test".to_string(),
        value: 42,
    };
    let g = Metric::Gauge {
        name: "test".to_string(),
        value: 3.14,
    };
    assert_eq!(c.clone(), c);
    assert_eq!(g.clone(), g);
    assert_ne!(c, g);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn counter_value(reg: &MetricsRegistry, name: &str) -> u64 {
    reg.snapshot()
        .into_iter()
        .find_map(|m| match m {
            Metric::Counter { name: n, value } if n == name => Some(value),
            _ => None,
        })
        .unwrap_or(0)
}

fn find_histogram<'a>(metrics: &'a [Metric], name: &str) -> Option<&'a HistogramSummary> {
    metrics.iter().find_map(|m| match m {
        Metric::Histogram { name: n, summary } if n == name => Some(summary),
        _ => None,
    })
}
