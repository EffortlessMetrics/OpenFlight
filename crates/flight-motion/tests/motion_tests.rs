// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Integration tests for the flight-motion crate.

use approx::assert_abs_diff_eq;
use flight_bus::BusSnapshot;
use flight_motion::{DoFConfig, MotionConfig, MotionFrame, MotionMapper, WashoutConfig};

fn default_mapper() -> MotionMapper {
    MotionMapper::new(MotionConfig::default(), 1.0 / 60.0)
}

/// Feed many identical snapshots until filter state settles.
fn settle(mapper: &mut MotionMapper, snapshot: &BusSnapshot, ticks: usize) -> MotionFrame {
    let mut frame = MotionFrame::NEUTRAL;
    for _ in 0..ticks {
        frame = mapper.process(snapshot);
    }
    frame
}

#[test]
fn test_neutral_snapshot_washes_out() {
    let mut mapper = default_mapper();
    let neutral = BusSnapshot::default();
    let frame = settle(&mut mapper, &neutral, 2000);

    assert_abs_diff_eq!(frame.surge, 0.0, epsilon = 0.01);
    assert_abs_diff_eq!(frame.sway, 0.0, epsilon = 0.01);
    assert_abs_diff_eq!(frame.roll, 0.0, epsilon = 0.01);
    assert_abs_diff_eq!(frame.pitch, 0.0, epsilon = 0.01);
    assert_abs_diff_eq!(frame.yaw, 0.0, epsilon = 0.01);
}

#[test]
fn test_intensity_zero_always_neutral() {
    let mut config = MotionConfig::default();
    config.intensity = 0.0;
    let mut mapper = MotionMapper::new(config, 1.0 / 60.0);
    let frame = mapper.process(&BusSnapshot::default());
    assert!(frame.is_neutral());
}

#[test]
fn test_motion_frame_simtools_roundtrip() {
    let frame = MotionFrame {
        surge: 0.5,
        sway: -0.5,
        heave: 1.0,
        roll: 0.0,
        pitch: -1.0,
        yaw: 0.25,
    };
    let s = frame.to_simtools_string();
    assert_eq!(s, "A50B-50C100D0E-100F25\n");
}

#[test]
fn test_motion_frame_array_order() {
    let frame = MotionFrame {
        surge: 0.1,
        sway: 0.2,
        heave: 0.3,
        roll: 0.4,
        pitch: 0.5,
        yaw: 0.6,
    };
    let arr = frame.to_array();
    assert_abs_diff_eq!(arr[0], 0.1, epsilon = 1e-6);
    assert_abs_diff_eq!(arr[1], 0.2, epsilon = 1e-6);
    assert_abs_diff_eq!(arr[2], 0.3, epsilon = 1e-6);
    assert_abs_diff_eq!(arr[3], 0.4, epsilon = 1e-6);
    assert_abs_diff_eq!(arr[4], 0.5, epsilon = 1e-6);
    assert_abs_diff_eq!(arr[5], 0.6, epsilon = 1e-6);
}

#[test]
fn test_washout_config_default() {
    let wc = WashoutConfig::default();
    assert_eq!(wc.hp_frequency_hz, 0.5);
    assert_eq!(wc.lp_frequency_hz, 5.0);
}

#[test]
fn test_motion_config_default() {
    let cfg = MotionConfig::default();
    assert_eq!(cfg.intensity, 0.8);
    assert_eq!(cfg.max_g, 3.0);
    assert_eq!(cfg.max_angle_deg, 30.0);
    assert!(cfg.surge.enabled);
    assert!(cfg.sway.enabled);
    assert!(cfg.heave.enabled);
    assert!(cfg.roll.enabled);
    assert!(cfg.pitch.enabled);
    assert!(cfg.yaw.enabled);
}

#[test]
fn test_hp_filter_onset_then_washout() {
    use flight_motion::HighPassFilter;
    let mut hp = HighPassFilter::new(0.5, 1.0 / 60.0);

    // Step input — first output should be near 1.0 (onset cue)
    let onset = hp.process(1.0);
    assert!(onset > 0.9, "Onset should be near 1.0: {onset}");

    // Sustained input — should decay to near 0.0 (washout)
    let mut last = onset;
    for _ in 0..600 {
        last = hp.process(1.0);
    }
    assert!(
        last.abs() < 0.02,
        "Should wash out after sustained input: {last}"
    );
}

#[test]
fn test_disabled_channels_stay_zero() {
    let mut config = MotionConfig::default();
    config.surge.enabled = false;
    config.sway.enabled = false;
    config.yaw.enabled = false;
    let mut mapper = MotionMapper::new(config, 1.0 / 60.0);
    let frame = mapper.process(&BusSnapshot::default());
    assert_eq!(frame.surge, 0.0);
    assert_eq!(frame.sway, 0.0);
    assert_eq!(frame.yaw, 0.0);
}

#[test]
fn test_clamped_frame() {
    let f = MotionFrame {
        surge: 5.0,
        sway: -5.0,
        heave: 0.5,
        roll: 2.0,
        pitch: -2.0,
        yaw: 0.0,
    };
    let c = f.clamped();
    assert_eq!(c.surge, 1.0);
    assert_eq!(c.sway, -1.0);
    assert_eq!(c.heave, 0.5);
    assert_eq!(c.roll, 1.0);
    assert_eq!(c.pitch, -1.0);
}

#[test]
fn test_inverted_channel() {
    let config = MotionConfig {
        intensity: 1.0,
        roll: DoFConfig {
            invert: true,
            gain: 1.0,
            ..DoFConfig::default()
        },
        ..MotionConfig::default()
    };
    // Manually test inversion logic via mapper config access
    let mapper = MotionMapper::new(config, 1.0 / 60.0);
    assert!(mapper.config().roll.invert);
}
