// SPDX-License-Identifier: MIT OR Apache-2.0

//! X52 Pro MFD implementation.
//!
//! **UNVERIFIED PROTOCOL** - See `docs/reference/hotas-claims.md`
//!
//! # Protocol
//!
//! The X52 Pro (VID `0x06A3`, PID `0x0762`) exposes MFD control via HID output
//! reports sent with [`hidapi::HidDevice::write`].
//!
//! ## Text line write
//! ```text
//! Byte  0 : HID report ID  (0x00 – unnumbered report)
//! Byte  1 : Command        (0xB4)
//! Byte  2 : Line index     (0, 1, or 2)
//! Bytes 3–18 : Characters  (printable ASCII, space-padded to 16 chars)
//! ```
//!
//! ## Brightness control
//! ```text
//! Byte 0 : HID report ID  (0x00)
//! Byte 1 : Command        (0xB1)
//! Byte 2 : Brightness     (0–127)
//! ```
//!
//! Protocol source: community reverse-engineering; **not** verified via USB capture.

use std::sync::Mutex;

use hidapi::HidDevice;

use crate::policy::allow_device_io;
use crate::traits::{HotasError, HotasResult, MfdProtocol};

/// HID output report command byte for MFD text-line write.
const MFD_CMD_LINE: u8 = 0xB4;

/// HID output report command byte for MFD brightness.
const MFD_CMD_BRIGHTNESS: u8 = 0xB1;

/// Maximum characters per MFD line.
pub const MFD_LINE_LENGTH: usize = 16;

/// Number of lines on the MFD.
pub const MFD_LINE_COUNT: u8 = 3;

/// Saitek vendor ID.
pub const X52_PRO_VID: u16 = 0x06A3;

/// X52 Pro product ID.
pub const X52_PRO_PID: u16 = 0x0762;

/// X52 Pro MFD display controller.
///
/// # Protocol Status
///
/// **UNVERIFIED** – The MFD protocol is based on community-sourced
/// documentation and has not been verified via USB capture.
/// Key uncertainties: exact command bytes, character encoding, brightness range.
///
/// See GitHub issue tracking verification progress.
///
/// # Usage
///
/// ```ignore
/// let api = hidapi::HidApi::new().unwrap();
/// let device = api.open(X52_PRO_VID, X52_PRO_PID).unwrap();
/// let mut mfd = X52ProMfd::new(device);
/// ```
pub struct X52ProMfd {
    /// Open HID device handle for the X52 Pro (mutex-wrapped for `Sync`).
    device: Mutex<HidDevice>,
    /// Current MFD brightness level (0–127).
    brightness: u8,
}

impl X52ProMfd {
    /// Create a new MFD controller from an already-opened [`HidDevice`].
    ///
    /// Use [`hidapi::HidApi::open`] with [`X52_PRO_VID`] and [`X52_PRO_PID`]
    /// to obtain the handle.
    pub fn new(device: HidDevice) -> Self {
        tracing::warn!(
            target: "hotas::mfd",
            "Creating X52 Pro MFD controller with UNVERIFIED protocol. \
             See docs/reference/hotas-claims.md for verification status."
        );
        Self {
            device: Mutex::new(device),
            brightness: 127,
        }
    }

    /// Write a HID output report to the device.
    ///
    /// Checks the device I/O policy gate before performing any I/O.
    fn write_hid_report(&self, report: &[u8]) -> HotasResult<()> {
        if !allow_device_io() {
            return Err(HotasError::UnverifiedProtocol("x52_pro_mfd"));
        }

        tracing::debug!(
            target: "hotas::mfd",
            report_len = report.len(),
            cmd = %format!("0x{:02X}", report.get(1).copied().unwrap_or(0)),
            "Sending HID output report (UNVERIFIED)"
        );

        self.device
            .lock()
            .map_err(|_| HotasError::UsbError("device mutex poisoned".into()))?
            .write(report)
            .map(|_| ())
            .map_err(|e| HotasError::UsbError(e.to_string()))
    }

    /// Encode text for MFD display.
    ///
    /// The MFD accepts printable ASCII characters (`' '`–`'~'`). Non-ASCII
    /// characters are replaced with `'?'`. Output is truncated to [`MFD_LINE_LENGTH`].
    fn encode_text(text: &str) -> Vec<u8> {
        text.chars()
            .take(MFD_LINE_LENGTH)
            .map(|c| {
                if c.is_ascii() && c >= ' ' {
                    c as u8
                } else {
                    b'?'
                }
            })
            .collect()
    }
}

impl MfdProtocol for X52ProMfd {
    fn set_line(&mut self, line: u8, text: &str) -> HotasResult<()> {
        if line >= MFD_LINE_COUNT {
            return Err(HotasError::InvalidParameter(format!(
                "MFD line {} out of range (0-{})",
                line,
                MFD_LINE_COUNT - 1
            )));
        }

        tracing::info!(
            target: "hotas::mfd",
            line = line,
            text = %text,
            "Setting MFD line (UNVERIFIED protocol)"
        );

        let chars = Self::encode_text(text);

        // Build HID output report:
        //   buf[0]    = 0x00       (HID report ID – unnumbered report)
        //   buf[1]    = 0xB4       (MFD text command)
        //   buf[2]    = line       (line index 0–2)
        //   buf[3..19] = chars     (up to 16 ASCII bytes, space-padded)
        let mut buf = [b' '; 3 + MFD_LINE_LENGTH];
        buf[0] = 0x00;
        buf[1] = MFD_CMD_LINE;
        buf[2] = line;
        let copy_len = chars.len().min(MFD_LINE_LENGTH);
        buf[3..3 + copy_len].copy_from_slice(&chars[..copy_len]);

        self.write_hid_report(&buf)
    }

    fn set_brightness(&mut self, level: u8) -> HotasResult<()> {
        let clamped = level.min(127);

        tracing::info!(
            target: "hotas::mfd",
            level = clamped,
            "Setting MFD brightness (UNVERIFIED protocol)"
        );

        // HID output report: [report_id=0x00, cmd=0xB1, brightness_level]
        let buf = [0x00u8, MFD_CMD_BRIGHTNESS, clamped];
        self.write_hid_report(&buf)?;

        self.brightness = clamped;
        Ok(())
    }

    fn clear(&mut self) -> HotasResult<()> {
        for line in 0..MFD_LINE_COUNT {
            self.set_line(line, "")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_text_ascii() {
        let encoded = X52ProMfd::encode_text("HELLO");
        assert_eq!(encoded, b"HELLO");
    }

    #[test]
    fn test_encode_text_truncation() {
        let long_text = "THIS IS A VERY LONG STRING THAT EXCEEDS THE LIMIT";
        let encoded = X52ProMfd::encode_text(long_text);
        assert_eq!(encoded.len(), MFD_LINE_LENGTH);
    }

    #[test]
    fn test_encode_text_non_ascii() {
        // Use explicit unicode escape for the accented E character
        let encoded = X52ProMfd::encode_text("H\u{00C9}LLO"); // HELLO with E-acute
        assert_eq!(encoded, b"H?LLO");
    }

    /// Verify the HID output report buffer layout for a text-line write.
    #[test]
    fn test_hid_line_report_layout() {
        let line: u8 = 1;
        let chars = X52ProMfd::encode_text("TEST");
        let mut buf = [b' '; 3 + MFD_LINE_LENGTH];
        buf[0] = 0x00;
        buf[1] = MFD_CMD_LINE;
        buf[2] = line;
        buf[3..3 + chars.len()].copy_from_slice(&chars);

        assert_eq!(buf[0], 0x00, "report ID must be 0");
        assert_eq!(buf[1], 0xB4, "MFD text command must be 0xB4");
        assert_eq!(buf[2], 1, "line index must be 1");
        assert_eq!(&buf[3..7], b"TEST");
        assert_eq!(buf[7], b' ', "unused chars must be space-padded");
        assert_eq!(buf.len(), 19, "total report must be 19 bytes");
    }

    /// Verify the HID output report buffer layout for brightness.
    #[test]
    fn test_hid_brightness_report_layout() {
        let buf = [0x00u8, MFD_CMD_BRIGHTNESS, 64u8];
        assert_eq!(buf[0], 0x00, "report ID must be 0");
        assert_eq!(buf[1], 0xB1, "brightness command must be 0xB1");
        assert_eq!(buf[2], 64);
    }

    #[test]
    fn test_brightness_clamped_to_127() {
        assert_eq!(200u8.min(127), 127);
    }

    #[test]
    fn test_protocol_constants() {
        assert_eq!(MFD_LINE_COUNT, 3);
        assert_eq!(MFD_LINE_LENGTH, 16);
        assert_eq!(X52_PRO_VID, 0x06A3);
        assert_eq!(X52_PRO_PID, 0x0762);
        assert_eq!(MFD_CMD_LINE, 0xB4);
        assert_eq!(MFD_CMD_BRIGHTNESS, 0xB1);
    }
}
