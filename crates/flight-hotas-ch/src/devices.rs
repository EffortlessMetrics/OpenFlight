// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! CH Products device identification database.
//!
//! All CH Products flight peripherals share USB VID `0x068E`. Use
//! [`identify_device`] to map a `(vid, pid)` pair to a [`ChDevice`] variant,
//! or iterate [`DEVICE_TABLE`] for the complete catalogue.
//!
//! PIDs sourced from the Linux kernel `hid-ids.h` and CH Products documentation.

pub use flight_hid_support::device_support::{
    CH_COMBAT_STICK_PID, CH_ECLIPSE_YOKE_PID, CH_FIGHTERSTICK_PID, CH_FLIGHT_YOKE_PID,
    CH_PRO_PEDALS_PID, CH_PRO_THROTTLE_PID, CH_VENDOR_ID,
};

/// Complete catalogue of known CH Products flight-sim USB devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChDevice {
    /// CH Fighterstick — 3 axes + twist, 32 buttons, 4 hats.
    Fighterstick,
    /// CH Pro Throttle — throttle lever + mini-stick + rotaries, 24 buttons.
    ProThrottle,
    /// CH Pro Pedals — rudder + differential toe brakes.
    ProPedals,
    /// CH Combat Stick — 4 axes, 24 buttons, 1 hat.
    CombatStick,
    /// CH Flight Sim Eclipse Yoke — yoke (roll/pitch) + throttle knob.
    EclipseYoke,
    /// CH Flight Sim Yoke — classic yoke form factor.
    FlightYoke,
}

impl ChDevice {
    /// Human-readable product name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fighterstick => "CH Fighterstick",
            Self::ProThrottle => "CH Pro Throttle",
            Self::ProPedals => "CH Pro Pedals",
            Self::CombatStick => "CH Combat Stick",
            Self::EclipseYoke => "CH Flight Sim Eclipse Yoke",
            Self::FlightYoke => "CH Flight Sim Yoke",
        }
    }

    /// USB Product ID for this device.
    pub const fn pid(self) -> u16 {
        match self {
            Self::Fighterstick => CH_FIGHTERSTICK_PID,
            Self::ProThrottle => CH_PRO_THROTTLE_PID,
            Self::ProPedals => CH_PRO_PEDALS_PID,
            Self::CombatStick => CH_COMBAT_STICK_PID,
            Self::EclipseYoke => CH_ECLIPSE_YOKE_PID,
            Self::FlightYoke => CH_FLIGHT_YOKE_PID,
        }
    }
}

/// A row in the device identification table.
#[derive(Debug, Clone, Copy)]
pub struct DeviceEntry {
    /// USB Product ID.
    pub pid: u16,
    /// Enumerated device variant.
    pub device: ChDevice,
}

/// Complete VID/PID lookup table for all known CH Products flight devices.
///
/// All entries share VID [`CH_VENDOR_ID`] (`0x068E`).
pub const DEVICE_TABLE: &[DeviceEntry] = &[
    DeviceEntry {
        pid: CH_PRO_THROTTLE_PID,
        device: ChDevice::ProThrottle,
    },
    DeviceEntry {
        pid: CH_PRO_PEDALS_PID,
        device: ChDevice::ProPedals,
    },
    DeviceEntry {
        pid: CH_FIGHTERSTICK_PID,
        device: ChDevice::Fighterstick,
    },
    DeviceEntry {
        pid: CH_COMBAT_STICK_PID,
        device: ChDevice::CombatStick,
    },
    DeviceEntry {
        pid: CH_ECLIPSE_YOKE_PID,
        device: ChDevice::EclipseYoke,
    },
    DeviceEntry {
        pid: CH_FLIGHT_YOKE_PID,
        device: ChDevice::FlightYoke,
    },
];

/// Identify a CH Products device by USB VID/PID.
///
/// Returns `None` if the VID is not `0x068E` or the PID is unknown.
pub fn identify_device(vendor_id: u16, product_id: u16) -> Option<ChDevice> {
    if vendor_id != CH_VENDOR_ID {
        return None;
    }
    DEVICE_TABLE
        .iter()
        .find(|e| e.pid == product_id)
        .map(|e| e.device)
}

/// Return all known CH Products devices.
pub fn all_devices() -> &'static [DeviceEntry] {
    DEVICE_TABLE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identify_fighterstick() {
        assert_eq!(
            identify_device(CH_VENDOR_ID, CH_FIGHTERSTICK_PID),
            Some(ChDevice::Fighterstick)
        );
    }

    #[test]
    fn identify_pro_throttle() {
        assert_eq!(
            identify_device(CH_VENDOR_ID, CH_PRO_THROTTLE_PID),
            Some(ChDevice::ProThrottle)
        );
    }

    #[test]
    fn identify_pro_pedals() {
        assert_eq!(
            identify_device(CH_VENDOR_ID, CH_PRO_PEDALS_PID),
            Some(ChDevice::ProPedals)
        );
    }

    #[test]
    fn identify_combatstick() {
        assert_eq!(
            identify_device(CH_VENDOR_ID, CH_COMBAT_STICK_PID),
            Some(ChDevice::CombatStick)
        );
    }

    #[test]
    fn identify_eclipse_yoke() {
        assert_eq!(
            identify_device(CH_VENDOR_ID, CH_ECLIPSE_YOKE_PID),
            Some(ChDevice::EclipseYoke)
        );
    }

    #[test]
    fn identify_flight_yoke() {
        assert_eq!(
            identify_device(CH_VENDOR_ID, CH_FLIGHT_YOKE_PID),
            Some(ChDevice::FlightYoke)
        );
    }

    #[test]
    fn wrong_vendor_returns_none() {
        assert_eq!(identify_device(0x1234, CH_FIGHTERSTICK_PID), None);
    }

    #[test]
    fn unknown_pid_returns_none() {
        assert_eq!(identify_device(CH_VENDOR_ID, 0xFFFF), None);
    }

    #[test]
    fn device_table_has_no_duplicate_pids() {
        let mut pids: Vec<u16> = DEVICE_TABLE.iter().map(|e| e.pid).collect();
        pids.sort();
        pids.dedup();
        assert_eq!(
            pids.len(),
            DEVICE_TABLE.len(),
            "duplicate PID in DEVICE_TABLE"
        );
    }

    #[test]
    fn all_devices_have_nonempty_names() {
        for entry in DEVICE_TABLE {
            assert!(
                !entry.device.name().is_empty(),
                "empty name for {:?}",
                entry.device
            );
        }
    }

    #[test]
    fn device_pid_method_matches_table() {
        for entry in DEVICE_TABLE {
            assert_eq!(
                entry.device.pid(),
                entry.pid,
                "{:?} pid() mismatch",
                entry.device
            );
        }
    }
}
