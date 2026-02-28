// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Per-device axis and button profile descriptors for all supported
//! Thrustmaster flight peripherals.
//!
//! Each profile describes the full input surface (axes, buttons, hats) of a
//! device, together with recommended normalisation parameters. Profiles are
//! used by the axis engine and UI layer to automatically configure deadzones,
//! curve shapes, and label overlays.
//!
//! # Example
//!
//! ```
//! use flight_hotas_thrustmaster::profiles::{device_profile, DeviceProfile};
//! use flight_hotas_thrustmaster::protocol::ThrustmasterDevice;
//!
//! let profile = device_profile(ThrustmasterDevice::WarthogJoystick).unwrap();
//! assert_eq!(profile.name, "HOTAS Warthog Joystick");
//! assert_eq!(profile.axes.len(), 2); // X, Y (no twist on the Warthog stick)
//! assert_eq!(profile.button_count, 19);
//! ```

use crate::protocol::ThrustmasterDevice;

// ─── Axis descriptor ────────────────────────────────────────────────────────

/// How a raw HID axis value should be normalised.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxisNormalization {
    /// Centered bipolar: raw center maps to 0.0, extremes to ±1.0.
    Bipolar {
        /// Raw value at the center of the axis (e.g. 8191.5 for 14-bit).
        center: f32,
        /// Half-span (center-to-extreme distance).
        half_span: f32,
    },
    /// Unipolar: 0 → 0.0, max → 1.0.
    Unipolar {
        /// Maximum raw value (e.g. 65535 for u16).
        max: f32,
    },
}

/// Descriptor for a single analog axis.
#[derive(Debug, Clone)]
pub struct AxisDescriptor {
    /// Short identifier (e.g. `"x"`, `"throttle_left"`, `"rudder"`).
    pub id: &'static str,
    /// Human-readable label for UI display.
    pub label: &'static str,
    /// How the raw HID value should be normalised.
    pub normalization: AxisNormalization,
    /// Recommended deadzone (0.0–1.0).
    pub deadzone: f32,
    /// Optional EMA filter alpha (None = no filtering).
    pub filter_alpha: Option<f32>,
}

// ─── Device profile ─────────────────────────────────────────────────────────

/// Complete input profile for a Thrustmaster device.
#[derive(Debug, Clone)]
pub struct DeviceProfile {
    /// Enumerated device variant.
    pub device: ThrustmasterDevice,
    /// Human-readable device name.
    pub name: &'static str,
    /// All analog axes.
    pub axes: Vec<AxisDescriptor>,
    /// Total button count (1-indexed maximum).
    pub button_count: u8,
    /// Number of hat switches.
    pub hat_count: u8,
    /// Whether the device has LED control.
    pub has_leds: bool,
    /// Notes about the device (quirks, calibration hints).
    pub notes: &'static str,
}

// ─── Normalisation presets ──────────────────────────────────────────────────

/// 14-bit centred bipolar (T.16000M X/Y/Twist).
const NORM_14BIT_BIPOLAR: AxisNormalization = AxisNormalization::Bipolar {
    center: 8191.5,
    half_span: 8191.5,
};

/// 16-bit centred bipolar (Warthog stick X/Y/Rz, TWCS mini-stick, etc.).
const NORM_16BIT_BIPOLAR: AxisNormalization = AxisNormalization::Bipolar {
    center: 32767.5,
    half_span: 32767.5,
};

/// 16-bit unipolar (throttle levers, sliders, pedal axes).
const NORM_16BIT_UNIPOLAR: AxisNormalization = AxisNormalization::Unipolar { max: 65535.0 };

// ─── Profile builders ───────────────────────────────────────────────────────

fn t16000m_profile() -> DeviceProfile {
    DeviceProfile {
        device: ThrustmasterDevice::T16000mJoystick,
        name: "T.16000M FCS Joystick",
        axes: vec![
            AxisDescriptor {
                id: "x",
                label: "Stick X (Roll)",
                normalization: NORM_14BIT_BIPOLAR,
                deadzone: 0.05,
                filter_alpha: Some(0.15),
            },
            AxisDescriptor {
                id: "y",
                label: "Stick Y (Pitch)",
                normalization: NORM_14BIT_BIPOLAR,
                deadzone: 0.05,
                filter_alpha: Some(0.15),
            },
            AxisDescriptor {
                id: "twist",
                label: "Twist (Rudder)",
                normalization: NORM_14BIT_BIPOLAR,
                deadzone: 0.08,
                filter_alpha: Some(0.15),
            },
            AxisDescriptor {
                id: "slider",
                label: "Throttle Slider",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.02,
                filter_alpha: Some(0.10),
            },
        ],
        button_count: 16,
        hat_count: 1,
        has_leds: false,
        notes: "14-bit HALL sensor axes (X/Y/Twist), 16-bit slider. Hat uses standard 8-way encoding.",
    }
}

fn twcs_profile() -> DeviceProfile {
    DeviceProfile {
        device: ThrustmasterDevice::TwcsThrottle,
        name: "TWCS Throttle",
        axes: vec![
            AxisDescriptor {
                id: "throttle",
                label: "Main Throttle",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.02,
                filter_alpha: Some(0.10),
            },
            AxisDescriptor {
                id: "mini_stick_x",
                label: "Mini-stick X",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.10,
                filter_alpha: Some(0.20),
            },
            AxisDescriptor {
                id: "mini_stick_y",
                label: "Mini-stick Y",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.10,
                filter_alpha: Some(0.20),
            },
            AxisDescriptor {
                id: "rocker",
                label: "Rocker (Rudder)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.05,
                filter_alpha: Some(0.15),
            },
        ],
        button_count: 14,
        hat_count: 0,
        has_leds: false,
        notes: "Mini-stick has significant play; larger deadzone recommended. Rocker is often used for rudder.",
    }
}

fn warthog_stick_profile() -> DeviceProfile {
    DeviceProfile {
        device: ThrustmasterDevice::WarthogJoystick,
        name: "HOTAS Warthog Joystick",
        axes: vec![
            AxisDescriptor {
                id: "x",
                label: "Stick X (Roll)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.03,
                filter_alpha: None,
            },
            AxisDescriptor {
                id: "y",
                label: "Stick Y (Pitch)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.03,
                filter_alpha: None,
            },
        ],
        button_count: 19,
        hat_count: 1,
        has_leds: false,
        notes: "Metal gimbal, no twist axis. Pinkie (btn 2) acts as shift key in TARGET. Hat is 4-way + center.",
    }
}

fn warthog_throttle_profile() -> DeviceProfile {
    DeviceProfile {
        device: ThrustmasterDevice::WarthogThrottle,
        name: "HOTAS Warthog Throttle",
        axes: vec![
            AxisDescriptor {
                id: "throttle_left",
                label: "Left Throttle",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.01,
                filter_alpha: Some(0.08),
            },
            AxisDescriptor {
                id: "throttle_right",
                label: "Right Throttle",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.01,
                filter_alpha: Some(0.08),
            },
            AxisDescriptor {
                id: "throttle_combined",
                label: "Combined Throttle",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.01,
                filter_alpha: Some(0.08),
            },
            AxisDescriptor {
                id: "slew_x",
                label: "Slew Control X",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.15,
                filter_alpha: Some(0.25),
            },
            AxisDescriptor {
                id: "slew_y",
                label: "Slew Control Y",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.15,
                filter_alpha: Some(0.25),
            },
        ],
        button_count: 40,
        hat_count: 2,
        has_leds: true,
        notes: "Dual throttle with interlock (split/merge). Slew mini-stick has large dead zone. 2 hats (DMS, CSL). Backlight LED controllable.",
    }
}

fn tfrp_profile() -> DeviceProfile {
    DeviceProfile {
        device: ThrustmasterDevice::TfrpRudderPedals,
        name: "T.Flight Rudder Pedals (TFRP)",
        axes: vec![
            AxisDescriptor {
                id: "rudder",
                label: "Rudder (Combined)",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.03,
                filter_alpha: Some(0.10),
            },
            AxisDescriptor {
                id: "right_pedal",
                label: "Right Toe Brake",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.03,
                filter_alpha: Some(0.10),
            },
            AxisDescriptor {
                id: "left_pedal",
                label: "Left Toe Brake",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.03,
                filter_alpha: Some(0.10),
            },
        ],
        button_count: 0,
        hat_count: 0,
        has_leds: false,
        notes: "Rudder is unipolar 0.0-1.0 (center ~0.5). Apply center-subtract in profile for bipolar -1..1.",
    }
}

fn tpr_profile() -> DeviceProfile {
    DeviceProfile {
        device: ThrustmasterDevice::TprPendular,
        name: "T-Pendular Rudder (TPR)",
        axes: vec![
            AxisDescriptor {
                id: "rudder",
                label: "Rudder (Combined)",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.02,
                filter_alpha: None,
            },
            AxisDescriptor {
                id: "right_pedal",
                label: "Right Toe Brake",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.02,
                filter_alpha: None,
            },
            AxisDescriptor {
                id: "left_pedal",
                label: "Left Toe Brake",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.02,
                filter_alpha: None,
            },
        ],
        button_count: 0,
        hat_count: 0,
        has_leds: false,
        notes: "Pendular design with longer travel; higher resolution than TFRP. Same HID layout. No filtering needed.",
    }
}

fn cougar_profile() -> DeviceProfile {
    DeviceProfile {
        device: ThrustmasterDevice::HotasCougar,
        name: "HOTAS Cougar",
        axes: vec![
            AxisDescriptor {
                id: "x",
                label: "Stick X (Roll)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.05,
                filter_alpha: Some(0.12),
            },
            AxisDescriptor {
                id: "y",
                label: "Stick Y (Pitch)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.05,
                filter_alpha: Some(0.12),
            },
            AxisDescriptor {
                id: "throttle",
                label: "Throttle",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.02,
                filter_alpha: Some(0.10),
            },
        ],
        button_count: 16,
        hat_count: 1,
        has_leds: false,
        notes: "Legacy F-16 replica. Combined stick+throttle on single USB endpoint. Potentiometer axes may need larger deadzone.",
    }
}

// ─── Profile lookup ─────────────────────────────────────────────────────────

/// Retrieve the input profile for a given Thrustmaster device.
///
/// Returns `None` for device variants that do not yet have a dedicated profile
/// (e.g. TCA Boeing, T.Flight HOTAS X).
pub fn device_profile(device: ThrustmasterDevice) -> Option<DeviceProfile> {
    match device {
        ThrustmasterDevice::T16000mJoystick => Some(t16000m_profile()),
        ThrustmasterDevice::TwcsThrottle => Some(twcs_profile()),
        ThrustmasterDevice::WarthogJoystick => Some(warthog_stick_profile()),
        ThrustmasterDevice::WarthogThrottle => Some(warthog_throttle_profile()),
        ThrustmasterDevice::TfrpRudderPedals | ThrustmasterDevice::TRudder => {
            Some(tfrp_profile())
        }
        ThrustmasterDevice::TprPendular | ThrustmasterDevice::TprPendularBulk => {
            Some(tpr_profile())
        }
        ThrustmasterDevice::HotasCougar => Some(cougar_profile()),
        _ => None,
    }
}

/// Return all devices that have a profile.
pub fn profiled_devices() -> Vec<ThrustmasterDevice> {
    vec![
        ThrustmasterDevice::T16000mJoystick,
        ThrustmasterDevice::TwcsThrottle,
        ThrustmasterDevice::WarthogJoystick,
        ThrustmasterDevice::WarthogThrottle,
        ThrustmasterDevice::TfrpRudderPedals,
        ThrustmasterDevice::TprPendular,
        ThrustmasterDevice::HotasCougar,
    ]
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Profile completeness ─────────────────────────────────────────────

    #[test]
    fn all_profiled_devices_return_some() {
        for dev in profiled_devices() {
            assert!(
                device_profile(dev).is_some(),
                "device_profile({:?}) returned None",
                dev
            );
        }
    }

    #[test]
    fn profiles_have_nonempty_names() {
        for dev in profiled_devices() {
            let p = device_profile(dev).unwrap();
            assert!(!p.name.is_empty(), "{:?} has empty name", dev);
        }
    }

    #[test]
    fn profiles_have_at_least_one_axis() {
        for dev in profiled_devices() {
            let p = device_profile(dev).unwrap();
            assert!(!p.axes.is_empty(), "{:?} has no axes", dev);
        }
    }

    #[test]
    fn all_axis_ids_are_unique_within_profile() {
        for dev in profiled_devices() {
            let p = device_profile(dev).unwrap();
            let mut ids: Vec<&str> = p.axes.iter().map(|a| a.id).collect();
            let count = ids.len();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), count, "{:?} has duplicate axis IDs", dev);
        }
    }

    #[test]
    fn deadzones_are_reasonable() {
        for dev in profiled_devices() {
            let p = device_profile(dev).unwrap();
            for ax in &p.axes {
                assert!(
                    ax.deadzone >= 0.0 && ax.deadzone <= 0.3,
                    "{:?}/{}: deadzone {} out of range",
                    dev,
                    ax.id,
                    ax.deadzone
                );
            }
        }
    }

    #[test]
    fn filter_alphas_are_reasonable() {
        for dev in profiled_devices() {
            let p = device_profile(dev).unwrap();
            for ax in &p.axes {
                if let Some(alpha) = ax.filter_alpha {
                    assert!(
                        alpha > 0.0 && alpha <= 1.0,
                        "{:?}/{}: alpha {} out of range",
                        dev,
                        ax.id,
                        alpha
                    );
                }
            }
        }
    }

    // ── T.16000M ─────────────────────────────────────────────────────────

    #[test]
    fn t16000m_has_4_axes_16_buttons_1_hat() {
        let p = device_profile(ThrustmasterDevice::T16000mJoystick).unwrap();
        assert_eq!(p.axes.len(), 4);
        assert_eq!(p.button_count, 16);
        assert_eq!(p.hat_count, 1);
        assert!(!p.has_leds);
    }

    #[test]
    fn t16000m_axis_ids() {
        let p = device_profile(ThrustmasterDevice::T16000mJoystick).unwrap();
        let ids: Vec<&str> = p.axes.iter().map(|a| a.id).collect();
        assert!(ids.contains(&"x"));
        assert!(ids.contains(&"y"));
        assert!(ids.contains(&"twist"));
        assert!(ids.contains(&"slider"));
    }

    #[test]
    fn t16000m_uses_14bit_for_stick_axes() {
        let p = device_profile(ThrustmasterDevice::T16000mJoystick).unwrap();
        let x_ax = p.axes.iter().find(|a| a.id == "x").unwrap();
        match x_ax.normalization {
            AxisNormalization::Bipolar { center, .. } => {
                assert!((center - 8191.5).abs() < 0.01, "expected 14-bit center");
            }
            _ => panic!("expected bipolar normalization for T.16000M X axis"),
        }
    }

    // ── TWCS ─────────────────────────────────────────────────────────────

    #[test]
    fn twcs_has_4_axes_14_buttons_0_hats() {
        let p = device_profile(ThrustmasterDevice::TwcsThrottle).unwrap();
        assert_eq!(p.axes.len(), 4);
        assert_eq!(p.button_count, 14);
        assert_eq!(p.hat_count, 0);
    }

    #[test]
    fn twcs_axis_ids() {
        let p = device_profile(ThrustmasterDevice::TwcsThrottle).unwrap();
        let ids: Vec<&str> = p.axes.iter().map(|a| a.id).collect();
        assert!(ids.contains(&"throttle"));
        assert!(ids.contains(&"mini_stick_x"));
        assert!(ids.contains(&"mini_stick_y"));
        assert!(ids.contains(&"rocker"));
    }

    // ── Warthog Joystick ─────────────────────────────────────────────────

    #[test]
    fn warthog_stick_has_2_axes_19_buttons_1_hat() {
        let p = device_profile(ThrustmasterDevice::WarthogJoystick).unwrap();
        assert_eq!(p.axes.len(), 2);
        assert_eq!(p.button_count, 19);
        assert_eq!(p.hat_count, 1);
        assert!(!p.has_leds);
    }

    #[test]
    fn warthog_stick_no_twist_axis() {
        let p = device_profile(ThrustmasterDevice::WarthogJoystick).unwrap();
        assert!(
            !p.axes.iter().any(|a| a.id == "twist"),
            "Warthog stick has no twist axis"
        );
    }

    // ── Warthog Throttle ─────────────────────────────────────────────────

    #[test]
    fn warthog_throttle_has_5_axes_40_buttons_2_hats_leds() {
        let p = device_profile(ThrustmasterDevice::WarthogThrottle).unwrap();
        assert_eq!(p.axes.len(), 5);
        assert_eq!(p.button_count, 40);
        assert_eq!(p.hat_count, 2);
        assert!(p.has_leds);
    }

    #[test]
    fn warthog_throttle_axis_ids() {
        let p = device_profile(ThrustmasterDevice::WarthogThrottle).unwrap();
        let ids: Vec<&str> = p.axes.iter().map(|a| a.id).collect();
        assert!(ids.contains(&"throttle_left"));
        assert!(ids.contains(&"throttle_right"));
        assert!(ids.contains(&"throttle_combined"));
        assert!(ids.contains(&"slew_x"));
        assert!(ids.contains(&"slew_y"));
    }

    #[test]
    fn warthog_throttle_slew_has_large_deadzone() {
        let p = device_profile(ThrustmasterDevice::WarthogThrottle).unwrap();
        let slew_x = p.axes.iter().find(|a| a.id == "slew_x").unwrap();
        assert!(
            slew_x.deadzone >= 0.10,
            "slew deadzone should be large: {}",
            slew_x.deadzone
        );
    }

    // ── TFRP ─────────────────────────────────────────────────────────────

    #[test]
    fn tfrp_has_3_axes_0_buttons_0_hats() {
        let p = device_profile(ThrustmasterDevice::TfrpRudderPedals).unwrap();
        assert_eq!(p.axes.len(), 3);
        assert_eq!(p.button_count, 0);
        assert_eq!(p.hat_count, 0);
    }

    #[test]
    fn tfrp_axis_ids() {
        let p = device_profile(ThrustmasterDevice::TfrpRudderPedals).unwrap();
        let ids: Vec<&str> = p.axes.iter().map(|a| a.id).collect();
        assert!(ids.contains(&"rudder"));
        assert!(ids.contains(&"right_pedal"));
        assert!(ids.contains(&"left_pedal"));
    }

    #[test]
    fn tfrp_uses_unipolar_normalization() {
        let p = device_profile(ThrustmasterDevice::TfrpRudderPedals).unwrap();
        for ax in &p.axes {
            assert!(
                matches!(ax.normalization, AxisNormalization::Unipolar { .. }),
                "TFRP axis {} should be unipolar",
                ax.id
            );
        }
    }

    // ── TPR ──────────────────────────────────────────────────────────────

    #[test]
    fn tpr_has_3_axes_0_buttons_no_filter() {
        let p = device_profile(ThrustmasterDevice::TprPendular).unwrap();
        assert_eq!(p.axes.len(), 3);
        assert_eq!(p.button_count, 0);
        for ax in &p.axes {
            assert!(
                ax.filter_alpha.is_none(),
                "TPR axis {} should have no filter",
                ax.id
            );
        }
    }

    #[test]
    fn tpr_bulk_variant_gets_same_profile() {
        let p1 = device_profile(ThrustmasterDevice::TprPendular).unwrap();
        let p2 = device_profile(ThrustmasterDevice::TprPendularBulk).unwrap();
        assert_eq!(p1.axes.len(), p2.axes.len());
        assert_eq!(p1.button_count, p2.button_count);
    }

    // ── Cougar ───────────────────────────────────────────────────────────

    #[test]
    fn cougar_has_3_axes_16_buttons_1_hat() {
        let p = device_profile(ThrustmasterDevice::HotasCougar).unwrap();
        assert_eq!(p.axes.len(), 3);
        assert_eq!(p.button_count, 16);
        assert_eq!(p.hat_count, 1);
    }

    // ── Unsupported devices ──────────────────────────────────────────────

    #[test]
    fn tflight_hotas_x_profile_is_none() {
        assert!(device_profile(ThrustmasterDevice::TFlightHotasX).is_none());
    }

    // ── Axis normalization parameters ────────────────────────────────────

    #[test]
    fn normalization_constants_are_positive() {
        let norms = [NORM_14BIT_BIPOLAR, NORM_16BIT_BIPOLAR, NORM_16BIT_UNIPOLAR];
        for n in &norms {
            match n {
                AxisNormalization::Bipolar { center, half_span } => {
                    assert!(*center > 0.0);
                    assert!(*half_span > 0.0);
                }
                AxisNormalization::Unipolar { max } => {
                    assert!(*max > 0.0);
                }
            }
        }
    }
}
