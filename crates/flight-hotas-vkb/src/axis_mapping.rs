// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! VKB device-family-specific axis mapping and resolution.
//!
//! This module provides axis maps and a unified `resolve_axis` function
//! that extracts and normalises a named axis from a raw HID report payload
//! based on the device family.

use crate::profiles::{AxisNormMode, gladiator_nxt_evo_profile, gunfighter_mcg_profile};
use crate::protocol::VkbDeviceFamily;

// ─── VKB axis identifiers ────────────────────────────────────────────────────

/// Named axis identifiers for VKB joystick-class devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VkbAxis {
    /// Main stick roll (X).
    Roll,
    /// Main stick pitch (Y).
    Pitch,
    /// Stick twist / yaw (Z).
    Yaw,
    /// Throttle wheel or slider.
    Throttle,
    /// Mini-stick analogue X.
    MiniX,
    /// Mini-stick analogue Y.
    MiniY,
}

impl VkbAxis {
    /// Return the profile axis name matching this identifier.
    #[cfg(test)]
    fn profile_name(self) -> &'static str {
        match self {
            Self::Roll => "roll",
            Self::Pitch => "pitch",
            Self::Yaw => "yaw",
            Self::Throttle => "throttle",
            Self::MiniX => "mini_x",
            Self::MiniY => "mini_y",
        }
    }
}

// ─── Axis map entry ──────────────────────────────────────────────────────────

/// One entry in a device-specific axis map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisMapEntry {
    /// Which axis this entry resolves.
    pub axis: VkbAxis,
    /// Byte offset into the HID report payload (after report ID).
    pub offset: usize,
    /// Normalisation mode.
    pub mode: AxisNormMode,
}

// ─── Gladiator axis map ──────────────────────────────────────────────────────

/// Axis map for the VKB Gladiator NXT EVO family.
///
/// 6 axes: roll, pitch, yaw, mini_x, mini_y, throttle.
/// All at 16-bit resolution; bidirectional axes use signed normalisation.
pub static GLADIATOR_AXIS_MAP: [AxisMapEntry; 6] = [
    AxisMapEntry {
        axis: VkbAxis::Roll,
        offset: 0,
        mode: AxisNormMode::Signed,
    },
    AxisMapEntry {
        axis: VkbAxis::Pitch,
        offset: 2,
        mode: AxisNormMode::Signed,
    },
    AxisMapEntry {
        axis: VkbAxis::Yaw,
        offset: 4,
        mode: AxisNormMode::Signed,
    },
    AxisMapEntry {
        axis: VkbAxis::MiniX,
        offset: 6,
        mode: AxisNormMode::Signed,
    },
    AxisMapEntry {
        axis: VkbAxis::MiniY,
        offset: 8,
        mode: AxisNormMode::Signed,
    },
    AxisMapEntry {
        axis: VkbAxis::Throttle,
        offset: 10,
        mode: AxisNormMode::Unsigned,
    },
];

// ─── Gunfighter axis map ─────────────────────────────────────────────────────

/// Axis map for VKB Gunfighter-class devices (all cam configurations).
///
/// Shares the same 6-axis layout as the Gladiator NXT EVO.  Cam selection
/// affects mechanical feel but not the HID report offsets.
pub static GUNFIGHTER_AXIS_MAP: [AxisMapEntry; 6] = [
    AxisMapEntry {
        axis: VkbAxis::Roll,
        offset: 0,
        mode: AxisNormMode::Signed,
    },
    AxisMapEntry {
        axis: VkbAxis::Pitch,
        offset: 2,
        mode: AxisNormMode::Signed,
    },
    AxisMapEntry {
        axis: VkbAxis::Yaw,
        offset: 4,
        mode: AxisNormMode::Signed,
    },
    AxisMapEntry {
        axis: VkbAxis::MiniX,
        offset: 6,
        mode: AxisNormMode::Signed,
    },
    AxisMapEntry {
        axis: VkbAxis::MiniY,
        offset: 8,
        mode: AxisNormMode::Signed,
    },
    AxisMapEntry {
        axis: VkbAxis::Throttle,
        offset: 10,
        mode: AxisNormMode::Unsigned,
    },
];

// ─── Map lookup ──────────────────────────────────────────────────────────────

/// Return the axis map for a given device family.
///
/// Returns `None` for families that are not joystick-class (e.g. SEM THQ)
/// because their axis layout differs from the standard stick model.
pub fn axis_map_for_family(family: VkbDeviceFamily) -> Option<&'static [AxisMapEntry]> {
    match family {
        VkbDeviceFamily::GladiatorNxtEvo | VkbDeviceFamily::GladiatorNxtEvoSem => {
            Some(&GLADIATOR_AXIS_MAP)
        }
        VkbDeviceFamily::Gunfighter | VkbDeviceFamily::GladiatorMcp => Some(&GUNFIGHTER_AXIS_MAP),
        // SEM THQ, Gladiator Mk2, etc. have different axis models.
        _ => None,
    }
}

// ─── Axis resolution ─────────────────────────────────────────────────────────

/// Error returned by [`resolve_axis`] when the axis cannot be resolved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AxisResolveError {
    /// No axis map is available for this device family.
    #[error("no axis map for device family")]
    UnsupportedFamily,
    /// The requested axis is not present in this device's axis map.
    #[error("axis not found in device map")]
    AxisNotFound,
    /// Report payload too short to read the axis at the expected offset.
    #[error("report too short: need at least {needed} bytes, got {actual}")]
    ReportTooShort {
        /// Minimum bytes needed.
        needed: usize,
        /// Actual payload length.
        actual: usize,
    },
}

/// Resolve a named axis from a raw HID report payload.
///
/// Reads a 16-bit little-endian value at the axis's report offset and
/// normalises it according to the axis's mode:
/// - [`AxisNormMode::Signed`]: `0x0000` → `−1.0`, `0x8000` → `0.0`, `0xFFFF` → `≈1.0`
/// - [`AxisNormMode::Unsigned`]: `0x0000` → `0.0`, `0xFFFF` → `1.0`
///
/// The `report` slice must be the HID payload **after** any report ID byte
/// has been stripped.
pub fn resolve_axis(
    family: VkbDeviceFamily,
    report: &[u8],
    axis: VkbAxis,
) -> Result<f64, AxisResolveError> {
    let map = axis_map_for_family(family).ok_or(AxisResolveError::UnsupportedFamily)?;

    let entry = map
        .iter()
        .find(|e| e.axis == axis)
        .ok_or(AxisResolveError::AxisNotFound)?;

    let needed = entry.offset + 2;
    if report.len() < needed {
        return Err(AxisResolveError::ReportTooShort {
            needed,
            actual: report.len(),
        });
    }

    let raw = u16::from_le_bytes([report[entry.offset], report[entry.offset + 1]]);

    let normalised = match entry.mode {
        AxisNormMode::Signed => ((raw as f64 / 32767.5) - 1.0).clamp(-1.0, 1.0),
        AxisNormMode::Unsigned => (raw as f64 / u16::MAX as f64).clamp(0.0, 1.0),
    };

    Ok(normalised)
}

// ─── Profile-based lookup ────────────────────────────────────────────────────

/// Resolve an axis using profile metadata (name-based lookup).
///
/// This is a convenience wrapper that looks up the axis by its profile name
/// in the appropriate device profile and normalises the raw value.
pub fn resolve_axis_by_name(
    family: VkbDeviceFamily,
    report: &[u8],
    axis_name: &str,
) -> Option<f64> {
    let profile = match family {
        VkbDeviceFamily::GladiatorNxtEvo | VkbDeviceFamily::GladiatorNxtEvoSem => {
            gladiator_nxt_evo_profile()
        }
        VkbDeviceFamily::Gunfighter | VkbDeviceFamily::GladiatorMcp => gunfighter_mcg_profile(),
        _ => return None,
    };

    let axis = profile.axis_by_name(axis_name)?;
    let needed = axis.report_offset + 2;
    if report.len() < needed {
        return None;
    }

    let raw = u16::from_le_bytes([report[axis.report_offset], report[axis.report_offset + 1]]);

    let normalised = match axis.mode {
        AxisNormMode::Signed => ((raw as f64 / 32767.5) - 1.0).clamp(-1.0, 1.0),
        AxisNormMode::Unsigned => (raw as f64 / u16::MAX as f64).clamp(0.0, 1.0),
    };

    Some(normalised)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal joystick report payload (no report ID) with 6 u16 LE axes.
    fn make_payload(axes: [u16; 6]) -> Vec<u8> {
        let mut payload = Vec::with_capacity(12);
        for &v in &axes {
            payload.extend_from_slice(&v.to_le_bytes());
        }
        payload
    }

    // ─── resolve_axis ─────────────────────────────────────────────────────

    #[test]
    fn resolve_roll_centre() {
        let payload = make_payload([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000]);
        let val = resolve_axis(VkbDeviceFamily::GladiatorNxtEvo, &payload, VkbAxis::Roll).unwrap();
        assert!(val.abs() < 0.01, "centre roll should be ~0.0, got {val}");
    }

    #[test]
    fn resolve_roll_full_right() {
        let payload = make_payload([0xFFFF, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000]);
        let val = resolve_axis(VkbDeviceFamily::GladiatorNxtEvo, &payload, VkbAxis::Roll).unwrap();
        assert!(
            (val - 1.0).abs() < 0.01,
            "full right roll should be ~1.0, got {val}"
        );
    }

    #[test]
    fn resolve_roll_full_left() {
        let payload = make_payload([0x0000, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000]);
        let val = resolve_axis(VkbDeviceFamily::GladiatorNxtEvo, &payload, VkbAxis::Roll).unwrap();
        assert!(
            (val - (-1.0)).abs() < 0.01,
            "full left roll should be ~-1.0, got {val}"
        );
    }

    #[test]
    fn resolve_throttle_zero() {
        let payload = make_payload([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000]);
        let val = resolve_axis(
            VkbDeviceFamily::GladiatorNxtEvo,
            &payload,
            VkbAxis::Throttle,
        )
        .unwrap();
        assert!(val.abs() < 1e-5, "zero throttle should be 0.0, got {val}");
    }

    #[test]
    fn resolve_throttle_full() {
        let payload = make_payload([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0xFFFF]);
        let val = resolve_axis(
            VkbDeviceFamily::GladiatorNxtEvo,
            &payload,
            VkbAxis::Throttle,
        )
        .unwrap();
        assert!(
            (val - 1.0).abs() < 1e-4,
            "full throttle should be 1.0, got {val}"
        );
    }

    #[test]
    fn resolve_gunfighter_roll() {
        let payload = make_payload([0xFFFF, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000]);
        let val = resolve_axis(VkbDeviceFamily::Gunfighter, &payload, VkbAxis::Roll).unwrap();
        assert!((val - 1.0).abs() < 0.01);
    }

    #[test]
    fn resolve_gunfighter_mcp_uses_gunfighter_map() {
        let payload = make_payload([0x8000, 0x0000, 0x8000, 0x8000, 0x8000, 0x0000]);
        let val = resolve_axis(VkbDeviceFamily::GladiatorMcp, &payload, VkbAxis::Pitch).unwrap();
        assert!((val - (-1.0)).abs() < 0.01);
    }

    #[test]
    fn resolve_nxt_evo_sem_uses_gladiator_map() {
        let payload = make_payload([0x8000, 0x8000, 0xFFFF, 0x8000, 0x8000, 0x0000]);
        let val =
            resolve_axis(VkbDeviceFamily::GladiatorNxtEvoSem, &payload, VkbAxis::Yaw).unwrap();
        assert!((val - 1.0).abs() < 0.01);
    }

    #[test]
    fn resolve_unsupported_family() {
        let payload = make_payload([0x8000; 6]);
        let err = resolve_axis(VkbDeviceFamily::SemThq, &payload, VkbAxis::Roll);
        assert!(matches!(err, Err(AxisResolveError::UnsupportedFamily)));
    }

    #[test]
    fn resolve_report_too_short() {
        let payload = [0u8; 4]; // only 4 bytes, not enough for any axis past offset 2
        let err = resolve_axis(
            VkbDeviceFamily::GladiatorNxtEvo,
            &payload,
            VkbAxis::Throttle,
        );
        assert!(matches!(err, Err(AxisResolveError::ReportTooShort { .. })));
    }

    // ─── resolve_axis_by_name ─────────────────────────────────────────────

    #[test]
    fn resolve_by_name_roll_gladiator() {
        let payload = make_payload([0xFFFF, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000]);
        let val = resolve_axis_by_name(VkbDeviceFamily::GladiatorNxtEvo, &payload, "roll").unwrap();
        assert!((val - 1.0).abs() < 0.01);
    }

    #[test]
    fn resolve_by_name_throttle_gunfighter() {
        let payload = make_payload([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0xFFFF]);
        let val = resolve_axis_by_name(VkbDeviceFamily::Gunfighter, &payload, "throttle").unwrap();
        assert!((val - 1.0).abs() < 1e-4);
    }

    #[test]
    fn resolve_by_name_unknown_axis() {
        let payload = make_payload([0x8000; 6]);
        let val = resolve_axis_by_name(VkbDeviceFamily::GladiatorNxtEvo, &payload, "nonexistent");
        assert!(val.is_none());
    }

    #[test]
    fn resolve_by_name_unsupported_family() {
        let payload = make_payload([0x8000; 6]);
        let val = resolve_axis_by_name(VkbDeviceFamily::GladiatorMk2, &payload, "roll");
        assert!(val.is_none());
    }

    #[test]
    fn resolve_by_name_report_too_short() {
        let payload = [0u8; 4];
        let val = resolve_axis_by_name(VkbDeviceFamily::GladiatorNxtEvo, &payload, "throttle");
        assert!(val.is_none());
    }

    // ─── axis map lookup ──────────────────────────────────────────────────

    #[test]
    fn axis_map_gladiator_has_six_entries() {
        let map = axis_map_for_family(VkbDeviceFamily::GladiatorNxtEvo).unwrap();
        assert_eq!(map.len(), 6);
    }

    #[test]
    fn axis_map_gunfighter_has_six_entries() {
        let map = axis_map_for_family(VkbDeviceFamily::Gunfighter).unwrap();
        assert_eq!(map.len(), 6);
    }

    #[test]
    fn axis_map_sem_thq_returns_none() {
        assert!(axis_map_for_family(VkbDeviceFamily::SemThq).is_none());
    }

    #[test]
    fn axis_map_gladiator_mk2_returns_none() {
        assert!(axis_map_for_family(VkbDeviceFamily::GladiatorMk2).is_none());
    }

    // ─── VkbAxis profile name mapping ─────────────────────────────────────

    #[test]
    fn axis_profile_names() {
        assert_eq!(VkbAxis::Roll.profile_name(), "roll");
        assert_eq!(VkbAxis::Pitch.profile_name(), "pitch");
        assert_eq!(VkbAxis::Yaw.profile_name(), "yaw");
        assert_eq!(VkbAxis::Throttle.profile_name(), "throttle");
        assert_eq!(VkbAxis::MiniX.profile_name(), "mini_x");
        assert_eq!(VkbAxis::MiniY.profile_name(), "mini_y");
    }

    // ─── Consistency with profiles ────────────────────────────────────────

    #[test]
    fn gladiator_axis_map_matches_profile() {
        let profile = gladiator_nxt_evo_profile();
        for entry in &GLADIATOR_AXIS_MAP {
            let profile_axis = profile
                .axis_by_name(entry.axis.profile_name())
                .unwrap_or_else(|| panic!("missing profile axis: {}", entry.axis.profile_name()));
            assert_eq!(
                entry.offset,
                profile_axis.report_offset,
                "offset mismatch for {}",
                entry.axis.profile_name()
            );
            assert_eq!(
                entry.mode,
                profile_axis.mode,
                "mode mismatch for {}",
                entry.axis.profile_name()
            );
        }
    }

    #[test]
    fn gunfighter_axis_map_matches_profile() {
        let profile = gunfighter_mcg_profile();
        for entry in &GUNFIGHTER_AXIS_MAP {
            let profile_axis = profile
                .axis_by_name(entry.axis.profile_name())
                .unwrap_or_else(|| panic!("missing profile axis: {}", entry.axis.profile_name()));
            assert_eq!(
                entry.offset,
                profile_axis.report_offset,
                "offset mismatch for {}",
                entry.axis.profile_name()
            );
            assert_eq!(
                entry.mode,
                profile_axis.mode,
                "mode mismatch for {}",
                entry.axis.profile_name()
            );
        }
    }
}
