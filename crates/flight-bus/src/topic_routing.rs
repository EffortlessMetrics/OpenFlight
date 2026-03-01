// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Topic-based routing, subscriber filtering, and backpressure for the RT bus.
//!
//! All hot-path operations use bitflag-based filtering and pre-allocated
//! fixed-size structures — no `Vec`, `String`, `HashMap`, or heap allocation.
//!
//! # Overview
//!
//! - [`EventDomain`] categorises bus events (telemetry, device, profile, etc.).
//! - [`TopicFilter`] holds a compact bitmask selecting one or more domains.
//! - [`BackpressurePolicy`] controls behaviour when a subscriber falls behind.
//! - [`BusStatistics`] tracks publish/drop counts with atomic counters.
//! - [`FilteredSubscriber`] wraps a destination ID with a topic filter.

use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// EventDomain
// ---------------------------------------------------------------------------

/// Domain categories for bus events.
///
/// Each variant maps to a single bit, allowing zero-allocation set membership
/// tests via [`TopicFilter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventDomain {
    Telemetry = 0,
    DeviceState = 1,
    ProfileChange = 2,
    SystemHealth = 3,
    AxisData = 4,
    FfbCommand = 5,
}

impl EventDomain {
    /// Total number of domain variants.
    pub const COUNT: usize = 6;

    /// All domain variants in definition order.
    pub const ALL: [EventDomain; Self::COUNT] = [
        EventDomain::Telemetry,
        EventDomain::DeviceState,
        EventDomain::ProfileChange,
        EventDomain::SystemHealth,
        EventDomain::AxisData,
        EventDomain::FfbCommand,
    ];

    /// Return the bitmask for this domain.
    #[inline]
    #[must_use]
    const fn mask(self) -> u8 {
        1 << (self as u8)
    }
}

// ---------------------------------------------------------------------------
// TopicFilter
// ---------------------------------------------------------------------------

/// Compact bitflag filter over [`EventDomain`] variants.
///
/// An empty filter (mask `0`) is treated as a wildcard that matches **all**
/// domains, following the principle of least surprise for default-constructed
/// filters.
///
/// All operations are `Copy` and allocation-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopicFilter {
    mask: u8,
}

impl TopicFilter {
    /// Create a filter that matches **all** domains (wildcard).
    #[inline]
    #[must_use]
    pub const fn all() -> Self {
        Self { mask: 0 }
    }

    /// Create a filter that matches nothing.
    #[inline]
    #[must_use]
    pub const fn none() -> Self {
        // Bit 7 is not used by any domain (domains use bits 0–5), so this
        // mask is non-zero (avoiding the wildcard) yet has no domain bits set.
        Self { mask: 0x80 }
    }

    /// Create a filter for a single domain.
    #[inline]
    #[must_use]
    pub const fn single(domain: EventDomain) -> Self {
        Self {
            mask: domain.mask(),
        }
    }

    /// Create a filter from multiple domains.
    #[must_use]
    pub fn from_domains(domains: &[EventDomain]) -> Self {
        let mut mask: u8 = 0;
        for d in domains {
            mask |= d.mask();
        }
        Self { mask }
    }

    /// Add a domain to this filter.
    #[inline]
    pub fn add(&mut self, domain: EventDomain) {
        if self.mask == 0 {
            // Transitioning from wildcard to explicit; start fresh.
            self.mask = domain.mask();
        } else {
            self.mask |= domain.mask();
        }
    }

    /// Remove a domain from this filter.
    #[inline]
    pub fn remove(&mut self, domain: EventDomain) {
        self.mask &= !domain.mask();
    }

    /// Check whether `domain` passes this filter.
    ///
    /// A wildcard filter (`mask == 0`) matches everything.
    #[inline]
    #[must_use]
    pub const fn matches(self, domain: EventDomain) -> bool {
        self.mask == 0 || (self.mask & domain.mask()) != 0
    }

    /// Return the raw bitmask (mostly useful for debugging / serialization).
    #[inline]
    #[must_use]
    pub const fn raw_mask(self) -> u8 {
        self.mask
    }

    /// Returns `true` when the filter is the wildcard (matches all domains).
    #[inline]
    #[must_use]
    pub const fn is_wildcard(self) -> bool {
        self.mask == 0
    }
}

impl Default for TopicFilter {
    /// Default filter is the wildcard — matches everything.
    fn default() -> Self {
        Self::all()
    }
}

// ---------------------------------------------------------------------------
// BackpressurePolicy
// ---------------------------------------------------------------------------

/// Per-subscriber policy for handling a full event buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressurePolicy {
    /// Drop the oldest event in the buffer to make room.
    DropOldest,
    /// Drop the newest (incoming) event.
    DropNewest,
    /// Block the publisher until the subscriber drains space (non-RT only!).
    Block,
}

impl Default for BackpressurePolicy {
    /// Default is `DropNewest` — safe for RT paths.
    fn default() -> Self {
        Self::DropNewest
    }
}

// ---------------------------------------------------------------------------
// BusStatistics
// ---------------------------------------------------------------------------

/// Maximum number of per-domain counters tracked by [`BusStatistics`].
const MAX_DOMAIN_STATS: usize = EventDomain::COUNT;

/// Maximum number of subscribers whose lag is tracked.
pub const MAX_TRACKED_SUBSCRIBERS: usize = 64;

/// Observable statistics for the bus.
///
/// All counters are atomic to allow lock-free recording from the RT thread
/// and lock-free reading from diagnostics threads.
#[derive(Debug)]
pub struct BusStatistics {
    published: AtomicU64,
    dropped: AtomicU64,
    per_domain_published: [AtomicU64; MAX_DOMAIN_STATS],
    per_domain_dropped: [AtomicU64; MAX_DOMAIN_STATS],
    /// Subscriber lag (events pending) — indexed by subscriber slot.
    subscriber_lag: [AtomicU64; MAX_TRACKED_SUBSCRIBERS],
}

impl BusStatistics {
    /// Create zeroed statistics.
    #[must_use]
    pub fn new() -> Self {
        Self {
            published: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            per_domain_published: std::array::from_fn(|_| AtomicU64::new(0)),
            per_domain_dropped: std::array::from_fn(|_| AtomicU64::new(0)),
            subscriber_lag: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }

    /// Record a published event in the given domain.
    #[inline]
    pub fn record_publish(&self, domain: EventDomain) {
        self.published.fetch_add(1, Ordering::Relaxed);
        self.per_domain_published[domain as usize].fetch_add(1, Ordering::Relaxed);
    }

    /// Record a dropped event in the given domain.
    #[inline]
    pub fn record_drop(&self, domain: EventDomain) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
        self.per_domain_dropped[domain as usize].fetch_add(1, Ordering::Relaxed);
    }

    /// Update subscriber lag (events pending in their buffer).
    #[inline]
    pub fn set_subscriber_lag(&self, slot: usize, lag: u64) {
        if slot < MAX_TRACKED_SUBSCRIBERS {
            self.subscriber_lag[slot].store(lag, Ordering::Relaxed);
        }
    }

    /// Total events published.
    #[inline]
    #[must_use]
    pub fn total_published(&self) -> u64 {
        self.published.load(Ordering::Relaxed)
    }

    /// Total events dropped.
    #[inline]
    #[must_use]
    pub fn total_dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Published count for a specific domain.
    #[inline]
    #[must_use]
    pub fn domain_published(&self, domain: EventDomain) -> u64 {
        self.per_domain_published[domain as usize].load(Ordering::Relaxed)
    }

    /// Dropped count for a specific domain.
    #[inline]
    #[must_use]
    pub fn domain_dropped(&self, domain: EventDomain) -> u64 {
        self.per_domain_dropped[domain as usize].load(Ordering::Relaxed)
    }

    /// Current lag for subscriber `slot`.
    #[inline]
    #[must_use]
    pub fn subscriber_lag(&self, slot: usize) -> u64 {
        if slot < MAX_TRACKED_SUBSCRIBERS {
            self.subscriber_lag[slot].load(Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Take a consistent snapshot of all statistics.
    #[must_use]
    pub fn snapshot(&self) -> BusStatisticsSnapshot {
        let mut per_domain_published = [0u64; MAX_DOMAIN_STATS];
        let mut per_domain_dropped = [0u64; MAX_DOMAIN_STATS];
        for i in 0..MAX_DOMAIN_STATS {
            per_domain_published[i] = self.per_domain_published[i].load(Ordering::Relaxed);
            per_domain_dropped[i] = self.per_domain_dropped[i].load(Ordering::Relaxed);
        }
        let mut subscriber_lag = [0u64; MAX_TRACKED_SUBSCRIBERS];
        for (i, lag) in subscriber_lag.iter_mut().enumerate() {
            *lag = self.subscriber_lag[i].load(Ordering::Relaxed);
        }
        BusStatisticsSnapshot {
            published: self.published.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
            per_domain_published,
            per_domain_dropped,
            subscriber_lag,
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&self) {
        self.published.store(0, Ordering::Relaxed);
        self.dropped.store(0, Ordering::Relaxed);
        for a in &self.per_domain_published {
            a.store(0, Ordering::Relaxed);
        }
        for a in &self.per_domain_dropped {
            a.store(0, Ordering::Relaxed);
        }
        for a in &self.subscriber_lag {
            a.store(0, Ordering::Relaxed);
        }
    }
}

impl Default for BusStatistics {
    fn default() -> Self {
        Self::new()
    }
}

/// Plain-data snapshot of [`BusStatistics`].
#[derive(Debug, Clone)]
pub struct BusStatisticsSnapshot {
    pub published: u64,
    pub dropped: u64,
    pub per_domain_published: [u64; MAX_DOMAIN_STATS],
    pub per_domain_dropped: [u64; MAX_DOMAIN_STATS],
    pub subscriber_lag: [u64; MAX_TRACKED_SUBSCRIBERS],
}

// ---------------------------------------------------------------------------
// FilteredSubscriber
// ---------------------------------------------------------------------------

/// Maximum number of filtered subscribers managed by [`FilteredSubscriberSet`].
pub const MAX_FILTERED_SUBSCRIBERS: usize = 64;

/// A subscriber destination paired with a topic filter and backpressure policy.
#[derive(Debug, Clone, Copy)]
pub struct FilteredSubscriber {
    /// Destination identifier (matches route destination IDs).
    pub destination: u32,
    /// Which event domains this subscriber is interested in.
    pub filter: TopicFilter,
    /// How to handle backpressure.
    pub policy: BackpressurePolicy,
    /// Current pending count (for backpressure tracking).
    pub pending: u32,
    /// Maximum buffer capacity.
    pub capacity: u32,
}

impl FilteredSubscriber {
    /// Create a new filtered subscriber.
    #[must_use]
    pub const fn new(destination: u32, filter: TopicFilter, policy: BackpressurePolicy, capacity: u32) -> Self {
        Self {
            destination,
            filter,
            policy,
            pending: 0,
            capacity,
        }
    }

    /// Check whether the subscriber should receive an event in the given domain.
    #[inline]
    #[must_use]
    pub const fn accepts(&self, domain: EventDomain) -> bool {
        self.filter.matches(domain)
    }

    /// Returns `true` if the subscriber's buffer is full.
    #[inline]
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.pending >= self.capacity
    }

    /// Try to enqueue an event. Returns `true` if accepted.
    ///
    /// When the buffer is full, the behaviour depends on the
    /// [`BackpressurePolicy`]:
    /// - `DropOldest`: accepts the event, conceptually evicting the oldest.
    /// - `DropNewest`: rejects the event.
    /// - `Block`: accepts the event (caller is responsible for actual blocking).
    #[inline]
    pub fn try_enqueue(&mut self, domain: EventDomain) -> EnqueueResult {
        if !self.accepts(domain) {
            return EnqueueResult::Filtered;
        }

        if !self.is_full() {
            self.pending += 1;
            return EnqueueResult::Accepted;
        }

        match self.policy {
            BackpressurePolicy::DropOldest => {
                // Pending stays the same (one out, one in).
                EnqueueResult::AcceptedDroppedOldest
            }
            BackpressurePolicy::DropNewest => EnqueueResult::DroppedNewest,
            BackpressurePolicy::Block => {
                self.pending += 1;
                EnqueueResult::Accepted
            }
        }
    }

    /// Acknowledge consumption of one event from the buffer.
    #[inline]
    pub fn ack(&mut self) {
        self.pending = self.pending.saturating_sub(1);
    }
}

/// Result of attempting to enqueue an event into a [`FilteredSubscriber`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnqueueResult {
    /// Event was accepted into the buffer.
    Accepted,
    /// Buffer was full; oldest event was dropped to make room.
    AcceptedDroppedOldest,
    /// Event was dropped because the buffer is full (`DropNewest` policy).
    DroppedNewest,
    /// Event was filtered out (domain not accepted).
    Filtered,
}

// ---------------------------------------------------------------------------
// FilteredSubscriberSet
// ---------------------------------------------------------------------------

/// Pre-allocated set of [`FilteredSubscriber`]s for the RT path.
pub struct FilteredSubscriberSet {
    subscribers: [Option<FilteredSubscriber>; MAX_FILTERED_SUBSCRIBERS],
    count: usize,
}

impl FilteredSubscriberSet {
    /// Create an empty set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            subscribers: [None; MAX_FILTERED_SUBSCRIBERS],
            count: 0,
        }
    }

    /// Add a filtered subscriber. Returns the slot index, or `None` if full.
    pub fn add(&mut self, sub: FilteredSubscriber) -> Option<usize> {
        let slot = self.subscribers.iter().position(|s| s.is_none())?;
        self.subscribers[slot] = Some(sub);
        self.count += 1;
        Some(slot)
    }

    /// Remove a subscriber by destination ID. Returns `true` if found.
    pub fn remove(&mut self, destination: u32) -> bool {
        for slot in &mut self.subscribers {
            if let Some(s) = slot
                && s.destination == destination
            {
                *slot = None;
                self.count -= 1;
                return true;
            }
        }
        false
    }

    /// Number of active subscribers.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.count
    }

    /// Returns `true` when there are no subscribers.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Dispatch an event domain to all matching subscribers, respecting
    /// backpressure policies. Returns the number of subscribers that accepted
    /// the event and the number of drops.
    ///
    /// Updates `stats` with per-domain and per-subscriber counters.
    pub fn dispatch(
        &mut self,
        domain: EventDomain,
        stats: &BusStatistics,
    ) -> DispatchSummary {
        let mut accepted = 0u32;
        let mut dropped = 0u32;

        for (i, slot) in self.subscribers.iter_mut().enumerate() {
            let sub = match slot {
                Some(s) => s,
                None => continue,
            };

            match sub.try_enqueue(domain) {
                EnqueueResult::Accepted | EnqueueResult::AcceptedDroppedOldest => {
                    accepted += 1;
                    stats.set_subscriber_lag(i, u64::from(sub.pending));
                }
                EnqueueResult::DroppedNewest => {
                    dropped += 1;
                    stats.record_drop(domain);
                    stats.set_subscriber_lag(i, u64::from(sub.pending));
                }
                EnqueueResult::Filtered => {}
            }
        }

        stats.record_publish(domain);

        DispatchSummary { accepted, dropped }
    }

    /// Get subscriber at a given slot (for inspection).
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<&FilteredSubscriber> {
        self.subscribers.get(slot).and_then(|s| s.as_ref())
    }

    /// Get a mutable reference to subscriber at a given slot.
    #[must_use]
    pub fn get_mut(&mut self, slot: usize) -> Option<&mut FilteredSubscriber> {
        self.subscribers.get_mut(slot).and_then(|s| s.as_mut())
    }
}

impl Default for FilteredSubscriberSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of a single dispatch call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchSummary {
    /// Number of subscribers that accepted the event.
    pub accepted: u32,
    /// Number of drops due to backpressure.
    pub dropped: u32,
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- EventDomain --------------------------------------------------------

    #[test]
    fn domain_mask_is_unique_per_variant() {
        let mut seen: u8 = 0;
        for d in EventDomain::ALL {
            let m = d.mask();
            assert_eq!(seen & m, 0, "duplicate mask for {d:?}");
            seen |= m;
        }
    }

    // -- TopicFilter --------------------------------------------------------

    #[test]
    fn empty_filter_matches_everything() {
        let f = TopicFilter::all();
        assert!(f.is_wildcard());
        for d in EventDomain::ALL {
            assert!(f.matches(d), "wildcard should match {d:?}");
        }
    }

    #[test]
    fn single_domain_filter_matches_only_that_domain() {
        let f = TopicFilter::single(EventDomain::Telemetry);
        assert!(f.matches(EventDomain::Telemetry));
        assert!(!f.matches(EventDomain::DeviceState));
        assert!(!f.matches(EventDomain::ProfileChange));
        assert!(!f.matches(EventDomain::SystemHealth));
        assert!(!f.matches(EventDomain::AxisData));
        assert!(!f.matches(EventDomain::FfbCommand));
    }

    #[test]
    fn filter_rejects_non_matching_events() {
        let f = TopicFilter::single(EventDomain::FfbCommand);
        assert!(!f.matches(EventDomain::Telemetry));
        assert!(!f.matches(EventDomain::DeviceState));
        assert!(!f.matches(EventDomain::AxisData));
    }

    #[test]
    fn multiple_domains_in_single_filter() {
        let f = TopicFilter::from_domains(&[
            EventDomain::Telemetry,
            EventDomain::AxisData,
            EventDomain::SystemHealth,
        ]);
        assert!(f.matches(EventDomain::Telemetry));
        assert!(f.matches(EventDomain::AxisData));
        assert!(f.matches(EventDomain::SystemHealth));
        assert!(!f.matches(EventDomain::DeviceState));
        assert!(!f.matches(EventDomain::ProfileChange));
        assert!(!f.matches(EventDomain::FfbCommand));
    }

    #[test]
    fn filter_add_and_remove() {
        let mut f = TopicFilter::single(EventDomain::Telemetry);
        f.add(EventDomain::AxisData);
        assert!(f.matches(EventDomain::Telemetry));
        assert!(f.matches(EventDomain::AxisData));

        f.remove(EventDomain::Telemetry);
        assert!(!f.matches(EventDomain::Telemetry));
        assert!(f.matches(EventDomain::AxisData));
    }

    #[test]
    fn default_filter_is_wildcard() {
        let f = TopicFilter::default();
        assert!(f.is_wildcard());
        for d in EventDomain::ALL {
            assert!(f.matches(d));
        }
    }

    #[test]
    fn none_filter_matches_nothing() {
        let f = TopicFilter::none();
        for d in EventDomain::ALL {
            assert!(!f.matches(d), "none filter should reject {d:?}");
        }
    }

    // -- BackpressurePolicy -------------------------------------------------

    #[test]
    fn default_backpressure_is_drop_newest() {
        assert_eq!(BackpressurePolicy::default(), BackpressurePolicy::DropNewest);
    }

    // -- FilteredSubscriber -------------------------------------------------

    #[test]
    fn filtered_subscriber_accepts_matching_domain() {
        let mut sub = FilteredSubscriber::new(
            1,
            TopicFilter::single(EventDomain::Telemetry),
            BackpressurePolicy::DropNewest,
            8,
        );
        assert_eq!(sub.try_enqueue(EventDomain::Telemetry), EnqueueResult::Accepted);
        assert_eq!(sub.pending, 1);
    }

    #[test]
    fn filtered_subscriber_rejects_non_matching_domain() {
        let mut sub = FilteredSubscriber::new(
            1,
            TopicFilter::single(EventDomain::Telemetry),
            BackpressurePolicy::DropNewest,
            8,
        );
        assert_eq!(sub.try_enqueue(EventDomain::FfbCommand), EnqueueResult::Filtered);
        assert_eq!(sub.pending, 0);
    }

    #[test]
    fn backpressure_drop_newest_works() {
        let mut sub = FilteredSubscriber::new(
            1,
            TopicFilter::all(),
            BackpressurePolicy::DropNewest,
            2,
        );
        assert_eq!(sub.try_enqueue(EventDomain::Telemetry), EnqueueResult::Accepted);
        assert_eq!(sub.try_enqueue(EventDomain::Telemetry), EnqueueResult::Accepted);
        // Buffer full
        assert_eq!(sub.try_enqueue(EventDomain::Telemetry), EnqueueResult::DroppedNewest);
        assert_eq!(sub.pending, 2);
    }

    #[test]
    fn backpressure_drop_oldest_works() {
        let mut sub = FilteredSubscriber::new(
            1,
            TopicFilter::all(),
            BackpressurePolicy::DropOldest,
            2,
        );
        assert_eq!(sub.try_enqueue(EventDomain::Telemetry), EnqueueResult::Accepted);
        assert_eq!(sub.try_enqueue(EventDomain::Telemetry), EnqueueResult::Accepted);
        // Buffer full — oldest is dropped
        assert_eq!(sub.try_enqueue(EventDomain::Telemetry), EnqueueResult::AcceptedDroppedOldest);
        assert_eq!(sub.pending, 2); // stayed at capacity
    }

    #[test]
    fn backpressure_block_keeps_accepting() {
        let mut sub = FilteredSubscriber::new(
            1,
            TopicFilter::all(),
            BackpressurePolicy::Block,
            2,
        );
        sub.try_enqueue(EventDomain::Telemetry);
        sub.try_enqueue(EventDomain::Telemetry);
        // Buffer "full" but Block allows overflow
        assert_eq!(sub.try_enqueue(EventDomain::Telemetry), EnqueueResult::Accepted);
        assert_eq!(sub.pending, 3);
    }

    #[test]
    fn subscriber_ack_decrements_pending() {
        let mut sub = FilteredSubscriber::new(
            1,
            TopicFilter::all(),
            BackpressurePolicy::DropNewest,
            8,
        );
        sub.try_enqueue(EventDomain::Telemetry);
        sub.try_enqueue(EventDomain::Telemetry);
        assert_eq!(sub.pending, 2);
        sub.ack();
        assert_eq!(sub.pending, 1);
        sub.ack();
        assert_eq!(sub.pending, 0);
        sub.ack(); // saturating
        assert_eq!(sub.pending, 0);
    }

    // -- BusStatistics ------------------------------------------------------

    #[test]
    fn bus_statistics_tracking() {
        let stats = BusStatistics::new();
        assert_eq!(stats.total_published(), 0);
        assert_eq!(stats.total_dropped(), 0);

        stats.record_publish(EventDomain::Telemetry);
        stats.record_publish(EventDomain::Telemetry);
        stats.record_publish(EventDomain::AxisData);
        assert_eq!(stats.total_published(), 3);
        assert_eq!(stats.domain_published(EventDomain::Telemetry), 2);
        assert_eq!(stats.domain_published(EventDomain::AxisData), 1);
        assert_eq!(stats.domain_published(EventDomain::FfbCommand), 0);

        stats.record_drop(EventDomain::Telemetry);
        assert_eq!(stats.total_dropped(), 1);
        assert_eq!(stats.domain_dropped(EventDomain::Telemetry), 1);
    }

    #[test]
    fn subscriber_lag_calculation() {
        let stats = BusStatistics::new();
        assert_eq!(stats.subscriber_lag(0), 0);

        stats.set_subscriber_lag(0, 5);
        stats.set_subscriber_lag(1, 10);
        assert_eq!(stats.subscriber_lag(0), 5);
        assert_eq!(stats.subscriber_lag(1), 10);

        // Out of range returns 0
        assert_eq!(stats.subscriber_lag(MAX_TRACKED_SUBSCRIBERS + 1), 0);
    }

    #[test]
    fn bus_statistics_snapshot() {
        let stats = BusStatistics::new();
        stats.record_publish(EventDomain::DeviceState);
        stats.record_publish(EventDomain::DeviceState);
        stats.record_drop(EventDomain::ProfileChange);
        stats.set_subscriber_lag(3, 42);

        let snap = stats.snapshot();
        assert_eq!(snap.published, 2);
        assert_eq!(snap.dropped, 1);
        assert_eq!(snap.per_domain_published[EventDomain::DeviceState as usize], 2);
        assert_eq!(snap.per_domain_dropped[EventDomain::ProfileChange as usize], 1);
        assert_eq!(snap.subscriber_lag[3], 42);
    }

    #[test]
    fn bus_statistics_reset() {
        let stats = BusStatistics::new();
        stats.record_publish(EventDomain::Telemetry);
        stats.record_drop(EventDomain::Telemetry);
        stats.set_subscriber_lag(0, 99);
        stats.reset();

        assert_eq!(stats.total_published(), 0);
        assert_eq!(stats.total_dropped(), 0);
        assert_eq!(stats.subscriber_lag(0), 0);
    }

    // -- FilteredSubscriberSet ----------------------------------------------

    #[test]
    fn subscriber_set_add_remove() {
        let mut set = FilteredSubscriberSet::new();
        assert!(set.is_empty());

        let slot = set
            .add(FilteredSubscriber::new(
                10,
                TopicFilter::all(),
                BackpressurePolicy::DropNewest,
                8,
            ))
            .unwrap();
        assert_eq!(set.len(), 1);
        assert_eq!(set.get(slot).unwrap().destination, 10);

        assert!(set.remove(10));
        assert!(set.is_empty());
        assert!(!set.remove(10)); // idempotent
    }

    #[test]
    fn filtered_subscriber_end_to_end() {
        let mut set = FilteredSubscriberSet::new();
        let stats = BusStatistics::new();

        // Sub A: interested in Telemetry only
        set.add(FilteredSubscriber::new(
            1,
            TopicFilter::single(EventDomain::Telemetry),
            BackpressurePolicy::DropNewest,
            4,
        ));
        // Sub B: interested in everything
        set.add(FilteredSubscriber::new(
            2,
            TopicFilter::all(),
            BackpressurePolicy::DropOldest,
            4,
        ));

        // Dispatch a Telemetry event — both should accept
        let s = set.dispatch(EventDomain::Telemetry, &stats);
        assert_eq!(s.accepted, 2);
        assert_eq!(s.dropped, 0);

        // Dispatch a DeviceState event — only Sub B should accept
        let s = set.dispatch(EventDomain::DeviceState, &stats);
        assert_eq!(s.accepted, 1);
        assert_eq!(s.dropped, 0);

        // Verify statistics
        assert_eq!(stats.total_published(), 2);
        assert_eq!(stats.domain_published(EventDomain::Telemetry), 1);
        assert_eq!(stats.domain_published(EventDomain::DeviceState), 1);
    }

    #[test]
    fn dispatch_with_backpressure_drop_newest() {
        let mut set = FilteredSubscriberSet::new();
        let stats = BusStatistics::new();

        set.add(FilteredSubscriber::new(
            1,
            TopicFilter::all(),
            BackpressurePolicy::DropNewest,
            2,
        ));

        // Fill the buffer
        set.dispatch(EventDomain::Telemetry, &stats);
        set.dispatch(EventDomain::Telemetry, &stats);
        // Third should be dropped
        let s = set.dispatch(EventDomain::Telemetry, &stats);
        assert_eq!(s.dropped, 1);
        assert_eq!(stats.total_dropped(), 1);
    }

    #[test]
    fn dispatch_with_backpressure_drop_oldest() {
        let mut set = FilteredSubscriberSet::new();
        let stats = BusStatistics::new();

        set.add(FilteredSubscriber::new(
            1,
            TopicFilter::all(),
            BackpressurePolicy::DropOldest,
            2,
        ));

        set.dispatch(EventDomain::Telemetry, &stats);
        set.dispatch(EventDomain::Telemetry, &stats);
        // Third accepted (oldest dropped)
        let s = set.dispatch(EventDomain::Telemetry, &stats);
        assert_eq!(s.accepted, 1);
        assert_eq!(s.dropped, 0); // not counted as a "drop" stat; it's a replacement
    }

    #[test]
    fn dispatch_updates_subscriber_lag_in_stats() {
        let mut set = FilteredSubscriberSet::new();
        let stats = BusStatistics::new();

        let slot = set
            .add(FilteredSubscriber::new(
                1,
                TopicFilter::all(),
                BackpressurePolicy::DropNewest,
                8,
            ))
            .unwrap();

        set.dispatch(EventDomain::Telemetry, &stats);
        set.dispatch(EventDomain::AxisData, &stats);
        assert_eq!(stats.subscriber_lag(slot), 2);

        // Ack one
        set.get_mut(slot).unwrap().ack();
        stats.set_subscriber_lag(slot, u64::from(set.get(slot).unwrap().pending));
        assert_eq!(stats.subscriber_lag(slot), 1);
    }
}
