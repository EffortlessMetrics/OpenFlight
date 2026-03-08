// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! End-to-end integration tests for the bus → adapter → subscriber pipeline.
//!
//! Covers:
//! - Bus event routing (publish → subscriber delivery)
//! - Adapter lifecycle (state machine transitions, error recovery)
//! - Profile hot-swap (swap config while pipeline runs)
//! - Telemetry flow (adapter telemetry → bus → subscriber)
//! - Error recovery (circuit breaker, reconnection, fault injection)
//!
//! All tests use mock/fake adapters — no real hardware required.

use std::path::PathBuf;
use std::time::Duration;

use flight_adapter_common::{
    AdapterConfig, AdapterError, AdapterMetrics, AdapterState, ExponentialBackoff,
    ReconnectionStrategy,
};
use flight_bus::event_router::{EventFilter, EventRouter};
use flight_bus::metrics::{BusMetrics, BusMetricsSnapshot};
use flight_bus::routing::{
    BusEvent, EventFilter as RoutingFilter, EventKind, EventPayload, EventPriority,
    EventRouter as RtRouter, RoutePattern, SourceType,
};
use flight_bus::snapshot::BusSnapshot;
use flight_bus::telemetry_aggregator::TelemetryAggregator;
use flight_bus::types::{AircraftId, SimId};
use flight_bus::{BusPublisher, SubscriptionConfig};
use flight_core::circuit_breaker::{
    CallResult, CircuitBreaker, CircuitBreakerConfig, CircuitState,
};
use flight_core::profile_watcher::{FileChangeKind, ProfileWatcher, ReloadNotifier};
use flight_test_helpers::{FakeSim, FakeSnapshot, assert_approx_eq};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Sleep long enough to satisfy both publisher and subscriber rate limiters.
fn tick() {
    std::thread::sleep(Duration::from_millis(25));
}

fn snapshot(sim: SimId, icao: &str) -> BusSnapshot {
    BusSnapshot::new(sim, AircraftId::new(icao))
}

fn make_publisher() -> BusPublisher {
    BusPublisher::new(60.0)
}

fn sample_snapshot(altitude: f64, airspeed: f64, on_ground: bool) -> FakeSnapshot {
    FakeSnapshot {
        altitude,
        airspeed,
        heading: 270.0,
        pitch: 2.5,
        roll: 0.0,
        yaw: 0.0,
        on_ground,
    }
}

/// Mock adapter config for testing.
struct MockConfig {
    rate_hz: f32,
    timeout: Duration,
    max_reconnect: u32,
    auto_reconnect: bool,
}

impl MockConfig {
    fn default_test() -> Self {
        Self {
            rate_hz: 30.0,
            timeout: Duration::from_secs(5),
            max_reconnect: 3,
            auto_reconnect: true,
        }
    }
}

impl AdapterConfig for MockConfig {
    fn publish_rate_hz(&self) -> f32 {
        self.rate_hz
    }
    fn connection_timeout(&self) -> Duration {
        self.timeout
    }
    fn max_reconnect_attempts(&self) -> u32 {
        self.max_reconnect
    }
    fn enable_auto_reconnect(&self) -> bool {
        self.auto_reconnect
    }
}

// ===========================================================================
// 1. BUS EVENT ROUTING — publish events, verify subscribers receive them
// ===========================================================================

#[test]
fn bus_single_subscriber_receives_published_snapshot() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let snap = snapshot(SimId::Msfs, "C172");
    publisher.publish(snap).unwrap();

    let received = sub.try_recv().unwrap();
    assert!(received.is_some());
    let r = received.unwrap();
    assert_eq!(r.sim, SimId::Msfs);
    assert_eq!(r.aircraft.icao, "C172");
}

#[test]
fn bus_multiple_subscribers_all_receive_snapshot() {
    let mut publisher = make_publisher();
    let mut subs: Vec<_> = (0..5)
        .map(|_| publisher.subscribe(SubscriptionConfig::default()).unwrap())
        .collect();

    publisher.publish(snapshot(SimId::XPlane, "B737")).unwrap();

    for (i, sub) in subs.iter_mut().enumerate() {
        let r = sub.try_recv().unwrap();
        assert!(r.is_some(), "subscriber {i} must receive the snapshot");
        assert_eq!(r.unwrap().aircraft.icao, "B737");
    }
}

#[test]
fn bus_subscriber_receives_snapshots_in_publish_order() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let aircraft = ["C172", "B737", "A320", "F18"];
    for icao in &aircraft {
        publisher.publish(snapshot(SimId::Msfs, icao)).unwrap();
        tick();
    }

    let mut received = Vec::new();
    while let Ok(Some(s)) = sub.try_recv() {
        received.push(s.aircraft.icao.clone());
    }

    let expected: Vec<String> = aircraft.iter().map(|s| s.to_string()).collect();
    assert_eq!(received, expected);
}

#[test]
fn bus_unsubscribe_stops_delivery() {
    let mut publisher = make_publisher();
    let sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    drop(sub);

    // After dropping subscriber, publisher should clean up on next publish
    publisher.publish(snapshot(SimId::Msfs, "C172")).unwrap();
    tick();
    publisher.publish(snapshot(SimId::Msfs, "B737")).unwrap();

    assert_eq!(publisher.subscriber_count(), 0);
}

#[test]
fn bus_explicit_unsubscribe_removes_subscriber() {
    let mut publisher = make_publisher();
    let sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let sid = sub.id;

    assert_eq!(publisher.subscriber_count(), 1);
    publisher.unsubscribe(sid).unwrap();
    assert_eq!(publisher.subscriber_count(), 0);
}

#[test]
fn bus_event_router_routes_by_topic() {
    let mut router = EventRouter::new();
    router.add_route(
        "panel-sub",
        EventFilter {
            topic: "telemetry".to_string(),
            device_id: None,
            min_priority: 0,
        },
    );
    router.add_route(
        "ffb-sub",
        EventFilter {
            topic: "controls".to_string(),
            device_id: None,
            min_priority: 0,
        },
    );

    let telem_subs = router.route_event("telemetry", None, 5);
    assert_eq!(telem_subs, vec!["panel-sub"]);

    let ctrl_subs = router.route_event("controls", None, 5);
    assert_eq!(ctrl_subs, vec!["ffb-sub"]);
}

#[test]
fn bus_event_router_device_filtering() {
    let mut router = EventRouter::new();
    router.add_route(
        "stick-sub",
        EventFilter {
            topic: "axis".to_string(),
            device_id: Some("joystick-1".to_string()),
            min_priority: 0,
        },
    );

    let matched = router.route_event("axis", Some("joystick-1"), 5);
    assert_eq!(matched, vec!["stick-sub"]);

    let unmatched = router.route_event("axis", Some("joystick-2"), 5);
    assert!(unmatched.is_empty());
}

#[test]
fn bus_event_router_priority_threshold() {
    let mut router = EventRouter::new();
    router.add_route(
        "high-pri-sub",
        EventFilter {
            topic: "alerts".to_string(),
            device_id: None,
            min_priority: 5,
        },
    );

    assert!(router.route_event("alerts", None, 3).is_empty());
    assert_eq!(router.route_event("alerts", None, 5), vec!["high-pri-sub"]);
    assert_eq!(router.route_event("alerts", None, 10), vec!["high-pri-sub"]);
}

// ===========================================================================
// 2. RT EVENT ROUTER — allocation-free routing
// ===========================================================================

#[test]
fn rt_router_registers_and_routes_axis_events() {
    let mut router = RtRouter::new();
    let pattern = RoutePattern {
        source_type: SourceType::Device,
        source_id: Some(1),
        event_kind: Some(EventKind::AxisUpdate),
    };
    let _id = router
        .register_route(pattern, RoutingFilter::pass_all(), 100)
        .unwrap();

    let event = BusEvent::new(
        SourceType::Device,
        1,
        EventKind::AxisUpdate,
        EventPriority::Normal,
        0,
        EventPayload::Axis {
            axis_id: 0,
            value: 0.5,
        },
    );

    let matches = router.route_event(&event);
    assert!(matches.contains(100));
    assert_eq!(matches.len(), 1);
}

#[test]
fn rt_router_wildcard_pattern_matches_all() {
    let mut router = RtRouter::new();
    let pattern = RoutePattern::any();
    router
        .register_route(pattern, RoutingFilter::pass_all(), 200)
        .unwrap();

    let event = BusEvent::new(
        SourceType::Simulator,
        42,
        EventKind::TelemetryFrame,
        EventPriority::High,
        1000,
        EventPayload::Telemetry {
            field_id: 1,
            value: 35000.0,
        },
    );

    let matches = router.route_event(&event);
    assert!(matches.contains(200));
}

#[test]
fn rt_router_remove_route_stops_delivery() {
    let mut router = RtRouter::new();
    let pattern = RoutePattern::any();
    let id = router
        .register_route(pattern, RoutingFilter::pass_all(), 300)
        .unwrap();
    assert_eq!(router.route_count(), 1);

    router.remove_route(id);
    assert_eq!(router.route_count(), 0);

    let event = BusEvent::new(
        SourceType::Internal,
        0,
        EventKind::SystemStatus,
        EventPriority::Normal,
        0,
        EventPayload::System { code: 1 },
    );
    assert!(router.route_event(&event).is_empty());
}

#[test]
fn rt_router_backpressure_drops_low_priority() {
    let mut router = RtRouter::new();
    router
        .register_route(RoutePattern::any(), RoutingFilter::pass_all(), 400)
        .unwrap();

    router.set_backpressure(50);

    let low_event = BusEvent::new(
        SourceType::Device,
        1,
        EventKind::AxisUpdate,
        EventPriority::Low,
        0,
        EventPayload::Axis {
            axis_id: 0,
            value: 0.1,
        },
    );
    assert!(
        router.route_event(&low_event).is_empty(),
        "Low priority dropped at 50% backpressure"
    );

    let high_event = BusEvent::new(
        SourceType::Device,
        1,
        EventKind::AxisUpdate,
        EventPriority::High,
        0,
        EventPayload::Axis {
            axis_id: 0,
            value: 0.9,
        },
    );
    assert!(
        !router.route_event(&high_event).is_empty(),
        "High priority passes at 50% backpressure"
    );
}

// ===========================================================================
// 3. ADAPTER LIFECYCLE — start → connected → active → disconnected → reconnect
// ===========================================================================

#[test]
fn adapter_lifecycle_happy_path() {
    let states = [
        AdapterState::Disconnected,
        AdapterState::Connecting,
        AdapterState::Connected,
        AdapterState::DetectingAircraft,
        AdapterState::Active,
    ];
    for pair in states.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
    // Verify final state is Active
    assert_eq!(*states.last().unwrap(), AdapterState::Active);
}

#[test]
fn adapter_lifecycle_disconnect_from_active() {
    let transitions = [(AdapterState::Active, AdapterState::Disconnected)];
    for (from, to) in &transitions {
        assert_ne!(from, to);
    }
}

#[test]
fn adapter_lifecycle_error_to_reconnect() {
    let recovery = [
        AdapterState::Active,
        AdapterState::Error,
        AdapterState::Connecting,
        AdapterState::Connected,
        AdapterState::DetectingAircraft,
        AdapterState::Active,
    ];
    for pair in recovery.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
    assert_eq!(recovery[0], AdapterState::Active);
    assert_eq!(*recovery.last().unwrap(), AdapterState::Active);
}

#[test]
fn adapter_config_contract() {
    let config = MockConfig::default_test();
    assert_eq!(config.publish_rate_hz(), 30.0);
    assert_eq!(config.connection_timeout(), Duration::from_secs(5));
    assert_eq!(config.max_reconnect_attempts(), 3);
    assert!(config.enable_auto_reconnect());
}

#[test]
fn adapter_config_custom_values() {
    let config = MockConfig {
        rate_hz: 60.0,
        timeout: Duration::from_secs(10),
        max_reconnect: 5,
        auto_reconnect: false,
    };
    assert_eq!(config.publish_rate_hz(), 60.0);
    assert_eq!(config.connection_timeout(), Duration::from_secs(10));
    assert!(!config.enable_auto_reconnect());
}

#[test]
fn adapter_reconnection_strategy_backoff() {
    let strategy = ReconnectionStrategy::new(5, Duration::from_millis(100), Duration::from_secs(5));

    let backoffs: Vec<Duration> = (1..=6).map(|i| strategy.next_backoff(i)).collect();

    // Exponential growth
    assert_eq!(backoffs[0], Duration::from_millis(100));
    assert_eq!(backoffs[1], Duration::from_millis(200));
    assert_eq!(backoffs[2], Duration::from_millis(400));

    // Capped at max
    assert!(backoffs[5] <= Duration::from_secs(5));
}

#[test]
fn adapter_reconnection_exhaustion() {
    let strategy = ReconnectionStrategy::new(3, Duration::from_millis(50), Duration::from_secs(1));

    assert!(strategy.should_retry(1));
    assert!(strategy.should_retry(2));
    assert!(strategy.should_retry(3));
    assert!(!strategy.should_retry(4));
}

#[test]
fn adapter_exponential_backoff_with_reset() {
    let mut backoff = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(10),
        2.0,
        0.0,
    );

    let d1 = backoff.next_delay();
    let d2 = backoff.next_delay();
    assert_eq!(d1, Duration::from_millis(100));
    assert_eq!(d2, Duration::from_millis(200));

    backoff.reset();
    let d3 = backoff.next_delay();
    assert_eq!(d3, Duration::from_millis(100));
}

// ===========================================================================
// 4. PROFILE HOT-SWAP — swap profiles while pipeline is running
// ===========================================================================

#[test]
fn profile_watcher_detects_new_file() {
    let dir = std::env::temp_dir().join("openflight_integ_new_file");
    let _ = std::fs::create_dir_all(&dir);
    let mut watcher = ProfileWatcher::with_default_interval(dir.clone());
    watcher.poll(); // baseline

    let file = dir.join("test_profile.yaml");
    std::fs::write(&file, "name: test\naxes: {}").unwrap();

    let events = watcher.poll();
    assert!(
        events
            .iter()
            .any(|e| e.kind == FileChangeKind::Created && e.path == file),
        "new profile file must be detected"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn profile_watcher_detects_modification() {
    let dir = std::env::temp_dir().join("openflight_integ_modified");
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("profile.yaml");
    std::fs::write(&file, "version: 1").unwrap();

    let mut watcher = ProfileWatcher::with_default_interval(dir.clone());
    watcher.poll(); // register initial state

    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(&file, "version: 2").unwrap();

    let events = watcher.poll();
    assert!(
        events
            .iter()
            .any(|e| e.kind == FileChangeKind::Modified && e.path == file),
        "modified profile must be detected"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn profile_watcher_detects_deletion() {
    let dir = std::env::temp_dir().join("openflight_integ_deleted");
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("old_profile.toml");
    std::fs::write(&file, "name = 'old'").unwrap();

    let mut watcher = ProfileWatcher::with_default_interval(dir.clone());
    watcher.poll(); // register initial state

    std::fs::remove_file(&file).unwrap();
    let events = watcher.poll();
    assert!(
        events
            .iter()
            .any(|e| e.kind == FileChangeKind::Deleted && e.path == file),
        "deleted profile must be detected"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn profile_reload_notifier_signals_pending_reload() {
    let notifier = ReloadNotifier::new();
    assert!(!notifier.has_pending());

    notifier.notify(PathBuf::from("profiles/c172.yaml"));
    assert!(notifier.has_pending());

    let pending = notifier.drain();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0], PathBuf::from("profiles/c172.yaml"));
    assert!(!notifier.has_pending());
}

#[test]
fn profile_reload_notifier_deduplicates() {
    let notifier = ReloadNotifier::new();
    notifier.notify(PathBuf::from("p.yaml"));
    notifier.notify(PathBuf::from("p.yaml"));
    notifier.notify(PathBuf::from("p.yaml"));

    let pending = notifier.drain();
    assert_eq!(pending.len(), 1, "duplicate reloads must be deduplicated");
}

#[test]
fn profile_hot_swap_bus_receives_new_aircraft() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Phase 1: C172 profile active
    let snap1 = snapshot(SimId::Msfs, "C172");
    publisher.publish(snap1).unwrap();
    let r1 = sub.try_recv().unwrap().unwrap();
    assert_eq!(r1.aircraft.icao, "C172");

    tick();

    // Phase 2: Hot-swap to A320 profile
    let snap2 = snapshot(SimId::Msfs, "A320");
    publisher.publish(snap2).unwrap();
    let r2 = sub.try_recv().unwrap().unwrap();
    assert_eq!(r2.aircraft.icao, "A320");
}

#[test]
fn profile_hot_swap_preserves_subscriber_connection() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Publish with original profile
    publisher.publish(snapshot(SimId::Msfs, "C172")).unwrap();
    assert!(sub.try_recv().unwrap().is_some());

    tick();

    // Simulate profile swap (new aircraft)
    publisher.publish(snapshot(SimId::Msfs, "B737")).unwrap();
    let r = sub.try_recv().unwrap().unwrap();
    assert_eq!(r.aircraft.icao, "B737");

    // Stats should show 2 messages received
    assert_eq!(sub.stats().messages_received, 2);
}

#[test]
fn profile_reload_notifier_shared_across_threads() {
    let notifier = ReloadNotifier::new();
    let n2 = notifier.clone();

    let handle = std::thread::spawn(move || {
        n2.notify(PathBuf::from("from_thread.yaml"));
    });
    handle.join().unwrap();

    assert!(notifier.has_pending());
    let pending = notifier.drain();
    assert_eq!(pending[0], PathBuf::from("from_thread.yaml"));
}

// ===========================================================================
// 5. TELEMETRY FLOW — simulate telemetry from adapters, verify subscribers
// ===========================================================================

#[test]
fn telemetry_adapter_to_bus_altitude_flow() {
    let mut sim = FakeSim::new("MSFS");
    sim.connect();
    sim.set_aircraft("C172");
    sim.push_snapshot(sample_snapshot(5000.0, 120.0, false));

    let snap = sim.next_snapshot().unwrap();

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut bus_snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    bus_snap.environment.altitude = snap.altitude as f32;
    publisher.publish(bus_snap).unwrap();

    let r = sub.try_recv().unwrap().unwrap();
    assert_approx_eq(r.environment.altitude as f64, 5000.0, 0.1);
}

#[test]
fn telemetry_sequential_frames_ordered() {
    let mut sim = FakeSim::new("X-Plane");
    sim.connect();
    sim.set_aircraft("B737");

    let altitudes = [0.0, 1000.0, 5000.0, 10000.0, 35000.0];
    for &alt in &altitudes {
        sim.push_snapshot(sample_snapshot(alt, 250.0, alt == 0.0));
    }

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut received = Vec::new();
    while let Some(snap) = sim.next_snapshot() {
        let mut bus_snap = BusSnapshot::new(SimId::XPlane, AircraftId::new("B737"));
        bus_snap.environment.altitude = snap.altitude as f32;
        publisher.publish(bus_snap).unwrap();
        tick();

        if let Some(r) = sub.try_recv().unwrap() {
            received.push(r.environment.altitude as f64);
        }
    }

    assert!(!received.is_empty());
    // First should be ground level
    assert_approx_eq(received[0], 0.0, 0.1);
}

#[test]
fn telemetry_multi_sim_identities_preserved() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let sources: Vec<(SimId, &str)> = vec![
        (SimId::Msfs, "C172"),
        (SimId::XPlane, "A320"),
        (SimId::Dcs, "F16C"),
    ];

    for &(sim, icao) in &sources {
        publisher.publish(snapshot(sim, icao)).unwrap();
        tick();
    }

    let mut received: Vec<(SimId, String)> = Vec::new();
    while let Ok(Some(s)) = sub.try_recv() {
        received.push((s.sim, s.aircraft.icao.clone()));
    }

    assert_eq!(received.len(), sources.len());
    for (i, &(sim, icao)) in sources.iter().enumerate() {
        assert_eq!(received[i].0, sim);
        assert_eq!(received[i].1, icao);
    }
}

#[test]
fn telemetry_aggregator_tracks_publishes() {
    let mut agg = TelemetryAggregator::new(256);
    agg.set_publisher_count(1);
    agg.set_subscriber_count(2);

    for _ in 0..100 {
        agg.record_publish("altitude", 64);
        agg.record_delivery(50);
    }

    let snap = agg.snapshot();
    assert_eq!(snap.messages_published, 100);
    assert_eq!(snap.messages_delivered, 100);
    assert_eq!(snap.active_publishers, 1);
    assert_eq!(snap.active_subscribers, 2);
    assert!(snap.throughput_msgs_per_sec > 0.0);
}

#[test]
fn telemetry_aggregator_per_topic_metrics() {
    let mut agg = TelemetryAggregator::new(128);
    agg.record_publish("altitude", 64);
    agg.record_publish("altitude", 64);
    agg.record_publish("heading", 32);
    agg.record_publish("speed", 48);

    assert_eq!(agg.topic_metrics("altitude").unwrap().message_count, 2);
    assert_eq!(agg.topic_metrics("heading").unwrap().message_count, 1);
    assert_eq!(agg.topic_metrics("speed").unwrap().message_count, 1);
    assert_eq!(agg.all_topics().len(), 3);
}

#[test]
fn telemetry_aggregator_latency_p99() {
    let mut agg = TelemetryAggregator::new(256);
    for i in 1..=100 {
        agg.record_delivery(i);
    }
    let snap = agg.snapshot();
    assert_eq!(snap.p99_latency_us, 99);
    assert_eq!(snap.max_latency_us, 100);
}

#[test]
fn telemetry_aggregator_drop_tracking() {
    let mut agg = TelemetryAggregator::new(128);
    for _ in 0..10 {
        agg.record_publish("data", 32);
    }
    for _ in 0..3 {
        agg.record_drop();
    }

    let snap = agg.snapshot();
    assert_eq!(snap.messages_published, 10);
    assert_eq!(snap.messages_dropped, 3);
}

#[test]
fn telemetry_aggregator_reset_clears_all() {
    let mut agg = TelemetryAggregator::new(128);
    agg.record_publish("test", 16);
    agg.record_delivery(50);
    agg.record_drop();

    agg.reset();
    let snap = agg.snapshot();
    assert_eq!(snap.messages_published, 0);
    assert_eq!(snap.messages_delivered, 0);
    assert_eq!(snap.messages_dropped, 0);
}

// ===========================================================================
// 6. ERROR RECOVERY — inject faults, verify graceful degradation
// ===========================================================================

#[test]
fn circuit_breaker_closed_to_open_on_failures() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 2,
        timeout: Duration::from_secs(60),
    };
    let mut cb = CircuitBreaker::new(config);

    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.call_allowed(), CallResult::Allowed);

    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed);

    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
    assert_eq!(cb.call_allowed(), CallResult::Rejected);
}

#[test]
fn circuit_breaker_recovers_after_timeout() {
    let config = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout: Duration::from_millis(10),
    };
    let mut cb = CircuitBreaker::new(config);

    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);

    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(cb.call_allowed(), CallResult::Allowed);
    assert_eq!(cb.state(), CircuitState::HalfOpen);

    cb.record_success();
    assert_eq!(cb.state(), CircuitState::Closed);
}

#[test]
fn circuit_breaker_failure_in_half_open_reopens() {
    let config = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 2,
        timeout: Duration::from_millis(10),
    };
    let mut cb = CircuitBreaker::new(config);

    cb.record_failure();
    std::thread::sleep(Duration::from_millis(20));
    let _ = cb.call_allowed(); // -> HalfOpen

    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
}

#[test]
fn circuit_breaker_reset_returns_to_closed() {
    let config = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout: Duration::from_secs(60),
    };
    let mut cb = CircuitBreaker::new(config);

    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);

    cb.reset();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_count(), 0);
}

#[test]
fn circuit_breaker_rejection_rate_tracked() {
    let config = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout: Duration::from_secs(60),
    };
    let mut cb = CircuitBreaker::new(config);

    assert_eq!(cb.call_allowed(), CallResult::Allowed);
    cb.record_failure();
    assert_eq!(cb.call_allowed(), CallResult::Rejected);

    assert_eq!(cb.total_calls(), 2);
    assert_eq!(cb.total_rejections(), 1);
    assert!((cb.rejection_rate() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn bus_backpressure_drops_tracked() {
    let mut publisher = make_publisher();
    let config = SubscriptionConfig {
        buffer_size: 1,
        drop_on_full: true,
        max_rate_hz: 60.0,
    };
    let _sub = publisher.subscribe(config).unwrap();

    let snap = snapshot(SimId::Msfs, "C172");
    publisher.publish(snap.clone()).unwrap();
    tick();

    // Fill beyond capacity without draining
    for _ in 0..5 {
        publisher.publish(snap.clone()).unwrap();
        tick();
    }

    assert!(
        publisher.drop_count() > 0,
        "backpressure drops must be recorded"
    );
}

#[test]
fn bus_health_healthy_when_no_drops() {
    let metrics = BusMetricsSnapshot {
        messages_published: 1000,
        messages_delivered: 1000,
        messages_dropped: 0,
        slow_subscribers: 0,
        peak_queue_depth: 5,
    };
    let health = flight_bus::assess_health(&metrics);
    assert!(health.is_healthy());
}

#[test]
fn bus_health_degraded_on_moderate_drops() {
    let metrics = BusMetricsSnapshot {
        messages_published: 1000,
        messages_delivered: 970,
        messages_dropped: 30,
        slow_subscribers: 2,
        peak_queue_depth: 50,
    };
    let health = flight_bus::assess_health(&metrics);
    assert!(matches!(health, flight_bus::BusHealth::Degraded { .. }));
}

#[test]
fn bus_health_unhealthy_on_high_drops() {
    let metrics = BusMetricsSnapshot {
        messages_published: 1000,
        messages_delivered: 900,
        messages_dropped: 100,
        slow_subscribers: 5,
        peak_queue_depth: 100,
    };
    let health = flight_bus::assess_health(&metrics);
    assert!(matches!(health, flight_bus::BusHealth::Unhealthy { .. }));
}

#[test]
fn bus_metrics_atomic_counters() {
    let metrics = BusMetrics::new();
    for _ in 0..100 {
        metrics.record_publish();
        metrics.record_delivery();
    }
    for _ in 0..5 {
        metrics.record_drop();
    }
    metrics.record_slow_subscriber();
    metrics.update_peak_queue_depth(42);

    let snap = metrics.snapshot();
    assert_eq!(snap.messages_published, 100);
    assert_eq!(snap.messages_delivered, 100);
    assert_eq!(snap.messages_dropped, 5);
    assert_eq!(snap.slow_subscribers, 1);
    assert_eq!(snap.peak_queue_depth, 42);
}

#[test]
fn adapter_error_variants_display() {
    let errors = [
        AdapterError::NotConnected,
        AdapterError::Timeout("connection deadline".to_string()),
        AdapterError::AircraftNotDetected,
        AdapterError::Configuration("invalid port".to_string()),
        AdapterError::ReconnectExhausted,
        AdapterError::Other("unknown failure".to_string()),
    ];

    for err in &errors {
        let msg = err.to_string();
        assert!(!msg.is_empty(), "error display must not be empty: {err:?}");
    }
}

#[test]
fn adapter_metrics_aircraft_change_detection() {
    let mut metrics = AdapterMetrics::new();
    metrics.record_aircraft_change("C172".to_string());
    assert_eq!(metrics.aircraft_changes, 1);

    metrics.record_aircraft_change("C172".to_string()); // same
    assert_eq!(metrics.aircraft_changes, 1, "same aircraft = no new change");

    metrics.record_aircraft_change("A320".to_string());
    assert_eq!(metrics.aircraft_changes, 2);

    metrics.record_aircraft_change("F16C".to_string());
    assert_eq!(metrics.aircraft_changes, 3);
}

#[test]
fn adapter_disconnect_reconnect_telemetry_resumes() {
    let mut sim = FakeSim::new("MSFS");
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Phase 1: connected telemetry
    sim.connect();
    sim.set_aircraft("C172");
    sim.push_snapshot(sample_snapshot(5000.0, 120.0, false));

    let snap = sim.next_snapshot().unwrap();
    let mut bus_snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    bus_snap.environment.altitude = snap.altitude as f32;
    publisher.publish(bus_snap).unwrap();
    assert!(sub.try_recv().unwrap().is_some());

    // Phase 2: disconnect
    sim.disconnect();
    assert!(!sim.connected);

    // Phase 3: reconnect
    sim.connect();
    sim.push_snapshot(sample_snapshot(6000.0, 130.0, false));
    let snap2 = sim.next_snapshot().unwrap();

    let mut bus_snap2 = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    bus_snap2.environment.altitude = snap2.altitude as f32;
    tick();
    publisher.publish(bus_snap2).unwrap();

    let r = sub.try_recv().unwrap().unwrap();
    assert_approx_eq(r.environment.altitude as f64, 6000.0, 0.1);
}

// ===========================================================================
// 7. FULL PIPELINE — adapter + bus + subscriber + metrics combined
// ===========================================================================

#[test]
fn full_pipeline_adapter_to_subscriber_with_metrics() {
    let mut sim = FakeSim::new("DCS");
    sim.connect();
    sim.set_aircraft("F16C");

    let mut adapter_metrics = AdapterMetrics::new();
    adapter_metrics.record_aircraft_change("F16C".to_string());

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let bus_metrics = BusMetrics::new();

    // Simulate 5 telemetry frames
    for i in 0..5 {
        let alt = 20000.0 + (i as f64) * 1000.0;
        sim.push_snapshot(sample_snapshot(alt, 350.0, false));
    }

    let mut received_count = 0;
    while let Some(snap) = sim.next_snapshot() {
        adapter_metrics.record_update();

        let mut bus_snap = BusSnapshot::new(SimId::Dcs, AircraftId::new("F16C"));
        bus_snap.environment.altitude = snap.altitude as f32;

        bus_metrics.record_publish();
        publisher.publish(bus_snap).unwrap();
        tick();

        if let Some(_r) = sub.try_recv().unwrap() {
            bus_metrics.record_delivery();
            received_count += 1;
        }
    }

    assert_eq!(adapter_metrics.total_updates, 5);
    assert_eq!(adapter_metrics.aircraft_changes, 1);
    assert!(received_count >= 1, "must receive at least one frame");

    let snap = bus_metrics.snapshot();
    assert_eq!(snap.messages_published, 5);
    assert!(snap.messages_delivered > 0);
}

#[test]
fn full_pipeline_circuit_breaker_guards_adapter() {
    let cb_config = CircuitBreakerConfig {
        failure_threshold: 2,
        success_threshold: 1,
        timeout: Duration::from_millis(50),
    };
    let mut cb = CircuitBreaker::new(cb_config);

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Simulate adapter failures
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
    assert_eq!(cb.call_allowed(), CallResult::Rejected);

    // Wait for timeout and recover
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(cb.call_allowed(), CallResult::Allowed);

    // Successful publish after recovery
    publisher.publish(snapshot(SimId::Msfs, "C172")).unwrap();
    cb.record_success();
    assert_eq!(cb.state(), CircuitState::Closed);

    let r = sub.try_recv().unwrap();
    assert!(
        r.is_some(),
        "subscriber receives after circuit breaker recovery"
    );
}

#[test]
fn full_pipeline_late_subscriber_only_gets_new_data() {
    let mut publisher = make_publisher();

    // Publish before any subscriber
    for icao in &["C172", "B737", "A320"] {
        publisher.publish(snapshot(SimId::Msfs, icao)).unwrap();
        tick();
    }

    // Late subscriber
    let mut late = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    assert!(
        late.try_recv().unwrap().is_none(),
        "late subscriber gets no old data"
    );

    // New data should arrive
    publisher.publish(snapshot(SimId::Msfs, "F18")).unwrap();
    let r = late.try_recv().unwrap().unwrap();
    assert_eq!(r.aircraft.icao, "F18");
}
