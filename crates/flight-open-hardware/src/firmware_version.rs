// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID IN Report 0xF0 — firmware version from device to host.
//!
//! Layout (8 bytes):
//!
//! | Bytes | Field      | Type | Notes                        |
//! |-------|------------|------|------------------------------|
//! | 0     | report_id  | u8   | 0xF0                         |
//! | 1     | major      | u8   | Semantic version major       |
//! | 2     | minor      | u8   | Semantic version minor       |
//! | 3     | patch      | u8   | Semantic version patch       |
//! | 4–7   | build_hash | [u8;4]| Lower 4 bytes of git SHA   |

/// Report ID for the firmware version report.
pub const FIRMWARE_REPORT_ID: u8 = 0xF0;

/// Firmware version info returned from the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareVersionReport {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub build_hash: [u8; 4],
}

impl FirmwareVersionReport {
    /// Parse an 8-byte firmware version report.
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 8 || buf[0] != FIRMWARE_REPORT_ID {
            return None;
        }
        Some(Self {
            major: buf[1],
            minor: buf[2],
            patch: buf[3],
            build_hash: [buf[4], buf[5], buf[6], buf[7]],
        })
    }

    /// Serialise to an 8-byte buffer.
    pub fn to_bytes(&self) -> [u8; 8] {
        [
            FIRMWARE_REPORT_ID,
            self.major,
            self.minor,
            self.patch,
            self.build_hash[0],
            self.build_hash[1],
            self.build_hash[2],
            self.build_hash[3],
        ]
    }

    /// Return the version as a `(major, minor, patch)` tuple.
    pub fn version(&self) -> (u8, u8, u8) {
        (self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let fw = FirmwareVersionReport {
            major: 1,
            minor: 2,
            patch: 3,
            build_hash: [0xDE, 0xAD, 0xBE, 0xEF],
        };
        let bytes = fw.to_bytes();
        let parsed = FirmwareVersionReport::parse(&bytes).unwrap();
        assert_eq!(fw, parsed);
        assert_eq!(parsed.version(), (1, 2, 3));
    }

    #[test]
    fn test_wrong_id_returns_none() {
        let bytes = [0x01u8; 8];
        assert!(FirmwareVersionReport::parse(&bytes).is_none());
    }
}
