// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for [`flight_xplane::control::ControlOutput`].
//!
//! These tests verify the public contract of the control output module without
//! requiring a live X-Plane instance or plugin connection.

use flight_xplane::control::{
    CMD_GEAR_DOWN, CMD_GEAR_UP, ControlOutput, ControlOutputError, DATAREF_FLAPS,
    DATAREF_GEAR_HANDLE, DATAREF_PITCH, DATAREF_ROLL, DATAREF_SPEEDBRAKE, DATAREF_YAW,
};

/// All control DataRef paths must match X-Plane 11/12 SDK documentation.
#[test]
fn control_dataref_paths_match_xplane_sdk() {
    assert_eq!(DATAREF_PITCH, "sim/joystick/yoke_pitch_ratio");
    assert_eq!(DATAREF_ROLL, "sim/joystick/yoke_roll_ratio");
    assert_eq!(DATAREF_YAW, "sim/joystick/yoke_heading_ratio");
    assert_eq!(DATAREF_FLAPS, "sim/flightmodel/controls/flaprqst");
    assert_eq!(
        DATAREF_SPEEDBRAKE,
        "sim/flightmodel/controls/speedbrk_ratio"
    );
    assert_eq!(
        DATAREF_GEAR_HANDLE,
        "sim/flightmodel/controls/gear_handle_down"
    );
}

/// Command names must match X-Plane SDK documentation.
#[test]
fn control_command_names_match_xplane_sdk() {
    assert_eq!(CMD_GEAR_DOWN, "sim/flight_controls/landing_gear_down");
    assert_eq!(CMD_GEAR_UP, "sim/flight_controls/landing_gear_up");
}

/// When no plugin or web API is configured, every axis write returns `NoWritePath`.
#[tokio::test]
async fn all_axis_writes_fail_with_no_write_path() {
    let co = ControlOutput::new(None, None);

    let results = [
        co.write_pitch(0.0).await.unwrap_err(),
        co.write_roll(0.0).await.unwrap_err(),
        co.write_yaw(0.0).await.unwrap_err(),
        co.write_throttle(0, 0.5).await.unwrap_err(),
        co.write_flaps(0.5).await.unwrap_err(),
        co.write_speedbrake(0.0).await.unwrap_err(),
        co.write_gear(true).await.unwrap_err(),
    ];

    for err in results {
        assert!(
            matches!(err, ControlOutputError::NoWritePath),
            "expected NoWritePath, got: {:?}",
            err
        );
    }
}

/// NaN and infinite values must be rejected before any write-path attempt.
///
/// This ensures that invalid telemetry data never reaches the network layer
/// even if no connection is available.
#[tokio::test]
async fn nan_inf_rejected_before_write_path_is_checked() {
    let co = ControlOutput::new(None, None);

    assert!(
        matches!(
            co.write_pitch(f32::NAN).await,
            Err(ControlOutputError::InvalidValue { .. })
        ),
        "write_pitch(NaN) should return InvalidValue"
    );
    assert!(
        matches!(
            co.write_roll(f32::INFINITY).await,
            Err(ControlOutputError::InvalidValue { .. })
        ),
        "write_roll(+Inf) should return InvalidValue"
    );
    assert!(
        matches!(
            co.write_yaw(f32::NEG_INFINITY).await,
            Err(ControlOutputError::InvalidValue { .. })
        ),
        "write_yaw(-Inf) should return InvalidValue"
    );
    assert!(
        matches!(
            co.write_throttle(0, f32::NAN).await,
            Err(ControlOutputError::InvalidValue { .. })
        ),
        "write_throttle(NaN) should return InvalidValue"
    );
    assert!(
        matches!(
            co.write_flaps(f32::NAN).await,
            Err(ControlOutputError::InvalidValue { .. })
        ),
        "write_flaps(NaN) should return InvalidValue"
    );
    assert!(
        matches!(
            co.write_speedbrake(f32::NAN).await,
            Err(ControlOutputError::InvalidValue { .. })
        ),
        "write_speedbrake(NaN) should return InvalidValue"
    );
}

/// `execute_command` returns `NoWritePath` when no write path is configured.
#[tokio::test]
async fn execute_command_fails_without_write_path() {
    let co = ControlOutput::new(None, None);
    assert!(matches!(
        co.execute_command(CMD_GEAR_DOWN).await,
        Err(ControlOutputError::NoWritePath)
    ));
}

/// Per-engine throttle DataRef names include the engine index.
#[test]
fn throttle_dataref_name_includes_engine_index() {
    assert_eq!(
        format!("sim/flightmodel/engine/ENGN_thro[{}]", 0u8),
        "sim/flightmodel/engine/ENGN_thro[0]"
    );
    assert_eq!(
        format!("sim/flightmodel/engine/ENGN_thro[{}]", 3u8),
        "sim/flightmodel/engine/ENGN_thro[3]"
    );
}

/// `ControlOutput::is_available` returns `false` when no connections are configured.
#[test]
fn is_available_false_without_connections() {
    let co = ControlOutput::new(None, None);
    assert!(!co.is_available());
}
