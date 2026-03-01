// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! LED zone map and backlight helpers for WinWing panels.
//!
//! WinWing panels that support backlight control expose individual LEDs
//! (or groups) that can be addressed through the feature-report protocol
//! (see [`crate::protocol`]).  This module provides per-device LED zone
//! maps so callers can address LEDs by logical name rather than raw index.
//!
//! # Protocol overview
//!
//! LED control uses the **Backlight** command category (`0x02`) in the
//! WinWing feature-report protocol.  Each command targets a `panel_id`
//! (assigned per-device) and a 0-based `button_index` within that panel.
//!
//! Supported sub-commands:
//! - `SetSingle` (`0x01`): intensity 0–255 for one LED
//! - `SetSingleRgb` (`0x02`): R/G/B colour for one LED (RGB-capable panels)
//! - `SetAll` (`0x03`): uniform intensity for all LEDs
//! - `SetAllRgb` (`0x04`): uniform RGB colour for all LEDs
//!
//! # RGB support
//!
//! Some newer WinWing panels (e.g. Combat Ready Panel MkII) support per-LED
//! RGB colour.  Older panels only support single-channel intensity.  The
//! [`LedZone::rgb`] field indicates whether RGB addressing is available for
//! that zone.

/// A single addressable LED zone on a WinWing panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedZone {
    /// Human-readable label (e.g. "MASTER ARM", "APU START").
    pub name: &'static str,
    /// 0-based LED index used in backlight feature-report commands.
    pub index: u8,
    /// `true` if this zone supports RGB colour commands.
    pub rgb: bool,
}

/// Complete LED zone map for a specific WinWing device.
#[derive(Debug, Clone)]
pub struct LedZoneMap {
    /// Device name for display purposes.
    pub device: &'static str,
    /// Panel ID used in feature-report addressing.
    pub panel_id: u8,
    /// Ordered list of LED zones.
    pub zones: &'static [LedZone],
}

// ── Combat Ready Panel LED map ────────────────────────────────────────────────

/// LED zones for the WinWing F/A-18 Combat Ready Panel (PID 0xBE05).
///
/// The panel has 30 individually backlit push-buttons arranged in 3 rows of 10.
/// ASSUMED zone ordering — verify with hardware capture.
pub static COMBAT_READY_PANEL_LEDS: LedZoneMap = LedZoneMap {
    device: "WinWing Combat Ready Panel",
    panel_id: 0x10, // ASSUMED panel ID
    zones: &[
        LedZone {
            name: "MASTER ARM",
            index: 0,
            rgb: false,
        },
        LedZone {
            name: "STORES JETT",
            index: 1,
            rgb: false,
        },
        LedZone {
            name: "EMCON",
            index: 2,
            rgb: false,
        },
        LedZone {
            name: "A/A",
            index: 3,
            rgb: false,
        },
        LedZone {
            name: "A/G",
            index: 4,
            rgb: false,
        },
        LedZone {
            name: "NAV",
            index: 5,
            rgb: false,
        },
        LedZone {
            name: "GUN",
            index: 6,
            rgb: false,
        },
        LedZone {
            name: "MSL",
            index: 7,
            rgb: false,
        },
        LedZone {
            name: "CMBT",
            index: 8,
            rgb: false,
        },
        LedZone {
            name: "FUEL DUMP",
            index: 9,
            rgb: false,
        },
        // Row 2 (indices 10–19) and Row 3 (indices 20–29) follow the same
        // pattern but without confirmed label assignment.
        LedZone {
            name: "ROW2_01",
            index: 10,
            rgb: false,
        },
        LedZone {
            name: "ROW2_02",
            index: 11,
            rgb: false,
        },
        LedZone {
            name: "ROW2_03",
            index: 12,
            rgb: false,
        },
        LedZone {
            name: "ROW2_04",
            index: 13,
            rgb: false,
        },
        LedZone {
            name: "ROW2_05",
            index: 14,
            rgb: false,
        },
        LedZone {
            name: "ROW2_06",
            index: 15,
            rgb: false,
        },
        LedZone {
            name: "ROW2_07",
            index: 16,
            rgb: false,
        },
        LedZone {
            name: "ROW2_08",
            index: 17,
            rgb: false,
        },
        LedZone {
            name: "ROW2_09",
            index: 18,
            rgb: false,
        },
        LedZone {
            name: "ROW2_10",
            index: 19,
            rgb: false,
        },
        LedZone {
            name: "ROW3_01",
            index: 20,
            rgb: false,
        },
        LedZone {
            name: "ROW3_02",
            index: 21,
            rgb: false,
        },
        LedZone {
            name: "ROW3_03",
            index: 22,
            rgb: false,
        },
        LedZone {
            name: "ROW3_04",
            index: 23,
            rgb: false,
        },
        LedZone {
            name: "ROW3_05",
            index: 24,
            rgb: false,
        },
        LedZone {
            name: "ROW3_06",
            index: 25,
            rgb: false,
        },
        LedZone {
            name: "ROW3_07",
            index: 26,
            rgb: false,
        },
        LedZone {
            name: "ROW3_08",
            index: 27,
            rgb: false,
        },
        LedZone {
            name: "ROW3_09",
            index: 28,
            rgb: false,
        },
        LedZone {
            name: "ROW3_10",
            index: 29,
            rgb: false,
        },
    ],
};

/// Look up a LED zone by name within a zone map (case-insensitive).
pub fn find_zone_by_name<'a>(map: &'a LedZoneMap, name: &str) -> Option<&'a LedZone> {
    let upper = name.to_uppercase();
    map.zones.iter().find(|z| z.name.to_uppercase() == upper)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combat_ready_panel_led_count() {
        assert_eq!(COMBAT_READY_PANEL_LEDS.zones.len(), 30);
    }

    #[test]
    fn test_combat_ready_panel_indices_unique() {
        let indices: Vec<u8> = COMBAT_READY_PANEL_LEDS
            .zones
            .iter()
            .map(|z| z.index)
            .collect();
        let mut sorted = indices.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(indices.len(), sorted.len(), "LED indices must be unique");
    }

    #[test]
    fn test_combat_ready_panel_indices_sequential() {
        for (i, zone) in COMBAT_READY_PANEL_LEDS.zones.iter().enumerate() {
            assert_eq!(
                zone.index, i as u8,
                "zone {} has non-sequential index",
                zone.name
            );
        }
    }

    #[test]
    fn test_find_zone_by_name_found() {
        let zone = find_zone_by_name(&COMBAT_READY_PANEL_LEDS, "MASTER ARM");
        assert!(zone.is_some());
        assert_eq!(zone.unwrap().index, 0);
    }

    #[test]
    fn test_find_zone_by_name_not_found() {
        let zone = find_zone_by_name(&COMBAT_READY_PANEL_LEDS, "NONEXISTENT");
        assert!(zone.is_none());
    }

    #[test]
    fn test_led_zone_map_panel_id() {
        assert_eq!(COMBAT_READY_PANEL_LEDS.panel_id, 0x10);
    }
}
