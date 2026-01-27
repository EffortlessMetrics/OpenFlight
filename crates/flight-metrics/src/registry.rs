// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Metrics registry implementation.

use crate::collector::MetricsCollector;
use crate::types::{HistogramSummary, Metric};
use std::collections::{HashMap, VecDeque};
use std::sync::{
    Mutex, RwLock,
    atomic::{AtomicU64, Ordering},
};

/// Thread-safe metrics registry.
pub struct MetricsRegistry {
    counters: RwLock<HashMap<String, AtomicU64>>,
    gauges: RwLock<HashMap<String, AtomicU64>>,
    histograms: RwLock<HashMap<String, Histogram>>,
    max_histogram_samples: usize,
}

impl MetricsRegistry {
    /// Create a new registry with default histogram capacity.
    pub fn new() -> Self {
        Self::with_histogram_capacity(1024)
    }

    /// Create a new registry with a custom histogram sample capacity.
    pub fn with_histogram_capacity(max_histogram_samples: usize) -> Self {
        Self {
            counters: RwLock::new(HashMap::new()),
            gauges: RwLock::new(HashMap::new()),
            histograms: RwLock::new(HashMap::new()),
            max_histogram_samples: max_histogram_samples.max(1),
        }
    }

    /// Increment a counter by the provided delta.
    pub fn inc_counter(&self, name: &str, delta: u64) {
        let mut counters = self
            .counters
            .write()
            .expect("metrics counters lock poisoned");
        let counter = counters
            .entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        counter.fetch_add(delta, Ordering::Relaxed);
    }

    /// Set a gauge value.
    pub fn set_gauge(&self, name: &str, value: f64) {
        let mut gauges = self.gauges.write().expect("metrics gauges lock poisoned");
        let gauge = gauges
            .entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        gauge.store(value.to_bits(), Ordering::Relaxed);
    }

    /// Read a gauge value if present.
    pub fn gauge_value(&self, name: &str) -> Option<f64> {
        let gauges = self.gauges.read().expect("metrics gauges lock poisoned");
        gauges
            .get(name)
            .map(|value| f64::from_bits(value.load(Ordering::Relaxed)))
    }

    /// Observe a histogram sample.
    pub fn observe(&self, name: &str, value: f64) {
        if !value.is_finite() {
            return;
        }

        let mut histograms = self
            .histograms
            .write()
            .expect("metrics histograms lock poisoned");
        let histogram = histograms
            .entry(name.to_string())
            .or_insert_with(|| Histogram::new(self.max_histogram_samples));
        histogram.observe(value);
    }

    /// Snapshot current metrics into a vector.
    pub fn snapshot(&self) -> Vec<Metric> {
        let mut metrics = Vec::new();

        {
            let counters = self
                .counters
                .read()
                .expect("metrics counters lock poisoned");
            for (name, value) in counters.iter() {
                metrics.push(Metric::Counter {
                    name: name.clone(),
                    value: value.load(Ordering::Relaxed),
                });
            }
        }

        {
            let gauges = self.gauges.read().expect("metrics gauges lock poisoned");
            for (name, value) in gauges.iter() {
                metrics.push(Metric::Gauge {
                    name: name.clone(),
                    value: f64::from_bits(value.load(Ordering::Relaxed)),
                });
            }
        }

        {
            let histograms = self
                .histograms
                .read()
                .expect("metrics histograms lock poisoned");
            for (name, histogram) in histograms.iter() {
                if let Some(summary) = histogram.summary() {
                    metrics.push(Metric::Histogram {
                        name: name.clone(),
                        summary,
                    });
                }
            }
        }

        metrics
    }

    /// Reset all metrics.
    pub fn reset(&self) {
        self.counters
            .write()
            .expect("metrics counters lock poisoned")
            .clear();
        self.gauges
            .write()
            .expect("metrics gauges lock poisoned")
            .clear();
        self.histograms
            .write()
            .expect("metrics histograms lock poisoned")
            .clear();
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector for MetricsRegistry {
    fn collect(&self) -> Vec<Metric> {
        self.snapshot()
    }

    fn reset(&self) {
        self.reset();
    }
}

struct Histogram {
    samples: Mutex<VecDeque<f64>>,
    max_samples: usize,
}

impl Histogram {
    fn new(max_samples: usize) -> Self {
        Self {
            samples: Mutex::new(VecDeque::with_capacity(max_samples.min(1024))),
            max_samples,
        }
    }

    fn observe(&self, value: f64) {
        let mut samples = self
            .samples
            .lock()
            .expect("histogram samples lock poisoned");
        samples.push_back(value);
        if samples.len() > self.max_samples {
            samples.pop_front();
        }
    }

    fn summary(&self) -> Option<HistogramSummary> {
        let samples = self
            .samples
            .lock()
            .expect("histogram samples lock poisoned");
        if samples.is_empty() {
            return None;
        }

        let mut sorted: Vec<f64> = samples.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let count = sorted.len();
        let min = *sorted.first().unwrap_or(&0.0);
        let max = *sorted.last().unwrap_or(&0.0);
        let mean = sorted.iter().sum::<f64>() / count as f64;
        let p50 = percentile(&sorted, 0.50);
        let p95 = percentile(&sorted, 0.95);
        let p99 = percentile(&sorted, 0.99);

        Some(HistogramSummary {
            count,
            min,
            max,
            mean,
            p50,
            p95,
            p99,
        })
    }
}

fn percentile(sorted: &[f64], quantile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }

    let idx = ((sorted.len() - 1) as f64 * quantile).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_and_gauge_snapshot() {
        let registry = MetricsRegistry::new();
        registry.inc_counter("updates", 2);
        registry.set_gauge("latency_ms", 12.5);

        let metrics = registry.snapshot();

        let counter = metrics.iter().find_map(|metric| match metric {
            Metric::Counter { name, value } if name == "updates" => Some(*value),
            _ => None,
        });
        assert_eq!(counter, Some(2));

        let gauge = metrics.iter().find_map(|metric| match metric {
            Metric::Gauge { name, value } if name == "latency_ms" => Some(*value),
            _ => None,
        });
        assert_eq!(gauge, Some(12.5));
    }

    #[test]
    fn test_histogram_summary() {
        let registry = MetricsRegistry::new();
        registry.observe("latency", 1.0);
        registry.observe("latency", 2.0);
        registry.observe("latency", 3.0);

        let metrics = registry.snapshot();
        let summary = metrics.iter().find_map(|metric| match metric {
            Metric::Histogram { name, summary } if name == "latency" => Some(summary.clone()),
            _ => None,
        });

        let summary = summary.expect("histogram summary missing");
        assert_eq!(summary.count, 3);
        assert_eq!(summary.min, 1.0);
        assert_eq!(summary.max, 3.0);
        assert_eq!(summary.mean, 2.0);
    }

    #[test]
    fn test_reset_clears_metrics() {
        let registry = MetricsRegistry::new();
        registry.inc_counter("updates", 1);
        registry.set_gauge("latency_ms", 9.0);
        registry.observe("latency", 4.0);

        registry.reset();

        let metrics = registry.snapshot();
        assert!(metrics.is_empty());
    }
}
