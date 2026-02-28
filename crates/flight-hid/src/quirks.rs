// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Device quirks database.
//!
//! Some HID devices have known hardware defects or non-standard behaviour that
//! must be compensated for in software. [`QuirksDatabase`] provides a VID/PID
//! lookup table of [`DeviceQuirk`] entries that describe these anomalies.

use std::collections::HashMap;

// ── DeviceQuirk ──────────────────────────────────────────────────────────

/// A single hardware quirk that requires software compensation.
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceQuirk {
    /// An axis reports inverted values (min ↔ max).
    InvertedAxis { axis_name: String },
    /// An axis whose physical center is not at the electrical midpoint.
    OffsetCenter { axis_name: String, offset: f32 },
    /// An axis that has inherent electrical noise below `noise_floor`.
    NoisyAxis { axis_name: String, noise_floor: f32 },
    /// The device needs a warmup delay before it reports reliable data.
    SlowInit { delay_ms: u32 },
}

impl DeviceQuirk {
    /// Convenience constructor for an inverted axis quirk.
    pub fn inverted_axis(axis_name: impl Into<String>) -> Self {
        Self::InvertedAxis {
            axis_name: axis_name.into(),
        }
    }

    /// Convenience constructor for an offset-center quirk.
    pub fn offset_center(axis_name: impl Into<String>, offset: f32) -> Self {
        Self::OffsetCenter {
            axis_name: axis_name.into(),
            offset,
        }
    }

    /// Convenience constructor for a noisy-axis quirk.
    pub fn noisy_axis(axis_name: impl Into<String>, noise_floor: f32) -> Self {
        Self::NoisyAxis {
            axis_name: axis_name.into(),
            noise_floor,
        }
    }

    /// Convenience constructor for a slow-init quirk.
    pub fn slow_init(delay_ms: u32) -> Self {
        Self::SlowInit { delay_ms }
    }
}

// ── QuirksDatabase ───────────────────────────────────────────────────────

/// VID/PID → quirks lookup table.
///
/// Ships with embedded data for commonly-used flight simulation controllers.
/// Additional entries can be registered at runtime.
pub struct QuirksDatabase {
    entries: HashMap<(u16, u16), Vec<DeviceQuirk>>,
}

impl QuirksDatabase {
    /// Create an empty database.
    pub fn empty() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Create the database pre-populated with known flight-sim device quirks.
    pub fn with_defaults() -> Self {
        let mut db = Self::empty();
        db.populate_defaults();
        db
    }

    /// Look up quirks for a VID/PID pair.
    pub fn get_quirks(&self, vid: u16, pid: u16) -> Vec<DeviceQuirk> {
        self.entries.get(&(vid, pid)).cloned().unwrap_or_default()
    }

    /// Register one or more quirks for a device.
    pub fn add_quirks(&mut self, vid: u16, pid: u16, quirks: Vec<DeviceQuirk>) {
        self.entries.entry((vid, pid)).or_default().extend(quirks);
    }

    /// Register a single quirk for a device.
    pub fn add_quirk(&mut self, vid: u16, pid: u16, quirk: DeviceQuirk) {
        self.entries.entry((vid, pid)).or_default().push(quirk);
    }

    /// Returns `true` if there are any quirks for the given VID/PID.
    pub fn has_quirks(&self, vid: u16, pid: u16) -> bool {
        self.entries.get(&(vid, pid)).is_some_and(|v| !v.is_empty())
    }

    /// Number of VID/PID entries in the database.
    pub fn device_count(&self) -> usize {
        self.entries.len()
    }

    /// Total number of quirk entries across all devices.
    pub fn quirk_count(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }

    /// Populate built-in quirks for well-known flight simulation hardware.
    fn populate_defaults(&mut self) {
        // Thrustmaster T16000M — known for noisy X/Y axes
        self.add_quirks(
            0x044F,
            0xB10A,
            vec![
                DeviceQuirk::noisy_axis("X", 0.02),
                DeviceQuirk::noisy_axis("Y", 0.02),
                DeviceQuirk::noisy_axis("Rz", 0.03),
            ],
        );

        // Thrustmaster TWCS Throttle — slider noise + slow USB init
        self.add_quirks(
            0x044F,
            0xB687,
            vec![
                DeviceQuirk::noisy_axis("Slider", 0.015),
                DeviceQuirk::slow_init(500),
            ],
        );

        // CH Products Pro Throttle — known inverted ministick Y
        self.add_quirk(0x068E, 0x00F4, DeviceQuirk::inverted_axis("ministick_y"));

        // Saitek X52 Pro — slight center offset on X
        self.add_quirk(0x06A3, 0x0762, DeviceQuirk::offset_center("X", 0.02));

        // VKB Gladiator NXT — clean device, slow USB enumeration
        self.add_quirk(0x231D, 0x0200, DeviceQuirk::slow_init(300));

        // Logitech Extreme 3D Pro — noisy twist axis
        self.add_quirk(0x046D, 0xC215, DeviceQuirk::noisy_axis("Rz", 0.04));
    }
}

impl Default for QuirksDatabase {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_db_returns_no_quirks() {
        let db = QuirksDatabase::empty();
        let quirks = db.get_quirks(0x044F, 0xB10A);
        assert!(quirks.is_empty());
    }

    #[test]
    fn defaults_contain_t16000m() {
        let db = QuirksDatabase::with_defaults();
        let quirks = db.get_quirks(0x044F, 0xB10A);
        assert!(!quirks.is_empty());
        assert!(
            quirks
                .iter()
                .any(|q| matches!(q, DeviceQuirk::NoisyAxis { axis_name, .. } if axis_name == "X"))
        );
    }

    #[test]
    fn defaults_contain_twcs_throttle() {
        let db = QuirksDatabase::with_defaults();
        let quirks = db.get_quirks(0x044F, 0xB687);
        assert!(
            quirks
                .iter()
                .any(|q| matches!(q, DeviceQuirk::SlowInit { delay_ms: 500 }))
        );
    }

    #[test]
    fn defaults_contain_ch_pro_throttle() {
        let db = QuirksDatabase::with_defaults();
        let quirks = db.get_quirks(0x068E, 0x00F4);
        assert!(
            quirks
                .iter()
                .any(|q| matches!(q, DeviceQuirk::InvertedAxis { .. }))
        );
    }

    #[test]
    fn defaults_contain_x52_pro() {
        let db = QuirksDatabase::with_defaults();
        let quirks = db.get_quirks(0x06A3, 0x0762);
        assert!(
            quirks
                .iter()
                .any(|q| matches!(q, DeviceQuirk::OffsetCenter { .. }))
        );
    }

    #[test]
    fn defaults_contain_extreme_3d_pro() {
        let db = QuirksDatabase::with_defaults();
        let quirks = db.get_quirks(0x046D, 0xC215);
        assert!(!quirks.is_empty());
    }

    #[test]
    fn unknown_device_returns_empty() {
        let db = QuirksDatabase::with_defaults();
        assert!(db.get_quirks(0xFFFF, 0xFFFF).is_empty());
    }

    #[test]
    fn add_quirk_single() {
        let mut db = QuirksDatabase::empty();
        db.add_quirk(0x1234, 0x5678, DeviceQuirk::slow_init(200));
        let quirks = db.get_quirks(0x1234, 0x5678);
        assert_eq!(quirks.len(), 1);
        assert_eq!(quirks[0], DeviceQuirk::SlowInit { delay_ms: 200 });
    }

    #[test]
    fn add_quirks_multiple() {
        let mut db = QuirksDatabase::empty();
        db.add_quirks(
            0x1234,
            0x5678,
            vec![
                DeviceQuirk::inverted_axis("X"),
                DeviceQuirk::noisy_axis("Y", 0.05),
            ],
        );
        assert_eq!(db.get_quirks(0x1234, 0x5678).len(), 2);
    }

    #[test]
    fn add_quirk_appends() {
        let mut db = QuirksDatabase::empty();
        db.add_quirk(0x1234, 0x5678, DeviceQuirk::slow_init(100));
        db.add_quirk(0x1234, 0x5678, DeviceQuirk::inverted_axis("X"));
        assert_eq!(db.get_quirks(0x1234, 0x5678).len(), 2);
    }

    #[test]
    fn has_quirks_true() {
        let mut db = QuirksDatabase::empty();
        db.add_quirk(0x1234, 0x5678, DeviceQuirk::slow_init(100));
        assert!(db.has_quirks(0x1234, 0x5678));
    }

    #[test]
    fn has_quirks_false() {
        let db = QuirksDatabase::empty();
        assert!(!db.has_quirks(0x1234, 0x5678));
    }

    #[test]
    fn device_count() {
        let mut db = QuirksDatabase::empty();
        db.add_quirk(0x0001, 0x0001, DeviceQuirk::slow_init(100));
        db.add_quirk(0x0002, 0x0002, DeviceQuirk::slow_init(200));
        assert_eq!(db.device_count(), 2);
    }

    #[test]
    fn quirk_count() {
        let mut db = QuirksDatabase::empty();
        db.add_quirks(
            0x0001,
            0x0001,
            vec![DeviceQuirk::slow_init(100), DeviceQuirk::inverted_axis("X")],
        );
        db.add_quirk(0x0002, 0x0002, DeviceQuirk::noisy_axis("Y", 0.01));
        assert_eq!(db.quirk_count(), 3);
    }

    #[test]
    fn default_is_populated() {
        let db = QuirksDatabase::default();
        assert!(db.device_count() > 0);
        assert!(db.quirk_count() > 0);
    }

    #[test]
    fn convenience_constructors() {
        assert_eq!(
            DeviceQuirk::inverted_axis("X"),
            DeviceQuirk::InvertedAxis {
                axis_name: "X".into()
            }
        );
        assert_eq!(
            DeviceQuirk::offset_center("Y", 0.05),
            DeviceQuirk::OffsetCenter {
                axis_name: "Y".into(),
                offset: 0.05
            }
        );
        assert_eq!(
            DeviceQuirk::noisy_axis("Rz", 0.03),
            DeviceQuirk::NoisyAxis {
                axis_name: "Rz".into(),
                noise_floor: 0.03
            }
        );
        assert_eq!(
            DeviceQuirk::slow_init(500),
            DeviceQuirk::SlowInit { delay_ms: 500 }
        );
    }

    #[test]
    fn quirk_clone() {
        let q = DeviceQuirk::noisy_axis("X", 0.01);
        let q2 = q.clone();
        assert_eq!(q, q2);
    }

    #[test]
    fn defaults_vkb_gladiator() {
        let db = QuirksDatabase::with_defaults();
        let quirks = db.get_quirks(0x231D, 0x0200);
        assert!(
            quirks
                .iter()
                .any(|q| matches!(q, DeviceQuirk::SlowInit { .. }))
        );
    }
}
