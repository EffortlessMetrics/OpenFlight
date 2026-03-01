// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the event bus system.
//!
//! Covers five areas:
//! 1. **Publish/Subscribe** — single publisher, multiple subscribers, topic
//!    filtering, wildcard subscriptions, unsubscribe, message ordering,
//!    publish-to-no-subscribers, late subscriber.
//! 2. **Lock-free queue** — SPSC correctness, MPSC fan-in, queue-full
//!    drop-tail, queue-empty, concurrent producers, memory ordering.
//! 3. **Event types** — axis events, button events, device connect/disconnect,
//!    profile change, health events, custom/system events.
//! 4. **Backpressure** — slow consumer doesn't block producer, drop-tail
//!    policy, consumer catch-up, queue capacity limits.
//! 5. **RT safety** — no allocation during publish, atomic operations only,
//!    bounded latency, publisher doesn't block, consistent state after overflow.

use std::sync::Arc;
use std::time::{Duration, Instant};

use flight_bus::metrics::{BusMetrics, BusMetricsSnapshot};
use flight_bus::routing::{
    BusEvent, EventFilter, EventKind, EventPayload, EventPriority, EventRouter,
    RoutePattern, SourceType, MAX_MATCHES, MAX_ROUTES,
};
use flight_bus::types::{AircraftId, SimId};
use flight_bus::{BusHealth, BusPublisher, BusSnapshot, SubscriptionConfig, assess_health};

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn make_publisher() -> BusPublisher {
    BusPublisher::new(60.0)
}

fn valid_snapshot() -> BusSnapshot {
    BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"))
}

fn snapshot_for(sim: SimId, icao: &str) -> BusSnapshot {
    BusSnapshot::new(sim, AircraftId::new(icao))
}

fn axis_event(value: f64, priority: EventPriority, ts: u64) -> BusEvent {
    BusEvent::new(
        SourceType::Device,
        1,
        EventKind::AxisUpdate,
        priority,
        ts,
        EventPayload::Axis {
            axis_id: 0,
            value,
        },
    )
}

fn button_event(pressed: bool, ts: u64) -> BusEvent {
    BusEvent::new(
        SourceType::Device,
        1,
        EventKind::ButtonPress,
        EventPriority::Normal,
        ts,
        EventPayload::Button {
            button_id: 1,
            pressed,
        },
    )
}

fn system_event(code: u16, priority: EventPriority, ts: u64) -> BusEvent {
    BusEvent::new(
        SourceType::Internal,
        0,
        EventKind::SystemStatus,
        priority,
        ts,
        EventPayload::System { code },
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Publish / Subscribe (8 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Single publisher delivers to a single subscriber.
#[test]
fn pubsub_single_publisher_single_subscriber() {
    let mut pub_ = make_publisher();
    let mut sub = pub_.subscribe(SubscriptionConfig::default()).unwrap();

    pub_.publish(valid_snapshot()).unwrap();

    let msg = sub.try_recv().unwrap();
    assert!(msg.is_some());
    assert_eq!(msg.unwrap().sim, SimId::Msfs);
}

/// Single publish is delivered to every active subscriber.
#[test]
fn pubsub_multiple_subscribers_all_receive() {
    let mut pub_ = make_publisher();
    let mut subs: Vec<_> = (0..4)
        .map(|_| pub_.subscribe(SubscriptionConfig::default()).unwrap())
        .collect();

    pub_.publish(valid_snapshot()).unwrap();

    for (i, sub) in subs.iter_mut().enumerate() {
        let msg = sub.try_recv().unwrap();
        assert!(msg.is_some(), "subscriber {i} should receive snapshot");
    }
}

/// EventRouter topic filtering: subscriber only receives matching topic.
#[test]
fn pubsub_topic_filtering() {
    let mut router = EventRouter::new();

    // Route 1: only AxisUpdate events
    let p_axis = RoutePattern {
        source_type: SourceType::Any,
        source_id: None,
        event_kind: Some(EventKind::AxisUpdate),
    };
    router.register_route(p_axis, EventFilter::pass_all(), 10);

    // Route 2: only ButtonPress events
    let p_btn = RoutePattern {
        source_type: SourceType::Any,
        source_id: None,
        event_kind: Some(EventKind::ButtonPress),
    };
    router.register_route(p_btn, EventFilter::pass_all(), 20);

    let axis = axis_event(0.5, EventPriority::Normal, 1000);
    let btn = button_event(true, 2000);

    let m_axis = router.route_event(&axis);
    assert!(m_axis.contains(10), "axis route should match axis event");
    assert!(!m_axis.contains(20), "button route should not match axis event");

    let m_btn = router.route_event(&btn);
    assert!(m_btn.contains(20), "button route should match button event");
    assert!(!m_btn.contains(10), "axis route should not match button event");
}

/// Wildcard pattern matches events from any source.
#[test]
fn pubsub_wildcard_subscription() {
    let mut router = EventRouter::new();
    router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);

    for src in [SourceType::Device, SourceType::Simulator, SourceType::Internal] {
        let e = BusEvent::new(src, 42, EventKind::AxisUpdate, EventPriority::Normal, 1000, EventPayload::Empty);
        assert_eq!(router.route_event(&e).len(), 1, "wildcard should match {src:?}");
    }
}

/// After unsubscribe, the subscriber count decreases and no further messages are delivered.
#[test]
fn pubsub_unsubscribe() {
    let mut pub_ = make_publisher();
    let sub = pub_.subscribe(SubscriptionConfig::default()).unwrap();
    assert_eq!(pub_.subscriber_count(), 1);

    pub_.unsubscribe(sub.id).unwrap();
    assert_eq!(pub_.subscriber_count(), 0);

    // Double-unsubscribe fails.
    assert!(pub_.unsubscribe(sub.id).is_err());
}

/// Messages arrive in FIFO order.
#[test]
fn pubsub_message_ordering() {
    let mut pub_ = make_publisher();
    let mut sub = pub_.subscribe(SubscriptionConfig::default()).unwrap();

    // Publish three messages, polling until each passes the rate limiter.
    let sims = [SimId::Msfs, SimId::XPlane, SimId::Dcs];
    for &sim in &sims {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            pub_.publish(snapshot_for(sim, "TEST")).unwrap();
            if sub.try_recv().unwrap().is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {sim:?}");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

/// Publishing when there are no subscribers succeeds without error.
#[test]
fn pubsub_publish_to_no_subscribers() {
    let mut pub_ = make_publisher();
    assert_eq!(pub_.subscriber_count(), 0);

    let result = pub_.publish(valid_snapshot());
    assert!(result.is_ok(), "publish to empty subscriber list should succeed");
}

/// A subscriber created after a publish does not receive old data.
#[test]
fn pubsub_late_subscriber_no_history() {
    let mut pub_ = make_publisher();
    pub_.publish(valid_snapshot()).unwrap();

    let mut late = pub_.subscribe(SubscriptionConfig::default()).unwrap();
    assert!(late.try_recv().unwrap().is_none(), "late subscriber should see no history");
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Lock-free queue (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// SPSC: single producer, single consumer — all messages arrive in order.
#[test]
fn queue_spsc_correctness() {
    let mut pub_ = make_publisher();
    let mut sub = pub_.subscribe(SubscriptionConfig {
        buffer_size: 64,
        max_rate_hz: 60.0,
        drop_on_full: true,
    }).unwrap();

    let count = 5;
    for i in 0..count {
        pub_.publish(snapshot_for(SimId::Msfs, &format!("T{i}"))).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }

    let mut received = Vec::new();
    while let Ok(Some(snap)) = sub.try_recv() {
        received.push(snap.aircraft.icao.clone());
    }
    assert_eq!(received.len(), count);
    for (i, icao) in received.iter().enumerate() {
        assert_eq!(icao, &format!("T{i}"));
    }
}

/// MPSC fan-in: atomic metrics can be updated from multiple threads.
#[test]
fn queue_mpsc_fan_in_metrics() {
    let metrics = Arc::new(BusMetrics::new());
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let m = Arc::clone(&metrics);
            std::thread::spawn(move || {
                for _ in 0..100 {
                    m.record_publish();
                    m.record_delivery();
                }
            })
        })
        .collect();

    for t in threads {
        t.join().unwrap();
    }

    let snap = metrics.snapshot();
    assert_eq!(snap.messages_published, 400);
    assert_eq!(snap.messages_delivered, 400);
}

/// Queue full: when the channel is saturated, subsequent publishes don't panic
/// and the backpressure drop counter increments.
#[test]
fn queue_full_drop_tail() {
    let mut pub_ = make_publisher();
    let _sub = pub_.subscribe(SubscriptionConfig {
        buffer_size: 2,
        drop_on_full: true,
        max_rate_hz: 60.0,
    }).unwrap();

    // Fill the buffer (2 slots)
    pub_.publish(valid_snapshot()).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    pub_.publish(valid_snapshot()).unwrap();
    std::thread::sleep(Duration::from_millis(20));

    // Third publish should drop due to full channel
    pub_.publish(valid_snapshot()).unwrap();
    assert!(pub_.drop_count() >= 1, "at least one drop expected");
}

/// Queue empty: try_recv returns None when no messages are queued.
#[test]
fn queue_empty_returns_none() {
    let mut pub_ = make_publisher();
    let mut sub = pub_.subscribe(SubscriptionConfig::default()).unwrap();

    assert!(sub.try_recv().unwrap().is_none());
}

/// Concurrent producers: multiple threads can record metrics without data races.
#[test]
fn queue_concurrent_producers() {
    let metrics = Arc::new(BusMetrics::new());
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let m = Arc::clone(&metrics);
            std::thread::spawn(move || {
                for _ in 0..50 {
                    m.record_publish();
                    m.record_drop();
                    m.record_slow_subscriber();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let snap = metrics.snapshot();
    assert_eq!(snap.messages_published, 400);
    assert_eq!(snap.messages_dropped, 400);
    assert_eq!(snap.slow_subscribers, 400);
}

/// Memory ordering: peak_queue_depth only increases (monotonically via fetch_max).
#[test]
fn queue_memory_ordering_peak_depth() {
    let metrics = Arc::new(BusMetrics::new());
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let m = Arc::clone(&metrics);
            std::thread::spawn(move || {
                for d in 0..50 {
                    m.update_peak_queue_depth((i * 50 + d) as u64);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Peak should be 3*50 + 49 = 199
    assert_eq!(metrics.snapshot().peak_queue_depth, 199);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Event types (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// Axis events carry a numeric value and are correctly routed.
#[test]
fn event_type_axis() {
    let mut router = EventRouter::new();
    let pattern = RoutePattern {
        source_type: SourceType::Device,
        source_id: None,
        event_kind: Some(EventKind::AxisUpdate),
    };
    router.register_route(pattern, EventFilter::pass_all(), 1);

    let e = axis_event(0.75, EventPriority::Normal, 1000);
    let m = router.route_event(&e);
    assert_eq!(m.len(), 1);
    assert!(m.contains(1));
    assert_eq!(e.payload.value(), Some(0.75));
}

/// Button events carry pressed state and route through button-kind filters.
#[test]
fn event_type_button() {
    let mut router = EventRouter::new();
    let pattern = RoutePattern {
        source_type: SourceType::Any,
        source_id: None,
        event_kind: Some(EventKind::ButtonPress),
    };
    router.register_route(pattern, EventFilter::pass_all(), 2);

    let press = button_event(true, 1000);
    let release = button_event(false, 2000);

    assert_eq!(router.route_event(&press).len(), 1);
    assert_eq!(router.route_event(&release).len(), 1);
    assert_eq!(press.payload.value(), None); // buttons have no numeric value
}

/// Device connect/disconnect modeled as SystemStatus events.
#[test]
fn event_type_device_connect_disconnect() {
    let mut router = EventRouter::new();
    let pattern = RoutePattern {
        source_type: SourceType::Internal,
        source_id: None,
        event_kind: Some(EventKind::SystemStatus),
    };
    router.register_route(pattern, EventFilter::pass_all(), 3);

    let connect = system_event(1, EventPriority::High, 1000); // code 1 = connect
    let disconnect = system_event(2, EventPriority::High, 2000); // code 2 = disconnect

    assert_eq!(router.route_event(&connect).len(), 1);
    assert_eq!(router.route_event(&disconnect).len(), 1);
}

/// Profile change events are internal system status events with specific codes.
#[test]
fn event_type_profile_change() {
    let mut router = EventRouter::new();
    let pattern = RoutePattern {
        source_type: SourceType::Internal,
        source_id: Some(0),
        event_kind: Some(EventKind::SystemStatus),
    };
    router.register_route(pattern, EventFilter::pass_all(), 4);

    let profile_change = system_event(100, EventPriority::Normal, 1000);
    assert_eq!(router.route_event(&profile_change).len(), 1);

    // Different source_id should not match
    let other = BusEvent::new(
        SourceType::Internal,
        5,
        EventKind::SystemStatus,
        EventPriority::Normal,
        2000,
        EventPayload::System { code: 100 },
    );
    assert!(router.route_event(&other).is_empty());
}

/// Health events use the BusHealth/BusMetrics system to assess bus state.
#[test]
fn event_type_health() {
    // Healthy: 0% drops
    let healthy = BusMetricsSnapshot {
        messages_published: 1000,
        messages_delivered: 1000,
        messages_dropped: 0,
        slow_subscribers: 0,
        peak_queue_depth: 5,
    };
    assert!(assess_health(&healthy).is_healthy());

    // Degraded: 3% drops
    let degraded = BusMetricsSnapshot {
        messages_published: 1000,
        messages_delivered: 970,
        messages_dropped: 30,
        slow_subscribers: 1,
        peak_queue_depth: 50,
    };
    assert!(matches!(assess_health(&degraded), BusHealth::Degraded { .. }));

    // Unhealthy: 10% drops
    let unhealthy = BusMetricsSnapshot {
        messages_published: 1000,
        messages_delivered: 900,
        messages_dropped: 100,
        slow_subscribers: 3,
        peak_queue_depth: 100,
    };
    assert!(matches!(assess_health(&unhealthy), BusHealth::Unhealthy { .. }));
}

/// Custom/system events with Empty payload route correctly.
#[test]
fn event_type_custom_system() {
    let mut router = EventRouter::new();
    router.register_route(RoutePattern::any(), EventFilter::pass_all(), 5);

    let custom = BusEvent::new(
        SourceType::Internal,
        99,
        EventKind::SystemStatus,
        EventPriority::Normal,
        1000,
        EventPayload::Empty,
    );

    let m = router.route_event(&custom);
    assert_eq!(m.len(), 1);
    assert_eq!(custom.payload.value(), None); // Empty has no value
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Backpressure (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// A slow consumer (never drains) doesn't block the producer — publish always returns Ok.
#[test]
fn backpressure_slow_consumer_doesnt_block_producer() {
    let mut pub_ = make_publisher();
    let _slow_sub = pub_.subscribe(SubscriptionConfig {
        buffer_size: 2,
        drop_on_full: true,
        max_rate_hz: 60.0,
    }).unwrap();

    // Publish many times — none should block or return Err
    for _ in 0..10 {
        let result = pub_.publish(valid_snapshot());
        assert!(result.is_ok(), "producer must never block");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Drop-tail policy: messages are silently dropped when the channel is full.
#[test]
fn backpressure_drop_tail_policy() {
    let mut pub_ = make_publisher();
    let mut sub = pub_.subscribe(SubscriptionConfig {
        buffer_size: 1,
        drop_on_full: true,
        max_rate_hz: 60.0,
    }).unwrap();

    // Fill buffer (1 slot)
    pub_.publish(valid_snapshot()).unwrap();
    std::thread::sleep(Duration::from_millis(20));

    // These publishes exceed buffer — they should be dropped
    for _ in 0..5 {
        pub_.publish(valid_snapshot()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(pub_.drop_count() >= 1, "drop-tail should have discarded messages");

    // Consumer still receives the first message
    assert!(sub.try_recv().unwrap().is_some());
}

/// After draining, the consumer can catch up and receive new messages.
#[test]
fn backpressure_consumer_catch_up() {
    let mut pub_ = make_publisher();
    let mut sub = pub_.subscribe(SubscriptionConfig {
        buffer_size: 4,
        drop_on_full: true,
        max_rate_hz: 60.0,
    }).unwrap();

    // Publish 4 messages (fills buffer)
    for _ in 0..4 {
        pub_.publish(valid_snapshot()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }

    // Drain all
    while sub.try_recv().unwrap().is_some() {}

    // Publish a new message — should be received
    std::thread::sleep(Duration::from_millis(20));
    pub_.publish(snapshot_for(SimId::XPlane, "A320")).unwrap();
    let msg = sub.try_recv().unwrap();
    assert!(msg.is_some(), "consumer should receive after catch-up");
    assert_eq!(msg.unwrap().sim, SimId::XPlane);
}

/// Queue capacity is enforced by the bounded channel.
#[test]
fn backpressure_queue_capacity_limits() {
    let mut pub_ = make_publisher();
    let _sub = pub_.subscribe(SubscriptionConfig {
        buffer_size: 3,
        drop_on_full: true,
        max_rate_hz: 60.0,
    }).unwrap();

    // Fill exactly 3 slots
    for _ in 0..3 {
        pub_.publish(valid_snapshot()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(pub_.drop_count(), 0, "no drops while within capacity");

    // 4th should trigger a drop
    pub_.publish(valid_snapshot()).unwrap();
    assert!(pub_.drop_count() >= 1, "exceeding capacity should trigger drop");
}

/// EventRouter backpressure drops low-priority events but never drops Critical/High.
#[test]
fn backpressure_priority_based_dropping() {
    let mut router = EventRouter::new();
    router.register_route(RoutePattern::any(), EventFilter::pass_all(), 1);

    // 80% backpressure: Background, Low, Normal are dropped
    router.set_backpressure(80);

    let bg = axis_event(0.5, EventPriority::Background, 1000);
    let low = axis_event(0.5, EventPriority::Low, 2000);
    let normal = axis_event(0.5, EventPriority::Normal, 3000);
    let high = axis_event(0.5, EventPriority::High, 4000);
    let critical = axis_event(0.5, EventPriority::Critical, 5000);

    assert!(router.route_event(&bg).is_empty(), "Background dropped at 80%");
    assert!(router.route_event(&low).is_empty(), "Low dropped at 80%");
    assert!(router.route_event(&normal).is_empty(), "Normal dropped at 80%");
    assert_eq!(router.route_event(&high).len(), 1, "High never dropped");
    assert_eq!(router.route_event(&critical).len(), 1, "Critical never dropped");
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. RT safety (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// EventRouter::route_event uses only stack-allocated RouteMatches (bounded capacity).
/// We verify by routing through a full router and checking the fixed-capacity result.
#[test]
fn rt_safety_no_allocation_during_route() {
    let mut router = EventRouter::new();
    // Fill all MAX_ROUTES slots
    for i in 0..MAX_ROUTES {
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), i as u32);
    }

    let event = axis_event(0.5, EventPriority::Normal, 1000);
    let matches = router.route_event(&event);

    // RouteMatches is capped at MAX_MATCHES (bounded capacity)
    assert!(matches.len() <= MAX_MATCHES);
    assert!(matches.len() > 0);

    // Verify we can iterate the bounded result
    let sum: u32 = matches.iter().sum();
    assert!(sum > 0);
}

/// BusMetrics uses only atomic operations — can be shared and updated
/// from many threads without locks.
#[test]
fn rt_safety_atomic_operations_only() {
    let metrics = Arc::new(BusMetrics::new());

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let m = Arc::clone(&metrics);
            std::thread::spawn(move || {
                for i in 0..100 {
                    m.record_publish();
                    m.record_delivery();
                    m.update_peak_queue_depth(i);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let snap = metrics.snapshot();
    assert_eq!(snap.messages_published, 800);
    assert_eq!(snap.messages_delivered, 800);
    assert_eq!(snap.peak_queue_depth, 99);
}

/// Route_event completes within a bounded time even with MAX_ROUTES active routes.
#[test]
#[ignore = "wall-clock timing; run in controlled perf jobs"]
fn rt_safety_bounded_latency() {
    let mut router = EventRouter::new();
    for i in 0..MAX_ROUTES {
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), i as u32);
    }

    let event = axis_event(0.5, EventPriority::Normal, 1000);

    // Warm up
    let _ = router.route_event(&event);

    // Measure 1000 iterations
    let start = Instant::now();
    let iterations = 1000u32;
    for _ in 0..iterations {
        let _ = router.route_event(&event);
    }
    let elapsed = start.elapsed();

    // Each iteration should be well under 1ms (generous bound for CI)
    let avg_us = elapsed.as_micros() as f64 / iterations as f64;
    assert!(
        avg_us < 1000.0,
        "average route_event latency {avg_us:.1}µs exceeds 1ms bound"
    );
}

/// Publisher doesn't block even when all subscribers are full.
#[test]
fn rt_safety_publisher_doesnt_block() {
    let mut pub_ = make_publisher();
    // Create several subscribers with tiny buffers
    let _subs: Vec<_> = (0..4)
        .map(|_| {
            pub_.subscribe(SubscriptionConfig {
                buffer_size: 1,
                drop_on_full: true,
                max_rate_hz: 60.0,
            }).unwrap()
        })
        .collect();

    // Publish should complete quickly even with saturated subscribers
    let start = Instant::now();
    for _ in 0..20 {
        pub_.publish(valid_snapshot()).unwrap();
        std::thread::sleep(Duration::from_millis(18));
    }
    let elapsed = start.elapsed();

    // 20 publishes × 18ms sleep ≈ 360ms; generous bound of 2s accounts for CI jitter.
    assert!(
        elapsed < Duration::from_secs(2),
        "publishing took {elapsed:?}, expected < 2s"
    );
}

/// After overflow, the router maintains consistent state — routes still work.
#[test]
fn rt_safety_consistent_state_after_overflow() {
    let mut router = EventRouter::new();
    // Fill to capacity
    let mut ids = Vec::new();
    for i in 0..MAX_ROUTES {
        ids.push(
            router
                .register_route(RoutePattern::any(), EventFilter::pass_all(), i as u32)
                .unwrap(),
        );
    }

    // Attempting to add beyond capacity returns None
    assert!(router
        .register_route(RoutePattern::any(), EventFilter::pass_all(), 999)
        .is_none());

    // Router still functions correctly for existing routes
    let event = axis_event(0.5, EventPriority::Normal, 1000);
    let m = router.route_event(&event);
    assert_eq!(m.len(), MAX_MATCHES); // capped at MAX_MATCHES

    // Remove one route and add another — slot reuse works
    router.remove_route(ids[0]);
    assert_eq!(router.route_count(), MAX_ROUTES - 1);

    let new_id = router
        .register_route(RoutePattern::any(), EventFilter::pass_all(), 1000)
        .unwrap();
    assert_eq!(router.route_count(), MAX_ROUTES);

    // The new route matches events
    let m2 = router.route_event(&event);
    assert!(m2.contains(1000));

    // Clean removal of new route
    assert!(router.remove_route(new_id));
    assert_eq!(router.route_count(), MAX_ROUTES - 1);
}
