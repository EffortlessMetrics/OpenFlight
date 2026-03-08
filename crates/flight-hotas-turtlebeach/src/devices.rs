// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Turtle Beach VelocityOne device database.
//!
//! Catalogues all known Turtle Beach VelocityOne flight simulation peripherals
//! by USB VID/PID. VID `0x10F5` is Turtle Beach Corporation's registered USB
//! vendor identifier.
//!
//! # Device family
//!
//! | Model                    | PID      | Tier | Category       |
//! |--------------------------|----------|------|----------------|
//! | VelocityOne Flight       | `0x1050` | 2    | Yoke + Panel   |
//! | VelocityOne Rudder       | `0x1051` | 2    | Pedals         |
//! | VelocityOne Flightstick  | `0x1052` | 2    | Joystick       |
//! | VelocityOne Flight Pro   | `0x0210` | 3    | Premium Yoke   |
//! | VelocityOne Flight Univ. | `0x1073` | 3    | All-in-One     |
//! | VelocityOne Flight Yoke  | `0x3085` | 2    | Dedicated Yoke |
//!
//! PIDs are community-reported; verify with `lsusb` / USBView before relying
//! on them for production device matching.

/// USB Vendor ID for Turtle Beach Corporation.
pub const TURTLE_BEACH_VID: u16 = 0x10F5;

/// PID for VelocityOne Flight (Flightdeck) — yoke + throttle quadrant + panel.
pub const VELOCITYONE_FLIGHT_PID: u16 = 0x1050;

/// PID for VelocityOne Rudder pedals.
pub const VELOCITYONE_RUDDER_PID: u16 = 0x1051;

/// PID for VelocityOne Flightstick (standalone joystick).
pub const VELOCITYONE_FLIGHTSTICK_PID: u16 = 0x1052;

/// PID for VelocityOne Flight Universal (all-in-one control system).
pub const VELOCITYONE_FLIGHT_UNIVERSAL_PID: u16 = 0x1073;

/// PID for VelocityOne Flight Pro (premium yoke variant).
pub const VELOCITYONE_FLIGHT_PRO_PID: u16 = 0x0210;

/// PID for VelocityOne Flight Yoke (dedicated GA/airliner yoke).
pub const VELOCITYONE_FLIGHT_YOKE_PID: u16 = 0x3085;

/// All known VelocityOne product IDs.
pub const ALL_PIDS: &[u16] = &[
    VELOCITYONE_FLIGHT_PID,
    VELOCITYONE_RUDDER_PID,
    VELOCITYONE_FLIGHTSTICK_PID,
    VELOCITYONE_FLIGHT_UNIVERSAL_PID,
    VELOCITYONE_FLIGHT_PRO_PID,
    VELOCITYONE_FLIGHT_YOKE_PID,
];

/// VelocityOne device model identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VelocityOneDevice {
    /// VelocityOne Flight (Flightdeck) — yoke with 2 axes + rudder twist,
    /// throttle quadrant with 2 levers, trim wheel, landing gear lever with
    /// LED indicators, 7 toggle switches, display with mode selector.
    Flight,
    /// VelocityOne Flightstick — stick with 3 axes (X/Y/twist),
    /// throttle slider, 16 buttons, and hat switch.
    Flightstick,
    /// VelocityOne Rudder — 3-axis pedals (rudder + left/right toe brake).
    Rudder,
    /// VelocityOne Flight Pro — premium yoke with 4 axes.
    FlightPro,
    /// VelocityOne Flight Universal — all-in-one control system.
    FlightUniversal,
    /// VelocityOne Flight Yoke — dedicated GA/airliner yoke.
    FlightYoke,
}

impl VelocityOneDevice {
    /// Human-readable device name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Flight => "Turtle Beach VelocityOne Flight",
            Self::Flightstick => "Turtle Beach VelocityOne Flightstick",
            Self::Rudder => "Turtle Beach VelocityOne Rudder",
            Self::FlightPro => "Turtle Beach VelocityOne Flight Pro",
            Self::FlightUniversal => "Turtle Beach VelocityOne Flight Universal",
            Self::FlightYoke => "Turtle Beach VelocityOne Flight Yoke",
        }
    }

    /// Returns the USB Product ID for this device.
    pub fn product_id(self) -> u16 {
        match self {
            Self::Flight => VELOCITYONE_FLIGHT_PID,
            Self::Flightstick => VELOCITYONE_FLIGHTSTICK_PID,
            Self::Rudder => VELOCITYONE_RUDDER_PID,
            Self::FlightPro => VELOCITYONE_FLIGHT_PRO_PID,
            Self::FlightUniversal => VELOCITYONE_FLIGHT_UNIVERSAL_PID,
            Self::FlightYoke => VELOCITYONE_FLIGHT_YOKE_PID,
        }
    }

    /// Returns all known device variants.
    pub fn all() -> &'static [VelocityOneDevice] {
        &[
            Self::Flight,
            Self::Flightstick,
            Self::Rudder,
            Self::FlightPro,
            Self::FlightUniversal,
            Self::FlightYoke,
        ]
    }
}

impl std::fmt::Display for VelocityOneDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Capabilities descriptor for a VelocityOne device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCapabilities {
    /// Number of analog axes.
    pub axes: u8,
    /// Number of buttons.
    pub buttons: u8,
    /// Number of hat switches.
    pub hats: u8,
    /// Axis resolution in bits.
    pub resolution_bits: u8,
    /// Device has LED output support (gear indicators, etc.).
    pub has_leds: bool,
    /// Device has an integrated display.
    pub has_display: bool,
    /// Device has a trim wheel input.
    pub has_trim_wheel: bool,
    /// Device has a landing gear lever.
    pub has_gear_lever: bool,
    /// Number of toggle switches (0 if none).
    pub toggle_switch_count: u8,
}

/// Returns the capabilities for a given VelocityOne device.
pub fn capabilities(device: VelocityOneDevice) -> DeviceCapabilities {
    match device {
        VelocityOneDevice::Flight => DeviceCapabilities {
            axes: 6,
            buttons: 63,
            hats: 1,
            resolution_bits: 12,
            has_leds: true,
            has_display: true,
            has_trim_wheel: true,
            has_gear_lever: true,
            toggle_switch_count: 7,
        },
        VelocityOneDevice::Flightstick => DeviceCapabilities {
            axes: 4,
            buttons: 16,
            hats: 1,
            resolution_bits: 12,
            has_leds: false,
            has_display: false,
            has_trim_wheel: false,
            has_gear_lever: false,
            toggle_switch_count: 0,
        },
        VelocityOneDevice::Rudder => DeviceCapabilities {
            axes: 3,
            buttons: 0,
            hats: 0,
            resolution_bits: 12,
            has_leds: false,
            has_display: false,
            has_trim_wheel: false,
            has_gear_lever: false,
            toggle_switch_count: 0,
        },
        VelocityOneDevice::FlightPro => DeviceCapabilities {
            axes: 4,
            buttons: 18,
            hats: 1,
            resolution_bits: 12,
            has_leds: false,
            has_display: false,
            has_trim_wheel: false,
            has_gear_lever: false,
            toggle_switch_count: 0,
        },
        VelocityOneDevice::FlightUniversal => DeviceCapabilities {
            axes: 6,
            buttons: 32,
            hats: 1,
            resolution_bits: 12,
            has_leds: true,
            has_display: true,
            has_trim_wheel: true,
            has_gear_lever: true,
            toggle_switch_count: 6,
        },
        VelocityOneDevice::FlightYoke => DeviceCapabilities {
            axes: 6,
            buttons: 18,
            hats: 1,
            resolution_bits: 12,
            has_leds: false,
            has_display: false,
            has_trim_wheel: false,
            has_gear_lever: false,
            toggle_switch_count: 0,
        },
    }
}

/// Returns `true` if the VID/PID combination belongs to a known Turtle Beach
/// VelocityOne device.
pub fn is_turtle_beach_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == TURTLE_BEACH_VID && ALL_PIDS.contains(&product_id)
}

/// Identify a VelocityOne device model from its USB Product ID.
///
/// Returns `None` if the PID is not recognised.
pub fn identify_device(product_id: u16) -> Option<VelocityOneDevice> {
    match product_id {
        VELOCITYONE_FLIGHT_PID => Some(VelocityOneDevice::Flight),
        VELOCITYONE_FLIGHTSTICK_PID => Some(VelocityOneDevice::Flightstick),
        VELOCITYONE_RUDDER_PID => Some(VelocityOneDevice::Rudder),
        VELOCITYONE_FLIGHT_PRO_PID => Some(VelocityOneDevice::FlightPro),
        VELOCITYONE_FLIGHT_UNIVERSAL_PID => Some(VelocityOneDevice::FlightUniversal),
        VELOCITYONE_FLIGHT_YOKE_PID => Some(VelocityOneDevice::FlightYoke),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identify_all_known_devices() {
        assert_eq!(identify_device(0x1050), Some(VelocityOneDevice::Flight));
        assert_eq!(identify_device(0x1051), Some(VelocityOneDevice::Rudder));
        assert_eq!(
            identify_device(0x1052),
            Some(VelocityOneDevice::Flightstick)
        );
        assert_eq!(
            identify_device(0x1073),
            Some(VelocityOneDevice::FlightUniversal)
        );
        assert_eq!(identify_device(0x0210), Some(VelocityOneDevice::FlightPro));
        assert_eq!(identify_device(0x3085), Some(VelocityOneDevice::FlightYoke));
    }

    #[test]
    fn test_unknown_pid_returns_none() {
        assert_eq!(identify_device(0xFFFF), None);
        assert_eq!(identify_device(0x0000), None);
    }

    #[test]
    fn test_is_turtle_beach_device_known() {
        for pid in ALL_PIDS {
            assert!(
                is_turtle_beach_device(TURTLE_BEACH_VID, *pid),
                "PID 0x{pid:04X} should be recognised"
            );
        }
    }

    #[test]
    fn test_is_turtle_beach_device_wrong_vid() {
        assert!(!is_turtle_beach_device(0x1234, VELOCITYONE_FLIGHT_PID));
    }

    #[test]
    fn test_is_turtle_beach_device_unknown_pid() {
        assert!(!is_turtle_beach_device(TURTLE_BEACH_VID, 0xFFFF));
    }

    #[test]
    fn test_flight_has_full_capabilities() {
        let caps = capabilities(VelocityOneDevice::Flight);
        assert_eq!(caps.axes, 6);
        assert!(caps.has_leds);
        assert!(caps.has_display);
        assert!(caps.has_trim_wheel);
        assert!(caps.has_gear_lever);
        assert!(caps.toggle_switch_count >= 6);
    }

    #[test]
    fn test_flightstick_capabilities() {
        let caps = capabilities(VelocityOneDevice::Flightstick);
        assert_eq!(caps.axes, 4);
        assert_eq!(caps.hats, 1);
        assert!(!caps.has_leds);
        assert!(!caps.has_display);
        assert!(!caps.has_trim_wheel);
    }

    #[test]
    fn test_rudder_capabilities() {
        let caps = capabilities(VelocityOneDevice::Rudder);
        assert_eq!(caps.axes, 3);
        assert_eq!(caps.buttons, 0);
        assert_eq!(caps.hats, 0);
    }

    #[test]
    fn test_all_devices_have_12bit_resolution() {
        for device in VelocityOneDevice::all() {
            assert_eq!(capabilities(*device).resolution_bits, 12);
        }
    }

    #[test]
    fn test_device_names_non_empty() {
        for device in VelocityOneDevice::all() {
            assert!(!device.name().is_empty());
            assert!(device.name().contains("VelocityOne"));
        }
    }

    #[test]
    fn test_device_display_trait() {
        let s = format!("{}", VelocityOneDevice::Flight);
        assert_eq!(s, "Turtle Beach VelocityOne Flight");
    }

    #[test]
    fn test_all_devices_count() {
        assert_eq!(VelocityOneDevice::all().len(), 6);
        assert_eq!(ALL_PIDS.len(), 6);
    }

    #[test]
    fn test_product_id_roundtrip() {
        for device in VelocityOneDevice::all() {
            let pid = device.product_id();
            assert_eq!(
                identify_device(pid),
                Some(*device),
                "roundtrip failed for {:?}",
                device
            );
        }
    }
}
