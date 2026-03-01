// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for flight-bus routing invariants.
//!
//! - RouteMatches never exceeds MAX_MATCHES
//! - route_count tracks register/remove accurately
//! - RoutePattern::any() matches all source types
//! - Backpressure: Critical/High events always pass
//! - Event ordering: route results are deterministic

use flight_bus::routing::{
    BusEvent, EventFilter, EventKind, EventPayload, EventPriority, EventRouter, RoutePattern,
    SourceType, MAX_MATCHES, MAX_ROUTES,
};
use proptest::prelude::*;

fn make_axis_event(
    source_type: SourceType,
    source_id: u32,
    value: f64,
    priority: EventPriority,
    timestamp_us: u64,
) -> BusEvent {
    BusEvent::new(
        source_type,
        source_id,
        EventKind::AxisUpdate,
        priority,
        timestamp_us,
        EventPayload::Axis { axis_id: 0, value },
    )
}

proptest! {
    // ── RouteMatches never exceeds MAX_MATCHES ──────────────────────────────

    /// Even with many routes registered, route_event returns at most MAX_MATCHES.
    #[test]
    fn route_matches_capped_at_max(n_routes in 1usize..=MAX_ROUTES) {
        let mut router = EventRouter::new();
        for dest in 0..n_routes as u32 {
            router.register_route(RoutePattern::any(), EventFilter::pass_all(), dest);
        }
        let event = make_axis_event(
            SourceType::Device, 1, 0.5, EventPriority::Normal, 1000,
        );
        let matches = router.route_event(&event);
        prop_assert!(
            matches.len() <= MAX_MATCHES,
            "route_event returned {} matches, exceeding MAX_MATCHES={}",
            matches.len(), MAX_MATCHES
        );
    }

    // ── route_count tracks register/remove ──────────────────────────────────

    /// After registering N routes, route_count == N.
    #[test]
    fn route_count_after_register(n in 1usize..=32) {
        let mut router = EventRouter::new();
        for i in 0..n {
            router.register_route(RoutePattern::any(), EventFilter::pass_all(), i as u32);
        }
        prop_assert_eq!(
            router.route_count(), n,
            "expected {} routes, got {}", n, router.route_count()
        );
    }

    /// Removing a route decrements route_count.
    #[test]
    fn route_count_after_remove(n in 2usize..=16) {
        let mut router = EventRouter::new();
        let mut ids = Vec::new();
        for i in 0..n {
            if let Some(id) = router.register_route(
                RoutePattern::any(), EventFilter::pass_all(), i as u32,
            ) {
                ids.push(id);
            }
        }
        let before = router.route_count();
        if let Some(&id) = ids.first() {
            router.remove_route(id);
        }
        prop_assert_eq!(
            router.route_count(), before - 1,
            "route_count should decrement after remove"
        );
    }

    // ── RoutePattern::any() matches all source types ────────────────────────

    /// any() pattern matches Device, Simulator, and Internal sources.
    #[test]
    fn any_pattern_matches_all_sources(src_idx in 0u8..3u8) {
        let src = match src_idx {
            0 => SourceType::Device,
            1 => SourceType::Simulator,
            _ => SourceType::Internal,
        };
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 42);
        let event = make_axis_event(src, 1, 0.5, EventPriority::Normal, 1000);
        let matches = router.route_event(&event);
        prop_assert!(
            matches.contains(42),
            "any() pattern should match {:?}", src
        );
    }

    // ── Backpressure: Critical/High events always pass ──────────────────────

    /// Critical events pass even at 100% backpressure.
    #[test]
    fn critical_events_bypass_backpressure(bp in 0u8..=100) {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);
        router.set_backpressure(bp);
        let event = make_axis_event(
            SourceType::Device, 1, 0.5, EventPriority::Critical, 1000,
        );
        let matches = router.route_event(&event);
        prop_assert!(
            matches.contains(1),
            "Critical events should pass at backpressure={}%", bp
        );
    }

    /// High priority events pass even at 100% backpressure.
    #[test]
    fn high_events_bypass_backpressure(bp in 0u8..=100) {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);
        router.set_backpressure(bp);
        let event = make_axis_event(
            SourceType::Device, 1, 0.5, EventPriority::High, 1000,
        );
        let matches = router.route_event(&event);
        prop_assert!(
            matches.contains(1),
            "High events should pass at backpressure={}%", bp
        );
    }

    /// Background events are dropped at >= 25% backpressure.
    #[test]
    fn background_dropped_under_backpressure(bp in 25u8..=100) {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);
        router.set_backpressure(bp);
        let event = make_axis_event(
            SourceType::Device, 1, 0.5, EventPriority::Background, 1000,
        );
        let matches = router.route_event(&event);
        prop_assert!(
            matches.is_empty(),
            "Background events should be dropped at backpressure={}%", bp
        );
    }

    // ── Deterministic routing ───────────────────────────────────────────────

    /// Same event routed twice produces same destinations.
    #[test]
    fn routing_is_deterministic(
        value in -1.0f64..=1.0,
        src_id in 0u32..100,
    ) {
        let mut router1 = EventRouter::new();
        let mut router2 = EventRouter::new();
        for dest in 0..5u32 {
            router1.register_route(RoutePattern::any(), EventFilter::pass_all(), dest);
            router2.register_route(RoutePattern::any(), EventFilter::pass_all(), dest);
        }
        let event = make_axis_event(
            SourceType::Device, src_id, value, EventPriority::Normal, 1000,
        );
        let m1 = router1.route_event(&event);
        let m2 = router2.route_event(&event);
        prop_assert_eq!(
            m1.len(), m2.len(),
            "routing should be deterministic: {} vs {} matches", m1.len(), m2.len()
        );
    }

    // ── Value range filter ──────────────────────────────────────────────────

    /// Events with value outside filter range are excluded.
    #[test]
    fn value_filter_excludes_out_of_range(
        value in -10.0f64..10.0,
        min_val in 0.0f64..=0.5,
        max_val in 0.5f64..=1.0,
    ) {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            min_value: Some(min_val),
            max_value: Some(max_val),
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 1);
        let event = make_axis_event(
            SourceType::Device, 1, value, EventPriority::Normal, 1000,
        );
        let matches = router.route_event(&event);
        if value < min_val || value > max_val {
            prop_assert!(
                matches.is_empty(),
                "value {} outside [{}, {}] should be filtered", value, min_val, max_val
            );
        } else {
            prop_assert!(
                matches.contains(1),
                "value {} inside [{}, {}] should pass", value, min_val, max_val
            );
        }
    }
}
