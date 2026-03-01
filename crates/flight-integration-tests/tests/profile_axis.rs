// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile → Axis engine integration tests.
//!
//! Loads profiles, creates axis engine configurations from them,
//! processes sample inputs, and verifies outputs match expected
//! curves and deadzones.

use flight_axis::{AxisEngine, AxisFrame, PipelineBuilder};
use flight_core::profile::{AxisConfig, PROFILE_SCHEMA_VERSION, Profile};
use flight_test_helpers::{
    DeterministicClock, ProfileFixtureBuilder, assert_approx_eq, assert_in_range,
};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn profile_with_deadzone_and_expo(dz: f32, expo: f32) -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(dz),
            expo: Some(expo),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    }
}

// ===========================================================================
// 1. Profile deadzone applied through axis engine
// ===========================================================================

#[test]
fn profile_deadzone_zeros_small_inputs() {
    let profile = profile_with_deadzone_and_expo(0.10, 0.0);
    assert!(profile.validate().is_ok());

    let pitch_cfg = profile.axes.get("pitch").unwrap();
    let dz = pitch_cfg.deadzone.unwrap();

    let engine = AxisEngine::new_for_axis("pitch".to_string());
    let pipeline = PipelineBuilder::new()
        .deadzone(dz)
        .curve(0.0)
        .unwrap()
        .compile()
        .expect("compile");
    engine.update_pipeline(pipeline);

    // Values within 10% deadzone must produce zero output
    let test_inputs = [0.0_f32, 0.05, 0.09, -0.05, -0.09];
    for &raw in &test_inputs {
        let mut frame = AxisFrame::new(raw, 0);
        engine.process(&mut frame).expect("process");
        assert_eq!(
            frame.out, 0.0,
            "input {raw} within dz {dz} must output 0.0, got {}",
            frame.out
        );
    }

    // Values outside deadzone must produce non-zero output
    let mut frame = AxisFrame::new(0.5, 0);
    engine.process(&mut frame).expect("process");
    assert!(frame.out > 0.0, "0.5 outside dz must produce positive");
}

// ===========================================================================
// 2. Profile expo curve applied through axis engine
// ===========================================================================

#[test]
fn profile_expo_reduces_sensitivity_near_centre() {
    let profile = profile_with_deadzone_and_expo(0.03, 0.5);
    assert!(profile.validate().is_ok());

    let pitch_cfg = profile.axes.get("pitch").unwrap();

    let engine = AxisEngine::new_for_axis("pitch".to_string());
    let pipeline = PipelineBuilder::new()
        .deadzone(pitch_cfg.deadzone.unwrap())
        .curve(pitch_cfg.expo.unwrap())
        .unwrap()
        .compile()
        .expect("compile");
    engine.update_pipeline(pipeline);

    // With expo, mid-range inputs produce less output than linear
    let mut frame_mid = AxisFrame::new(0.5, 0);
    engine.process(&mut frame_mid).expect("process");

    // With expo 0.5, f(0.5) should be significantly less than 0.5
    assert!(
        frame_mid.out < 0.5,
        "expo should reduce mid-range: got {}",
        frame_mid.out
    );
    assert!(frame_mid.out > 0.0, "output must be positive");

    // Full deflection must still reach 1.0
    let mut frame_full = AxisFrame::new(1.0, 0);
    engine.process(&mut frame_full).expect("process");
    assert_approx_eq(frame_full.out as f64, 1.0, 1e-6);
}

// ===========================================================================
// 3. Profile cascade: merged profile applied to axis engine
// ===========================================================================

#[test]
fn profile_cascade_merged_deadzone_applied_to_engine() {
    let global = profile_with_deadzone_and_expo(0.05, 0.2);
    let aircraft_override = profile_with_deadzone_and_expo(0.02, 0.35);
    let merged = global
        .merge_with(&aircraft_override)
        .expect("merge must succeed");

    let pitch_cfg = merged.axes.get("pitch").unwrap();
    // Aircraft override should win
    assert_eq!(pitch_cfg.deadzone, Some(0.02));
    assert_eq!(pitch_cfg.expo, Some(0.35));

    let engine = AxisEngine::new_for_axis("pitch".to_string());
    let pipeline = PipelineBuilder::new()
        .deadzone(pitch_cfg.deadzone.unwrap())
        .curve(pitch_cfg.expo.unwrap())
        .unwrap()
        .compile()
        .expect("compile");
    engine.update_pipeline(pipeline);

    // 0.01 is inside 2% deadzone → zero
    let mut frame_in_dz = AxisFrame::new(0.01, 0);
    engine.process(&mut frame_in_dz).expect("process");
    assert_eq!(frame_in_dz.out, 0.0);

    // 0.03 is inside 5% global dz but outside 2% merged dz → non-zero
    let mut frame_out_dz = AxisFrame::new(0.1, 0);
    engine.process(&mut frame_out_dz).expect("process");
    assert!(frame_out_dz.out > 0.0, "0.1 outside 2% dz must produce output");
}

// ===========================================================================
// 4. Profile with fixture builder → axis engine
// ===========================================================================

#[test]
fn profile_fixture_builder_produces_valid_axis_config() {
    let fixture = ProfileFixtureBuilder::new("combat")
        .simulator("DCS")
        .aircraft("F-16C")
        .deadzone(0.04)
        .with_linear_curve()
        .build();

    assert_eq!(fixture.simulator, "DCS");
    assert_eq!(fixture.aircraft.as_deref(), Some("F-16C"));

    // Use fixture deadzone with axis engine
    let engine = AxisEngine::new_for_axis("pitch".to_string());
    let pipeline = PipelineBuilder::new()
        .deadzone(fixture.deadzone as f32)
        .curve(0.0)
        .unwrap()
        .compile()
        .expect("compile");
    engine.update_pipeline(pipeline);

    // Process a sweep of inputs
    let mut clock = DeterministicClock::new(0);
    for i in 0..=10 {
        let raw = i as f32 * 0.1;
        let ts_ns = clock.now_us() * 1_000;
        let mut frame = AxisFrame::new(raw, ts_ns);
        engine.process(&mut frame).expect("process");
        assert!(frame.out.is_finite());
        assert_in_range(frame.out as f64, -1.0, 1.0);
        clock.advance_ticks(1);
    }
}

// ===========================================================================
// 5. Multi-axis profile applied to independent engines
// ===========================================================================

#[test]
fn profile_multi_axis_independent_processing() {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.3),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.08),
            expo: Some(0.1),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    let profile = Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: None,
        aircraft: None,
        axes,
        pof_overrides: None,
    };
    assert!(profile.validate().is_ok());

    // Create independent engines for each axis
    let pitch_engine = AxisEngine::new_for_axis("pitch".to_string());
    let roll_engine = AxisEngine::new_for_axis("roll".to_string());

    let pitch_cfg = profile.axes.get("pitch").unwrap();
    let roll_cfg = profile.axes.get("roll").unwrap();

    pitch_engine.update_pipeline(
        PipelineBuilder::new()
            .deadzone(pitch_cfg.deadzone.unwrap())
            .curve(pitch_cfg.expo.unwrap())
            .unwrap()
            .compile()
            .expect("compile pitch"),
    );
    roll_engine.update_pipeline(
        PipelineBuilder::new()
            .deadzone(roll_cfg.deadzone.unwrap())
            .curve(roll_cfg.expo.unwrap())
            .unwrap()
            .compile()
            .expect("compile roll"),
    );

    // Same raw input produces different outputs due to different configs
    let raw = 0.5_f32;
    let mut pitch_frame = AxisFrame::new(raw, 0);
    let mut roll_frame = AxisFrame::new(raw, 0);
    pitch_engine.process(&mut pitch_frame).expect("pitch");
    roll_engine.process(&mut roll_frame).expect("roll");

    // Different expo values → different outputs
    assert!(
        (pitch_frame.out - roll_frame.out).abs() > 0.001,
        "different configs must produce different outputs: pitch={}, roll={}",
        pitch_frame.out,
        roll_frame.out
    );
}
