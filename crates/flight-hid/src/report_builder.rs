// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Builder for HID output reports (LEDs, motor control, etc.).
//!
//! [`HidReportBuilder`] provides a mutable byte buffer with bit-level and
//! byte-level setters to construct HID output and feature reports before
//! sending them to a device.

use std::fmt;

/// Builder for HID output/feature reports.
///
/// # Example
///
/// ```rust
/// use flight_hid::report_builder::HidReportBuilder;
///
/// let report = HidReportBuilder::new(4)
///     .with_report_id(0x01)
///     .set_bit(0, true)
///     .set_byte(1, 0xFF)
///     .build();
///
/// assert_eq!(report[0], 0x01); // report ID
/// assert_eq!(report[1], 0x01); // bit 0 set
/// assert_eq!(report[2], 0xFF); // byte 1
/// ```
pub struct HidReportBuilder {
    report_id: Option<u8>,
    data: Vec<u8>,
    bit_offset: usize,
}

impl HidReportBuilder {
    /// Create a new builder with a zero-filled data buffer of `size_bytes`.
    pub fn new(size_bytes: usize) -> Self {
        Self {
            report_id: None,
            data: vec![0u8; size_bytes],
            bit_offset: 0,
        }
    }

    /// Set the HID report ID prepended to the output buffer.
    #[must_use]
    pub fn with_report_id(mut self, id: u8) -> Self {
        self.report_id = Some(id);
        self
    }

    /// Set or clear a single bit at `bit_offset` within the data buffer.
    ///
    /// Bit 0 is the LSB of byte 0.  Out-of-range offsets are silently ignored.
    #[must_use]
    pub fn set_bit(mut self, bit_offset: usize, value: bool) -> Self {
        self.set_bit_mut(bit_offset, value);
        self
    }

    /// Mutable variant of [`set_bit`](Self::set_bit) for use in loops.
    pub fn set_bit_mut(&mut self, bit_offset: usize, value: bool) {
        let byte_idx = bit_offset / 8;
        let bit_idx = bit_offset % 8;
        if byte_idx < self.data.len() {
            if value {
                self.data[byte_idx] |= 1 << bit_idx;
            } else {
                self.data[byte_idx] &= !(1 << bit_idx);
            }
        }
    }

    /// Set a full byte at the given byte offset.
    ///
    /// Out-of-range offsets are silently ignored.
    #[must_use]
    pub fn set_byte(mut self, offset: usize, value: u8) -> Self {
        self.set_byte_mut(offset, value);
        self
    }

    /// Mutable variant of [`set_byte`](Self::set_byte).
    pub fn set_byte_mut(&mut self, offset: usize, value: u8) {
        if offset < self.data.len() {
            self.data[offset] = value;
        }
    }

    /// Set a 16-bit little-endian value at `offset` (byte index).
    ///
    /// Requires at least `offset + 2` bytes in the buffer.
    #[must_use]
    pub fn set_u16_le(mut self, offset: usize, value: u16) -> Self {
        self.set_u16_le_mut(offset, value);
        self
    }

    /// Mutable variant of [`set_u16_le`](Self::set_u16_le).
    pub fn set_u16_le_mut(&mut self, offset: usize, value: u16) {
        if offset + 1 < self.data.len() {
            let bytes = value.to_le_bytes();
            self.data[offset] = bytes[0];
            self.data[offset + 1] = bytes[1];
        }
    }

    /// Advance the internal bit cursor and set the next bit.
    ///
    /// Useful for sequentially writing a series of bit fields.
    #[must_use]
    pub fn push_bit(mut self, value: bool) -> Self {
        self.set_bit_mut(self.bit_offset, value);
        self.bit_offset += 1;
        self
    }

    /// Reset all data bytes to zero without changing the buffer size or report ID.
    pub fn clear(&mut self) {
        for b in &mut self.data {
            *b = 0;
        }
        self.bit_offset = 0;
    }

    /// Current length of the data buffer (excluding report ID).
    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    /// Finalize and return the report bytes.
    ///
    /// If a report ID was set, it is prepended as the first byte.
    pub fn build(&self) -> Vec<u8> {
        match self.report_id {
            Some(id) => {
                let mut out = Vec::with_capacity(1 + self.data.len());
                out.push(id);
                out.extend_from_slice(&self.data);
                out
            }
            None => self.data.clone(),
        }
    }
}

impl fmt::Debug for HidReportBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HidReportBuilder")
            .field("report_id", &self.report_id)
            .field("data_len", &self.data.len())
            .field("bit_offset", &self.bit_offset)
            .finish()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_zeroed() {
        let b = HidReportBuilder::new(4);
        assert_eq!(b.build(), vec![0, 0, 0, 0]);
    }

    #[test]
    fn with_report_id_prepends() {
        let b = HidReportBuilder::new(2).with_report_id(0x05);
        let report = b.build();
        assert_eq!(report.len(), 3);
        assert_eq!(report[0], 0x05);
    }

    #[test]
    fn set_bit_lsb() {
        let report = HidReportBuilder::new(1).set_bit(0, true).build();
        assert_eq!(report, vec![0x01]);
    }

    #[test]
    fn set_bit_msb() {
        let report = HidReportBuilder::new(1).set_bit(7, true).build();
        assert_eq!(report, vec![0x80]);
    }

    #[test]
    fn set_bit_second_byte() {
        let report = HidReportBuilder::new(2).set_bit(8, true).build();
        assert_eq!(report, vec![0x00, 0x01]);
    }

    #[test]
    fn set_bit_clear() {
        let report = HidReportBuilder::new(1)
            .set_bit(0, true)
            .set_bit(0, false)
            .build();
        assert_eq!(report, vec![0x00]);
    }

    #[test]
    fn set_bit_out_of_range_ignored() {
        let report = HidReportBuilder::new(1).set_bit(100, true).build();
        assert_eq!(report, vec![0x00]);
    }

    #[test]
    fn set_byte_basic() {
        let report = HidReportBuilder::new(3).set_byte(1, 0xAB).build();
        assert_eq!(report, vec![0x00, 0xAB, 0x00]);
    }

    #[test]
    fn set_byte_out_of_range_ignored() {
        let report = HidReportBuilder::new(2).set_byte(5, 0xFF).build();
        assert_eq!(report, vec![0x00, 0x00]);
    }

    #[test]
    fn set_u16_le_basic() {
        let report = HidReportBuilder::new(4).set_u16_le(1, 0x1234).build();
        assert_eq!(report[1], 0x34); // low byte
        assert_eq!(report[2], 0x12); // high byte
    }

    #[test]
    fn set_u16_le_out_of_range_ignored() {
        let report = HidReportBuilder::new(2).set_u16_le(2, 0xFFFF).build();
        assert_eq!(report, vec![0x00, 0x00]);
    }

    #[test]
    fn push_bit_sequential() {
        let report = HidReportBuilder::new(1)
            .push_bit(true)
            .push_bit(false)
            .push_bit(true)
            .push_bit(true)
            .build();
        // Bits: 1,0,1,1,0,0,0,0 = 0x0D
        assert_eq!(report, vec![0x0D]);
    }

    #[test]
    fn clear_resets_data() {
        let mut b = HidReportBuilder::new(3).with_report_id(0x01);
        b.set_byte_mut(0, 0xFF);
        b.set_byte_mut(1, 0xAA);
        b.clear();
        let report = b.build();
        assert_eq!(report, vec![0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn data_len_excludes_report_id() {
        let b = HidReportBuilder::new(5).with_report_id(0x02);
        assert_eq!(b.data_len(), 5);
        assert_eq!(b.build().len(), 6); // 1 (id) + 5
    }

    #[test]
    fn debug_impl() {
        let b = HidReportBuilder::new(4).with_report_id(0x01);
        let s = format!("{b:?}");
        assert!(s.contains("HidReportBuilder"));
        assert!(s.contains("report_id"));
    }

    #[test]
    fn combined_operations() {
        let report = HidReportBuilder::new(4)
            .with_report_id(0x02)
            .set_byte(0, 0x0F)
            .set_bit(4, true)
            .set_u16_le(2, 0xBEEF)
            .build();
        assert_eq!(report[0], 0x02); // report ID
        assert_eq!(report[1], 0x1F); // 0x0F | (1<<4)
        assert_eq!(report[3], 0xEF); // low byte of 0xBEEF
        assert_eq!(report[4], 0xBE); // high byte of 0xBEEF
    }
}
