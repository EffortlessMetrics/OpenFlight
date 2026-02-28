// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Full axis pipeline end-to-end integration tests.
//!
//! Proves: raw HID input → axis processing → bus publish → subscriber receive.
//! Uses mock devices, deterministic timing, and the real axis/bus stack.

use flight_axis::pipeline::{
    AxisPipeline, ClampStage, CurveStage, DeadzoneStage, SensitivityStage, SmoothingStage,
};
use flight_axis::{AxisEngine, AxisFrame, InputValidator, PipelineBuilder};
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_test_helpers::{
    DeterministicClock, FakeDevice, FakeInput, assert_approx_eq, assert_bounded_rate,
    assert_in_range, assert_monotonic,
};

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

/// Compute the expected output of the standard pipeline for a given raw value.
fn expected_standard_output(raw: f64) -> f64 {
    let dz_inner = 0.05_f64;
    let dz_outer = 1.0_f64;
    let expo = 0.3_f64;

    let abs_raw = raw.abs();
    if abs_raw <= dz_inner {
        return 0.0;
    }
    let rescaled = (abs_raw - dz_inner) / (dz_outer - dz_inner);
    let curved = rescaled.powf(1.0 + expo);
    let result = raw.signum() * curved;
    result.clamp(-1.0, 1.0)
}

// ===========================================================================
// 1. Raw device → pipeline → verified output
// ===========================================================================

#[test]
fn e2e_fake_device_ramp_through_pipeline() {
    let mut device = make_joystick("Ramp Test Stick");

    let ramp: Vec<f64> = (0..=20).map(|i| i as f64 * 0.05).collect();
    for &val in &ramp {
        device.enqueue_input(FakeInput {
            axes: vec![val, 0.0, 0.0, 0.0],
            buttons: vec![false; 12],
            delay_ms: 4,
        });
    }

    let pipeline = standard_pipeline();
    let mut outputs = Vec::new();

    while let Some(input) = device.next_input() {
        let out = pipeline.process(input.axes[0], 0.004);
        outputs.push(out);
    }

    assert_eq!(outputs.len(), ramp.len());

    for (i, (&raw, &out)) in ramp.iter().zip(outputs.iter()).enumerate() {
        let expected = expected_standard_output(raw);
        assert_approx_eq(out, expected, 1e-6);
        assert_in_range(out, -1.0, 1.0);
        assert!(out.is_finite(), "output[{i}] must be finite");
    }
}

#[test]
fn e2e_negative_ramp_through_pipeline() {
    let mut device = make_joystick("Neg Ramp Stick");

    let ramp: Vec<f64> = (0..=20).map(|i| -(i as f64) * 0.05).collect();
    for &val in &ramp {
        device.enqueue_input(FakeInput {
            axes: vec![val, 0.0, 0.0, 0.0],
            buttons: vec![false; 12],
            delay_ms: 4,
        });
    }

    let pipeline = standard_pipeline();
    let mut outputs = Vec::new();

    while let Some(input) = device.next_input() {
        outputs.push(pipeline.process(input.axes[0], 0.004));
    }

    for (&raw, &out) in ramp.iter().zip(outputs.iter()) {
        let expected = expected_standard_output(raw);
        assert_approx_eq(out, expected, 1e-6);
        assert_in_range(out, -1.0, 1.0);
    }
}

// ===========================================================================
// 2. Multi-axis independent processing
// ===========================================================================

#[test]
fn e2e_four_axes_process_independently() {
    let mut device = make_joystick("4-Axis HOTAS");

    device.enqueue_input(FakeInput {
        axes: vec![0.8, -0.6, 0.02, 1.0],
        buttons: vec![false; 12],
        delay_ms: 4,
    });

    let pipelines: Vec<AxisPipeline> = (0..4).map(|_| standard_pipeline()).collect();
    let input = device.next_input().unwrap();
    let outputs: Vec<f64> = input
        .axes
        .iter()
        .zip(pipelines.iter())
        .map(|(&val, pipe)| pipe.process(val, 0.004))
        .collect();

    assert!(outputs[0] > 0.0, "pitch should be positive");
    assert!(outputs[1] < 0.0, "roll should be negative");
    assert!(outputs[2].abs() < f64::EPSILON, "yaw 0.02 inside deadzone");
    assert!((outputs[3] - 1.0).abs() < 1e-10, "throttle at full");

    for (i, &o) in outputs.iter().enumerate() {
        assert_approx_eq(o, expected_standard_output(input.axes[i]), 1e-6);
    }
}

// ===========================================================================
// 3. Pipeline → bus publish → subscriber receive
// ===========================================================================

#[test]
fn e2e_pipeline_output_published_to_bus_subscriber() {
    let mut device = make_joystick("Bus Integration Stick");
    device.enqueue_input(FakeInput {
        axes: vec![0.7, -0.3, 0.0, 0.5],
        buttons: vec![false; 12],
        delay_ms: 4,
    });

    let pipeline = standard_pipeline();
    let input = device.next_input().unwrap();
    let processed = pipeline.process(input.axes[0], 0.004);

    let mut publisher = make_publisher();
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snapshot.control_inputs.pitch = processed as f32;

    publisher.publish(snapshot).expect("publish must succeed");

    let received = subscriber.try_recv().expect("channel ok");
    assert!(received.is_some(), "subscriber must receive snapshot");
    let snap = received.unwrap();

    assert_approx_eq(snap.control_inputs.pitch as f64, processed, 1e-5);
    assert_eq!(snap.sim, SimId::Msfs);
}

#[test]
fn e2e_multiple_subscribers_receive_pipeline_output() {
    let pipeline = standard_pipeline();
    let processed = pipeline.process(0.6, 0.004);

    let mut publisher = make_publisher();
    let mut sub1 = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let mut sub2 = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("A320"));
    snapshot.control_inputs.roll = processed as f32;

    publisher.publish(snapshot).expect("publish");

    let r1 = sub1.try_recv().unwrap().expect("sub1 snapshot");
    let r2 = sub2.try_recv().unwrap().expect("sub2 snapshot");

    assert_approx_eq(r1.control_inputs.roll as f64, processed, 1e-5);
    assert_approx_eq(r2.control_inputs.roll as f64, processed, 1e-5);
}

// ===========================================================================
// 4. Deterministic 250Hz timing simulation
// ===========================================================================

#[test]
fn e2e_250hz_deterministic_timing_full_pipeline() {
    let mut clock = DeterministicClock::new(0);
    let mut device = make_joystick("250Hz Stick");

    // Generate 250 ticks (1 second) of a sinusoidal input
    for tick in 0..250 {
        let t = tick as f64 / 250.0;
        let val = (t * std::f64::consts::TAU).sin() * 0.8;
        device.enqueue_input(FakeInput {
            axes: vec![val, 0.0, 0.0, 0.0],
            buttons: vec![false; 12],
            delay_ms: 4,
        });
    }

    let pipeline = standard_pipeline();
    let mut timestamps = Vec::new();
    let mut outputs = Vec::new();

    while let Some(input) = device.next_input() {
        timestamps.push(clock.now_us());
        let out = pipeline.process(input.axes[0], 0.004);
        outputs.push(out);
        clock.advance_ticks(1);
    }

    assert_eq!(outputs.len(), 250);
    assert_eq!(clock.now_us(), 1_000_000, "250 ticks = 1 second");

    // Verify timestamp spacing is exactly 4ms
    for pair in timestamps.windows(2) {
        assert_eq!(pair[1] - pair[0], 4_000, "expected 4ms tick spacing");
    }

    // All outputs must be finite and in range
    for (i, &o) in outputs.iter().enumerate() {
        assert!(o.is_finite(), "output[{i}] not finite: {o}");
        assert_in_range(o, -1.0, 1.0);
    }
}

#[test]
fn e2e_compiled_pipeline_250hz_with_engine() {
    let engine = AxisEngine::new_for_axis("pitch".to_string());
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .unwrap()
        .compile()
        .expect("pipeline should compile");
    engine.update_pipeline(pipeline);

    let mut clock = DeterministicClock::new(0);
    let mut outputs = Vec::new();

    let inputs = [0.0_f32, 0.03, 0.1, 0.5, 0.8, 1.0, -0.5, -1.0, 0.0, 0.02];
    for &raw in &inputs {
        let ts_ns = clock.now_us() * 1_000;
        let mut frame = AxisFrame::new(raw, ts_ns);
        engine.process(&mut frame).expect("process ok");
        outputs.push(frame.out);
        clock.advance_ticks(1);
    }

    // Deadzone coverage
    assert_eq!(outputs[0], 0.0, "0.0 → 0");
    assert_eq!(outputs[1], 0.0, "0.03 inside 5% dz");
    assert_eq!(outputs[9], 0.0, "0.02 inside 5% dz");

    // Full deflection
    assert!((outputs[5] - 1.0).abs() < 1e-6, "1.0 → 1.0");
    assert!((outputs[7] - (-1.0)).abs() < 1e-6, "-1.0 → -1.0");

    // Monotonicity in positive ramp region
    assert!(outputs[3] > outputs[2], "0.5 > 0.1 in output");
    assert!(outputs[4] > outputs[3], "0.8 > 0.5 in output");
    assert!(outputs[5] >= outputs[4], "1.0 >= 0.8 in output");
}

// ===========================================================================
// 5. NaN/Inf rejection through full pipeline
// ===========================================================================

#[test]
fn e2e_nan_rejected_at_validator_layer() {
    let mut validator = InputValidator::new();
    let pipeline = standard_pipeline();

    // Feed valid baseline
    validator.update(0.5);

    // NaN injection
    let sanitised = validator.update(f32::NAN);
    assert_eq!(sanitised, 0.5, "NaN replaced with last valid");

    let out = pipeline.process(sanitised as f64, 0.004);
    assert!(out.is_finite(), "pipeline output after NaN must be finite");
    assert_in_range(out, -1.0, 1.0);
}

#[test]
fn e2e_inf_rejected_at_validator_layer() {
    let mut validator = InputValidator::new();
    let pipeline = standard_pipeline();

    let sanitised = validator.update(f32::INFINITY);
    assert_eq!(sanitised, 1.0, "+Inf clamped to 1.0");

    let out = pipeline.process(sanitised as f64, 0.004);
    assert!((out - 1.0).abs() < 1e-10, "full deflection output");

    let sanitised_neg = validator.update(f32::NEG_INFINITY);
    assert_eq!(sanitised_neg, -1.0, "-Inf clamped to -1.0");

    let out_neg = pipeline.process(sanitised_neg as f64, 0.004);
    assert!((out_neg - (-1.0)).abs() < 1e-10);
}

#[test]
fn e2e_nan_snapshot_rejected_by_bus() {
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut bad_snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    bad_snap.angular_rates.p = f32::NAN;

    let result = publisher.publish(bad_snap);
    assert!(result.is_err(), "NaN snapshot must be rejected");
    assert!(
        sub.try_recv().unwrap().is_none(),
        "invalid snap must not arrive"
    );
}

// ===========================================================================
// 6. Smoothing stage convergence at 250Hz
// ===========================================================================

#[test]
fn e2e_smoothing_converges_at_250hz() {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(SmoothingStage::new(0.3)));

    let mut clock = DeterministicClock::new(0);
    let mut values = Vec::new();

    // Step input for 40 ticks (160ms)
    for _ in 0..40 {
        let out = pipeline.process(1.0, 0.004);
        values.push(out);
        clock.advance_ticks(1);
    }

    // Must be monotonically non-decreasing
    assert_monotonic(&values);

    // Must converge close to 1.0
    assert!(values.last().unwrap() > &0.999, "must converge near 1.0");

    // Rate of change must be bounded (smoothing limits jumps)
    assert_bounded_rate(&values, 0.35);
}

// ===========================================================================
// 7. Sensitivity + clamp pipeline → bus
// ===========================================================================

#[test]
fn e2e_sensitivity_clamp_to_bus() {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(SensitivityStage { multiplier: 2.5 }));
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));

    // 0.6 * 2.5 = 1.5, clamped to 1.0
    let out = pipeline.process(0.6, 0.004);
    assert!((out - 1.0).abs() < f64::EPSILON, "clamped to 1.0");

    // Publish clamped value
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap.control_inputs.throttle = vec![out as f32];
    publisher.publish(snap).unwrap();

    let received = sub.try_recv().unwrap().unwrap();
    assert_eq!(received.control_inputs.throttle[0], 1.0);
}

// ===========================================================================
// 8. Full pipeline with all stages chained
// ===========================================================================

#[test]
fn e2e_full_stage_chain_deadzone_curve_sensitivity_smoothing_clamp() {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(DeadzoneStage {
        inner: 0.05,
        outer: 1.0,
    }));
    pipeline.add_stage(Box::new(CurveStage { expo: 0.2 }));
    pipeline.add_stage(Box::new(SensitivityStage { multiplier: 1.2 }));
    pipeline.add_stage(Box::new(SmoothingStage::new(0.5)));
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));

    let mut clock = DeterministicClock::new(0);
    let test_inputs = [0.0, 0.03, 0.1, 0.3, 0.5, 0.7, 0.9, 1.0, 1.0, 1.0];
    let mut outputs = Vec::new();

    for &input in &test_inputs {
        let out = pipeline.process(input, 0.004);
        outputs.push(out);
        clock.advance_ticks(1);
    }

    // Deadzone: first two inputs (0.0, 0.03) produce small or zero output
    assert!(outputs[0].abs() < f64::EPSILON, "zero input → zero");

    // All outputs finite and in range
    for (i, &o) in outputs.iter().enumerate() {
        assert!(o.is_finite(), "output[{i}] not finite");
        assert_in_range(o, -1.0, 1.0);
    }

    // Outputs should generally increase for increasing input (with smoothing lag)
    assert!(outputs[5] > outputs[3], "0.7 > 0.3 output");
    assert!(outputs[7] > outputs[5], "1.0 > 0.7 output");
}

// ===========================================================================
// 9. Bypass/enable stage mid-stream
// ===========================================================================

#[test]
fn e2e_bypass_curve_stage_midstream() {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(DeadzoneStage {
        inner: 0.05,
        outer: 1.0,
    }));
    pipeline.add_stage(Box::new(CurveStage { expo: 0.5 }));
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));

    let input = 0.6;
    let with_curve = pipeline.process(input, 0.004);

    // Bypass the curve stage
    pipeline.bypass_stage(1);
    let without_curve = pipeline.process(input, 0.004);

    assert!(
        (with_curve - without_curve).abs() > 0.01,
        "curve bypass must change output"
    );

    // Re-enable
    pipeline.enable_stage(1);
    let restored = pipeline.process(input, 0.004);
    assert_approx_eq(restored, with_curve, 1e-10);
}
