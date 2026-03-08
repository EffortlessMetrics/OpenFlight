// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Bus pipeline integration tests.
//!
//! Wires [`BusPublisher`] + [`Subscriber`] together with axis pipeline
//! processing, fake devices, and deterministic timing to prove end-to-end
//! data flow through the bus.

use flight_axis::pipeline::{AxisPipeline, ClampStage, CurveStage, DeadzoneStage};
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_test_helpers::{
    DeterministicClock, DeviceFixtureBuilder, FakeDevice, FakeInput, assert_approx_eq,
    assert_in_range,
};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_publisher() -> BusPublisher {
    BusPublisher::new(60.0)
}

fn tick() {
    std::thread::sleep(Duration::from_millis(25));
}

fn standard_pipeline() -> AxisPipeline {
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

// ===========================================================================
// 1. Single publisher, single subscriber — axis snapshot round-trip
// ===========================================================================

#[test]
fn bus_publish_axis_snapshot_received_by_subscriber() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let pipeline = standard_pipeline();
    let processed_pitch = pipeline.process(0.6, 0.004);

    let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snapshot.control_inputs.pitch = processed_pitch as f32;
    snapshot.control_inputs.roll = 0.0;

    publisher.publish(snapshot).expect("publish ok");

    let received = sub.try_recv().unwrap().expect("must receive snapshot");
    assert_eq!(received.sim, SimId::Msfs);
    assert_eq!(received.aircraft.icao, "C172");
    assert_approx_eq(received.control_inputs.pitch as f64, processed_pitch, 1e-5);
}

// ===========================================================================
// 2. Multiple subscribers all receive the same snapshot
// ===========================================================================

#[test]
fn bus_all_subscribers_receive_published_snapshot() {
    let mut publisher = make_publisher();
    let mut subs: Vec<_> = (0..3)
        .map(|_| publisher.subscribe(SubscriptionConfig::default()).unwrap())
        .collect();

    let snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("B737"));
    publisher.publish(snapshot).expect("publish");

    for (i, sub) in subs.iter_mut().enumerate() {
        let received = sub.try_recv().unwrap().expect("sub must receive");
        assert_eq!(
            received.aircraft.icao, "B737",
            "subscriber {i} got wrong aircraft"
        );
    }
}

// ===========================================================================
// 3. Sequential axis values published at 250Hz tick rate
// ===========================================================================

#[test]
fn bus_250hz_axis_stream_through_pipeline_and_bus() {
    let mut clock = DeterministicClock::new(0);
    let mut device = FakeDevice::new("Test Stick", 0x044F, 0xB10A, 4, 12);
    device.connect();

    // Generate 10 ticks of linearly increasing pitch
    for i in 0..10 {
        device.enqueue_input(FakeInput {
            axes: vec![i as f64 * 0.1, 0.0, 0.0, 0.0],
            buttons: vec![false; 12],
            delay_ms: 4,
        });
    }

    let pipeline = standard_pipeline();
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let mut received_values = Vec::new();

    while let Some(input) = device.next_input() {
        let processed = pipeline.process(input.axes[0], 0.004);
        let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        snap.control_inputs.pitch = processed as f32;
        publisher.publish(snap).expect("publish");
        tick();
        clock.advance_ticks(1);

        if let Some(r) = sub.try_recv().unwrap() {
            received_values.push(r.control_inputs.pitch as f64);
        }
    }

    assert!(!received_values.is_empty(), "must receive axis data");
    // All values must be in valid range
    for &v in &received_values {
        assert_in_range(v, -1.0, 1.0);
    }
}

// ===========================================================================
// 4. Snapshot validation rejects NaN
// ===========================================================================

#[test]
fn bus_rejects_nan_snapshot() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut bad_snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    bad_snap.angular_rates.p = f32::NAN;

    let result = publisher.publish(bad_snap);
    assert!(result.is_err(), "NaN snapshot must be rejected by bus");
    assert!(
        sub.try_recv().unwrap().is_none(),
        "invalid snapshot must not reach subscriber"
    );
}

// ===========================================================================
// 5. Device fixture → pipeline → bus integration
// ===========================================================================

#[test]
fn bus_device_fixture_through_pipeline_to_subscriber() {
    let fixture = DeviceFixtureBuilder::new("hotas-1")
        .name("Saitek X52 Pro")
        .with_standard_axes()
        .build();

    // Simulate axis values from the fixture
    let pipeline = standard_pipeline();
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Process each fixture axis value through the pipeline
    let mut processed = Vec::new();
    for axis in &fixture.axes {
        processed.push(pipeline.process(axis.value, 0.004));
    }

    let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap.control_inputs.pitch = processed[0] as f32; // pitch
    snap.control_inputs.roll = processed[1] as f32; // roll
    snap.control_inputs.yaw = processed[2] as f32; // yaw
    snap.control_inputs.throttle = vec![processed[3] as f32]; // throttle

    publisher.publish(snap).expect("publish");
    let received = sub.try_recv().unwrap().expect("receive");

    // Standard axes start at 0.0 which is inside 5% deadzone → output 0.0
    assert_eq!(received.control_inputs.pitch, 0.0);
    assert_eq!(received.control_inputs.roll, 0.0);
    assert_eq!(received.control_inputs.yaw, 0.0);
    assert_eq!(received.control_inputs.throttle[0], 0.0);
}
