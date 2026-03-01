// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Subscription manager for IPC streaming topics.
//!
//! Provides [`SubscriptionManager`] for server-side fan-out of streaming
//! events and [`SubscriptionHandle`] for client-side subscription lifecycle
//! management.  Subscriptions target a [`Topic`] and may carry an optional
//! [`SubscriptionFilter`] to narrow delivery (e.g. by device, axis, or rate).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Topic
// ---------------------------------------------------------------------------

/// Event topics that clients can subscribe to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Topic {
    /// Real-time axis value updates (250 Hz spine data).
    AxisData,
    /// Device connect / disconnect / error events.
    DeviceEvents,
    /// Simulator adapter telemetry (sim variables, frame timing).
    SimTelemetry,
    /// Profile load / switch / merge events.
    ProfileChanges,
    /// System health updates (jitter, latency, faults).
    HealthStatus,
    /// Force-feedback engine state changes.
    FfbStatus,
}

impl Topic {
    /// All topic variants, useful for iteration.
    pub const ALL: &'static [Topic] = &[
        Topic::AxisData,
        Topic::DeviceEvents,
        Topic::SimTelemetry,
        Topic::ProfileChanges,
        Topic::HealthStatus,
        Topic::FfbStatus,
    ];
}

impl std::fmt::Display for Topic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Topic::AxisData => write!(f, "AxisData"),
            Topic::DeviceEvents => write!(f, "DeviceEvents"),
            Topic::SimTelemetry => write!(f, "SimTelemetry"),
            Topic::ProfileChanges => write!(f, "ProfileChanges"),
            Topic::HealthStatus => write!(f, "HealthStatus"),
            Topic::FfbStatus => write!(f, "FfbStatus"),
        }
    }
}

// ---------------------------------------------------------------------------
// SubscriptionFilter
// ---------------------------------------------------------------------------

/// Optional criteria that narrow which events are delivered to a subscriber.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SubscriptionFilter {
    /// Only deliver events for this device.
    pub device_id: Option<String>,
    /// Only deliver events for this axis.
    pub axis_id: Option<String>,
    /// Minimum interval between deliveries (throttle), in milliseconds.
    pub min_interval_ms: Option<u64>,
    /// When `true`, only deliver when the value has changed.
    pub changed_only: bool,
}

impl SubscriptionFilter {
    /// Returns `true` when the filter has no criteria set.
    pub fn is_empty(&self) -> bool {
        self.device_id.is_none()
            && self.axis_id.is_none()
            && self.min_interval_ms.is_none()
            && !self.changed_only
    }

    /// Check whether a message with the given device/axis IDs passes this filter.
    pub fn matches(&self, device_id: Option<&str>, axis_id: Option<&str>) -> bool {
        if let Some(ref required) = self.device_id {
            match device_id {
                Some(actual) if actual == required => {}
                _ => return false,
            }
        }
        if let Some(ref required) = self.axis_id {
            match axis_id {
                Some(actual) if actual == required => {}
                _ => return false,
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// SubscriptionHandle
// ---------------------------------------------------------------------------

/// Unique subscription identifier.
pub type SubscriptionId = u64;

/// Client-side handle to an active subscription.
///
/// The subscription is automatically cancelled when the last clone of the
/// handle is dropped (via the shared `active` flag).
#[derive(Debug, Clone)]
pub struct SubscriptionHandle {
    /// Unique subscription ID.
    pub id: SubscriptionId,
    /// The topic this subscription targets.
    pub topic: Topic,
    /// Optional filter criteria.
    pub filter: SubscriptionFilter,
    /// Monotonic instant when the subscription was created.
    pub created_at: Instant,
    /// Shared flag: `true` while the subscription is active.
    active: Arc<AtomicBool>,
}

impl SubscriptionHandle {
    /// Returns `true` if this subscription is still active.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Cancel the subscription.  Idempotent.
    pub fn cancel(&self) {
        self.active.store(false, Ordering::Release);
    }
}

impl Drop for SubscriptionHandle {
    fn drop(&mut self) {
        // When the last *client-side* reference is dropped (only the manager's
        // copy remains), mark the subscription inactive so GC reclaims it.
        // strong_count == 2: one in the manager record, one being dropped now.
        if Arc::strong_count(&self.active) <= 2 {
            self.active.store(false, Ordering::Release);
        }
    }
}

// ---------------------------------------------------------------------------
// BroadcastMessage
// ---------------------------------------------------------------------------

/// A message delivered through [`SubscriptionManager::broadcast`].
#[derive(Debug, Clone)]
pub struct BroadcastMessage {
    /// Topic the message belongs to.
    pub topic: Topic,
    /// Opaque payload (JSON-serialisable).
    pub payload: String,
    /// Optional device ID for filter matching.
    pub device_id: Option<String>,
    /// Optional axis ID for filter matching.
    pub axis_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Internal record kept by the manager
// ---------------------------------------------------------------------------

struct SubscriptionRecord {
    id: SubscriptionId,
    topic: Topic,
    filter: SubscriptionFilter,
    created_at: Instant,
    active: Arc<AtomicBool>,
    /// Last delivery instant – used for rate-throttling.
    last_delivery: Option<Instant>,
    /// Last delivered payload hash – used for `changed_only`.
    last_payload_hash: Option<u64>,
    /// Maximum pending deliveries before the subscriber is considered slow.
    capacity: usize,
    /// Number of pending (unacknowledged) deliveries.
    pending: usize,
    /// Cumulative number of events dropped due to backpressure.
    dropped: u64,
}

// ---------------------------------------------------------------------------
// BackpressureStats
// ---------------------------------------------------------------------------

/// Per-broadcast statistics returned by
/// [`SubscriptionManager::broadcast_with_backpressure`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackpressureStats {
    /// Subscription IDs that received the event.
    pub delivered: Vec<SubscriptionId>,
    /// Subscription IDs that were skipped because their queue was full.
    pub dropped: Vec<SubscriptionId>,
}

// ---------------------------------------------------------------------------
// SubscriptionManager
// ---------------------------------------------------------------------------

/// Server-side manager for streaming subscriptions.
///
/// Tracks all active subscriptions, supports per-topic fan-out with optional
/// filtering and rate-throttling, and garbage-collects cancelled handles.
pub struct SubscriptionManager {
    subscriptions: HashMap<SubscriptionId, SubscriptionRecord>,
    next_id: AtomicU64,
}

impl SubscriptionManager {
    /// Create an empty subscription manager.
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    /// Subscribe to `topic` with an optional `filter`.
    ///
    /// Returns a [`SubscriptionHandle`] the caller can use to check liveness
    /// or cancel the subscription.
    pub fn subscribe(&mut self, topic: Topic, filter: SubscriptionFilter) -> SubscriptionHandle {
        self.subscribe_with_capacity(topic, filter, usize::MAX)
    }

    /// Subscribe with an explicit backpressure capacity.
    ///
    /// When the subscriber accumulates more than `capacity` pending deliveries,
    /// new events are dropped and counted via [`dropped_count`](Self::dropped_count).
    pub fn subscribe_with_capacity(
        &mut self,
        topic: Topic,
        filter: SubscriptionFilter,
        capacity: usize,
    ) -> SubscriptionHandle {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let active = Arc::new(AtomicBool::new(true));
        let now = Instant::now();

        self.subscriptions.insert(
            id,
            SubscriptionRecord {
                id,
                topic,
                filter: filter.clone(),
                created_at: now,
                active: Arc::clone(&active),
                last_delivery: None,
                last_payload_hash: None,
                capacity,
                pending: 0,
                dropped: 0,
            },
        );

        SubscriptionHandle {
            id,
            topic,
            filter,
            created_at: now,
            active,
        }
    }

    /// Cancel a subscription by handle.  Returns `true` if the subscription
    /// was found and removed.
    pub fn unsubscribe(&mut self, handle: &SubscriptionHandle) -> bool {
        handle.cancel();
        self.subscriptions.remove(&handle.id).is_some()
    }

    /// Return a snapshot of all currently active subscriptions.
    pub fn active_subscriptions(&mut self) -> Vec<SubscriptionHandle> {
        self.gc();
        self.subscriptions
            .values()
            .map(|rec| SubscriptionHandle {
                id: rec.id,
                topic: rec.topic,
                filter: rec.filter.clone(),
                created_at: rec.created_at,
                active: Arc::clone(&rec.active),
            })
            .collect()
    }

    /// Broadcast a message to all matching subscribers.
    ///
    /// Returns the subscription IDs that received the message (i.e. passed
    /// both filter matching and rate-throttle checks).
    pub fn broadcast(&mut self, message: &BroadcastMessage) -> Vec<SubscriptionId> {
        self.gc();

        let now = Instant::now();
        let payload_hash = simple_hash(&message.payload);
        let mut delivered = Vec::new();

        for rec in self.subscriptions.values_mut() {
            if rec.topic != message.topic {
                continue;
            }

            // Filter matching
            if !rec
                .filter
                .matches(message.device_id.as_deref(), message.axis_id.as_deref())
            {
                continue;
            }

            // Rate throttle
            if let Some(min_ms) = rec.filter.min_interval_ms
                && let Some(last) = rec.last_delivery
                && now.duration_since(last).as_millis() < u128::from(min_ms)
            {
                continue;
            }

            // Changed-only check
            if rec.filter.changed_only && rec.last_payload_hash == Some(payload_hash) {
                continue;
            }

            rec.last_delivery = Some(now);
            rec.last_payload_hash = Some(payload_hash);
            delivered.push(rec.id);
        }

        delivered
    }

    /// Broadcast with backpressure: slow subscribers whose pending count has
    /// reached their capacity will have the event dropped (and counted).
    pub fn broadcast_with_backpressure(&mut self, message: &BroadcastMessage) -> BackpressureStats {
        self.gc();

        let now = Instant::now();
        let payload_hash = simple_hash(&message.payload);
        let mut delivered = Vec::new();
        let mut dropped = Vec::new();

        for rec in self.subscriptions.values_mut() {
            if rec.topic != message.topic {
                continue;
            }

            if !rec
                .filter
                .matches(message.device_id.as_deref(), message.axis_id.as_deref())
            {
                continue;
            }

            if let Some(min_ms) = rec.filter.min_interval_ms
                && let Some(last) = rec.last_delivery
                && now.duration_since(last).as_millis() < u128::from(min_ms)
            {
                continue;
            }

            if rec.filter.changed_only && rec.last_payload_hash == Some(payload_hash) {
                continue;
            }

            // Backpressure check
            if rec.pending >= rec.capacity {
                rec.dropped += 1;
                dropped.push(rec.id);
                continue;
            }

            rec.last_delivery = Some(now);
            rec.last_payload_hash = Some(payload_hash);
            rec.pending += 1;
            delivered.push(rec.id);
        }

        BackpressureStats { delivered, dropped }
    }

    /// Acknowledge that a subscriber has consumed a pending delivery.
    ///
    /// This decrements the subscriber's pending counter, allowing future
    /// deliveries when backpressure is in effect.
    pub fn acknowledge(&mut self, id: SubscriptionId) {
        if let Some(rec) = self.subscriptions.get_mut(&id) {
            rec.pending = rec.pending.saturating_sub(1);
        }
    }

    /// Return the cumulative number of events dropped for `id` due to
    /// backpressure.
    pub fn dropped_count(&self, id: SubscriptionId) -> u64 {
        self.subscriptions.get(&id).map_or(0, |rec| rec.dropped)
    }

    /// Number of active subscriptions (after garbage collection).
    pub fn active_count(&mut self) -> usize {
        self.gc();
        self.subscriptions.len()
    }

    /// Remove records whose active flag has been cleared (cancelled or
    /// dropped handles).
    fn gc(&mut self) {
        self.subscriptions
            .retain(|_, rec| rec.active.load(Ordering::Acquire));
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Trivial FNV-1a-style hash for `changed_only` payload dedup.
fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in s.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers --

    fn no_filter() -> SubscriptionFilter {
        SubscriptionFilter::default()
    }

    fn device_filter(id: &str) -> SubscriptionFilter {
        SubscriptionFilter {
            device_id: Some(id.to_owned()),
            ..Default::default()
        }
    }

    fn axis_filter(id: &str) -> SubscriptionFilter {
        SubscriptionFilter {
            axis_id: Some(id.to_owned()),
            ..Default::default()
        }
    }

    fn msg(topic: Topic, payload: &str) -> BroadcastMessage {
        BroadcastMessage {
            topic,
            payload: payload.to_owned(),
            device_id: None,
            axis_id: None,
        }
    }

    fn msg_with_device(topic: Topic, payload: &str, device: &str) -> BroadcastMessage {
        BroadcastMessage {
            topic,
            payload: payload.to_owned(),
            device_id: Some(device.to_owned()),
            axis_id: None,
        }
    }

    fn msg_with_axis(topic: Topic, payload: &str, axis: &str) -> BroadcastMessage {
        BroadcastMessage {
            topic,
            payload: payload.to_owned(),
            device_id: None,
            axis_id: Some(axis.to_owned()),
        }
    }

    // -----------------------------------------------------------------------
    // 1. New manager is empty
    // -----------------------------------------------------------------------
    #[test]
    fn new_manager_is_empty() {
        let mut mgr = SubscriptionManager::new();
        assert_eq!(mgr.active_count(), 0);
        assert!(mgr.active_subscriptions().is_empty());
    }

    // -----------------------------------------------------------------------
    // 2. Default trait creates empty manager
    // -----------------------------------------------------------------------
    #[test]
    fn default_creates_empty_manager() {
        let mut mgr = SubscriptionManager::default();
        assert_eq!(mgr.active_count(), 0);
    }

    // -----------------------------------------------------------------------
    // 3. Subscribe returns a handle with correct metadata
    // -----------------------------------------------------------------------
    #[test]
    fn subscribe_returns_valid_handle() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::AxisData, no_filter());

        assert!(h.is_active());
        assert_eq!(h.topic, Topic::AxisData);
        assert!(h.filter.is_empty());
        assert_eq!(mgr.active_count(), 1);
    }

    // -----------------------------------------------------------------------
    // 4. Each subscription gets a unique ID
    // -----------------------------------------------------------------------
    #[test]
    fn subscription_ids_are_unique() {
        let mut mgr = SubscriptionManager::new();
        let h1 = mgr.subscribe(Topic::AxisData, no_filter());
        let h2 = mgr.subscribe(Topic::DeviceEvents, no_filter());
        let h3 = mgr.subscribe(Topic::AxisData, no_filter());

        assert_ne!(h1.id, h2.id);
        assert_ne!(h2.id, h3.id);
        assert_ne!(h1.id, h3.id);
    }

    // -----------------------------------------------------------------------
    // 5. Unsubscribe removes subscription and marks handle inactive
    // -----------------------------------------------------------------------
    #[test]
    fn unsubscribe_removes_and_deactivates() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::DeviceEvents, no_filter());

        assert!(mgr.unsubscribe(&h));
        assert!(!h.is_active());
        assert_eq!(mgr.active_count(), 0);
    }

    // -----------------------------------------------------------------------
    // 6. Unsubscribe unknown handle returns false
    // -----------------------------------------------------------------------
    #[test]
    fn unsubscribe_unknown_returns_false() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::AxisData, no_filter());
        mgr.unsubscribe(&h);

        // Second unsubscribe should return false
        assert!(!mgr.unsubscribe(&h));
    }

    // -----------------------------------------------------------------------
    // 7. Handle cancel marks inactive; GC removes it
    // -----------------------------------------------------------------------
    #[test]
    fn handle_cancel_then_gc() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::HealthStatus, no_filter());
        h.cancel();

        assert!(!h.is_active());
        assert_eq!(mgr.active_count(), 0);
    }

    // -----------------------------------------------------------------------
    // 8. Handle drop (last client clone) marks inactive via GC
    // -----------------------------------------------------------------------
    #[test]
    fn handle_drop_marks_inactive() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::FfbStatus, no_filter());

        // Dropping the only client handle should deactivate the subscription
        drop(h);
        assert_eq!(mgr.active_count(), 0);
    }

    // -----------------------------------------------------------------------
    // 9. Multiple subscribers on same topic
    // -----------------------------------------------------------------------
    #[test]
    fn multiple_subscribers_same_topic() {
        let mut mgr = SubscriptionManager::new();
        let _h1 = mgr.subscribe(Topic::AxisData, no_filter());
        let _h2 = mgr.subscribe(Topic::AxisData, no_filter());
        let _h3 = mgr.subscribe(Topic::AxisData, no_filter());

        assert_eq!(mgr.active_count(), 3);

        let ids = mgr.broadcast(&msg(Topic::AxisData, "test"));
        assert_eq!(ids.len(), 3);
    }

    // -----------------------------------------------------------------------
    // 10. Broadcast only delivers to matching topic
    // -----------------------------------------------------------------------
    #[test]
    fn broadcast_topic_matching() {
        let mut mgr = SubscriptionManager::new();
        let h_axis = mgr.subscribe(Topic::AxisData, no_filter());
        let _h_dev = mgr.subscribe(Topic::DeviceEvents, no_filter());

        let ids = mgr.broadcast(&msg(Topic::AxisData, "axis-val"));
        assert_eq!(ids, vec![h_axis.id]);
    }

    // -----------------------------------------------------------------------
    // 11. Filter matching — device_id
    // -----------------------------------------------------------------------
    #[test]
    fn filter_device_id() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::DeviceEvents, device_filter("dev-1"));

        // Matching device
        let ids = mgr.broadcast(&msg_with_device(Topic::DeviceEvents, "evt", "dev-1"));
        assert_eq!(ids, vec![h.id]);

        // Non-matching device
        let ids = mgr.broadcast(&msg_with_device(Topic::DeviceEvents, "evt", "dev-2"));
        assert!(ids.is_empty());

        // No device in message — fails device filter
        let ids = mgr.broadcast(&msg(Topic::DeviceEvents, "evt"));
        assert!(ids.is_empty());
    }

    // -----------------------------------------------------------------------
    // 12. Filter matching — axis_id
    // -----------------------------------------------------------------------
    #[test]
    fn filter_axis_id() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::AxisData, axis_filter("pitch"));

        let ids = mgr.broadcast(&msg_with_axis(Topic::AxisData, "v", "pitch"));
        assert_eq!(ids, vec![h.id]);

        let ids = mgr.broadcast(&msg_with_axis(Topic::AxisData, "v", "roll"));
        assert!(ids.is_empty());
    }

    // -----------------------------------------------------------------------
    // 13. Filter matching — no filter passes everything
    // -----------------------------------------------------------------------
    #[test]
    fn no_filter_matches_all() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe(Topic::SimTelemetry, no_filter());

        let ids = mgr.broadcast(&msg(Topic::SimTelemetry, "data"));
        assert_eq!(ids, vec![h.id]);

        let ids = mgr.broadcast(&msg_with_device(Topic::SimTelemetry, "data", "dev-x"));
        assert_eq!(ids, vec![h.id]);
    }

    // -----------------------------------------------------------------------
    // 14. Empty filter is detected
    // -----------------------------------------------------------------------
    #[test]
    fn filter_is_empty() {
        assert!(no_filter().is_empty());
        assert!(!device_filter("x").is_empty());
        assert!(
            !SubscriptionFilter {
                changed_only: true,
                ..Default::default()
            }
            .is_empty()
        );
    }

    // -----------------------------------------------------------------------
    // 15. Rate throttling — min_interval_ms
    // -----------------------------------------------------------------------
    #[test]
    fn rate_throttle() {
        let mut mgr = SubscriptionManager::new();
        let filter = SubscriptionFilter {
            min_interval_ms: Some(1_000), // 1 second throttle
            ..Default::default()
        };
        let h = mgr.subscribe(Topic::AxisData, filter);

        // First delivery should succeed
        let ids = mgr.broadcast(&msg(Topic::AxisData, "v1"));
        assert_eq!(ids, vec![h.id]);

        // Immediate second delivery should be throttled
        let ids = mgr.broadcast(&msg(Topic::AxisData, "v2"));
        assert!(ids.is_empty());
    }

    // -----------------------------------------------------------------------
    // 16. Changed-only filter
    // -----------------------------------------------------------------------
    #[test]
    fn changed_only_filter() {
        let mut mgr = SubscriptionManager::new();
        let filter = SubscriptionFilter {
            changed_only: true,
            ..Default::default()
        };
        let h = mgr.subscribe(Topic::HealthStatus, filter);

        // First delivery always goes through
        let ids = mgr.broadcast(&msg(Topic::HealthStatus, "state-A"));
        assert_eq!(ids, vec![h.id]);

        // Same payload — suppressed
        let ids = mgr.broadcast(&msg(Topic::HealthStatus, "state-A"));
        assert!(ids.is_empty());

        // Different payload — delivered
        let ids = mgr.broadcast(&msg(Topic::HealthStatus, "state-B"));
        assert_eq!(ids, vec![h.id]);
    }

    // -----------------------------------------------------------------------
    // 17. active_subscriptions returns only active subs
    // -----------------------------------------------------------------------
    #[test]
    fn active_subscriptions_snapshot() {
        let mut mgr = SubscriptionManager::new();
        let h1 = mgr.subscribe(Topic::AxisData, no_filter());
        let h2 = mgr.subscribe(Topic::DeviceEvents, no_filter());
        let _h3 = mgr.subscribe(Topic::FfbStatus, no_filter());

        h1.cancel();
        mgr.unsubscribe(&h2);

        let active = mgr.active_subscriptions();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].topic, Topic::FfbStatus);
    }

    // -----------------------------------------------------------------------
    // 18. Broadcast skips cancelled subscriptions
    // -----------------------------------------------------------------------
    #[test]
    fn broadcast_skips_cancelled() {
        let mut mgr = SubscriptionManager::new();
        let h1 = mgr.subscribe(Topic::ProfileChanges, no_filter());
        let _h2 = mgr.subscribe(Topic::ProfileChanges, no_filter());

        h1.cancel();

        let ids = mgr.broadcast(&msg(Topic::ProfileChanges, "p"));
        assert_eq!(ids.len(), 1);
        assert_ne!(ids[0], h1.id);
    }

    // -----------------------------------------------------------------------
    // 19. Combined device + axis filter
    // -----------------------------------------------------------------------
    #[test]
    fn combined_device_axis_filter() {
        let mut mgr = SubscriptionManager::new();
        let filter = SubscriptionFilter {
            device_id: Some("js-1".to_owned()),
            axis_id: Some("roll".to_owned()),
            ..Default::default()
        };
        let h = mgr.subscribe(Topic::AxisData, filter);

        // Both match
        let m = BroadcastMessage {
            topic: Topic::AxisData,
            payload: "v".to_owned(),
            device_id: Some("js-1".to_owned()),
            axis_id: Some("roll".to_owned()),
        };
        assert_eq!(mgr.broadcast(&m), vec![h.id]);

        // Device matches, axis doesn't
        let m2 = BroadcastMessage {
            topic: Topic::AxisData,
            payload: "v".to_owned(),
            device_id: Some("js-1".to_owned()),
            axis_id: Some("pitch".to_owned()),
        };
        assert!(mgr.broadcast(&m2).is_empty());
    }

    // -----------------------------------------------------------------------
    // 20. Topic Display impl
    // -----------------------------------------------------------------------
    #[test]
    fn topic_display() {
        assert_eq!(format!("{}", Topic::AxisData), "AxisData");
        assert_eq!(format!("{}", Topic::FfbStatus), "FfbStatus");
    }

    // -----------------------------------------------------------------------
    // 21. Topic::ALL contains all variants
    // -----------------------------------------------------------------------
    #[test]
    fn topic_all_variants() {
        assert_eq!(Topic::ALL.len(), 6);
    }

    // -----------------------------------------------------------------------
    // 22. Handle clone shares active flag
    // -----------------------------------------------------------------------
    #[test]
    fn handle_clone_shares_state() {
        let mut mgr = SubscriptionManager::new();
        let h1 = mgr.subscribe(Topic::AxisData, no_filter());
        let h2 = h1.clone();

        h1.cancel();
        assert!(!h2.is_active());
    }

    // -----------------------------------------------------------------------
    // 23. Broadcast to empty manager returns empty
    // -----------------------------------------------------------------------
    #[test]
    fn broadcast_empty_manager() {
        let mut mgr = SubscriptionManager::new();
        let ids = mgr.broadcast(&msg(Topic::AxisData, "noop"));
        assert!(ids.is_empty());
    }

    // -----------------------------------------------------------------------
    // 24. Many subscriptions across different topics
    // -----------------------------------------------------------------------
    #[test]
    fn many_subscriptions_different_topics() {
        let mut mgr = SubscriptionManager::new();
        let mut handles = Vec::new();
        for &topic in Topic::ALL {
            handles.push(mgr.subscribe(topic, no_filter()));
        }
        assert_eq!(mgr.active_count(), 6);

        // Broadcast to one topic should reach exactly one subscriber
        let ids = mgr.broadcast(&msg(Topic::SimTelemetry, "t"));
        assert_eq!(ids.len(), 1);
    }

    // -----------------------------------------------------------------------
    // 25. SubscriptionFilter with min_interval_ms but no delivery yet
    // -----------------------------------------------------------------------
    #[test]
    fn throttle_first_message_always_delivered() {
        let mut mgr = SubscriptionManager::new();
        let filter = SubscriptionFilter {
            min_interval_ms: Some(5_000),
            ..Default::default()
        };
        let h = mgr.subscribe(Topic::FfbStatus, filter);

        let ids = mgr.broadcast(&msg(Topic::FfbStatus, "first"));
        assert_eq!(ids, vec![h.id]);
    }

    // -----------------------------------------------------------------------
    // 26. changed_only with different payloads always delivers
    // -----------------------------------------------------------------------
    #[test]
    fn changed_only_different_payloads_always_deliver() {
        let mut mgr = SubscriptionManager::new();
        let filter = SubscriptionFilter {
            changed_only: true,
            ..Default::default()
        };
        let h = mgr.subscribe(Topic::ProfileChanges, filter);

        for i in 0..10 {
            let ids = mgr.broadcast(&msg(Topic::ProfileChanges, &format!("payload-{i}")));
            assert_eq!(ids, vec![h.id]);
        }
    }

    // -----------------------------------------------------------------------
    // 27. Unsubscribe one of many on same topic
    // -----------------------------------------------------------------------
    #[test]
    fn unsubscribe_one_of_many() {
        let mut mgr = SubscriptionManager::new();
        let h1 = mgr.subscribe(Topic::AxisData, no_filter());
        let _h2 = mgr.subscribe(Topic::AxisData, no_filter());
        let _h3 = mgr.subscribe(Topic::AxisData, no_filter());

        mgr.unsubscribe(&h1);
        assert_eq!(mgr.active_count(), 2);

        let ids = mgr.broadcast(&msg(Topic::AxisData, "val"));
        assert_eq!(ids.len(), 2);
        assert!(!ids.contains(&h1.id));
    }

    // ===== Backpressure tests ==============================================

    // -----------------------------------------------------------------------
    // 28. Backpressure drops events when subscriber is slow
    // -----------------------------------------------------------------------
    #[test]
    fn backpressure_drops_slow_subscriber() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe_with_capacity(Topic::AxisData, no_filter(), 2);

        // First two deliveries should succeed
        let stats = mgr.broadcast_with_backpressure(&msg(Topic::AxisData, "v1"));
        assert_eq!(stats.delivered, vec![h.id]);
        assert!(stats.dropped.is_empty());

        let stats = mgr.broadcast_with_backpressure(&msg(Topic::AxisData, "v2"));
        assert_eq!(stats.delivered, vec![h.id]);

        // Third delivery exceeds capacity → dropped
        let stats = mgr.broadcast_with_backpressure(&msg(Topic::AxisData, "v3"));
        assert!(stats.delivered.is_empty());
        assert_eq!(stats.dropped, vec![h.id]);

        assert_eq!(mgr.dropped_count(h.id), 1);
    }

    // -----------------------------------------------------------------------
    // 29. Acknowledge frees capacity
    // -----------------------------------------------------------------------
    #[test]
    fn acknowledge_frees_capacity() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe_with_capacity(Topic::DeviceEvents, no_filter(), 1);

        let stats = mgr.broadcast_with_backpressure(&msg(Topic::DeviceEvents, "e1"));
        assert_eq!(stats.delivered, vec![h.id]);

        // At capacity — next would be dropped
        let stats = mgr.broadcast_with_backpressure(&msg(Topic::DeviceEvents, "e2"));
        assert!(stats.delivered.is_empty());

        // Acknowledge → free a slot
        mgr.acknowledge(h.id);

        let stats = mgr.broadcast_with_backpressure(&msg(Topic::DeviceEvents, "e3"));
        assert_eq!(stats.delivered, vec![h.id]);
    }

    // -----------------------------------------------------------------------
    // 30. Dropped count accumulates
    // -----------------------------------------------------------------------
    #[test]
    fn dropped_count_accumulates() {
        let mut mgr = SubscriptionManager::new();
        let h = mgr.subscribe_with_capacity(Topic::HealthStatus, no_filter(), 0);

        for i in 0..5 {
            mgr.broadcast_with_backpressure(&msg(Topic::HealthStatus, &format!("d{i}")));
        }
        assert_eq!(mgr.dropped_count(h.id), 5);
    }

    // -----------------------------------------------------------------------
    // 31. Mixed fast and slow subscribers
    // -----------------------------------------------------------------------
    #[test]
    fn backpressure_mixed_fast_slow() {
        let mut mgr = SubscriptionManager::new();
        let fast = mgr.subscribe(Topic::AxisData, no_filter()); // unlimited
        let slow = mgr.subscribe_with_capacity(Topic::AxisData, no_filter(), 1);

        // First event reaches both
        let stats = mgr.broadcast_with_backpressure(&msg(Topic::AxisData, "a"));
        assert!(stats.delivered.contains(&fast.id));
        assert!(stats.delivered.contains(&slow.id));

        // Second event: slow is at capacity
        let stats = mgr.broadcast_with_backpressure(&msg(Topic::AxisData, "b"));
        assert!(stats.delivered.contains(&fast.id));
        assert!(stats.dropped.contains(&slow.id));
        assert_eq!(mgr.dropped_count(slow.id), 1);
        assert_eq!(mgr.dropped_count(fast.id), 0);
    }

    // -----------------------------------------------------------------------
    // 32. Dropped count for unknown subscription returns 0
    // -----------------------------------------------------------------------
    #[test]
    fn dropped_count_unknown_id() {
        let mgr = SubscriptionManager::new();
        assert_eq!(mgr.dropped_count(9999), 0);
    }
}
