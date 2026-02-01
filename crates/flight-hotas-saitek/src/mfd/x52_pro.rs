// SPDX-License-Identifier: MIT OR Apache-2.0

//! X52 Pro MFD implementation.
//!
//! **UNVERIFIED PROTOCOL** - See `docs/reference/hotas-claims.md`

use crate::traits::{HotasError, HotasResult, MfdProtocol};

/// Hypothesized USB control transfer request type for MFD.
const MFD_REQUEST_TYPE: u8 = 0x40; // Vendor, host-to-device

/// Hypothesized bRequest value for MFD line write.
const MFD_REQUEST_LINE: u8 = 0x91;

/// Hypothesized bRequest value for MFD brightness.
const MFD_REQUEST_BRIGHTNESS: u8 = 0xB1;

/// Maximum characters per MFD line.
pub const MFD_LINE_LENGTH: usize = 16;

/// Number of lines on the MFD.
pub const MFD_LINE_COUNT: u8 = 3;

/// X52 Pro MFD display controller.
///
/// # Protocol Status
///
/// **UNVERIFIED** - The MFD protocol is based on hypothesis from community
/// documentation. Key uncertainties:
///
/// - Exact `bmRequestType` and `bRequest` values
/// - Character encoding (ASCII subset assumed)
/// - Brightness control protocol
///
/// See GitHub issue tracking verification progress.
pub struct X52ProMfd {
    /// Placeholder for device handle - actual implementation would use hidapi or similar
    device_path: String,
    /// Current brightness level
    brightness: u8,
    /// Protocol verification status
    verified: bool,
}

impl X52ProMfd {
    /// Create a new MFD controller for the specified device.
    pub fn new(device_path: String) -> Self {
        tracing::warn!(
            target: "hotas::mfd",
            device = %device_path,
            "Creating X52 Pro MFD controller with UNVERIFIED protocol. \
             See docs/reference/hotas-claims.md for verification status."
        );

        Self {
            device_path,
            brightness: 127,
            verified: false,
        }
    }

    /// Attempt to send a control transfer to the device.
    ///
    /// This is a placeholder - actual implementation would use platform HID APIs.
    fn send_control_transfer(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
    ) -> HotasResult<()> {
        tracing::debug!(
            target: "hotas::mfd",
            device = %self.device_path,
            request_type = %format!("0x{:02X}", request_type),
            request = %format!("0x{:02X}", request),
            value = %format!("0x{:04X}", value),
            index = %format!("0x{:04X}", index),
            data_len = data.len(),
            "Attempting MFD control transfer (UNVERIFIED)"
        );

        // TODO: Implement actual USB control transfer
        // This would use hidapi or platform-specific APIs

        // For now, return an error indicating the protocol is unverified
        if !self.verified {
            Err(HotasError::UnverifiedProtocol("x52_pro_mfd"))
        } else {
            Ok(())
        }
    }

    /// Encode text for MFD display.
    ///
    /// The MFD likely accepts ASCII subset only. Non-ASCII characters
    /// are replaced with '?'.
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

        let encoded = Self::encode_text(text);

        // Hypothesis: wValue = line number, wIndex = 0
        self.send_control_transfer(MFD_REQUEST_TYPE, MFD_REQUEST_LINE, line as u16, 0, &encoded)
    }

    fn set_brightness(&mut self, level: u8) -> HotasResult<()> {
        let clamped = level.min(127);

        tracing::info!(
            target: "hotas::mfd",
            level = clamped,
            "Setting MFD brightness (UNVERIFIED protocol)"
        );

        // Hypothesis: wValue = brightness level, wIndex = 0
        self.send_control_transfer(
            MFD_REQUEST_TYPE,
            MFD_REQUEST_BRIGHTNESS,
            clamped as u16,
            0,
            &[],
        )?;

        self.brightness = clamped;
        Ok(())
    }

    fn clear(&mut self) -> HotasResult<()> {
        // Clear all lines
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
}
