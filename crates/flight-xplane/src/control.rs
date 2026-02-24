// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X-Plane control output — writes Flight Hub processed inputs back into X-Plane.
//!
//! Dispatches axis and command data to X-Plane via the plugin interface (TCP
//! port 52000) or the web API fallback. All DataRef paths follow the
//! X-Plane 11/12 SDK.
//!
//! ## Control DataRef paths
//!
//! | Axis | DataRef | Range |
//! |------|---------|-------|
//! | Pitch | [`DATAREF_PITCH`] | −1..+1 |
//! | Roll | [`DATAREF_ROLL`] | −1..+1 |
//! | Yaw | [`DATAREF_YAW`] | −1..+1 |
//! | Throttle | `sim/flightmodel/engine/ENGN_thro[{n}]` | 0..1 |
//! | Flaps | [`DATAREF_FLAPS`] | 0..1 |
//! | Speedbrake | [`DATAREF_SPEEDBRAKE`] | 0..1 |
//! | Gear handle | [`DATAREF_GEAR_HANDLE`] | int 0/1 |

use crate::{
    dataref::DataRefValue,
    plugin::{PluginError, PluginInterface},
    web_api::{WebApiClient, WebApiError},
};
use thiserror::Error;
use tracing::{debug, warn};

// ── DataRef name constants ────────────────────────────────────────────────────

/// Elevator/pitch yoke ratio: −1 (full forward) to +1 (full back).
pub const DATAREF_PITCH: &str = "sim/joystick/yoke_pitch_ratio";

/// Aileron/roll yoke ratio: −1 (full left) to +1 (full right).
pub const DATAREF_ROLL: &str = "sim/joystick/yoke_roll_ratio";

/// Rudder/yaw yoke ratio: −1 (left rudder) to +1 (right rudder).
pub const DATAREF_YAW: &str = "sim/joystick/yoke_heading_ratio";

/// Speedbrake/spoiler ratio: 0 (retracted) to 1 (fully deployed).
pub const DATAREF_SPEEDBRAKE: &str = "sim/flightmodel/controls/speedbrk_ratio";

/// Flap deployment request: 0 (retracted) to 1 (full extension).
pub const DATAREF_FLAPS: &str = "sim/flightmodel/controls/flaprqst";

/// Landing-gear handle: 0 = retracted, 1 = extended.
pub const DATAREF_GEAR_HANDLE: &str = "sim/flightmodel/controls/gear_handle_down";

// ── Command name constants ────────────────────────────────────────────────────

/// Lower the landing gear.
pub const CMD_GEAR_DOWN: &str = "sim/flight_controls/landing_gear_down";

/// Raise the landing gear.
pub const CMD_GEAR_UP: &str = "sim/flight_controls/landing_gear_up";

/// Retract flaps one notch.
pub const CMD_FLAPS_UP: &str = "sim/flight_controls/flaps_up";

/// Extend flaps one notch.
pub const CMD_FLAPS_DOWN: &str = "sim/flight_controls/flaps_down";

/// Toggle speedbrakes/spoilers.
pub const CMD_SPEEDBRAKES_TOGGLE: &str = "sim/flight_controls/speed_brakes_toggle";

/// Toggle thrust reversers.
pub const CMD_THRUST_REVERSERS: &str = "sim/flight_controls/thrust_reverse_toggle";

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors returned by [`ControlOutput`] write operations.
#[derive(Error, Debug)]
pub enum ControlOutputError {
    /// No write path is available (plugin not connected, web API not configured).
    #[error("no write path available (plugin not connected, web API not configured)")]
    NoWritePath,
    /// Plugin communication error.
    #[error("plugin error: {0}")]
    Plugin(#[from] PluginError),
    /// Web API communication error.
    #[error("web API error: {0}")]
    WebApi(#[from] WebApiError),
    /// Axis value is NaN or infinite.
    #[error("axis value {value:.3} is invalid (NaN or Infinite)")]
    InvalidValue { value: f32 },
}

// ── ControlOutput ─────────────────────────────────────────────────────────────

/// Writes processed Flight Hub control inputs back into X-Plane.
///
/// Prefers the plugin interface (lower latency, bidirectional) and falls back
/// to the X-Plane web API when the plugin is not connected.
#[derive(Clone)]
pub struct ControlOutput {
    plugin: Option<PluginInterface>,
    web_api: Option<WebApiClient>,
}

impl ControlOutput {
    /// Create a new control output using the given write paths.
    ///
    /// At least one of `plugin` or `web_api` should be `Some`; otherwise every
    /// write returns [`ControlOutputError::NoWritePath`].
    pub fn new(plugin: Option<PluginInterface>, web_api: Option<WebApiClient>) -> Self {
        Self { plugin, web_api }
    }

    /// Return `true` if at least one write path is currently available.
    pub fn is_available(&self) -> bool {
        self.plugin
            .as_ref()
            .map(|p| p.is_connected())
            .unwrap_or(false)
            || self.web_api.is_some()
    }

    // ── axis writes ──────────────────────────────────────────────────────────

    /// Write a pitch axis value in `[-1.0, +1.0]`.
    ///
    /// Positive = back-stick (nose up), negative = forward-stick (nose down).
    /// Out-of-range values are clamped; NaN/Inf returns [`ControlOutputError::InvalidValue`].
    pub async fn write_pitch(&self, value: f32) -> Result<(), ControlOutputError> {
        let v = Self::sanitize(value)?.clamp(-1.0, 1.0);
        self.write_float(DATAREF_PITCH, v).await
    }

    /// Write a roll axis value in `[-1.0, +1.0]`.
    ///
    /// Positive = right stick (right bank), negative = left stick (left bank).
    pub async fn write_roll(&self, value: f32) -> Result<(), ControlOutputError> {
        let v = Self::sanitize(value)?.clamp(-1.0, 1.0);
        self.write_float(DATAREF_ROLL, v).await
    }

    /// Write a yaw/rudder axis value in `[-1.0, +1.0]`.
    ///
    /// Positive = right rudder, negative = left rudder.
    pub async fn write_yaw(&self, value: f32) -> Result<(), ControlOutputError> {
        let v = Self::sanitize(value)?.clamp(-1.0, 1.0);
        self.write_float(DATAREF_YAW, v).await
    }

    /// Write a throttle value in `[0.0, 1.0]` for engine `engine_index`.
    ///
    /// 0.0 = idle, 1.0 = full/TOGA. Values are clamped to the valid range.
    pub async fn write_throttle(
        &self,
        engine_index: u8,
        value: f32,
    ) -> Result<(), ControlOutputError> {
        let v = Self::sanitize(value)?.clamp(0.0, 1.0);
        let name = format!("sim/flightmodel/engine/ENGN_thro[{}]", engine_index);
        self.write_float(&name, v).await
    }

    /// Write a flap deployment request in `[0.0, 1.0]`.
    ///
    /// 0.0 = fully retracted, 1.0 = fully extended.
    pub async fn write_flaps(&self, value: f32) -> Result<(), ControlOutputError> {
        let v = Self::sanitize(value)?.clamp(0.0, 1.0);
        self.write_float(DATAREF_FLAPS, v).await
    }

    /// Write a speedbrake/spoiler value in `[0.0, 1.0]`.
    ///
    /// 0.0 = retracted, 1.0 = fully deployed.
    pub async fn write_speedbrake(&self, value: f32) -> Result<(), ControlOutputError> {
        let v = Self::sanitize(value)?.clamp(0.0, 1.0);
        self.write_float(DATAREF_SPEEDBRAKE, v).await
    }

    /// Set the landing-gear handle position (`true` = down/extended).
    pub async fn write_gear(&self, down: bool) -> Result<(), ControlOutputError> {
        self.write_dataref(DATAREF_GEAR_HANDLE, DataRefValue::Int(i32::from(down)))
            .await
    }

    // ── command execution ────────────────────────────────────────────────────

    /// Execute a named X-Plane command (e.g. [`CMD_GEAR_DOWN`]).
    ///
    /// Returns `Ok(())` once the command is dispatched. The web API has no
    /// command endpoint in v1; those calls log a warning and succeed silently.
    pub async fn execute_command(&self, name: &str) -> Result<(), ControlOutputError> {
        debug!(command = name, "executing X-Plane command");
        if let Some(plugin) = &self.plugin {
            if plugin.is_connected() {
                return plugin
                    .execute_command(name)
                    .await
                    .map_err(ControlOutputError::Plugin);
            }
        }
        if self.web_api.is_some() {
            warn!(
                command = name,
                "web API has no command endpoint; command ignored"
            );
            return Ok(());
        }
        Err(ControlOutputError::NoWritePath)
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Reject NaN and infinite values.
    fn sanitize(value: f32) -> Result<f32, ControlOutputError> {
        if value.is_finite() {
            Ok(value)
        } else {
            Err(ControlOutputError::InvalidValue { value })
        }
    }

    async fn write_float(&self, name: &str, value: f32) -> Result<(), ControlOutputError> {
        self.write_dataref(name, DataRefValue::Float(value)).await
    }

    /// Write a DataRefValue via the best available path (plugin first, then web API).
    async fn write_dataref(
        &self,
        name: &str,
        value: DataRefValue,
    ) -> Result<(), ControlOutputError> {
        if let Some(plugin) = &self.plugin {
            if plugin.is_connected() {
                return plugin
                    .set_dataref(name, value)
                    .await
                    .map_err(ControlOutputError::Plugin);
            }
        }
        if let Some(web) = &self.web_api {
            return web
                .set_dataref(name, value)
                .await
                .map_err(ControlOutputError::WebApi);
        }
        Err(ControlOutputError::NoWritePath)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataref_name_constants_are_correct() {
        assert_eq!(DATAREF_PITCH, "sim/joystick/yoke_pitch_ratio");
        assert_eq!(DATAREF_ROLL, "sim/joystick/yoke_roll_ratio");
        assert_eq!(DATAREF_YAW, "sim/joystick/yoke_heading_ratio");
        assert_eq!(
            DATAREF_SPEEDBRAKE,
            "sim/flightmodel/controls/speedbrk_ratio"
        );
        assert_eq!(DATAREF_FLAPS, "sim/flightmodel/controls/flaprqst");
        assert_eq!(
            DATAREF_GEAR_HANDLE,
            "sim/flightmodel/controls/gear_handle_down"
        );
    }

    #[test]
    fn command_name_constants_are_correct() {
        assert_eq!(CMD_GEAR_DOWN, "sim/flight_controls/landing_gear_down");
        assert_eq!(CMD_GEAR_UP, "sim/flight_controls/landing_gear_up");
        assert_eq!(CMD_FLAPS_UP, "sim/flight_controls/flaps_up");
        assert_eq!(CMD_FLAPS_DOWN, "sim/flight_controls/flaps_down");
        assert_eq!(
            CMD_SPEEDBRAKES_TOGGLE,
            "sim/flight_controls/speed_brakes_toggle"
        );
    }

    #[test]
    fn sanitize_rejects_nan_and_inf() {
        assert!(matches!(
            ControlOutput::sanitize(f32::NAN),
            Err(ControlOutputError::InvalidValue { .. })
        ));
        assert!(matches!(
            ControlOutput::sanitize(f32::INFINITY),
            Err(ControlOutputError::InvalidValue { .. })
        ));
        assert!(matches!(
            ControlOutput::sanitize(f32::NEG_INFINITY),
            Err(ControlOutputError::InvalidValue { .. })
        ));
        assert_eq!(ControlOutput::sanitize(0.5).unwrap(), 0.5);
    }

    #[test]
    fn new_without_connections_is_unavailable() {
        let co = ControlOutput::new(None, None);
        assert!(!co.is_available());
    }

    #[tokio::test]
    async fn write_pitch_returns_no_write_path_without_connections() {
        let co = ControlOutput::new(None, None);
        assert!(matches!(
            co.write_pitch(0.5).await,
            Err(ControlOutputError::NoWritePath)
        ));
    }

    #[tokio::test]
    async fn write_roll_returns_no_write_path_without_connections() {
        let co = ControlOutput::new(None, None);
        assert!(matches!(
            co.write_roll(-0.3).await,
            Err(ControlOutputError::NoWritePath)
        ));
    }

    #[tokio::test]
    async fn write_yaw_returns_no_write_path_without_connections() {
        let co = ControlOutput::new(None, None);
        assert!(matches!(
            co.write_yaw(0.0).await,
            Err(ControlOutputError::NoWritePath)
        ));
    }

    #[tokio::test]
    async fn write_throttle_returns_no_write_path_without_connections() {
        let co = ControlOutput::new(None, None);
        assert!(matches!(
            co.write_throttle(0, 1.0).await,
            Err(ControlOutputError::NoWritePath)
        ));
    }

    #[tokio::test]
    async fn write_pitch_rejects_nan_before_checking_write_path() {
        let co = ControlOutput::new(None, None);
        // NaN must error with InvalidValue, not NoWritePath
        assert!(matches!(
            co.write_pitch(f32::NAN).await,
            Err(ControlOutputError::InvalidValue { .. })
        ));
    }

    #[test]
    fn throttle_dataref_name_includes_engine_index() {
        let name0 = format!("sim/flightmodel/engine/ENGN_thro[{}]", 0u8);
        let name3 = format!("sim/flightmodel/engine/ENGN_thro[{}]", 3u8);
        assert_eq!(name0, "sim/flightmodel/engine/ENGN_thro[0]");
        assert_eq!(name3, "sim/flightmodel/engine/ENGN_thro[3]");
    }
}
