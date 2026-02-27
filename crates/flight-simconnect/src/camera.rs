// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! MSFS camera axis injection (REQ-707)
//!
//! Provides camera control via SimConnect SimVars, allowing pan, tilt, and
//! zoom axes to be written back to the simulator. Values are clamped to the
//! valid `[-1.0, 1.0]` range for pan/tilt and `[0.0, 1.0]` for zoom.

/// Clamp a value to the range `[min, max]`.
fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Camera channel axis values.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraChannel {
    /// Horizontal pan in `[-1.0, 1.0]` (left to right).
    pub pan: f64,
    /// Vertical tilt in `[-1.0, 1.0]` (down to up).
    pub tilt: f64,
    /// Zoom level in `[0.0, 1.0]` (wide to narrow).
    pub zoom: f64,
}

impl Default for CameraChannel {
    fn default() -> Self {
        Self {
            pan: 0.0,
            tilt: 0.0,
            zoom: 0.0,
        }
    }
}

impl CameraChannel {
    /// Create a new camera channel with clamped values.
    pub fn new(pan: f64, tilt: f64, zoom: f64) -> Self {
        Self {
            pan: clamp(pan, -1.0, 1.0),
            tilt: clamp(tilt, -1.0, 1.0),
            zoom: clamp(zoom, 0.0, 1.0),
        }
    }

    /// Returns a clamped copy of this channel.
    pub fn clamped(&self) -> Self {
        Self::new(self.pan, self.tilt, self.zoom)
    }
}

/// Configuration for camera injection.
#[derive(Debug, Clone, Default)]
pub struct CameraConfig {
    /// Whether camera injection is enabled.
    pub enabled: bool,
    /// SimConnect channel identifier for camera events.
    pub channel_id: u32,
}

/// Format camera channel values into SimConnect SimVar name/value pairs.
///
/// Returns a list of `(simvar_name, value)` tuples that can be written to
/// the simulator. Values are clamped before formatting.
pub fn format_camera_simvar(channel: &CameraChannel) -> Vec<(String, f64)> {
    let clamped = channel.clamped();
    vec![
        ("CAMERA PAN".to_string(), clamped.pan),
        ("CAMERA TILT".to_string(), clamped.tilt),
        ("CAMERA ZOOM".to_string(), clamped.zoom),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_produces_valid_simvar_names() {
        let channel = CameraChannel::new(0.5, -0.3, 0.8);
        let vars = format_camera_simvar(&channel);
        assert_eq!(vars.len(), 3);
        assert_eq!(vars[0].0, "CAMERA PAN");
        assert_eq!(vars[1].0, "CAMERA TILT");
        assert_eq!(vars[2].0, "CAMERA ZOOM");
    }

    #[test]
    fn test_format_values_match_channel() {
        let channel = CameraChannel::new(0.5, -0.3, 0.8);
        let vars = format_camera_simvar(&channel);
        assert_eq!(vars[0].1, 0.5);
        assert_eq!(vars[1].1, -0.3);
        assert_eq!(vars[2].1, 0.8);
    }

    #[test]
    fn test_toggle_enabled() {
        let mut config = CameraConfig::default();
        assert!(!config.enabled);
        config.enabled = true;
        assert!(config.enabled);
    }

    #[test]
    fn test_channel_values_clamped_pan_tilt() {
        let channel = CameraChannel::new(2.0, -5.0, 0.5);
        assert_eq!(channel.pan, 1.0);
        assert_eq!(channel.tilt, -1.0);
    }

    #[test]
    fn test_channel_zoom_clamped() {
        let channel = CameraChannel::new(0.0, 0.0, 1.5);
        assert_eq!(channel.zoom, 1.0);

        let channel2 = CameraChannel::new(0.0, 0.0, -0.5);
        assert_eq!(channel2.zoom, 0.0);
    }

    #[test]
    fn test_format_clamps_before_output() {
        let channel = CameraChannel {
            pan: 999.0,
            tilt: -999.0,
            zoom: 999.0,
        };
        let vars = format_camera_simvar(&channel);
        assert_eq!(vars[0].1, 1.0);
        assert_eq!(vars[1].1, -1.0);
        assert_eq!(vars[2].1, 1.0);
    }

    #[test]
    fn test_default_channel() {
        let ch = CameraChannel::default();
        assert_eq!(ch.pan, 0.0);
        assert_eq!(ch.tilt, 0.0);
        assert_eq!(ch.zoom, 0.0);
    }

    #[test]
    fn test_default_config() {
        let config = CameraConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.channel_id, 0);
    }

    #[test]
    fn test_custom_channel_id() {
        let config = CameraConfig {
            enabled: true,
            channel_id: 42,
        };
        assert!(config.enabled);
        assert_eq!(config.channel_id, 42);
    }
}
