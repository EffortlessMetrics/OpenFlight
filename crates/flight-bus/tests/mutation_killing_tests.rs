// SPDX-License-Identifier: MIT OR Apache-2.0
// Mutation-killing tests for the allocation-free event router in routing.rs.
// Each test asserts specific values to catch operator/boundary mutations.

use flight_bus::routing::{
    BusEvent, EventFilter, EventKind, EventPayload, EventPriority, EventRouter, RoutePattern,
    SourceType,
};

/// Helper: create an AxisUpdate event from a Device source.
fn device_axis_event(source_id: u32, value: f64, timestamp_us: u64) -> BusEvent {
    BusEvent::new(
        SourceType::Device,
        source_id,
        EventKind::AxisUpdate,
        EventPriority::Normal,
        timestamp_us,
        EventPayload::Axis { axis_id: 0, value },
    )
}

/// Helper: create a ButtonPress event from a Device source.
fn device_button_event(source_id: u32, pressed: bool, timestamp_us: u64) -> BusEvent {
    BusEvent::new(
        SourceType::Device,
        source_id,
        EventKind::ButtonPress,
        EventPriority::Normal,
        timestamp_us,
        EventPayload::Button {
            button_id: 1,
            pressed,
        },
    )
}

// ---------------------------------------------------------------------------
// 1. Subscriber receives published message — value assertion
// ---------------------------------------------------------------------------
#[test]
fn subscriber_receives_published_message_value() {
    let mut router = EventRouter::new();

    let pattern = RoutePattern {
        source_type: SourceType::Device,
        source_id: None,
        event_kind: Some(EventKind::AxisUpdate),
    };
    router.register_route(pattern, EventFilter::pass_all(), 42);

    // Matching event: Device + AxisUpdate → destination 42
    let event = device_axis_event(1, 0.5, 1000);
    let matches = router.route_event(&event);
    assert_eq!(matches.len(), 1, "exactly one route should match");
    assert!(matches.contains(42), "destination 42 must be present");

    // Non-matching event: Simulator source should NOT match Device pattern
    let sim_event = BusEvent::new(
        SourceType::Simulator,
        1,
        EventKind::AxisUpdate,
        EventPriority::Normal,
        2000,
        EventPayload::Axis {
            axis_id: 0,
            value: 0.5,
        },
    );
    let matches2 = router.route_event(&sim_event);
    assert_eq!(matches2.len(), 0, "simulator event must not match device route");
}

// ---------------------------------------------------------------------------
// 2. Message ordering preserved in route matches
// ---------------------------------------------------------------------------
#[test]
fn message_ordering_preserved() {
    let mut router = EventRouter::new();

    let pattern = RoutePattern {
        source_type: SourceType::Device,
        source_id: None,
        event_kind: Some(EventKind::AxisUpdate),
    };

    // Register destination 10 first, then 20
    router.register_route(pattern, EventFilter::pass_all(), 10);
    router.register_route(pattern, EventFilter::pass_all(), 20);

    let event = device_axis_event(1, 0.75, 1000);
    let matches = router.route_event(&event);

    assert_eq!(matches.len(), 2, "both routes must match");
    assert_eq!(matches.get(0), Some(10), "first match must be destination 10");
    assert_eq!(matches.get(1), Some(20), "second match must be destination 20");
}

// ---------------------------------------------------------------------------
// 3. Rate-limited route drops excess events
// ---------------------------------------------------------------------------
#[test]
fn queue_full_drops_not_silently_lost() {
    let mut router = EventRouter::new();

    let pattern = RoutePattern {
        source_type: SourceType::Device,
        source_id: None,
        event_kind: Some(EventKind::AxisUpdate),
    };
    let filter = EventFilter {
        rate_limit_hz: Some(2.0),
        ..EventFilter::pass_all()
    };
    router.register_route(pattern, filter, 99);

    // All three events share the same 1-second window (same timestamp base).
    // rate_limit_hz = 2.0 → budget of 2 events per window.
    let ts = 1_000_000; // 1 second mark

    // Event 1: starts new window, events_in_window becomes 1 → passes
    let e1 = device_axis_event(1, 0.1, ts);
    let m1 = router.route_event(&e1);
    assert_eq!(m1.len(), 1, "first event within budget must match");

    // Event 2: events_in_window becomes 2 → still < 2.0 is false, but count was 1 < 2.0 → passes
    let e2 = device_axis_event(1, 0.2, ts + 100);
    let m2 = router.route_event(&e2);
    assert_eq!(m2.len(), 1, "second event within budget must match");

    // Event 3: events_in_window is 2, 2 < 2.0 is false → rejected
    let e3 = device_axis_event(1, 0.3, ts + 200);
    let m3 = router.route_event(&e3);
    assert_eq!(m3.len(), 0, "third event exceeds rate limit and must be dropped");
}

// ---------------------------------------------------------------------------
// 4. Unsubscribe stops delivery
// ---------------------------------------------------------------------------
#[test]
fn unsubscribe_stops_delivery() {
    let mut router = EventRouter::new();

    let pattern = RoutePattern {
        source_type: SourceType::Device,
        source_id: None,
        event_kind: Some(EventKind::AxisUpdate),
    };
    let route_id = router
        .register_route(pattern, EventFilter::pass_all(), 100)
        .expect("registration must succeed");

    // Verify the route matches before removal
    let event = device_axis_event(1, 0.5, 1000);
    let matches = router.route_event(&event);
    assert_eq!(matches.len(), 1, "route must match before removal");
    assert!(matches.contains(100), "destination 100 must be present");

    // Remove the route
    let removed = router.remove_route(route_id);
    assert!(removed, "remove_route must return true for existing route");
    assert_eq!(router.route_count(), 0, "route count must be 0 after removal");

    // Same event must no longer match
    let event2 = device_axis_event(1, 0.6, 2000);
    let matches2 = router.route_event(&event2);
    assert_eq!(matches2.len(), 0, "removed route must not match any event");
}

// ---------------------------------------------------------------------------
// 5. Topic filtering correctness
// ---------------------------------------------------------------------------
#[test]
fn topic_filtering_correctness() {
    let mut router = EventRouter::new();

    // Route A: AxisUpdate → destination 1
    let pattern_a = RoutePattern {
        source_type: SourceType::Device,
        source_id: None,
        event_kind: Some(EventKind::AxisUpdate),
    };
    router.register_route(pattern_a, EventFilter::pass_all(), 1);

    // Route B: ButtonPress → destination 2
    let pattern_b = RoutePattern {
        source_type: SourceType::Device,
        source_id: None,
        event_kind: Some(EventKind::ButtonPress),
    };
    router.register_route(pattern_b, EventFilter::pass_all(), 2);

    // AxisUpdate event: must match route A (dest 1), not route B (dest 2)
    let axis_ev = device_axis_event(1, 0.5, 1000);
    let m_axis = router.route_event(&axis_ev);
    assert_eq!(m_axis.len(), 1, "only axis route should match axis event");
    assert!(m_axis.contains(1), "destination 1 must match AxisUpdate");
    assert!(!m_axis.contains(2), "destination 2 must NOT match AxisUpdate");

    // ButtonPress event: must match route B (dest 2), not route A (dest 1)
    let btn_ev = device_button_event(1, true, 2000);
    let m_btn = router.route_event(&btn_ev);
    assert_eq!(m_btn.len(), 1, "only button route should match button event");
    assert!(m_btn.contains(2), "destination 2 must match ButtonPress");
    assert!(!m_btn.contains(1), "destination 1 must NOT match ButtonPress");
}
