// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Overlay display state — snapshot of what the VR panel currently shows.

use serde::{Deserialize, Serialize};

/// Normalised axis value snapshot for display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AxisStatus {
    /// Logical axis name (e.g. "Roll", "Pitch", "Throttle").
    pub name: String,
    /// Raw normalised value in \[-1.0, 1.0\].
    pub raw: f32,
    /// Post-curve, post-deadzone value in \[-1.0, 1.0\].
    pub processed: f32,
    /// Whether this axis is currently in its deadzone.
    pub in_deadzone: bool,
}

impl AxisStatus {
    /// Construct a new axis status entry.
    pub fn new(name: impl Into<String>, raw: f32, processed: f32, in_deadzone: bool) -> Self {
        Self {
            name: name.into(),
            raw,
            processed,
            in_deadzone,
        }
    }
}

/// Simulator connection state shown in the overlay header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimConnectionStatus {
    /// No simulator detected.
    #[default]
    Disconnected,
    /// Connection handshake in progress.
    Connecting,
    /// Simulator connected and streaming telemetry.
    Connected,
    /// Simulator paused (no telemetry updates).
    Paused,
}

impl std::fmt::Display for SimConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => f.write_str("Disconnected"),
            Self::Connecting => f.write_str("Connecting…"),
            Self::Connected => f.write_str("Connected"),
            Self::Paused => f.write_str("Paused"),
        }
    }
}

/// FFB status for the overlay footer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfbStatus {
    /// Whether FFB is currently active.
    pub active: bool,
    /// Current FFB mode label (e.g. "Turbulence", "Stall", "Trim").
    pub mode: String,
    /// Trim position in \[-1.0, 1.0\].
    pub trim_position: f32,
    /// Whether the FFB safety interlock is engaged.
    pub safety_engaged: bool,
}

impl Default for FfbStatus {
    fn default() -> Self {
        Self {
            active: false,
            mode: "Idle".to_string(),
            trim_position: 0.0,
            safety_engaged: false,
        }
    }
}

/// Complete state snapshot for the VR overlay panel.
///
/// This struct is cheaply cloneable and sent to the renderer on every frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayState {
    /// Currently active profile name.
    pub profile_name: String,
    /// Simulator currently connected (e.g. "MSFS 2024", "X-Plane 12", "DCS").
    pub sim_name: Option<String>,
    /// Simulator connection status.
    pub sim_status: SimConnectionStatus,
    /// Live axis values (populated if `show_axis_status = true`).
    pub axes: Vec<AxisStatus>,
    /// FFB status (populated if `show_ffb_status = true`).
    pub ffb: FfbStatus,
    /// Whether the overlay is currently visible to the user.
    pub visible: bool,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            profile_name: "Default".to_string(),
            sim_name: None,
            sim_status: SimConnectionStatus::Disconnected,
            axes: Vec::new(),
            ffb: FfbStatus::default(),
            visible: true,
        }
    }
}

impl OverlayState {
    /// Create a state with the given profile name.
    pub fn with_profile(profile_name: impl Into<String>) -> Self {
        Self {
            profile_name: profile_name.into(),
            ..Self::default()
        }
    }

    /// Update the simulator connection information.
    pub fn set_sim(&mut self, name: impl Into<String>, status: SimConnectionStatus) {
        self.sim_name = Some(name.into());
        self.sim_status = status;
    }

    /// Replace the axis status list.
    pub fn set_axes(&mut self, axes: Vec<AxisStatus>) {
        self.axes = axes;
    }

    /// Update the FFB status.
    pub fn set_ffb(&mut self, ffb: FfbStatus) {
        self.ffb = ffb;
    }

    /// Toggle visibility.
    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    /// Returns the number of axes in deadzone.
    pub fn axes_in_deadzone(&self) -> usize {
        self.axes.iter().filter(|a| a.in_deadzone).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let s = OverlayState::default();
        assert_eq!(s.profile_name, "Default");
        assert_eq!(s.sim_status, SimConnectionStatus::Disconnected);
        assert!(s.visible);
    }

    #[test]
    fn test_with_profile() {
        let s = OverlayState::with_profile("MSFS-A320");
        assert_eq!(s.profile_name, "MSFS-A320");
    }

    #[test]
    fn test_set_sim() {
        let mut s = OverlayState::default();
        s.set_sim("X-Plane 12", SimConnectionStatus::Connected);
        assert_eq!(s.sim_name.as_deref(), Some("X-Plane 12"));
        assert_eq!(s.sim_status, SimConnectionStatus::Connected);
    }

    #[test]
    fn test_toggle_visible() {
        let mut s = OverlayState::default();
        assert!(s.visible);
        s.toggle_visible();
        assert!(!s.visible);
        s.toggle_visible();
        assert!(s.visible);
    }

    #[test]
    fn test_axes_in_deadzone_count() {
        let mut s = OverlayState::default();
        s.set_axes(vec![
            AxisStatus::new("Roll", 0.01, 0.0, true),
            AxisStatus::new("Pitch", 0.5, 0.5, false),
            AxisStatus::new("Throttle", 0.0, 0.0, true),
        ]);
        assert_eq!(s.axes_in_deadzone(), 2);
    }

    #[test]
    fn test_sim_connection_display() {
        assert_eq!(
            SimConnectionStatus::Disconnected.to_string(),
            "Disconnected"
        );
        assert_eq!(SimConnectionStatus::Connected.to_string(), "Connected");
        assert_eq!(SimConnectionStatus::Paused.to_string(), "Paused");
    }

    #[test]
    fn test_ffb_status_default() {
        let ffb = FfbStatus::default();
        assert!(!ffb.active);
        assert_eq!(ffb.mode, "Idle");
        assert!(!ffb.safety_engaged);
    }

    #[test]
    fn test_state_serde_round_trip() {
        let mut s = OverlayState::with_profile("DCS-F16");
        s.set_sim("DCS World", SimConnectionStatus::Connected);
        let json = serde_json::to_string(&s).unwrap();
        let back: OverlayState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.profile_name, s.profile_name);
        assert_eq!(back.sim_status, SimConnectionStatus::Connected);
    }
}
