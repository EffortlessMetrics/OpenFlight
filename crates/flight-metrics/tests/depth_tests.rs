// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for `flight-metrics`.
//!
//! Covers counter overflow, gauge inc/dec semantics, histogram percentile
//! invariants, timer-like duration recording, registry management,
//! JSON/Prometheus export, thread safety, and property-based invariants.

use flight_metrics::export::MetricsCollector;
use flight_metrics::prometheus_export::PrometheusRegistry;
use flight_metrics::{HistogramSummary, Metric, MetricsRegistry};
use std::collections::BTreeMap;
use std::sync::Arc;

// ══════════════════════════════════════════════════════════════════════════════
// 1. Counter depth tests
// ══════════════════════════════════════════════════════════════════════════════

mod counter_depth {
    use super::*;

    #[test]
    fn counter_starts_at_zero() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "test");
        assert_eq!(counter.value(), 0);
    }

    #[test]
    fn counter_increment_single() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "test");
        counter.increment();
        assert_eq!(counter.value(), 1);
    }

    #[test]
    fn counter_increment_by_accumulates() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "test");
        counter.increment_by(10);
        counter.increment_by(20);
        counter.increment_by(30);
        assert_eq!(counter.value(), 60);
    }

    #[test]
    fn counter_increment_by_zero_is_noop() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "test");
        counter.increment_by(42);
        counter.increment_by(0);
        assert_eq!(counter.value(), 42);
    }

    #[test]
    fn counter_overflow_wraps() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "test");
        counter.increment_by(u64::MAX);
        counter.increment();
        // AtomicU64::fetch_add wraps on overflow
        assert_eq!(counter.value(), 0);
    }

    #[test]
    fn counter_overflow_wraps_partial() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "test");
        counter.increment_by(u64::MAX - 5);
        counter.increment_by(10);
        // (MAX - 5) + 10 = MAX + 5 wraps to 4
        assert_eq!(counter.value(), 4);
    }

    #[test]
    fn counter_large_value_preserved() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "test");
        counter.increment_by(u64::MAX / 2);
        assert_eq!(counter.value(), u64::MAX / 2);
    }

    #[test]
    fn counter_cloned_handle_shares_state() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "test");
        let clone = counter.clone();
        counter.increment_by(5);
        clone.increment_by(3);
        assert_eq!(counter.value(), 8);
        assert_eq!(clone.value(), 8);
    }

    // Registry counter tests
    #[test]
    fn registry_counter_overflow_wraps() {
        let reg = MetricsRegistry::new();
        reg.inc_counter("c", u64::MAX);
        reg.inc_counter("c", 1);
        let val = counter_value(&reg, "c");
        assert_eq!(val, 0);
    }

    #[test]
    fn registry_counter_reset_then_reuse() {
        let reg = MetricsRegistry::new();
        reg.inc_counter("c", 100);
        assert_eq!(counter_value(&reg, "c"), 100);
        reg.reset();
        assert_eq!(counter_value(&reg, "c"), 0);
        reg.inc_counter("c", 50);
        assert_eq!(counter_value(&reg, "c"), 50);
    }

    #[test]
    fn registry_many_counters_independent() {
        let reg = MetricsRegistry::new();
        for i in 0..100 {
            reg.inc_counter(&format!("counter_{i}"), i as u64);
        }
        let metrics = reg.snapshot();
        assert_eq!(metrics.len(), 100);
        for i in 0..100 {
            assert_eq!(counter_value(&reg, &format!("counter_{i}")), i as u64);
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 2. Gauge depth tests
// ══════════════════════════════════════════════════════════════════════════════

mod gauge_depth {
    use super::*;

    #[test]
    fn gauge_initial_value_is_zero() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test");
        assert_eq!(gauge.value(), 0.0);
    }

    #[test]
    fn gauge_set_and_read() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test");
        gauge.set(42.5);
        assert!((gauge.value() - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_set_overwrites() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test");
        gauge.set(10.0);
        gauge.set(20.0);
        gauge.set(30.0);
        assert!((gauge.value() - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_increment_from_zero() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test");
        gauge.increment();
        assert!((gauge.value() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_decrement_from_zero() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test");
        gauge.decrement();
        assert!((gauge.value() - (-1.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_increment_decrement_sequence() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test");
        gauge.set(10.0);
        gauge.increment();
        gauge.increment();
        gauge.decrement();
        assert!((gauge.value() - 11.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_negative_value() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test");
        gauge.set(-999.99);
        assert!((gauge.value() - (-999.99)).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_very_small_value() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test");
        gauge.set(1e-15);
        assert!((gauge.value() - 1e-15).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_very_large_value() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test");
        gauge.set(1e300);
        assert!((gauge.value() - 1e300).abs() < 1e285);
    }

    #[test]
    fn gauge_cloned_handle_shares_state() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test");
        let clone = gauge.clone();
        gauge.set(5.0);
        assert!((clone.value() - 5.0).abs() < f64::EPSILON);
        clone.increment();
        assert!((gauge.value() - 6.0).abs() < f64::EPSILON);
    }

    // Registry gauge tests
    #[test]
    fn registry_gauge_overwrite_preserves_latest() {
        let reg = MetricsRegistry::new();
        reg.set_gauge("g", 1.0);
        reg.set_gauge("g", 2.0);
        reg.set_gauge("g", 3.0);
        assert!((reg.gauge_value("g").unwrap() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn registry_gauge_missing_returns_none() {
        let reg = MetricsRegistry::new();
        assert!(reg.gauge_value("nonexistent").is_none());
    }

    #[test]
    fn registry_gauge_reset_then_lookup() {
        let reg = MetricsRegistry::new();
        reg.set_gauge("g", 42.0);
        reg.reset();
        assert!(reg.gauge_value("g").is_none());
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 3. Histogram depth tests
// ══════════════════════════════════════════════════════════════════════════════

mod histogram_depth {
    use super::*;

    #[test]
    fn histogram_empty_returns_zero_quantile() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test", &[]);
        assert_eq!(hist.quantile(0.5), 0.0);
        assert_eq!(hist.count(), 0);
    }

    #[test]
    fn histogram_single_sample_all_quantiles_equal() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test", &[]);
        hist.observe(42.0);
        assert!((hist.quantile(0.0) - 42.0).abs() < f64::EPSILON);
        assert!((hist.quantile(0.5) - 42.0).abs() < f64::EPSILON);
        assert!((hist.quantile(1.0) - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_p50_p95_p99_ordering() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test", &[]);
        for i in 1..=1000 {
            hist.observe(i as f64);
        }
        let p50 = hist.quantile(0.50);
        let p95 = hist.quantile(0.95);
        let p99 = hist.quantile(0.99);
        assert!(p50 <= p95, "p50 ({p50}) must be <= p95 ({p95})");
        assert!(p95 <= p99, "p95 ({p95}) must be <= p99 ({p99})");
    }

    #[test]
    fn histogram_min_max_from_quantiles() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test", &[]);
        hist.observe(5.0);
        hist.observe(15.0);
        hist.observe(10.0);
        // p0 should be min, p100 should be max
        assert!((hist.quantile(0.0) - 5.0).abs() < f64::EPSILON);
        assert!((hist.quantile(1.0) - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_count_and_sum() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test", &[]);
        hist.observe(1.0);
        hist.observe(2.0);
        hist.observe(3.0);
        assert_eq!(hist.count(), 3);
        assert!((hist.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_ignores_nan() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test", &[]);
        hist.observe(f64::NAN);
        assert_eq!(hist.count(), 0);
        assert!((hist.sum() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_ignores_infinity() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test", &[]);
        hist.observe(f64::INFINITY);
        hist.observe(f64::NEG_INFINITY);
        hist.observe(5.0);
        assert_eq!(hist.count(), 1);
        assert!((hist.sum() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_negative_values() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test", &[]);
        hist.observe(-10.0);
        hist.observe(-5.0);
        hist.observe(0.0);
        hist.observe(5.0);
        assert_eq!(hist.count(), 4);
        assert!((hist.quantile(0.0) - (-10.0)).abs() < f64::EPSILON);
        assert!((hist.quantile(1.0) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_quantile_clamped_out_of_range() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test", &[]);
        hist.observe(1.0);
        hist.observe(10.0);
        // Quantile values outside [0, 1] are clamped
        let below = hist.quantile(-1.0);
        let above = hist.quantile(2.0);
        assert!((below - 1.0).abs() < f64::EPSILON, "clamped to 0.0 → min");
        assert!((above - 10.0).abs() < f64::EPSILON, "clamped to 1.0 → max");
    }

    // Registry histogram tests
    #[test]
    fn registry_histogram_capacity_eviction() {
        let reg = MetricsRegistry::with_histogram_capacity(5);
        for i in 0..10 {
            reg.observe("h", i as f64);
        }
        let metrics = reg.snapshot();
        let summary = find_histogram(&metrics, "h").unwrap();
        assert_eq!(summary.count, 5, "count capped at capacity");
        // After eviction, oldest values are dropped; remaining = [5,6,7,8,9]
        assert_eq!(summary.min, 5.0);
        assert_eq!(summary.max, 9.0);
    }

    #[test]
    fn registry_histogram_min_capacity_is_one() {
        let reg = MetricsRegistry::with_histogram_capacity(0);
        reg.observe("h", 42.0);
        reg.observe("h", 99.0);
        let metrics = reg.snapshot();
        let summary = find_histogram(&metrics, "h").unwrap();
        // Capacity is clamped to at least 1
        assert_eq!(summary.count, 1);
        assert_eq!(summary.max, 99.0);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 4. Timer tests (duration recording via histogram)
// ══════════════════════════════════════════════════════════════════════════════

mod timer_depth {
    use super::*;
    use std::time::Instant;

    #[test]
    fn timer_records_duration() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("duration_us", "latency", &[100.0, 500.0, 1000.0]);

        let start = Instant::now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let elapsed_us = start.elapsed().as_micros() as f64;
        hist.observe(elapsed_us);

        assert_eq!(hist.count(), 1);
        assert!(hist.sum() > 0.0, "recorded duration should be positive");
    }

    #[test]
    fn timer_multiple_durations_accumulate() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("lat", "latency", &[]);

        // Record synthetic durations
        hist.observe(100.0);
        hist.observe(200.0);
        hist.observe(300.0);

        assert_eq!(hist.count(), 3);
        assert!((hist.sum() - 600.0).abs() < f64::EPSILON);
        let p50 = hist.quantile(0.5);
        assert!(
            (100.0..=300.0).contains(&p50),
            "p50 ({p50}) should be in [100, 300]"
        );
    }

    #[test]
    fn timer_zero_duration_recorded() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("lat", "latency", &[]);
        hist.observe(0.0);
        assert_eq!(hist.count(), 1);
        assert!((hist.sum() - 0.0).abs() < f64::EPSILON);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 5. Registry tests
// ══════════════════════════════════════════════════════════════════════════════

mod registry_depth {
    use super::*;

    #[test]
    fn register_and_lookup_by_name() {
        let collector = MetricsCollector::new();
        let _c1 = collector.register_counter("requests", "Total requests");
        let _c2 = collector.register_counter("errors", "Total errors");
        let _g = collector.register_gauge("memory", "Memory usage");

        let snap = collector.snapshot();
        assert_eq!(snap.len(), 3);
        let text = snap.to_prometheus_text();
        assert!(text.contains("requests"));
        assert!(text.contains("errors"));
        assert!(text.contains("memory"));
    }

    #[test]
    fn prometheus_registry_register_and_lookup() {
        let mut reg = PrometheusRegistry::new();
        reg.register_counter("req", "Requests", BTreeMap::new(), 0.0);
        reg.register_gauge("mem", "Memory", BTreeMap::new(), 0.0);
        assert_eq!(reg.metric_count(), 2);
        assert!(reg.get_metric("req").is_some());
        assert!(reg.get_metric("mem").is_some());
        assert!(reg.get_metric("nonexistent").is_none());
    }

    #[test]
    fn prometheus_registry_clear_empties() {
        let mut reg = PrometheusRegistry::new();
        reg.register_counter("a", "A", BTreeMap::new(), 1.0);
        reg.register_gauge("b", "B", BTreeMap::new(), 2.0);
        assert_eq!(reg.metric_count(), 2);
        reg.clear();
        assert_eq!(reg.metric_count(), 0);
        assert!(reg.export_prometheus().is_empty());
    }

    #[test]
    fn prometheus_registry_increment_missing_returns_false() {
        let mut reg = PrometheusRegistry::new();
        assert!(!reg.increment_counter("missing", 1.0));
    }

    #[test]
    fn prometheus_registry_set_gauge_missing_returns_false() {
        let mut reg = PrometheusRegistry::new();
        assert!(!reg.set_gauge("missing", 1.0));
    }

    #[test]
    fn registry_default_trait() {
        let reg = MetricsRegistry::default();
        assert!(reg.snapshot().is_empty());
    }

    #[test]
    fn collector_default_trait() {
        let c = MetricsCollector::default();
        assert!(c.snapshot().is_empty());
    }

    #[test]
    fn registry_snapshot_after_reset_allows_new_metrics() {
        let reg = MetricsRegistry::new();
        reg.inc_counter("c1", 10);
        reg.reset();
        reg.inc_counter("c2", 20);
        let metrics = reg.snapshot();
        assert_eq!(metrics.len(), 1);
        assert_eq!(counter_value(&reg, "c2"), 20);
    }

    #[test]
    fn multiple_collectors_are_independent() {
        let c1 = MetricsCollector::new();
        let c2 = MetricsCollector::new();
        let counter1 = c1.register_counter("x", "x");
        let counter2 = c2.register_counter("x", "x");
        counter1.increment_by(10);
        counter2.increment_by(20);
        assert_eq!(counter1.value(), 10);
        assert_eq!(counter2.value(), 20);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 6. Export tests (JSON / Prometheus format)
// ══════════════════════════════════════════════════════════════════════════════

mod export_depth {
    use super::*;

    #[test]
    fn prometheus_counter_format() {
        let collector = MetricsCollector::new();
        let c = collector.register_counter("http_requests_total", "Total HTTP requests");
        c.increment_by(42);
        let text = collector.snapshot().to_prometheus_text();
        assert!(text.contains("# HELP http_requests_total Total HTTP requests"));
        assert!(text.contains("# TYPE http_requests_total counter"));
        assert!(text.contains("http_requests_total 42"));
    }

    #[test]
    fn prometheus_gauge_format() {
        let collector = MetricsCollector::new();
        let g = collector.register_gauge("temperature_celsius", "Temperature");
        g.set(36.6);
        let text = collector.snapshot().to_prometheus_text();
        assert!(text.contains("# HELP temperature_celsius Temperature"));
        assert!(text.contains("# TYPE temperature_celsius gauge"));
        assert!(text.contains("temperature_celsius 36.6"));
    }

    #[test]
    fn prometheus_histogram_buckets_format() {
        let collector = MetricsCollector::new();
        let h = collector.register_histogram("lat", "Latency", &[1.0, 5.0, 10.0]);
        h.observe(0.5);
        h.observe(3.0);
        h.observe(7.0);
        let text = collector.snapshot().to_prometheus_text();
        assert!(text.contains("lat_bucket{le=\"1.0\"} 1"));
        assert!(text.contains("lat_bucket{le=\"5.0\"} 2"));
        assert!(text.contains("lat_bucket{le=\"10.0\"} 3"));
        assert!(text.contains("lat_bucket{le=\"+Inf\"} 3"));
        assert!(text.contains("lat_count 3"));
    }

    #[test]
    fn prometheus_empty_snapshot_is_empty_string() {
        let collector = MetricsCollector::new();
        assert!(collector.snapshot().to_prometheus_text().is_empty());
    }

    #[test]
    fn json_contains_timestamp() {
        let collector = MetricsCollector::new();
        collector.register_counter("c", "counter");
        let json = collector.snapshot().to_json();
        assert!(json.contains("\"timestamp_ms\":"));
    }

    #[test]
    fn json_contains_all_metric_types() {
        let collector = MetricsCollector::new();
        let c = collector.register_counter("events", "Events");
        c.increment_by(10);
        let g = collector.register_gauge("mem", "Memory");
        g.set(1024.0);
        let h = collector.register_histogram("lat", "Latency", &[1.0]);
        h.observe(0.5);

        let json = collector.snapshot().to_json();
        assert!(json.contains("\"name\":\"events\""));
        assert!(json.contains("\"type\":\"counter\""));
        assert!(json.contains("\"name\":\"mem\""));
        assert!(json.contains("\"type\":\"gauge\""));
        assert!(json.contains("\"name\":\"lat\""));
        assert!(json.contains("\"type\":\"histogram\""));
    }

    #[test]
    fn json_empty_snapshot() {
        let collector = MetricsCollector::new();
        let json = collector.snapshot().to_json();
        assert!(json.contains("\"metrics\":[]"));
    }

    #[test]
    fn json_escapes_quotes_in_description() {
        let collector = MetricsCollector::new();
        collector.register_counter("c", "desc with \"quotes\"");
        let json = collector.snapshot().to_json();
        assert!(json.contains("desc with \\\"quotes\\\""));
    }

    #[test]
    fn prometheus_registry_json_export() {
        let mut reg = PrometheusRegistry::new();
        reg.register_counter("req", "Requests", BTreeMap::new(), 42.0);
        let json = reg.export_json();
        assert!(json.contains("\"name\":\"req\""));
        assert!(json.contains("\"type\":\"counter\""));
        assert!(json.contains("\"value\":42.0"));
    }

    #[test]
    fn prometheus_registry_labels_in_output() {
        let mut reg = PrometheusRegistry::new();
        let mut labels = BTreeMap::new();
        labels.insert("method".to_string(), "GET".to_string());
        labels.insert("status".to_string(), "200".to_string());
        reg.register_counter("http_requests", "HTTP requests", labels, 10.0);
        let output = reg.export_prometheus();
        assert!(output.contains("http_requests{method=\"GET\",status=\"200\"} 10.0"));
    }

    #[test]
    fn prometheus_registry_labels_in_json() {
        let mut reg = PrometheusRegistry::new();
        let mut labels = BTreeMap::new();
        labels.insert("mount".to_string(), "/data".to_string());
        reg.register_gauge("disk", "Disk", labels, 85.0);
        let json = reg.export_json();
        assert!(json.contains("\"mount\":\"/data\""));
    }

    #[test]
    fn snapshot_len_and_is_empty() {
        let collector = MetricsCollector::new();
        assert!(collector.snapshot().is_empty());
        assert_eq!(collector.snapshot().len(), 0);

        collector.register_counter("c", "c");
        collector.register_gauge("g", "g");
        collector.register_histogram("h", "h", &[]);
        let snap = collector.snapshot();
        assert_eq!(snap.len(), 3);
        assert!(!snap.is_empty());
    }

    #[test]
    fn snapshot_reflects_current_values() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "c");
        counter.increment_by(5);
        let text1 = collector.snapshot().to_prometheus_text();
        counter.increment_by(3);
        let text2 = collector.snapshot().to_prometheus_text();
        assert!(text1.contains("c 5"));
        assert!(text2.contains("c 8"));
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 7. Thread safety tests
// ══════════════════════════════════════════════════════════════════════════════

mod thread_safety {
    use super::*;

    #[test]
    fn concurrent_counter_increments() {
        let collector = Arc::new(MetricsCollector::new());
        let counter = collector.register_counter("shared", "shared counter");

        let n_threads = 8usize;
        let increments_per_thread = 1000u64;

        let handles: Vec<_> = (0..n_threads)
            .map(|_| {
                let c = counter.clone();
                std::thread::spawn(move || {
                    for _ in 0..increments_per_thread {
                        c.increment();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread should not panic");
        }

        assert_eq!(counter.value(), n_threads as u64 * increments_per_thread);
    }

    #[test]
    fn concurrent_gauge_set_converges() {
        let collector = Arc::new(MetricsCollector::new());
        let gauge = collector.register_gauge("shared", "shared gauge");

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let g = gauge.clone();
                std::thread::spawn(move || {
                    for _ in 0..100 {
                        g.set(i as f64);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // After all threads complete, the value should be one of {0, 1, 2, 3}
        let val = gauge.value();
        assert!(
            (0.0..=3.0).contains(&val),
            "gauge must hold one of the set values, got {val}"
        );
    }

    #[test]
    fn concurrent_histogram_observations() {
        let collector = Arc::new(MetricsCollector::new());
        let hist = collector.register_histogram("shared", "shared hist", &[]);

        let n_threads = 4usize;
        let obs_per_thread = 500u64;

        let handles: Vec<_> = (0..n_threads)
            .map(|i| {
                let h = hist.clone();
                std::thread::spawn(move || {
                    for j in 0..obs_per_thread {
                        h.observe((i as f64 * 1000.0) + j as f64);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(hist.count(), n_threads as u64 * obs_per_thread);
    }

    #[test]
    fn concurrent_registry_counter_increments() {
        let reg = Arc::new(MetricsRegistry::new());
        let n_threads = 8usize;
        let increments_per_thread = 500u64;

        let handles: Vec<_> = (0..n_threads)
            .map(|_| {
                let r = Arc::clone(&reg);
                std::thread::spawn(move || {
                    for _ in 0..increments_per_thread {
                        r.inc_counter("shared", 1);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(
            counter_value(&reg, "shared"),
            n_threads as u64 * increments_per_thread
        );
    }

    #[test]
    fn concurrent_registry_mixed_operations() {
        let reg = Arc::new(MetricsRegistry::new());
        let n_threads = 4usize;

        let handles: Vec<_> = (0..n_threads)
            .map(|i| {
                let r = Arc::clone(&reg);
                std::thread::spawn(move || {
                    for j in 0..100u64 {
                        r.inc_counter(&format!("counter_{i}"), 1);
                        r.set_gauge(&format!("gauge_{i}"), j as f64);
                        r.observe(&format!("hist_{i}"), j as f64);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let metrics = reg.snapshot();
        // 4 counters + 4 gauges + 4 histograms = 12
        assert_eq!(metrics.len(), n_threads * 3);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 8. Property tests
// ══════════════════════════════════════════════════════════════════════════════

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Counter value equals the exact sum of all deltas.
        #[test]
        fn counter_value_equals_sum(deltas in proptest::collection::vec(0u64..10_000, 1..50)) {
            let collector = MetricsCollector::new();
            let counter = collector.register_counter("c", "test");
            for &d in &deltas {
                counter.increment_by(d);
            }
            let expected: u64 = deltas.iter().sum();
            prop_assert_eq!(counter.value(), expected);
        }

        /// Gauge round-trips finite values bit-exactly.
        #[test]
        fn gauge_round_trips_finite(v in -1e15f64..1e15f64) {
            let collector = MetricsCollector::new();
            let gauge = collector.register_gauge("g", "test");
            gauge.set(v);
            prop_assert_eq!(gauge.value().to_bits(), v.to_bits());
        }

        /// For any histogram: min <= p50 <= max (when count > 0).
        #[test]
        fn histogram_min_le_p50_le_max(
            values in proptest::collection::vec(-1000.0f64..1000.0, 1..100)
        ) {
            let reg = MetricsRegistry::new();
            for &v in &values {
                reg.observe("h", v);
            }
            let metrics = reg.snapshot();
            let summary = find_histogram(&metrics, "h").unwrap();
            prop_assert!(summary.min <= summary.p50, "min ({}) <= p50 ({})", summary.min, summary.p50);
            prop_assert!(summary.p50 <= summary.max, "p50 ({}) <= max ({})", summary.p50, summary.max);
        }

        /// For any histogram: p50 <= p95 <= p99.
        #[test]
        fn histogram_percentile_ordering(
            values in proptest::collection::vec(0.0f64..10000.0, 2..200)
        ) {
            let reg = MetricsRegistry::new();
            for &v in &values {
                reg.observe("h", v);
            }
            let metrics = reg.snapshot();
            let s = find_histogram(&metrics, "h").unwrap();
            prop_assert!(s.p50 <= s.p95, "p50 ({}) <= p95 ({})", s.p50, s.p95);
            prop_assert!(s.p95 <= s.p99, "p95 ({}) <= p99 ({})", s.p95, s.p99);
        }

        /// For any histogram: mean is between min and max.
        #[test]
        fn histogram_mean_bounded(
            values in proptest::collection::vec(0.0f64..1000.0, 1..100)
        ) {
            let reg = MetricsRegistry::new();
            for &v in &values {
                reg.observe("h", v);
            }
            let metrics = reg.snapshot();
            let s = find_histogram(&metrics, "h").unwrap();
            prop_assert!(s.mean >= s.min, "mean ({}) >= min ({})", s.mean, s.min);
            prop_assert!(s.mean <= s.max, "mean ({}) <= max ({})", s.mean, s.max);
        }

        /// After reset, snapshot is always empty regardless of prior state.
        #[test]
        fn reset_always_empties(
            n_counters in 0usize..10,
            n_gauges in 0usize..10,
            n_histograms in 0usize..10,
        ) {
            let reg = MetricsRegistry::new();
            for i in 0..n_counters {
                reg.inc_counter(&format!("c{i}"), 1);
            }
            for i in 0..n_gauges {
                reg.set_gauge(&format!("g{i}"), i as f64);
            }
            for i in 0..n_histograms {
                reg.observe(&format!("h{i}"), i as f64);
            }
            reg.reset();
            prop_assert!(reg.snapshot().is_empty());
        }

        /// Counter handle quantile invariant through export collector.
        #[test]
        fn export_histogram_quantile_ordering(
            values in proptest::collection::vec(0.0f64..10000.0, 2..100)
        ) {
            let collector = MetricsCollector::new();
            let hist = collector.register_histogram("h", "test", &[]);
            for &v in &values {
                hist.observe(v);
            }
            let p50 = hist.quantile(0.50);
            let p95 = hist.quantile(0.95);
            let p99 = hist.quantile(0.99);
            prop_assert!(p50 <= p95, "p50 ({p50}) <= p95 ({p95})");
            prop_assert!(p95 <= p99, "p95 ({p95}) <= p99 ({p99})");
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════════════════

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
