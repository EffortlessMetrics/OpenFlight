// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Saitek/Logitech HOTAS device identification and topology detection.
//!
//! Supports X52, X52 Pro, X55, and X56 HOTAS controllers.
//! See `docs/reference/hotas-claims.md` for protocol verification status.

use crate::device_support::{
    LOGITECH_VENDOR_ID, MAD_CATZ_VENDOR_ID, SAITEK_VENDOR_ID, X52_PID, X52_PRO_PID, X52_V1_PID,
    X55_STICK_PID, X55_THROTTLE_PID, X56_LOGITECH_STICK_PID, X56_MADCATZ_STICK_PID,
    X56_MADCATZ_THROTTLE_PID, X65F_PID,
};

/// Saitek/Logitech HOTAS device types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaitekHotasType {
    /// X52 Flight Control System
    X52,
    /// X52 Pro Flight Control System
    X52Pro,
    /// X65F F-22 Raptor HOTAS
    X65F,
    /// X55 Rhino H.O.T.A.S. - Stick component
    X55Stick,
    /// X55 Rhino H.O.T.A.S. - Throttle component
    X55Throttle,
    /// X56 Rhino H.O.T.A.S. - Stick component (Saitek or Logitech branded)
    X56Stick,
    /// X56 Rhino H.O.T.A.S. - Throttle component (Saitek or Logitech branded)
    X56Throttle,
}

impl SaitekHotasType {
    /// Identify device type from USB VID/PID.
    ///
    /// Returns `None` if the VID/PID combination is not a known Saitek HOTAS.
    ///
    /// # Note
    /// Logitech X56 Throttle PID is intentionally NOT matched until verified.
    /// See `docs/reference/hotas-claims.md` for the PID collision warning.
    pub fn from_vid_pid(vid: u16, pid: u16) -> Option<Self> {
        // Check Logitech VID (X56 stick only - throttle PID is suspect)
        if vid == LOGITECH_VENDOR_ID {
            return match pid {
                X56_LOGITECH_STICK_PID => Some(Self::X56Stick),
                // NOTE: We intentionally do NOT match Logitech throttle PID (0xC22A)
                // because it may conflict with Logitech G110 keyboard.
                // Requires lsusb verification from real hardware.
                _ => None,
            };
        }

        // Check Mad Catz VID (X55/X56 era)
        if vid == MAD_CATZ_VENDOR_ID {
            return match pid {
                X55_STICK_PID => Some(Self::X55Stick),
                X55_THROTTLE_PID => Some(Self::X55Throttle),
                X56_MADCATZ_STICK_PID => Some(Self::X56Stick),
                X56_MADCATZ_THROTTLE_PID => Some(Self::X56Throttle),
                _ => None,
            };
        }

        // Check Saitek VID (original devices)
        if vid == SAITEK_VENDOR_ID {
            return match pid {
                X52_V1_PID | X52_PID => Some(Self::X52),
                X52_PRO_PID => Some(Self::X52Pro),
                X65F_PID => Some(Self::X65F),
                // X55 may also appear under Saitek VID on some units
                X55_STICK_PID => Some(Self::X55Stick),
                X55_THROTTLE_PID => Some(Self::X55Throttle),
                _ => None,
            };
        }

        None
    }

    /// Returns `true` if this device uses unified USB topology (stick + throttle on one cable).
    pub fn is_unified_topology(&self) -> bool {
        matches!(self, Self::X52 | Self::X52Pro | Self::X65F)
    }

    /// Returns `true` if this device uses split USB topology (separate cables).
    pub fn is_split_topology(&self) -> bool {
        !self.is_unified_topology()
    }

    /// Returns `true` if this is a stick component.
    pub fn is_stick(&self) -> bool {
        matches!(
            self,
            Self::X52 | Self::X52Pro | Self::X65F | Self::X55Stick | Self::X56Stick
        )
    }

    /// Returns `true` if this is a throttle component.
    pub fn is_throttle(&self) -> bool {
        matches!(self, Self::X55Throttle | Self::X56Throttle)
    }

    /// Returns `true` if this device has MFD display capability.
    ///
    /// Note: MFD protocol is currently unverified. See `docs/reference/hotas-claims.md`.
    pub fn has_mfd(&self) -> bool {
        matches!(self, Self::X52Pro)
    }

    /// Returns `true` if this device has LED indicators.
    pub fn has_leds(&self) -> bool {
        matches!(self, Self::X52 | Self::X52Pro | Self::X65F)
    }

    /// Returns `true` if this device has RGB lighting.
    ///
    /// Note: RGB protocol is currently unverified. See `docs/reference/hotas-claims.md`.
    pub fn has_rgb(&self) -> bool {
        matches!(self, Self::X56Stick | Self::X56Throttle)
    }

    /// Human-readable device name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::X52 => "Saitek X52 Flight Control System",
            Self::X52Pro => "Saitek X52 Pro Flight Control System",
            Self::X65F => "Saitek X65F F-22 Raptor HOTAS",
            Self::X55Stick => "Saitek X55 Rhino Stick",
            Self::X55Throttle => "Saitek X55 Rhino Throttle",
            Self::X56Stick => "Saitek/Logitech X56 Rhino Stick",
            Self::X56Throttle => "Saitek/Logitech X56 Rhino Throttle",
        }
    }

    /// Short identifier for logging/display.
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::X52 => "X52",
            Self::X52Pro => "X52 Pro",
            Self::X65F => "X65F",
            Self::X55Stick => "X55 Stick",
            Self::X55Throttle => "X55 Throttle",
            Self::X56Stick => "X56 Stick",
            Self::X56Throttle => "X56 Throttle",
        }
    }

    /// Returns the device family for grouping related devices.
    pub fn family(&self) -> SaitekHotasFamily {
        match self {
            Self::X52 | Self::X52Pro => SaitekHotasFamily::X52,
            Self::X65F => SaitekHotasFamily::X65,
            Self::X55Stick | Self::X55Throttle => SaitekHotasFamily::X55,
            Self::X56Stick | Self::X56Throttle => SaitekHotasFamily::X56,
        }
    }
}

/// Device family grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaitekHotasFamily {
    /// X52 and X52 Pro
    X52,
    /// X65F F-22 Raptor
    X65,
    /// X55 Rhino
    X55,
    /// X56 Rhino
    X56,
}

impl SaitekHotasFamily {
    /// Human-readable family name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::X52 => "X52 Series",
            Self::X65 => "X65F",
            Self::X55 => "X55 Rhino",
            Self::X56 => "X56 Rhino",
        }
    }
}

/// Check if a VID/PID pair is a Saitek HOTAS device.
pub fn is_saitek_hotas(vid: u16, pid: u16) -> bool {
    SaitekHotasType::from_vid_pid(vid, pid).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x52_detection() {
        assert_eq!(
            SaitekHotasType::from_vid_pid(SAITEK_VENDOR_ID, X52_PID),
            Some(SaitekHotasType::X52)
        );
        assert!(SaitekHotasType::X52.is_unified_topology());
        assert!(SaitekHotasType::X52.is_stick());
        assert!(!SaitekHotasType::X52.is_throttle());
    }

    #[test]
    fn test_x52_v1_detection() {
        assert_eq!(
            SaitekHotasType::from_vid_pid(SAITEK_VENDOR_ID, X52_V1_PID),
            Some(SaitekHotasType::X52)
        );
    }

    #[test]
    fn test_x52_pro_detection() {
        assert_eq!(
            SaitekHotasType::from_vid_pid(SAITEK_VENDOR_ID, X52_PRO_PID),
            Some(SaitekHotasType::X52Pro)
        );
        assert!(SaitekHotasType::X52Pro.has_mfd());
        assert!(SaitekHotasType::X52Pro.has_leds());
    }

    #[test]
    fn test_x55_split_topology() {
        // X55 under Saitek VID
        assert_eq!(
            SaitekHotasType::from_vid_pid(SAITEK_VENDOR_ID, X55_STICK_PID),
            Some(SaitekHotasType::X55Stick)
        );
        assert_eq!(
            SaitekHotasType::from_vid_pid(SAITEK_VENDOR_ID, X55_THROTTLE_PID),
            Some(SaitekHotasType::X55Throttle)
        );
        // X55 under Mad Catz VID (some units were shipped this way)
        assert_eq!(
            SaitekHotasType::from_vid_pid(MAD_CATZ_VENDOR_ID, X55_STICK_PID),
            Some(SaitekHotasType::X55Stick)
        );
        assert_eq!(
            SaitekHotasType::from_vid_pid(MAD_CATZ_VENDOR_ID, X55_THROTTLE_PID),
            Some(SaitekHotasType::X55Throttle)
        );
        assert!(SaitekHotasType::X55Stick.is_split_topology());
        assert!(SaitekHotasType::X55Throttle.is_split_topology());
    }

    #[test]
    fn test_x56_logitech_detection() {
        // Logitech stick is matched
        assert_eq!(
            SaitekHotasType::from_vid_pid(LOGITECH_VENDOR_ID, X56_LOGITECH_STICK_PID),
            Some(SaitekHotasType::X56Stick)
        );
        // Logitech throttle is intentionally NOT matched due to PID collision risk
        // See docs/reference/hotas-claims.md
        assert_eq!(
            SaitekHotasType::from_vid_pid(LOGITECH_VENDOR_ID, 0xC22A),
            None
        );
        assert!(SaitekHotasType::X56Stick.has_rgb());
    }

    #[test]
    fn test_x56_madcatz_detection() {
        // Mad Catz X56 uses VID 0x0738 with PIDs 0x2221/0xA221
        assert_eq!(
            SaitekHotasType::from_vid_pid(MAD_CATZ_VENDOR_ID, X56_MADCATZ_STICK_PID),
            Some(SaitekHotasType::X56Stick)
        );
        assert_eq!(
            SaitekHotasType::from_vid_pid(MAD_CATZ_VENDOR_ID, X56_MADCATZ_THROTTLE_PID),
            Some(SaitekHotasType::X56Throttle)
        );
    }

    #[test]
    fn test_unknown_device() {
        assert_eq!(SaitekHotasType::from_vid_pid(0x1234, 0x5678), None);
        assert_eq!(
            SaitekHotasType::from_vid_pid(SAITEK_VENDOR_ID, 0x9999),
            None
        );
    }

    #[test]
    fn test_is_saitek_hotas() {
        assert!(is_saitek_hotas(SAITEK_VENDOR_ID, X52_PID));
        assert!(is_saitek_hotas(LOGITECH_VENDOR_ID, X56_LOGITECH_STICK_PID));
        assert!(!is_saitek_hotas(0x1234, 0x5678));
    }

    #[test]
    fn test_family_grouping() {
        assert_eq!(SaitekHotasType::X52.family(), SaitekHotasFamily::X52);
        assert_eq!(SaitekHotasType::X52Pro.family(), SaitekHotasFamily::X52);
        assert_eq!(SaitekHotasType::X55Stick.family(), SaitekHotasFamily::X55);
        assert_eq!(
            SaitekHotasType::X56Throttle.family(),
            SaitekHotasFamily::X56
        );
    }
}
