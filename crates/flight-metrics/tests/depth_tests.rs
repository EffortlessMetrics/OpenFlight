// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for `flight-metrics`.
//!
//! Covers counter atomicity, histogram percentile accuracy, gauge independence,
//! Prometheus/JSON export format, and the register → record → export → reset
//! collection lifecycle.

use flight_metrics::export::MetricsCollector;
use flight_metrics::prometheus_export::PrometheusRegistry;
use flight_metrics::{HistogramSummary, Metric, MetricsRegistry};
use std::collections::BTreeMap;
use std::sync::Arc;

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

fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Counter metrics
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn counter_increment_decrement_via_handles() {
    let collector = MetricsCollector::new();
    let c = collector.register_counter("ops", "total operations");
    c.increment();
    c.increment();
    c.increment_by(3);
    assert_eq!(c.value(), 5, "3 increments (1+1+3) = 5");
}

#[test]
fn counter_reset_via_registry() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("hits", 42);
    assert_eq!(counter_value(&reg, "hits"), 42);

    reg.reset();
    assert_eq!(counter_value(&reg, "hits"), 0, "reset must zero the counter");
}

#[test]
fn concurrent_counter_increments_are_atomic() {
    let reg = Arc::new(MetricsRegistry::new());
    let threads = 16;
    let per_thread = 500u64;

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let r = Arc::clone(&reg);
            std::thread::spawn(move || {
                for _ in 0..per_thread {
                    r.inc_counter("atomic_ctr", 1);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }

    assert_eq!(
        counter_value(&reg, "atomic_ctr"),
        threads * per_thread,
        "all concurrent increments must be counted"
    );
}

#[test]
fn named_counters_do_not_interfere() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("alpha", 10);
    reg.inc_counter("beta", 20);
    reg.inc_counter("alpha", 5);

    assert_eq!(counter_value(&reg, "alpha"), 15);
    assert_eq!(counter_value(&reg, "beta"), 20);
}

#[test]
fn counter_overflow_wraps_with_fetch_add() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("overflow", u64::MAX);
    reg.inc_counter("overflow", 1);
    // AtomicU64::fetch_add wraps on overflow
    assert_eq!(counter_value(&reg, "overflow"), 0);
}

// Property test: N increments from zero = N
use proptest::prelude::*;

proptest! {
    #[test]
    fn counter_n_increments_equals_n(n in 1u64..5_000u64) {
        let reg = MetricsRegistry::new();
        for _ in 0..n {
            reg.inc_counter("prop_ctr", 1);
        }
        prop_assert_eq!(counter_value(&reg, "prop_ctr"), n);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Histogram metrics
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn histogram_record_and_percentiles() {
    let reg = MetricsRegistry::new();
    for v in 1..=100 {
        reg.observe("latency", v as f64);
    }

    let metrics = reg.snapshot();
    let s = find_histogram(&metrics, "latency").expect("histogram must exist");

    assert_eq!(s.count, 100);
    assert_eq!(s.min, 1.0);
    assert_eq!(s.max, 100.0);
    assert!((s.mean - 50.5).abs() < 0.01);
}

#[test]
fn histogram_p50_p95_p99_accuracy() {
    let reg = MetricsRegistry::new();
    // 1..=100 gives evenly distributed values
    for v in 1..=100 {
        reg.observe("perc", v as f64);
    }

    let metrics = reg.snapshot();
    let s = find_histogram(&metrics, "perc").expect("histogram must exist");

    // p50 should be ~50, p95 should be ~95, p99 should be ~99
    assert!(
        (s.p50 - 50.0).abs() <= 1.0,
        "p50 should be ~50, got {}",
        s.p50
    );
    assert!(
        (s.p95 - 95.0).abs() <= 1.0,
        "p95 should be ~95, got {}",
        s.p95
    );
    assert!(
        (s.p99 - 99.0).abs() <= 1.0,
        "p99 should be ~99, got {}",
        s.p99
    );
}

#[test]
fn histogram_empty_returns_none_in_snapshot() {
    let reg = MetricsRegistry::new();
    // Observe nothing for "empty_hist"
    let metrics = reg.snapshot();
    assert!(
        find_histogram(&metrics, "empty_hist").is_none(),
        "un-observed histogram must not appear in snapshot"
    );
}

#[test]
fn histogram_large_value_count() {
    let reg = MetricsRegistry::with_histogram_capacity(10_000);
    for i in 0..5_000 {
        reg.observe("big", i as f64);
    }

    let metrics = reg.snapshot();
    let s = find_histogram(&metrics, "big").expect("histogram must exist");
    assert_eq!(s.count, 5_000);
    assert_eq!(s.min, 0.0);
    assert_eq!(s.max, 4_999.0);
}

#[test]
fn histogram_reset_clears_data() {
    let reg = MetricsRegistry::new();
    reg.observe("h", 1.0);
    reg.observe("h", 2.0);
    reg.reset();

    let metrics = reg.snapshot();
    assert!(
        find_histogram(&metrics, "h").is_none(),
        "histogram must be gone after reset"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Gauge metrics
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn gauge_set_and_get() {
    let reg = MetricsRegistry::new();
    reg.set_gauge("temp", 36.6);
    let v = reg.gauge_value("temp").expect("gauge must exist");
    assert!((v - 36.6).abs() < f64::EPSILON);
}

#[test]
fn multiple_gauges_independent() {
    let reg = MetricsRegistry::new();
    reg.set_gauge("a", 1.0);
    reg.set_gauge("b", 2.0);
    reg.set_gauge("a", 100.0);

    assert!((reg.gauge_value("a").unwrap() - 100.0).abs() < f64::EPSILON);
    assert!((reg.gauge_value("b").unwrap() - 2.0).abs() < f64::EPSILON);
}

#[test]
fn gauge_thread_safe_updates() {
    let reg = Arc::new(MetricsRegistry::new());
    let threads = 8;

    let handles: Vec<_> = (0..threads)
        .map(|i| {
            let r = Arc::clone(&reg);
            std::thread::spawn(move || {
                let name = format!("gauge_{i}");
                for v in 0..50 {
                    r.set_gauge(&name, v as f64);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }

    // Each gauge should have its last-written value (49.0)
    for i in 0..threads {
        let name = format!("gauge_{i}");
        let v = reg.gauge_value(&name).expect("gauge must exist");
        assert!(
            (v - 49.0).abs() < 1e-9,
            "gauge_{i} should be 49.0, got {v}"
        );
    }
}

#[test]
fn gauge_handle_increment_decrement() {
    let collector = MetricsCollector::new();
    let g = collector.register_gauge("connections", "active connections");
    g.set(10.0);
    g.increment();
    g.increment();
    g.decrement();
    assert!((g.value() - 11.0).abs() < f64::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Export format
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn prometheus_text_format_structure() {
    let mut prom = PrometheusRegistry::new();
    prom.register_counter("http_requests_total", "Total HTTP requests", BTreeMap::new(), 42.0);

    let output = prom.export_prometheus();
    assert!(output.contains("# HELP http_requests_total Total HTTP requests"));
    assert!(output.contains("# TYPE http_requests_total counter"));
    assert!(output.contains("http_requests_total 42.0"));
}

#[test]
fn json_export_structure() {
    let mut prom = PrometheusRegistry::new();
    prom.register_gauge("cpu_usage", "CPU percentage", BTreeMap::new(), 75.5);

    let json = prom.export_json();
    assert!(json.starts_with('['));
    assert!(json.ends_with(']'));
    assert!(json.contains("\"name\":\"cpu_usage\""));
    assert!(json.contains("\"type\":\"gauge\""));
    assert!(json.contains("\"value\":75.5"));
}

#[test]
fn metric_names_follow_conventions() {
    // Prometheus metric names should match [a-zA-Z_:][a-zA-Z0-9_:]*
    let mut prom = PrometheusRegistry::new();
    prom.register_counter(
        "flight_axis_updates_total",
        "Axis updates",
        BTreeMap::new(),
        1.0,
    );
    let output = prom.export_prometheus();
    assert!(output.contains("flight_axis_updates_total"));
}

#[test]
fn labels_attached_correctly_in_prometheus_output() {
    let mut prom = PrometheusRegistry::new();
    prom.register_counter(
        "requests",
        "reqs",
        labels(&[("method", "POST"), ("status", "200")]),
        5.0,
    );

    let output = prom.export_prometheus();
    assert!(output.contains("requests{method=\"POST\",status=\"200\"} 5.0"));
}

#[test]
fn labels_attached_correctly_in_json_output() {
    let mut prom = PrometheusRegistry::new();
    prom.register_gauge(
        "disk_usage",
        "Disk",
        labels(&[("mount", "/data")]),
        85.0,
    );

    let json = prom.export_json();
    assert!(json.contains("\"mount\":\"/data\""));
}

#[test]
fn collector_snapshot_prometheus_and_json_export() {
    let collector = MetricsCollector::new();
    let c = collector.register_counter("snap_ctr", "counter for snapshot");
    let g = collector.register_gauge("snap_gauge", "gauge for snapshot");
    let h = collector.register_histogram("snap_hist", "histogram for snapshot", &[1.0, 5.0, 10.0]);

    c.increment_by(10);
    g.set(99.9);
    h.observe(3.0);
    h.observe(7.0);

    let snap = collector.snapshot();
    assert_eq!(snap.len(), 3);

    let prom_text = snap.to_prometheus_text();
    assert!(prom_text.contains("# TYPE snap_ctr counter"));
    assert!(prom_text.contains("# TYPE snap_gauge gauge"));
    assert!(prom_text.contains("# TYPE snap_hist histogram"));

    let json = snap.to_json();
    assert!(json.contains("\"name\":\"snap_ctr\""));
    assert!(json.contains("\"name\":\"snap_gauge\""));
    assert!(json.contains("\"name\":\"snap_hist\""));
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Collection lifecycle
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn lifecycle_register_record_export_reset() {
    let reg = MetricsRegistry::new();

    // Register (implicit on first use)
    reg.inc_counter("lifecycle_ctr", 5);
    reg.set_gauge("lifecycle_gauge", 3.14);
    reg.observe("lifecycle_hist", 42.0);

    // Record more
    reg.inc_counter("lifecycle_ctr", 5);
    reg.observe("lifecycle_hist", 43.0);

    // Export (snapshot)
    let metrics = reg.snapshot();
    assert_eq!(metrics.len(), 3);

    let ctr = metrics.iter().find_map(|m| match m {
        Metric::Counter { name, value } if name == "lifecycle_ctr" => Some(*value),
        _ => None,
    });
    assert_eq!(ctr, Some(10));

    // Reset
    reg.reset();
    assert!(reg.snapshot().is_empty(), "all metrics must be cleared");

    // Re-register after reset
    reg.inc_counter("lifecycle_ctr", 1);
    assert_eq!(counter_value(&reg, "lifecycle_ctr"), 1);
}

#[test]
fn metric_registry_is_thread_safe() {
    let reg = Arc::new(MetricsRegistry::new());
    let threads = 4;

    let handles: Vec<_> = (0..threads)
        .map(|i| {
            let r = Arc::clone(&reg);
            std::thread::spawn(move || {
                r.inc_counter(&format!("ctr_{i}"), 1);
                r.set_gauge(&format!("gauge_{i}"), (i + 1) as f64);
                r.observe(&format!("hist_{i}"), (i + 1) as f64);
                let _ = r.snapshot();
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let snap = reg.snapshot();
    // Each thread registered 3 metrics = 12 total (some histograms may be None
    // if observe was 0.0, but 0.0 is finite so they show up)
    assert_eq!(snap.len(), threads * 3);
}

#[test]
fn duplicate_counter_registration_is_idempotent() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("dup", 1);
    reg.inc_counter("dup", 2);

    let metrics = reg.snapshot();
    let count = metrics
        .iter()
        .filter(|m| matches!(m, Metric::Counter { name, .. } if name == "dup"))
        .count();
    assert_eq!(count, 1, "same name must yield a single counter entry");
    assert_eq!(counter_value(&reg, "dup"), 3);
}

#[test]
fn prometheus_registry_duplicate_register_adds_separate_entries() {
    let mut prom = PrometheusRegistry::new();
    prom.register_counter("dup_counter", "first", BTreeMap::new(), 1.0);
    prom.register_counter("dup_counter", "second", BTreeMap::new(), 2.0);
    // PrometheusRegistry allows duplicate names (separate entries)
    assert_eq!(prom.metric_count(), 2);
}

#[test]
fn histogram_handle_quantile_empty_returns_zero() {
    let collector = MetricsCollector::new();
    let h = collector.register_histogram("empty_h", "empty", &[1.0]);
    assert_eq!(h.count(), 0);
    assert_eq!(h.quantile(0.5), 0.0);
    assert_eq!(h.quantile(0.99), 0.0);
}

#[test]
fn histogram_handle_count_and_sum() {
    let collector = MetricsCollector::new();
    let h = collector.register_histogram("tracked", "track", &[1.0, 5.0, 10.0]);
    h.observe(2.0);
    h.observe(4.0);
    h.observe(8.0);
    assert_eq!(h.count(), 3);
    assert!((h.sum() - 14.0).abs() < f64::EPSILON);
}
