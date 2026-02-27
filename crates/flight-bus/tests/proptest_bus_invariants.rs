// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for the three core flight-bus invariants.
//!
//! **Invariant A – FIFO ordering**: Snapshots reach a subscriber in exactly the
//! sequence they were published (single publisher, single channel).
//!
//! **Invariant B – No panic on arbitrary float payloads**: Publishing a snapshot
//! whose unconstrained float fields carry any `f32` bit pattern (NaN, ±Inf,
//! subnormal, extreme values) must never unwind; it either succeeds or returns
//! a typed error.
//!
//! **Invariant C – Consistent all-or-nothing drop**: When the publisher-level
//! rate limiter suppresses a snapshot it must not reach *any* subscriber
//! (no partial delivery across N concurrent subscribers).

use flight_bus::types::{AircraftId, SimId};
use flight_bus::{BusPublisher, BusSnapshot, SubscriptionConfig};
use proptest::prelude::*;
use std::time::Duration;

// ─── helpers ─────────────────────────────────────────────────────────────────

fn publisher_60hz() -> BusPublisher {
    BusPublisher::new(60.0)
}

fn base_snapshot(sim: SimId) -> BusSnapshot {
    BusSnapshot::new(sim, AircraftId::new("C172"))
}

fn sim_from_index(i: usize) -> SimId {
    match i % 5 {
        0 => SimId::Msfs,
        1 => SimId::Msfs2024,
        2 => SimId::XPlane,
        3 => SimId::Dcs,
        _ => SimId::Unknown,
    }
}

// ─── Invariant A: FIFO ordering ──────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Invariant A: N snapshots published in sequence arrive at the subscriber
    /// in exactly that order.
    ///
    /// Each publish is separated by 25 ms to satisfy the 60 Hz publisher-level
    /// rate limiter (minimum interval ≈ 16.7 ms).  Distinct `SimId` values act
    /// as sequence tags so ordering violations are unambiguous.
    #[test]
    fn fifo_ordering_within_channel(n in 2usize..=4usize) {
        let mut publisher = publisher_60hz();
        let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

        let sims: Vec<SimId> = (0..n).map(sim_from_index).collect();

        for (i, &sim) in sims.iter().enumerate() {
            if i > 0 {
                // Satisfy the publisher-level 60 Hz rate limiter (≈16.7 ms min
                // interval) before each subsequent publish.
                std::thread::sleep(Duration::from_millis(25));
            }
            publisher.publish(base_snapshot(sim)).unwrap();
        }

        for (i, &expected) in sims.iter().enumerate() {
            let received = sub.try_recv().unwrap();
            prop_assert!(
                received.is_some(),
                "snapshot {i} must be delivered (expected sim {:?})",
                expected
            );
            prop_assert_eq!(
                received.unwrap().sim,
                expected,
                "snapshot {} arrived out of order",
                i
            );
        }
    }
}

// ─── Invariant B: No panic on arbitrary float payloads ───────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Invariant B: Publishing a snapshot whose unconstrained float fields carry
    /// any `f32` bit pattern (including NaN, ±Inf, subnormal, and extreme values)
    /// must never cause a panic.
    ///
    /// The call may return `Ok(())` (accepted) or `Err(_)` (validation rejected
    /// it), but it must never unwind.
    #[test]
    fn no_panic_on_arbitrary_float_payloads(
        ang_p    in any::<f32>(),
        ang_q    in any::<f32>(),
        altitude in any::<f32>(),
        oat      in any::<f32>(),
    ) {
        let mut publisher = publisher_60hz();
        let mut snap = base_snapshot(SimId::Msfs);

        // These fields only require finiteness (no range constraint), so NaN/Inf
        // will be rejected, while all finite values — including subnormals and
        // extreme magnitudes — should be accepted.
        snap.angular_rates.p = ang_p;
        snap.angular_rates.q = ang_q;
        snap.environment.altitude = altitude;
        snap.environment.oat = oat;

        // The call must not panic; Ok or Err is acceptable.
        let _ = publisher.publish(snap);
    }
}

// ─── Invariant C: Consistent all-or-nothing drop ─────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Invariant C: When the publisher-level rate limiter suppresses a snapshot,
    /// *none* of the N registered subscribers (1–4) receive it.
    ///
    /// Dropping must be all-or-nothing: there must be no partial delivery where
    /// some subscribers receive the snapshot while others do not.
    #[test]
    fn rate_limited_drop_reaches_no_subscriber(n_subs in 1usize..=4usize) {
        let mut publisher = publisher_60hz();
        let mut subscribers: Vec<_> = (0..n_subs)
            .map(|_| publisher.subscribe(SubscriptionConfig::default()).unwrap())
            .collect();

        // First publish always passes the rate limiter (pre-initialized to
        // `now - min_interval` so the very first call succeeds immediately).
        publisher.publish(base_snapshot(SimId::Msfs)).unwrap();

        // Drain the first snapshot from every subscriber so the channels are
        // empty before the second publish.
        for sub in &mut subscribers {
            let _ = sub.try_recv().unwrap();
        }

        // Back-to-back second publish is always rate-limited and discarded.
        publisher.publish(base_snapshot(SimId::XPlane)).unwrap();

        // No subscriber should have received the rate-limited snapshot.
        for (i, sub) in subscribers.iter_mut().enumerate() {
            let msg = sub.try_recv().unwrap();
            prop_assert!(
                msg.is_none(),
                "subscriber {i} of {n_subs} received a rate-limited (dropped) snapshot"
            );
        }
    }
}
