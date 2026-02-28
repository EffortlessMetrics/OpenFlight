// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Low-level X-Plane UDP packet codec
//!
//! X-Plane communicates over UDP with fixed-format packets identified by a
//! 4-byte ASCII header followed by a NUL byte. This module handles:
//!
//! * **DATA** packets — groups of 8 floats keyed by an index
//! * **DREF** write commands — set a single dataref to a float value
//! * **CMND** commands — trigger a named X-Plane command
//! * **RREF** responses — dataref subscription responses (id + float)

use thiserror::Error;

/// Errors that can occur while parsing or building UDP packets.
#[derive(Error, Debug, PartialEq)]
pub enum ParseError {
    #[error("packet too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
    #[error("unrecognised header: {header:?}")]
    UnknownHeader { header: [u8; 4] },
    #[error("invalid DATA group at offset {offset}: packet truncated")]
    TruncatedDataGroup { offset: usize },
    #[error("invalid RREF entry at offset {offset}: packet truncated")]
    TruncatedRrefEntry { offset: usize },
}

/// A group of 8 float values keyed by an integer index (DATA packet row).
#[derive(Debug, Clone, PartialEq)]
pub struct DataGroup {
    pub index: u32,
    pub values: [f32; 8],
}

/// Parsed DATA packet consisting of one or more [`DataGroup`]s.
#[derive(Debug, Clone, PartialEq)]
pub struct XPlaneDataPacket {
    pub header: [u8; 4],
    pub data_groups: Vec<DataGroup>,
}

// ── Packet constants ────────────────────────────────────────────────
const HEADER_LEN: usize = 5; // 4-byte tag + NUL separator
const DATA_GROUP_LEN: usize = 4 + 8 * 4; // u32 index + 8 × f32
const RREF_ENTRY_LEN: usize = 4 + 4; // u32 id + f32 value
const DREF_PACKET_LEN: usize = HEADER_LEN + 4 + 500; // header + f32 + 500-byte path field
const CMND_PATH_LEN: usize = 500;

/// Parse an incoming DATA packet from X-Plane.
///
/// Format: `DATA\0` followed by N groups of `(u32 index, [f32; 8])`.
pub fn parse_data_packet(bytes: &[u8]) -> Result<XPlaneDataPacket, ParseError> {
    if bytes.len() < HEADER_LEN {
        return Err(ParseError::TooShort {
            expected: HEADER_LEN,
            actual: bytes.len(),
        });
    }

    let header: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
    if &header != b"DATA" {
        return Err(ParseError::UnknownHeader { header });
    }

    let payload = &bytes[HEADER_LEN..];
    if !payload.len().is_multiple_of(DATA_GROUP_LEN) {
        return Err(ParseError::TruncatedDataGroup {
            offset: HEADER_LEN + payload.len() - (payload.len() % DATA_GROUP_LEN),
        });
    }

    let mut groups = Vec::with_capacity(payload.len() / DATA_GROUP_LEN);
    for chunk in payload.chunks_exact(DATA_GROUP_LEN) {
        let index = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let mut values = [0.0f32; 8];
        for (i, val) in values.iter_mut().enumerate() {
            let off = 4 + i * 4;
            *val = f32::from_le_bytes([chunk[off], chunk[off + 1], chunk[off + 2], chunk[off + 3]]);
        }
        groups.push(DataGroup { index, values });
    }

    Ok(XPlaneDataPacket {
        header,
        data_groups: groups,
    })
}

/// Build a DREF write command to set a dataref to a float value.
///
/// Format: `DREF\0` + f32 (LE) + 500-byte NUL-padded path.
pub fn build_dref_command(path: &str, value: f32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(DREF_PACKET_LEN);
    buf.extend_from_slice(b"DREF\0");
    buf.extend_from_slice(&value.to_le_bytes());

    let path_bytes = path.as_bytes();
    let copy_len = path_bytes.len().min(500);
    buf.extend_from_slice(&path_bytes[..copy_len]);
    buf.resize(DREF_PACKET_LEN, 0);

    buf
}

/// Build a CMND command to trigger a named X-Plane command.
///
/// Format: `CMND\0` + NUL-terminated command path.
pub fn build_cmnd_command(command: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(HEADER_LEN + CMND_PATH_LEN);
    buf.extend_from_slice(b"CMND\0");

    let cmd_bytes = command.as_bytes();
    let copy_len = cmd_bytes.len().min(CMND_PATH_LEN);
    buf.extend_from_slice(&cmd_bytes[..copy_len]);
    // NUL terminator
    buf.push(0);
    buf
}

/// Parse an RREF response packet from X-Plane.
///
/// Format: `RREF\0` followed by N entries of `(u32 id, f32 value)`.
pub fn parse_rref_response(bytes: &[u8]) -> Result<Vec<(u32, f32)>, ParseError> {
    if bytes.len() < HEADER_LEN {
        return Err(ParseError::TooShort {
            expected: HEADER_LEN,
            actual: bytes.len(),
        });
    }

    let header: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
    if &header != b"RREF" {
        return Err(ParseError::UnknownHeader { header });
    }

    let payload = &bytes[HEADER_LEN..];
    if !payload.len().is_multiple_of(RREF_ENTRY_LEN) {
        return Err(ParseError::TruncatedRrefEntry {
            offset: HEADER_LEN + payload.len() - (payload.len() % RREF_ENTRY_LEN),
        });
    }

    let mut entries = Vec::with_capacity(payload.len() / RREF_ENTRY_LEN);
    for chunk in payload.chunks_exact(RREF_ENTRY_LEN) {
        let id = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let val = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        entries.push((id, val));
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ──────────────────────────────────────────────────────

    fn make_data_packet(groups: &[(u32, [f32; 8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"DATA\0");
        for (idx, vals) in groups {
            buf.extend_from_slice(&idx.to_le_bytes());
            for v in vals {
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        buf
    }

    fn make_rref_packet(entries: &[(u32, f32)]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"RREF\0");
        for (id, val) in entries {
            buf.extend_from_slice(&id.to_le_bytes());
            buf.extend_from_slice(&val.to_le_bytes());
        }
        buf
    }

    // ── DATA parsing ────────────────────────────────────────────────

    #[test]
    fn test_parse_single_data_group() {
        let vals = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let pkt = make_data_packet(&[(3, vals)]);
        let parsed = parse_data_packet(&pkt).unwrap();
        assert_eq!(parsed.header, *b"DATA");
        assert_eq!(parsed.data_groups.len(), 1);
        assert_eq!(parsed.data_groups[0].index, 3);
        assert_eq!(parsed.data_groups[0].values, vals);
    }

    #[test]
    fn test_parse_multiple_data_groups() {
        let v1 = [10.0; 8];
        let v2 = [20.0; 8];
        let pkt = make_data_packet(&[(0, v1), (17, v2)]);
        let parsed = parse_data_packet(&pkt).unwrap();
        assert_eq!(parsed.data_groups.len(), 2);
        assert_eq!(parsed.data_groups[0].index, 0);
        assert_eq!(parsed.data_groups[1].index, 17);
    }

    #[test]
    fn test_parse_data_empty_groups() {
        let pkt = b"DATA\0";
        let parsed = parse_data_packet(pkt).unwrap();
        assert!(parsed.data_groups.is_empty());
    }

    #[test]
    fn test_parse_data_too_short() {
        let err = parse_data_packet(b"DAT").unwrap_err();
        assert!(matches!(err, ParseError::TooShort { .. }));
    }

    #[test]
    fn test_parse_data_wrong_header() {
        let err = parse_data_packet(b"XXXX\0").unwrap_err();
        assert!(matches!(err, ParseError::UnknownHeader { .. }));
    }

    #[test]
    fn test_parse_data_truncated_group() {
        let mut pkt = make_data_packet(&[(1, [0.0; 8])]);
        pkt.truncate(pkt.len() - 1);
        let err = parse_data_packet(&pkt).unwrap_err();
        assert!(matches!(err, ParseError::TruncatedDataGroup { .. }));
    }

    // ── DREF build ──────────────────────────────────────────────────

    #[test]
    fn test_build_dref_command_length() {
        let buf = build_dref_command("sim/test", 1.5);
        assert_eq!(buf.len(), DREF_PACKET_LEN);
    }

    #[test]
    fn test_build_dref_command_header() {
        let buf = build_dref_command("sim/test", 0.0);
        assert_eq!(&buf[..5], b"DREF\0");
    }

    #[test]
    fn test_build_dref_command_value() {
        let buf = build_dref_command("sim/test", 42.0f32);
        let val = f32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
        assert!((val - 42.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_build_dref_command_path() {
        let path = "sim/cockpit2/controls/yoke_pitch_ratio";
        let buf = build_dref_command(path, 0.0);
        let path_region = &buf[9..9 + path.len()];
        assert_eq!(path_region, path.as_bytes());
        // Remaining bytes are NUL padding
        assert!(buf[9 + path.len()..].iter().all(|&b| b == 0));
    }

    // ── CMND build ──────────────────────────────────────────────────

    #[test]
    fn test_build_cmnd_command() {
        let buf = build_cmnd_command("sim/autopilot/heading_up");
        assert_eq!(&buf[..5], b"CMND\0");
        let cmd_end = buf[5..].iter().position(|&b| b == 0).unwrap();
        let cmd = std::str::from_utf8(&buf[5..5 + cmd_end]).unwrap();
        assert_eq!(cmd, "sim/autopilot/heading_up");
    }

    // ── RREF parsing ────────────────────────────────────────────────

    #[test]
    fn test_parse_rref_single_entry() {
        let pkt = make_rref_packet(&[(7, 123.456)]);
        let entries = parse_rref_response(&pkt).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, 7);
        assert!((entries[0].1 - 123.456).abs() < 0.001);
    }

    #[test]
    fn test_parse_rref_multiple_entries() {
        let pkt = make_rref_packet(&[(1, 10.0), (2, 20.0), (3, 30.0)]);
        let entries = parse_rref_response(&pkt).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_parse_rref_empty() {
        let pkt = b"RREF\0";
        let entries = parse_rref_response(pkt).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_rref_wrong_header() {
        let err = parse_rref_response(b"DATA\0").unwrap_err();
        assert!(matches!(err, ParseError::UnknownHeader { .. }));
    }

    #[test]
    fn test_parse_rref_truncated() {
        let mut pkt = make_rref_packet(&[(1, 1.0)]);
        pkt.truncate(pkt.len() - 1);
        let err = parse_rref_response(&pkt).unwrap_err();
        assert!(matches!(err, ParseError::TruncatedRrefEntry { .. }));
    }

    // ── round-trip / integration ────────────────────────────────────

    #[test]
    fn test_data_group_negative_and_special_values() {
        let vals = [
            -1.0f32,
            0.0,
            f32::INFINITY,
            f32::NEG_INFINITY,
            999.999,
            -0.001,
            0.0,
            0.0,
        ];
        let pkt = make_data_packet(&[(42, vals)]);
        let parsed = parse_data_packet(&pkt).unwrap();
        assert_eq!(parsed.data_groups[0].values[0], -1.0);
        assert!(parsed.data_groups[0].values[2].is_infinite());
    }
}
