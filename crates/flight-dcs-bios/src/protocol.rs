// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS-BIOS export protocol frame parser.
//!
//! The export stream uses a binary format:
//! - Sync sequence: `0x55 0x55 0x55 0x55` marks frame boundaries
//! - Updates: `(address: u16le, length: u16le, data: [u8; length])` segments
//! - All multi-byte values are little-endian
//! - Address and length are always even (16-bit aligned)

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Sync sequence that marks the start of a DCS-BIOS frame.
pub const SYNC_BYTES: [u8; 4] = [0x55, 0x55, 0x55, 0x55];

/// Maximum address space size (64 KiB).
pub const ADDRESS_SPACE_SIZE: usize = 65536;

/// Maximum single update data length (entire address space).
const MAX_UPDATE_LENGTH: u16 = 0xFFFE;

/// Errors during DCS-BIOS frame parsing.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The data does not start with the sync sequence.
    #[error("missing sync sequence (expected 0x55 0x55 0x55 0x55)")]
    MissingSync,
    /// Not enough bytes to read an update header.
    #[error("incomplete update header at offset {offset}")]
    IncompleteHeader { offset: usize },
    /// Not enough bytes to read the update data.
    #[error(
        "incomplete update data at offset {offset}: expected {expected} bytes, got {available}"
    )]
    IncompleteData {
        offset: usize,
        expected: u16,
        available: usize,
    },
    /// Address is odd (must be 16-bit aligned).
    #[error("misaligned address 0x{address:04X} at offset {offset}")]
    MisalignedAddress { address: u16, offset: usize },
    /// Length is odd (must be even).
    #[error("odd data length {length} at offset {offset}")]
    OddLength { length: u16, offset: usize },
    /// Update would write past the end of the address space.
    #[error("update at 0x{address:04X} with length {length} exceeds address space")]
    AddressOverflow { address: u16, length: u16 },
    /// Frame is empty (contains only sync bytes).
    #[error("empty frame (no updates after sync)")]
    EmptyFrame,
}

/// Address descriptor for a DCS-BIOS integer output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DcsBiosAddress {
    /// Start address in the 65536-byte address space (always even).
    pub address: u16,
    /// Bitmask selecting the relevant bits within the 16-bit word.
    pub mask: u16,
    /// Right-shift to apply after masking.
    pub shift: u8,
    /// Maximum value after mask and shift.
    pub max_value: u16,
}

impl DcsBiosAddress {
    /// Create a new address descriptor, computing `max_value` from mask/shift.
    ///
    /// `shift` must be in the range `0..=15` (bit position within a 16-bit word).
    #[must_use]
    pub fn new(address: u16, mask: u16, shift: u8) -> Self {
        assert!(shift < 16, "shift must be 0..=15");
        let max_value = mask >> shift;
        Self {
            address,
            mask,
            shift,
            max_value,
        }
    }

    /// Decode a value from a 16-bit word at this address.
    ///
    /// `shift` must be in the range `0..=15` (bit position within a 16-bit word).
    #[must_use]
    pub fn decode(&self, word: u16) -> u16 {
        assert!(self.shift < 16, "shift must be 0..=15");
        (word & self.mask) >> self.shift
    }
}

/// A single update from a DCS-BIOS export frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DcsBiosUpdate {
    /// Start address (always even, 16-bit aligned).
    pub address: u16,
    /// Data payload.
    pub data: Vec<u8>,
}

/// Parse a complete DCS-BIOS export frame.
///
/// The frame must begin with the 4-byte sync sequence (`0x55 0x55 0x55 0x55`),
/// followed by one or more update segments. Each segment is:
/// `(address: u16le, length: u16le, data: [u8; length])`.
///
/// Returns a list of updates, or an error if the frame is malformed.
pub fn parse_frame(data: &[u8]) -> Result<Vec<DcsBiosUpdate>, ParseError> {
    if data.len() < 4 || data[..4] != SYNC_BYTES {
        return Err(ParseError::MissingSync);
    }

    let mut offset = 4;
    let mut updates = Vec::new();

    while offset < data.len() {
        // Check for another sync sequence (next frame boundary)
        if data.len() >= offset + 4 && data[offset..offset + 4] == SYNC_BYTES {
            break;
        }

        // Need at least 4 bytes for address + length header
        if offset + 4 > data.len() {
            return Err(ParseError::IncompleteHeader { offset });
        }

        let address = u16::from_le_bytes([data[offset], data[offset + 1]]);
        let length = u16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        offset += 4;

        // Validate alignment
        if !address.is_multiple_of(2) {
            return Err(ParseError::MisalignedAddress {
                address,
                offset: offset - 4,
            });
        }
        if !length.is_multiple_of(2) {
            return Err(ParseError::OddLength {
                length,
                offset: offset - 4,
            });
        }

        // Validate length
        if length > MAX_UPDATE_LENGTH {
            return Err(ParseError::AddressOverflow { address, length });
        }

        // Validate address + length doesn't overflow
        let end = address as u32 + length as u32;
        if end > ADDRESS_SPACE_SIZE as u32 {
            return Err(ParseError::AddressOverflow { address, length });
        }

        let len = length as usize;
        if offset + len > data.len() {
            return Err(ParseError::IncompleteData {
                offset,
                expected: length,
                available: data.len() - offset,
            });
        }

        updates.push(DcsBiosUpdate {
            address,
            data: data[offset..offset + len].to_vec(),
        });
        offset += len;
    }

    if updates.is_empty() {
        return Err(ParseError::EmptyFrame);
    }

    Ok(updates)
}

/// Find sync sequences in a byte stream, returning offsets of each frame start.
#[must_use]
pub fn find_sync_positions(data: &[u8]) -> Vec<usize> {
    let mut positions = Vec::new();
    if data.len() < 4 {
        return positions;
    }
    for i in 0..=data.len() - 4 {
        if data[i..i + 4] == SYNC_BYTES {
            positions.push(i);
        }
    }
    positions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(updates: &[(u16, &[u8])]) -> Vec<u8> {
        let mut frame = vec![0x55, 0x55, 0x55, 0x55];
        for (addr, data) in updates {
            frame.extend_from_slice(&addr.to_le_bytes());
            frame.extend_from_slice(&(data.len() as u16).to_le_bytes());
            frame.extend_from_slice(data);
        }
        frame
    }

    #[test]
    fn parse_single_update() {
        let frame = make_frame(&[(0x1000, &[0x41, 0x2D, 0x31, 0x30])]);
        let updates = parse_frame(&frame).unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].address, 0x1000);
        assert_eq!(updates[0].data, vec![0x41, 0x2D, 0x31, 0x30]);
    }

    #[test]
    fn parse_multiple_updates() {
        let frame = make_frame(&[(0x0000, &[0x01, 0x00]), (0x1000, &[0xFF, 0x00, 0xAA, 0xBB])]);
        let updates = parse_frame(&frame).unwrap();
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].address, 0x0000);
        assert_eq!(updates[1].address, 0x1000);
    }

    #[test]
    fn missing_sync_returns_error() {
        let data = vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x02, 0x00, 0xAA, 0xBB];
        assert!(matches!(parse_frame(&data), Err(ParseError::MissingSync)));
    }

    #[test]
    fn empty_data_returns_missing_sync() {
        assert!(matches!(parse_frame(&[]), Err(ParseError::MissingSync)));
    }

    #[test]
    fn sync_only_returns_empty_frame() {
        let data = vec![0x55, 0x55, 0x55, 0x55];
        assert!(matches!(parse_frame(&data), Err(ParseError::EmptyFrame)));
    }

    #[test]
    fn incomplete_header_returns_error() {
        let data = vec![0x55, 0x55, 0x55, 0x55, 0x00, 0x10];
        assert!(matches!(
            parse_frame(&data),
            Err(ParseError::IncompleteHeader { .. })
        ));
    }

    #[test]
    fn incomplete_data_returns_error() {
        let data = vec![0x55, 0x55, 0x55, 0x55, 0x00, 0x10, 0x04, 0x00, 0xAA];
        assert!(matches!(
            parse_frame(&data),
            Err(ParseError::IncompleteData { .. })
        ));
    }

    #[test]
    fn misaligned_address_returns_error() {
        // Odd address 0x1001
        let data = vec![0x55, 0x55, 0x55, 0x55, 0x01, 0x10, 0x02, 0x00, 0xAA, 0xBB];
        assert!(matches!(
            parse_frame(&data),
            Err(ParseError::MisalignedAddress { .. })
        ));
    }

    #[test]
    fn odd_length_returns_error() {
        // Length = 3 (odd)
        let data = vec![
            0x55, 0x55, 0x55, 0x55, 0x00, 0x10, 0x03, 0x00, 0xAA, 0xBB, 0xCC,
        ];
        assert!(matches!(
            parse_frame(&data),
            Err(ParseError::OddLength { .. })
        ));
    }

    #[test]
    fn address_overflow_returns_error() {
        // Address 0xFFF0 + length 0x0020 > 0x10000
        let mut data = vec![0x55, 0x55, 0x55, 0x55, 0xF0, 0xFF, 0x20, 0x00];
        data.extend_from_slice(&[0x00; 0x20]);
        assert!(matches!(
            parse_frame(&data),
            Err(ParseError::AddressOverflow { .. })
        ));
    }

    #[test]
    fn stop_at_next_sync_sequence() {
        let mut frame1 = make_frame(&[(0x0000, &[0x01, 0x00])]);
        let frame2 = make_frame(&[(0x1000, &[0xFF, 0x00])]);
        frame1.extend_from_slice(&frame2);

        let updates = parse_frame(&frame1).unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].address, 0x0000);
    }

    #[test]
    fn find_sync_positions_in_stream() {
        let mut stream = make_frame(&[(0x0000, &[0x01, 0x00])]);
        let frame2 = make_frame(&[(0x1000, &[0xFF, 0x00])]);
        stream.extend_from_slice(&frame2);

        let positions = find_sync_positions(&stream);
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0], 0);
    }

    #[test]
    fn dcs_bios_address_decode() {
        let addr = DcsBiosAddress::new(0x7400, 0x0100, 8);
        // Word value 0x0100: mask 0x0100 → 0x0100, shift 8 → 1
        assert_eq!(addr.decode(0x0100), 1);
        assert_eq!(addr.decode(0x0000), 0);
        assert_eq!(addr.max_value, 1);
    }

    #[test]
    fn dcs_bios_address_multi_bit() {
        // 3-bit value at bits 2..4 (mask 0x001C, shift 2)
        let addr = DcsBiosAddress::new(0x0000, 0x001C, 2);
        assert_eq!(addr.decode(0x0014), 5); // 0b10100 >> 2 = 5
        assert_eq!(addr.max_value, 7);
    }

    #[test]
    fn zero_length_update_is_valid() {
        // Zero-length updates can occur (no data after header)
        // But length must be even, and 0 is even → should produce a valid but empty update
        let frame = vec![
            0x55, 0x55, 0x55, 0x55, 0x00, 0x10, 0x00, 0x00, // addr=0x1000, len=0
            0x00, 0x20, 0x02, 0x00, 0xAA, 0xBB, // second real update
        ];
        let updates = parse_frame(&frame).unwrap();
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].address, 0x1000);
        assert!(updates[0].data.is_empty());
    }

    #[test]
    fn large_update_parses() {
        let data_payload = vec![0xAB; 256];
        let frame = make_frame(&[(0x0000, &data_payload)]);
        let updates = parse_frame(&frame).unwrap();
        assert_eq!(updates[0].data.len(), 256);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Fuzzy inputs to parse_frame must never panic.
        #[test]
        fn parse_frame_never_panics(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
            let _ = parse_frame(&data);
        }

        /// Valid frames with random even-length payloads always parse.
        #[test]
        fn valid_frames_always_parse(
            addr in (0u16..32768).prop_map(|a| a * 2),
            data_len in (0u8..64).prop_map(|l| (l as usize) * 2),
        ) {
            let payload = vec![0xAA; data_len];
            // Ensure address + length doesn't overflow address space
            let end = addr as u32 + data_len as u32;
            if end <= ADDRESS_SPACE_SIZE as u32 {
                let mut frame = vec![0x55, 0x55, 0x55, 0x55];
                frame.extend_from_slice(&addr.to_le_bytes());
                frame.extend_from_slice(&(data_len as u16).to_le_bytes());
                frame.extend_from_slice(&payload);

                // Zero-length updates produce a valid update with empty data
                let result = parse_frame(&frame);
                prop_assert!(result.is_ok(), "Failed for addr=0x{:04X}, len={}", addr, data_len);
            }
        }

        /// DcsBiosAddress decode never panics for any word value.
        #[test]
        fn address_decode_never_panics(
            addr in any::<u16>(),
            mask in any::<u16>(),
            shift in 0u8..16,
            word in any::<u16>(),
        ) {
            let desc = DcsBiosAddress::new(addr, mask, shift);
            let _ = desc.decode(word);
        }

        /// find_sync_positions never panics.
        #[test]
        fn find_sync_never_panics(data in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = find_sync_positions(&data);
        }
    }
}
