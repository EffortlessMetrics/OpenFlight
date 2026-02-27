// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for BusSnapshot serialization and BusPublisher invariants.
//!
//! Covers:
//! - Invariant 1: Any SimId variant is preserved exactly through publish/receive.
//! - Invariant 2: Every subscriber created before a publish receives the snapshot.
//! - Invariant 3: Publisher rate limiter drops back-to-back publishes.
//! - Serialization: BusSnapshot round-trips through JSON without data loss.

use flight_bus::snapshot::BusSnapshot;
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, SubscriptionConfig};
use proptest::prelude::*;

// ─── helpers ─────────────────────────────────────────────────────────────────

fn sim_id_from_index(idx: usize) -> SimId {
    match idx % 10 {
        0 => SimId::Msfs,
        1 => SimId::Msfs2024,
        2 => SimId::XPlane,
        3 => SimId::Dcs,
        4 => SimId::AceCombat7,
        5 => SimId::WarThunder,
        6 => SimId::EliteDangerous,
        7 => SimId::Ksp,
        8 => SimId::Wingman,
        _ => SimId::Unknown,
    }
}

fn valid_snapshot(sim: SimId) -> BusSnapshot {
    BusSnapshot::new(sim, AircraftId::new("C172"))
}

// ─── property tests ───────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Invariant 1: The `sim` field of a snapshot is preserved exactly through a
    /// publish/receive cycle, for every SimId variant.
    #[test]
    fn bus_snapshot_sim_id_preserved_through_publish(sim_idx in 0usize..10) {
        let sim = sim_id_from_index(sim_idx);
        let mut publisher = BusPublisher::new(60.0);
        let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

        publisher.publish(valid_snapshot(sim)).unwrap();

        let received = sub.try_recv().unwrap().expect("snapshot must be delivered");
        prop_assert_eq!(
            received.sim, sim,
            "sim field must survive the publish/receive round-trip"
        );
    }

    /// Invariant 2: Every subscriber that exists at publish time receives the
    /// snapshot, regardless of how many subscribers are registered (1–5).
    #[test]
    fn multiple_subscribers_each_receive_snapshot(n_subscribers in 1usize..=5) {
        let mut publisher = BusPublisher::new(60.0);
        let mut subscribers: Vec<_> = (0..n_subscribers)
            .map(|_| publisher.subscribe(SubscriptionConfig::default()).unwrap())
            .collect();

        publisher.publish(valid_snapshot(SimId::Msfs)).unwrap();

        for (i, sub) in subscribers.iter_mut().enumerate() {
            let received = sub.try_recv().unwrap();
            prop_assert!(
                received.is_some(),
                "subscriber {i} of {n_subscribers} must receive the snapshot"
            );
        }
    }

    /// Invariant 3: Two back-to-back publish calls never both reach the subscriber.
    ///
    /// `BusPublisher::new` clamps the rate to 30–60 Hz, giving a minimum
    /// inter-publish interval of ≥ ~16.7 ms.  A tight-loop second publish will
    /// always fire before that interval elapses, so it must be rate-limited.
    #[test]
    fn publisher_respects_rate_limit(rate_hz in 1.0f64..=120.0f64) {
        let mut publisher = BusPublisher::new(rate_hz as f32);
        let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

        let snap = valid_snapshot(SimId::Msfs);

        // First publish always succeeds: the rate limiter pre-initialises
        // `last_publish` to `now - min_interval`.
        publisher.publish(snap.clone()).unwrap();

        // Second back-to-back publish must be dropped by the rate limiter.
        publisher.publish(snap.clone()).unwrap();

        let first = sub.try_recv().unwrap();
        prop_assert!(first.is_some(), "first snapshot must be delivered to subscriber");

        let second = sub.try_recv().unwrap();
        prop_assert!(
            second.is_none(),
            "rate-limited second snapshot must not reach the subscriber"
        );

        prop_assert!(
            publisher.stats().snapshots_dropped >= 1,
            "snapshots_dropped counter must reflect the rate-limited publish"
        );
    }

    /// Serialization: a default BusSnapshot round-trips through JSON without
    /// losing the `sim` or `aircraft` fields.
    #[test]
    fn bus_snapshot_serde_round_trip(sim_idx in 0usize..10) {
        let sim = sim_id_from_index(sim_idx);
        let original = BusSnapshot::new(sim, AircraftId::new("B738"));

        let json = serde_json::to_string(&original).expect("serialization must succeed");
        let restored: BusSnapshot =
            serde_json::from_str(&json).expect("deserialization must succeed");

        prop_assert_eq!(restored.sim, original.sim, "sim must survive JSON round-trip");
        prop_assert_eq!(
            restored.aircraft,
            original.aircraft,
            "aircraft must survive JSON round-trip"
        );
    }
}
