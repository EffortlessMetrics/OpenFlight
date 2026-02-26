// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Metric collection trait.

use crate::types::Metric;

/// Common interface for metric collection backends.
pub trait MetricsCollector {
    /// Collect a snapshot of metrics.
    fn collect(&self) -> Vec<Metric>;

    /// Reset stored metrics.
    fn reset(&self);
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
        let found = metrics.iter().any(|m| {
            matches!(m, Metric::Counter { name, value } if name == "calls" && *value == 5)
        });
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
        let via_trait = { let c: &dyn MetricsCollector = &reg; c.collect() };
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
            value: 3.14,
        }];
        let collector: Box<dyn MetricsCollector> = Box::new(FakeCollector::new(metrics));
        assert_eq!(collector.collect().len(), 1);
    }
}
