// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Trace recording and replay end-to-end integration tests.
//!
//! Proves: record from fake pipeline, replay with verification,
//! serialization/deserialization, and golden-file comparison.

use flight_axis::pipeline::{AxisPipeline, ClampStage, CurveStage, DeadzoneStage};
use flight_test_helpers::{
    DeterministicClock, FakeDevice, FakeInput, SnapshotStore, TraceComparator, TraceEvent,
    TraceEventType, TracePlayer, TraceRecording, TraceSource, assert_approx_eq,
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

/// Run a fake pipeline and return a trace recording.
fn record_pipeline_trace(inputs: &[f64]) -> TraceRecording {
    let mut device = FakeDevice::new("Trace Stick", 0x044F, 0xB10A, 4, 12);
    device.connect();

    for &val in inputs {
        device.enqueue_input(FakeInput {
            axes: vec![val, 0.0, 0.0, 0.0],
            buttons: vec![false; 12],
            delay_ms: 4,
        });
    }

    let pipeline = standard_pipeline();
    let mut clock = DeterministicClock::new(0);
    let mut recording = TraceRecording::new("pipeline-trace");
    recording.device_id = Some("Trace Stick".to_string());

    while let Some(input) = device.next_input() {
        // Record input event
        recording.add_event(TraceEvent {
            timestamp_us: clock.now_us(),
            event_type: TraceEventType::AxisInput,
            source: TraceSource::Device,
            data: input.axes.clone(),
        });

        // Process and record output
        let out = pipeline.process(input.axes[0], 0.004);
        recording.add_event(TraceEvent {
            timestamp_us: clock.now_us(),
            event_type: TraceEventType::TelemetryUpdate,
            source: TraceSource::Simulator,
            data: vec![out],
        });

        clock.advance_ticks(1);
    }

    recording
}

// ===========================================================================
// 1. Record a trace from fake pipeline
// ===========================================================================

#[test]
fn e2e_trace_record_from_pipeline() {
    let inputs: Vec<f64> = (0..50).map(|i| i as f64 * 0.02).collect();
    let recording = record_pipeline_trace(&inputs);

    // Each input produces 2 events (input + output)
    assert_eq!(recording.event_count(), 100, "50 inputs × 2 events each");
    assert_eq!(recording.name, "pipeline-trace");
    assert_eq!(
        recording.device_id.as_deref(),
        Some("Trace Stick")
    );

    // Duration should be (50-1) × 4ms = 196ms = 196_000µs
    assert_eq!(recording.duration(), 196_000);

    // Verify event types alternate: AxisInput, TelemetryUpdate
    let input_events = recording.events_of_type(TraceEventType::AxisInput);
    let output_events = recording.events_of_type(TraceEventType::TelemetryUpdate);
    assert_eq!(input_events.len(), 50);
    assert_eq!(output_events.len(), 50);

    // All input events come from Device, all outputs from Simulator
    for e in &input_events {
        assert_eq!(e.source, TraceSource::Device);
    }
    for e in &output_events {
        assert_eq!(e.source, TraceSource::Simulator);
    }
}

// ===========================================================================
// 2. Replay trace and verify same outputs
// ===========================================================================

#[test]
fn e2e_trace_replay_verify_outputs() {
    let inputs: Vec<f64> = vec![0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 1.0, 0.5, 0.0];
    let recording = record_pipeline_trace(&inputs);

    // Replay: advance through entire recording
    let mut player = TracePlayer::new(recording.clone());
    let events = player.advance_to(recording.duration());

    assert!(!events.is_empty(), "replay must yield events");

    // Re-process through the same pipeline and compare outputs
    let pipeline = standard_pipeline();
    let output_events: Vec<&TraceEvent> = events
        .iter()
        .filter(|e| e.event_type == TraceEventType::TelemetryUpdate)
        .copied()
        .collect();

    let input_events: Vec<&TraceEvent> = events
        .iter()
        .filter(|e| e.event_type == TraceEventType::AxisInput)
        .copied()
        .collect();

    for (inp, out) in input_events.iter().zip(output_events.iter()) {
        let replayed = pipeline.process(inp.data[0], 0.004);
        assert_approx_eq(
            out.data[0],
            replayed,
            1e-10,
        );
    }
}

// ===========================================================================
// 3. Trace format serialization/deserialization
// ===========================================================================

#[test]
fn e2e_trace_serialization_roundtrip() {
    let inputs: Vec<f64> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    let recording = record_pipeline_trace(&inputs);

    // Save to temp file
    let dir = flight_test_helpers::create_temp_dir("trace-serde");
    let trace_path = dir.path().join("test_trace.json");

    recording
        .save_to_file(&trace_path)
        .expect("save must succeed");

    // Load back
    let loaded =
        TraceRecording::load_from_file(&trace_path).expect("load must succeed");

    // Verify all fields match
    assert_eq!(loaded.name, recording.name);
    assert_eq!(loaded.event_count(), recording.event_count());
    assert_eq!(loaded.duration(), recording.duration());
    assert_eq!(loaded.device_id, recording.device_id);

    // Verify every event matches
    for (expected, actual) in recording.events.iter().zip(loaded.events.iter()) {
        assert_eq!(expected.timestamp_us, actual.timestamp_us);
        assert_eq!(expected.event_type, actual.event_type);
        assert_eq!(expected.source, actual.source);
        assert_eq!(expected.data.len(), actual.data.len());
        for (e, a) in expected.data.iter().zip(actual.data.iter()) {
            assert_approx_eq(*e, *a, 1e-15);
        }
    }
}

// ===========================================================================
// 4. Trace comparison (golden file style)
// ===========================================================================

#[test]
fn e2e_trace_golden_comparison() {
    let inputs: Vec<f64> = vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0];

    // Run pipeline twice — must produce identical traces
    let trace_a = record_pipeline_trace(&inputs);
    let trace_b = record_pipeline_trace(&inputs);

    // Exact comparison
    let comparator = TraceComparator::new();
    let diff = comparator.compare(&trace_a, &trace_b);
    assert!(
        diff.is_match(),
        "identical inputs must produce identical traces: {}",
        diff.report()
    );
    assert_eq!(diff.missing_events, 0);
    assert_eq!(diff.extra_events, 0);

    // Now create a slightly different trace
    let inputs_modified: Vec<f64> = vec![0.0, 0.2, 0.4, 0.6, 0.8, 0.99];
    let trace_c = record_pipeline_trace(&inputs_modified);

    // Exact comparison should fail
    let diff_exact = comparator.compare(&trace_a, &trace_c);
    assert!(
        !diff_exact.is_match(),
        "different inputs should produce different traces"
    );

    // Tolerance comparison should still detect the difference
    let tolerant_comparator = TraceComparator::within_tolerance(1e-6);
    let diff_tolerant = tolerant_comparator.compare(&trace_a, &trace_c);
    assert!(
        !diff_tolerant.is_match(),
        "0.99 vs 1.0 should differ beyond 1e-6"
    );

    // Very large tolerance should pass
    let wide_comparator = TraceComparator::within_tolerance(1.0);
    let diff_wide = wide_comparator.compare(&trace_a, &trace_c);
    assert!(
        diff_wide.is_match(),
        "1.0 tolerance should accept small differences"
    );

    // Verify golden snapshot store integration
    let mut store = SnapshotStore::new();
    let golden_json =
        serde_json::to_string_pretty(&trace_a).expect("serialize golden");
    store.record("pipeline-golden", &golden_json);

    let replay_json =
        serde_json::to_string_pretty(&trace_b).expect("serialize replay");
    let result = store.verify("pipeline-golden", &replay_json);
    assert_eq!(
        result,
        flight_test_helpers::SnapshotResult::Match,
        "golden snapshot must match"
    );
}
