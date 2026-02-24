// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! VR Overlay configuration.

use serde::{Deserialize, Serialize};

/// Position anchor for the overlay panel relative to the cockpit origin.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorPoint {
    /// Centred in front of the pilot at eye height.
    CentreFront,
    /// Lower-left quadrant (ideal for status readouts).
    LowerLeft,
    /// Lower-right quadrant.
    LowerRight,
    /// Upper-left quadrant.
    UpperLeft,
    /// Upper-right quadrant.
    UpperRight,
}

impl Default for AnchorPoint {
    fn default() -> Self {
        Self::LowerLeft
    }
}

/// Configuration for the VR overlay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayConfig {
    /// Whether the overlay is visible by default on startup.
    pub enabled: bool,
    /// Opacity of the overlay panel (0.0 = transparent, 1.0 = fully opaque).
    pub opacity: f32,
    /// Scale factor relative to the default panel size.
    pub scale: f32,
    /// Position anchor in cockpit space.
    pub anchor: AnchorPoint,
    /// Distance from the cockpit origin in metres (depth into the scene).
    pub depth_m: f32,
    /// Horizontal offset from the anchor in metres.
    pub offset_x_m: f32,
    /// Vertical offset from the anchor in metres.
    pub offset_y_m: f32,
    /// Maximum number of notifications displayed simultaneously.
    pub max_notifications: usize,
    /// Default time-to-live for notifications in seconds.
    pub notification_ttl_secs: u64,
    /// Show live axis values in the overlay.
    pub show_axis_status: bool,
    /// Show current profile name in the overlay.
    pub show_profile_name: bool,
    /// Show FFB status (mode, trim level) in the overlay.
    pub show_ffb_status: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            opacity: 0.85,
            scale: 1.0,
            anchor: AnchorPoint::default(),
            depth_m: 2.0,
            offset_x_m: 0.0,
            offset_y_m: 0.0,
            max_notifications: 5,
            notification_ttl_secs: 6,
            show_axis_status: true,
            show_profile_name: true,
            show_ffb_status: false,
        }
    }
}

impl OverlayConfig {
    /// Return a minimal configuration suitable for testing or headless operation.
    pub fn minimal() -> Self {
        Self {
            enabled: false,
            opacity: 1.0,
            scale: 1.0,
            anchor: AnchorPoint::CentreFront,
            depth_m: 2.0,
            offset_x_m: 0.0,
            offset_y_m: 0.0,
            max_notifications: 3,
            notification_ttl_secs: 2,
            show_axis_status: false,
            show_profile_name: true,
            show_ffb_status: false,
        }
    }

    /// Validate the configuration, returning an error string if invalid.
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.opacity) {
            return Err(format!(
                "opacity must be in [0.0, 1.0], got {}",
                self.opacity
            ));
        }
        if self.scale <= 0.0 {
            return Err(format!("scale must be > 0.0, got {}", self.scale));
        }
        if self.depth_m < 0.1 {
            return Err(format!("depth_m must be >= 0.1 m, got {}", self.depth_m));
        }
        if self.max_notifications == 0 {
            return Err("max_notifications must be >= 1".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validates() {
        assert!(OverlayConfig::default().validate().is_ok());
    }

    #[test]
    fn test_minimal_config_validates() {
        assert!(OverlayConfig::minimal().validate().is_ok());
    }

    #[test]
    fn test_opacity_out_of_range_rejected() {
        let mut cfg = OverlayConfig::default();
        cfg.opacity = 1.5;
        assert!(cfg.validate().is_err());
        cfg.opacity = -0.1;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_zero_scale_rejected() {
        let mut cfg = OverlayConfig::default();
        cfg.scale = 0.0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_depth_too_small_rejected() {
        let mut cfg = OverlayConfig::default();
        cfg.depth_m = 0.05;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_zero_max_notifications_rejected() {
        let mut cfg = OverlayConfig::default();
        cfg.max_notifications = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_anchor_point_round_trip() {
        let json = serde_json::to_string(&AnchorPoint::LowerLeft).unwrap();
        assert_eq!(json, r#""lower_left""#);
        let back: AnchorPoint = serde_json::from_str(&json).unwrap();
        assert_eq!(back, AnchorPoint::LowerLeft);
    }

    #[test]
    fn test_config_serde_round_trip() {
        let cfg = OverlayConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: OverlayConfig = serde_json::from_str(&json).unwrap();
        assert!((back.opacity - cfg.opacity).abs() < 1e-6);
        assert_eq!(back.max_notifications, cfg.max_notifications);
        assert_eq!(back.anchor, cfg.anchor);
    }
}
