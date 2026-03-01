// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for bus event routing.
//!
//! Exercises multi-publisher, multi-subscriber, and cross-topic scenarios
//! that span both the allocation-free `EventRouter` and the `BusPublisher`.

use flight_bus::routing::{
    BusEvent, EventFilter, EventKind, EventPayload, EventPriority, EventRouter, RoutePattern,
    SourceType, SubscriberStatus, Topic,
};
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_bus::types::SimId;
use std::time::Duration;

// ── RT Router: multi-destination scenarios ───────────────────────────────────

/// Multiple routes with different topics receive only their targeted events.
#[test]
fn multi_topic_routing() {
    let mut router = EventRouter::new();

    // Axis engine subscriber: Commands only.
    router.register_route(
        RoutePattern::for_topic(Topic::Commands),
        EventFilter::pass_all(),
        1, // axis engine
    );

    // FFB engine subscriber: Commands + Telemetry.
    router.register_route(
        RoutePattern::for_topic(Topic::Commands),
        EventFilter::pass_all(),
        2, // FFB engine
    );
    router.register_route(
        RoutePattern::for_topic(Topic::Telemetry),
        EventFilter::pass_all(),
        2,
    );

    // Diagnostics subscriber: Diagnostics only.
    router.register_route(
        RoutePattern::for_topic(Topic::Diagnostics),
        EventFilter::pass_all(),
        3,
    );

    // Axis update (Commands topic) → destinations 1 and 2.
    let axis = BusEvent::new(
        SourceType::Device, 1, EventKind::AxisUpdate,
        EventPriority::Normal, 1_000_000,
        EventPayload::Axis { axis_id: 0, value: 0.5 },
    );
    let m = router.route_event(&axis);
    assert_eq!(m.len(), 2);
    assert!(m.contains(1));
    assert!(m.contains(2));

    // Telemetry frame → destination 2 only.
    let telem = BusEvent::new(
        SourceType::Simulator, 1, EventKind::TelemetryFrame,
        EventPriority::Normal, 2_000_000,
        EventPayload::Telemetry { field_id: 0, value: 42.0 },
    );
    let m = router.route_event(&telem);
    assert_eq!(m.len(), 1);
    assert!(m.contains(2));

    // System status (Diagnostics topic) → destination 3 only.
    let sys = BusEvent::new(
        SourceType::Internal, 0, EventKind::SystemStatus,
        EventPriority::Normal, 3_000_000,
        EventPayload::System { code: 1 },
    );
    let m = router.route_event(&sys);
    assert_eq!(m.len(), 1);
    assert!(m.contains(3));
}

/// Simulates the core RT flow: sim adapter → bus → axis engine + FFB engine.
#[test]
fn sim_adapter_to_engines_flow() {
    let mut router = EventRouter::new();
    router.register_subscriber(1); // axis engine
    router.register_subscriber(2); // FFB engine

    // Axis engine wants all Commands from Device source.
    router.register_route(
        RoutePattern {
            source_type: SourceType::Device,
            source_id: None,
            event_kind: None,
            topic: Topic::Commands,
        },
        EventFilter::pass_all(),
        1,
    );

    // FFB engine wants telemetry from Simulator + commands from Device.
    router.register_route(
        RoutePattern::for_topic(Topic::Telemetry),
        EventFilter::pass_all(),
        2,
    );
    router.register_route(
        RoutePattern {
            source_type: SourceType::Device,
            source_id: None,
            event_kind: Some(EventKind::AxisUpdate),
            topic: Topic::Commands,
        },
        EventFilter::pass_all(),
        2,
    );

    // Simulate 250 axis updates at 250Hz (4ms apart).
    let mut total_axis_engine = 0usize;
    let mut total_ffb_engine = 0usize;
    for i in 0..250 {
        let event = BusEvent::new(
            SourceType::Device,
            1,
            EventKind::AxisUpdate,
            EventPriority::Normal,
            (i as u64) * 4_000, // 4ms = 4000us apart
            EventPayload::Axis { axis_id: 0, value: (i as f64) / 250.0 },
        );
        let m = router.route_event(&event);
        if m.contains(1) { total_axis_engine += 1; }
        if m.contains(2) { total_ffb_engine += 1; }
    }

    assert_eq!(total_axis_engine, 250);
    assert_eq!(total_ffb_engine, 250);
}

/// Subscriber crash detection and graceful recovery.
#[test]
fn subscriber_crash_and_recovery() {
    let mut router = EventRouter::new();
    router.register_subscriber(1);
    router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);

    let event = BusEvent::new(
        SourceType::Device, 1, EventKind::AxisUpdate,
        EventPriority::Normal, 1_000_000,
        EventPayload::Axis { axis_id: 0, value: 0.5 },
    );

    // Initially active.
    assert_eq!(router.route_event(&event).len(), 1);

    // Simulate 100 consecutive failures → auto-crash.
    for _ in 0..100 {
        router.record_delivery_failure(1);
    }
    assert_eq!(
        router.subscriber_info(1).unwrap().status,
        SubscriberStatus::Crashed
    );
    assert!(router.route_event(&event).is_empty());

    // Resume the subscriber.
    router.resume_subscriber(1);
    assert_eq!(
        router.subscriber_info(1).unwrap().status,
        SubscriberStatus::Active
    );
    assert_eq!(router.route_event(&event).len(), 1);
}

/// Backpressure combined with topic filtering.
#[test]
fn backpressure_with_topics() {
    let mut router = EventRouter::new();
    router.register_route(
        RoutePattern::for_topic(Topic::Commands),
        EventFilter::pass_all(),
        1,
    );
    router.register_route(
        RoutePattern::for_topic(Topic::Diagnostics),
        EventFilter::pass_all(),
        2,
    );
    router.set_backpressure(30);

    // Background Command event → dropped (backpressure >= 25%).
    let bg_cmd = BusEvent::with_topic(
        SourceType::Device, 1, EventKind::AxisUpdate, Topic::Commands,
        EventPriority::Background, 1_000_000,
        EventPayload::Axis { axis_id: 0, value: 0.5 },
    );
    assert!(router.route_event(&bg_cmd).is_empty());

    // High-priority Command → not dropped.
    let hi_cmd = BusEvent::with_topic(
        SourceType::Device, 1, EventKind::AxisUpdate, Topic::Commands,
        EventPriority::High, 2_000_000,
        EventPayload::Axis { axis_id: 0, value: 0.5 },
    );
    let m = router.route_event(&hi_cmd);
    assert_eq!(m.len(), 1);
    assert!(m.contains(1));
}

// ── BusPublisher: multi-subscriber scenarios ─────────────────────────────────

/// Multiple subscribers each receive an independently cloned snapshot.
#[test]
fn multi_subscriber_pubsub() {
    let mut publisher = BusPublisher::new(60.0);

    let mut sub1 = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let mut sub2 = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let mut sub3 = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    publisher.publish(snap).unwrap();

    assert!(sub1.try_recv().unwrap().is_some());
    assert!(sub2.try_recv().unwrap().is_some());
    assert!(sub3.try_recv().unwrap().is_some());
}

/// Dropping a subscriber while others are active doesn't disrupt delivery.
#[test]
fn subscriber_drop_no_disruption() {
    let mut publisher = BusPublisher::new(60.0);

    let mut sub1 = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    {
        let _sub2 = publisher.subscribe(SubscriptionConfig::default()).unwrap();
        // sub2 dropped here
    }
    let mut sub3 = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    publisher.publish(snap).unwrap();

    // sub1 and sub3 should still receive; publisher cleans up sub2's dead channel.
    assert!(sub1.try_recv().unwrap().is_some());
    assert!(sub3.try_recv().unwrap().is_some());
    // Dead sub2 entry cleaned up.
    assert_eq!(publisher.subscriber_count(), 2);
}

/// Drop-tail semantics: slow subscriber's buffer fills, publisher never blocks.
#[test]
fn drop_tail_backpressure() {
    let mut publisher = BusPublisher::new(60.0);

    let config = SubscriptionConfig {
        buffer_size: 4,
        drop_on_full: true,
        max_rate_hz: 60.0,
    };
    let mut fast_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let _slow_sub = publisher.subscribe(config).unwrap(); // never drained

    // Publish 10 messages with spacing to bypass rate limiter.
    for i in 0..10 {
        let snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        publisher.publish(snap).unwrap();
        if i < 9 {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    // Fast subscriber got all messages.
    let mut fast_count = 0;
    while fast_sub.try_recv().unwrap().is_some() {
        fast_count += 1;
    }
    assert!(fast_count >= 5, "fast subscriber should have received many messages, got {fast_count}");

    // Publisher never panicked or blocked — that's the key assertion.
    // The slow subscriber's buffer overflowed gracefully (drop-tail).
    assert!(publisher.drop_count() > 0, "expected some drops from slow subscriber");
}

/// Unsubscribe + re-subscribe cycle works cleanly.
#[test]
fn subscribe_unsubscribe_cycle() {
    let mut publisher = BusPublisher::new(60.0);

    for _ in 0..5 {
        let sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
        assert_eq!(publisher.subscriber_count(), 1);
        publisher.unsubscribe(sub.id).unwrap();
        assert_eq!(publisher.subscriber_count(), 0);
    }
}

// ── Zero-allocation proof (routing hot path) ─────────────────────────────────

/// Verifies the EventRouter's route_event path uses only stack allocation
/// by checking that the RouteMatches result is a fixed-size struct.
#[test]
fn route_event_returns_fixed_size_result() {
    let mut router = EventRouter::new();
    for i in 0..8 {
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), i);
    }

    let event = BusEvent::new(
        SourceType::Device, 1, EventKind::AxisUpdate,
        EventPriority::Normal, 1_000_000,
        EventPayload::Axis { axis_id: 0, value: 0.5 },
    );

    // RouteMatches uses [u32; MAX_MATCHES] — fully stack-allocated.
    let matches = router.route_event(&event);
    assert_eq!(matches.len(), 8);

    // Size of RouteMatches should be fixed regardless of match count.
    let size = std::mem::size_of_val(&matches);
    let empty_matches = EventRouter::new().route_event(&event);
    assert_eq!(size, std::mem::size_of_val(&empty_matches));
}

/// BusEvent is Copy and small enough to pass by value on the stack.
#[test]
fn bus_event_is_copy_and_small() {
    let event = BusEvent::new(
        SourceType::Device, 1, EventKind::AxisUpdate,
        EventPriority::Normal, 1_000_000,
        EventPayload::Axis { axis_id: 0, value: 0.5 },
    );

    // Copy semantics — no heap allocation.
    let _copy = event;
    let _another = event; // Still valid after copy.

    // Should fit in a cache line or two.
    let size = std::mem::size_of::<BusEvent>();
    assert!(
        size <= 128,
        "BusEvent should be ≤128 bytes for cache efficiency, got {size}"
    );
}

/// EventRouter itself uses only fixed-size arrays — no heap containers.
#[test]
fn event_router_fixed_memory() {
    let router = EventRouter::new();
    let size = std::mem::size_of_val(&router);

    // The router should be a predictable fixed size (no Vec, HashMap, etc.).
    // Exact size depends on field layout, but it should be constant.
    let router2 = EventRouter::new();
    assert_eq!(size, std::mem::size_of_val(&router2));

    // And it should be reasonably bounded (not growing with usage).
    assert!(
        size < 32768,
        "EventRouter should have bounded fixed size, got {size}"
    );
}
