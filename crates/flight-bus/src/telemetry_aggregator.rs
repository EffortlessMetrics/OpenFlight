// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Telemetry aggregation layer for the event bus.
//!
//! Collects and summarizes bus metrics including message counts,
//! latency percentiles, throughput, and per-topic breakdowns.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Aggregated telemetry metrics from the event bus.
#[derive(Debug, Clone)]
pub struct BusTelemetry {
    pub messages_published: u64,
    pub messages_delivered: u64,
    pub messages_dropped: u64,
    pub avg_latency_us: f64,
    pub max_latency_us: u64,
    pub p99_latency_us: u64,
    pub throughput_msgs_per_sec: f64,
    pub active_publishers: usize,
    pub active_subscribers: usize,
    pub uptime: Duration,
}

/// Per-topic metrics.
#[derive(Debug, Clone, Default)]
pub struct TopicMetrics {
    pub topic: String,
    pub message_count: u64,
    pub byte_count: u64,
    pub last_message_at: Option<Instant>,
    pub subscriber_count: usize,
}

/// Collects and aggregates bus telemetry.
pub struct TelemetryAggregator {
    start_time: Instant,
    total_published: u64,
    total_delivered: u64,
    total_dropped: u64,
    latencies: Vec<u64>,
    latency_head: usize,
    latency_count: usize,
    latency_capacity: usize,
    topics: HashMap<String, TopicMetrics>,
    publishers: usize,
    subscribers: usize,
}

impl TelemetryAggregator {
    /// Create a new aggregator with a fixed-capacity ring buffer for latencies.
    pub fn new(latency_buffer_size: usize) -> Self {
        Self {
            start_time: Instant::now(),
            total_published: 0,
            total_delivered: 0,
            total_dropped: 0,
            latencies: vec![0; latency_buffer_size],
            latency_head: 0,
            latency_count: 0,
            latency_capacity: latency_buffer_size,
            topics: HashMap::new(),
            publishers: 0,
            subscribers: 0,
        }
    }

    /// Record a published message for the given topic.
    pub fn record_publish(&mut self, topic: &str, bytes: u64) {
        self.total_published += 1;
        let metrics = self
            .topics
            .entry(topic.to_owned())
            .or_insert_with(|| TopicMetrics {
                topic: topic.to_owned(),
                ..Default::default()
            });
        metrics.message_count += 1;
        metrics.byte_count += bytes;
        metrics.last_message_at = Some(Instant::now());
    }

    /// Record a successful delivery with measured latency in microseconds.
    pub fn record_delivery(&mut self, latency_us: u64) {
        self.total_delivered += 1;
        if self.latency_capacity > 0 {
            self.latencies[self.latency_head] = latency_us;
            self.latency_head = (self.latency_head + 1) % self.latency_capacity;
            if self.latency_count < self.latency_capacity {
                self.latency_count += 1;
            }
        }
    }

    /// Record a dropped message.
    pub fn record_drop(&mut self) {
        self.total_dropped += 1;
    }

    /// Set the current number of active publishers.
    pub fn set_publisher_count(&mut self, count: usize) {
        self.publishers = count;
    }

    /// Set the current number of active subscribers.
    pub fn set_subscriber_count(&mut self, count: usize) {
        self.subscribers = count;
    }

    /// Compute a point-in-time snapshot of aggregated metrics.
    pub fn snapshot(&self) -> BusTelemetry {
        let uptime = self.start_time.elapsed();
        let (avg_latency_us, max_latency_us, p99_latency_us) = self.compute_latency_stats();
        let throughput_msgs_per_sec = if uptime.as_secs_f64() > 0.0 {
            self.total_delivered as f64 / uptime.as_secs_f64()
        } else {
            0.0
        };

        BusTelemetry {
            messages_published: self.total_published,
            messages_delivered: self.total_delivered,
            messages_dropped: self.total_dropped,
            avg_latency_us,
            max_latency_us,
            p99_latency_us,
            throughput_msgs_per_sec,
            active_publishers: self.publishers,
            active_subscribers: self.subscribers,
            uptime,
        }
    }

    /// Look up metrics for a specific topic.
    pub fn topic_metrics(&self, topic: &str) -> Option<&TopicMetrics> {
        self.topics.get(topic)
    }

    /// Return all per-topic metrics.
    pub fn all_topics(&self) -> &HashMap<String, TopicMetrics> {
        &self.topics
    }

    /// Reset all counters and buffers to initial state.
    pub fn reset(&mut self) {
        self.start_time = Instant::now();
        self.total_published = 0;
        self.total_delivered = 0;
        self.total_dropped = 0;
        self.latencies.fill(0);
        self.latency_head = 0;
        self.latency_count = 0;
        self.topics.clear();
        self.publishers = 0;
        self.subscribers = 0;
    }

    fn compute_latency_stats(&self) -> (f64, u64, u64) {
        if self.latency_count == 0 {
            return (0.0, 0, 0);
        }

        let mut sorted: Vec<u64> = self.latencies[..self.latency_count].to_vec();
        sorted.sort_unstable();

        let sum: u64 = sorted.iter().sum();
        let avg = sum as f64 / self.latency_count as f64;
        let max = sorted[self.latency_count - 1];

        // p99: index at ceil(0.99 * count) - 1, clamped
        let p99_idx = ((self.latency_count as f64 * 0.99).ceil() as usize).saturating_sub(1);
        let p99_idx = p99_idx.min(self.latency_count - 1);
        let p99 = sorted[p99_idx];

        (avg, max, p99)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn initial_snapshot_shows_zeros() {
        let agg = TelemetryAggregator::new(128);
        let snap = agg.snapshot();
        assert_eq!(snap.messages_published, 0);
        assert_eq!(snap.messages_delivered, 0);
        assert_eq!(snap.messages_dropped, 0);
        assert_eq!(snap.avg_latency_us, 0.0);
        assert_eq!(snap.max_latency_us, 0);
        assert_eq!(snap.p99_latency_us, 0);
        assert_eq!(snap.active_publishers, 0);
        assert_eq!(snap.active_subscribers, 0);
    }

    #[test]
    fn record_publish_increments_count() {
        let mut agg = TelemetryAggregator::new(128);
        agg.record_publish("altitude", 64);
        agg.record_publish("heading", 32);
        assert_eq!(agg.snapshot().messages_published, 2);
    }

    #[test]
    fn record_delivery_tracks_latency() {
        let mut agg = TelemetryAggregator::new(128);
        agg.record_delivery(100);
        agg.record_delivery(200);
        let snap = agg.snapshot();
        assert_eq!(snap.messages_delivered, 2);
        assert!(snap.avg_latency_us > 0.0);
    }

    #[test]
    fn record_drop_increments_counter() {
        let mut agg = TelemetryAggregator::new(128);
        agg.record_drop();
        agg.record_drop();
        agg.record_drop();
        assert_eq!(agg.snapshot().messages_dropped, 3);
    }

    #[test]
    fn avg_latency_calculated_correctly() {
        let mut agg = TelemetryAggregator::new(128);
        agg.record_delivery(100);
        agg.record_delivery(200);
        agg.record_delivery(300);
        let snap = agg.snapshot();
        assert!((snap.avg_latency_us - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn max_latency_tracked() {
        let mut agg = TelemetryAggregator::new(128);
        agg.record_delivery(50);
        agg.record_delivery(999);
        agg.record_delivery(100);
        assert_eq!(agg.snapshot().max_latency_us, 999);
    }

    #[test]
    fn p99_latency_correct() {
        let mut agg = TelemetryAggregator::new(256);
        for i in 1..=100 {
            agg.record_delivery(i);
        }
        let snap = agg.snapshot();
        // p99 of 1..=100: ceil(0.99 * 100) - 1 = 98 → sorted[98] = 99
        assert_eq!(snap.p99_latency_us, 99);
    }

    #[test]
    fn throughput_calculation() {
        let mut agg = TelemetryAggregator::new(128);
        for _ in 0..100 {
            agg.record_delivery(10);
        }
        // Sleep briefly so uptime is non-zero
        thread::sleep(Duration::from_millis(50));
        let snap = agg.snapshot();
        assert!(snap.throughput_msgs_per_sec > 0.0);
    }

    #[test]
    fn topic_metrics_tracked_per_topic() {
        let mut agg = TelemetryAggregator::new(128);
        agg.record_publish("altitude", 64);
        agg.record_publish("altitude", 64);
        agg.record_publish("heading", 32);

        let alt = agg.topic_metrics("altitude").unwrap();
        assert_eq!(alt.message_count, 2);
        assert_eq!(alt.byte_count, 128);

        let hdg = agg.topic_metrics("heading").unwrap();
        assert_eq!(hdg.message_count, 1);
        assert_eq!(hdg.byte_count, 32);

        assert!(agg.topic_metrics("nonexistent").is_none());
    }

    #[test]
    fn reset_clears_everything() {
        let mut agg = TelemetryAggregator::new(128);
        agg.record_publish("altitude", 64);
        agg.record_delivery(100);
        agg.record_drop();
        agg.set_publisher_count(3);
        agg.set_subscriber_count(5);

        agg.reset();

        let snap = agg.snapshot();
        assert_eq!(snap.messages_published, 0);
        assert_eq!(snap.messages_delivered, 0);
        assert_eq!(snap.messages_dropped, 0);
        assert_eq!(snap.active_publishers, 0);
        assert_eq!(snap.active_subscribers, 0);
        assert!(agg.all_topics().is_empty());
    }

    #[test]
    fn multiple_topics_tracked_independently() {
        let mut agg = TelemetryAggregator::new(128);
        agg.record_publish("altitude", 64);
        agg.record_publish("speed", 32);
        agg.record_publish("heading", 16);
        agg.record_publish("altitude", 64);

        assert_eq!(agg.all_topics().len(), 3);
        assert_eq!(agg.topic_metrics("altitude").unwrap().message_count, 2);
        assert_eq!(agg.topic_metrics("speed").unwrap().message_count, 1);
        assert_eq!(agg.topic_metrics("heading").unwrap().message_count, 1);
    }

    #[test]
    fn publisher_subscriber_counts() {
        let mut agg = TelemetryAggregator::new(128);
        agg.set_publisher_count(4);
        agg.set_subscriber_count(8);
        let snap = agg.snapshot();
        assert_eq!(snap.active_publishers, 4);
        assert_eq!(snap.active_subscribers, 8);
    }

    #[test]
    fn ring_buffer_wraps_correctly() {
        let mut agg = TelemetryAggregator::new(4);
        // Fill buffer and overflow
        for i in 1..=6 {
            agg.record_delivery(i * 100);
        }
        // Buffer should contain: [500, 600, 300, 400] (head wrapped)
        // Count capped at capacity
        let snap = agg.snapshot();
        assert_eq!(snap.messages_delivered, 6);
        // Max should be 600 (from the 4 most recent: 300, 400, 500, 600)
        assert_eq!(snap.max_latency_us, 600);
    }
}
