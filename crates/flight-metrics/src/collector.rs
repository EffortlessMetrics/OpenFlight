// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Metric collection trait and atomic name-based collector.

use crate::histogram::FixedBucketHistogram;
use crate::types::Metric;
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Common interface for metric collection backends.
pub trait MetricsCollector {
    /// Collect a snapshot of metrics.
    fn collect(&self) -> Vec<Metric>;

    /// Reset stored metrics.
    fn reset(&self);
}

/// Name-keyed metrics collector using atomics for lock-free recording.
///
/// Register counters, gauges, and histograms up front, then record values
/// without heap allocation. Counters use [`AtomicU64`], gauges use
/// [`AtomicI64`], and histograms use [`FixedBucketHistogram`].
pub struct AtomicCollector {
    counters: RwLock<HashMap<String, AtomicU64>>,
    gauges: RwLock<HashMap<String, AtomicI64>>,
    histograms: RwLock<HashMap<String, FixedBucketHistogram>>,
}

impl AtomicCollector {
    /// Create an empty collector.
    pub fn new() -> Self {
        Self {
            counters: RwLock::new(HashMap::new()),
            gauges: RwLock::new(HashMap::new()),
            histograms: RwLock::new(HashMap::new()),
        }
    }

    /// Register a counter. No-op if the name already exists.
    pub fn register_counter(&self, name: &str) {
        let mut map = self.counters.write().expect("counters lock poisoned");
        map.entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0));
    }

    /// Register a gauge. No-op if the name already exists.
    pub fn register_gauge(&self, name: &str) {
        let mut map = self.gauges.write().expect("gauges lock poisoned");
        map.entry(name.to_string())
            .or_insert_with(|| AtomicI64::new(0));
    }

    /// Register a histogram with bucket boundaries. No-op if the name exists.
    pub fn register_histogram(&self, name: &str, buckets: &[f64]) {
        let mut map = self.histograms.write().expect("histograms lock poisoned");
        map.entry(name.to_string())
            .or_insert_with(|| FixedBucketHistogram::new(buckets));
    }

    /// Increment a counter by 1. No-op if unregistered.
    pub fn counter_inc(&self, name: &str) {
        let map = self.counters.read().expect("counters lock poisoned");
        if let Some(c) = map.get(name) {
            c.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Read the current counter value.
    pub fn counter_get(&self, name: &str) -> Option<u64> {
        let map = self.counters.read().expect("counters lock poisoned");
        map.get(name).map(|c| c.load(Ordering::Relaxed))
    }

    /// Set a gauge to an absolute value. No-op if unregistered.
    pub fn gauge_set(&self, name: &str, value: i64) {
        let map = self.gauges.read().expect("gauges lock poisoned");
        if let Some(g) = map.get(name) {
            g.store(value, Ordering::Relaxed);
        }
    }

    /// Read the current gauge value.
    pub fn gauge_get(&self, name: &str) -> Option<i64> {
        let map = self.gauges.read().expect("gauges lock poisoned");
        map.get(name).map(|g| g.load(Ordering::Relaxed))
    }

    /// Record a histogram observation. No-op if unregistered.
    pub fn histogram_observe(&self, name: &str, value: f64) {
        let map = self.histograms.read().expect("histograms lock poisoned");
        if let Some(h) = map.get(name) {
            h.observe(value);
        }
    }
}

impl Default for AtomicCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MetricsRegistry;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    // ── MetricsRegistry as MetricsCollector ──────────────────────────────────

    #[test]
    fn registry_collect_via_trait_returns_recorded_counter() {
        let reg = MetricsRegistry::new();
        reg.inc_counter("calls", 5);
        let collector: &dyn MetricsCollector = &reg;
        let metrics = collector.collect();
        let found = metrics.iter().any(
            |m| matches!(m, Metric::Counter { name, value } if name == "calls" && *value == 5),
        );
        assert!(found);
    }

    #[test]
    fn registry_reset_via_trait_clears_all_metrics() {
        let reg = MetricsRegistry::new();
        reg.inc_counter("events", 10);
        reg.set_gauge("temp", 37.0);
        let collector: &dyn MetricsCollector = &reg;
        collector.reset();
        assert!(collector.collect().is_empty());
    }

    #[test]
    fn registry_collect_equals_snapshot() {
        let reg = MetricsRegistry::new();
        reg.inc_counter("x", 3);
        reg.set_gauge("y", 1.5);
        reg.observe("z", 9.0);
        let via_trait = {
            let c: &dyn MetricsCollector = &reg;
            c.collect()
        };
        let via_snapshot = reg.snapshot();
        // Same number of entries (order may differ)
        assert_eq!(via_trait.len(), via_snapshot.len());
    }

    // ── Custom MetricsCollector implementation ───────────────────────────────

    struct FakeCollector {
        metrics: Vec<Metric>,
        reset_called: Arc<AtomicBool>,
    }

    impl FakeCollector {
        fn new(metrics: Vec<Metric>) -> Self {
            Self {
                metrics,
                reset_called: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    impl MetricsCollector for FakeCollector {
        fn collect(&self) -> Vec<Metric> {
            self.metrics.clone()
        }
        fn reset(&self) {
            self.reset_called.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn custom_collector_returns_supplied_metrics() {
        let metrics = vec![Metric::Counter {
            name: "hits".to_string(),
            value: 42,
        }];
        let collector = FakeCollector::new(metrics.clone());
        assert_eq!(collector.collect(), metrics);
    }

    #[test]
    fn custom_collector_reset_flag_is_set() {
        let collector = FakeCollector::new(vec![]);
        let flag = Arc::clone(&collector.reset_called);
        collector.reset();
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn custom_collector_empty_collect_returns_empty_vec() {
        let collector = FakeCollector::new(vec![]);
        assert!(collector.collect().is_empty());
    }

    #[test]
    fn custom_collector_usable_as_dyn_trait() {
        let metrics = vec![Metric::Gauge {
            name: "g".to_string(),
            value: 4.56,
        }];
        let collector: Box<dyn MetricsCollector> = Box::new(FakeCollector::new(metrics));
        assert_eq!(collector.collect().len(), 1);
    }

    // ── AtomicCollector ─────────────────────────────────────────────────────

    #[test]
    fn atomic_counter_inc_and_get() {
        let ac = AtomicCollector::new();
        ac.register_counter("hits");
        ac.counter_inc("hits");
        ac.counter_inc("hits");
        ac.counter_inc("hits");
        assert_eq!(ac.counter_get("hits"), Some(3));
    }

    #[test]
    fn atomic_counter_unregistered_is_none() {
        let ac = AtomicCollector::new();
        assert_eq!(ac.counter_get("missing"), None);
    }

    #[test]
    fn atomic_counter_inc_unregistered_is_noop() {
        let ac = AtomicCollector::new();
        ac.counter_inc("missing"); // should not panic
    }

    #[test]
    fn atomic_gauge_set_and_get() {
        let ac = AtomicCollector::new();
        ac.register_gauge("temp");
        ac.gauge_set("temp", 42);
        assert_eq!(ac.gauge_get("temp"), Some(42));
    }

    #[test]
    fn atomic_gauge_overwrite() {
        let ac = AtomicCollector::new();
        ac.register_gauge("level");
        ac.gauge_set("level", 10);
        ac.gauge_set("level", -5);
        assert_eq!(ac.gauge_get("level"), Some(-5));
    }

    #[test]
    fn atomic_histogram_observe() {
        let ac = AtomicCollector::new();
        ac.register_histogram("lat", &[1.0, 5.0, 10.0]);
        ac.histogram_observe("lat", 2.5);
        ac.histogram_observe("lat", 7.0);
        // Verify via counter_get that the histogram is tracked
        // (no direct getter, but at least confirm no panic)
    }

    #[test]
    fn atomic_histogram_unregistered_is_noop() {
        let ac = AtomicCollector::new();
        ac.histogram_observe("missing", 1.0); // should not panic
    }
}
