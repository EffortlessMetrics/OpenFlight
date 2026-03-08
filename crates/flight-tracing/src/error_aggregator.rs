// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Error frequency tracking with flood suppression.
//!
//! [`ErrorAggregator`] records error occurrences by error code and severity,
//! tracks per-code rates over sliding windows, identifies the most frequent
//! errors, and provides a flood-suppression check to avoid log storms.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// ── Severity ──────────────────────────────────────────────────────────────

/// Error severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => f.write_str("INFO"),
            Self::Warning => f.write_str("WARN"),
            Self::Error => f.write_str("ERROR"),
            Self::Critical => f.write_str("CRITICAL"),
        }
    }
}

// ── SuppressionConfig ─────────────────────────────────────────────────────

/// Configuration for flood suppression.
#[derive(Debug, Clone)]
pub struct SuppressionConfig {
    /// Window over which rate is measured.
    pub window: Duration,
    /// Maximum errors per second before suppression kicks in.
    pub max_rate: f64,
}

impl Default for SuppressionConfig {
    fn default() -> Self {
        Self {
            window: Duration::from_secs(10),
            max_rate: 100.0,
        }
    }
}

// ── Internal tracking ─────────────────────────────────────────────────────

/// Per-error-code accumulator.
struct ErrorBucket {
    total: u64,
    #[allow(dead_code)]
    severity: Severity,
    /// Timestamps of recent occurrences (ring buffer).
    timestamps: Vec<Instant>,
    /// Write cursor.
    cursor: usize,
    capacity: usize,
}

impl ErrorBucket {
    fn new(severity: Severity, capacity: usize) -> Self {
        Self {
            total: 0,
            severity,
            timestamps: Vec::with_capacity(capacity),
            cursor: 0,
            capacity,
        }
    }

    fn record(&mut self, now: Instant) {
        self.total += 1;
        if self.timestamps.len() < self.capacity {
            self.timestamps.push(now);
        } else {
            self.timestamps[self.cursor] = now;
        }
        self.cursor = (self.cursor + 1) % self.capacity;
    }

    /// Count occurrences within the given window ending at `now`.
    fn count_in_window(&self, now: Instant, window: Duration) -> u64 {
        let cutoff = now - window;
        self.timestamps.iter().filter(|&&t| t >= cutoff).count() as u64
    }

    /// Errors per second within the window.
    fn rate(&self, now: Instant, window: Duration) -> f64 {
        let count = self.count_in_window(now, window);
        let secs = window.as_secs_f64();
        if secs > 0.0 { count as f64 / secs } else { 0.0 }
    }
}

// ── ErrorAggregator ───────────────────────────────────────────────────────

/// Thread-safe error frequency tracker with flood suppression.
pub struct ErrorAggregator {
    inner: Mutex<AggregatorInner>,
    suppression: SuppressionConfig,
}

struct AggregatorInner {
    buckets: HashMap<u32, ErrorBucket>,
    /// Maximum timestamps stored per error code.
    bucket_capacity: usize,
}

impl ErrorAggregator {
    /// Create an aggregator with default suppression settings.
    pub fn new() -> Self {
        Self::with_config(SuppressionConfig::default(), 1024)
    }

    /// Create an aggregator with custom suppression config and per-code
    /// ring-buffer capacity.
    pub fn with_config(suppression: SuppressionConfig, bucket_capacity: usize) -> Self {
        assert!(bucket_capacity > 0, "bucket_capacity must be > 0");
        Self {
            inner: Mutex::new(AggregatorInner {
                buckets: HashMap::new(),
                bucket_capacity,
            }),
            suppression,
        }
    }

    /// Record an error occurrence.
    pub fn record_error(&self, error_code: u32, severity: Severity) {
        let now = Instant::now();
        let mut inner = self.inner.lock();
        let cap = inner.bucket_capacity;
        inner
            .buckets
            .entry(error_code)
            .or_insert_with(|| ErrorBucket::new(severity, cap))
            .record(now);
    }

    /// Compute the error rate (errors / second) for a given code over `window`.
    pub fn error_rate(&self, error_code: u32, window: Duration) -> f64 {
        let now = Instant::now();
        let inner = self.inner.lock();
        inner
            .buckets
            .get(&error_code)
            .map_or(0.0, |b| b.rate(now, window))
    }

    /// Return the `n` most frequent error codes by total count (descending).
    pub fn top_errors(&self, n: usize) -> Vec<(u32, u64)> {
        let inner = self.inner.lock();
        let mut entries: Vec<(u32, u64)> =
            inner.buckets.iter().map(|(&k, v)| (k, v.total)).collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(n);
        entries
    }

    /// Returns `true` if the error is firing faster than the suppression
    /// threshold, indicating a flood.
    pub fn suppression_check(&self, error_code: u32) -> bool {
        let now = Instant::now();
        let inner = self.inner.lock();
        inner
            .buckets
            .get(&error_code)
            .is_some_and(|b| b.rate(now, self.suppression.window) > self.suppression.max_rate)
    }

    /// Total count for a specific error code.
    pub fn total_count(&self, error_code: u32) -> u64 {
        let inner = self.inner.lock();
        inner.buckets.get(&error_code).map_or(0, |b| b.total)
    }

    /// Remove all tracked errors.
    pub fn clear(&self) {
        self.inner.lock().buckets.clear();
    }
}

impl Default for ErrorAggregator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_count() {
        let agg = ErrorAggregator::new();
        agg.record_error(1, Severity::Error);
        agg.record_error(1, Severity::Error);
        agg.record_error(2, Severity::Warning);

        assert_eq!(agg.total_count(1), 2);
        assert_eq!(agg.total_count(2), 1);
        assert_eq!(agg.total_count(99), 0);
    }

    #[test]
    fn error_rate_within_window() {
        let agg = ErrorAggregator::new();
        for _ in 0..50 {
            agg.record_error(1, Severity::Error);
        }
        // All 50 happened just now; rate over 10s = 50/10 = 5.0
        let rate = agg.error_rate(1, Duration::from_secs(10));
        assert!((rate - 5.0).abs() < 0.5, "expected ~5.0, got {rate}");
    }

    #[test]
    fn error_rate_unknown_code() {
        let agg = ErrorAggregator::new();
        assert_eq!(agg.error_rate(42, Duration::from_secs(1)), 0.0);
    }

    #[test]
    fn top_errors_ordering() {
        let agg = ErrorAggregator::new();
        for _ in 0..10 {
            agg.record_error(1, Severity::Error);
        }
        for _ in 0..30 {
            agg.record_error(2, Severity::Warning);
        }
        for _ in 0..20 {
            agg.record_error(3, Severity::Critical);
        }

        let top = agg.top_errors(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0], (2, 30)); // most frequent
        assert_eq!(top[1], (3, 20)); // second
    }

    #[test]
    fn top_errors_fewer_than_n() {
        let agg = ErrorAggregator::new();
        agg.record_error(1, Severity::Info);
        let top = agg.top_errors(10);
        assert_eq!(top.len(), 1);
    }

    #[test]
    fn suppression_below_threshold() {
        let config = SuppressionConfig {
            window: Duration::from_secs(10),
            max_rate: 100.0,
        };
        let agg = ErrorAggregator::with_config(config, 1024);
        // 5 errors in 10s → 0.5/s → no suppression
        for _ in 0..5 {
            agg.record_error(1, Severity::Error);
        }
        assert!(!agg.suppression_check(1));
    }

    #[test]
    fn suppression_above_threshold() {
        let config = SuppressionConfig {
            window: Duration::from_secs(1),
            max_rate: 10.0,
        };
        let agg = ErrorAggregator::with_config(config, 2048);
        // 50 errors → 50/s over 1s window → suppressed
        for _ in 0..50 {
            agg.record_error(1, Severity::Error);
        }
        assert!(agg.suppression_check(1));
    }

    #[test]
    fn suppression_unknown_code_is_false() {
        let agg = ErrorAggregator::new();
        assert!(!agg.suppression_check(999));
    }

    #[test]
    fn clear_resets_all() {
        let agg = ErrorAggregator::new();
        agg.record_error(1, Severity::Error);
        agg.record_error(2, Severity::Warning);
        agg.clear();
        assert_eq!(agg.total_count(1), 0);
        assert_eq!(agg.total_count(2), 0);
        assert!(agg.top_errors(10).is_empty());
    }

    #[test]
    fn severity_display() {
        assert_eq!(Severity::Info.to_string(), "INFO");
        assert_eq!(Severity::Warning.to_string(), "WARN");
        assert_eq!(Severity::Error.to_string(), "ERROR");
        assert_eq!(Severity::Critical.to_string(), "CRITICAL");
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
        assert!(Severity::Error < Severity::Critical);
    }

    #[test]
    fn default_aggregator() {
        let agg = ErrorAggregator::default();
        agg.record_error(1, Severity::Info);
        assert_eq!(agg.total_count(1), 1);
    }

    #[test]
    #[should_panic(expected = "bucket_capacity must be > 0")]
    fn zero_bucket_capacity_panics() {
        ErrorAggregator::with_config(SuppressionConfig::default(), 0);
    }

    #[test]
    fn multiple_severities_same_code() {
        let agg = ErrorAggregator::new();
        agg.record_error(1, Severity::Warning);
        agg.record_error(1, Severity::Error);
        // Both count toward the same code bucket
        assert_eq!(agg.total_count(1), 2);
    }
}
