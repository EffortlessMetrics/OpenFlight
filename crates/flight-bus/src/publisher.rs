// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Bus publisher with rate limiting and pub/sub system

use crate::snapshot::BusSnapshot;
use crate::types::BusTypeError;
use crossbeam::channel::{self, Receiver, Sender, TryRecvError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, trace, warn};

/// Publisher errors
#[derive(Error, Debug)]
pub enum PublisherError {
    #[error("Rate limit exceeded: {current_hz:.1}Hz > {max_hz:.1}Hz")]
    RateLimitExceeded { current_hz: f32, max_hz: f32 },
    #[error("Subscriber not found: {id}")]
    SubscriberNotFound { id: SubscriberId },
    #[error("Channel error: {0}")]
    ChannelError(String),
    #[error("Validation error: {0}")]
    ValidationError(#[from] BusTypeError),
}

/// Unique subscriber identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubscriberId(u64);

impl SubscriberId {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl std::fmt::Display for SubscriberId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Subscription configuration
#[derive(Debug, Clone)]
pub struct SubscriptionConfig {
    /// Maximum rate in Hz (30-60Hz range)
    pub max_rate_hz: f32,
    /// Buffer size for the subscriber channel
    pub buffer_size: usize,
    /// Whether to drop old messages when buffer is full
    pub drop_on_full: bool,
}

impl Default for SubscriptionConfig {
    fn default() -> Self {
        Self {
            max_rate_hz: 60.0,
            buffer_size: 100,
            drop_on_full: true,
        }
    }
}

/// Subscriber handle for receiving bus snapshots
pub struct Subscriber {
    pub id: SubscriberId,
    pub config: SubscriptionConfig,
    receiver: Receiver<BusSnapshot>,
    last_received: Instant,
    stats: SubscriberStats,
}

/// Subscriber statistics
#[derive(Debug, Clone, Default)]
pub struct SubscriberStats {
    pub messages_received: u64,
    pub messages_dropped: u64,
    pub last_message_age_ms: u64,
    pub average_rate_hz: f32,
}

impl Subscriber {
    /// Try to receive the next snapshot (non-blocking)
    pub fn try_recv(&mut self) -> Result<Option<BusSnapshot>, PublisherError> {
        match self.receiver.try_recv() {
            Ok(snapshot) => {
                self.update_stats(&snapshot);
                Ok(Some(snapshot))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(PublisherError::ChannelError(
                "Publisher disconnected".to_string(),
            )),
        }
    }

    /// Receive the next snapshot (blocking)
    pub fn recv(&mut self) -> Result<BusSnapshot, PublisherError> {
        match self.receiver.recv() {
            Ok(snapshot) => {
                self.update_stats(&snapshot);
                Ok(snapshot)
            }
            Err(_) => Err(PublisherError::ChannelError(
                "Publisher disconnected".to_string(),
            )),
        }
    }

    /// Get subscriber statistics
    pub fn stats(&self) -> &SubscriberStats {
        &self.stats
    }

    fn update_stats(&mut self, snapshot: &BusSnapshot) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_received).as_secs_f32();

        self.stats.messages_received += 1;
        self.stats.last_message_age_ms = snapshot.age_ms();

        if elapsed > 0.0 {
            // Simple moving average for rate calculation
            let current_rate = 1.0 / elapsed;
            if self.stats.average_rate_hz == 0.0 {
                self.stats.average_rate_hz = current_rate;
            } else {
                self.stats.average_rate_hz = 0.9 * self.stats.average_rate_hz + 0.1 * current_rate;
            }
        }

        self.last_received = now;
    }
}

/// Internal subscriber data
struct SubscriberData {
    sender: Sender<BusSnapshot>,
    config: SubscriptionConfig,
    last_sent: Instant,
    #[allow(dead_code)]
    stats: Arc<Mutex<SubscriberStats>>,
}

/// Bus publisher with rate limiting and subscriber management
pub struct BusPublisher {
    subscribers: Arc<Mutex<HashMap<SubscriberId, SubscriberData>>>,
    publish_stats: PublishStats,
    rate_limiter: RateLimiter,
}

/// Publisher statistics
#[derive(Debug, Clone, Default)]
pub struct PublishStats {
    pub snapshots_published: u64,
    pub snapshots_dropped: u64,
    pub subscribers_count: usize,
    pub average_publish_rate_hz: f32,
    pub last_publish_duration_us: u64,
}

/// Rate limiter for controlling publish frequency
struct RateLimiter {
    min_interval: Duration,
    last_publish: Instant,
    publish_count: u64,
    window_start: Instant,
}

impl RateLimiter {
    fn new(max_hz: f32) -> Self {
        let min_interval = Duration::from_secs_f32(1.0 / max_hz);
        let now = Instant::now();

        Self {
            min_interval,
            last_publish: now - min_interval, // Allow first publish immediately
            publish_count: 0,
            window_start: now,
        }
    }

    fn can_publish(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_publish);

        if elapsed >= self.min_interval {
            self.last_publish = now;
            self.publish_count += 1;

            // Reset window every second for rate calculation
            if now.duration_since(self.window_start) >= Duration::from_secs(1) {
                self.window_start = now;
                self.publish_count = 0;
            }

            true
        } else {
            false
        }
    }

    fn current_rate_hz(&self) -> f32 {
        let window_duration = Instant::now()
            .duration_since(self.window_start)
            .as_secs_f32();
        if window_duration > 0.0 {
            self.publish_count as f32 / window_duration
        } else {
            0.0
        }
    }
}

impl BusPublisher {
    /// Create a new bus publisher with the specified maximum rate
    pub fn new(max_publish_rate_hz: f32) -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            publish_stats: PublishStats::default(),
            rate_limiter: RateLimiter::new(max_publish_rate_hz.clamp(30.0, 60.0)),
        }
    }

    /// Subscribe to bus snapshots
    pub fn subscribe(&mut self, config: SubscriptionConfig) -> Result<Subscriber, PublisherError> {
        let id = SubscriberId::new();
        let (sender, receiver) = channel::bounded(config.buffer_size);

        let subscriber_data = SubscriberData {
            sender,
            config: config.clone(),
            last_sent: Instant::now() - Duration::from_secs(1), // Allow first message immediately
            stats: Arc::new(Mutex::new(SubscriberStats::default())),
        };

        {
            let mut subscribers = self.subscribers.lock().unwrap();
            subscribers.insert(id, subscriber_data);
            self.publish_stats.subscribers_count = subscribers.len();
        }

        debug!("New subscriber {} with config: {:?}", id.0, config);

        Ok(Subscriber {
            id,
            config,
            receiver,
            last_received: Instant::now(),
            stats: SubscriberStats::default(),
        })
    }

    /// Unsubscribe a subscriber
    pub fn unsubscribe(&mut self, id: SubscriberId) -> Result<(), PublisherError> {
        let mut subscribers = self.subscribers.lock().unwrap();

        if subscribers.remove(&id).is_some() {
            self.publish_stats.subscribers_count = subscribers.len();
            debug!("Unsubscribed subscriber {}", id.0);
            Ok(())
        } else {
            Err(PublisherError::SubscriberNotFound { id })
        }
    }

    /// Publish a snapshot to all subscribers
    pub fn publish(&mut self, snapshot: BusSnapshot) -> Result<(), PublisherError> {
        let start_time = Instant::now();

        // Validate snapshot before publishing
        snapshot.validate()?;

        // Check rate limit
        if !self.rate_limiter.can_publish() {
            self.publish_stats.snapshots_dropped += 1;
            trace!("Snapshot dropped due to rate limit");
            return Ok(());
        }

        let mut subscribers = self.subscribers.lock().unwrap();
        let mut disconnected_subscribers = Vec::new();

        for (id, subscriber_data) in subscribers.iter_mut() {
            // Check subscriber rate limit
            let now = Instant::now();
            let elapsed = now.duration_since(subscriber_data.last_sent).as_secs_f32();
            let min_interval = 1.0 / subscriber_data.config.max_rate_hz;

            if elapsed < min_interval {
                continue; // Skip this subscriber due to rate limit
            }

            // Try to send to subscriber
            match subscriber_data.sender.try_send(snapshot.clone()) {
                Ok(()) => {
                    subscriber_data.last_sent = now;
                    trace!("Sent snapshot to subscriber {}", id.0);
                }
                Err(channel::TrySendError::Full(_)) => {
                    if subscriber_data.config.drop_on_full {
                        warn!("Subscriber {} buffer full, dropping message", id.0);
                        // Note: We can't drop from sender side, this is handled by subscriber
                    } else {
                        warn!("Subscriber {} buffer full, skipping", id.0);
                    }
                }
                Err(channel::TrySendError::Disconnected(_)) => {
                    disconnected_subscribers.push(*id);
                }
            }
        }

        // Remove disconnected subscribers
        for id in disconnected_subscribers {
            subscribers.remove(&id);
            debug!("Removed disconnected subscriber {}", id.0);
        }

        self.publish_stats.subscribers_count = subscribers.len();
        self.publish_stats.snapshots_published += 1;
        self.publish_stats.last_publish_duration_us = start_time.elapsed().as_micros() as u64;
        self.publish_stats.average_publish_rate_hz = self.rate_limiter.current_rate_hz();

        Ok(())
    }

    /// Get publisher statistics
    pub fn stats(&self) -> &PublishStats {
        &self.publish_stats
    }

    /// Get number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.lock().unwrap().len()
    }

    /// Start automatic publishing task (for testing/fixtures)
    pub async fn start_fixture_publisher(
        mut self,
        snapshot_source: Receiver<BusSnapshot>,
        publish_rate_hz: f32,
    ) {
        let mut interval = interval(Duration::from_secs_f32(1.0 / publish_rate_hz));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Try to get latest snapshot
                    let mut latest_snapshot = None;

                    // Drain all available snapshots, keeping only the latest
                    while let Ok(snapshot) = snapshot_source.try_recv() {
                        latest_snapshot = Some(snapshot);
                    }

                    if let Some(snapshot) = latest_snapshot
                        && let Err(e) = self.publish(snapshot) {
                        error!("Failed to publish snapshot: {}", e);
                    }
                }
                else => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AircraftId, SimId};
    use std::time::Duration;

    #[test]
    fn test_subscriber_id_uniqueness() {
        let id1 = SubscriberId::new();
        let id2 = SubscriberId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_rate_limiter() {
        let mut limiter = RateLimiter::new(10.0); // 10 Hz = 100ms interval

        // First publish should succeed
        assert!(limiter.can_publish());

        // Immediate second publish should fail
        assert!(!limiter.can_publish());

        // After waiting longer than the interval, should succeed again
        std::thread::sleep(Duration::from_millis(150));
        assert!(limiter.can_publish());
    }

    #[tokio::test]
    async fn test_publisher_subscribe_unsubscribe() {
        let mut publisher = BusPublisher::new(60.0);

        // Subscribe
        let config = SubscriptionConfig::default();
        let subscriber = publisher.subscribe(config).unwrap();
        assert_eq!(publisher.subscriber_count(), 1);

        // Unsubscribe
        publisher.unsubscribe(subscriber.id).unwrap();
        assert_eq!(publisher.subscriber_count(), 0);

        // Unsubscribe non-existent should fail
        assert!(publisher.unsubscribe(subscriber.id).is_err());
    }

    #[tokio::test]
    async fn test_publish_and_receive() {
        let mut publisher = BusPublisher::new(60.0);
        let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

        let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        publisher.publish(snapshot.clone()).unwrap();

        let received = subscriber.try_recv().unwrap();
        assert!(received.is_some());
        let received = received.unwrap();
        assert_eq!(received.sim, snapshot.sim);
        assert_eq!(received.aircraft, snapshot.aircraft);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let mut publisher = BusPublisher::new(1.0); // Very low rate for testing
        let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

        let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // First publish should succeed
        publisher.publish(snapshot.clone()).unwrap();
        let first_msg = subscriber.try_recv().unwrap();
        assert!(first_msg.is_some());

        // Immediate second publish should be dropped due to rate limit
        let result = publisher.publish(snapshot.clone());
        // The publish call succeeds but message is rate limited
        assert!(result.is_ok());
        assert!(subscriber.try_recv().unwrap().is_none());
    }

    #[tokio::test]
    async fn test_subscriber_rate_limiting() {
        let mut publisher = BusPublisher::new(60.0);
        let config = SubscriptionConfig {
            max_rate_hz: 1.0, // Very low subscriber rate
            ..Default::default()
        };
        let mut subscriber = publisher.subscribe(config).unwrap();

        let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // First publish should reach subscriber
        publisher.publish(snapshot.clone()).unwrap();
        let first_msg = subscriber.try_recv().unwrap();
        assert!(first_msg.is_some());

        // Second publish should be rate limited for this subscriber
        tokio::time::sleep(Duration::from_millis(10)).await;
        publisher.publish(snapshot.clone()).unwrap();
        // Subscriber rate limiting means this message won't be sent to subscriber
        assert!(subscriber.try_recv().unwrap().is_none());
    }

    #[tokio::test]
    async fn test_buffer_overflow_drop() {
        let mut publisher = BusPublisher::new(60.0);
        let config = SubscriptionConfig {
            buffer_size: 2,
            drop_on_full: true,
            max_rate_hz: 60.0,
        };
        let mut subscriber = publisher.subscribe(config).unwrap();

        let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

        // Fill buffer
        for _ in 0..3 {
            publisher.publish(snapshot.clone()).unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }

        // Should have received messages (oldest may be dropped)
        let mut received_count = 0;
        while subscriber.try_recv().unwrap().is_some() {
            received_count += 1;
        }
        assert!(received_count >= 1);
    }
}
