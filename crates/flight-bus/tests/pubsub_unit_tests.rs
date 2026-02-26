// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Unit tests for the BusPublisher / Subscriber pub/sub patterns.
//!
//! These tests exercise the public API directly without any adapter harness:
//! - `BusPublisher::subscribe()` creating a working subscriber
//! - `BusPublisher::publish()` followed by `Subscriber::try_recv()`
//! - Multiple subscribers receiving the same snapshot
//! - `Subscriber::try_recv()` returning `None` when nothing has been published
//! - Subscribers receiving only snapshots published after they subscribed
//! - Invalid snapshots (NaN fields) being rejected before reaching subscribers
//! - Dropped subscribers being cleaned up on the next `publish()` call

use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, PublisherError, SubscriptionConfig};
use proptest::prelude::*;
use std::time::Duration;

// ─── helpers ────────────────────────────────────────────────────────────────

fn make_publisher() -> BusPublisher {
    // 60.0 Hz is the maximum accepted by the clamped constructor.
    // The rate limiter starts with `last_publish = now - min_interval` so the
    // very first `publish()` call always succeeds.
    BusPublisher::new(60.0)
}

fn valid_snapshot() -> BusSnapshot {
    BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"))
}

// ─── basic publish / receive ─────────────────────────────────────────────────

/// Publishing one snapshot lets a subscriber receive it with `try_recv`.
#[test]
fn test_publish_and_receive() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    publisher.publish(valid_snapshot()).unwrap();

    let received = sub.try_recv().expect("channel ok");
    assert!(
        received.is_some(),
        "subscriber should have received the snapshot"
    );
    assert_eq!(received.unwrap().sim, SimId::Msfs);
}

/// `try_recv` returns `Ok(None)` when no snapshot has been published yet.
#[test]
fn test_try_recv_empty() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let result = sub.try_recv().expect("channel ok");
    assert!(result.is_none(), "nothing published; should be None");
}

/// Subscribing before any publish means no old snapshots arrive (no history).
#[test]
fn test_subscriber_receives_no_data_before_publish() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // No publish has happened yet.
    assert!(
        sub.try_recv().unwrap().is_none(),
        "subscriber should not have any snapshot before first publish"
    );

    publisher.publish(valid_snapshot()).unwrap();

    assert!(
        sub.try_recv().unwrap().is_some(),
        "subscriber should get the snapshot published after it subscribed"
    );
}

// ─── multiple subscribers ────────────────────────────────────────────────────

/// Two subscribers both receive the single published snapshot.
#[test]
fn test_multiple_subscribers_each_receive_snapshot() {
    let mut publisher = make_publisher();
    let mut sub1 = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let mut sub2 = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    publisher.publish(valid_snapshot()).unwrap();

    assert!(
        sub1.try_recv().unwrap().is_some(),
        "sub1 should receive the snapshot"
    );
    assert!(
        sub2.try_recv().unwrap().is_some(),
        "sub2 should receive the snapshot"
    );
}

/// A subscriber created AFTER a publish does not receive the old snapshot.
#[test]
fn test_late_subscriber_gets_no_old_data() {
    let mut publisher = make_publisher();

    // Publish before subscribing.
    publisher.publish(valid_snapshot()).unwrap();

    // Subscribe after the publish.
    let mut late_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    assert!(
        late_sub.try_recv().unwrap().is_none(),
        "late subscriber must not receive snapshots published before it subscribed"
    );
}

// ─── multiple publishes ───────────────────────────────────────────────────────

/// Subscriber receives multiple snapshots in FIFO order.
///
/// Publishes are spaced 25 ms apart so both the publisher-level rate limiter
/// (≥ 16.7 ms at 60 Hz) and the per-subscriber rate limit are satisfied.
#[test]
fn test_subscriber_after_multiple_publishes() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let snap1 = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    let snap2 = BusSnapshot::new(SimId::XPlane, AircraftId::new("A320"));

    publisher.publish(snap1).unwrap();
    std::thread::sleep(Duration::from_millis(25)); // satisfy 60 Hz rate limiter
    publisher.publish(snap2).unwrap();

    let r1 = sub.try_recv().unwrap().expect("first snapshot");
    let r2 = sub.try_recv().unwrap().expect("second snapshot");

    assert_eq!(r1.sim, SimId::Msfs, "first received should be from MSFS");
    assert_eq!(
        r2.sim,
        SimId::XPlane,
        "second received should be from X-Plane"
    );
    assert!(
        sub.try_recv().unwrap().is_none(),
        "queue should be empty after two receives"
    );
}

// ─── invalid snapshot rejection ──────────────────────────────────────────────

/// Publishing a snapshot with NaN in `angular_rates.p` is rejected.
///
/// The snapshot must not be forwarded to subscribers.
#[test]
fn test_publish_nan_angular_rate_rejected() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut bad = valid_snapshot();
    bad.angular_rates.p = f32::NAN;

    let result = publisher.publish(bad);
    assert!(result.is_err(), "NaN angular rate should be rejected");
    assert!(
        matches!(result.unwrap_err(), PublisherError::ValidationError(_)),
        "error kind should be ValidationError"
    );

    // The invalid snapshot must not have reached the subscriber.
    assert!(
        sub.try_recv().unwrap().is_none(),
        "invalid snapshot must not be forwarded to subscriber"
    );
}

/// Publishing a snapshot with `Inf` in `environment.altitude` is rejected.
#[test]
fn test_publish_inf_altitude_rejected() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut bad = valid_snapshot();
    bad.environment.altitude = f32::INFINITY;

    let result = publisher.publish(bad);
    assert!(result.is_err(), "Inf altitude should be rejected");
    assert!(
        matches!(result.unwrap_err(), PublisherError::ValidationError(_)),
        "error kind should be ValidationError"
    );
    assert!(sub.try_recv().unwrap().is_none());
}

// ─── subscribe / unsubscribe bookkeeping ─────────────────────────────────────

/// `BusPublisher::subscribe()` increments the subscriber count; `unsubscribe()`
/// decrements it and further unsubscribes with the same id return an error.
#[test]
fn test_subscribe_creates_working_subscriber() {
    let mut publisher = make_publisher();
    assert_eq!(publisher.subscriber_count(), 0);

    let sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    assert_eq!(publisher.subscriber_count(), 1);

    publisher.unsubscribe(sub.id).unwrap();
    assert_eq!(publisher.subscriber_count(), 0);

    // Double-unsubscribe should fail.
    assert!(publisher.unsubscribe(sub.id).is_err());
}

/// When a `Subscriber` is dropped the publisher cleans up the dead channel on
/// the next `publish()` and does not return an error.
#[test]
fn test_dropped_subscriber_cleaned_up_on_next_publish() {
    let mut publisher = make_publisher();
    {
        let _sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
        // `_sub` (and therefore its Receiver) is dropped here.
    }
    // The internal entry still exists until the next publish.
    assert_eq!(publisher.subscriber_count(), 1, "entry not yet cleaned up");

    // publish() detects the disconnected sender and removes the dead entry.
    assert!(
        publisher.publish(valid_snapshot()).is_ok(),
        "publishing to a dropped subscriber must not error"
    );
    assert_eq!(
        publisher.subscriber_count(),
        0,
        "dead entry removed after publish"
    );
}

// ─── proptest ────────────────────────────────────────────────────────────────

fn sim_id_strategy() -> impl Strategy<Value = SimId> {
    prop_oneof![
        Just(SimId::Msfs),
        Just(SimId::Msfs2024),
        Just(SimId::XPlane),
        Just(SimId::Dcs),
        Just(SimId::Unknown),
    ]
}

fn aircraft_strategy() -> impl Strategy<Value = AircraftId> {
    prop_oneof![
        Just(AircraftId::new("C172")),
        Just(AircraftId::new("A320")),
        Just(AircraftId::new("B738")),
        Just(AircraftId::new("F16C")),
        Just(AircraftId::new("A10C")),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Any valid `BusSnapshot` published to the bus can be received by a
    /// subscriber, and the `sim` / `aircraft` fields are preserved exactly.
    #[test]
    fn prop_any_valid_snapshot_published_is_received(
        sim in sim_id_strategy(),
        aircraft in aircraft_strategy(),
    ) {
        let mut publisher = make_publisher();
        let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

        let snap = BusSnapshot::new(sim, aircraft.clone());
        publisher.publish(snap).unwrap();

        let received = sub.try_recv().unwrap().expect("snapshot should be received");
        prop_assert_eq!(received.sim, sim);
        prop_assert_eq!(received.aircraft, aircraft);
    }
}
