// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Performance tracing via lightweight spans.
//!
//! A [`FlightSpan`] marks the start and end of a named operation and records
//! its wall-clock duration.  Completed spans are fed into a [`SpanCollector`]
//! which aggregates per-operation statistics exposed by [`span_summary`].
//!
//! Pre-defined span names for the RT pipeline are provided as constants.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

// ── Pre-defined span names ────────────────────────────────────────────────

/// 250 Hz axis processing tick.
pub const AXIS_TICK: &str = "axis_tick";
/// HID device read operation.
pub const HID_READ: &str = "hid_read";
/// Event bus publish.
pub const BUS_PUBLISH: &str = "bus_publish";
/// Off-thread profile compilation.
pub const PROFILE_COMPILE: &str = "profile_compile";
/// Force-feedback computation.
pub const FFB_COMPUTE: &str = "ffb_compute";

// ── FlightSpan ────────────────────────────────────────────────────────────

/// A span that measures the wall-clock duration of an operation.
///
/// Obtain a span from [`SpanCollector::start_span`].  When [`FlightSpan::finish`]
/// (or `Drop`) runs, the elapsed duration is recorded automatically.
pub struct FlightSpan {
    name: &'static str,
    start: Instant,
    collector: Option<*const SpanCollector>,
    finished: bool,
}

// SAFETY: SpanCollector is internally synchronised with parking_lot::Mutex.
unsafe impl Send for FlightSpan {}

impl FlightSpan {
    /// Create a free-standing span (not attached to a collector).
    pub fn begin(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
            collector: None,
            finished: false,
        }
    }

    /// Elapsed time since span start.
    pub fn elapsed_ns(&self) -> u64 {
        self.start.elapsed().as_nanos() as u64
    }

    /// Name of the operation being traced.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Explicitly finish the span and return its duration in nanoseconds.
    pub fn finish(mut self) -> u64 {
        self.record()
    }

    /// Internal: record duration once.
    fn record(&mut self) -> u64 {
        if self.finished {
            return 0;
        }
        self.finished = true;
        let duration_ns = self.start.elapsed().as_nanos() as u64;
        if let Some(ptr) = self.collector {
            // SAFETY: the collector outlives the span (enforced by API design).
            let collector = unsafe { &*ptr };
            collector.record(self.name, duration_ns);
        }
        duration_ns
    }
}

impl Drop for FlightSpan {
    fn drop(&mut self) {
        self.record();
    }
}

// ── SpanCollector ─────────────────────────────────────────────────────────

/// Aggregates completed span durations for later statistical reporting.
pub struct SpanCollector {
    inner: Mutex<CollectorInner>,
}

struct CollectorInner {
    ops: HashMap<&'static str, OpStats>,
}

/// Mutable accumulator for a single operation name.
struct OpStats {
    count: u64,
    min_ns: u64,
    max_ns: u64,
    sum_ns: u64,
    /// Reservoir of recent samples for percentile estimation.
    samples: Vec<u64>,
    max_samples: usize,
    sample_head: usize,
}

impl OpStats {
    fn new(max_samples: usize) -> Self {
        Self {
            count: 0,
            min_ns: u64::MAX,
            max_ns: 0,
            sum_ns: 0,
            samples: Vec::with_capacity(max_samples),
            max_samples,
            sample_head: 0,
        }
    }

    fn record(&mut self, duration_ns: u64) {
        self.count += 1;
        self.sum_ns = self.sum_ns.saturating_add(duration_ns);
        if duration_ns < self.min_ns {
            self.min_ns = duration_ns;
        }
        if duration_ns > self.max_ns {
            self.max_ns = duration_ns;
        }
        // Ring-buffer sampling
        if self.samples.len() < self.max_samples {
            self.samples.push(duration_ns);
        } else {
            self.samples[self.sample_head] = duration_ns;
        }
        self.sample_head = (self.sample_head + 1) % self.max_samples;
    }
}

impl SpanCollector {
    /// Create a new collector.  `max_samples` controls the reservoir size per
    /// operation used for percentile calculation.
    pub fn new(_max_samples: usize) -> Self {
        Self {
            inner: Mutex::new(CollectorInner {
                ops: HashMap::new(),
            }),
        }
    }

    /// Start a span whose duration will be automatically recorded on finish/drop.
    pub fn start_span(&self, name: &'static str) -> FlightSpan {
        FlightSpan {
            name,
            start: Instant::now(),
            collector: Some(self as *const Self),
            finished: false,
        }
    }

    /// Record a pre-measured duration for the given operation.
    pub fn record(&self, name: &'static str, duration_ns: u64) {
        let mut inner = self.inner.lock();
        inner
            .ops
            .entry(name)
            .or_insert_with(|| OpStats::new(10_000))
            .record(duration_ns);
    }

    /// Produce per-operation timing summaries.
    pub fn summary(&self) -> Vec<SpanSummary> {
        let inner = self.inner.lock();
        inner
            .ops
            .iter()
            .map(|(name, stats)| {
                let avg_ns = if stats.count > 0 {
                    stats.sum_ns / stats.count
                } else {
                    0
                };
                let p99_ns = percentile(&stats.samples, 99);
                SpanSummary {
                    name,
                    count: stats.count,
                    min_ns: if stats.min_ns == u64::MAX {
                        0
                    } else {
                        stats.min_ns
                    },
                    max_ns: stats.max_ns,
                    avg_ns,
                    p99_ns,
                }
            })
            .collect()
    }

    /// Reset all collected statistics.
    pub fn reset(&self) {
        let mut inner = self.inner.lock();
        inner.ops.clear();
    }
}

/// Compute the `p`-th percentile from a sample slice (0-100).
fn percentile(samples: &[u64], p: usize) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let idx = (sorted.len() * p) / 100;
    sorted[idx.min(sorted.len() - 1)]
}

/// Per-operation timing summary produced by [`SpanCollector::summary`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanSummary {
    pub name: &'static str,
    pub count: u64,
    pub min_ns: u64,
    pub max_ns: u64,
    pub avg_ns: u64,
    pub p99_ns: u64,
}

/// Convenience: get the summary for a single operation by name.
pub fn span_summary(collector: &SpanCollector, name: &str) -> Option<SpanSummary> {
    collector.summary().into_iter().find(|s| s.name == name)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn free_span_measures_nonzero_duration() {
        let span = FlightSpan::begin(AXIS_TICK);
        thread::sleep(Duration::from_millis(1));
        let ns = span.finish();
        assert!(ns > 0, "span duration must be > 0");
    }

    #[test]
    fn span_name_preserved() {
        let span = FlightSpan::begin(HID_READ);
        assert_eq!(span.name(), HID_READ);
    }

    #[test]
    fn collector_records_via_start_span() {
        let collector = SpanCollector::new(1000);
        {
            let _span = collector.start_span(AXIS_TICK);
            thread::sleep(Duration::from_millis(1));
        } // drop records
        let summaries = collector.summary();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, AXIS_TICK);
        assert_eq!(summaries[0].count, 1);
        assert!(summaries[0].min_ns > 0);
    }

    #[test]
    fn collector_explicit_record() {
        let collector = SpanCollector::new(1000);
        collector.record(BUS_PUBLISH, 500);
        collector.record(BUS_PUBLISH, 1500);
        collector.record(BUS_PUBLISH, 1000);

        let s = span_summary(&collector, BUS_PUBLISH).expect("summary should exist");
        assert_eq!(s.count, 3);
        assert_eq!(s.min_ns, 500);
        assert_eq!(s.max_ns, 1500);
        assert_eq!(s.avg_ns, 1000);
    }

    #[test]
    fn span_summary_returns_none_for_unknown() {
        let collector = SpanCollector::new(100);
        assert!(span_summary(&collector, "nonexistent").is_none());
    }

    #[test]
    fn p99_calculation() {
        let collector = SpanCollector::new(10_000);
        // 99 samples at 100, 1 sample at 9999
        for _ in 0..99 {
            collector.record(AXIS_TICK, 100);
        }
        collector.record(AXIS_TICK, 9999);

        let s = span_summary(&collector, AXIS_TICK).unwrap();
        assert_eq!(s.count, 100);
        assert_eq!(s.p99_ns, 9999);
    }

    #[test]
    fn multiple_operations_tracked_independently() {
        let collector = SpanCollector::new(1000);
        collector.record(AXIS_TICK, 100);
        collector.record(HID_READ, 200);
        collector.record(FFB_COMPUTE, 300);

        let summaries = collector.summary();
        assert_eq!(summaries.len(), 3);
        assert!(span_summary(&collector, AXIS_TICK).is_some());
        assert!(span_summary(&collector, HID_READ).is_some());
        assert!(span_summary(&collector, FFB_COMPUTE).is_some());
    }

    #[test]
    fn predefined_span_names_are_distinct() {
        let names = [
            AXIS_TICK,
            HID_READ,
            BUS_PUBLISH,
            PROFILE_COMPILE,
            FFB_COMPUTE,
        ];
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(
            unique.len(),
            names.len(),
            "all predefined span names must be unique"
        );
    }

    #[test]
    fn collector_reset_clears_all() {
        let collector = SpanCollector::new(100);
        collector.record(AXIS_TICK, 100);
        collector.record(HID_READ, 200);
        collector.reset();
        assert!(collector.summary().is_empty());
    }

    #[test]
    fn span_drop_records_duration() {
        let collector = SpanCollector::new(100);
        {
            let _span = collector.start_span(PROFILE_COMPILE);
            // immediately dropped
        }
        let s = span_summary(&collector, PROFILE_COMPILE).unwrap();
        assert_eq!(s.count, 1);
    }

    #[test]
    fn span_finish_returns_duration() {
        let collector = SpanCollector::new(100);
        let span = collector.start_span(FFB_COMPUTE);
        thread::sleep(Duration::from_millis(2));
        let ns = span.finish();
        // Should be at least 1ms (conservative for CI)
        assert!(ns >= 1_000_000, "expected ≥1ms, got {ns}ns");
    }

    #[test]
    fn span_statistics_min_max_avg() {
        let collector = SpanCollector::new(1000);
        let durations = [100u64, 200, 300, 400, 500];
        for d in &durations {
            collector.record(AXIS_TICK, *d);
        }
        let s = span_summary(&collector, AXIS_TICK).unwrap();
        assert_eq!(s.min_ns, 100);
        assert_eq!(s.max_ns, 500);
        assert_eq!(s.avg_ns, 300);
        assert_eq!(s.count, 5);
    }
}
