// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Event correlation for tracing causal chains across subsystems.
//!
//! A [`CorrelationId`] is a unique opaque identifier that links related events
//! together (e.g. device input → axis processing → sim output).
//!
//! [`EventChain`] collects a chronologically ordered sequence of events that
//! share the same correlation ID, enabling end-to-end latency analysis.

use crate::structured::FlightEvent;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

// ── CorrelationId ─────────────────────────────────────────────────────────

/// Unique identifier linking related events across subsystems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(u64);

impl CorrelationId {
    /// Generate a new, globally unique correlation ID.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Create a correlation ID from a raw value (for deserialization / testing).
    pub fn from_raw(value: u64) -> Self {
        Self(value)
    }

    /// Return the raw numeric value.
    pub fn as_raw(self) -> u64 {
        self.0
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "corr-{:016x}", self.0)
    }
}

// ── CorrelatedEvent ───────────────────────────────────────────────────────

/// A [`FlightEvent`] annotated with a correlation ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelatedEvent {
    pub correlation_id: CorrelationId,
    pub event: FlightEvent,
}

impl CorrelatedEvent {
    /// Wrap an existing event with a correlation ID.
    pub fn new(correlation_id: CorrelationId, event: FlightEvent) -> Self {
        Self {
            correlation_id,
            event,
        }
    }
}

// ── EventChain ────────────────────────────────────────────────────────────

/// An ordered sequence of events sharing the same [`CorrelationId`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventChain {
    pub correlation_id: CorrelationId,
    events: Vec<FlightEvent>,
}

impl EventChain {
    /// Start a new chain with the given ID.
    pub fn new(correlation_id: CorrelationId) -> Self {
        Self {
            correlation_id,
            events: Vec::new(),
        }
    }

    /// Append an event to the chain.
    pub fn push(&mut self, event: FlightEvent) {
        self.events.push(event);
    }

    /// Number of events in the chain.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the chain has no events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Iterate over events in insertion order.
    pub fn events(&self) -> &[FlightEvent] {
        &self.events
    }

    /// Total end-to-end duration of the chain in nanoseconds.
    ///
    /// Returns `None` if the chain has fewer than two events.
    pub fn duration_ns(&self) -> Option<u64> {
        if self.events.len() < 2 {
            return None;
        }
        let first = self.events.first().unwrap().timestamp_ns;
        let last = self.events.last().unwrap().timestamp_ns;
        Some(last.saturating_sub(first))
    }
}

// ── ChainCollector ────────────────────────────────────────────────────────

/// Thread-safe collector that groups correlated events into chains.
pub struct ChainCollector {
    inner: Mutex<CollectorInner>,
}

struct CollectorInner {
    chains: HashMap<CorrelationId, EventChain>,
    max_chains: usize,
}

impl ChainCollector {
    /// Create a collector that retains up to `max_chains` concurrent chains.
    pub fn new(max_chains: usize) -> Self {
        Self {
            inner: Mutex::new(CollectorInner {
                chains: HashMap::new(),
                max_chains,
            }),
        }
    }

    /// Record a correlated event.  A new chain is created automatically if
    /// this is the first event for the given correlation ID.
    pub fn record(&self, correlated: CorrelatedEvent) {
        let mut inner = self.inner.lock();
        // Evict oldest chain if at capacity and this is a new ID
        if !inner.chains.contains_key(&correlated.correlation_id)
            && inner.chains.len() >= inner.max_chains
        {
            // Remove the chain with the smallest (oldest) correlation ID.
            if let Some(&oldest) = inner.chains.keys().min_by_key(|id| id.as_raw()) {
                inner.chains.remove(&oldest);
            }
        }
        inner
            .chains
            .entry(correlated.correlation_id)
            .or_insert_with(|| EventChain::new(correlated.correlation_id))
            .push(correlated.event);
    }

    /// Retrieve the chain for a given correlation ID (if it exists).
    pub fn get_chain(&self, id: &CorrelationId) -> Option<EventChain> {
        let inner = self.inner.lock();
        inner.chains.get(id).cloned()
    }

    /// Remove and return a completed chain.
    pub fn take_chain(&self, id: &CorrelationId) -> Option<EventChain> {
        let mut inner = self.inner.lock();
        inner.chains.remove(id)
    }

    /// Number of active chains.
    pub fn active_chains(&self) -> usize {
        self.inner.lock().chains.len()
    }

    /// Remove all chains.
    pub fn clear(&self) {
        self.inner.lock().chains.clear();
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structured::{EventBuilder, EventLevel};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_ns() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    fn make_event(component: &str, message: &str) -> FlightEvent {
        FlightEvent {
            timestamp_ns: now_ns(),
            level: EventLevel::Info,
            component: component.to_owned(),
            message: message.to_owned(),
            context: Default::default(),
        }
    }

    // -- CorrelationId --

    #[test]
    fn correlation_ids_are_unique() {
        let a = CorrelationId::new();
        let b = CorrelationId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn correlation_id_from_raw_round_trips() {
        let id = CorrelationId::from_raw(42);
        assert_eq!(id.as_raw(), 42);
    }

    #[test]
    fn correlation_id_display() {
        let id = CorrelationId::from_raw(255);
        let s = id.to_string();
        assert!(s.starts_with("corr-"), "display should start with 'corr-'");
        assert!(s.contains("ff"), "should contain hex representation");
    }

    #[test]
    fn correlation_id_serialization() {
        let id = CorrelationId::new();
        let json = serde_json::to_string(&id).unwrap();
        let recovered: CorrelationId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, recovered);
    }

    // -- EventChain --

    #[test]
    fn chain_starts_empty() {
        let chain = EventChain::new(CorrelationId::new());
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
        assert!(chain.duration_ns().is_none());
    }

    #[test]
    fn chain_push_and_len() {
        let mut chain = EventChain::new(CorrelationId::new());
        chain.push(make_event("hid", "read"));
        chain.push(make_event("axis", "process"));
        chain.push(make_event("sim", "output"));
        assert_eq!(chain.len(), 3);
        assert_eq!(chain.events()[0].component, "hid");
        assert_eq!(chain.events()[2].component, "sim");
    }

    #[test]
    fn chain_duration_with_explicit_timestamps() {
        let id = CorrelationId::new();
        let mut chain = EventChain::new(id);
        chain.push(FlightEvent {
            timestamp_ns: 1_000_000,
            level: EventLevel::Info,
            component: "hid".into(),
            message: "start".into(),
            context: Default::default(),
        });
        chain.push(FlightEvent {
            timestamp_ns: 3_500_000,
            level: EventLevel::Info,
            component: "sim".into(),
            message: "end".into(),
            context: Default::default(),
        });
        assert_eq!(chain.duration_ns(), Some(2_500_000));
    }

    #[test]
    fn chain_duration_single_event_is_none() {
        let mut chain = EventChain::new(CorrelationId::new());
        chain.push(make_event("x", "y"));
        assert!(chain.duration_ns().is_none());
    }

    // -- CorrelatedEvent --

    #[test]
    fn correlated_event_wraps_flight_event() {
        let id = CorrelationId::new();
        let ev = EventBuilder::new(EventLevel::Debug, "hid", "read complete")
            .device_id("js-0")
            .build();
        let ce = CorrelatedEvent::new(id, ev);
        assert_eq!(ce.correlation_id, id);
        assert_eq!(ce.event.component, "hid");
    }

    // -- ChainCollector --

    #[test]
    fn collector_groups_by_correlation_id() {
        let collector = ChainCollector::new(100);
        let id_a = CorrelationId::new();
        let id_b = CorrelationId::new();

        collector.record(CorrelatedEvent::new(id_a, make_event("hid", "read")));
        collector.record(CorrelatedEvent::new(id_b, make_event("hid", "read 2")));
        collector.record(CorrelatedEvent::new(id_a, make_event("axis", "process")));

        assert_eq!(collector.active_chains(), 2);
        let chain_a = collector.get_chain(&id_a).unwrap();
        assert_eq!(chain_a.len(), 2);
        let chain_b = collector.get_chain(&id_b).unwrap();
        assert_eq!(chain_b.len(), 1);
    }

    #[test]
    fn collector_take_removes_chain() {
        let collector = ChainCollector::new(100);
        let id = CorrelationId::new();
        collector.record(CorrelatedEvent::new(id, make_event("hid", "read")));

        let chain = collector.take_chain(&id);
        assert!(chain.is_some());
        assert_eq!(chain.unwrap().len(), 1);
        assert!(collector.get_chain(&id).is_none());
        assert_eq!(collector.active_chains(), 0);
    }

    #[test]
    fn collector_evicts_oldest_at_capacity() {
        let collector = ChainCollector::new(2);
        let id1 = CorrelationId::from_raw(10);
        let id2 = CorrelationId::from_raw(20);
        let id3 = CorrelationId::from_raw(30);

        collector.record(CorrelatedEvent::new(id1, make_event("a", "1")));
        collector.record(CorrelatedEvent::new(id2, make_event("b", "2")));
        // Adding id3 should evict id1 (smallest raw value)
        collector.record(CorrelatedEvent::new(id3, make_event("c", "3")));

        assert_eq!(collector.active_chains(), 2);
        assert!(
            collector.get_chain(&id1).is_none(),
            "oldest should be evicted"
        );
        assert!(collector.get_chain(&id2).is_some());
        assert!(collector.get_chain(&id3).is_some());
    }

    #[test]
    fn collector_clear() {
        let collector = ChainCollector::new(100);
        let id = CorrelationId::new();
        collector.record(CorrelatedEvent::new(id, make_event("x", "y")));
        collector.clear();
        assert_eq!(collector.active_chains(), 0);
    }

    #[test]
    fn end_to_end_pipeline_trace() {
        let collector = ChainCollector::new(100);
        let id = CorrelationId::new();

        // Simulate: device input → axis processing → sim output
        let input = FlightEvent {
            timestamp_ns: 1_000_000,
            level: EventLevel::Info,
            component: "hid".into(),
            message: "joystick input".into(),
            context: Default::default(),
        };
        let process = FlightEvent {
            timestamp_ns: 1_200_000,
            level: EventLevel::Debug,
            component: "axis".into(),
            message: "curve applied".into(),
            context: Default::default(),
        };
        let output = FlightEvent {
            timestamp_ns: 1_800_000,
            level: EventLevel::Info,
            component: "simconnect".into(),
            message: "value sent".into(),
            context: Default::default(),
        };

        collector.record(CorrelatedEvent::new(id, input));
        collector.record(CorrelatedEvent::new(id, process));
        collector.record(CorrelatedEvent::new(id, output));

        let chain = collector.take_chain(&id).unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain.duration_ns(), Some(800_000)); // 1.8ms - 1.0ms
        assert_eq!(chain.events()[0].component, "hid");
        assert_eq!(chain.events()[1].component, "axis");
        assert_eq!(chain.events()[2].component, "simconnect");
    }

    #[test]
    fn chain_serialization_round_trip() {
        let mut chain = EventChain::new(CorrelationId::from_raw(999));
        chain.push(make_event("a", "first"));
        chain.push(make_event("b", "second"));

        let json = serde_json::to_string(&chain).unwrap();
        let recovered: EventChain = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.correlation_id, chain.correlation_id);
        assert_eq!(recovered.len(), 2);
    }
}
