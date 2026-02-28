// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Persistent device identification across reboots, USB port changes, and
//! driver updates.
//!
//! [`StableDeviceId`] is a content-addressed identifier derived from immutable
//! USB/HID attributes. [`DeviceFingerprint`] captures the full set of
//! observable device characteristics, and [`DeviceRegistry`] persists the
//! mapping between fingerprints and stable IDs to JSON on disk.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::Path;

use serde::{Deserialize, Serialize};

// ── StableDeviceId ───────────────────────────────────────────────────────

/// A persistent, content-addressed device identifier.
///
/// Computed from VID + PID + serial + interface number when a serial is
/// available, or VID + PID + USB path (port topology) as a fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StableDeviceId(u64);

impl StableDeviceId {
    /// Build an ID from raw components.
    ///
    /// Prefers serial-based identification; falls back to path-based when no
    /// serial is available.
    pub fn new(
        vid: u16,
        pid: u16,
        serial: Option<&str>,
        interface_number: Option<u8>,
        usb_path: Option<&str>,
    ) -> Self {
        let mut h = DefaultHasher::new();
        vid.hash(&mut h);
        pid.hash(&mut h);
        if let Some(sn) = serial {
            // Serial-based path: hash serial + interface
            "serial".hash(&mut h);
            sn.hash(&mut h);
            interface_number.hash(&mut h);
        } else if let Some(path) = usb_path {
            // Fallback: port topology
            "path".hash(&mut h);
            path.hash(&mut h);
        } else {
            // Last resort: VID+PID only
            "vidpid".hash(&mut h);
            interface_number.hash(&mut h);
        }
        Self(h.finish())
    }

    /// Construct a `StableDeviceId` from a [`DeviceFingerprint`].
    pub fn from_fingerprint(fp: &DeviceFingerprint) -> Self {
        Self::new(
            fp.vid,
            fp.pid,
            fp.serial.as_deref(),
            fp.interface_number,
            fp.usb_path.as_deref(),
        )
    }

    /// Construct a `StableDeviceId` from a [`HidDeviceInfo`](crate::HidDeviceInfo).
    pub fn from_device(info: &crate::HidDeviceInfo) -> Self {
        Self::new(
            info.vendor_id,
            info.product_id,
            info.serial_number.as_deref(),
            None, // HidDeviceInfo doesn't carry interface_number
            Some(&info.device_path),
        )
    }

    /// The underlying 64-bit hash value.
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Check whether two IDs refer to the same device with configurable
    /// strictness.
    pub fn matches(self, other: StableDeviceId, strictness: MatchStrictness) -> bool {
        match strictness {
            MatchStrictness::Exact => self.0 == other.0,
            MatchStrictness::Relaxed => self.0 == other.0,
        }
    }
}

impl fmt::Display for StableDeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016X}", self.0)
    }
}

/// How strictly two [`StableDeviceId`]s must agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchStrictness {
    /// Hash must be identical.
    Exact,
    /// Currently the same as `Exact`. Reserved for future fuzzy matching
    /// (e.g. ignore interface number).
    Relaxed,
}

// ── DeviceFingerprint ────────────────────────────────────────────────────

/// Full set of observable device characteristics used for matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceFingerprint {
    pub vid: u16,
    pub pid: u16,
    pub serial: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub interface_number: Option<u8>,
    pub usage_page: u16,
    pub usage: u16,
    pub usb_path: Option<String>,
}

impl DeviceFingerprint {
    /// Create a fingerprint from a [`HidDeviceInfo`](crate::HidDeviceInfo).
    pub fn from_device(info: &crate::HidDeviceInfo) -> Self {
        Self {
            vid: info.vendor_id,
            pid: info.product_id,
            serial: info.serial_number.clone(),
            manufacturer: info.manufacturer.clone(),
            product: info.product_name.clone(),
            interface_number: None,
            usage_page: info.usage_page,
            usage: info.usage,
            usb_path: Some(info.device_path.clone()),
        }
    }

    /// Compute the [`StableDeviceId`] for this fingerprint.
    pub fn stable_id(&self) -> StableDeviceId {
        StableDeviceId::from_fingerprint(self)
    }

    /// Returns `true` if VID, PID, and serial (when both present) match.
    pub fn matches_loosely(&self, other: &DeviceFingerprint) -> bool {
        if self.vid != other.vid || self.pid != other.pid {
            return false;
        }
        match (&self.serial, &other.serial) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        }
    }
}

// ── DeviceRegistry ───────────────────────────────────────────────────────

/// Persistent registry mapping [`StableDeviceId`] → [`DeviceFingerprint`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRegistry {
    devices: HashMap<StableDeviceId, DeviceFingerprint>,
}

impl DeviceRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }

    /// Register a device. If a matching fingerprint already exists, the
    /// existing [`StableDeviceId`] is returned; otherwise a new entry is
    /// created.
    pub fn register(&mut self, fingerprint: DeviceFingerprint) -> StableDeviceId {
        let id = fingerprint.stable_id();
        // If the ID already exists, update the fingerprint (fields may have
        // changed, e.g. device_path after re-plug).
        self.devices.insert(id, fingerprint);
        id
    }

    /// Lookup a fingerprint by stable ID.
    pub fn lookup(&self, id: StableDeviceId) -> Option<&DeviceFingerprint> {
        self.devices.get(&id)
    }

    /// List all registered devices.
    pub fn list_known(&self) -> Vec<(StableDeviceId, &DeviceFingerprint)> {
        self.devices.iter().map(|(&id, fp)| (id, fp)).collect()
    }

    /// Remove a device from the registry.
    pub fn forget(&mut self, id: StableDeviceId) -> bool {
        self.devices.remove(&id).is_some()
    }

    /// Number of registered devices.
    pub fn len(&self) -> usize {
        self.devices.len()
    }

    /// Returns `true` if the registry contains no devices.
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    /// Persist the registry to a JSON file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    /// Load a registry from a JSON file.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

impl Default for DeviceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn warthog_fp() -> DeviceFingerprint {
        DeviceFingerprint {
            vid: 0x044F,
            pid: 0x0402,
            serial: Some("WH001".into()),
            manufacturer: Some("Thrustmaster".into()),
            product: Some("HOTAS Warthog Joystick".into()),
            interface_number: Some(0),
            usage_page: 0x01,
            usage: 0x04,
            usb_path: Some("1-2.3".into()),
        }
    }

    fn vkb_fp() -> DeviceFingerprint {
        DeviceFingerprint {
            vid: 0x231D,
            pid: 0x0136,
            serial: Some("VKB001".into()),
            manufacturer: Some("VKB".into()),
            product: Some("Gladiator NXT EVO".into()),
            interface_number: None,
            usage_page: 0x01,
            usage: 0x04,
            usb_path: Some("1-4.1".into()),
        }
    }

    fn no_serial_fp() -> DeviceFingerprint {
        DeviceFingerprint {
            vid: 0x044F,
            pid: 0xB679,
            serial: None,
            manufacturer: Some("Thrustmaster".into()),
            product: Some("T16000M".into()),
            interface_number: None,
            usage_page: 0x01,
            usage: 0x04,
            usb_path: Some("1-5.2".into()),
        }
    }

    // ── StableDeviceId ───────────────────────────────────────────────

    #[test]
    fn id_from_serial_deterministic() {
        let a = StableDeviceId::new(0x044F, 0x0402, Some("SN1"), Some(0), None);
        let b = StableDeviceId::new(0x044F, 0x0402, Some("SN1"), Some(0), None);
        assert_eq!(a, b);
    }

    #[test]
    fn id_differs_by_serial() {
        let a = StableDeviceId::new(0x044F, 0x0402, Some("SN1"), None, None);
        let b = StableDeviceId::new(0x044F, 0x0402, Some("SN2"), None, None);
        assert_ne!(a, b);
    }

    #[test]
    fn id_differs_by_vid() {
        let a = StableDeviceId::new(0x044F, 0x0402, None, None, Some("1-2"));
        let b = StableDeviceId::new(0x231D, 0x0402, None, None, Some("1-2"));
        assert_ne!(a, b);
    }

    #[test]
    fn id_differs_by_pid() {
        let a = StableDeviceId::new(0x044F, 0x0402, None, None, Some("1-2"));
        let b = StableDeviceId::new(0x044F, 0x0403, None, None, Some("1-2"));
        assert_ne!(a, b);
    }

    #[test]
    fn id_fallback_to_path_when_no_serial() {
        let a = StableDeviceId::new(0x044F, 0x0402, None, None, Some("1-2.3"));
        let b = StableDeviceId::new(0x044F, 0x0402, None, None, Some("1-2.4"));
        assert_ne!(a, b, "different USB paths should produce different IDs");
    }

    #[test]
    fn id_serial_path_independent() {
        // With a serial, the path is ignored
        let a = StableDeviceId::new(0x044F, 0x0402, Some("SN1"), None, Some("1-2.3"));
        let b = StableDeviceId::new(0x044F, 0x0402, Some("SN1"), None, Some("1-2.4"));
        assert_eq!(a, b, "with serial, path should not affect ID");
    }

    #[test]
    fn id_interface_number_distinguishes() {
        let a = StableDeviceId::new(0x044F, 0x0402, Some("SN1"), Some(0), None);
        let b = StableDeviceId::new(0x044F, 0x0402, Some("SN1"), Some(1), None);
        assert_ne!(a, b, "different interfaces = different IDs");
    }

    #[test]
    fn id_from_fingerprint() {
        let fp = warthog_fp();
        let id = StableDeviceId::from_fingerprint(&fp);
        let id2 = fp.stable_id();
        assert_eq!(id, id2);
    }

    #[test]
    fn id_from_device_info() {
        let info = crate::HidDeviceInfo {
            vendor_id: 0x044F,
            product_id: 0x0402,
            serial_number: Some("WH001".into()),
            manufacturer: Some("Thrustmaster".into()),
            product_name: Some("HOTAS Warthog".into()),
            device_path: "1-2.3".into(),
            usage_page: 0x01,
            usage: 0x04,
            report_descriptor: None,
        };
        let id = StableDeviceId::from_device(&info);
        assert_ne!(id.as_u64(), 0);
    }

    #[test]
    fn id_matches_exact() {
        let id = StableDeviceId::new(0x044F, 0x0402, Some("SN1"), None, None);
        assert!(id.matches(id, MatchStrictness::Exact));
    }

    #[test]
    fn id_matches_relaxed() {
        let id = StableDeviceId::new(0x044F, 0x0402, Some("SN1"), None, None);
        assert!(id.matches(id, MatchStrictness::Relaxed));
    }

    #[test]
    fn id_no_match_different() {
        let a = StableDeviceId::new(0x044F, 0x0402, Some("SN1"), None, None);
        let b = StableDeviceId::new(0x044F, 0x0402, Some("SN2"), None, None);
        assert!(!a.matches(b, MatchStrictness::Exact));
    }

    #[test]
    fn id_display_hex() {
        let id = StableDeviceId::new(0x044F, 0x0402, Some("SN1"), None, None);
        let s = format!("{id}");
        assert_eq!(s.len(), 16, "display should be 16 hex chars");
    }

    // ── DeviceFingerprint ────────────────────────────────────────────

    #[test]
    fn fingerprint_from_device_info() {
        let info = crate::HidDeviceInfo {
            vendor_id: 0x231D,
            product_id: 0x0136,
            serial_number: Some("VKB001".into()),
            manufacturer: Some("VKB".into()),
            product_name: Some("Gladiator NXT EVO".into()),
            device_path: "/dev/hidraw0".into(),
            usage_page: 0x01,
            usage: 0x04,
            report_descriptor: None,
        };
        let fp = DeviceFingerprint::from_device(&info);
        assert_eq!(fp.vid, 0x231D);
        assert_eq!(fp.pid, 0x0136);
        assert_eq!(fp.serial.as_deref(), Some("VKB001"));
        assert_eq!(fp.usb_path.as_deref(), Some("/dev/hidraw0"));
    }

    #[test]
    fn fingerprint_matches_loosely_same() {
        let fp = warthog_fp();
        assert!(fp.matches_loosely(&fp));
    }

    #[test]
    fn fingerprint_matches_loosely_missing_serial() {
        let a = warthog_fp();
        let mut b = warthog_fp();
        b.serial = None;
        assert!(a.matches_loosely(&b));
    }

    #[test]
    fn fingerprint_no_match_different_vid() {
        let a = warthog_fp();
        let b = vkb_fp();
        assert!(!a.matches_loosely(&b));
    }

    #[test]
    fn fingerprint_no_match_different_serial() {
        let a = warthog_fp();
        let mut b = warthog_fp();
        b.serial = Some("OTHER".into());
        assert!(!a.matches_loosely(&b));
    }

    // ── DeviceRegistry ───────────────────────────────────────────────

    #[test]
    fn registry_register_and_lookup() {
        let mut reg = DeviceRegistry::new();
        let fp = warthog_fp();
        let id = reg.register(fp.clone());
        let found = reg.lookup(id).unwrap();
        assert_eq!(found.vid, fp.vid);
        assert_eq!(found.serial, fp.serial);
    }

    #[test]
    fn registry_register_idempotent() {
        let mut reg = DeviceRegistry::new();
        let id1 = reg.register(warthog_fp());
        let id2 = reg.register(warthog_fp());
        assert_eq!(id1, id2);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn registry_multiple_devices() {
        let mut reg = DeviceRegistry::new();
        reg.register(warthog_fp());
        reg.register(vkb_fp());
        reg.register(no_serial_fp());
        assert_eq!(reg.len(), 3);
    }

    #[test]
    fn registry_list_known() {
        let mut reg = DeviceRegistry::new();
        reg.register(warthog_fp());
        reg.register(vkb_fp());
        let known = reg.list_known();
        assert_eq!(known.len(), 2);
    }

    #[test]
    fn registry_forget() {
        let mut reg = DeviceRegistry::new();
        let id = reg.register(warthog_fp());
        assert!(reg.forget(id));
        assert!(reg.lookup(id).is_none());
        assert!(reg.is_empty());
    }

    #[test]
    fn registry_forget_nonexistent() {
        let mut reg = DeviceRegistry::new();
        let fake = StableDeviceId::new(0, 0, None, None, None);
        assert!(!reg.forget(fake));
    }

    #[test]
    fn registry_save_load_round_trip() {
        let mut reg = DeviceRegistry::new();
        reg.register(warthog_fp());
        reg.register(vkb_fp());
        reg.register(no_serial_fp());

        let dir = std::env::temp_dir().join("flight_hid_test_registry");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("devices.json");

        reg.save(&path).unwrap();
        let loaded = DeviceRegistry::load(&path).unwrap();

        assert_eq!(loaded.len(), 3);
        for (id, fp) in reg.list_known() {
            let loaded_fp = loaded.lookup(id).expect("device should survive round-trip");
            assert_eq!(loaded_fp.vid, fp.vid);
            assert_eq!(loaded_fp.pid, fp.pid);
            assert_eq!(loaded_fp.serial, fp.serial);
        }

        // cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn registry_load_nonexistent_file() {
        let result = DeviceRegistry::load(Path::new("/nonexistent/path/devices.json"));
        assert!(result.is_err());
    }

    #[test]
    fn registry_load_invalid_json() {
        let dir = std::env::temp_dir().join("flight_hid_test_invalid");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.json");
        std::fs::write(&path, "not valid json").unwrap();

        let result = DeviceRegistry::load(&path);
        assert!(result.is_err());

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn registry_default_is_empty() {
        let reg = DeviceRegistry::default();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }
}
