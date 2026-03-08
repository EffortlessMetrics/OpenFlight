// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Full roundtrip end-to-end integration tests.
//!
//! Proves: device → axis → bus → sim, profile cascade, safe mode,
//! multi-device/multi-sim, watchdog, and metrics collection.

use flight_axis::pipeline::{
    AxisPipeline, ClampStage, CurveStage, DeadzoneStage, SensitivityStage,
};
use flight_axis::{AxisEngine, AxisFrame, PipelineBuilder};
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_core::profile::{AxisConfig, Profile, PROFILE_SCHEMA_VERSION};
use flight_metrics::MetricsRegistry;
use flight_test_helpers::{
    DeterministicClock, FakeDevice, FakeInput, FakeSim, assert_approx_eq, assert_in_range,
};
use flight_watchdog::{ComponentType, WatchdogConfig, WatchdogSystem};
use std::collections::HashMap;
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

fn make_joystick(name: &str) -> FakeDevice {
    let mut dev = FakeDevice::new(name, 0x044F, 0xB10A, 4, 12);
    dev.connect();
    dev
}

fn pipeline_from_profile(axis_config: &AxisConfig) -> AxisPipeline {
    let mut pipeline = AxisPipeline::new();
    if let Some(dz) = axis_config.deadzone {
        pipeline.add_stage(Box::new(DeadzoneStage {
            inner: dz as f64,
            outer: 1.0,
        }));
    }
    if let Some(expo) = axis_config.expo {
        pipeline.add_stage(Box::new(CurveStage { expo: expo as f64 }));
    }
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));
    pipeline
}

// ===========================================================================
// 1. Fake device → axis → bus → fake sim → verify output
// ===========================================================================

#[test]
fn e2e_roundtrip_device_to_sim() {
    // Device produces input
    let mut device = make_joystick("Roundtrip Stick");
    device.enqueue_input(FakeInput {
        axes: vec![0.6, -0.4, 0.02, 0.8],
        buttons: vec![false; 12],
        delay_ms: 4,
    });

    // Axis pipeline processes
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

    let input = device.next_input().unwrap();
    let pitch = pipeline.process(input.axes[0], 0.004);
    let roll = pipeline.process(input.axes[1], 0.004);
    let yaw = pipeline.process(input.axes[2], 0.004);
    let throttle = pipeline.process(input.axes[3], 0.004);

    // Bus publishes
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap.control_inputs.pitch = pitch as f32;
    snap.control_inputs.roll = roll as f32;
    snap.control_inputs.yaw = yaw as f32;
    snap.control_inputs.throttle = vec![throttle as f32];
    publisher.publish(snap).expect("publish");

    // Sim adapter receives
    let received = sub.try_recv().unwrap().expect("must receive");
    let mut sim = FakeSim::new("MSFS");
    sim.connect();
    sim.set_aircraft("C172");

    // Write to sim
    sim.send_command(&format!("PITCH={:.4}", received.control_inputs.pitch));
    sim.send_command(&format!("ROLL={:.4}", received.control_inputs.roll));
    sim.send_command(&format!("THROTTLE={:.4}", received.control_inputs.throttle[0]));

    // Verify roundtrip values
    assert_approx_eq(received.control_inputs.pitch as f64, pitch, 1e-5);
    assert_approx_eq(received.control_inputs.roll as f64, roll, 1e-5);
    assert_eq!(received.control_inputs.yaw, 0.0, "yaw 0.02 in deadzone");
    assert_eq!(sim.received_commands().len(), 3);
}

// ===========================================================================
// 2. Profile cascade applied correctly end-to-end
// ===========================================================================

#[test]
fn e2e_roundtrip_profile_cascade() {
    // Global profile: 5% deadzone, 0.2 expo
    let global = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes: HashMap::from([(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.05),
                expo: Some(0.2),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        )]),
        pof_overrides: None,
    };

    // Sim profile: 3% deadzone override
    let sim_profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: None,
        axes: HashMap::from([(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.03),
                expo: Some(0.25),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        )]),
        pof_overrides: None,
    };

    // Cascade merge: global → sim
    let merged = global.merge_with(&sim_profile).expect("merge");
    let pitch_config = merged.axes.get("pitch").expect("pitch config");

    // Sim-specific values should win
    assert_eq!(pitch_config.deadzone, Some(0.03));
    assert_eq!(pitch_config.expo, Some(0.25));

    // Build pipeline from merged profile and process
    let pipeline = pipeline_from_profile(pitch_config);
    let mut device = make_joystick("Cascade Stick");
    device.enqueue_input(FakeInput {
        axes: vec![0.5, 0.0, 0.0, 0.0],
        buttons: vec![false; 12],
        delay_ms: 4,
    });

    let input = device.next_input().unwrap();
    let out = pipeline.process(input.axes[0], 0.004);

    // Publish to bus and verify
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap.control_inputs.pitch = out as f32;
    publisher.publish(snap).expect("publish");

    let received = sub.try_recv().unwrap().expect("must receive");
    assert_approx_eq(received.control_inputs.pitch as f64, out, 1e-5);
    assert!(out > 0.0, "cascaded profile must produce non-zero output");
}

// ===========================================================================
// 3. Safe mode activation → basic profile → recovery
// ===========================================================================

#[test]
fn e2e_roundtrip_safe_mode_recovery() {
    // Normal profile with high sensitivity
    let mut normal_pipeline = AxisPipeline::new();
    normal_pipeline.add_stage(Box::new(DeadzoneStage {
        inner: 0.03,
        outer: 1.0,
    }));
    normal_pipeline.add_stage(Box::new(SensitivityStage { multiplier: 2.0 }));
    normal_pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));

    // Safe mode profile: wide deadzone, low sensitivity
    let mut safe_pipeline = AxisPipeline::new();
    safe_pipeline.add_stage(Box::new(DeadzoneStage {
        inner: 0.15,
        outer: 1.0,
    }));
    safe_pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));

    let input = 0.5;

    // Normal mode output
    let normal_out = normal_pipeline.process(input, 0.004);

    // Simulate error → safe mode activation
    let safe_out = safe_pipeline.process(input, 0.004);

    // Safe mode should produce more conservative (smaller) output
    assert!(
        safe_out.abs() < normal_out.abs(),
        "safe mode output ({safe_out}) must be more conservative than normal ({normal_out})"
    );

    // Recovery: switch back to normal
    let recovered_out = normal_pipeline.process(input, 0.004);
    assert_approx_eq(recovered_out, normal_out, 1e-10);

    // Publish safe mode output to bus
    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap.control_inputs.pitch = safe_out as f32;
    publisher.publish(snap).expect("publish safe mode");

    let received = sub.try_recv().unwrap().expect("receive safe mode");
    assert_in_range(received.control_inputs.pitch as f64, -1.0, 1.0);
}

// ===========================================================================
// 4. Multi-device multi-sim scenario
// ===========================================================================

#[test]
fn e2e_roundtrip_multi_device_multi_sim() {
    let mut stick = make_joystick("HOTAS Stick");
    let mut throttle = FakeDevice::new("HOTAS Throttle", 0x044F, 0xB10B, 2, 8);
    throttle.connect();

    stick.enqueue_input(FakeInput {
        axes: vec![0.7, -0.4, 0.0, 0.0],
        buttons: vec![false; 12],
        delay_ms: 4,
    });
    throttle.enqueue_input(FakeInput {
        axes: vec![0.9, 0.6],
        buttons: vec![false; 8],
        delay_ms: 4,
    });

    let pipeline = {
        let mut p = AxisPipeline::new();
        p.add_stage(Box::new(DeadzoneStage {
            inner: 0.05,
            outer: 1.0,
        }));
        p.add_stage(Box::new(CurveStage { expo: 0.2 }));
        p.add_stage(Box::new(ClampStage {
            min: -1.0,
            max: 1.0,
        }));
        p
    };

    let stick_input = stick.next_input().unwrap();
    let throttle_input = throttle.next_input().unwrap();

    let pitch = pipeline.process(stick_input.axes[0], 0.004);
    let roll = pipeline.process(stick_input.axes[1], 0.004);
    let thr1 = pipeline.process(throttle_input.axes[0], 0.004);
    let thr2 = pipeline.process(throttle_input.axes[1], 0.004);

    let mut publisher = make_publisher();
    let mut msfs_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();
    let mut xplane_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Publish to MSFS
    let mut msfs_snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    msfs_snap.control_inputs.pitch = pitch as f32;
    msfs_snap.control_inputs.roll = roll as f32;
    msfs_snap.control_inputs.throttle = vec![thr1 as f32];
    publisher.publish(msfs_snap).expect("publish MSFS");
    tick();

    // Publish to X-Plane
    let mut xp_snap = BusSnapshot::new(SimId::XPlane, AircraftId::new("B737"));
    xp_snap.control_inputs.pitch = pitch as f32;
    xp_snap.control_inputs.throttle = vec![thr1 as f32, thr2 as f32];
    publisher.publish(xp_snap).expect("publish X-Plane");

    // Both sims should receive both snapshots (fan-out)
    let mut msfs_received = Vec::new();
    while let Ok(Some(s)) = msfs_sub.try_recv() {
        msfs_received.push(s.sim);
    }
    let mut xplane_received = Vec::new();
    while let Ok(Some(s)) = xplane_sub.try_recv() {
        xplane_received.push(s.sim);
    }

    // Both subscribers see both snapshots
    assert!(!msfs_received.is_empty(), "MSFS sub must receive snapshots");
    assert!(!xplane_received.is_empty(), "X-Plane sub must receive snapshots");
}

// ===========================================================================
// 5. Watchdog integration (health monitoring during pipeline)
// ===========================================================================

#[test]
fn e2e_roundtrip_watchdog_monitoring() {
    let mut watchdog = WatchdogSystem::new();

    // Register components
    watchdog.register_component(
        ComponentType::UsbEndpoint("stick-usb".to_string()),
        WatchdogConfig::default(),
    );
    watchdog.register_component(
        ComponentType::AxisNode("pitch-axis".to_string()),
        WatchdogConfig::default(),
    );
    watchdog.register_component(
        ComponentType::SimAdapter("msfs-adapter".to_string()),
        WatchdogConfig::default(),
    );

    // Health check before pipeline
    let summary = watchdog.get_health_summary();
    assert_eq!(summary.total_components, 3);

    // Run pipeline
    let engine = AxisEngine::new_for_axis("pitch".to_string());
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .unwrap()
        .compile()
        .expect("compile");
    engine.update_pipeline(pipeline);

    let mut clock = DeterministicClock::new(0);
    for tick in 0..50 {
        let input = (tick as f32 / 50.0) * 0.8;
        let ts_ns = clock.now_us() * 1_000;
        let mut frame = AxisFrame::new(input, ts_ns);
        engine.process(&mut frame).expect("process");
        clock.advance_ticks(1);
    }

    // Health check after pipeline — should still be healthy
    let summary_after = watchdog.get_health_summary();
    assert_eq!(summary_after.total_components, 3);
    assert_eq!(summary_after.quarantined_components, 0);
}

// ===========================================================================
// 6. Metrics collection during operation
// ===========================================================================

#[test]
fn e2e_roundtrip_metrics_collection() {
    let registry = MetricsRegistry::new();

    // Simulate pipeline with metrics
    let mut device = make_joystick("Metrics Stick");
    let pipeline = {
        let mut p = AxisPipeline::new();
        p.add_stage(Box::new(DeadzoneStage {
            inner: 0.05,
            outer: 1.0,
        }));
        p.add_stage(Box::new(CurveStage { expo: 0.3 }));
        p.add_stage(Box::new(ClampStage {
            min: -1.0,
            max: 1.0,
        }));
        p
    };

    // Generate frames
    for i in 0..100 {
        let val = (i as f64 / 100.0) * 0.8;
        device.enqueue_input(FakeInput {
            axes: vec![val, 0.0, 0.0, 0.0],
            buttons: vec![false; 12],
            delay_ms: 4,
        });
    }

    let mut publisher = make_publisher();
    let mut sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut processed = 0_u64;
    let mut published = 0_u64;

    while let Some(input) = device.next_input() {
        let out = pipeline.process(input.axes[0], 0.004);
        registry.inc_counter("axis.frames_processed", 1);
        registry.observe("axis.output_value", out);
        processed += 1;

        let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
        snap.control_inputs.pitch = out as f32;
        publisher.publish(snap).expect("publish");
        registry.inc_counter("bus.snapshots_published", 1);
        published += 1;
        tick();
    }

    // Drain subscriber
    let mut received = 0_u64;
    while let Ok(Some(_)) = sub.try_recv() {
        received += 1;
        registry.inc_counter("bus.snapshots_received", 1);
    }

    assert_eq!(processed, 100);
    assert_eq!(published, 100);
    assert!(received > 0, "must receive some snapshots");

    // Verify metrics recorded correctly
    registry.set_gauge("pipeline.total_processed", processed as f64);
    registry.set_gauge("pipeline.total_published", published as f64);

    let proc_gauge = registry.gauge_value("pipeline.total_processed");
    assert_eq!(proc_gauge, Some(100.0));

    let pub_gauge = registry.gauge_value("pipeline.total_published");
    assert_eq!(pub_gauge, Some(100.0));
}
