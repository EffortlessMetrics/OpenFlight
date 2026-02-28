// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Bus health assessment based on metrics snapshots.
//!
//! Evaluates a [`BusMetricsSnapshot`] and returns a [`BusHealth`] verdict
//! using configurable drop-rate thresholds.

use crate::metrics::BusMetricsSnapshot;

/// Health status of the telemetry bus.
#[derive(Debug, Clone, PartialEq)]
pub enum BusHealth {
    /// Drop rate is below the degraded threshold.
    Healthy,
    /// Drop rate is elevated but below the unhealthy threshold.
    Degraded { reason: String },
    /// Drop rate exceeds the unhealthy threshold.
    Unhealthy { reason: String },
}

impl BusHealth {
    /// Returns `true` when the bus is [`BusHealth::Healthy`].
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        matches!(self, BusHealth::Healthy)
    }
}

/// Assess bus health from a metrics snapshot.
///
/// Thresholds:
/// - Drop rate < 1 %  → [`BusHealth::Healthy`]
/// - Drop rate 1–5 %  → [`BusHealth::Degraded`]
/// - Drop rate > 5 %  → [`BusHealth::Unhealthy`]
#[must_use]
pub fn assess_health(metrics: &BusMetricsSnapshot) -> BusHealth {
    let drop_rate = metrics.drop_rate_percent();

    if drop_rate > 5.0 {
        BusHealth::Unhealthy {
            reason: format!("drop rate {drop_rate:.1}% exceeds 5% threshold"),
        }
    } else if drop_rate >= 1.0 {
        BusHealth::Degraded {
            reason: format!("drop rate {drop_rate:.1}% exceeds 1% threshold"),
        }
    } else {
        BusHealth::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_with_drops(published: u64, dropped: u64) -> BusMetricsSnapshot {
        BusMetricsSnapshot {
            messages_published: published,
            messages_delivered: published.saturating_sub(dropped),
            messages_dropped: dropped,
            slow_subscribers: 0,
            peak_queue_depth: 0,
        }
    }

    #[test]
    fn healthy_when_no_drops() {
        let s = snapshot_with_drops(1000, 0);
        assert_eq!(assess_health(&s), BusHealth::Healthy);
    }

    #[test]
    fn healthy_below_one_percent() {
        // 0.5% drop rate
        let s = snapshot_with_drops(1000, 5);
        assert!(assess_health(&s).is_healthy());
    }

    #[test]
    fn degraded_at_one_percent() {
        let s = snapshot_with_drops(1000, 10);
        assert!(matches!(assess_health(&s), BusHealth::Degraded { .. }));
    }

    #[test]
    fn degraded_between_one_and_five_percent() {
        // 3% drop rate
        let s = snapshot_with_drops(1000, 30);
        match assess_health(&s) {
            BusHealth::Degraded { reason } => {
                assert!(reason.contains("3.0%"), "unexpected reason: {reason}");
            }
            other => panic!("expected Degraded, got {other:?}"),
        }
    }

    #[test]
    fn unhealthy_above_five_percent() {
        // 10% drop rate
        let s = snapshot_with_drops(1000, 100);
        match assess_health(&s) {
            BusHealth::Unhealthy { reason } => {
                assert!(reason.contains("10.0%"), "unexpected reason: {reason}");
            }
            other => panic!("expected Unhealthy, got {other:?}"),
        }
    }

    #[test]
    fn healthy_when_no_messages_published() {
        let s = snapshot_with_drops(0, 0);
        assert_eq!(assess_health(&s), BusHealth::Healthy);
    }

    #[test]
    fn degraded_at_five_percent_boundary() {
        // Exactly 5% → still Degraded (> 5% required for Unhealthy)
        let s = snapshot_with_drops(1000, 50);
        assert!(matches!(assess_health(&s), BusHealth::Degraded { .. }));
    }

    #[test]
    fn unhealthy_just_above_five_percent() {
        // 5.1% drop rate
        let s = snapshot_with_drops(1000, 51);
        assert!(matches!(assess_health(&s), BusHealth::Unhealthy { .. }));
    }

    #[test]
    fn is_healthy_helper() {
        assert!(BusHealth::Healthy.is_healthy());
        assert!(
            !BusHealth::Degraded {
                reason: "test".into()
            }
            .is_healthy()
        );
        assert!(
            !BusHealth::Unhealthy {
                reason: "test".into()
            }
            .is_healthy()
        );
    }
}
