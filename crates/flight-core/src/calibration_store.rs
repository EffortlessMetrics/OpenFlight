// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Persistent calibration storage.
//!
//! Saves and loads axis calibration data (min/max/center) to/from a TOML file
//! in the platform-appropriate configuration directory.

use std::collections::HashMap;
use std::path::Path;

/// Calibration data for a single axis on a single device.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AxisCalibration {
    pub axis_id: u8,
    pub raw_min: i32,
    pub raw_max: i32,
    pub raw_center: i32,
    /// When this calibration was last updated (ISO 8601 string).
    pub updated_at: String,
}

impl AxisCalibration {
    pub fn new(axis_id: u8, raw_min: i32, raw_max: i32, raw_center: i32) -> Self {
        Self {
            axis_id,
            raw_min,
            raw_max,
            raw_center,
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    /// Normalize a raw value to [-1.0, 1.0] using bipolar normalization around center.
    pub fn normalize(&self, raw: i32) -> f32 {
        if raw >= self.raw_center {
            let range = (self.raw_max - self.raw_center) as f32;
            if range == 0.0 {
                return 0.0;
            }
            ((raw - self.raw_center) as f32 / range).clamp(0.0, 1.0)
        } else {
            let range = (self.raw_center - self.raw_min) as f32;
            if range == 0.0 {
                return 0.0;
            }
            ((raw - self.raw_center) as f32 / range).clamp(-1.0, 0.0)
        }
    }
}

/// Persistent calibration store.
///
/// Maps (vendor_id, product_id) device keys to lists of per-axis calibrations.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CalibrationStore {
    /// Map from `"VVVV:PPPP"` (hex) to list of per-axis calibrations.
    calibrations: HashMap<String, Vec<AxisCalibration>>,
}

impl CalibrationStore {
    pub fn new() -> Self {
        Default::default()
    }

    /// Format a device key as `"VVVV:PPPP"` (hex).
    fn key(vid: u16, pid: u16) -> String {
        format!("{:04X}:{:04X}", vid, pid)
    }

    /// Store calibration data for the given device.
    pub fn set(&mut self, vid: u16, pid: u16, cals: Vec<AxisCalibration>) {
        self.calibrations.insert(Self::key(vid, pid), cals);
    }

    /// Retrieve calibration data for the given device, if present.
    pub fn get(&self, vid: u16, pid: u16) -> Option<&Vec<AxisCalibration>> {
        self.calibrations.get(&Self::key(vid, pid))
    }

    /// Remove calibration data for the given device. Returns `true` if it existed.
    pub fn remove(&mut self, vid: u16, pid: u16) -> bool {
        self.calibrations.remove(&Self::key(vid, pid)).is_some()
    }

    /// Number of devices with stored calibration data.
    pub fn device_count(&self) -> usize {
        self.calibrations.len()
    }

    /// Load from a TOML file. Returns an empty store if the file does not exist.
    pub fn load_from_file(path: &Path) -> Result<Self, CalibrationStoreError> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let contents = std::fs::read_to_string(path)?;
        toml::from_str(&contents).map_err(|e| CalibrationStoreError::TomlParse(e.to_string()))
    }

    /// Save to a TOML file, creating parent directories as needed.
    pub fn save_to_file(&self, path: &Path) -> Result<(), CalibrationStoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string(self)
            .map_err(|e| CalibrationStoreError::TomlSerialize(e.to_string()))?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}

/// Errors that can occur when loading or saving a [`CalibrationStore`].
#[derive(Debug, thiserror::Error)]
pub enum CalibrationStoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    TomlParse(String),
    #[error("TOML serialize error: {0}")]
    TomlSerialize(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_cal(axis_id: u8) -> AxisCalibration {
        AxisCalibration::new(axis_id, 0, 65535, 32767)
    }

    #[test]
    fn test_empty_store_has_zero_devices() {
        let store = CalibrationStore::new();
        assert_eq!(store.device_count(), 0);
    }

    #[test]
    fn test_set_and_get_calibration() {
        let mut store = CalibrationStore::new();
        let cals = vec![make_cal(0), make_cal(1)];
        store.set(0x044F, 0xB10A, cals.clone());

        let got = store.get(0x044F, 0xB10A).expect("should find calibration");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].axis_id, 0);
        assert_eq!(got[1].axis_id, 1);
        assert_eq!(store.device_count(), 1);
    }

    #[test]
    fn test_remove_calibration() {
        let mut store = CalibrationStore::new();
        store.set(0x044F, 0xB10A, vec![make_cal(0)]);
        assert_eq!(store.device_count(), 1);

        let removed = store.remove(0x044F, 0xB10A);
        assert!(removed, "remove should return true for existing entry");
        assert_eq!(store.device_count(), 0);

        let removed_again = store.remove(0x044F, 0xB10A);
        assert!(
            !removed_again,
            "remove should return false when not present"
        );
    }

    #[test]
    fn test_normalize_above_center() {
        let cal = AxisCalibration::new(0, 0, 65535, 32767);
        // max value should normalize to ~1.0
        let n = cal.normalize(65535);
        assert!((n - 1.0).abs() < 1e-4, "expected ~1.0, got {n}");
        // halfway above center
        let mid = cal.normalize(49151);
        assert!(mid > 0.0 && mid < 1.0, "expected (0,1), got {mid}");
    }

    #[test]
    fn test_normalize_below_center() {
        let cal = AxisCalibration::new(0, 0, 65535, 32767);
        // min value should normalize to ~-1.0
        let n = cal.normalize(0);
        assert!((n + 1.0).abs() < 1e-4, "expected ~-1.0, got {n}");
        // halfway below center
        let mid = cal.normalize(16383);
        assert!(mid > -1.0 && mid < 0.0, "expected (-1,0), got {mid}");
    }

    #[test]
    fn test_normalize_at_center() {
        let cal = AxisCalibration::new(0, 0, 65535, 32767);
        let n = cal.normalize(32767);
        assert_eq!(n, 0.0, "center should normalize to 0.0");
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path: PathBuf = dir.path().join("calibration.toml");

        let mut store = CalibrationStore::new();
        store.set(0x044F, 0xB10A, vec![make_cal(0), make_cal(1)]);
        store.set(0x0483, 0x5740, vec![make_cal(2)]);

        store.save_to_file(&path).expect("save should succeed");
        assert!(path.exists(), "file should be created");

        let loaded = CalibrationStore::load_from_file(&path).expect("load should succeed");
        assert_eq!(loaded.device_count(), 2);

        let thrustmaster = loaded.get(0x044F, 0xB10A).expect("should find device");
        assert_eq!(thrustmaster.len(), 2);
        assert_eq!(thrustmaster[0], make_cal(0));
        assert_eq!(thrustmaster[1], make_cal(1));

        let stm = loaded
            .get(0x0483, 0x5740)
            .expect("should find second device");
        assert_eq!(stm.len(), 1);
        assert_eq!(stm[0], make_cal(2));
    }

    #[test]
    fn test_load_nonexistent_file_returns_empty_store() {
        let path = PathBuf::from("/nonexistent/path/that/does/not/exist/calibration.toml");
        let store = CalibrationStore::load_from_file(&path).expect("should return empty store");
        assert_eq!(store.device_count(), 0);
    }

    #[test]
    fn test_normalize_below_center_nonzero_min() {
        // raw_min=1000, raw_center=5000, raw_max=9000
        // This catches the mutation that replaces (raw_center - raw_min) with (raw_center + raw_min)
        let cal = AxisCalibration::new(0, 1000, 9000, 5000);

        // At raw_min the result should be -1.0
        let at_min = cal.normalize(1000);
        assert!(
            (at_min + 1.0).abs() < 1e-4,
            "raw_min should normalize to -1.0, got {at_min}"
        );

        // Midpoint between min and center: raw=3000, expected -0.5
        let mid = cal.normalize(3000);
        assert!(
            (mid + 0.5).abs() < 1e-4,
            "midpoint below center should normalize to -0.5, got {mid}"
        );

        // Above center still works
        let at_max = cal.normalize(9000);
        assert!(
            (at_max - 1.0).abs() < 1e-4,
            "raw_max should normalize to 1.0, got {at_max}"
        );
    }
}
