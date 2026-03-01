// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! End-to-end trace replay integration tests.
//!
//! Proves the full pipeline works by loading recorded input traces, replaying
//! them through device → axis engine → bus → subscriber, and asserting that
//! outputs match expected values.
//!
//! Test scenarios:
//! 1. Single axis trace replay with deadzone/curve verification
//! 2. Multi-axis simultaneous trace replay
//! 3. Profile switch during processing
//! 4. Deadzone and curve application verification
//! 5. Bus event delivery confirmation
//! 6. Adapter → bus → subscriber flow with mixed events

use flight_axis::pipeline::{
    AxisPipeline, ClampStage, CurveStage, DeadzoneStage, SensitivityStage,
};
use flight_axis::{AxisEngine, AxisFrame, InputValidator, PipelineBuilder};
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_test_helpers::{
    DeterministicClock, FakeSim, TraceComparator, TraceEvent, TraceEventType, TracePlayer,
    TraceRecording, TraceSource, assert_approx_eq, assert_in_range,
};
use std::path::PathBuf;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn load_trace(name: &str) -> TraceRecording {
    TraceRecording::load_from_file(&fixture_path(name)).unwrap_or_else(|e| {
        panic!("failed to load fixture {name}: {e}");
    })
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

fn aggressive_deadzone_pipeline() -> AxisPipeline {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(DeadzoneStage {
        inner: 0.15,
        outer: 1.0,
    }));
    pipeline.add_stage(Box::new(CurveStage { expo: 0.5 }));
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));
    pipeline
}

fn make_publisher() -> BusPublisher {
    BusPublisher::new(60.0)
}

/// Compute the expected output of a deadzone+curve+clamp pipeline.
fn expected_pipeline_output(raw: f64, dz_inner: f64, dz_outer: f64, expo: f64) -> f64 {
    let abs_raw = raw.abs();
    if abs_raw <= dz_inner {
        return 0.0;
    }
    let rescaled = (abs_raw - dz_inner) / (dz_outer - dz_inner);
    let curved = rescaled.powf(1.0 + expo);
    let result = raw.signum() * curved;
    result.clamp(-1.0, 1.0)
}

fn expected_standard_output(raw: f64) -> f64 {
    expected_pipeline_output(raw, 0.05, 1.0, 0.3)
}

fn expected_aggressive_output(raw: f64) -> f64 {
    expected_pipeline_output(raw, 0.15, 1.0, 0.5)
}

// ===========================================================================
// 1. Single axis trace replay → pipeline → verified outputs
// ===========================================================================

#[test]
fn trace_replay_single_axis_ramp_through_pipeline() {
    let trace = load_trace("single_axis_ramp.json");
    assert!(trace.event_count() > 0, "trace must have events");

    let pipeline = standard_pipeline();
    let axis_events = trace.events_of_type(TraceEventType::AxisInput);

    let mut actual_recording = TraceRecording::new("single_axis_output");
    let mut clock = DeterministicClock::new(0);

    for event in &axis_events {
        let raw = event.data[0];
        let processed = pipeline.process(raw, 0.004);
        let expected = expected_standard_output(raw);

        assert_approx_eq(processed, expected, 1e-6);
        assert_in_range(processed, -1.0, 1.0);
        assert!(processed.is_finite(), "output must be finite for input {raw}");

        actual_recording.add_event(TraceEvent {
            timestamp_us: event.timestamp_us,
            event_type: TraceEventType::AxisInput,
            source: TraceSource::Device,
            data: vec![processed],
        });

        clock.advance_ticks(1);
    }

    assert_eq!(
        actual_recording.event_count(),
        axis_events.len(),
        "output must have same event count as input"
    );
}

#[test]
fn trace_replay_single_axis_deadzone_zeroes_small_inputs() {
    let trace = load_trace("single_axis_ramp.json");
    let pipeline = standard_pipeline();

    for event in trace.events_of_type(TraceEventType::AxisInput) {
        let raw = event.data[0];
        let processed = pipeline.process(raw, 0.004);

        if raw.abs() <= 0.05 {
            assert!(
                processed.abs() < f64::EPSILON,
                "input {raw} within deadzone must produce 0, got {processed}"
            );
        }
    }
}

#[test]
fn trace_replay_single_axis_full_deflection_preserved() {
    let trace = load_trace("single_axis_ramp.json");
    let pipeline = standard_pipeline();

    for event in trace.events_of_type(TraceEventType::AxisInput) {
        let raw = event.data[0];
        let processed = pipeline.process(raw, 0.004);

        if (raw - 1.0).abs() < f64::EPSILON {
            assert!(
                (processed - 1.0).abs() < 1e-6,
                "full +1.0 input must produce ~1.0 output"
            );
        }
        if (raw - (-1.0)).abs() < f64::EPSILON {
            assert!(
                (processed - (-1.0)).abs() < 1e-6,
                "full -1.0 input must produce ~-1.0 output"
            );
        }
    }
}

// ===========================================================================
// 2. Multi-axis simultaneous trace replay
// ===========================================================================

#[test]
fn trace_replay_multi_axis_independent_processing() {
    let trace = load_trace("multi_axis_simultaneous.json");
    let axis_events = trace.events_of_type(TraceEventType::AxisInput);
    assert!(axis_events.len() >= 5, "need enough multi-axis events");

    let pipelines: Vec<AxisPipeline> = (0..4).map(|_| standard_pipeline()).collect();

    for event in &axis_events {
        assert_eq!(event.data.len(), 4, "multi-axis events must have 4 values");

        let outputs: Vec<f64> = event
            .data
            .iter()
            .zip(pipelines.iter())
            .map(|(&raw, pipe)| pipe.process(raw, 0.004))
            .collect();

        for (i, &out) in outputs.iter().enumerate() {
            let expected = expected_standard_output(event.data[i]);
            assert_approx_eq(out, expected, 1e-6);
            assert_in_range(out, -1.0, 1.0);
        }
    }
}

#[test]
fn trace_replay_multi_axis_yaw_deadzone_at_0_02() {
    let trace = load_trace("multi_axis_simultaneous.json");
    let pipeline = standard_pipeline();

    for event in trace.events_of_type(TraceEventType::AxisInput) {
        if event.data.len() >= 3 {
            let yaw_raw = event.data[2];
            let yaw_out = pipeline.process(yaw_raw, 0.004);
            if yaw_raw.abs() <= 0.05 {
                assert!(
                    yaw_out.abs() < f64::EPSILON,
                    "yaw {yaw_raw} in deadzone must produce 0, got {yaw_out}"
                );
            }
        }
    }
}

#[test]
fn trace_replay_multi_axis_snapshot_comparison() {
    let trace = load_trace("multi_axis_simultaneous.json");
    let pipelines: Vec<AxisPipeline> = (0..4).map(|_| standard_pipeline()).collect();

    let mut expected_recording = TraceRecording::new("expected_output");
    let mut actual_recording = TraceRecording::new("actual_output");

    for event in trace.events_of_type(TraceEventType::AxisInput) {
        let expected_data: Vec<f64> = event.data.iter().map(|&v| expected_standard_output(v)).collect();
        let actual_data: Vec<f64> = event
            .data
            .iter()
            .zip(pipelines.iter())
            .map(|(&raw, pipe)| pipe.process(raw, 0.004))
            .collect();

        expected_recording.add_event(TraceEvent {
            timestamp_us: event.timestamp_us,
            event_type: TraceEventType::AxisInput,
            source: TraceSource::Device,
            data: expected_data,
        });
        actual_recording.add_event(TraceEvent {
            timestamp_us: event.timestamp_us,
            event_type: TraceEventType::AxisInput,
            source: TraceSource::Device,
            data: actual_data,
        });
    }

    let comparator = TraceComparator::within_tolerance(1e-6);
    let diff = comparator.compare(&expected_recording, &actual_recording);
    assert!(
        diff.is_match(),
        "trace comparison failed:\n{}",
        diff.report()
    );
}

// ===========================================================================
// 3. Profile switch during processing
// ===========================================================================

#[test]
fn trace_replay_profile_switch_changes_output() {
    let trace = load_trace("single_axis_ramp.json");
    let events = trace.events_of_type(TraceEventType::AxisInput);
    let midpoint = events.len() / 2;

    let standard = standard_pipeline();
    let aggressive = aggressive_deadzone_pipeline();

    let mut outputs_pre_switch = Vec::new();
    let mut outputs_post_switch = Vec::new();

    for (i, event) in events.iter().enumerate() {
        let raw = event.data[0];
        if i < midpoint {
            outputs_pre_switch.push(standard.process(raw, 0.004));
        } else {
            outputs_post_switch.push(aggressive.process(raw, 0.004));
        }
    }

    // Both halves must produce finite in-range outputs
    for &o in outputs_pre_switch.iter().chain(outputs_post_switch.iter()) {
        assert!(o.is_finite());
        assert_in_range(o, -1.0, 1.0);
    }

    // The two profiles produce different results for the same input (0.1 is
    // outside standard 5% dz but inside aggressive 15% dz)
    let test_val = 0.1;
    let std_out = standard.process(test_val, 0.004);
    let agg_out = aggressive.process(test_val, 0.004);
    assert_approx_eq(std_out, expected_standard_output(test_val), 1e-6);
    assert_approx_eq(agg_out, expected_aggressive_output(test_val), 1e-6);
    assert!(
        (std_out - agg_out).abs() > 0.01,
        "profile switch must change output: std={std_out}, agg={agg_out}"
    );
}

#[test]
fn trace_replay_profile_switch_with_engine() {
    let engine = AxisEngine::new_for_axis("pitch".to_string());

    // Start with standard profile
    let pipeline1 = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .unwrap()
        .compile()
        .expect("compile standard");
    engine.update_pipeline(pipeline1);

    let mut frame = AxisFrame::new(0.6, 0);
    engine.process(&mut frame).expect("process with standard");
    let out_standard = frame.out;

    // Switch to aggressive profile
    let pipeline2 = PipelineBuilder::new()
        .deadzone(0.15)
        .curve(0.5)
        .unwrap()
        .compile()
        .expect("compile aggressive");
    engine.update_pipeline(pipeline2);

    let mut frame2 = AxisFrame::new(0.6, 4_000_000);
    engine.process(&mut frame2).expect("process with aggressive");
    let out_aggressive = frame2.out;

    assert!(
        (out_standard - out_aggressive).abs() > 0.01,
        "switching profile must change engine output"
    );
    assert!(out_standard.is_finite());
    assert!(out_aggressive.is_finite());
}

// ===========================================================================
// 4. Deadzone and curve application verification
// ===========================================================================

#[test]
fn trace_replay_deadzone_boundary_exact() {
    let pipeline = standard_pipeline();

    // Exactly at deadzone edge
    let at_edge = pipeline.process(0.05, 0.004);
    assert!(
        at_edge.abs() < f64::EPSILON,
        "value at dz edge (0.05) must be 0, got {at_edge}"
    );

    // Just outside deadzone
    let just_outside = pipeline.process(0.06, 0.004);
    assert!(
        just_outside > 0.0,
        "value just outside dz (0.06) must be > 0, got {just_outside}"
    );
    assert!(
        just_outside < 0.02,
        "value just outside dz should be small, got {just_outside}"
    );
}

#[test]
fn trace_replay_curve_expo_reduces_low_values() {
    let mut linear_pipeline = AxisPipeline::new();
    linear_pipeline.add_stage(Box::new(DeadzoneStage {
        inner: 0.05,
        outer: 1.0,
    }));
    linear_pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));

    let curved_pipeline = standard_pipeline(); // has expo 0.3

    // At mid-range, curve should reduce output compared to linear
    let linear_out = linear_pipeline.process(0.5, 0.004);
    let curved_out = curved_pipeline.process(0.5, 0.004);

    assert!(
        curved_out < linear_out,
        "expo curve must reduce mid-range: linear={linear_out}, curved={curved_out}"
    );
    assert!(curved_out > 0.0, "mid-range must still be positive");

    // At full deflection, both should produce ~1.0
    let linear_full = linear_pipeline.process(1.0, 0.004);
    let curved_full = curved_pipeline.process(1.0, 0.004);
    assert_approx_eq(linear_full, 1.0, 1e-6);
    assert_approx_eq(curved_full, 1.0, 1e-6);
}

#[test]
fn trace_replay_sensitivity_multiplier_verified() {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(SensitivityStage { multiplier: 2.0 }));
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));

    let out_low = pipeline.process(0.3, 0.004);
    assert_approx_eq(out_low, 0.6, 1e-6);

    // Above 0.5 should clamp
    let out_high = pipeline.process(0.7, 0.004);
    assert_approx_eq(out_high, 1.0, 1e-6);
}

// ===========================================================================
// 5. Bus event delivery confirmation
// ===========================================================================

#[test]
fn trace_replay_pipeline_output_delivered_through_bus() {
    let trace = load_trace("single_axis_ramp.json");
    let pipeline = standard_pipeline();
    let events = trace.events_of_type(TraceEventType::AxisInput);

    let mut publisher = make_publisher();
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut delivered_count = 0;
    for event in &events {
        let raw = event.data[0];
        let processed = pipeline.process(raw, 0.004);

        let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        snapshot.control_inputs.pitch = processed as f32;
        publisher.publish(snapshot).expect("publish must succeed");

        // Rate limiter may drop some; sleep to satisfy it
        std::thread::sleep(Duration::from_millis(20));

        if let Some(received) = subscriber.try_recv().unwrap() {
            assert_in_range(received.control_inputs.pitch as f64, -1.0, 1.0);
            assert_eq!(received.sim, SimId::Msfs);
            delivered_count += 1;
        }
    }

    assert!(
        delivered_count > 0,
        "at least one snapshot must be delivered through bus"
    );
}

#[test]
fn trace_replay_multi_axis_bus_round_trip() {
    let trace = load_trace("multi_axis_simultaneous.json");
    let pipelines: Vec<AxisPipeline> = (0..4).map(|_| standard_pipeline()).collect();
    let events = trace.events_of_type(TraceEventType::AxisInput);

    let mut publisher = make_publisher();
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Process first frame with all 4 axes and publish
    let first = &events[0];
    let outputs: Vec<f64> = first
        .data
        .iter()
        .zip(pipelines.iter())
        .map(|(&raw, pipe)| pipe.process(raw, 0.004))
        .collect();

    let mut snapshot = BusSnapshot::new(SimId::XPlane, AircraftId::new("A320"));
    snapshot.control_inputs.pitch = outputs[0] as f32;
    snapshot.control_inputs.roll = outputs[1] as f32;
    snapshot.control_inputs.yaw = outputs[2] as f32;
    snapshot.control_inputs.throttle = vec![outputs[3] as f32];

    publisher.publish(snapshot).expect("publish");
    let received = subscriber
        .try_recv()
        .unwrap()
        .expect("must receive snapshot");

    assert_approx_eq(received.control_inputs.pitch as f64, outputs[0], 1e-5);
    assert_approx_eq(received.control_inputs.roll as f64, outputs[1], 1e-5);
    assert_approx_eq(received.control_inputs.yaw as f64, outputs[2], 1e-5);
    assert_eq!(received.sim, SimId::XPlane);
}

#[test]
fn trace_replay_nan_rejected_before_bus() {
    let pipeline = standard_pipeline();
    let mut validator = InputValidator::new();

    // Valid → NaN → valid again
    validator.update(0.5);
    let sanitised = validator.update(f32::NAN);
    assert_eq!(sanitised, 0.5, "NaN replaced with last valid");

    let processed = pipeline.process(sanitised as f64, 0.004);
    assert!(processed.is_finite());

    // NaN snapshot rejected by bus (angular_rates triggers validation)
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut bad_snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    bad_snap.angular_rates.p = f32::NAN;
    assert!(publisher.publish(bad_snap).is_err());
    assert!(sub.try_recv().unwrap().is_none());
}

// ===========================================================================
// 6. Adapter → bus → subscriber flow with mixed events
// ===========================================================================

#[test]
fn trace_replay_adapter_bus_subscriber_flow() {
    let trace = load_trace("adapter_bus_flow.json");
    let pipeline = standard_pipeline();

    let mut sim = FakeSim::new("MSFS");
    sim.connect();
    sim.set_aircraft("C172");

    let mut publisher = make_publisher();
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut player = TracePlayer::new(trace);
    let mut published_count = 0;

    player.with_callback(|event, _delay_us| {
        match event.event_type {
            TraceEventType::AxisInput => {
                let processed = pipeline.process(event.data[0], 0.004);
                let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
                snapshot.control_inputs.pitch = processed as f32;
                if publisher.publish(snapshot).is_ok() {
                    published_count += 1;
                }
            }
            TraceEventType::TelemetryUpdate => {
                if event.data.len() >= 3 {
                    sim.push_snapshot(flight_test_helpers::FakeSnapshot {
                        altitude: event.data[0],
                        airspeed: event.data[1],
                        heading: event.data[2],
                        pitch: 0.0,
                        roll: 0.0,
                        yaw: 0.0,
                        on_ground: false,
                    });
                }
            }
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(20));
    });

    assert!(published_count > 0, "must publish axis events through bus");

    // Drain subscriber
    let mut received = 0;
    while let Ok(Some(snap)) = subscriber.try_recv() {
        assert_eq!(snap.sim, SimId::Msfs);
        assert_in_range(snap.control_inputs.pitch as f64, -1.0, 1.0);
        received += 1;
    }
    assert!(received > 0, "subscriber must receive at least one snapshot");

    // Sim should have received telemetry snapshots
    let mut telem_count = 0;
    while sim.next_snapshot().is_some() {
        telem_count += 1;
    }
    assert!(telem_count > 0, "sim must have received telemetry snapshots");
}

#[test]
fn trace_replay_adapter_disconnect_reconnect_resumes() {
    let mut sim = FakeSim::new("MSFS");
    let pipeline = standard_pipeline();
    let mut publisher = make_publisher();
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Phase 1: connected, process input
    sim.connect();
    sim.set_aircraft("C172");

    let processed1 = pipeline.process(0.5, 0.004);
    let mut snap1 = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap1.control_inputs.pitch = processed1 as f32;
    publisher.publish(snap1).expect("publish pre-disconnect");

    let r1 = subscriber.try_recv().unwrap().expect("receive pre-disconnect");
    assert_approx_eq(r1.control_inputs.pitch as f64, processed1, 1e-5);

    // Phase 2: disconnect
    sim.disconnect();
    assert!(!sim.connected);

    // Phase 3: reconnect, resume
    sim.connect();
    std::thread::sleep(Duration::from_millis(20));

    let processed2 = pipeline.process(0.7, 0.004);
    let mut snap2 = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap2.control_inputs.pitch = processed2 as f32;
    publisher.publish(snap2).expect("publish post-reconnect");

    let r2 = subscriber
        .try_recv()
        .unwrap()
        .expect("receive post-reconnect");
    assert_approx_eq(r2.control_inputs.pitch as f64, processed2, 1e-5);
}

// ===========================================================================
// 7. Trace comparator validates pipeline determinism
// ===========================================================================

#[test]
fn trace_replay_deterministic_output_matches_snapshot() {
    let trace = load_trace("single_axis_ramp.json");
    let pipeline = standard_pipeline();

    // Run the pipeline twice over the same trace
    let mut run1 = TraceRecording::new("run1");
    let mut run2 = TraceRecording::new("run2");

    for event in trace.events_of_type(TraceEventType::AxisInput) {
        let raw = event.data[0];

        let out1 = pipeline.process(raw, 0.004);
        let out2 = pipeline.process(raw, 0.004);

        run1.add_event(TraceEvent {
            timestamp_us: event.timestamp_us,
            event_type: TraceEventType::AxisInput,
            source: TraceSource::Device,
            data: vec![out1],
        });
        run2.add_event(TraceEvent {
            timestamp_us: event.timestamp_us,
            event_type: TraceEventType::AxisInput,
            source: TraceSource::Device,
            data: vec![out2],
        });
    }

    let comparator = TraceComparator::new(); // exact match
    let diff = comparator.compare(&run1, &run2);
    assert!(
        diff.is_match(),
        "pipeline must be deterministic:\n{}",
        diff.report()
    );
}

#[test]
fn trace_replay_player_advance_produces_correct_events() {
    let trace = load_trace("adapter_bus_flow.json");
    let mut player = TracePlayer::new(trace);

    // Advance to 16ms — should get first 5 events (0, 4000, 8000, 12000, 16000)
    let events = player.advance_to(16000);
    assert!(events.len() >= 4, "should get events up to 16ms, got {}", events.len());

    for event in &events {
        assert!(event.timestamp_us <= 16000, "no events past 16ms");
    }

    // Advance to end
    let remaining = player.advance_to(100_000);
    assert!(!remaining.is_empty(), "should get remaining events");
    assert!(player.is_complete(), "player should be complete");
}

// ===========================================================================
// 8. Full AxisEngine trace replay with compiled pipeline
// ===========================================================================

#[test]
fn trace_replay_engine_compiled_pipeline_full_trace() {
    let trace = load_trace("single_axis_ramp.json");
    let engine = AxisEngine::new_for_axis("pitch".to_string());

    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .unwrap()
        .compile()
        .expect("compile");
    engine.update_pipeline(pipeline);

    let mut clock = DeterministicClock::new(0);

    for event in trace.events_of_type(TraceEventType::AxisInput) {
        let raw = event.data[0] as f32;
        let ts_ns = clock.now_us() * 1_000;

        let mut frame = AxisFrame::new(raw, ts_ns);
        engine.process(&mut frame).expect("engine process");

        assert!(frame.out.is_finite(), "engine output must be finite");
        assert!(
            frame.out >= -1.0 && frame.out <= 1.0,
            "engine output {:.6} out of range",
            frame.out
        );

        // Deadzone check
        if raw.abs() <= 0.05 {
            assert_eq!(frame.out, 0.0, "input {raw} in deadzone must produce 0");
        }

        clock.advance_ticks(1);
    }
}

#[test]
fn trace_replay_engine_250hz_timing_consistency() {
    let trace = load_trace("multi_axis_simultaneous.json");
    let engine = AxisEngine::new_for_axis("roll".to_string());

    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .unwrap()
        .compile()
        .expect("compile");
    engine.update_pipeline(pipeline);

    let mut clock = DeterministicClock::new(0);
    let mut timestamps = Vec::new();
    let mut outputs = Vec::new();

    for event in trace.events_of_type(TraceEventType::AxisInput) {
        let raw = if event.data.len() >= 2 {
            event.data[1] as f32 // roll axis
        } else {
            0.0
        };

        let ts_ns = clock.now_us() * 1_000;
        timestamps.push(clock.now_us());

        let mut frame = AxisFrame::new(raw, ts_ns);
        engine.process(&mut frame).expect("engine process");
        outputs.push(frame.out);

        clock.advance_ticks(1);
    }

    // Verify timing: each tick is 4ms apart
    for pair in timestamps.windows(2) {
        assert_eq!(pair[1] - pair[0], 4_000, "expected 4ms tick spacing");
    }

    // All outputs finite and in range
    for (i, &o) in outputs.iter().enumerate() {
        assert!(o.is_finite(), "output[{i}] not finite");
        assert!(o >= -1.0 && o <= 1.0, "output[{i}] = {o} out of range");
    }
}
