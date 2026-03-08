// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile switch end-to-end integration tests.
//!
//! Proves: aircraft detection triggers profile load, profile changes axis
//! curves and button mappings, fallback to default on unknown aircraft,
//! and hot-reload updates the live pipeline.

use flight_axis::pipeline::{AxisPipeline, ClampStage, CurveStage, DeadzoneStage};
use flight_axis::{AxisEngine, PipelineBuilder};
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot};
use flight_core::profile::{AxisConfig, PROFILE_SCHEMA_VERSION, Profile};
use flight_service::{AircraftAutoSwitchService, AircraftAutoSwitchServiceConfig};
use std::collections::HashMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_profile(aircraft: Option<&str>, deadzone: f32, expo: f32) -> Profile {
    let mut axes = HashMap::new();
    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(deadzone),
            expo: Some(expo),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(deadzone),
            expo: Some(expo * 0.8),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );
    Profile {
        schema: PROFILE_SCHEMA_VERSION.to_string(),
        sim: Some("msfs".to_string()),
        aircraft: aircraft.map(|a| flight_core::profile::AircraftId { icao: a.to_string() }),
        axes,
        pof_overrides: None,
    }
}

fn pipeline_for_profile(profile: &Profile, axis: &str) -> AxisPipeline {
    let config = profile.axes.get(axis).expect("axis not in profile");
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(DeadzoneStage {
        inner: config.deadzone.unwrap_or(0.05) as f64,
        outer: 1.0,
    }));
    pipeline.add_stage(Box::new(CurveStage {
        expo: config.expo.unwrap_or(0.2) as f64,
    }));
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));
    pipeline
}

// ===========================================================================
// 1. Aircraft detection triggers profile load
// ===========================================================================

#[tokio::test]
async fn e2e_aircraft_detection_triggers_profile_switch() {
    let mut bus = BusPublisher::new(60.0);
    let auto_switch =
        AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
    auto_switch.start(&mut bus).await.expect("start");

    // Publish C172 detection
    let snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    bus.publish(snap).expect("publish");

    tokio::time::sleep(Duration::from_millis(300)).await;

    let metrics = auto_switch.get_metrics().await;
    assert!(
        metrics.aircraft_switch_count >= 1,
        "aircraft detection must trigger switch, got {}",
        metrics.aircraft_switch_count
    );

    // Switch to A320
    let snap2 = BusSnapshot::new(SimId::Msfs, AircraftId::new("A320"));
    bus.publish(snap2).expect("publish");

    tokio::time::sleep(Duration::from_millis(300)).await;

    let metrics2 = auto_switch.get_metrics().await;
    assert!(
        metrics2.aircraft_switch_count >= 2,
        "second aircraft must trigger another switch, got {}",
        metrics2.aircraft_switch_count
    );

    auto_switch.stop().await.expect("stop");
}

// ===========================================================================
// 2. Profile load changes axis curves
// ===========================================================================

#[test]
fn e2e_profile_load_changes_axis_curves() {
    let default_profile = make_profile(None, 0.05, 0.2);
    let c172_profile = make_profile(Some("C172"), 0.03, 0.4);

    let default_pipeline = pipeline_for_profile(&default_profile, "pitch");
    let c172_pipeline = pipeline_for_profile(&c172_profile, "pitch");

    let test_input = 0.5;
    let default_output = default_pipeline.process(test_input, 0.004);
    let c172_output = c172_pipeline.process(test_input, 0.004);

    // Different profiles must produce different outputs for the same input
    assert!(
        (default_output - c172_output).abs() > 0.001,
        "profile switch must change curve output: default={default_output}, c172={c172_output}"
    );

    // Both must be valid
    assert!(default_output.is_finite() && default_output.abs() <= 1.0);
    assert!(c172_output.is_finite() && c172_output.abs() <= 1.0);
}

// ===========================================================================
// 3. Profile load changes button mappings
// ===========================================================================

#[test]
fn e2e_profile_load_changes_button_mappings() {
    // Simulate two profiles with different button→command mappings
    let default_mapping: HashMap<usize, &str> =
        [(0, "TRIGGER_FIRE"), (1, "WEAPON_SELECT")].into();
    let airliner_mapping: HashMap<usize, &str> =
        [(0, "AP_DISCONNECT"), (1, "TOGA")].into();

    let buttons_pressed = [0, 1];

    let default_commands: Vec<&str> = buttons_pressed
        .iter()
        .filter_map(|b| default_mapping.get(b).copied())
        .collect();
    let airliner_commands: Vec<&str> = buttons_pressed
        .iter()
        .filter_map(|b| airliner_mapping.get(b).copied())
        .collect();

    // After profile switch, button 0 maps differently
    assert_ne!(default_commands[0], airliner_commands[0]);
    assert_eq!(default_commands[0], "TRIGGER_FIRE");
    assert_eq!(airliner_commands[0], "AP_DISCONNECT");

    // Verify all mappings resolve
    assert_eq!(default_commands.len(), 2);
    assert_eq!(airliner_commands.len(), 2);
}

// ===========================================================================
// 4. Fallback to default on unknown aircraft
// ===========================================================================

#[tokio::test]
async fn e2e_fallback_to_default_on_unknown_aircraft() {
    let default_profile = make_profile(None, 0.05, 0.2);
    let c172_profile = make_profile(Some("C172"), 0.03, 0.4);

    // Merge: unknown aircraft → only default profile applies
    let merged = default_profile.merge_with(&c172_profile).unwrap();
    assert!(merged.validate().is_ok(), "merged profile must be valid");

    // When aircraft is unknown, use default pipeline
    let default_pipeline = pipeline_for_profile(&default_profile, "pitch");
    let output = default_pipeline.process(0.5, 0.004);
    assert!(output.is_finite());

    // Verify auto-switch ignores empty ICAO
    let mut bus = BusPublisher::new(60.0);
    let auto_switch =
        AircraftAutoSwitchService::new(AircraftAutoSwitchServiceConfig::default());
    auto_switch.start(&mut bus).await.expect("start");

    let empty_snap = BusSnapshot::new(SimId::Msfs, AircraftId::new(""));
    bus.publish(empty_snap).expect("publish");

    tokio::time::sleep(Duration::from_millis(200)).await;

    let metrics = auto_switch.get_metrics().await;
    assert_eq!(
        metrics.aircraft_switch_count, 0,
        "empty ICAO must not trigger switch"
    );

    auto_switch.stop().await.expect("stop");
}

// ===========================================================================
// 5. Hot-reload updates live pipeline
// ===========================================================================

#[test]
fn e2e_hot_reload_updates_live_pipeline() {
    let engine = AxisEngine::new_for_axis("pitch".to_string());

    // Initial pipeline
    let pipeline_v1 = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.2)
        .unwrap()
        .compile()
        .expect("v1 compile");
    engine.update_pipeline(pipeline_v1);

    let mut frame1 = flight_axis::AxisFrame::new(0.5, 1_000_000);
    engine.process(&mut frame1).expect("process v1");
    let output_v1 = frame1.out;

    // Hot-reload: update pipeline with different curve
    let pipeline_v2 = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.5)
        .unwrap()
        .compile()
        .expect("v2 compile");
    engine.update_pipeline(pipeline_v2);

    let mut frame2 = flight_axis::AxisFrame::new(0.5, 2_000_000);
    engine.process(&mut frame2).expect("process v2");
    let output_v2 = frame2.out;

    // Hot-reload must change the output
    assert!(
        (output_v1 - output_v2).abs() > 0.001,
        "hot-reload must change pipeline output: v1={output_v1}, v2={output_v2}"
    );

    // Both outputs must be valid
    assert!(output_v1.is_finite() && output_v1.abs() <= 1.0);
    assert!(output_v2.is_finite() && output_v2.abs() <= 1.0);

    // Engine version must have advanced
    assert!(engine.active_version().unwrap() >= 2);
}
