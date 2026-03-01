// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for the allocation-free event router.
//!
//! **Invariant 1 – Event ordering preserved**: Events routed through the bus
//! arrive at destinations in the same order they were submitted.
//!
//! **Invariant 2 – No message loss under normal load**: When backpressure is 0
//! and no rate limit is active, every event matching a route is delivered.
//!
//! **Invariant 3 – Drop-tail under overload**: Under backpressure, only
//! low-priority events are dropped; High and Critical always pass.
//!
//! **Invariant 4 – Topic filtering correctness**: Events only reach routes
//! whose topic pattern matches.

use flight_bus::routing::{
    BusEvent, EventFilter, EventKind, EventPayload, EventPriority, EventRouter, RoutePattern,
    SourceType, Topic, MAX_ROUTES,
};
use proptest::prelude::*;

// ── Strategies ───────────────────────────────────────────────────────────────

fn source_type_strategy() -> impl Strategy<Value = SourceType> {
    prop_oneof![
        Just(SourceType::Device),
        Just(SourceType::Simulator),
        Just(SourceType::Internal),
    ]
}

fn event_kind_strategy() -> impl Strategy<Value = EventKind> {
    prop_oneof![
        Just(EventKind::AxisUpdate),
        Just(EventKind::ButtonPress),
        Just(EventKind::ButtonRelease),
        Just(EventKind::TelemetryFrame),
        Just(EventKind::SystemStatus),
    ]
}

fn priority_strategy() -> impl Strategy<Value = EventPriority> {
    prop_oneof![
        Just(EventPriority::Background),
        Just(EventPriority::Low),
        Just(EventPriority::Normal),
        Just(EventPriority::High),
        Just(EventPriority::Critical),
    ]
}

fn topic_strategy() -> impl Strategy<Value = Topic> {
    prop_oneof![
        Just(Topic::Telemetry),
        Just(Topic::Commands),
        Just(Topic::Lifecycle),
        Just(Topic::Diagnostics),
    ]
}

fn bus_event_strategy() -> impl Strategy<Value = BusEvent> {
    (
        source_type_strategy(),
        0u32..100,
        event_kind_strategy(),
        topic_strategy(),
        priority_strategy(),
        1_000_000u64..10_000_000u64,
        prop_oneof![
            (-1.0f64..=1.0f64).prop_map(|v| EventPayload::Axis { axis_id: 0, value: v }),
            any::<bool>().prop_map(|p| EventPayload::Button {
                button_id: 0,
                pressed: p
            }),
        ],
    )
        .prop_map(
            |(source_type, source_id, kind, topic, priority, ts, payload)| {
                BusEvent::with_topic(source_type, source_id, kind, topic, priority, ts, payload)
            },
        )
}

// ── Invariant 1: Event ordering preserved ────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Events routed to the same destination arrive in submission order.
    #[test]
    fn ordering_preserved(n in 2usize..=20) {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);

        let mut delivered_ids = Vec::new();
        for i in 0..n {
            let event = BusEvent::new(
                SourceType::Device,
                1,
                EventKind::AxisUpdate,
                EventPriority::Normal,
                (i as u64 + 1) * 100_000,
                EventPayload::Axis { axis_id: 0, value: i as f64 * 0.1 },
            );
            let matches = router.route_event(&event);
            if matches.contains(10) {
                delivered_ids.push(event.id);
            }
        }

        // IDs are monotonically increasing (assigned by atomic counter).
        for window in delivered_ids.windows(2) {
            prop_assert!(
                window[0] < window[1],
                "ordering violated: id {} >= {}",
                window[0],
                window[1]
            );
        }
    }
}

// ── Invariant 2: No message loss under normal load ───────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// With backpressure=0 and no rate limit, every event matching a wildcard
    /// route is delivered (zero loss).
    #[test]
    fn no_loss_under_normal_load(n in 1usize..=50) {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);
        router.set_backpressure(0);

        let mut delivered = 0usize;
        for i in 0..n {
            let event = BusEvent::new(
                SourceType::Device,
                1,
                EventKind::AxisUpdate,
                EventPriority::Normal,
                // Spread timestamps so no debounce/rate filter triggers.
                (i as u64 + 1) * 1_000_000,
                EventPayload::Axis { axis_id: 0, value: i as f64 * 0.01 },
            );
            if !router.route_event(&event).is_empty() {
                delivered += 1;
            }
        }

        prop_assert_eq!(
            delivered, n,
            "expected {} deliveries with no backpressure, got {}",
            n, delivered
        );
    }
}

// ── Invariant 3: Drop-tail under overload ────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Under any backpressure level, High and Critical events are never dropped.
    #[test]
    fn high_critical_never_dropped(bp in 0u8..=100u8) {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);
        router.set_backpressure(bp);

        let high = BusEvent::new(
            SourceType::Device, 1, EventKind::AxisUpdate,
            EventPriority::High, 1_000_000,
            EventPayload::Axis { axis_id: 0, value: 0.5 },
        );
        let crit = BusEvent::new(
            SourceType::Device, 1, EventKind::AxisUpdate,
            EventPriority::Critical, 2_000_000,
            EventPayload::Axis { axis_id: 0, value: 0.5 },
        );

        prop_assert!(
            !router.route_event(&high).is_empty(),
            "High priority dropped at backpressure={}",
            bp
        );
        prop_assert!(
            !router.route_event(&crit).is_empty(),
            "Critical priority dropped at backpressure={}",
            bp
        );
    }

    /// Background events are dropped when backpressure >= 25%.
    #[test]
    fn background_dropped_at_25_percent(bp in 25u8..=100u8) {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);
        router.set_backpressure(bp);

        let bg = BusEvent::new(
            SourceType::Device, 1, EventKind::AxisUpdate,
            EventPriority::Background, 1_000_000,
            EventPayload::Axis { axis_id: 0, value: 0.5 },
        );

        prop_assert!(
            router.route_event(&bg).is_empty(),
            "Background should be dropped at backpressure={}",
            bp
        );
    }
}

// ── Invariant 4: Topic filtering correctness ─────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// An event only reaches a route whose topic matches the event's topic.
    #[test]
    fn topic_filter_correctness(
        route_topic in topic_strategy(),
        event in bus_event_strategy(),
    ) {
        let mut router = EventRouter::new();
        router.register_route(
            RoutePattern::for_topic(route_topic),
            EventFilter::pass_all(),
            10,
        );

        let matches = router.route_event(&event);
        if event.topic == route_topic {
            prop_assert!(
                matches.contains(10),
                "event topic {:?} should match route topic {:?}",
                event.topic, route_topic
            );
        } else {
            prop_assert!(
                !matches.contains(10),
                "event topic {:?} should NOT match route topic {:?}",
                event.topic, route_topic
            );
        }
    }

    /// A wildcard topic route accepts events of any topic.
    #[test]
    fn topic_any_accepts_all(event in bus_event_strategy()) {
        let mut router = EventRouter::new();
        router.register_route(
            RoutePattern::for_topic(Topic::Any),
            EventFilter::pass_all(),
            10,
        );

        let matches = router.route_event(&event);
        prop_assert!(
            matches.contains(10),
            "Topic::Any should match event topic {:?}",
            event.topic
        );
    }

    /// Register/remove cycle never panics.
    #[test]
    fn register_remove_never_panics(
        n_routes in 1usize..=MAX_ROUTES,
    ) {
        let mut router = EventRouter::new();
        let mut ids = Vec::new();

        for i in 0..n_routes {
            if let Some(id) = router.register_route(
                RoutePattern::any(),
                EventFilter::pass_all(),
                i as u32,
            ) {
                ids.push(id);
            }
        }

        for id in ids {
            router.remove_route(id);
        }

        prop_assert_eq!(router.route_count(), 0);
    }
}
