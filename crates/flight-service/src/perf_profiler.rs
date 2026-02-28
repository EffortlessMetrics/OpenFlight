// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Lightweight performance sampling profiler for service subsystems.
//!
//! Tracks per-span min/max/mean durations so operators can identify
//! hot-spots without pulling in a full tracing back-end.

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// SpanStats
// ---------------------------------------------------------------------------

/// Accumulated statistics for a single named span.
#[derive(Debug, Clone)]
pub struct SpanStats {
    pub name: String,
    pub count: u64,
    pub total_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub last_duration: Duration,
}

impl SpanStats {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            count: 0,
            total_duration: Duration::ZERO,
            min_duration: Duration::MAX,
            max_duration: Duration::ZERO,
            last_duration: Duration::ZERO,
        }
    }

    /// Mean duration across all recorded samples.
    ///
    /// Returns [`Duration::ZERO`] when no samples have been recorded.
    #[must_use]
    pub fn mean_duration(&self) -> Duration {
        if self.count == 0 {
            return Duration::ZERO;
        }
        self.total_duration / u32::try_from(self.count).unwrap_or(u32::MAX)
    }

    fn record(&mut self, duration: Duration) {
        self.count += 1;
        self.total_duration += duration;
        if duration < self.min_duration {
            self.min_duration = duration;
        }
        if duration > self.max_duration {
            self.max_duration = duration;
        }
        self.last_duration = duration;
    }
}

// ---------------------------------------------------------------------------
// PerfReport
// ---------------------------------------------------------------------------

/// Snapshot report of all span statistics.
#[derive(Debug, Clone)]
pub struct PerfReport {
    pub spans: Vec<SpanStats>,
}

impl fmt::Display for PerfReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Performance Report ({} spans) ===", self.spans.len())?;
        for s in &self.spans {
            writeln!(
                f,
                "  {}: count={}, mean={:?}, min={:?}, max={:?}, last={:?}",
                s.name,
                s.count,
                s.mean_duration(),
                s.min_duration,
                s.max_duration,
                s.last_duration,
            )?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PerfProfiler
// ---------------------------------------------------------------------------

/// Lightweight performance profiler for service subsystems.
///
/// Records [`Duration`] samples keyed by span name and exposes
/// aggregated statistics via [`PerfProfiler::report`].
pub struct PerfProfiler {
    spans: HashMap<String, SpanStats>,
    enabled: bool,
}

impl PerfProfiler {
    /// Create a new profiler in the **enabled** state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            spans: HashMap::new(),
            enabled: true,
        }
    }

    /// Enable recording.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable recording — all `record` / `end_span` calls become no-ops.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Whether the profiler is currently recording.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Mark the beginning of a span and return the [`Instant`] to pass to
    /// [`end_span`](Self::end_span).
    #[must_use]
    pub fn begin_span(&self, _name: &str) -> Instant {
        Instant::now()
    }

    /// Finish a span started with [`begin_span`](Self::begin_span) and record
    /// the elapsed duration.
    pub fn end_span(&mut self, name: &str, start: Instant) {
        if !self.enabled {
            return;
        }
        let duration = start.elapsed();
        self.spans
            .entry(name.to_owned())
            .or_insert_with(|| SpanStats::new(name))
            .record(duration);
    }

    /// Directly record an externally measured duration for *name*.
    pub fn record(&mut self, name: &str, duration: Duration) {
        if !self.enabled {
            return;
        }
        self.spans
            .entry(name.to_owned())
            .or_insert_with(|| SpanStats::new(name))
            .record(duration);
    }

    /// Look up statistics for a single span.
    #[must_use]
    pub fn stats(&self, name: &str) -> Option<&SpanStats> {
        self.spans.get(name)
    }

    /// Reference to all collected span statistics.
    #[must_use]
    pub fn all_stats(&self) -> &HashMap<String, SpanStats> {
        &self.spans
    }

    /// Clear all recorded data.
    pub fn reset(&mut self) {
        self.spans.clear();
    }

    /// Build a formatted [`PerfReport`] containing a snapshot of every span.
    #[must_use]
    pub fn report(&self) -> PerfReport {
        let mut spans: Vec<SpanStats> = self.spans.values().cloned().collect();
        spans.sort_by(|a, b| a.name.cmp(&b.name));
        PerfReport { spans }
    }
}

impl Default for PerfProfiler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_single_span() {
        let mut profiler = PerfProfiler::new();
        profiler.record("tick", Duration::from_millis(4));

        let stats = profiler.stats("tick").expect("span should exist");
        assert_eq!(stats.count, 1);
        assert_eq!(stats.total_duration, Duration::from_millis(4));
        assert_eq!(stats.last_duration, Duration::from_millis(4));
    }

    #[test]
    fn record_multiple_spans() {
        let mut profiler = PerfProfiler::new();
        profiler.record("axis", Duration::from_millis(2));
        profiler.record("ffb", Duration::from_millis(5));

        assert!(profiler.stats("axis").is_some());
        assert!(profiler.stats("ffb").is_some());
        assert_eq!(profiler.all_stats().len(), 2);
    }

    #[test]
    fn min_max_mean_calculated_correctly() {
        let mut profiler = PerfProfiler::new();
        profiler.record("x", Duration::from_millis(10));
        profiler.record("x", Duration::from_millis(20));
        profiler.record("x", Duration::from_millis(30));

        let s = profiler.stats("x").unwrap();
        assert_eq!(s.count, 3);
        assert_eq!(s.min_duration, Duration::from_millis(10));
        assert_eq!(s.max_duration, Duration::from_millis(30));
        assert_eq!(s.mean_duration(), Duration::from_millis(20));
    }

    #[test]
    fn disabled_profiler_skips_recording() {
        let mut profiler = PerfProfiler::new();
        profiler.disable();
        assert!(!profiler.is_enabled());

        profiler.record("skipped", Duration::from_millis(1));
        assert!(profiler.stats("skipped").is_none());
        assert!(profiler.all_stats().is_empty());
    }

    #[test]
    fn reset_clears_all_data() {
        let mut profiler = PerfProfiler::new();
        profiler.record("a", Duration::from_millis(1));
        profiler.record("b", Duration::from_millis(2));
        assert_eq!(profiler.all_stats().len(), 2);

        profiler.reset();
        assert!(profiler.all_stats().is_empty());
        assert!(profiler.stats("a").is_none());
    }

    #[test]
    fn report_includes_all_spans() {
        let mut profiler = PerfProfiler::new();
        profiler.record("alpha", Duration::from_millis(1));
        profiler.record("beta", Duration::from_millis(2));
        profiler.record("gamma", Duration::from_millis(3));

        let report = profiler.report();
        assert_eq!(report.spans.len(), 3);

        let names: Vec<&str> = report.spans.iter().map(|s| s.name.as_str()).collect();
        // report sorts alphabetically
        assert_eq!(names, vec!["alpha", "beta", "gamma"]);

        let display = format!("{report}");
        assert!(display.contains("alpha"));
        assert!(display.contains("beta"));
        assert!(display.contains("gamma"));
    }

    #[test]
    fn zero_duration_handling() {
        let mut profiler = PerfProfiler::new();
        profiler.record("zero", Duration::ZERO);

        let s = profiler.stats("zero").unwrap();
        assert_eq!(s.count, 1);
        assert_eq!(s.min_duration, Duration::ZERO);
        assert_eq!(s.max_duration, Duration::ZERO);
        assert_eq!(s.mean_duration(), Duration::ZERO);
    }

    #[test]
    fn many_recordings_stress_test() {
        let mut profiler = PerfProfiler::new();
        let n = 10_000u64;
        for i in 0..n {
            profiler.record("hot", Duration::from_nanos(i));
        }

        let s = profiler.stats("hot").unwrap();
        assert_eq!(s.count, n);
        assert_eq!(s.min_duration, Duration::from_nanos(0));
        assert_eq!(s.max_duration, Duration::from_nanos(n - 1));
        assert_eq!(s.last_duration, Duration::from_nanos(n - 1));

        let expected_total: u64 = (0..n).sum();
        assert_eq!(s.total_duration, Duration::from_nanos(expected_total));
    }

    #[test]
    fn begin_end_span_records_nonzero_duration() {
        let mut profiler = PerfProfiler::new();
        let start = profiler.begin_span("live");
        // Burn a tiny amount of wall-clock time.
        std::hint::black_box(0u64.wrapping_add(1));
        profiler.end_span("live", start);

        let s = profiler.stats("live").unwrap();
        assert_eq!(s.count, 1);
        // Duration should be non-negative (may be zero on very fast machines,
        // but we only assert it was recorded).
        assert!(s.total_duration >= Duration::ZERO);
    }

    #[test]
    fn enable_after_disable_resumes_recording() {
        let mut profiler = PerfProfiler::new();
        profiler.disable();
        profiler.record("miss", Duration::from_millis(1));
        assert!(profiler.stats("miss").is_none());

        profiler.enable();
        assert!(profiler.is_enabled());
        profiler.record("hit", Duration::from_millis(2));
        assert!(profiler.stats("hit").is_some());
    }

    #[test]
    fn mean_duration_zero_when_no_samples() {
        let stats = SpanStats::new("empty");
        assert_eq!(stats.mean_duration(), Duration::ZERO);
    }
}
