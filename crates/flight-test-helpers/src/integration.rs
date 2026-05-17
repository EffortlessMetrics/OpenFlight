// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Lightweight integration test harness primitives.

use crate::fixtures::TestConfig;
use flight_axis::pipeline::{AxisPipeline, ClampStage, CurveStage, DeadzoneStage};
use flight_bus::{BusPublisher, BusSnapshot, SubscriptionConfig};
use std::time::{Duration, Instant};

/// Basic harness with timing boundaries used by integration tests.
#[derive(Debug, Clone)]
pub struct TestHarness {
    config: TestConfig,
    started: Instant,
}

impl TestHarness {
    pub fn new(config: TestConfig) -> Self {
        Self {
            config,
            started: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn timed_out(&self) -> bool {
        self.elapsed() >= self.config.timeout
    }

    pub fn poll_interval(&self) -> Duration {
        self.config.poll_interval
    }
}

/// Build the standard test axis pipeline used by integration tests.
///
/// The pipeline applies a 5% deadzone, a 0.3 exponential curve, then clamps the
/// result to the normalized `[-1.0, 1.0]` axis range.
pub fn standard_axis_pipeline() -> AxisPipeline {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(DeadzoneStage {
        inner: 0.05,
        outer: 1.0,
    }));
    pipeline.add_stage(Box::new(CurveStage { expo: 0.3 }));
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));
    pipeline
}

/// Publish a snapshot through a fresh bus and return the first received value.
pub fn publish_and_receive(snapshot: BusSnapshot) -> BusSnapshot {
    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher
        .subscribe(SubscriptionConfig::default())
        .expect("subscriber must be created");
    publisher.publish(snapshot).expect("publish must succeed");
    subscriber
        .try_recv()
        .expect("channel must not error")
        .expect("snapshot must be present after publish")
}

#[cfg(test)]
mod tests {
    use super::{TestHarness, publish_and_receive, standard_axis_pipeline};
    use crate::fixtures::TestConfigBuilder;
    use flight_bus::{AircraftId, BusSnapshot, types::SimId};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_harness_timeout() {
        let config = TestConfigBuilder::default()
            .with_timeout(Duration::from_millis(20))
            .build();
        let harness = TestHarness::new(config);
        thread::sleep(Duration::from_millis(25));
        assert!(harness.timed_out());
    }

    #[test]
    fn standard_axis_pipeline_applies_expected_shape() {
        let pipeline = standard_axis_pipeline();

        assert_eq!(pipeline.process(0.03, 0.004), 0.0);
        assert!(pipeline.process(0.5, 0.004).is_finite());
        assert_eq!(pipeline.process(2.0, 0.004), 1.0);
    }

    #[test]
    fn publish_and_receive_round_trips_snapshot() {
        let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        snapshot.control_inputs.pitch = 0.25;

        let received = publish_and_receive(snapshot);

        assert_eq!(received.control_inputs.pitch, 0.25);
    }
}
