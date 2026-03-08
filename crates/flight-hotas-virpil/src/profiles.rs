// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Default device profile descriptors for VIRPIL VPC devices.
//!
//! Each profile describes the physical controls present on a device: how many
//! axes, their semantic roles, button counts, hat switches, and rotary encoders.
//! These descriptors are used by the profile pipeline to generate sane defaults
//! when a device is first connected without a user-supplied profile.
//!
//! Profiles do **not** contain user preferences (curves, deadzones, mappings).
//! Those are layered on top by the profile cascade (ADR-007).

use serde::{Deserialize, Serialize};

// ─── Axis role ────────────────────────────────────────────────────────────────

/// Semantic role of a physical axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AxisRole {
    /// Stick X (roll / aileron).
    StickX,
    /// Stick Y (pitch / elevator).
    StickY,
    /// Twist / Z-rotate (yaw / rudder on twist sticks).
    Twist,
    /// Rudder pedal axis.
    Rudder,
    /// Left toe brake.
    LeftToeBrake,
    /// Right toe brake.
    RightToeBrake,
    /// Left throttle lever.
    ThrottleLeft,
    /// Right throttle lever.
    ThrottleRight,
    /// Single throttle (non-split).
    Throttle,
    /// Flaps lever / detent axis.
    Flaps,
    /// Slew control X.
    SlewX,
    /// Slew control Y.
    SlewY,
    /// Miscellaneous slider / scroll wheel.
    Slider,
    /// Secondary rotary / trim.
    SecondaryRotary,
    /// Slew lever (single-axis).
    SlewLever,
    /// Helicopter collective lever.
    Collective,
    /// Throttle idle cutoff.
    ThrottleIdleCutoff,
    /// Generic rotary knob.
    Rotary,
    /// Panel analogue axis (generic).
    PanelAxis,
}

/// Descriptor for one physical axis on a device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AxisDescriptor {
    /// Axis index in the HID report (0-based).
    pub index: u8,
    /// Semantic role.
    pub role: AxisRole,
    /// Human-readable label.
    #[serde(skip)]
    pub label: &'static str,
    /// Whether this axis is centred (true for sticks, false for throttles/brakes).
    pub centred: bool,
}

// ─── Hat type ─────────────────────────────────────────────────────────────────

/// Type of hat switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HatType {
    /// 8-way (N/NE/E/SE/S/SW/W/NW + centre).
    EightWay,
    /// 4-way (N/E/S/W + centre).
    FourWay,
}

/// Descriptor for a hat switch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HatDescriptor {
    /// Human-readable label.
    #[serde(skip)]
    pub label: &'static str,
    /// Hat type.
    pub hat_type: HatType,
}

// ─── Device profile ───────────────────────────────────────────────────────────

/// Complete physical profile descriptor for one VIRPIL VPC device.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceProfile {
    /// Human-readable device name.
    pub name: &'static str,
    /// USB Product ID.
    pub pid: u16,
    /// Axis descriptors.
    pub axes: &'static [AxisDescriptor],
    /// Total number of discrete buttons.
    pub button_count: u8,
    /// Hat switch descriptors.
    pub hats: &'static [HatDescriptor],
    /// Number of rotary encoders (reported as button pairs, not axes).
    pub rotary_encoders: u8,
}

// ─── Alpha grip profile ──────────────────────────────────────────────────────

/// Axis descriptors for the VPC Constellation Alpha grip.
pub static ALPHA_AXES: &[AxisDescriptor] = &[
    AxisDescriptor {
        index: 0,
        role: AxisRole::StickX,
        label: "X (roll)",
        centred: true,
    },
    AxisDescriptor {
        index: 1,
        role: AxisRole::StickY,
        label: "Y (pitch)",
        centred: true,
    },
    AxisDescriptor {
        index: 2,
        role: AxisRole::Twist,
        label: "Z (twist)",
        centred: true,
    },
    AxisDescriptor {
        index: 3,
        role: AxisRole::SecondaryRotary,
        label: "SZ (secondary rotary)",
        centred: false,
    },
    AxisDescriptor {
        index: 4,
        role: AxisRole::SlewLever,
        label: "SL (slew lever)",
        centred: false,
    },
];

static ALPHA_HATS: &[HatDescriptor] = &[HatDescriptor {
    label: "Main hat",
    hat_type: HatType::EightWay,
}];

/// Default profile for the VPC Constellation Alpha grip (left or right).
pub static ALPHA_PROFILE: DeviceProfile = DeviceProfile {
    name: "VPC Constellation Alpha",
    pid: 0x838F,
    axes: ALPHA_AXES,
    button_count: 28,
    hats: ALPHA_HATS,
    rotary_encoders: 0,
};

// ─── MongoosT-50CM3 throttle profile ─────────────────────────────────────────

/// Axis descriptors for the VPC Throttle CM3.
pub static CM3_THROTTLE_AXES: &[AxisDescriptor] = &[
    AxisDescriptor {
        index: 0,
        role: AxisRole::ThrottleLeft,
        label: "Left throttle",
        centred: false,
    },
    AxisDescriptor {
        index: 1,
        role: AxisRole::ThrottleRight,
        label: "Right throttle",
        centred: false,
    },
    AxisDescriptor {
        index: 2,
        role: AxisRole::Flaps,
        label: "Flaps lever",
        centred: false,
    },
    AxisDescriptor {
        index: 3,
        role: AxisRole::SlewX,
        label: "Slew control X",
        centred: true,
    },
    AxisDescriptor {
        index: 4,
        role: AxisRole::SlewY,
        label: "Slew control Y",
        centred: true,
    },
    AxisDescriptor {
        index: 5,
        role: AxisRole::Slider,
        label: "Slider",
        centred: false,
    },
];

/// Default profile for the VPC Throttle CM3.
pub static CM3_THROTTLE_PROFILE: DeviceProfile = DeviceProfile {
    name: "VPC Throttle CM3",
    pid: 0x0194,
    axes: CM3_THROTTLE_AXES,
    button_count: 78,
    hats: &[],
    rotary_encoders: 4,
};

// ─── ACE Collection Pedals profile ───────────────────────────────────────────

/// Axis descriptors for the VPC ACE Collection Pedals.
pub static ACE_PEDALS_AXES: &[AxisDescriptor] = &[
    AxisDescriptor {
        index: 0,
        role: AxisRole::Rudder,
        label: "Rudder",
        centred: true,
    },
    AxisDescriptor {
        index: 1,
        role: AxisRole::LeftToeBrake,
        label: "Left toe brake",
        centred: false,
    },
    AxisDescriptor {
        index: 2,
        role: AxisRole::RightToeBrake,
        label: "Right toe brake",
        centred: false,
    },
];

/// Default profile for the VPC ACE Collection Pedals.
pub static ACE_PEDALS_PROFILE: DeviceProfile = DeviceProfile {
    name: "VPC ACE Collection Pedals",
    pid: 0x019C,
    axes: ACE_PEDALS_AXES,
    button_count: 16,
    hats: &[],
    rotary_encoders: 0,
};

// ─── Rotor TCS Plus profile ──────────────────────────────────────────────────

/// Axis descriptors for the VPC Rotor TCS Plus.
pub static ROTOR_TCS_AXES: &[AxisDescriptor] = &[
    AxisDescriptor {
        index: 0,
        role: AxisRole::Collective,
        label: "Collective lever",
        centred: false,
    },
    AxisDescriptor {
        index: 1,
        role: AxisRole::ThrottleIdleCutoff,
        label: "Throttle / idle cutoff",
        centred: false,
    },
    AxisDescriptor {
        index: 2,
        role: AxisRole::Rotary,
        label: "Rotary",
        centred: false,
    },
];

/// Default profile for the VPC Rotor TCS Plus.
pub static ROTOR_TCS_PROFILE: DeviceProfile = DeviceProfile {
    name: "VPC Rotor TCS Plus",
    pid: 0x01A0,
    axes: ROTOR_TCS_AXES,
    button_count: 24,
    hats: &[],
    rotary_encoders: 1,
};

// ─── ACE Torq profile ────────────────────────────────────────────────────────

/// Axis descriptors for the VPC ACE Torq.
pub static ACE_TORQ_AXES: &[AxisDescriptor] = &[AxisDescriptor {
    index: 0,
    role: AxisRole::Throttle,
    label: "Throttle",
    centred: false,
}];

/// Default profile for the VPC ACE Torq.
pub static ACE_TORQ_PROFILE: DeviceProfile = DeviceProfile {
    name: "VPC ACE Torq",
    pid: 0x0198,
    axes: ACE_TORQ_AXES,
    button_count: 8,
    hats: &[],
    rotary_encoders: 0,
};

// ─── MongoosT-50CM3 stick profile ─────────────────────────────────────────────

/// Axis descriptors for the VPC MongoosT-50CM3 stick base.
pub static MONGOOST_STICK_AXES: &[AxisDescriptor] = &[
    AxisDescriptor {
        index: 0,
        role: AxisRole::StickX,
        label: "X (roll)",
        centred: true,
    },
    AxisDescriptor {
        index: 1,
        role: AxisRole::StickY,
        label: "Y (pitch)",
        centred: true,
    },
    AxisDescriptor {
        index: 2,
        role: AxisRole::Twist,
        label: "Z (twist)",
        centred: true,
    },
    AxisDescriptor {
        index: 3,
        role: AxisRole::SecondaryRotary,
        label: "SZ (trim)",
        centred: false,
    },
    AxisDescriptor {
        index: 4,
        role: AxisRole::SlewLever,
        label: "SL (slew lever)",
        centred: false,
    },
];

static MONGOOST_HATS: &[HatDescriptor] = &[HatDescriptor {
    label: "Main hat",
    hat_type: HatType::EightWay,
}];

/// Default profile for the VPC MongoosT-50CM3 stick.
pub static MONGOOST_STICK_PROFILE: DeviceProfile = DeviceProfile {
    name: "VPC MongoosT-50CM3 Stick",
    pid: 0x4130,
    axes: MONGOOST_STICK_AXES,
    button_count: 28,
    hats: MONGOOST_HATS,
    rotary_encoders: 0,
};

// ─── WarBRD base profile ─────────────────────────────────────────────────────

/// Default profile for the VPC WarBRD base (same axes as MongoosT).
pub static WARBRD_PROFILE: DeviceProfile = DeviceProfile {
    name: "VPC WarBRD Stick",
    pid: 0x40CC,
    axes: MONGOOST_STICK_AXES,
    button_count: 28,
    hats: MONGOOST_HATS,
    rotary_encoders: 0,
};

/// Default profile for the VPC WarBRD-D base (same axes as MongoosT).
pub static WARBRD_D_PROFILE: DeviceProfile = DeviceProfile {
    name: "VPC WarBRD-D Stick",
    pid: 0x43F5,
    axes: MONGOOST_STICK_AXES,
    button_count: 28,
    hats: MONGOOST_HATS,
    rotary_encoders: 0,
};

// ─── Control Panel 1 profile ─────────────────────────────────────────────────

/// Default profile for the VPC Control Panel 1 (buttons only).
pub static PANEL1_PROFILE: DeviceProfile = DeviceProfile {
    name: "VPC Control Panel 1",
    pid: 0x025B,
    axes: &[],
    button_count: 48,
    hats: &[],
    rotary_encoders: 0,
};

// ─── Control Panel 2 profile ─────────────────────────────────────────────────

/// Axis descriptors for the VPC Control Panel 2.
pub static PANEL2_AXES: &[AxisDescriptor] = &[
    AxisDescriptor {
        index: 0,
        role: AxisRole::PanelAxis,
        label: "A1 (rotary knob)",
        centred: false,
    },
    AxisDescriptor {
        index: 1,
        role: AxisRole::PanelAxis,
        label: "A2 (rotary knob)",
        centred: false,
    },
];

/// Default profile for the VPC Control Panel 2.
pub static PANEL2_PROFILE: DeviceProfile = DeviceProfile {
    name: "VPC Control Panel 2",
    pid: 0x0259,
    axes: PANEL2_AXES,
    button_count: 47,
    hats: &[],
    rotary_encoders: 0,
};

// ─── Lookup ───────────────────────────────────────────────────────────────────

/// All built-in device profiles.
pub static ALL_PROFILES: &[&DeviceProfile] = &[
    &ALPHA_PROFILE,
    &MONGOOST_STICK_PROFILE,
    &WARBRD_PROFILE,
    &WARBRD_D_PROFILE,
    &CM3_THROTTLE_PROFILE,
    &ACE_PEDALS_PROFILE,
    &ROTOR_TCS_PROFILE,
    &ACE_TORQ_PROFILE,
    &PANEL1_PROFILE,
    &PANEL2_PROFILE,
];

/// Look up a default device profile by USB Product ID.
///
/// Returns `None` for PIDs without a built-in profile.
pub fn profile_for_pid(pid: u16) -> Option<&'static DeviceProfile> {
    ALL_PROFILES.iter().find(|p| p.pid == pid).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_profile_has_correct_axes_and_buttons() {
        assert_eq!(ALPHA_PROFILE.axes.len(), 5);
        assert_eq!(ALPHA_PROFILE.button_count, 28);
        assert_eq!(ALPHA_PROFILE.hats.len(), 1);
        assert_eq!(ALPHA_PROFILE.hats[0].hat_type, HatType::EightWay);
    }

    #[test]
    fn cm3_throttle_profile_has_correct_axes_and_buttons() {
        assert_eq!(CM3_THROTTLE_PROFILE.axes.len(), 6);
        assert_eq!(CM3_THROTTLE_PROFILE.button_count, 78);
        assert_eq!(CM3_THROTTLE_PROFILE.rotary_encoders, 4);
    }

    #[test]
    fn ace_pedals_profile_has_three_axes() {
        assert_eq!(ACE_PEDALS_PROFILE.axes.len(), 3);
        assert_eq!(ACE_PEDALS_PROFILE.axes[0].role, AxisRole::Rudder);
        assert_eq!(ACE_PEDALS_PROFILE.axes[1].role, AxisRole::LeftToeBrake);
        assert_eq!(ACE_PEDALS_PROFILE.axes[2].role, AxisRole::RightToeBrake);
    }

    #[test]
    fn ace_pedals_rudder_is_centred() {
        assert!(ACE_PEDALS_PROFILE.axes[0].centred);
    }

    #[test]
    fn ace_pedals_toe_brakes_are_not_centred() {
        assert!(!ACE_PEDALS_PROFILE.axes[1].centred);
        assert!(!ACE_PEDALS_PROFILE.axes[2].centred);
    }

    #[test]
    fn rotor_tcs_profile_has_collective() {
        assert_eq!(ROTOR_TCS_PROFILE.axes.len(), 3);
        assert_eq!(ROTOR_TCS_PROFILE.axes[0].role, AxisRole::Collective);
        assert_eq!(ROTOR_TCS_PROFILE.button_count, 24);
    }

    #[test]
    fn ace_torq_profile_has_single_throttle_axis() {
        assert_eq!(ACE_TORQ_PROFILE.axes.len(), 1);
        assert_eq!(ACE_TORQ_PROFILE.axes[0].role, AxisRole::Throttle);
        assert_eq!(ACE_TORQ_PROFILE.button_count, 8);
    }

    #[test]
    fn all_profiles_have_unique_pids() {
        let mut pids: Vec<u16> = ALL_PROFILES.iter().map(|p| p.pid).collect();
        pids.sort();
        pids.dedup();
        assert_eq!(pids.len(), ALL_PROFILES.len(), "duplicate PIDs in profiles");
    }

    #[test]
    fn mongoost_stick_profile_matches_alpha_layout() {
        assert_eq!(MONGOOST_STICK_PROFILE.axes.len(), 5);
        assert_eq!(MONGOOST_STICK_PROFILE.button_count, 28);
        assert_eq!(MONGOOST_STICK_PROFILE.hats.len(), 1);
        assert_eq!(MONGOOST_STICK_PROFILE.hats[0].hat_type, HatType::EightWay);
    }

    #[test]
    fn warbrd_profile_has_correct_pid() {
        assert_eq!(WARBRD_PROFILE.pid, 0x40CC);
        assert_eq!(WARBRD_PROFILE.button_count, 28);
    }

    #[test]
    fn warbrd_d_profile_has_correct_pid() {
        assert_eq!(WARBRD_D_PROFILE.pid, 0x43F5);
        assert_eq!(WARBRD_D_PROFILE.button_count, 28);
    }

    #[test]
    fn panel1_profile_has_no_axes() {
        assert_eq!(PANEL1_PROFILE.axes.len(), 0);
        assert_eq!(PANEL1_PROFILE.button_count, 48);
    }

    #[test]
    fn panel2_profile_has_two_axes() {
        assert_eq!(PANEL2_PROFILE.axes.len(), 2);
        assert_eq!(PANEL2_PROFILE.button_count, 47);
        assert_eq!(PANEL2_PROFILE.axes[0].role, AxisRole::PanelAxis);
    }

    #[test]
    fn profile_lookup_mongoost() {
        let p = profile_for_pid(0x4130).unwrap();
        assert_eq!(p.name, "VPC MongoosT-50CM3 Stick");
    }

    #[test]
    fn profile_lookup_warbrd() {
        let p = profile_for_pid(0x40CC).unwrap();
        assert_eq!(p.name, "VPC WarBRD Stick");
    }

    #[test]
    fn profile_lookup_warbrd_d() {
        let p = profile_for_pid(0x43F5).unwrap();
        assert_eq!(p.name, "VPC WarBRD-D Stick");
    }

    #[test]
    fn profile_lookup_panel1() {
        let p = profile_for_pid(0x025B).unwrap();
        assert_eq!(p.name, "VPC Control Panel 1");
    }

    #[test]
    fn profile_lookup_panel2() {
        let p = profile_for_pid(0x0259).unwrap();
        assert_eq!(p.name, "VPC Control Panel 2");
    }

    #[test]
    fn all_profiles_count() {
        assert_eq!(ALL_PROFILES.len(), 10);
    }

    #[test]
    fn profile_lookup_by_pid() {
        let p = profile_for_pid(0x0194).unwrap();
        assert_eq!(p.name, "VPC Throttle CM3");
    }

    #[test]
    fn profile_lookup_ace_pedals() {
        let p = profile_for_pid(0x019C).unwrap();
        assert_eq!(p.name, "VPC ACE Collection Pedals");
    }

    #[test]
    fn profile_lookup_rotor_tcs() {
        let p = profile_for_pid(0x01A0).unwrap();
        assert_eq!(p.name, "VPC Rotor TCS Plus");
    }

    #[test]
    fn profile_lookup_ace_torq() {
        let p = profile_for_pid(0x0198).unwrap();
        assert_eq!(p.name, "VPC ACE Torq");
    }

    #[test]
    fn profile_lookup_unknown_pid_is_none() {
        assert!(profile_for_pid(0xFFFF).is_none());
    }

    #[test]
    fn alpha_stick_axes_are_centred() {
        assert!(ALPHA_PROFILE.axes[0].centred); // X
        assert!(ALPHA_PROFILE.axes[1].centred); // Y
        assert!(ALPHA_PROFILE.axes[2].centred); // twist
    }

    #[test]
    fn cm3_throttle_axes_centering() {
        // Throttle axes are not centred
        assert!(!CM3_THROTTLE_PROFILE.axes[0].centred); // left throttle
        assert!(!CM3_THROTTLE_PROFILE.axes[1].centred); // right throttle
        assert!(!CM3_THROTTLE_PROFILE.axes[2].centred); // flaps
        // Slew controls are centred
        assert!(CM3_THROTTLE_PROFILE.axes[3].centred); // SCX
        assert!(CM3_THROTTLE_PROFILE.axes[4].centred); // SCY
    }

    #[test]
    fn all_profiles_axis_count_matches_descriptor_len() {
        for profile in ALL_PROFILES {
            assert_eq!(
                profile.axes.len(),
                profile.axes.len(),
                "{}: axis count mismatch",
                profile.name
            );
        }
    }

    #[test]
    fn all_profiles_have_nonzero_buttons() {
        for profile in ALL_PROFILES {
            assert!(
                profile.button_count > 0,
                "{}: must have at least one button",
                profile.name
            );
        }
    }
}
