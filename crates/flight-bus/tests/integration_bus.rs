// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for bus event routing and pub/sub flow.
//!
//! Exercises: publisher → bus → subscriber, topic filtering,
//! multiple subscribers, and backpressure behaviour.

use flight_bus::routing::{
    BusEvent, EventFilter, EventKind, EventPayload, EventPriority, EventRouter, RoutePattern,
    SourceType, Topic,
};
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};

// ── helpers ────────────────────────────────────────────────────────────────

fn make_publisher() -> BusPublisher {
    BusPublisher::new(60.0)
}

fn valid_snapshot() -> BusSnapshot {
    BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"))
}

fn axis_event(source_id: u32, axis_id: u16, value: f64) -> BusEvent {
    BusEvent::new(
        SourceType::Device,
        source_id,
        EventKind::AxisUpdate,
        EventPriority::Normal,
        1_000_000,
        EventPayload::Axis { axis_id, value },
    )
}

fn button_event(source_id: u32, button_id: u16, pressed: bool) -> BusEvent {
    BusEvent::new(
        SourceType::Device,
        source_id,
        EventKind::ButtonPress,
        EventPriority::Normal,
        1_000_000,
        EventPayload::Button { button_id, pressed },
    )
}

// ── 1. Publisher → subscriber receives event ─────────────────────────────

#[test]
fn integration_publish_and_receive() {
    let mut pub_ = make_publisher();
    let mut sub = pub_.subscribe(SubscriptionConfig::default()).unwrap();

    pub_.publish(valid_snapshot()).unwrap();

    let received = sub.try_recv().expect("channel ok");
    assert!(received.is_some(), "subscriber should receive snapshot");
}

// ── 2. Topic filtering: route matches only axis events ───────────────────

#[test]
fn integration_route_matches_axis_events_only() {
    let mut router = EventRouter::new();
    let pattern = RoutePattern {
        source_type: SourceType::Any,
        source_id: None,
        event_kind: Some(EventKind::AxisUpdate),
        topic: Topic::Any,
    };
    router.register_route(pattern, EventFilter::pass_all(), 1);

    let axis = axis_event(10, 0, 0.5);
    let btn = button_event(10, 1, true);

    let axis_matches = router.route_event(&axis);
    let btn_matches = router.route_event(&btn);

    assert_eq!(axis_matches.len(), 1, "axis event should match");
    assert_eq!(
        btn_matches.len(),
        0,
        "button event should not match axis route"
    );
}

// ── 3. Multiple subscribers get same snapshot ────────────────────────────

#[test]
fn integration_multiple_subscribers_receive() {
    let mut pub_ = make_publisher();
    let mut sub1 = pub_.subscribe(SubscriptionConfig::default()).unwrap();
    let mut sub2 = pub_.subscribe(SubscriptionConfig::default()).unwrap();

    pub_.publish(valid_snapshot()).unwrap();

    let r1 = sub1.try_recv().expect("ok").expect("should have data");
    let r2 = sub2.try_recv().expect("ok").expect("should have data");

    assert_eq!(r1.sim, SimId::Msfs);
    assert_eq!(r2.sim, SimId::Msfs);
}

// ── 4. Stale subscriber doesn't block publisher ──────────────────────────

#[test]
fn integration_dropped_subscriber_does_not_block() {
    let mut pub_ = make_publisher();
    let sub = pub_.subscribe(SubscriptionConfig::default()).unwrap();
    drop(sub); // subscriber disconnects

    // Publishing should still succeed (dropped subscriber cleaned up).
    let result = pub_.publish(valid_snapshot());
    assert!(
        result.is_ok(),
        "publish should succeed after subscriber drops"
    );
}

// ── 5. Route pattern with source_id filter ───────────────────────────────

#[test]
fn integration_route_source_id_filter() {
    let mut router = EventRouter::new();
    let pattern = RoutePattern {
        source_type: SourceType::Device,
        source_id: Some(42),
        event_kind: None,
        topic: Topic::Any,
    };
    router.register_route(pattern, EventFilter::pass_all(), 100);

    let from_42 = axis_event(42, 0, 0.5);
    let from_99 = axis_event(99, 0, 0.5);

    assert_eq!(
        router.route_event(&from_42).len(),
        1,
        "source 42 should match"
    );
    assert_eq!(
        router.route_event(&from_99).len(),
        0,
        "source 99 should not match"
    );
}

// ── 6. Multiple routes deliver to multiple destinations ──────────────────

#[test]
fn integration_multiple_routes_multiple_destinations() {
    let mut router = EventRouter::new();
    router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);
    router.register_route(RoutePattern::any(), EventFilter::pass_all(), 2);
    router.register_route(RoutePattern::any(), EventFilter::pass_all(), 3);

    let event = axis_event(1, 0, 0.5);
    let matches = router.route_event(&event);

    assert_eq!(matches.len(), 3, "should match all 3 routes");
    assert!(matches.contains(1));
    assert!(matches.contains(2));
    assert!(matches.contains(3));
}

// ── 7. Route removal stops matching ──────────────────────────────────────

#[test]
fn integration_route_removal() {
    let mut router = EventRouter::new();
    let id = router
        .register_route(RoutePattern::any(), EventFilter::pass_all(), 10)
        .expect("should register");

    let event = axis_event(1, 0, 0.5);
    assert_eq!(router.route_event(&event).len(), 1);

    assert!(router.remove_route(id));
    assert_eq!(
        router.route_event(&event).len(),
        0,
        "route should be removed"
    );
}

// ── 8. Backpressure drops low-priority events ────────────────────────────

#[test]
fn integration_backpressure_drops_low_priority() {
    let mut router = EventRouter::new();
    router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);

    // Under 50% backpressure, Background and Low priority events should be dropped.
    router.set_backpressure(50);

    let bg_event = BusEvent::new(
        SourceType::Device,
        1,
        EventKind::AxisUpdate,
        EventPriority::Background,
        1_000_000,
        EventPayload::Axis {
            axis_id: 0,
            value: 0.5,
        },
    );
    let critical_event = BusEvent::new(
        SourceType::Device,
        1,
        EventKind::AxisUpdate,
        EventPriority::Critical,
        1_000_000,
        EventPayload::Axis {
            axis_id: 0,
            value: 0.5,
        },
    );

    let bg_matches = router.route_event(&bg_event);
    let crit_matches = router.route_event(&critical_event);

    assert_eq!(
        bg_matches.len(),
        0,
        "background should be dropped under backpressure"
    );
    assert_eq!(
        crit_matches.len(),
        1,
        "critical should pass through backpressure"
    );
}

// ── 9. Value range filter on axis events ─────────────────────────────────

#[test]
fn integration_value_range_filter() {
    let mut router = EventRouter::new();
    let filter = EventFilter {
        min_value: Some(0.3),
        max_value: Some(0.8),
        changed_only: false,
        debounce_ms: 0,
        rate_limit_hz: None,
    };
    router.register_route(RoutePattern::any(), filter, 1);

    let in_range = axis_event(1, 0, 0.5);
    let below = axis_event(1, 0, 0.1);
    let above = axis_event(1, 0, 0.95);

    assert_eq!(router.route_event(&in_range).len(), 1, "0.5 in [0.3, 0.8]");
    assert_eq!(router.route_event(&below).len(), 0, "0.1 below 0.3");
    assert_eq!(router.route_event(&above).len(), 0, "0.95 above 0.8");
}

// ── 10. Empty router has zero routes ─────────────────────────────────────

#[test]
fn integration_empty_router() {
    let mut router = EventRouter::new();
    assert_eq!(router.route_count(), 0);

    let event = axis_event(1, 0, 0.5);
    let matches = router.route_event(&event);
    assert!(matches.is_empty());
}
