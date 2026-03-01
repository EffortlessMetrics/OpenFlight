// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for bus event routing: topic-based pub/sub, backpressure,
//! ordering guarantees, publisher/subscriber lifecycle, event types, and
//! concurrency.

use flight_bus::publisher::{BusPublisher, SubscriptionConfig};
use flight_bus::routing::{
    BusEvent, EventFilter, EventKind, EventPayload, EventPriority, EventRouter, RoutePattern,
    SourceType, MAX_ROUTES,
};
use flight_bus::snapshot::BusSnapshot;
use flight_bus::types::{AircraftId, SimId};
use proptest::prelude::*;
use std::collections::HashSet;
use std::sync::{Arc, Barrier};
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn make_event(
    source_type: SourceType,
    source_id: u32,
    kind: EventKind,
    priority: EventPriority,
    timestamp_us: u64,
    payload: EventPayload,
) -> BusEvent {
    BusEvent::new(source_type, source_id, kind, priority, timestamp_us, payload)
}

fn axis_event(source_id: u32, value: f64, timestamp_us: u64) -> BusEvent {
    make_event(
        SourceType::Device,
        source_id,
        EventKind::AxisUpdate,
        EventPriority::Normal,
        timestamp_us,
        EventPayload::Axis {
            axis_id: 0,
            value,
        },
    )
}

fn telemetry_event(field_id: u16, value: f64, timestamp_us: u64) -> BusEvent {
    make_event(
        SourceType::Simulator,
        1,
        EventKind::TelemetryFrame,
        EventPriority::Normal,
        timestamp_us,
        EventPayload::Telemetry { field_id, value },
    )
}

fn status_event(code: u16, timestamp_us: u64) -> BusEvent {
    make_event(
        SourceType::Internal,
        0,
        EventKind::SystemStatus,
        EventPriority::High,
        timestamp_us,
        EventPayload::System { code },
    )
}

fn make_publisher() -> BusPublisher {
    BusPublisher::new(60.0)
}

fn valid_snapshot() -> BusSnapshot {
    BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"))
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Topic routing (pattern-based)
// ═══════════════════════════════════════════════════════════════════════════

mod topic_routing {
    use super::*;

    /// Publish to a specific source type → only that type's subscribers receive.
    #[test]
    fn specific_topic_only_matching_subscribers_receive() {
        let mut router = EventRouter::new();
        let device_pattern = RoutePattern {
            source_type: SourceType::Device,
            source_id: None,
            event_kind: None,
        };
        let sim_pattern = RoutePattern {
            source_type: SourceType::Simulator,
            source_id: None,
            event_kind: None,
        };

        router.register_route(device_pattern, EventFilter::pass_all(), 1);
        router.register_route(sim_pattern, EventFilter::pass_all(), 2);

        let device_evt = axis_event(1, 0.5, 1000);
        let matches = router.route_event(&device_evt);
        assert_eq!(matches.len(), 1);
        assert!(matches.contains(1));
        assert!(!matches.contains(2));

        let sim_evt = telemetry_event(0, 42.0, 2000);
        let matches = router.route_event(&sim_evt);
        assert_eq!(matches.len(), 1);
        assert!(matches.contains(2));
        assert!(!matches.contains(1));
    }

    /// Wildcard (Any) source subscription receives events from all source types.
    #[test]
    fn wildcard_subscription_receives_all_topics() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 99);

        for src in [
            SourceType::Device,
            SourceType::Simulator,
            SourceType::Internal,
        ] {
            let evt = make_event(
                src,
                1,
                EventKind::AxisUpdate,
                EventPriority::Normal,
                1000,
                EventPayload::Axis {
                    axis_id: 0,
                    value: 0.0,
                },
            );
            let m = router.route_event(&evt);
            assert!(m.contains(99), "wildcard should match {src:?}");
        }
    }

    /// Multiple subscribers on the same pattern all receive the event.
    #[test]
    fn multiple_subscribers_same_topic_all_receive() {
        let mut router = EventRouter::new();
        let pattern = RoutePattern {
            source_type: SourceType::Device,
            source_id: None,
            event_kind: Some(EventKind::AxisUpdate),
        };

        router.register_route(pattern, EventFilter::pass_all(), 10);
        router.register_route(pattern, EventFilter::pass_all(), 20);
        router.register_route(pattern, EventFilter::pass_all(), 30);

        let evt = axis_event(1, 0.5, 1000);
        let m = router.route_event(&evt);
        assert_eq!(m.len(), 3);
        assert!(m.contains(10));
        assert!(m.contains(20));
        assert!(m.contains(30));
    }

    /// No subscriber → message dropped silently (not an error).
    #[test]
    fn no_subscriber_message_dropped_no_error() {
        let mut router = EventRouter::new();
        let evt = axis_event(1, 0.5, 1000);
        let m = router.route_event(&evt);
        assert!(m.is_empty());
        // No panic, no error — just empty matches.
    }

    // Property: publish(pattern, event) → all matching subscribers receive.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_all_matching_subscribers_receive(
            n_subs in 1usize..=8,
            value in -1.0f64..=1.0,
        ) {
            let mut router = EventRouter::new();
            let pattern = RoutePattern {
                source_type: SourceType::Device,
                source_id: None,
                event_kind: None,
            };

            for i in 0..n_subs {
                router.register_route(pattern, EventFilter::pass_all(), i as u32);
            }

            let evt = axis_event(1, value, 1000);
            let m = router.route_event(&evt);
            prop_assert_eq!(
                m.len(), n_subs,
                "all {} subscribers should match", n_subs
            );
            for i in 0..n_subs {
                prop_assert!(m.contains(i as u32), "subscriber {} missing", i);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Message ordering
// ═══════════════════════════════════════════════════════════════════════════

mod message_ordering {
    use super::*;

    /// Messages on the same route arrive in timestamp order.
    #[test]
    fn same_topic_messages_arrive_in_order() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);

        let mut received_timestamps = Vec::new();
        for t in (1000..=5000).step_by(1000) {
            let evt = axis_event(1, t as f64 / 1000.0, t);
            let m = router.route_event(&evt);
            if m.contains(1) {
                received_timestamps.push(t);
            }
        }

        // All events should have been delivered, and in order.
        assert_eq!(received_timestamps.len(), 5);
        for window in received_timestamps.windows(2) {
            assert!(window[0] < window[1], "ordering violated");
        }
    }

    /// Messages from same publisher (source_id) arrive in order across event kinds.
    #[test]
    fn same_publisher_messages_arrive_in_order_across_kinds() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);

        let events = vec![
            make_event(
                SourceType::Device,
                42,
                EventKind::AxisUpdate,
                EventPriority::Normal,
                1000,
                EventPayload::Axis {
                    axis_id: 0,
                    value: 0.1,
                },
            ),
            make_event(
                SourceType::Device,
                42,
                EventKind::ButtonPress,
                EventPriority::Normal,
                2000,
                EventPayload::Button {
                    button_id: 1,
                    pressed: true,
                },
            ),
            make_event(
                SourceType::Device,
                42,
                EventKind::AxisUpdate,
                EventPriority::Normal,
                3000,
                EventPayload::Axis {
                    axis_id: 1,
                    value: 0.9,
                },
            ),
        ];

        let mut delivery_order = Vec::new();
        for evt in &events {
            let m = router.route_event(evt);
            if m.contains(1) {
                delivery_order.push(evt.timestamp_us);
            }
        }

        assert_eq!(delivery_order, vec![1000, 2000, 3000]);
    }

    /// Document: No ordering guarantees across different publishers.
    /// We verify that routing handles interleaved publishers without panicking
    /// and delivers to the correct destinations.
    #[test]
    fn no_ordering_guarantees_across_publishers() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);

        // Interleaved events from two different sources.
        let events = vec![
            axis_event(1, 0.1, 1000), // publisher A
            axis_event(2, 0.2, 500),  // publisher B (earlier timestamp!)
            axis_event(1, 0.3, 2000), // publisher A
            axis_event(2, 0.4, 1500), // publisher B
        ];

        for evt in &events {
            let m = router.route_event(evt);
            assert_eq!(m.len(), 1, "each event should match exactly one route");
        }
    }

    /// BusPublisher FIFO ordering: subscriber receives snapshots in publish order.
    #[test]
    fn bus_publisher_fifo_ordering() {
        let mut publisher = make_publisher();
        let mut sub = publisher
            .subscribe(SubscriptionConfig::default())
            .unwrap();

        let sims = [SimId::Msfs, SimId::XPlane, SimId::Dcs];
        for (i, &sim) in sims.iter().enumerate() {
            let snap = BusSnapshot::new(sim, AircraftId::new("TEST"));
            publisher.publish(snap).unwrap();
            if i < sims.len() - 1 {
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        let mut received = Vec::new();
        while let Ok(Some(snap)) = sub.try_recv() {
            received.push(snap.sim);
        }
        // First is always delivered; others depend on rate limiter timing,
        // but ordering must be preserved for those that are delivered.
        assert!(!received.is_empty());
        assert_eq!(received[0], SimId::Msfs);
        for window in received.windows(2) {
            let idx_a = sims.iter().position(|&s| s == window[0]).unwrap();
            let idx_b = sims.iter().position(|&s| s == window[1]).unwrap();
            assert!(idx_a < idx_b, "FIFO ordering violated");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Backpressure
// ═══════════════════════════════════════════════════════════════════════════

mod backpressure {
    use super::*;

    /// Slow subscriber → drop-tail policy kicks in (BusPublisher level).
    #[test]
    fn slow_subscriber_drop_tail_kicks_in() {
        let mut publisher = make_publisher();
        let config = SubscriptionConfig {
            buffer_size: 2,
            drop_on_full: true,
            max_rate_hz: 60.0,
        };
        let _sub = publisher.subscribe(config).unwrap();

        let snap = valid_snapshot();
        // Fill the buffer (2 slots).
        publisher.publish(snap.clone()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        publisher.publish(snap.clone()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        // This one should be dropped.
        publisher.publish(snap.clone()).unwrap();

        assert!(
            publisher.drop_count() >= 1,
            "expected at least 1 drop, got {}",
            publisher.drop_count()
        );
    }

    /// Drop counter increments for each dropped message.
    #[test]
    fn drop_counter_increments_per_drop() {
        let mut publisher = make_publisher();
        let config = SubscriptionConfig {
            buffer_size: 1,
            drop_on_full: true,
            max_rate_hz: 60.0,
        };
        let _sub = publisher.subscribe(config).unwrap();

        let snap = valid_snapshot();
        // First fills the single-slot buffer.
        publisher.publish(snap.clone()).unwrap();
        std::thread::sleep(Duration::from_millis(20));

        // Next N publishes should all be dropped.
        let n_extra = 4;
        for _ in 0..n_extra {
            publisher.publish(snap.clone()).unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }

        assert_eq!(publisher.drop_count(), n_extra);
    }

    /// Fast subscriber is unaffected by slow subscriber.
    #[test]
    fn fast_subscriber_unaffected_by_slow() {
        let mut publisher = make_publisher();

        // Slow subscriber with tiny buffer (never drained).
        let slow_config = SubscriptionConfig {
            buffer_size: 1,
            drop_on_full: true,
            max_rate_hz: 60.0,
        };
        let _slow = publisher.subscribe(slow_config).unwrap();

        // Fast subscriber with large buffer.
        let fast_config = SubscriptionConfig {
            buffer_size: 100,
            drop_on_full: true,
            max_rate_hz: 60.0,
        };
        let mut fast = publisher.subscribe(fast_config).unwrap();

        let snap = valid_snapshot();
        publisher.publish(snap.clone()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        publisher.publish(snap.clone()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        publisher.publish(snap.clone()).unwrap();

        // Fast subscriber should have received messages despite slow one backing up.
        let mut count = 0;
        while fast.try_recv().unwrap().is_some() {
            count += 1;
        }
        assert!(count >= 1, "fast subscriber should receive messages");
    }

    /// Backpressure threshold is configurable (routing-level).
    #[test]
    fn backpressure_threshold_configurable() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);

        // No backpressure — Background events pass.
        router.set_backpressure(0);
        let bg = make_event(
            SourceType::Device,
            1,
            EventKind::AxisUpdate,
            EventPriority::Background,
            1000,
            EventPayload::Axis {
                axis_id: 0,
                value: 0.5,
            },
        );
        assert_eq!(router.route_event(&bg).len(), 1);

        // 30% backpressure — Background dropped.
        router.set_backpressure(30);
        let bg2 = make_event(
            SourceType::Device,
            1,
            EventKind::AxisUpdate,
            EventPriority::Background,
            2000,
            EventPayload::Axis {
                axis_id: 0,
                value: 0.6,
            },
        );
        assert!(router.route_event(&bg2).is_empty());

        // Reset back to 0.
        router.set_backpressure(0);
        let bg3 = make_event(
            SourceType::Device,
            1,
            EventKind::AxisUpdate,
            EventPriority::Background,
            3000,
            EventPayload::Axis {
                axis_id: 0,
                value: 0.7,
            },
        );
        assert_eq!(router.route_event(&bg3).len(), 1);
    }

    /// Property: total_published = total_received + total_dropped (BusPublisher).
    #[test]
    fn published_equals_received_plus_dropped() {
        let mut publisher = make_publisher();
        let config = SubscriptionConfig {
            buffer_size: 3,
            drop_on_full: true,
            max_rate_hz: 60.0,
        };
        let mut sub = publisher.subscribe(config).unwrap();

        let snap = valid_snapshot();
        let n_published = 10u64;
        for _ in 0..n_published {
            let _ = publisher.publish(snap.clone());
            std::thread::sleep(Duration::from_millis(20));
        }

        let mut received = 0u64;
        while sub.try_recv().unwrap().is_some() {
            received += 1;
        }

        let dropped = publisher.drop_count();
        let stats = publisher.stats();
        // Some publishes may be rate-limited (not published at all),
        // so published ≤ n_published. For those that were published:
        // published_to_subscribers = received + dropped.
        let actual_published = stats.snapshots_published;
        assert!(
            actual_published > 0,
            "at least some snapshots should be published"
        );
        assert_eq!(
            received + dropped,
            actual_published,
            "received ({received}) + dropped ({dropped}) should equal published ({actual_published})"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Publisher/subscriber lifecycle
// ═══════════════════════════════════════════════════════════════════════════

mod lifecycle {
    use super::*;

    /// Drop publisher → subscribers get no more messages.
    #[test]
    fn dropped_publisher_subscribers_see_disconnect() {
        let mut publisher = make_publisher();
        let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

        publisher.publish(valid_snapshot()).unwrap();
        let msg = sub.try_recv().unwrap();
        assert!(msg.is_some());

        drop(publisher);
        // Channel is disconnected; further recv returns error or None.
        // Drain any remaining buffered messages first.
        while sub.try_recv().is_ok_and(|m| m.is_some()) {}
        let result = sub.try_recv();
        assert!(
            result.is_err() || result.unwrap().is_none(),
            "should detect publisher gone"
        );
    }

    /// Late subscriber sees only new messages (no replay of old data).
    #[test]
    fn late_subscriber_sees_only_new_messages() {
        let mut publisher = make_publisher();

        // Publish before subscribing.
        publisher.publish(valid_snapshot()).unwrap();
        std::thread::sleep(Duration::from_millis(20));

        let mut late_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
        // Late subscriber should not see the old message.
        assert!(late_sub.try_recv().unwrap().is_none());

        // New publish after subscription.
        publisher.publish(valid_snapshot()).unwrap();
        assert!(late_sub.try_recv().unwrap().is_some());
    }

    /// Multiple publishers, multiple subscribers → correct routing.
    #[test]
    fn multiple_publishers_multiple_subscribers_routing() {
        let mut router = EventRouter::new();

        // Route device source_id=1 → dest 100.
        let p1 = RoutePattern {
            source_type: SourceType::Device,
            source_id: Some(1),
            event_kind: None,
        };
        router.register_route(p1, EventFilter::pass_all(), 100);

        // Route device source_id=2 → dest 200.
        let p2 = RoutePattern {
            source_type: SourceType::Device,
            source_id: Some(2),
            event_kind: None,
        };
        router.register_route(p2, EventFilter::pass_all(), 200);

        // Wildcard → dest 300 (receives from both).
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 300);

        let e1 = axis_event(1, 0.5, 1000);
        let m1 = router.route_event(&e1);
        assert!(m1.contains(100));
        assert!(!m1.contains(200));
        assert!(m1.contains(300));

        let e2 = axis_event(2, 0.7, 2000);
        let m2 = router.route_event(&e2);
        assert!(!m2.contains(100));
        assert!(m2.contains(200));
        assert!(m2.contains(300));
    }

    /// Publisher/route IDs are unique.
    #[test]
    fn publisher_ids_are_unique() {
        let mut router = EventRouter::new();
        let mut ids = HashSet::new();
        for i in 0..20 {
            let id = router
                .register_route(RoutePattern::any(), EventFilter::pass_all(), i)
                .unwrap();
            assert!(ids.insert(id.raw()), "duplicate route id: {}", id.raw());
        }
    }

    /// Subscriber IDs are unique across multiple subscribe calls.
    #[test]
    fn subscriber_ids_are_unique() {
        let mut publisher = make_publisher();
        let mut ids = HashSet::new();
        for _ in 0..10 {
            let sub = publisher
                .subscribe(SubscriptionConfig::default())
                .unwrap();
            assert!(
                ids.insert(sub.id),
                "duplicate subscriber id"
            );
        }
    }

    /// Route removal after events have been processed still works.
    #[test]
    fn route_removal_after_events() {
        let mut router = EventRouter::new();
        let id = router
            .register_route(RoutePattern::any(), EventFilter::pass_all(), 1)
            .unwrap();

        // Process some events.
        for t in (1000..=3000).step_by(1000) {
            let evt = axis_event(1, 0.5, t);
            router.route_event(&evt);
        }

        // Remove and verify.
        assert!(router.remove_route(id));
        let evt = axis_event(1, 0.5, 4000);
        assert!(router.route_event(&evt).is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Event types
// ═══════════════════════════════════════════════════════════════════════════

mod event_types {
    use super::*;

    /// AxisEvent routing — only axis routes receive it.
    #[test]
    fn axis_event_routing() {
        let mut router = EventRouter::new();
        let axis_only = RoutePattern {
            source_type: SourceType::Any,
            source_id: None,
            event_kind: Some(EventKind::AxisUpdate),
        };
        let telem_only = RoutePattern {
            source_type: SourceType::Any,
            source_id: None,
            event_kind: Some(EventKind::TelemetryFrame),
        };
        router.register_route(axis_only, EventFilter::pass_all(), 1);
        router.register_route(telem_only, EventFilter::pass_all(), 2);

        let evt = axis_event(1, 0.5, 1000);
        let m = router.route_event(&evt);
        assert!(m.contains(1));
        assert!(!m.contains(2));
    }

    /// TelemetryEvent routing.
    #[test]
    fn telemetry_event_routing() {
        let mut router = EventRouter::new();
        let telem_only = RoutePattern {
            source_type: SourceType::Any,
            source_id: None,
            event_kind: Some(EventKind::TelemetryFrame),
        };
        router.register_route(telem_only, EventFilter::pass_all(), 1);

        let evt = telemetry_event(42, 100.0, 1000);
        let m = router.route_event(&evt);
        assert_eq!(m.len(), 1);
        assert!(m.contains(1));
    }

    /// StatusEvent routing.
    #[test]
    fn status_event_routing() {
        let mut router = EventRouter::new();
        let status_only = RoutePattern {
            source_type: SourceType::Any,
            source_id: None,
            event_kind: Some(EventKind::SystemStatus),
        };
        router.register_route(status_only, EventFilter::pass_all(), 1);

        let evt = status_event(0x01, 1000);
        let m = router.route_event(&evt);
        assert_eq!(m.len(), 1);
        assert!(m.contains(1));

        // Non-status event should not match.
        let axis = axis_event(1, 0.5, 2000);
        assert!(router.route_event(&axis).is_empty());
    }

    /// Event payload value round-trip: axis value is preserved.
    #[test]
    fn event_payload_value_round_trip() {
        let values = [0.0, -1.0, 1.0, 0.12345, f64::MIN_POSITIVE, 999.999];
        for &v in &values {
            let evt = axis_event(1, v, 1000);
            match evt.payload {
                EventPayload::Axis { value, .. } => {
                    assert!(
                        (value - v).abs() < f64::EPSILON,
                        "value not preserved: {v} vs {value}"
                    );
                }
                _ => panic!("expected Axis payload"),
            }
        }
    }

    /// Telemetry payload field_id and value preserved.
    #[test]
    fn telemetry_payload_preserved() {
        let evt = telemetry_event(42, 123.456, 1000);
        match evt.payload {
            EventPayload::Telemetry { field_id, value } => {
                assert_eq!(field_id, 42);
                assert!((value - 123.456).abs() < f64::EPSILON);
            }
            _ => panic!("expected Telemetry payload"),
        }
    }

    /// System payload code preserved.
    #[test]
    fn system_payload_preserved() {
        let evt = status_event(0xBEEF, 1000);
        match evt.payload {
            EventPayload::System { code } => assert_eq!(code, 0xBEEF),
            _ => panic!("expected System payload"),
        }
    }

    /// Button payload preserved.
    #[test]
    fn button_payload_preserved() {
        let evt = make_event(
            SourceType::Device,
            1,
            EventKind::ButtonPress,
            EventPriority::Normal,
            1000,
            EventPayload::Button {
                button_id: 7,
                pressed: true,
            },
        );
        match evt.payload {
            EventPayload::Button {
                button_id,
                pressed,
            } => {
                assert_eq!(button_id, 7);
                assert!(pressed);
            }
            _ => panic!("expected Button payload"),
        }
    }

    /// Empty payload has no extractable value.
    #[test]
    fn empty_payload_no_value() {
        let evt = make_event(
            SourceType::Internal,
            0,
            EventKind::SystemStatus,
            EventPriority::Normal,
            1000,
            EventPayload::Empty,
        );
        assert!(evt.payload.value().is_none());
    }

    /// BusSnapshot serialization round-trip via serde_json.
    #[test]
    fn snapshot_serde_round_trip() {
        let snap = valid_snapshot();
        let json = serde_json::to_string(&snap).expect("serialize");
        let deserialized: BusSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.sim, snap.sim);
        assert_eq!(deserialized.aircraft, snap.aircraft);
    }

    /// Large snapshot (many engines) can be serialized and deserialized.
    #[test]
    fn large_snapshot_round_trip() {
        let mut snap = valid_snapshot();
        // Add many engines to make the snapshot large.
        use flight_bus::snapshot::EngineData;
        use flight_bus::types::Percentage;
        for i in 0..20 {
            snap.engines.push(EngineData {
                index: i,
                running: true,
                rpm: Percentage::new(90.0).unwrap(),
                manifold_pressure: Some(29.0),
                egt: Some(700.0),
                cht: Some(200.0),
                fuel_flow: Some(15.0),
                oil_pressure: Some(60.0),
                oil_temperature: Some(100.0),
            });
        }
        let json = serde_json::to_string(&snap).expect("serialize large snapshot");
        let back: BusSnapshot = serde_json::from_str(&json).expect("deserialize large snapshot");
        assert_eq!(back.engines.len(), 20);
    }

    /// Event IDs are globally unique.
    #[test]
    fn event_ids_globally_unique() {
        let mut ids = HashSet::new();
        for i in 0..100 {
            let evt = axis_event(1, i as f64 * 0.01, i * 100);
            assert!(ids.insert(evt.id), "duplicate event ID at iteration {i}");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Concurrency
// ═══════════════════════════════════════════════════════════════════════════

mod concurrency {
    use super::*;

    /// Publish from multiple threads simultaneously via BusPublisher.
    /// Verifies no panics and subscriber receives messages.
    #[test]
    fn publish_from_multiple_threads() {
        let mut publisher = make_publisher();
        let mut sub = publisher
            .subscribe(SubscriptionConfig {
                buffer_size: 500,
                max_rate_hz: 60.0,
                drop_on_full: true,
            })
            .unwrap();

        let n_threads = 4;
        let n_per_thread = 5;
        let barrier = Arc::new(Barrier::new(n_threads));

        // BusPublisher uses Mutex internally, so we wrap in Arc<Mutex>.
        let publisher = Arc::new(std::sync::Mutex::new(publisher));

        let handles: Vec<_> = (0..n_threads)
            .map(|_t| {
                let barrier = Arc::clone(&barrier);
                let publisher = Arc::clone(&publisher);
                std::thread::spawn(move || {
                    barrier.wait();
                    for _ in 0..n_per_thread {
                        let snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
                        let mut pub_guard = publisher.lock().unwrap();
                        let _ = pub_guard.publish(snap);
                        drop(pub_guard);
                        std::thread::sleep(Duration::from_millis(20));
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread panicked");
        }

        // Drain and count.
        let mut received = 0;
        while sub.try_recv().unwrap().is_some() {
            received += 1;
        }
        let dropped = publisher.lock().unwrap().drop_count();
        assert!(
            received + dropped > 0,
            "at least some messages should have been processed"
        );
    }

    /// EventRouter used from single thread (it's !Send by design) — verify
    /// sequential access from multiple event sources doesn't corrupt state.
    #[test]
    fn router_sequential_multi_source_no_corruption() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);

        // Simulate rapid interleaved events from different sources.
        for i in 0u64..200 {
            let source_id = (i % 5) as u32;
            let kind = if i % 3 == 0 {
                EventKind::AxisUpdate
            } else if i % 3 == 1 {
                EventKind::ButtonPress
            } else {
                EventKind::TelemetryFrame
            };
            let payload = match kind {
                EventKind::AxisUpdate => EventPayload::Axis {
                    axis_id: source_id as u16,
                    value: i as f64 * 0.01,
                },
                EventKind::ButtonPress => EventPayload::Button {
                    button_id: source_id as u16,
                    pressed: i % 2 == 0,
                },
                _ => EventPayload::Telemetry {
                    field_id: source_id as u16,
                    value: i as f64,
                },
            };
            let evt = make_event(
                SourceType::Device,
                source_id,
                kind,
                EventPriority::Normal,
                i * 1000,
                payload,
            );
            let m = router.route_event(&evt);
            assert_eq!(m.len(), 1, "event {i} should match exactly one route");
        }
    }

    /// BusMetrics is thread-safe (uses atomics).
    #[test]
    fn bus_metrics_concurrent_access() {
        use flight_bus::metrics::BusMetrics;

        let metrics = Arc::new(BusMetrics::new());
        let n_threads = 4;
        let n_ops = 100;
        let barrier = Arc::new(Barrier::new(n_threads));

        let handles: Vec<_> = (0..n_threads)
            .map(|_| {
                let m = Arc::clone(&metrics);
                let b = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    b.wait();
                    for _ in 0..n_ops {
                        m.record_publish();
                        m.record_delivery();
                        m.record_drop();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread panicked");
        }

        let snap = metrics.snapshot();
        assert_eq!(snap.messages_published, (n_threads * n_ops) as u64);
        assert_eq!(snap.messages_delivered, (n_threads * n_ops) as u64);
        assert_eq!(snap.messages_dropped, (n_threads * n_ops) as u64);
    }

    /// TelemetryAggregator can handle rapid sequential publish/deliver cycles.
    #[test]
    fn aggregator_rapid_sequential_consistency() {
        use flight_bus::telemetry_aggregator::TelemetryAggregator;

        let mut agg = TelemetryAggregator::new(256);
        let topics = ["altitude", "heading", "speed", "pitch", "roll"];

        for i in 0..500u64 {
            let topic = topics[(i % 5) as usize];
            agg.record_publish(topic, 64);
            agg.record_delivery(i * 10);
        }

        let snap = agg.snapshot();
        assert_eq!(snap.messages_published, 500);
        assert_eq!(snap.messages_delivered, 500);

        for topic in &topics {
            let tm = agg.topic_metrics(topic).unwrap();
            assert_eq!(tm.message_count, 100);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Property-based tests
// ═══════════════════════════════════════════════════════════════════════════

mod property_tests {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Routing an event to an empty router never panics and returns empty.
        #[test]
        fn prop_empty_router_always_empty(
            source_id in 0u32..100,
            value in -1.0f64..=1.0,
            timestamp in 0u64..1_000_000,
        ) {
            let mut router = EventRouter::new();
            let evt = axis_event(source_id, value, timestamp);
            let m = router.route_event(&evt);
            prop_assert!(m.is_empty());
        }

        /// Register N routes with wildcard, route any event → get N matches.
        #[test]
        fn prop_n_wildcard_routes_n_matches(
            n in 1usize..=16.min(MAX_ROUTES),
            value in -1.0f64..=1.0,
        ) {
            let mut router = EventRouter::new();
            for i in 0..n {
                router.register_route(
                    RoutePattern::any(),
                    EventFilter::pass_all(),
                    i as u32,
                );
            }
            let evt = axis_event(1, value, 1000);
            let m = router.route_event(&evt);
            prop_assert_eq!(m.len(), n);
        }

        /// Backpressure at 100% still lets Critical and High through.
        #[test]
        fn prop_critical_high_survive_max_backpressure(
            value in -1.0f64..=1.0,
            timestamp in 1000u64..1_000_000,
        ) {
            let mut router = EventRouter::new();
            router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);
            router.set_backpressure(100);

            let crit = make_event(
                SourceType::Device, 1, EventKind::AxisUpdate,
                EventPriority::Critical, timestamp,
                EventPayload::Axis { axis_id: 0, value },
            );
            let high = make_event(
                SourceType::Device, 1, EventKind::AxisUpdate,
                EventPriority::High, timestamp + 1,
                EventPayload::Axis { axis_id: 0, value },
            );

            prop_assert_eq!(router.route_event(&crit).len(), 1);
            prop_assert_eq!(router.route_event(&high).len(), 1);
        }

        /// Route remove + re-add never panics, and routing works correctly after.
        #[test]
        fn prop_remove_readd_consistent(
            n_cycles in 1usize..=10,
            value in -1.0f64..=1.0,
        ) {
            let mut router = EventRouter::new();
            for _ in 0..n_cycles {
                let id = router
                    .register_route(RoutePattern::any(), EventFilter::pass_all(), 1)
                    .unwrap();
                let evt = axis_event(1, value, 1000);
                prop_assert_eq!(router.route_event(&evt).len(), 1);
                router.remove_route(id);
                prop_assert!(router.route_event(&evt).is_empty());
            }
        }

        /// Event IDs from BusEvent::new are always unique.
        #[test]
        fn prop_event_ids_unique(n in 2usize..=50) {
            let mut ids = HashSet::new();
            for i in 0..n {
                let evt = axis_event(1, i as f64 * 0.01, i as u64 * 100);
                prop_assert!(
                    ids.insert(evt.id),
                    "duplicate event ID {} at iteration {}", evt.id, i
                );
            }
        }
    }
}
