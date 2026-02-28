// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Stable device identification across USB reconnects and port changes.
//!
//! [`DeviceId`] captures the attributes of a USB HID device that stay constant
//! regardless of which USB port it is plugged into or which OS-assigned path it
//! receives. A deterministic hash is computed so that profile bindings and
//! calibration data survive device reconnects.

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Stable identifier for a USB HID device.
///
/// Two `DeviceId`s compare as equal (via [`matches`](DeviceId::matches)) when
/// they refer to the same *physical* device, even if the OS device path has
/// changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceId {
    /// USB Vendor ID.
    pub vid: u16,
    /// USB Product ID.
    pub pid: u16,
    /// USB serial number string, if present.
    pub serial: Option<String>,
    /// HID top-level usage page.
    pub usage_page: u16,
    /// HID top-level usage.
    pub usage: u16,
    /// USB interface number, disambiguates composite devices.
    pub interface_number: Option<u8>,
    /// Pre-computed deterministic hash of the identifying fields.
    pub stable_hash: u64,
}

impl DeviceId {
    /// Create a new `DeviceId` from raw USB/HID attributes.
    pub fn new(
        vid: u16,
        pid: u16,
        serial: Option<String>,
        usage_page: u16,
        usage: u16,
        interface_number: Option<u8>,
    ) -> Self {
        let hash = compute_hash(
            vid,
            pid,
            serial.as_deref(),
            usage_page,
            usage,
            interface_number,
        );
        Self {
            vid,
            pid,
            serial,
            usage_page,
            usage,
            interface_number,
            stable_hash: hash,
        }
    }

    /// Create a minimal `DeviceId` from vendor/product IDs only.
    ///
    /// Usage page and usage default to Generic Desktop / Joystick (0x01/0x04).
    pub fn from_vid_pid(vid: u16, pid: u16) -> Self {
        Self::new(vid, pid, None, 0x01, 0x04, None)
    }

    /// Deterministic hash that survives USB port changes.
    ///
    /// The hash incorporates VID, PID, serial (if present), usage page, usage,
    /// and interface number. It does **not** include the OS device path.
    pub fn stable_hash(&self) -> u64 {
        self.stable_hash
    }

    /// Returns `true` if `other` refers to the same physical device.
    ///
    /// Matching rules:
    /// 1. VID and PID must be equal.
    /// 2. If both devices have a serial number, the serials must match.
    /// 3. Usage page and usage must match.
    /// 4. If both devices have an interface number, those must match.
    pub fn matches(&self, other: &DeviceId) -> bool {
        if self.vid != other.vid || self.pid != other.pid {
            return false;
        }
        if self.usage_page != other.usage_page || self.usage != other.usage {
            return false;
        }
        // Serial: if both are present they must match. If either is absent we
        // cannot distinguish on serial alone, so we accept the match.
        match (&self.serial, &other.serial) {
            (Some(a), Some(b)) if a != b => return false,
            _ => {}
        }
        // Interface number: same logic.
        if let (Some(a), Some(b)) = (self.interface_number, other.interface_number)
            && a != b
        {
            return false;
        }
        true
    }

    /// Human-readable name like `"Thrustmaster HOTAS Warthog [044F:0402]"`.
    ///
    /// Uses the VID/PID hex representation; a product-name lookup table is not
    /// included here — callers can override the display name via profiles.
    pub fn display_name(&self) -> String {
        let base = format!("HID Device [{:04X}:{:04X}]", self.vid, self.pid);
        if let Some(ref sn) = self.serial {
            format!("{base} S/N {sn}")
        } else {
            base
        }
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Compute a deterministic hash from the identifying fields.
fn compute_hash(
    vid: u16,
    pid: u16,
    serial: Option<&str>,
    usage_page: u16,
    usage: u16,
    interface_number: Option<u8>,
) -> u64 {
    let mut h = DefaultHasher::new();
    vid.hash(&mut h);
    pid.hash(&mut h);
    // Hash presence discriminator so Some("") ≠ None.
    serial.is_some().hash(&mut h);
    if let Some(s) = serial {
        s.hash(&mut h);
    }
    usage_page.hash(&mut h);
    usage.hash(&mut h);
    interface_number.hash(&mut h);
    h.finish()
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_computes_hash() {
        let id = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, None);
        assert_ne!(id.stable_hash(), 0);
    }

    #[test]
    fn from_vid_pid_defaults() {
        let id = DeviceId::from_vid_pid(0x044F, 0x0402);
        assert_eq!(id.usage_page, 0x01);
        assert_eq!(id.usage, 0x04);
        assert!(id.serial.is_none());
    }

    #[test]
    fn hash_is_deterministic() {
        let a = DeviceId::new(0x044F, 0x0402, Some("ABC".into()), 0x01, 0x04, Some(0));
        let b = DeviceId::new(0x044F, 0x0402, Some("ABC".into()), 0x01, 0x04, Some(0));
        assert_eq!(a.stable_hash(), b.stable_hash());
    }

    #[test]
    fn hash_differs_with_serial() {
        let a = DeviceId::new(0x044F, 0x0402, Some("ABC".into()), 0x01, 0x04, None);
        let b = DeviceId::new(0x044F, 0x0402, Some("DEF".into()), 0x01, 0x04, None);
        assert_ne!(a.stable_hash(), b.stable_hash());
    }

    #[test]
    fn hash_differs_none_vs_some_serial() {
        let a = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, None);
        let b = DeviceId::new(0x044F, 0x0402, Some("ABC".into()), 0x01, 0x04, None);
        assert_ne!(a.stable_hash(), b.stable_hash());
    }

    #[test]
    fn matches_same_device() {
        let a = DeviceId::new(0x044F, 0x0402, Some("SN1".into()), 0x01, 0x04, Some(0));
        let b = DeviceId::new(0x044F, 0x0402, Some("SN1".into()), 0x01, 0x04, Some(0));
        assert!(a.matches(&b));
    }

    #[test]
    fn matches_different_vid() {
        let a = DeviceId::from_vid_pid(0x044F, 0x0402);
        let b = DeviceId::from_vid_pid(0x231D, 0x0402);
        assert!(!a.matches(&b));
    }

    #[test]
    fn matches_different_serial() {
        let a = DeviceId::new(0x044F, 0x0402, Some("SN1".into()), 0x01, 0x04, None);
        let b = DeviceId::new(0x044F, 0x0402, Some("SN2".into()), 0x01, 0x04, None);
        assert!(!a.matches(&b));
    }

    #[test]
    fn matches_one_serial_missing() {
        let a = DeviceId::new(0x044F, 0x0402, Some("SN1".into()), 0x01, 0x04, None);
        let b = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, None);
        assert!(a.matches(&b), "absent serial should not reject match");
    }

    #[test]
    fn matches_different_interface() {
        let a = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, Some(0));
        let b = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, Some(1));
        assert!(!a.matches(&b));
    }

    #[test]
    fn matches_one_interface_missing() {
        let a = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, Some(0));
        let b = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, None);
        assert!(a.matches(&b));
    }

    #[test]
    fn matches_different_usage() {
        let a = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, None);
        let b = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x05, None);
        assert!(!a.matches(&b));
    }

    #[test]
    fn display_name_without_serial() {
        let id = DeviceId::from_vid_pid(0x044F, 0x0402);
        assert_eq!(id.display_name(), "HID Device [044F:0402]");
    }

    #[test]
    fn display_name_with_serial() {
        let id = DeviceId::new(0x044F, 0x0402, Some("ABC123".into()), 0x01, 0x04, None);
        assert_eq!(id.display_name(), "HID Device [044F:0402] S/N ABC123");
    }

    #[test]
    fn display_trait() {
        let id = DeviceId::from_vid_pid(0x231D, 0x0136);
        let s = format!("{id}");
        assert!(s.contains("231D"));
        assert!(s.contains("0136"));
    }

    #[test]
    fn hash_differs_with_interface() {
        let a = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, Some(0));
        let b = DeviceId::new(0x044F, 0x0402, None, 0x01, 0x04, Some(1));
        assert_ne!(a.stable_hash(), b.stable_hash());
    }
}
