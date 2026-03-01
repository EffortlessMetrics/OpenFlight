// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! End-to-end integration test: FakeHidDevice → axis pipeline → FakeSimAdapter.

use flight_axis::pipeline::{AxisPipeline, ClampStage, CurveStage, DeadzoneStage};
use flight_testkit::{
    DeviceBuilder, FakeClock, FakeHidDevice, FakeSimAdapter, HidReport, ProfileBuilder,
};

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

/// Full round-trip: fake device → pipeline stages → fake sim adapter.
#[test]
fn device_to_pipeline_to_adapter() {
    let clock = FakeClock::new();

    // Build a device spec and create a matching FakeHidDevice with scripted input.
    let spec = DeviceBuilder::new("Test Joystick").axes(3).buttons(0).hats(0).build();
    let reports: Vec<HidReport> = [0.0, 0.03, 0.25, 0.5, 0.75, 1.0, -0.5, -1.0]
        .iter()
        .map(|&v| HidReport {
            axes: vec![v, 0.0, 0.0],
            buttons: vec![],
            hats: vec![],
        })
        .collect();

    let mut device = FakeHidDevice::new(
        &spec.name,
        spec.vid,
        spec.pid,
        spec.axis_count,
        spec.button_count,
        spec.hat_count,
    )
    .with_script(reports);

    let pipeline = standard_pipeline();
    let mut adapter = FakeSimAdapter::new("MSFS").with_bounds(-1.0, 1.0);

    // Process all reports through the pipeline and write to the adapter.
    while let Some(report) = device.next_report() {
        device.ack_received();
        let processed = pipeline.process(report.axes[0], 0.004);
        let ts = clock.now();
        adapter.write_axis(0, processed, ts).unwrap();
        clock.tick();
    }

    // Verify device counters.
    assert_eq!(device.reports_sent(), 8);
    assert_eq!(device.reports_received(), 8);

    // Verify adapter recorded all outputs.
    let outputs = adapter.recordings();
    assert_eq!(outputs.len(), 8);

    // Values inside deadzone should be zero.
    assert!(outputs[0].value.abs() < f64::EPSILON);
    assert!(outputs[1].value.abs() < f64::EPSILON);

    // All outputs within bounds.
    for rec in outputs {
        assert!(
            (-1.0..=1.0).contains(&rec.value),
            "out-of-bounds: {}",
            rec.value
        );
    }

    // Timestamps should be 4 ms (4000 µs) apart.
    for pair in outputs.windows(2) {
        let delta = pair[1].timestamp_us - pair[0].timestamp_us;
        assert_eq!(delta, 4_000, "expected 4ms tick spacing");
    }
}

/// Adapter disconnect mid-stream should not lose earlier data.
#[test]
fn disconnect_preserves_prior_recordings() {
    let clock = FakeClock::new();
    let mut adapter = FakeSimAdapter::new("MSFS").with_bounds(-1.0, 1.0);
    let pipeline = standard_pipeline();

    // Write a few good values.
    for &v in &[0.5, 0.6, 0.7] {
        let processed = pipeline.process(v, 0.004);
        adapter.write_axis(0, processed, clock.now()).unwrap();
        clock.tick();
    }

    // Disconnect.
    adapter.simulate_disconnect(clock.now());
    let result = adapter.write_axis(0, 0.0, clock.now());
    assert!(result.is_err());

    // Reconnect and write more.
    adapter.simulate_reconnect(clock.now());
    clock.tick();
    adapter.write_axis(0, 0.0, clock.now()).unwrap();

    // Prior recordings must still be present.
    assert_eq!(adapter.recordings().len(), 4);
    assert_eq!(adapter.disconnect_events().len(), 1);
    assert_eq!(adapter.reconnect_events().len(), 1);
}

/// ProfileBuilder produces a profile whose parameters influence the pipeline.
#[test]
fn profile_builder_drives_pipeline_config() {
    let profile = ProfileBuilder::new("test").combat().build();
    assert_eq!(profile.simulator, "DCS");
    assert!((profile.deadzone - 0.03).abs() < f64::EPSILON);
    assert!((profile.curve_expo - 0.3).abs() < f64::EPSILON);
}

/// FakeClock clones share the same underlying time.
#[test]
fn shared_clock_between_device_and_adapter() {
    let clock = FakeClock::new();
    let device_clock = clock.clone();
    let adapter_clock = clock.clone();

    clock.tick_n(10);
    assert_eq!(device_clock.now(), adapter_clock.now());
    assert_eq!(device_clock.now(), 40_000);
}
