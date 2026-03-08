// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Brunner CLS2Sim protocol types and USB device identifiers.
//!
//! The Brunner CLS-E communicates force-feedback commands through the
//! **CLS2Sim** middleware, which exposes a TCP/UDP remote control interface.
//! Commands are sent as ASCII text lines terminated with `\r\n` (CRLF).
//!
//! # USB identifiers
//!
//! - **VID 0x25BB** (Brunner Elektronik AG)
//! - **PID 0x0063** — CLS-E joystick / yoke base
//! - **PID 0x006B** — CLS-P FFB pedals (CLS-E MK II Rudder, PRT.5123)
//!
//! # CLS2Sim command interface
//!
//! The remote interface uses simple text commands over TCP (default port 8090)
//! or UDP. Commands follow the pattern:
//!
//! ```text
//! SET <axis> <parameter> <value>\r\n
//! GET <axis> <parameter>\r\n
//! ```
//!
//! Axis identifiers: `ROLL`, `PITCH`, `YAW`, `COLLECTIVE`, `THROTTLE`.
//! Parameters include force profile, trim position, autopilot coupling, etc.

use serde::{Deserialize, Serialize};

/// USB Vendor ID for Brunner Elektronik AG.
pub const BRUNNER_VENDOR_ID: u16 = 0x25BB;

/// USB Product ID for the CLS-E joystick / yoke base.
pub const CLS_E_PID: u16 = 0x0063;

/// USB Product ID for the CLS-P FFB pedals (CLS-E MK II Rudder, PRT.5123).
pub const CLS_P_PID: u16 = 0x006B;

/// Default CLS2Sim TCP port.
pub const CLS2SIM_DEFAULT_PORT: u16 = 8090;

/// Maximum command length in bytes (protocol limit).
pub const MAX_COMMAND_LEN: usize = 256;

/// Brunner device model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceModel {
    /// CLS-E joystick / yoke base (2-axis FFB).
    ClsEJoystick,
    /// CLS-P FFB pedals (1-axis FFB: yaw).
    ClsPPedals,
}

impl DeviceModel {
    /// USB Product ID for this device model.
    pub fn pid(self) -> u16 {
        match self {
            Self::ClsEJoystick => CLS_E_PID,
            Self::ClsPPedals => CLS_P_PID,
        }
    }

    /// Number of force-feedback axes on this device.
    pub fn ffb_axis_count(self) -> u8 {
        match self {
            Self::ClsEJoystick => 2, // roll + pitch
            Self::ClsPPedals => 1,   // yaw only
        }
    }

    /// Human-readable name.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::ClsEJoystick => "Brunner CLS-E Joystick",
            Self::ClsPPedals => "Brunner CLS-P Pedals",
        }
    }
}

impl std::fmt::Display for DeviceModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

/// Force-feedback axis identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ForceAxis {
    Roll,
    Pitch,
    Yaw,
}

impl ForceAxis {
    /// CLS2Sim protocol axis name.
    pub fn protocol_name(self) -> &'static str {
        match self {
            Self::Roll => "ROLL",
            Self::Pitch => "PITCH",
            Self::Yaw => "YAW",
        }
    }
}

impl std::fmt::Display for ForceAxis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.protocol_name())
    }
}

/// Device capability descriptor returned during discovery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    /// Device model.
    pub model: DeviceModel,
    /// Firmware version string (e.g. "2.4.1").
    pub firmware_version: String,
    /// Maximum continuous force in Newtons.
    pub max_continuous_force_n: f32,
    /// Peak force in Newtons (short bursts only).
    pub peak_force_n: f32,
    /// Number of FFB axes.
    pub ffb_axes: u8,
    /// Supported force effect types.
    pub supported_effects: Vec<String>,
}

impl DeviceCapabilities {
    /// Create capabilities for a standard CLS-E joystick.
    pub fn cls_e_default() -> Self {
        Self {
            model: DeviceModel::ClsEJoystick,
            firmware_version: String::from("2.4.0"),
            max_continuous_force_n: 60.0,
            peak_force_n: 90.0,
            ffb_axes: 2,
            supported_effects: vec![
                "spring".into(),
                "damper".into(),
                "friction".into(),
                "constant".into(),
                "periodic".into(),
                "trim".into(),
            ],
        }
    }

    /// Create capabilities for a standard CLS-P pedals.
    pub fn cls_p_default() -> Self {
        Self {
            model: DeviceModel::ClsPPedals,
            firmware_version: String::from("2.4.0"),
            max_continuous_force_n: 40.0,
            peak_force_n: 60.0,
            ffb_axes: 1,
            supported_effects: vec![
                "spring".into(),
                "damper".into(),
                "friction".into(),
                "constant".into(),
            ],
        }
    }
}

/// CLS2Sim remote interface command.
///
/// Commands are serialised as ASCII text for transmission over TCP/UDP.
#[derive(Debug, Clone, PartialEq)]
pub enum Cls2SimCommand {
    /// Set force profile spring coefficient for an axis.
    SetSpring { axis: ForceAxis, coefficient: f32 },

    /// Set force profile damper coefficient for an axis.
    SetDamper { axis: ForceAxis, coefficient: f32 },

    /// Set constant (directional) force on an axis.
    SetConstantForce { axis: ForceAxis, magnitude: f32 },

    /// Set trim position for an axis (normalised -1.0..1.0).
    SetTrim { axis: ForceAxis, position: f32 },

    /// Enable or disable autopilot coupling on an axis.
    SetAutopilot { axis: ForceAxis, engaged: bool },

    /// Set periodic vibration effect (e.g. engine vibration, turbulence).
    SetVibration {
        axis: ForceAxis,
        frequency_hz: f32,
        amplitude: f32,
    },

    /// Emergency stop — immediately zero all forces.
    EmergencyStop,

    /// Query device capabilities.
    GetCapabilities,

    /// Query current device status.
    GetStatus,
}

impl Cls2SimCommand {
    /// Serialise the command to the CLS2Sim ASCII wire format.
    ///
    /// Returns `None` if the resulting command would exceed [`MAX_COMMAND_LEN`].
    pub fn to_wire(&self) -> Option<String> {
        let s = match self {
            Self::SetSpring { axis, coefficient } => {
                let c = coefficient.clamp(0.0, 1.0);
                format!("SET {} SPRING {:.4}\r\n", axis.protocol_name(), c)
            }
            Self::SetDamper { axis, coefficient } => {
                let c = coefficient.clamp(0.0, 1.0);
                format!("SET {} DAMPER {:.4}\r\n", axis.protocol_name(), c)
            }
            Self::SetConstantForce { axis, magnitude } => {
                let m = magnitude.clamp(-1.0, 1.0);
                format!("SET {} FORCE {:.4}\r\n", axis.protocol_name(), m)
            }
            Self::SetTrim { axis, position } => {
                let p = position.clamp(-1.0, 1.0);
                format!("SET {} TRIM {:.4}\r\n", axis.protocol_name(), p)
            }
            Self::SetAutopilot { axis, engaged } => {
                let v = if *engaged { "ON" } else { "OFF" };
                format!("SET {} AUTOPILOT {}\r\n", axis.protocol_name(), v)
            }
            Self::SetVibration {
                axis,
                frequency_hz,
                amplitude,
            } => {
                let f = frequency_hz.clamp(0.0, 200.0);
                let a = amplitude.clamp(0.0, 1.0);
                format!(
                    "SET {} VIBRATION {:.1} {:.4}\r\n",
                    axis.protocol_name(),
                    f,
                    a
                )
            }
            Self::EmergencyStop => "ESTOP\r\n".to_string(),
            Self::GetCapabilities => "GET CAPABILITIES\r\n".to_string(),
            Self::GetStatus => "GET STATUS\r\n".to_string(),
        };
        if s.len() > MAX_COMMAND_LEN {
            None
        } else {
            Some(s)
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_model_pid_roundtrip() {
        assert_eq!(DeviceModel::ClsEJoystick.pid(), CLS_E_PID);
        assert_eq!(DeviceModel::ClsPPedals.pid(), CLS_P_PID);
    }

    #[test]
    fn device_model_ffb_axes() {
        assert_eq!(DeviceModel::ClsEJoystick.ffb_axis_count(), 2);
        assert_eq!(DeviceModel::ClsPPedals.ffb_axis_count(), 1);
    }

    #[test]
    fn device_model_display() {
        assert_eq!(
            DeviceModel::ClsEJoystick.to_string(),
            "Brunner CLS-E Joystick"
        );
        assert_eq!(DeviceModel::ClsPPedals.to_string(), "Brunner CLS-P Pedals");
    }

    #[test]
    fn force_axis_protocol_names() {
        assert_eq!(ForceAxis::Roll.protocol_name(), "ROLL");
        assert_eq!(ForceAxis::Pitch.protocol_name(), "PITCH");
        assert_eq!(ForceAxis::Yaw.protocol_name(), "YAW");
    }

    #[test]
    fn force_axis_display() {
        assert_eq!(ForceAxis::Roll.to_string(), "ROLL");
    }

    #[test]
    fn cls_e_default_capabilities() {
        let caps = DeviceCapabilities::cls_e_default();
        assert_eq!(caps.model, DeviceModel::ClsEJoystick);
        assert_eq!(caps.ffb_axes, 2);
        assert!(caps.max_continuous_force_n > 0.0);
        assert!(caps.peak_force_n > caps.max_continuous_force_n);
        assert!(caps.supported_effects.contains(&"spring".to_string()));
        assert!(caps.supported_effects.contains(&"damper".to_string()));
    }

    #[test]
    fn cls_p_default_capabilities() {
        let caps = DeviceCapabilities::cls_p_default();
        assert_eq!(caps.model, DeviceModel::ClsPPedals);
        assert_eq!(caps.ffb_axes, 1);
    }

    // ── Wire format tests ─────────────────────────────────────────────────────

    #[test]
    fn wire_set_spring() {
        let cmd = Cls2SimCommand::SetSpring {
            axis: ForceAxis::Roll,
            coefficient: 0.75,
        };
        let wire = cmd.to_wire().unwrap();
        assert_eq!(wire, "SET ROLL SPRING 0.7500\r\n");
    }

    #[test]
    fn wire_set_damper() {
        let cmd = Cls2SimCommand::SetDamper {
            axis: ForceAxis::Pitch,
            coefficient: 0.5,
        };
        let wire = cmd.to_wire().unwrap();
        assert_eq!(wire, "SET PITCH DAMPER 0.5000\r\n");
    }

    #[test]
    fn wire_set_constant_force() {
        let cmd = Cls2SimCommand::SetConstantForce {
            axis: ForceAxis::Yaw,
            magnitude: -0.3,
        };
        let wire = cmd.to_wire().unwrap();
        assert_eq!(wire, "SET YAW FORCE -0.3000\r\n");
    }

    #[test]
    fn wire_set_trim() {
        let cmd = Cls2SimCommand::SetTrim {
            axis: ForceAxis::Pitch,
            position: 0.1,
        };
        let wire = cmd.to_wire().unwrap();
        assert_eq!(wire, "SET PITCH TRIM 0.1000\r\n");
    }

    #[test]
    fn wire_set_autopilot_on() {
        let cmd = Cls2SimCommand::SetAutopilot {
            axis: ForceAxis::Roll,
            engaged: true,
        };
        let wire = cmd.to_wire().unwrap();
        assert_eq!(wire, "SET ROLL AUTOPILOT ON\r\n");
    }

    #[test]
    fn wire_set_autopilot_off() {
        let cmd = Cls2SimCommand::SetAutopilot {
            axis: ForceAxis::Pitch,
            engaged: false,
        };
        let wire = cmd.to_wire().unwrap();
        assert_eq!(wire, "SET PITCH AUTOPILOT OFF\r\n");
    }

    #[test]
    fn wire_set_vibration() {
        let cmd = Cls2SimCommand::SetVibration {
            axis: ForceAxis::Roll,
            frequency_hz: 25.0,
            amplitude: 0.4,
        };
        let wire = cmd.to_wire().unwrap();
        assert_eq!(wire, "SET ROLL VIBRATION 25.0 0.4000\r\n");
    }

    #[test]
    fn wire_emergency_stop() {
        let wire = Cls2SimCommand::EmergencyStop.to_wire().unwrap();
        assert_eq!(wire, "ESTOP\r\n");
    }

    #[test]
    fn wire_get_capabilities() {
        let wire = Cls2SimCommand::GetCapabilities.to_wire().unwrap();
        assert_eq!(wire, "GET CAPABILITIES\r\n");
    }

    #[test]
    fn wire_get_status() {
        let wire = Cls2SimCommand::GetStatus.to_wire().unwrap();
        assert_eq!(wire, "GET STATUS\r\n");
    }

    #[test]
    fn wire_clamps_spring_coefficient() {
        let cmd = Cls2SimCommand::SetSpring {
            axis: ForceAxis::Roll,
            coefficient: 2.0,
        };
        let wire = cmd.to_wire().unwrap();
        assert!(wire.contains("1.0000"));
    }

    #[test]
    fn wire_clamps_negative_spring_coefficient() {
        let cmd = Cls2SimCommand::SetSpring {
            axis: ForceAxis::Roll,
            coefficient: -0.5,
        };
        let wire = cmd.to_wire().unwrap();
        assert!(wire.contains("0.0000"));
    }

    #[test]
    fn wire_clamps_force_magnitude() {
        let cmd = Cls2SimCommand::SetConstantForce {
            axis: ForceAxis::Roll,
            magnitude: 5.0,
        };
        let wire = cmd.to_wire().unwrap();
        assert!(wire.contains("1.0000"));
    }

    #[test]
    fn wire_clamps_vibration_frequency() {
        let cmd = Cls2SimCommand::SetVibration {
            axis: ForceAxis::Roll,
            frequency_hz: 500.0,
            amplitude: 0.5,
        };
        let wire = cmd.to_wire().unwrap();
        assert!(wire.contains("200.0"));
    }

    #[test]
    fn wire_clamps_trim_position() {
        let cmd = Cls2SimCommand::SetTrim {
            axis: ForceAxis::Pitch,
            position: -2.0,
        };
        let wire = cmd.to_wire().unwrap();
        assert!(wire.contains("-1.0000"));
    }
}
