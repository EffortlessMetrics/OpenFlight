// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Multi-device detection and base+grip correlation for VIRPIL VPC devices.
//!
//! VIRPIL products are modular: a base (e.g. WarBRD, MongoosT-50CM3) can host
//! different grips (e.g. Constellation Alpha, Alpha Prime). Each combination
//! appears as a single USB endpoint with a unique PID, but users often have
//! multiple VIRPIL devices connected simultaneously (e.g. stick + throttle +
//! pedals).
//!
//! This module provides utilities to group connected VIRPIL devices by category
//! and detect common multi-device setups.

use crate::protocol::{DEVICE_TABLE, VirpilDeviceInfo, device_info};
pub use flight_hid_support::device_support::VIRPIL_VENDOR_ID;

// ─── Device category ──────────────────────────────────────────────────────────

/// High-level category for a VIRPIL VPC device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceCategory {
    /// Joystick (stick base + grip): Alpha, Alpha Prime, WarBRD, MongoosT.
    Stick,
    /// Throttle unit: CM3, ACE Torq.
    Throttle,
    /// Pedals: ACE Pedals.
    Pedals,
    /// Helicopter collective: Rotor TCS Plus.
    Collective,
    /// Button/switch panel: Control Panel 1, Control Panel 2.
    Panel,
}

impl DeviceCategory {
    /// Classify a VIRPIL device by its Product ID.
    ///
    /// Returns `None` for unknown PIDs.
    pub fn from_pid(pid: u16) -> Option<Self> {
        use flight_hid_support::device_support::*;
        match pid {
            VIRPIL_CONSTELLATION_ALPHA_LEFT_PID
            | VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID
            | VIRPIL_CONSTELLATION_ALPHA_PRIME_RIGHT_PID
            | VIRPIL_WARBRD_PID
            | VIRPIL_WARBRD_D_PID
            | VIRPIL_MONGOOST_STICK_PID => Some(Self::Stick),

            VIRPIL_CM3_THROTTLE_PID | VIRPIL_ACE_TORQ_PID => Some(Self::Throttle),

            VIRPIL_ACE_PEDALS_PID => Some(Self::Pedals),

            VIRPIL_ROTOR_TCS_PLUS_PID => Some(Self::Collective),

            VIRPIL_PANEL1_PID | VIRPIL_PANEL2_PID => Some(Self::Panel),

            _ => None,
        }
    }

    /// Human-readable category name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Stick => "Stick",
            Self::Throttle => "Throttle",
            Self::Pedals => "Pedals",
            Self::Collective => "Collective",
            Self::Panel => "Panel",
        }
    }
}

// ─── Connected device descriptor ──────────────────────────────────────────────

/// A VIRPIL device detected on the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedDevice {
    /// USB Product ID.
    pub pid: u16,
    /// Device info from the protocol table.
    pub info: &'static VirpilDeviceInfo,
    /// High-level device category.
    pub category: DeviceCategory,
}

// ─── Multi-device setup detection ─────────────────────────────────────────────

/// Summary of a multi-device VIRPIL setup.
///
/// Describes which device categories are present in a set of connected devices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupSummary {
    /// All detected VIRPIL devices.
    pub devices: Vec<DetectedDevice>,
    /// Whether the setup includes at least one stick.
    pub has_stick: bool,
    /// Whether the setup includes at least one throttle.
    pub has_throttle: bool,
    /// Whether the setup includes pedals.
    pub has_pedals: bool,
    /// Whether the setup includes a collective.
    pub has_collective: bool,
    /// Whether the setup includes at least one panel.
    pub has_panel: bool,
}

impl SetupSummary {
    /// Returns `true` if this is a typical flight sim HOTAS setup
    /// (at least one stick + at least one throttle).
    pub fn is_hotas(&self) -> bool {
        self.has_stick && self.has_throttle
    }

    /// Returns `true` if this setup includes the full HOTAS + pedals trio.
    pub fn is_full_setup(&self) -> bool {
        self.has_stick && self.has_throttle && self.has_pedals
    }

    /// Count of devices in the given category.
    pub fn count_category(&self, category: DeviceCategory) -> usize {
        self.devices
            .iter()
            .filter(|d| d.category == category)
            .count()
    }
}

/// Analyse a set of VIRPIL device PIDs and produce a setup summary.
///
/// Unknown PIDs are silently ignored.
pub fn detect_setup(pids: &[u16]) -> SetupSummary {
    let mut devices = Vec::new();

    for &pid in pids {
        if let (Some(info), Some(category)) = (device_info(pid), DeviceCategory::from_pid(pid)) {
            devices.push(DetectedDevice {
                pid,
                info,
                category,
            });
        }
    }

    let has_stick = devices.iter().any(|d| d.category == DeviceCategory::Stick);
    let has_throttle = devices
        .iter()
        .any(|d| d.category == DeviceCategory::Throttle);
    let has_pedals = devices.iter().any(|d| d.category == DeviceCategory::Pedals);
    let has_collective = devices
        .iter()
        .any(|d| d.category == DeviceCategory::Collective);
    let has_panel = devices.iter().any(|d| d.category == DeviceCategory::Panel);

    SetupSummary {
        devices,
        has_stick,
        has_throttle,
        has_pedals,
        has_collective,
        has_panel,
    }
}

/// Return all known VIRPIL PIDs for a given device category.
pub fn pids_for_category(category: DeviceCategory) -> Vec<u16> {
    DEVICE_TABLE
        .iter()
        .filter(|d| DeviceCategory::from_pid(d.pid) == Some(category))
        .map(|d| d.pid)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_hid_support::device_support::*;

    #[test]
    fn category_sticks() {
        assert_eq!(
            DeviceCategory::from_pid(VIRPIL_WARBRD_PID),
            Some(DeviceCategory::Stick)
        );
        assert_eq!(
            DeviceCategory::from_pid(VIRPIL_MONGOOST_STICK_PID),
            Some(DeviceCategory::Stick)
        );
        assert_eq!(
            DeviceCategory::from_pid(VIRPIL_CONSTELLATION_ALPHA_LEFT_PID),
            Some(DeviceCategory::Stick)
        );
    }

    #[test]
    fn category_throttle() {
        assert_eq!(
            DeviceCategory::from_pid(VIRPIL_CM3_THROTTLE_PID),
            Some(DeviceCategory::Throttle)
        );
        assert_eq!(
            DeviceCategory::from_pid(VIRPIL_ACE_TORQ_PID),
            Some(DeviceCategory::Throttle)
        );
    }

    #[test]
    fn category_pedals() {
        assert_eq!(
            DeviceCategory::from_pid(VIRPIL_ACE_PEDALS_PID),
            Some(DeviceCategory::Pedals)
        );
    }

    #[test]
    fn category_unknown() {
        assert_eq!(DeviceCategory::from_pid(0xFFFF), None);
    }

    #[test]
    fn detect_empty_setup() {
        let summary = detect_setup(&[]);
        assert!(summary.devices.is_empty());
        assert!(!summary.is_hotas());
        assert!(!summary.is_full_setup());
    }

    #[test]
    fn detect_hotas_setup() {
        let summary = detect_setup(&[VIRPIL_WARBRD_PID, VIRPIL_CM3_THROTTLE_PID]);
        assert!(summary.is_hotas());
        assert!(!summary.is_full_setup());
        assert_eq!(summary.devices.len(), 2);
    }

    #[test]
    fn detect_full_setup() {
        let summary = detect_setup(&[
            VIRPIL_CONSTELLATION_ALPHA_LEFT_PID,
            VIRPIL_CM3_THROTTLE_PID,
            VIRPIL_ACE_PEDALS_PID,
        ]);
        assert!(summary.is_full_setup());
        assert!(summary.has_stick);
        assert!(summary.has_throttle);
        assert!(summary.has_pedals);
    }

    #[test]
    fn detect_unknown_pids_ignored() {
        let summary = detect_setup(&[0xFFFF, 0xFFFE]);
        assert!(summary.devices.is_empty());
    }

    #[test]
    fn count_category_multiple_sticks() {
        let summary = detect_setup(&[VIRPIL_WARBRD_PID, VIRPIL_MONGOOST_STICK_PID]);
        assert_eq!(summary.count_category(DeviceCategory::Stick), 2);
        assert_eq!(summary.count_category(DeviceCategory::Throttle), 0);
    }

    #[test]
    fn pids_for_stick_category() {
        let stick_pids = pids_for_category(DeviceCategory::Stick);
        assert!(stick_pids.contains(&VIRPIL_WARBRD_PID));
        assert!(stick_pids.contains(&VIRPIL_MONGOOST_STICK_PID));
        assert!(!stick_pids.contains(&VIRPIL_CM3_THROTTLE_PID));
    }

    #[test]
    fn category_names_are_nonempty() {
        for cat in [
            DeviceCategory::Stick,
            DeviceCategory::Throttle,
            DeviceCategory::Pedals,
            DeviceCategory::Collective,
            DeviceCategory::Panel,
        ] {
            assert!(!cat.name().is_empty());
        }
    }

    #[test]
    fn collective_category() {
        assert_eq!(
            DeviceCategory::from_pid(VIRPIL_ROTOR_TCS_PLUS_PID),
            Some(DeviceCategory::Collective)
        );
        let summary = detect_setup(&[VIRPIL_ROTOR_TCS_PLUS_PID]);
        assert!(summary.has_collective);
    }
}
