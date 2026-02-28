// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Per-device axis and button profile descriptors for all supported
//! CH Products flight peripherals.
//!
//! Each profile describes the full input surface (axes, buttons, hats) of a
//! device, together with recommended normalisation parameters. Profiles are
//! used by the axis engine and UI layer to automatically configure deadzones,
//! curve shapes, and label overlays.
//!
//! # Example
//!
//! ```
//! use flight_hotas_ch::profiles::{device_profile, DeviceProfile};
//! use flight_hotas_ch::devices::ChDevice;
//!
//! let profile = device_profile(ChDevice::Fighterstick).unwrap();
//! assert_eq!(profile.name, "CH Fighterstick");
//! assert_eq!(profile.axes.len(), 3);
//! assert_eq!(profile.button_count, 32);
//! ```

use crate::devices::ChDevice;

// ─── Axis descriptor ────────────────────────────────────────────────────────

/// How a raw HID axis value should be normalised.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxisNormalization {
    /// Centered bipolar: raw center maps to 0.0, extremes to ±1.0.
    Bipolar {
        /// Raw value at the center of the axis.
        center: f32,
        /// Half-span (center-to-extreme distance).
        half_span: f32,
    },
    /// Unipolar: 0 → 0.0, max → 1.0.
    Unipolar {
        /// Maximum raw value.
        max: f32,
    },
}

/// Descriptor for a single analog axis.
#[derive(Debug, Clone)]
pub struct AxisDescriptor {
    /// Short identifier (e.g. `"x"`, `"throttle"`, `"rudder"`).
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

/// Complete input profile for a CH Products device.
#[derive(Debug, Clone)]
pub struct DeviceProfile {
    /// Enumerated device variant.
    pub device: ChDevice,
    /// Human-readable device name.
    pub name: &'static str,
    /// All analog axes.
    pub axes: Vec<AxisDescriptor>,
    /// Total button count.
    pub button_count: u8,
    /// Number of hat switches.
    pub hat_count: u8,
    /// Notes about the device.
    pub notes: &'static str,
}

// ─── Normalisation presets ──────────────────────────────────────────────────

/// 16-bit centred bipolar (stick X/Y/Z axes).
const NORM_16BIT_BIPOLAR: AxisNormalization = AxisNormalization::Bipolar {
    center: 32767.5,
    half_span: 32767.5,
};

/// 16-bit unipolar (throttle levers, sliders, pedal axes).
const NORM_16BIT_UNIPOLAR: AxisNormalization = AxisNormalization::Unipolar { max: 65535.0 };

// ─── Profile builders ───────────────────────────────────────────────────────

fn fighterstick_profile() -> DeviceProfile {
    DeviceProfile {
        device: ChDevice::Fighterstick,
        name: "CH Fighterstick",
        axes: vec![
            AxisDescriptor {
                id: "x",
                label: "Stick X (Aileron)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.02,
                filter_alpha: Some(0.12),
            },
            AxisDescriptor {
                id: "y",
                label: "Stick Y (Elevator)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.02,
                filter_alpha: Some(0.12),
            },
            AxisDescriptor {
                id: "z",
                label: "Twist (Rudder)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.05,
                filter_alpha: Some(0.15),
            },
        ],
        button_count: 32,
        hat_count: 4,
        notes: "3 axes + twist, 4 hats (1 main 8-way + 3 secondary 4-way). Potentiometer axes.",
    }
}

fn pro_throttle_profile() -> DeviceProfile {
    DeviceProfile {
        device: ChDevice::ProThrottle,
        name: "CH Pro Throttle",
        axes: vec![
            AxisDescriptor {
                id: "throttle",
                label: "Main Throttle",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.01,
                filter_alpha: Some(0.08),
            },
            AxisDescriptor {
                id: "mini_stick_x",
                label: "Mini-stick X",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.08,
                filter_alpha: Some(0.20),
            },
            AxisDescriptor {
                id: "mini_stick_y",
                label: "Mini-stick Y",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.08,
                filter_alpha: Some(0.20),
            },
            AxisDescriptor {
                id: "rotary",
                label: "Rotary Dial",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.01,
                filter_alpha: Some(0.10),
            },
        ],
        button_count: 24,
        hat_count: 1,
        notes: "Throttle lever + spring-return mini-stick (analog) + rotary dial. 24 buttons, 1 hat.",
    }
}

fn pro_pedals_profile() -> DeviceProfile {
    DeviceProfile {
        device: ChDevice::ProPedals,
        name: "CH Pro Pedals",
        axes: vec![
            AxisDescriptor {
                id: "rudder",
                label: "Rudder (Combined)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.03,
                filter_alpha: Some(0.10),
            },
            AxisDescriptor {
                id: "left_toe",
                label: "Left Toe Brake",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.03,
                filter_alpha: Some(0.10),
            },
            AxisDescriptor {
                id: "right_toe",
                label: "Right Toe Brake",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.03,
                filter_alpha: Some(0.10),
            },
        ],
        button_count: 0,
        hat_count: 0,
        notes: "Differential toe brakes + combined rudder axis. No buttons or hats.",
    }
}

fn combatstick_profile() -> DeviceProfile {
    DeviceProfile {
        device: ChDevice::CombatStick,
        name: "CH Combat Stick",
        axes: vec![
            AxisDescriptor {
                id: "x",
                label: "Stick X (Aileron)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.02,
                filter_alpha: Some(0.12),
            },
            AxisDescriptor {
                id: "y",
                label: "Stick Y (Elevator)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.02,
                filter_alpha: Some(0.12),
            },
            AxisDescriptor {
                id: "z",
                label: "Twist (Rudder)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.05,
                filter_alpha: Some(0.15),
            },
        ],
        button_count: 24,
        hat_count: 1,
        notes: "Similar to Fighterstick but different grip. 3 axes + twist, 24 buttons, 1 hat.",
    }
}

fn eclipse_yoke_profile() -> DeviceProfile {
    DeviceProfile {
        device: ChDevice::EclipseYoke,
        name: "CH Flight Sim Eclipse Yoke",
        axes: vec![
            AxisDescriptor {
                id: "roll",
                label: "Yoke Roll (Aileron)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.02,
                filter_alpha: Some(0.10),
            },
            AxisDescriptor {
                id: "pitch",
                label: "Yoke Pitch (Elevator)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.02,
                filter_alpha: Some(0.10),
            },
            AxisDescriptor {
                id: "throttle",
                label: "Throttle Knob",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.02,
                filter_alpha: Some(0.10),
            },
        ],
        button_count: 32,
        hat_count: 1,
        notes: "Yoke form factor. Roll/Pitch via yoke + throttle knob on base. GA/transport sim use.",
    }
}

fn flight_yoke_profile() -> DeviceProfile {
    DeviceProfile {
        device: ChDevice::FlightYoke,
        name: "CH Flight Sim Yoke",
        axes: vec![
            AxisDescriptor {
                id: "roll",
                label: "Yoke Roll (Aileron)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.03,
                filter_alpha: Some(0.12),
            },
            AxisDescriptor {
                id: "pitch",
                label: "Yoke Pitch (Elevator)",
                normalization: NORM_16BIT_BIPOLAR,
                deadzone: 0.03,
                filter_alpha: Some(0.12),
            },
            AxisDescriptor {
                id: "throttle",
                label: "Throttle Lever",
                normalization: NORM_16BIT_UNIPOLAR,
                deadzone: 0.02,
                filter_alpha: Some(0.10),
            },
        ],
        button_count: 20,
        hat_count: 1,
        notes: "Classic CH yoke (circa 1997–2008). Predecessor to Eclipse Yoke. 8-bit axis resolution.",
    }
}

// ─── Profile lookup ─────────────────────────────────────────────────────────

/// Retrieve the input profile for a given CH Products device.
pub fn device_profile(device: ChDevice) -> Option<DeviceProfile> {
    match device {
        ChDevice::Fighterstick => Some(fighterstick_profile()),
        ChDevice::ProThrottle => Some(pro_throttle_profile()),
        ChDevice::ProPedals => Some(pro_pedals_profile()),
        ChDevice::CombatStick => Some(combatstick_profile()),
        ChDevice::EclipseYoke => Some(eclipse_yoke_profile()),
        ChDevice::FlightYoke => Some(flight_yoke_profile()),
    }
}

/// Return all devices that have a profile.
pub fn profiled_devices() -> Vec<ChDevice> {
    vec![
        ChDevice::Fighterstick,
        ChDevice::ProThrottle,
        ChDevice::ProPedals,
        ChDevice::CombatStick,
        ChDevice::EclipseYoke,
        ChDevice::FlightYoke,
    ]
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_profiled_devices_return_some() {
        for dev in profiled_devices() {
            assert!(
                device_profile(dev).is_some(),
                "device_profile({dev:?}) returned None"
            );
        }
    }

    #[test]
    fn profiles_have_nonempty_names() {
        for dev in profiled_devices() {
            let p = device_profile(dev).unwrap();
            assert!(!p.name.is_empty(), "{dev:?} has empty name");
        }
    }

    #[test]
    fn profiles_have_at_least_one_axis() {
        for dev in profiled_devices() {
            let p = device_profile(dev).unwrap();
            assert!(!p.axes.is_empty(), "{dev:?} has no axes");
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
            assert_eq!(ids.len(), count, "{dev:?} has duplicate axis IDs");
        }
    }

    #[test]
    fn deadzones_are_reasonable() {
        for dev in profiled_devices() {
            let p = device_profile(dev).unwrap();
            for ax in &p.axes {
                assert!(
                    ax.deadzone >= 0.0 && ax.deadzone <= 0.3,
                    "{dev:?}/{}: deadzone {} out of range",
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
                        "{dev:?}/{}: alpha {} out of range",
                        ax.id,
                        alpha
                    );
                }
            }
        }
    }

    #[test]
    fn fighterstick_has_3_axes_32_buttons_4_hats() {
        let p = device_profile(ChDevice::Fighterstick).unwrap();
        assert_eq!(p.axes.len(), 3);
        assert_eq!(p.button_count, 32);
        assert_eq!(p.hat_count, 4);
    }

    #[test]
    fn pro_throttle_has_4_axes_24_buttons_1_hat() {
        let p = device_profile(ChDevice::ProThrottle).unwrap();
        assert_eq!(p.axes.len(), 4);
        assert_eq!(p.button_count, 24);
        assert_eq!(p.hat_count, 1);
    }

    #[test]
    fn pro_throttle_has_mini_stick() {
        let p = device_profile(ChDevice::ProThrottle).unwrap();
        let ids: Vec<&str> = p.axes.iter().map(|a| a.id).collect();
        assert!(ids.contains(&"mini_stick_x"));
        assert!(ids.contains(&"mini_stick_y"));
    }

    #[test]
    fn pro_pedals_has_3_axes_0_buttons() {
        let p = device_profile(ChDevice::ProPedals).unwrap();
        assert_eq!(p.axes.len(), 3);
        assert_eq!(p.button_count, 0);
        assert_eq!(p.hat_count, 0);
    }

    #[test]
    fn combatstick_has_3_axes_24_buttons_1_hat() {
        let p = device_profile(ChDevice::CombatStick).unwrap();
        assert_eq!(p.axes.len(), 3);
        assert_eq!(p.button_count, 24);
        assert_eq!(p.hat_count, 1);
    }

    #[test]
    fn eclipse_yoke_has_3_axes_32_buttons_1_hat() {
        let p = device_profile(ChDevice::EclipseYoke).unwrap();
        assert_eq!(p.axes.len(), 3);
        assert_eq!(p.button_count, 32);
        assert_eq!(p.hat_count, 1);
    }

    #[test]
    fn flight_yoke_has_3_axes_20_buttons_1_hat() {
        let p = device_profile(ChDevice::FlightYoke).unwrap();
        assert_eq!(p.axes.len(), 3);
        assert_eq!(p.button_count, 20);
        assert_eq!(p.hat_count, 1);
    }

    #[test]
    fn yoke_profiles_have_roll_and_pitch() {
        for dev in [ChDevice::EclipseYoke, ChDevice::FlightYoke] {
            let p = device_profile(dev).unwrap();
            let ids: Vec<&str> = p.axes.iter().map(|a| a.id).collect();
            assert!(ids.contains(&"roll"), "{dev:?} missing roll axis");
            assert!(ids.contains(&"pitch"), "{dev:?} missing pitch axis");
            assert!(ids.contains(&"throttle"), "{dev:?} missing throttle axis");
        }
    }

    #[test]
    fn normalization_constants_are_positive() {
        let norms = [NORM_16BIT_BIPOLAR, NORM_16BIT_UNIPOLAR];
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
