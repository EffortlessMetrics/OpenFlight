// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! External test suite for `flight-metrics`.
//!
//! Covers counter monotonicity, gauge round-trips, histogram statistics,
//! thread-safety under concurrent access, reset semantics, snapshot structure,
//! metric name isolation, zero-value edge cases, and property-based invariants.

use flight_metrics::{HistogramSummary, Metric, MetricsRegistry};
use std::sync::Arc;

// ── 1. Counter monotonicity ──────────────────────────────────────────────────

/// Counter values must be strictly non-decreasing across successive snapshots.
#[test]
fn counter_is_monotonic() {
    let reg = MetricsRegistry::new();

    reg.inc_counter("ticks", 5);
    let v1 = counter_value(&reg, "ticks");

    reg.inc_counter("ticks", 10);
    let v2 = counter_value(&reg, "ticks");

    assert!(v2 >= v1, "counter must never decrease: v1={v1} v2={v2}");
    assert_eq!(v2, 15, "counter must accumulate all deltas");
}

/// Incrementing by 0 is idempotent — value stays the same.
#[test]
fn counter_increment_by_zero_is_idempotent() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("ticks", 7);
    reg.inc_counter("ticks", 0);
    assert_eq!(counter_value(&reg, "ticks"), 7);
}

// ── 2. Gauge set / read-back ─────────────────────────────────────────────────

/// The last `set_gauge` call wins; older values are discarded.
#[test]
fn gauge_overwrite_returns_latest_value() {
    let reg = MetricsRegistry::new();
    reg.set_gauge("temperature", 20.0);
    reg.set_gauge("temperature", 99.5);

    let val = reg.gauge_value("temperature").expect("gauge must exist");
    assert!(
        (val - 99.5).abs() < f64::EPSILON,
        "gauge must hold last written value, got {val}"
    );
}

/// A negative gauge value round-trips without sign loss.
#[test]
fn gauge_negative_value_round_trips() {
    let reg = MetricsRegistry::new();
    reg.set_gauge("offset", -12.75);
    let val = reg.gauge_value("offset").expect("gauge must exist");
    assert!((val - (-12.75)).abs() < f64::EPSILON, "got {val}");
}

/// A gauge missing from the registry returns `None`.
#[test]
fn gauge_missing_name_returns_none() {
    let reg = MetricsRegistry::new();
    assert!(reg.gauge_value("does_not_exist").is_none());
}

// ── 3. Histogram records samples within expected bounds ──────────────────────

/// `min`, `max`, and `count` must precisely reflect the observed samples.
#[test]
fn histogram_summary_has_correct_min_max_count() {
    let reg = MetricsRegistry::new();
    let samples = [1.0_f64, 5.0, 3.0, 2.0, 4.0];
    for &s in &samples {
        reg.observe("latency", s);
    }

    let metrics = reg.snapshot();
    let summary = find_histogram(&metrics, "latency").expect("histogram summary must be present");

    assert_eq!(summary.count, samples.len());
    assert_eq!(summary.min, 1.0);
    assert_eq!(summary.max, 5.0);
}

/// A single-sample histogram has all percentiles equal to that sample.
#[test]
fn histogram_single_sample_has_all_percentiles_equal() {
    let reg = MetricsRegistry::new();
    reg.observe("jitter", 42.0);

    let metrics = reg.snapshot();
    let s = find_histogram(&metrics, "jitter").expect("must have summary");

    assert_eq!(s.min, 42.0);
    assert_eq!(s.max, 42.0);
    assert_eq!(s.mean, 42.0);
    assert_eq!(s.p50, 42.0);
    assert_eq!(s.p95, 42.0);
    assert_eq!(s.p99, 42.0);
}

/// Non-finite samples are silently discarded and do not corrupt the summary.
#[test]
fn histogram_ignores_nan_and_infinity() {
    let reg = MetricsRegistry::new();
    reg.observe("sensor", f64::NAN);
    reg.observe("sensor", f64::INFINITY);
    reg.observe("sensor", f64::NEG_INFINITY);
    reg.observe("sensor", 7.0);

    let metrics = reg.snapshot();
    let s = find_histogram(&metrics, "sensor").expect("must have summary");
    assert_eq!(s.count, 1, "only the finite sample should be counted");
    assert_eq!(s.min, 7.0);
    assert_eq!(s.max, 7.0);
}

// ── 4. Thread-safe: concurrent counter increments ────────────────────────────

/// Many threads incrementing the same counter by 1 must yield the exact total.
#[test]
fn concurrent_counter_increments_are_correct() {
    let reg = Arc::new(MetricsRegistry::new());
    let n_threads = 8usize;
    let increments_per_thread = 200u64;

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
        h.join().expect("thread should not panic");
    }

    let total = counter_value(&reg, "shared");
    assert_eq!(
        total,
        n_threads as u64 * increments_per_thread,
        "all increments must be reflected in the final counter value"
    );
}

/// Multiple threads can set different gauges concurrently without interfering.
#[test]
fn concurrent_distinct_gauges_are_independent() {
    let reg = Arc::new(MetricsRegistry::new());

    let handles: Vec<_> = (0..4usize)
        .map(|i| {
            let r = Arc::clone(&reg);
            std::thread::spawn(move || {
                let name = format!("gauge_{i}");
                r.set_gauge(&name, i as f64 * 10.0);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    for i in 0..4usize {
        let name = format!("gauge_{i}");
        let val = reg.gauge_value(&name).expect("gauge must exist");
        assert!(
            (val - i as f64 * 10.0).abs() < f64::EPSILON,
            "gauge_{i} should be {}, got {val}",
            i as f64 * 10.0
        );
    }
}

// ── 5. Reset clears all metrics ──────────────────────────────────────────────

/// After reset, snapshot is empty and gauge lookup returns None.
#[test]
fn reset_removes_all_metric_types() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("c", 10);
    reg.set_gauge("g", 3.0);
    reg.observe("h", 7.0);

    assert_eq!(
        reg.snapshot().len(),
        3,
        "pre-reset snapshot should have 3 entries"
    );

    reg.reset();

    assert!(
        reg.snapshot().is_empty(),
        "snapshot must be empty after reset"
    );
    assert!(
        reg.gauge_value("g").is_none(),
        "gauge must be gone after reset"
    );
    assert_eq!(
        counter_value(&reg, "c"),
        0,
        "counter must be gone after reset"
    );
}

/// Resetting twice in a row is safe and idempotent.
#[test]
fn double_reset_is_safe() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("x", 5);
    reg.reset();
    reg.reset(); // must not panic
    assert!(reg.snapshot().is_empty());
}

// ── 6. Snapshot contains expected metric kinds ───────────────────────────────

/// Each metric kind appears in the snapshot with its exact registered name.
#[test]
fn snapshot_contains_correct_metric_variants_with_correct_names() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("events_total", 1);
    reg.set_gauge("rate_hz", 50.0);
    reg.observe("duration_ms", 15.0);

    let metrics = reg.snapshot();

    let has_counter = metrics
        .iter()
        .any(|m| matches!(m, Metric::Counter { name, .. } if name == "events_total"));
    let has_gauge = metrics
        .iter()
        .any(|m| matches!(m, Metric::Gauge { name, .. } if name == "rate_hz"));
    let has_histogram = metrics
        .iter()
        .any(|m| matches!(m, Metric::Histogram { name, .. } if name == "duration_ms"));

    assert!(
        has_counter,
        "snapshot must contain the counter 'events_total'"
    );
    assert!(has_gauge, "snapshot must contain the gauge 'rate_hz'");
    assert!(
        has_histogram,
        "snapshot must contain the histogram 'duration_ms'"
    );
}

/// An empty registry produces an empty snapshot.
#[test]
fn empty_registry_snapshot_is_empty() {
    let reg = MetricsRegistry::new();
    assert!(reg.snapshot().is_empty());
}

// ── 7. Named metrics don't conflict ─────────────────────────────────────────

/// Two counters with different names accumulate their deltas independently.
#[test]
fn distinct_counters_accumulate_independently() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("alpha", 3);
    reg.inc_counter("beta", 7);
    reg.inc_counter("alpha", 2); // only alpha grows

    assert_eq!(counter_value(&reg, "alpha"), 5, "alpha counter must be 5");
    assert_eq!(counter_value(&reg, "beta"), 7, "beta counter must be 7");
}

/// Two gauges with different names are completely independent.
#[test]
fn distinct_gauges_are_independent() {
    let reg = MetricsRegistry::new();
    reg.set_gauge("x", 1.0);
    reg.set_gauge("y", 2.0);

    assert!((reg.gauge_value("x").unwrap() - 1.0).abs() < f64::EPSILON);
    assert!((reg.gauge_value("y").unwrap() - 2.0).abs() < f64::EPSILON);
}

/// Writing to one histogram does not affect another.
#[test]
fn distinct_histograms_are_independent() {
    let reg = MetricsRegistry::new();
    reg.observe("h_a", 10.0);
    reg.observe("h_b", 20.0);

    let metrics = reg.snapshot();
    let a = find_histogram(&metrics, "h_a").expect("h_a must exist");
    let b = find_histogram(&metrics, "h_b").expect("h_b must exist");

    assert_eq!(a.min, 10.0);
    assert_eq!(b.min, 20.0);
    assert_eq!(a.count, 1);
    assert_eq!(b.count, 1);
}

// ── 8. Zero-value counter appears in snapshot ────────────────────────────────

/// A counter incremented by 0 still exists in the snapshot with value 0.
#[test]
fn counter_incremented_by_zero_appears_in_snapshot() {
    let reg = MetricsRegistry::new();
    reg.inc_counter("zero_counter", 0);

    let metrics = reg.snapshot();
    let found = metrics.iter().find_map(|m| match m {
        Metric::Counter { name, value } if name == "zero_counter" => Some(*value),
        _ => None,
    });
    assert_eq!(
        found,
        Some(0),
        "zero-increment counter must appear in snapshot with value 0"
    );
}

/// A gauge set to 0.0 round-trips correctly.
#[test]
fn zero_gauge_round_trips() {
    let reg = MetricsRegistry::new();
    reg.set_gauge("zero_gauge", 0.0);
    let val = reg.gauge_value("zero_gauge").expect("gauge must exist");
    assert_eq!(val, 0.0);
}

// ── 9. Max/min histogram values tracked correctly ────────────────────────────

/// Min and max must exactly reflect the smallest and largest observed samples.
#[test]
fn histogram_min_max_track_extremes() {
    let reg = MetricsRegistry::new();
    let values = [100.0_f64, 0.001, 999.9, 50.0, 0.001, 1000.0];
    for &v in &values {
        reg.observe("sensor", v);
    }

    let metrics = reg.snapshot();
    let s = find_histogram(&metrics, "sensor").expect("must have summary");

    assert_eq!(s.min, 0.001, "min must equal smallest observed value");
    assert_eq!(s.max, 1000.0, "max must equal largest observed value");
}

/// Percentile ordering invariant: p50 ≤ p95 ≤ p99.
#[test]
fn histogram_percentile_ordering_invariant() {
    let reg = MetricsRegistry::new();
    for v in 1..=100 {
        reg.observe("range", v as f64);
    }

    let metrics = reg.snapshot();
    let s = find_histogram(&metrics, "range").expect("must have summary");

    assert!(s.p50 <= s.p95, "p50 ({}) must be <= p95 ({})", s.p50, s.p95);
    assert!(s.p95 <= s.p99, "p95 ({}) must be <= p99 ({})", s.p95, s.p99);
}

/// Mean is bounded by [min, max].
#[test]
fn histogram_mean_between_min_and_max() {
    let reg = MetricsRegistry::new();
    for v in [2.0_f64, 4.0, 6.0, 8.0, 10.0] {
        reg.observe("vals", v);
    }

    let metrics = reg.snapshot();
    let s = find_histogram(&metrics, "vals").expect("must have summary");

    assert!(
        s.mean >= s.min && s.mean <= s.max,
        "mean {} must be within [{}, {}]",
        s.mean,
        s.min,
        s.max
    );
    assert!(
        (s.mean - 6.0).abs() < f64::EPSILON,
        "mean should be 6.0, got {}",
        s.mean
    );
}

// ── 10. proptest: counter increments stay non-negative ───────────────────────

use proptest::prelude::*;

proptest! {
    /// Counter value must equal the exact sum of all deltas applied to it.
    #[test]
    fn counter_equals_sum_of_deltas(deltas in proptest::collection::vec(0u64..100_000u64, 1..100)) {
        let reg = MetricsRegistry::new();
        for &d in &deltas {
            reg.inc_counter("ct", d);
        }
        let val = counter_value(&reg, "ct");
        let expected: u64 = deltas.iter().sum();
        prop_assert_eq!(val, expected, "counter must equal the exact sum of all deltas");
    }

    /// A finite f64 gauge value must be bit-exact after a round-trip through the registry.
    #[test]
    fn gauge_round_trips_finite_values(v in -1e15f64..1e15f64) {
        let reg = MetricsRegistry::new();
        reg.set_gauge("g", v);
        let got = reg.gauge_value("g").expect("gauge must exist");
        // Stored as bits (f64::to_bits / f64::from_bits), so must be bit-exact.
        prop_assert_eq!(got.to_bits(), v.to_bits(), "gauge must be bit-exact round-trip");
    }

    /// After reset, no previously recorded metric remains in the snapshot.
    #[test]
    fn reset_leaves_no_trace_of_previous_counters(
        names in proptest::collection::vec("[a-z]{2,6}", 1..8),
        values in proptest::collection::vec(1u64..1000u64, 1..8),
    ) {
        let reg = MetricsRegistry::new();
        for (name, &value) in names.iter().zip(values.iter()) {
            reg.inc_counter(name, value);
        }
        reg.reset();
        prop_assert!(reg.snapshot().is_empty(), "snapshot must be empty after reset");
    }
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
