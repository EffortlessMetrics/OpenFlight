// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Atomic bus metrics for lock-free monitoring and diagnostics.
//!
//! [`BusMetrics`] uses atomic counters so it can be shared across threads
//! without locking. Call [`BusMetrics::snapshot`] to obtain a plain
//! [`BusMetricsSnapshot`] suitable for serialization or health assessment.

use std::sync::atomic::{AtomicU64, Ordering};

/// Lock-free bus metrics using atomic counters.
///
/// All recording methods use `Relaxed` ordering — they are statistical
/// counters where exact cross-thread synchronization is not required.
#[derive(Debug)]
pub struct BusMetrics {
    messages_published: AtomicU64,
    messages_delivered: AtomicU64,
    messages_dropped: AtomicU64,
    slow_subscribers: AtomicU64,
    peak_queue_depth: AtomicU64,
}

impl BusMetrics {
    /// Create a new zeroed metrics instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            messages_published: AtomicU64::new(0),
            messages_delivered: AtomicU64::new(0),
            messages_dropped: AtomicU64::new(0),
            slow_subscribers: AtomicU64::new(0),
            peak_queue_depth: AtomicU64::new(0),
        }
    }

    /// Record a message publication.
    pub fn record_publish(&self) {
        self.messages_published.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful message delivery.
    pub fn record_delivery(&self) {
        self.messages_delivered.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a dropped message.
    pub fn record_drop(&self) {
        self.messages_dropped.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a slow subscriber event.
    pub fn record_slow_subscriber(&self) {
        self.slow_subscribers.fetch_add(1, Ordering::Relaxed);
    }

    /// Update peak queue depth if `depth` exceeds the current peak.
    pub fn update_peak_queue_depth(&self, depth: u64) {
        self.peak_queue_depth.fetch_max(depth, Ordering::Relaxed);
    }

    /// Take a point-in-time snapshot of all counters.
    #[must_use]
    pub fn snapshot(&self) -> BusMetricsSnapshot {
        BusMetricsSnapshot {
            messages_published: self.messages_published.load(Ordering::Relaxed),
            messages_delivered: self.messages_delivered.load(Ordering::Relaxed),
            messages_dropped: self.messages_dropped.load(Ordering::Relaxed),
            slow_subscribers: self.slow_subscribers.load(Ordering::Relaxed),
            peak_queue_depth: self.peak_queue_depth.load(Ordering::Relaxed),
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&self) {
        self.messages_published.store(0, Ordering::Relaxed);
        self.messages_delivered.store(0, Ordering::Relaxed);
        self.messages_dropped.store(0, Ordering::Relaxed);
        self.slow_subscribers.store(0, Ordering::Relaxed);
        self.peak_queue_depth.store(0, Ordering::Relaxed);
    }
}

impl Default for BusMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Plain-data snapshot of [`BusMetrics`] at a point in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusMetricsSnapshot {
    pub messages_published: u64,
    pub messages_delivered: u64,
    pub messages_dropped: u64,
    pub slow_subscribers: u64,
    pub peak_queue_depth: u64,
}

impl BusMetricsSnapshot {
    /// Compute the drop rate as a percentage (0.0–100.0).
    ///
    /// Returns 0.0 when no messages have been published.
    #[must_use]
    pub fn drop_rate_percent(&self) -> f64 {
        if self.messages_published == 0 {
            return 0.0;
        }
        (self.messages_dropped as f64 / self.messages_published as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_metrics_are_zero() {
        let m = BusMetrics::new();
        let s = m.snapshot();
        assert_eq!(s.messages_published, 0);
        assert_eq!(s.messages_delivered, 0);
        assert_eq!(s.messages_dropped, 0);
        assert_eq!(s.slow_subscribers, 0);
        assert_eq!(s.peak_queue_depth, 0);
    }

    #[test]
    fn record_publish_increments() {
        let m = BusMetrics::new();
        m.record_publish();
        m.record_publish();
        assert_eq!(m.snapshot().messages_published, 2);
    }

    #[test]
    fn record_delivery_increments() {
        let m = BusMetrics::new();
        m.record_delivery();
        m.record_delivery();
        m.record_delivery();
        assert_eq!(m.snapshot().messages_delivered, 3);
    }

    #[test]
    fn record_drop_increments() {
        let m = BusMetrics::new();
        m.record_drop();
        assert_eq!(m.snapshot().messages_dropped, 1);
    }

    #[test]
    fn record_slow_subscriber_increments() {
        let m = BusMetrics::new();
        m.record_slow_subscriber();
        m.record_slow_subscriber();
        assert_eq!(m.snapshot().slow_subscribers, 2);
    }

    #[test]
    fn peak_queue_depth_tracks_maximum() {
        let m = BusMetrics::new();
        m.update_peak_queue_depth(10);
        m.update_peak_queue_depth(5);
        m.update_peak_queue_depth(20);
        m.update_peak_queue_depth(15);
        assert_eq!(m.snapshot().peak_queue_depth, 20);
    }

    #[test]
    fn reset_clears_all_counters() {
        let m = BusMetrics::new();
        m.record_publish();
        m.record_delivery();
        m.record_drop();
        m.record_slow_subscriber();
        m.update_peak_queue_depth(42);
        m.reset();
        let s = m.snapshot();
        assert_eq!(s.messages_published, 0);
        assert_eq!(s.messages_delivered, 0);
        assert_eq!(s.messages_dropped, 0);
        assert_eq!(s.slow_subscribers, 0);
        assert_eq!(s.peak_queue_depth, 0);
    }

    #[test]
    fn drop_rate_percent_zero_when_no_publishes() {
        let s = BusMetricsSnapshot {
            messages_published: 0,
            messages_delivered: 0,
            messages_dropped: 0,
            slow_subscribers: 0,
            peak_queue_depth: 0,
        };
        assert!((s.drop_rate_percent() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn drop_rate_percent_calculated_correctly() {
        let s = BusMetricsSnapshot {
            messages_published: 200,
            messages_delivered: 190,
            messages_dropped: 10,
            slow_subscribers: 0,
            peak_queue_depth: 0,
        };
        assert!((s.drop_rate_percent() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn default_trait_creates_zeroed_metrics() {
        let m = BusMetrics::default();
        assert_eq!(m.snapshot().messages_published, 0);
    }
}
