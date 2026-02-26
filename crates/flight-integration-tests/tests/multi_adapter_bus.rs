// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Multi-adapter bus pipeline integration tests.
//!
//! Exercises the [`BusPublisher`] / [`Subscriber`] pipeline across multiple
//! simulated adapter sources without any hardware dependency.  Each test
//! focuses on a distinct cross-adapter property: ordering, identity
//! preservation, late-subscription semantics, high-volume drain, and default
//! filter behaviour.

use flight_bus::{
    BusPublisher, SubscriptionConfig,
    snapshot::BusSnapshot,
    types::{AircraftId, SimId},
};
use std::time::Duration;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Sleep long enough to satisfy both the publisher and per-subscriber rate
/// limiters at their maximum 60 Hz setting (minimum interval ≈ 16.7 ms).
fn tick() {
    std::thread::sleep(Duration::from_millis(25));
}

fn snapshot(sim: SimId, icao: &str) -> BusSnapshot {
    BusSnapshot::new(sim, AircraftId::new(icao))
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// Publish 5 sequential snapshots (different aircraft) through one publisher
/// and verify that two independent subscribers each receive all five in order.
#[test]
fn test_multiple_hotas_devices_publish_sequential_snapshots() {
    let mut publisher = BusPublisher::new(60.0);
    let mut sub_a = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let mut sub_b = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let aircraft = ["C172", "B737", "F18", "A320", "F16"];
    for icao in &aircraft {
        publisher
            .publish(snapshot(SimId::Msfs, icao))
            .expect("publish must succeed");
        tick();
    }

    let received_a: Vec<String> = {
        let mut v = Vec::new();
        while let Ok(Some(s)) = sub_a.try_recv() {
            v.push(s.aircraft.icao.clone());
        }
        v
    };
    let received_b: Vec<String> = {
        let mut v = Vec::new();
        while let Ok(Some(s)) = sub_b.try_recv() {
            v.push(s.aircraft.icao.clone());
        }
        v
    };

    let expected: Vec<String> = aircraft.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        received_a, expected,
        "subscriber A must receive all 5 aircraft in order"
    );
    assert_eq!(
        received_b, expected,
        "subscriber B must receive all 5 aircraft in order"
    );
}

/// Publish snapshots for three (SimId, AircraftId) pairs and verify that the
/// bus does not mix identities — each received snapshot carries the exact sim
/// and aircraft it was published with.
#[test]
fn test_bus_snapshot_aircraft_identity_preserved() {
    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let sources: &[(SimId, &str)] = &[
        (SimId::Msfs, "C172"),
        (SimId::XPlane, "B737"),
        (SimId::Dcs, "F18"),
    ];

    for &(sim, icao) in sources {
        publisher
            .publish(snapshot(sim, icao))
            .expect("publish must succeed");
        tick();
    }

    let mut received: Vec<(SimId, String)> = Vec::new();
    while let Ok(Some(s)) = subscriber.try_recv() {
        received.push((s.sim, s.aircraft.icao.clone()));
    }

    assert_eq!(
        received.len(),
        sources.len(),
        "must receive one snapshot per published source"
    );
    for (i, &(sim, icao)) in sources.iter().enumerate() {
        assert_eq!(
            received[i].0, sim,
            "snapshot {i} sim mismatch: expected {sim:?}, got {:?}",
            received[i].0
        );
        assert_eq!(
            received[i].1, icao,
            "snapshot {i} aircraft mismatch: expected {icao}, got {}",
            received[i].1
        );
    }
}

/// A subscriber created *after* snapshots have already been published must not
/// receive any of those earlier snapshots (no implicit replay).
#[test]
fn test_late_subscriber_misses_earlier_snapshots() {
    let mut publisher = BusPublisher::new(60.0);

    // Publish 3 snapshots before any subscriber exists.
    for icao in &["C172", "B737", "F18"] {
        publisher
            .publish(snapshot(SimId::Msfs, icao))
            .expect("publish must succeed");
        tick();
    }

    // Subscribe *after* all publishes — channel is brand-new and empty.
    let mut late_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let msg = late_sub.try_recv().expect("channel must not error");
    assert!(
        msg.is_none(),
        "late subscriber must not receive previously published snapshots"
    );
}

/// Publish exactly `CAPACITY` snapshots (with inter-publish delays so all pass
/// the rate limiter) and verify the pre-subscribed subscriber drains them all
/// without deadlock or unexpected drops.
#[test]
fn test_high_volume_publish_no_drops_under_capacity() {
    const CAPACITY: usize = 5;

    let mut publisher = BusPublisher::new(60.0);
    let config = SubscriptionConfig {
        buffer_size: CAPACITY,
        drop_on_full: false,
        max_rate_hz: 60.0,
    };
    let mut subscriber = publisher.subscribe(config).unwrap();

    for i in 0..CAPACITY {
        let icao = format!("AC{i:02}");
        publisher
            .publish(snapshot(SimId::Msfs, &icao))
            .expect("publish must succeed");
        tick();
    }

    let mut count = 0usize;
    while let Ok(Some(_)) = subscriber.try_recv() {
        count += 1;
    }

    assert_eq!(
        count, CAPACITY,
        "all {CAPACITY} snapshots must be received without drops"
    );
}

/// A subscriber created with [`SubscriptionConfig::default`] must receive
/// every snapshot that passes the publisher rate limiter — the default
/// configuration applies no additional filtering.
#[test]
fn test_subscriber_filters_nothing_without_config() {
    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let to_publish = [
        snapshot(SimId::Msfs, "C172"),
        snapshot(SimId::XPlane, "B737"),
        snapshot(SimId::Dcs, "F18"),
    ];

    for s in to_publish.iter().cloned() {
        publisher.publish(s).expect("publish must succeed");
        tick();
    }

    let mut received_count = 0usize;
    while let Ok(Some(_)) = subscriber.try_recv() {
        received_count += 1;
    }

    assert_eq!(
        received_count,
        to_publish.len(),
        "default SubscriptionConfig must not filter any snapshot; \
         expected {}, got {received_count}",
        to_publish.len()
    );
}
