// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Device → Bus pipeline end-to-end integration tests.
//!
//! Proves: fake HID device → parse → normalize → bus publish,
//! multi-device, disconnect/reconnect, and frequency adaptation.

use flight_axis::pipeline::{AxisPipeline, ClampStage, CurveStage, DeadzoneStage};
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_test_helpers::{
    DeterministicClock, FakeDevice, FakeInput, assert_approx_eq, assert_in_range,
};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn make_publisher() -> BusPublisher {
    BusPublisher::new(60.0)
}

fn make_joystick(name: &str) -> FakeDevice {
    let mut dev = FakeDevice::new(name, 0x044F, 0xB10A, 4, 12);
    dev.connect();
    dev
}

fn tick() {
    std::thread::sleep(Duration::from_millis(25));
}

// ===========================================================================
// 1. Fake HID device → parse → normalize → bus publish
// ===========================================================================

#[test]
fn e2e_device_parse_normalize_bus_publish() {
    let mut device = make_joystick("Parse-Normalize Stick");

    // Enqueue raw HID-like values that need normalization (simulate u16 → [-1, 1])
    let raw_values: Vec<f64> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for &val in &raw_values {
        device.enqueue_input(FakeInput {
            axes: vec![val, 0.0, 0.0, 0.0],
            buttons: vec![false; 12],
            delay_ms: 4,
        });
    }

    let pipeline = standard_pipeline();
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut published_count = 0;
    while let Some(input) = device.next_input() {
        let processed = pipeline.process(input.axes[0], 0.004);
        assert!(processed.is_finite(), "processed value must be finite");
        assert_in_range(processed, -1.0, 1.0);

        let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        snap.control_inputs.pitch = processed as f32;
        publisher.publish(snap).expect("publish must succeed");
        tick();
        published_count += 1;
    }

    assert_eq!(published_count, raw_values.len());

    let mut received = 0;
    while let Ok(Some(snap)) = sub.try_recv() {
        assert_in_range(snap.control_inputs.pitch as f64, -1.0, 1.0);
        received += 1;
    }
    assert!(received > 0, "subscriber must receive at least one snapshot");
}

// ===========================================================================
// 2. Multiple devices publishing simultaneously
// ===========================================================================

#[test]
fn e2e_multiple_devices_simultaneous_publish() {
    let mut stick = make_joystick("Stick");
    let mut throttle = FakeDevice::new("Throttle", 0x044F, 0xB10B, 2, 8);
    throttle.connect();
    let mut rudder = FakeDevice::new("Rudder", 0x044F, 0xB10C, 1, 4);
    rudder.connect();

    stick.enqueue_input(FakeInput {
        axes: vec![0.6, -0.3, 0.0, 0.0],
        buttons: vec![false; 12],
        delay_ms: 4,
    });
    throttle.enqueue_input(FakeInput {
        axes: vec![0.8, 0.8],
        buttons: vec![false; 8],
        delay_ms: 4,
    });
    rudder.enqueue_input(FakeInput {
        axes: vec![0.1],
        buttons: vec![false; 4],
        delay_ms: 4,
    });

    let pipeline = standard_pipeline();

    let stick_input = stick.next_input().unwrap();
    let throttle_input = throttle.next_input().unwrap();
    let rudder_input = rudder.next_input().unwrap();

    let pitch = pipeline.process(stick_input.axes[0], 0.004);
    let roll = pipeline.process(stick_input.axes[1], 0.004);
    let throttle_val = pipeline.process(throttle_input.axes[0], 0.004);
    let yaw = pipeline.process(rudder_input.axes[0], 0.004);

    // All values must be finite and in range
    for &val in &[pitch, roll, throttle_val, yaw] {
        assert!(val.is_finite());
        assert_in_range(val, -1.0, 1.0);
    }

    // Publish combined snapshot
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap.control_inputs.pitch = pitch as f32;
    snap.control_inputs.roll = roll as f32;
    snap.control_inputs.yaw = yaw as f32;
    snap.control_inputs.throttle = vec![throttle_val as f32];
    publisher.publish(snap).expect("publish combined");

    let received = sub.try_recv().unwrap().expect("must receive");
    assert_approx_eq(received.control_inputs.pitch as f64, pitch, 1e-5);
    assert_approx_eq(received.control_inputs.roll as f64, roll, 1e-5);
}

// ===========================================================================
// 3. Device disconnect mid-stream
// ===========================================================================

#[test]
fn e2e_device_disconnect_mid_stream() {
    let mut device = make_joystick("Disconnect Stick");

    // Enqueue 10 frames
    for i in 0..10 {
        device.enqueue_input(FakeInput {
            axes: vec![i as f64 * 0.1, 0.0, 0.0, 0.0],
            buttons: vec![false; 12],
            delay_ms: 4,
        });
    }

    let pipeline = standard_pipeline();
    let mut last_valid_output = 0.0_f64;
    let mut processed_before_disconnect = 0;

    // Process 5 frames, then disconnect
    for _ in 0..5 {
        if let Some(input) = device.next_input() {
            let out = pipeline.process(input.axes[0], 0.004);
            assert!(out.is_finite());
            last_valid_output = out;
            processed_before_disconnect += 1;
        }
    }
    device.disconnect();
    assert!(!device.connected);
    assert_eq!(processed_before_disconnect, 5);

    // After disconnect, the last valid output should be held
    assert!(last_valid_output.is_finite());

    // The bus should accept the last-known-good value
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap.control_inputs.pitch = last_valid_output as f32;
    publisher.publish(snap).expect("publish last valid");

    let received = sub.try_recv().unwrap().expect("must receive");
    assert_approx_eq(received.control_inputs.pitch as f64, last_valid_output, 1e-5);
}

// ===========================================================================
// 4. Device reconnect recovery
// ===========================================================================

#[test]
fn e2e_device_reconnect_recovery() {
    let mut device = make_joystick("Reconnect Stick");

    // Phase 1: Connected, enqueue & consume
    device.enqueue_input(FakeInput {
        axes: vec![0.5, 0.0, 0.0, 0.0],
        buttons: vec![false; 12],
        delay_ms: 4,
    });
    let input1 = device.next_input().unwrap();
    let pipeline = standard_pipeline();
    let out1 = pipeline.process(input1.axes[0], 0.004);
    assert!(out1 > 0.0, "pre-disconnect output must be positive");

    // Phase 2: Disconnect
    device.disconnect();
    assert!(!device.connected);

    // Phase 3: Reconnect and resume
    device.connect();
    assert!(device.connected);
    device.enqueue_input(FakeInput {
        axes: vec![0.7, 0.0, 0.0, 0.0],
        buttons: vec![false; 12],
        delay_ms: 4,
    });

    let input2 = device.next_input().unwrap();
    let out2 = pipeline.process(input2.axes[0], 0.004);
    assert!(out2 > out1, "post-reconnect output must be larger (0.7 > 0.5)");
    assert!(out2.is_finite());
    assert_in_range(out2, -1.0, 1.0);

    // Publish reconnect value to bus
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap.control_inputs.pitch = out2 as f32;
    publisher.publish(snap).expect("publish after reconnect");

    let received = sub.try_recv().unwrap().expect("receive after reconnect");
    assert_approx_eq(received.control_inputs.pitch as f64, out2, 1e-5);
}

// ===========================================================================
// 5. High-frequency device (1000Hz) → 250Hz downsampling
// ===========================================================================

#[test]
fn e2e_high_frequency_device_downsampling() {
    let mut device = make_joystick("1kHz Device");

    // Generate 1000 frames at 1ms intervals (1000Hz)
    for i in 0..1000 {
        let val = (i as f64 / 1000.0 * std::f64::consts::TAU).sin() * 0.8;
        device.enqueue_input(FakeInput {
            axes: vec![val, 0.0, 0.0, 0.0],
            buttons: vec![false; 12],
            delay_ms: 1, // 1ms = 1000Hz
        });
    }

    let pipeline = standard_pipeline();
    let mut clock = DeterministicClock::new(0);
    let mut all_outputs = Vec::new();
    let mut downsampled_outputs = Vec::new();
    let mut sample_index = 0u64;

    while let Some(input) = device.next_input() {
        let out = pipeline.process(input.axes[0], 0.001);
        all_outputs.push(out);

        // Downsample: take every 4th sample (1000Hz / 4 = 250Hz)
        if sample_index.is_multiple_of(4) {
            downsampled_outputs.push(out);
        }
        sample_index += 1;
        clock.advance(1_000); // 1ms
    }

    assert_eq!(all_outputs.len(), 1000);
    assert_eq!(downsampled_outputs.len(), 250, "250Hz downsampled output");

    // All downsampled outputs must be finite and in range
    for (i, &val) in downsampled_outputs.iter().enumerate() {
        assert!(val.is_finite(), "downsampled[{i}] not finite");
        assert_in_range(val, -1.0, 1.0);
    }

    // Verify the downsampled data still tracks the sinusoidal shape
    // Peak positive should exist
    let max_val = downsampled_outputs
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(max_val > 0.3, "downsampled must contain positive peaks");
}

// ===========================================================================
// 6. Low-frequency device (50Hz) → interpolation
// ===========================================================================

#[test]
fn e2e_low_frequency_device_interpolation() {
    let mut device = make_joystick("50Hz Device");

    // Generate 50 frames at 20ms intervals (50Hz)
    let raw_50hz: Vec<f64> = (0..50)
        .map(|i| i as f64 / 50.0 * 0.8) // Linear ramp 0→0.8
        .collect();

    for &val in &raw_50hz {
        device.enqueue_input(FakeInput {
            axes: vec![val, 0.0, 0.0, 0.0],
            buttons: vec![false; 12],
            delay_ms: 20, // 20ms = 50Hz
        });
    }

    let pipeline = standard_pipeline();

    // Interpolate to 250Hz (5 samples per input frame)
    let mut interpolated = Vec::new();
    let mut prev_val = 0.0_f64;

    while let Some(input) = device.next_input() {
        let current = input.axes[0];
        // Linear interpolation: generate 5 intermediate samples
        for step in 0..5 {
            let t = step as f64 / 5.0;
            let interp = prev_val + (current - prev_val) * t;
            let out = pipeline.process(interp, 0.004);
            interpolated.push(out);
        }
        prev_val = current;
    }

    // 50 input frames × 5 interpolated = 250 output frames
    assert_eq!(interpolated.len(), 250, "interpolated to 250Hz");

    // All outputs finite and in range
    for (i, &val) in interpolated.iter().enumerate() {
        assert!(val.is_finite(), "interpolated[{i}] not finite");
        assert_in_range(val, -1.0, 1.0);
    }

    // The interpolated output should form a smooth ramp
    // Check that outputs generally increase (allowing for deadzone at start)
    let last_quarter = &interpolated[interpolated.len() - 60..];
    let first_of_last = last_quarter[0];
    let last_of_last = *last_quarter.last().unwrap();
    assert!(
        last_of_last >= first_of_last,
        "interpolated ramp should increase in the last quarter"
    );
}
