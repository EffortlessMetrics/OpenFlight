// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Metrics export with lock-free handles and Prometheus/JSON output.
//!
//! [`MetricsCollector`] provides a registration-based approach where each metric
//! returns a typed handle ([`CounterHandle`], [`GaugeHandle`], [`HistogramHandle`])
//! for lock-free recording. Call [`MetricsCollector::snapshot`] to capture a
//! [`MetricsSnapshot`] exportable in Prometheus exposition or JSON format.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

// ── Internal metric storage ────────────────────────────────────────────────

struct CounterInner {
    name: String,
    description: String,
    value: AtomicU64,
}

struct GaugeInner {
    name: String,
    description: String,
    value: AtomicU64,
}

struct HistogramInner {
    name: String,
    description: String,
    buckets: Vec<f64>,
    count: AtomicU64,
    sum: AtomicU64,
    observations: Mutex<Vec<f64>>,
}

// ── Handles ────────────────────────────────────────────────────────────────

/// Lock-free handle to a counter metric.
#[derive(Clone)]
pub struct CounterHandle(Arc<CounterInner>);

impl CounterHandle {
    /// Increment by 1.
    pub fn increment(&self) {
        self.0.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment by `n`.
    pub fn increment_by(&self, n: u64) {
        self.0.value.fetch_add(n, Ordering::Relaxed);
    }

    /// Read the current value.
    pub fn value(&self) -> u64 {
        self.0.value.load(Ordering::Relaxed)
    }
}

/// Lock-free handle to a gauge metric.
#[derive(Clone)]
pub struct GaugeHandle(Arc<GaugeInner>);

impl GaugeHandle {
    /// Set to an absolute value.
    pub fn set(&self, value: f64) {
        self.0.value.store(value.to_bits(), Ordering::Relaxed);
    }

    /// Increment by 1.0 (atomic CAS loop).
    pub fn increment(&self) {
        self.add(1.0);
    }

    /// Decrement by 1.0 (atomic CAS loop).
    pub fn decrement(&self) {
        self.add(-1.0);
    }

    /// Read the current value.
    pub fn value(&self) -> f64 {
        f64::from_bits(self.0.value.load(Ordering::Relaxed))
    }

    fn add(&self, delta: f64) {
        loop {
            let current = self.0.value.load(Ordering::Relaxed);
            let new = f64::from_bits(current) + delta;
            if self
                .0
                .value
                .compare_exchange_weak(current, new.to_bits(), Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }
}

/// Handle to a histogram metric.
#[derive(Clone)]
pub struct HistogramHandle(Arc<HistogramInner>);

impl HistogramHandle {
    /// Record an observation. Non-finite values are silently dropped.
    pub fn observe(&self, value: f64) {
        if !value.is_finite() {
            return;
        }
        self.0.count.fetch_add(1, Ordering::Relaxed);
        // Atomic f64 add via CAS loop.
        loop {
            let current = self.0.sum.load(Ordering::Relaxed);
            let new = f64::from_bits(current) + value;
            if self
                .0
                .sum
                .compare_exchange_weak(current, new.to_bits(), Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
        let mut obs = self.0.observations.lock().expect("histogram lock poisoned");
        obs.push(value);
    }

    /// Total number of observations.
    pub fn count(&self) -> u64 {
        self.0.count.load(Ordering::Relaxed)
    }

    /// Sum of all observations.
    pub fn sum(&self) -> f64 {
        f64::from_bits(self.0.sum.load(Ordering::Relaxed))
    }

    /// Compute quantile `p` (0.0..=1.0) using the sorted-array approach.
    pub fn quantile(&self, p: f64) -> f64 {
        let obs = self.0.observations.lock().expect("histogram lock poisoned");
        if obs.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<f64> = obs.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((sorted.len() - 1) as f64 * p.clamp(0.0, 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

// ── MetricsSnapshot ────────────────────────────────────────────────────────

/// Point-in-time snapshot of all metrics, exportable to Prometheus or JSON.
pub struct MetricsSnapshot {
    /// When the snapshot was captured.
    pub timestamp: SystemTime,
    entries: Vec<SnapshotEntry>,
}

enum SnapshotEntry {
    Counter {
        name: String,
        description: String,
        value: u64,
    },
    Gauge {
        name: String,
        description: String,
        value: f64,
    },
    Histogram {
        name: String,
        description: String,
        buckets: Vec<f64>,
        count: u64,
        sum: f64,
        observations: Vec<f64>,
    },
}

impl MetricsSnapshot {
    /// Render in Prometheus exposition text format.
    pub fn to_prometheus_text(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            match entry {
                SnapshotEntry::Counter {
                    name,
                    description,
                    value,
                } => {
                    out.push_str(&format!("# HELP {name} {description}\n"));
                    out.push_str(&format!("# TYPE {name} counter\n"));
                    out.push_str(&format!("{name} {value}\n"));
                }
                SnapshotEntry::Gauge {
                    name,
                    description,
                    value,
                } => {
                    out.push_str(&format!("# HELP {name} {description}\n"));
                    out.push_str(&format!("# TYPE {name} gauge\n"));
                    out.push_str(&format!("{name} {}\n", format_f64(*value)));
                }
                SnapshotEntry::Histogram {
                    name,
                    description,
                    buckets,
                    count,
                    sum,
                    observations,
                } => {
                    out.push_str(&format!("# HELP {name} {description}\n"));
                    out.push_str(&format!("# TYPE {name} histogram\n"));

                    let mut sorted = observations.clone();
                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

                    for &bound in buckets {
                        let bucket_count = sorted.iter().filter(|&&v| v <= bound).count();
                        out.push_str(&format!(
                            "{name}_bucket{{le=\"{}\"}} {bucket_count}\n",
                            format_f64(bound),
                        ));
                    }
                    out.push_str(&format!("{name}_bucket{{le=\"+Inf\"}} {count}\n"));
                    out.push_str(&format!("{name}_sum {}\n", format_f64(*sum)));
                    out.push_str(&format!("{name}_count {count}\n"));
                }
            }
        }
        out
    }

    /// Render as a JSON string.
    pub fn to_json(&self) -> String {
        let timestamp_ms = self
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        let entries_json: Vec<String> = self
            .entries
            .iter()
            .map(|entry| match entry {
                SnapshotEntry::Counter {
                    name,
                    description,
                    value,
                } => {
                    format!(
                        "{{\"name\":\"{}\",\"type\":\"counter\",\
                         \"description\":\"{}\",\"value\":{}}}",
                        escape_json(name),
                        escape_json(description),
                        value,
                    )
                }
                SnapshotEntry::Gauge {
                    name,
                    description,
                    value,
                } => {
                    format!(
                        "{{\"name\":\"{}\",\"type\":\"gauge\",\
                         \"description\":\"{}\",\"value\":{}}}",
                        escape_json(name),
                        escape_json(description),
                        format_f64(*value),
                    )
                }
                SnapshotEntry::Histogram {
                    name,
                    description,
                    count,
                    sum,
                    ..
                } => {
                    format!(
                        "{{\"name\":\"{}\",\"type\":\"histogram\",\
                         \"description\":\"{}\",\"count\":{},\"sum\":{}}}",
                        escape_json(name),
                        escape_json(description),
                        count,
                        format_f64(*sum),
                    )
                }
            })
            .collect();

        format!(
            "{{\"timestamp_ms\":{timestamp_ms},\"metrics\":[{}]}}",
            entries_json.join(","),
        )
    }

    /// Number of metric entries in this snapshot.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether this snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn format_f64(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ── MetricsCollector ───────────────────────────────────────────────────────

/// Registration-based metrics collector with lock-free recording handles.
///
/// Counters and gauges use [`AtomicU64`] for lock-free operations.
/// Histograms use a [`Mutex`] only for observation storage (quantile queries).
pub struct MetricsCollector {
    counters: RwLock<Vec<Arc<CounterInner>>>,
    gauges: RwLock<Vec<Arc<GaugeInner>>>,
    histograms: RwLock<Vec<Arc<HistogramInner>>>,
}

impl MetricsCollector {
    /// Create an empty collector.
    pub fn new() -> Self {
        Self {
            counters: RwLock::new(Vec::new()),
            gauges: RwLock::new(Vec::new()),
            histograms: RwLock::new(Vec::new()),
        }
    }

    /// Register a counter and return a lock-free handle.
    pub fn register_counter(&self, name: &str, description: &str) -> CounterHandle {
        let inner = Arc::new(CounterInner {
            name: name.to_string(),
            description: description.to_string(),
            value: AtomicU64::new(0),
        });
        let handle = CounterHandle(Arc::clone(&inner));
        self.counters
            .write()
            .expect("counters lock poisoned")
            .push(inner);
        handle
    }

    /// Register a gauge and return a lock-free handle.
    pub fn register_gauge(&self, name: &str, description: &str) -> GaugeHandle {
        let inner = Arc::new(GaugeInner {
            name: name.to_string(),
            description: description.to_string(),
            value: AtomicU64::new(0.0f64.to_bits()),
        });
        let handle = GaugeHandle(Arc::clone(&inner));
        self.gauges
            .write()
            .expect("gauges lock poisoned")
            .push(inner);
        handle
    }

    /// Register a histogram with bucket boundaries and return a handle.
    pub fn register_histogram(
        &self,
        name: &str,
        description: &str,
        buckets: &[f64],
    ) -> HistogramHandle {
        let inner = Arc::new(HistogramInner {
            name: name.to_string(),
            description: description.to_string(),
            buckets: buckets.to_vec(),
            count: AtomicU64::new(0),
            sum: AtomicU64::new(0.0f64.to_bits()),
            observations: Mutex::new(Vec::new()),
        });
        let handle = HistogramHandle(Arc::clone(&inner));
        self.histograms
            .write()
            .expect("histograms lock poisoned")
            .push(inner);
        handle
    }

    /// Take a point-in-time snapshot of all registered metrics.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let timestamp = SystemTime::now();
        let mut entries = Vec::new();

        {
            let counters = self.counters.read().expect("counters lock poisoned");
            for c in counters.iter() {
                entries.push(SnapshotEntry::Counter {
                    name: c.name.clone(),
                    description: c.description.clone(),
                    value: c.value.load(Ordering::Relaxed),
                });
            }
        }

        {
            let gauges = self.gauges.read().expect("gauges lock poisoned");
            for g in gauges.iter() {
                entries.push(SnapshotEntry::Gauge {
                    name: g.name.clone(),
                    description: g.description.clone(),
                    value: f64::from_bits(g.value.load(Ordering::Relaxed)),
                });
            }
        }

        {
            let histograms = self.histograms.read().expect("histograms lock poisoned");
            for h in histograms.iter() {
                let obs = h.observations.lock().expect("histogram lock poisoned");
                entries.push(SnapshotEntry::Histogram {
                    name: h.name.clone(),
                    description: h.description.clone(),
                    buckets: h.buckets.clone(),
                    count: h.count.load(Ordering::Relaxed),
                    sum: f64::from_bits(h.sum.load(Ordering::Relaxed)),
                    observations: obs.clone(),
                });
            }
        }

        MetricsSnapshot { timestamp, entries }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

// ── Built-in flight metrics ────────────────────────────────────────────────

/// Pre-registered flight simulation metrics.
///
/// Created via [`FlightMetrics::register`] on an existing [`MetricsCollector`].
pub struct FlightMetrics {
    /// Histogram of axis tick processing latency in microseconds.
    pub axis_processing_latency_us: HistogramHandle,
    /// Counter of bus events by type.
    pub bus_events_total: CounterHandle,
    /// Gauge of currently connected devices.
    pub device_count: GaugeHandle,
    /// Counter of adapter reconnections per simulator.
    pub adapter_reconnections_total: CounterHandle,
    /// Counter of profile switches.
    pub profile_switches_total: CounterHandle,
    /// Histogram of FFB output magnitude.
    pub ffb_force_magnitude: HistogramHandle,
    /// Gauge of current memory usage in bytes.
    pub memory_usage_bytes: GaugeHandle,
}

/// Default latency histogram bucket boundaries in microseconds.
const LATENCY_BUCKETS_US: &[f64] = &[50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0];

/// Default FFB force magnitude bucket boundaries.
const FORCE_BUCKETS: &[f64] = &[0.1, 0.25, 0.5, 0.75, 1.0, 2.0, 5.0, 10.0];

impl FlightMetrics {
    /// Register all built-in flight metrics on the given collector.
    pub fn register(collector: &MetricsCollector) -> Self {
        Self {
            axis_processing_latency_us: collector.register_histogram(
                "axis_processing_latency_us",
                "Histogram of axis tick processing latency in microseconds",
                LATENCY_BUCKETS_US,
            ),
            bus_events_total: collector
                .register_counter("bus_events_total", "Total number of bus events by type"),
            device_count: collector
                .register_gauge("device_count", "Number of currently connected devices"),
            adapter_reconnections_total: collector.register_counter(
                "adapter_reconnections_total",
                "Total adapter reconnections per simulator",
            ),
            profile_switches_total: collector
                .register_counter("profile_switches_total", "Total number of profile switches"),
            ffb_force_magnitude: collector.register_histogram(
                "ffb_force_magnitude",
                "Histogram of force feedback output magnitude",
                FORCE_BUCKETS,
            ),
            memory_usage_bytes: collector
                .register_gauge("memory_usage_bytes", "Current memory usage in bytes"),
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Counter ────────────────────────────────────────────────────────────

    #[test]
    fn counter_starts_at_zero() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "test counter");
        assert_eq!(counter.value(), 0);
    }

    #[test]
    fn counter_increment() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "test counter");
        counter.increment();
        counter.increment();
        counter.increment();
        assert_eq!(counter.value(), 3);
    }

    #[test]
    fn counter_increment_by() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "test counter");
        counter.increment_by(5);
        counter.increment_by(10);
        assert_eq!(counter.value(), 15);
    }

    #[test]
    fn counter_increment_by_zero_is_idempotent() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "test counter");
        counter.increment_by(7);
        counter.increment_by(0);
        assert_eq!(counter.value(), 7);
    }

    #[test]
    fn counter_initial_value_in_snapshot() {
        let collector = MetricsCollector::new();
        let _counter = collector.register_counter("c", "desc");
        let text = collector.snapshot().to_prometheus_text();
        assert!(text.contains("c 0"));
    }

    // ── Gauge ──────────────────────────────────────────────────────────────

    #[test]
    fn gauge_starts_at_zero() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test gauge");
        assert_eq!(gauge.value(), 0.0);
    }

    #[test]
    fn gauge_set() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test gauge");
        gauge.set(42.5);
        assert!((gauge.value() - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_set_overwrites_previous() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test gauge");
        gauge.set(10.0);
        gauge.set(99.5);
        assert!((gauge.value() - 99.5).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_increment() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test gauge");
        gauge.set(10.0);
        gauge.increment();
        gauge.increment();
        assert!((gauge.value() - 12.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_decrement() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test gauge");
        gauge.set(10.0);
        gauge.decrement();
        gauge.decrement();
        gauge.decrement();
        assert!((gauge.value() - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_negative_value() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test gauge");
        gauge.set(-5.5);
        assert!((gauge.value() - (-5.5)).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_increment_from_zero() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test gauge");
        gauge.increment();
        assert!((gauge.value() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_decrement_below_zero() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "test gauge");
        gauge.decrement();
        assert!((gauge.value() - (-1.0)).abs() < f64::EPSILON);
    }

    // ── Histogram ──────────────────────────────────────────────────────────

    #[test]
    fn histogram_observe_and_count() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test hist", &[1.0, 5.0, 10.0]);
        hist.observe(2.0);
        hist.observe(4.0);
        hist.observe(6.0);
        assert_eq!(hist.count(), 3);
    }

    #[test]
    fn histogram_sum() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test hist", &[10.0]);
        hist.observe(1.0);
        hist.observe(2.0);
        hist.observe(3.0);
        assert!((hist.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_quantile_median() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test hist", &[]);
        for i in 1..=100 {
            hist.observe(i as f64);
        }
        // sorted = [1.0..=100.0], idx = round(99 * 0.5) = 50, sorted[50] = 51.0
        let p50 = hist.quantile(0.5);
        assert!((p50 - 51.0).abs() < f64::EPSILON, "p50 was {p50}");
    }

    #[test]
    fn histogram_quantile_p99() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test hist", &[]);
        for i in 1..=100 {
            hist.observe(i as f64);
        }
        // idx = round(99 * 0.99) = round(98.01) = 98, sorted[98] = 99.0
        let p99 = hist.quantile(0.99);
        assert!((p99 - 99.0).abs() < f64::EPSILON, "p99 was {p99}");
    }

    #[test]
    fn histogram_quantile_p0_returns_min() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test hist", &[]);
        hist.observe(5.0);
        hist.observe(10.0);
        hist.observe(15.0);
        assert!((hist.quantile(0.0) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_quantile_p100_returns_max() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test hist", &[]);
        hist.observe(5.0);
        hist.observe(10.0);
        hist.observe(15.0);
        assert!((hist.quantile(1.0) - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_quantile_empty_returns_zero() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test hist", &[]);
        assert_eq!(hist.quantile(0.5), 0.0);
    }

    #[test]
    fn histogram_quantile_single_sample() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test hist", &[]);
        hist.observe(42.0);
        assert!((hist.quantile(0.0) - 42.0).abs() < f64::EPSILON);
        assert!((hist.quantile(0.5) - 42.0).abs() < f64::EPSILON);
        assert!((hist.quantile(1.0) - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_ignores_non_finite() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test hist", &[]);
        hist.observe(f64::NAN);
        hist.observe(f64::INFINITY);
        hist.observe(f64::NEG_INFINITY);
        hist.observe(5.0);
        assert_eq!(hist.count(), 1);
        assert!((hist.sum() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_quantile_ordering_invariant() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "test hist", &[]);
        for i in 1..=100 {
            hist.observe(i as f64);
        }
        let p50 = hist.quantile(0.50);
        let p95 = hist.quantile(0.95);
        let p99 = hist.quantile(0.99);
        assert!(p50 <= p95, "p50 ({p50}) must be <= p95 ({p95})");
        assert!(p95 <= p99, "p95 ({p95}) must be <= p99 ({p99})");
    }

    // ── Prometheus text format ─────────────────────────────────────────────

    #[test]
    fn prometheus_counter_format() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("requests_total", "Total requests");
        counter.increment_by(42);
        let text = collector.snapshot().to_prometheus_text();
        assert!(text.contains("# HELP requests_total Total requests"));
        assert!(text.contains("# TYPE requests_total counter"));
        assert!(text.contains("requests_total 42"));
    }

    #[test]
    fn prometheus_gauge_format() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("temperature", "Current temperature");
        gauge.set(36.6);
        let text = collector.snapshot().to_prometheus_text();
        assert!(text.contains("# HELP temperature Current temperature"));
        assert!(text.contains("# TYPE temperature gauge"));
        assert!(text.contains("temperature 36.6"));
    }

    #[test]
    fn prometheus_histogram_format() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("latency", "Request latency", &[1.0, 5.0, 10.0]);
        hist.observe(0.5);
        hist.observe(3.0);
        hist.observe(7.0);
        let text = collector.snapshot().to_prometheus_text();
        assert!(text.contains("# HELP latency Request latency"));
        assert!(text.contains("# TYPE latency histogram"));
        assert!(text.contains("latency_bucket{le=\"1.0\"} 1"));
        assert!(text.contains("latency_bucket{le=\"5.0\"} 2"));
        assert!(text.contains("latency_bucket{le=\"10.0\"} 3"));
        assert!(text.contains("latency_bucket{le=\"+Inf\"} 3"));
        assert!(text.contains("latency_sum 10.5"));
        assert!(text.contains("latency_count 3"));
    }

    #[test]
    fn prometheus_empty_snapshot() {
        let collector = MetricsCollector::new();
        let text = collector.snapshot().to_prometheus_text();
        assert!(text.is_empty());
    }

    #[test]
    fn prometheus_multiple_metrics() {
        let collector = MetricsCollector::new();
        let c = collector.register_counter("c", "counter");
        c.increment();
        let g = collector.register_gauge("g", "gauge");
        g.set(3.14);
        let text = collector.snapshot().to_prometheus_text();
        assert!(text.contains("# TYPE c counter"));
        assert!(text.contains("# TYPE g gauge"));
    }

    // ── JSON export ────────────────────────────────────────────────────────

    #[test]
    fn json_export_contains_all_metrics() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("events", "Events");
        counter.increment_by(10);
        let gauge = collector.register_gauge("mem", "Memory");
        gauge.set(1024.0);
        let hist = collector.register_histogram("lat", "Latency", &[1.0]);
        hist.observe(0.5);

        let json = collector.snapshot().to_json();
        assert!(json.contains("\"timestamp_ms\":"));
        assert!(json.contains("\"name\":\"events\""));
        assert!(json.contains("\"type\":\"counter\""));
        assert!(json.contains("\"value\":10"));
        assert!(json.contains("\"name\":\"mem\""));
        assert!(json.contains("\"type\":\"gauge\""));
        assert!(json.contains("\"name\":\"lat\""));
        assert!(json.contains("\"type\":\"histogram\""));
    }

    #[test]
    fn json_export_valid_structure() {
        let collector = MetricsCollector::new();
        collector.register_counter("c", "counter");
        let json = collector.snapshot().to_json();
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
        assert!(json.contains("\"metrics\":["));
    }

    #[test]
    fn json_export_empty_snapshot() {
        let collector = MetricsCollector::new();
        let json = collector.snapshot().to_json();
        assert!(json.contains("\"metrics\":[]"));
    }

    #[test]
    fn json_escapes_special_characters() {
        let collector = MetricsCollector::new();
        collector.register_counter("c", "desc with \"quotes\"");
        let json = collector.snapshot().to_json();
        assert!(json.contains("desc with \\\"quotes\\\""));
    }

    // ── Snapshot consistency ───────────────────────────────────────────────

    #[test]
    fn snapshot_has_timestamp() {
        let before = SystemTime::now();
        let collector = MetricsCollector::new();
        collector.register_counter("c", "c");
        let snap = collector.snapshot();
        let after = SystemTime::now();
        assert!(snap.timestamp >= before);
        assert!(snap.timestamp <= after);
    }

    #[test]
    fn snapshot_reflects_current_values() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "c");
        counter.increment_by(5);
        let snap1 = collector.snapshot();
        counter.increment_by(3);
        let snap2 = collector.snapshot();

        let text1 = snap1.to_prometheus_text();
        let text2 = snap2.to_prometheus_text();
        assert!(text1.contains("c 5"));
        assert!(text2.contains("c 8"));
    }

    #[test]
    fn snapshot_len_and_empty() {
        let collector = MetricsCollector::new();
        assert!(collector.snapshot().is_empty());
        assert_eq!(collector.snapshot().len(), 0);

        collector.register_counter("c1", "c1");
        collector.register_gauge("g1", "g1");
        collector.register_histogram("h1", "h1", &[]);
        let snap = collector.snapshot();
        assert_eq!(snap.len(), 3);
        assert!(!snap.is_empty());
    }

    // ── Multiple collectors ────────────────────────────────────────────────

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

        assert_eq!(c1.snapshot().len(), 1);
        assert_eq!(c2.snapshot().len(), 1);
    }

    #[test]
    fn multiple_collectors_independent_snapshots() {
        let c1 = MetricsCollector::new();
        let c2 = MetricsCollector::new();

        c1.register_counter("a", "a");
        c1.register_gauge("b", "b");
        c2.register_histogram("c", "c", &[1.0]);

        assert_eq!(c1.snapshot().len(), 2);
        assert_eq!(c2.snapshot().len(), 1);
    }

    // ── Cloned handles ─────────────────────────────────────────────────────

    #[test]
    fn cloned_counter_handles_share_state() {
        let collector = MetricsCollector::new();
        let counter = collector.register_counter("c", "c");
        let clone = counter.clone();
        counter.increment();
        clone.increment();
        assert_eq!(counter.value(), 2);
        assert_eq!(clone.value(), 2);
    }

    #[test]
    fn cloned_gauge_handles_share_state() {
        let collector = MetricsCollector::new();
        let gauge = collector.register_gauge("g", "g");
        let clone = gauge.clone();
        gauge.set(5.0);
        assert!((clone.value() - 5.0).abs() < f64::EPSILON);
        clone.increment();
        assert!((gauge.value() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cloned_histogram_handles_share_state() {
        let collector = MetricsCollector::new();
        let hist = collector.register_histogram("h", "h", &[]);
        let clone = hist.clone();
        hist.observe(1.0);
        clone.observe(2.0);
        assert_eq!(hist.count(), 2);
        assert_eq!(clone.count(), 2);
    }

    // ── Built-in flight metrics ────────────────────────────────────────────

    #[test]
    fn flight_metrics_register_all() {
        let collector = MetricsCollector::new();
        let fm = FlightMetrics::register(&collector);

        fm.bus_events_total.increment_by(100);
        fm.device_count.set(3.0);
        fm.axis_processing_latency_us.observe(150.0);
        fm.adapter_reconnections_total.increment();
        fm.profile_switches_total.increment();
        fm.ffb_force_magnitude.observe(0.8);
        fm.memory_usage_bytes.set(1_048_576.0);

        // 3 counters + 2 gauges + 2 histograms = 7
        let snap = collector.snapshot();
        assert_eq!(snap.len(), 7);

        let text = snap.to_prometheus_text();
        assert!(text.contains("bus_events_total 100"));
        assert!(text.contains("device_count 3.0"));
        assert!(text.contains("axis_processing_latency_us_count 1"));
        assert!(text.contains("adapter_reconnections_total 1"));
        assert!(text.contains("profile_switches_total 1"));
        assert!(text.contains("ffb_force_magnitude_count 1"));
        assert!(text.contains("memory_usage_bytes"));
    }

    #[test]
    fn flight_metrics_histogram_buckets() {
        let collector = MetricsCollector::new();
        let fm = FlightMetrics::register(&collector);

        fm.axis_processing_latency_us.observe(75.0);
        fm.axis_processing_latency_us.observe(300.0);

        let text = collector.snapshot().to_prometheus_text();
        // 75.0 fits in the 100.0 bucket
        assert!(text.contains("axis_processing_latency_us_bucket{le=\"100.0\"} 1"));
        // Both fit in the 500.0 bucket
        assert!(text.contains("axis_processing_latency_us_bucket{le=\"500.0\"} 2"));
    }

    // ── Default trait ──────────────────────────────────────────────────────

    #[test]
    fn collector_default_is_empty() {
        let collector = MetricsCollector::default();
        assert!(collector.snapshot().is_empty());
    }
}
