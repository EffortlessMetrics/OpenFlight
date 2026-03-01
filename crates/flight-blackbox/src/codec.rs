// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Compact binary codec for blackbox records.
//!
//! Encodes and decodes [`Record`] values into a compact binary format suitable
//! for high-frequency recording paths. The format uses a version byte for
//! forward compatibility and fixed-size encodings to avoid heap allocation.

/// Current codec format version.
pub const CODEC_VERSION: u8 = 1;

// ── Record tag bytes ─────────────────────────────────────────────────

const TAG_AXIS_FRAME: u8 = 0x01;
const TAG_BUS_EVENT: u8 = 0x02;
const TAG_TIMING_MARK: u8 = 0x03;
const TAG_ANNOTATION: u8 = 0x04;

/// Maximum length of an inline annotation message.
pub const ANNOTATION_MAX: usize = 64;

// ── Record types ─────────────────────────────────────────────────────

/// A single axis frame captured at 250 Hz.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisFrame {
    pub timestamp_ns: u64,
    pub axis_id: u16,
    pub raw: f64,
    pub processed: f64,
}

/// An event from the bus (profile change, fault, etc.).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BusEvent {
    pub timestamp_ns: u64,
    pub event_code: u16,
    pub payload: [u8; 8],
    pub payload_len: u8,
}

/// A timing mark for synchronisation / jitter analysis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimingMark {
    pub timestamp_ns: u64,
    pub sequence: u32,
    pub delta_ns: u32,
}

/// A human-readable annotation injected into the recording stream.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Annotation {
    pub timestamp_ns: u64,
    pub msg: [u8; ANNOTATION_MAX],
    pub msg_len: u8,
}

impl Annotation {
    /// View the message as a UTF-8 string slice.
    pub fn message(&self) -> &str {
        std::str::from_utf8(&self.msg[..self.msg_len as usize]).unwrap_or("<invalid>")
    }
}

/// Tagged union of all record types stored in the blackbox.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Record {
    AxisFrame(AxisFrame),
    BusEvent(BusEvent),
    TimingMark(TimingMark),
    Annotation(Annotation),
}

impl Record {
    /// Returns the monotonic timestamp (ns) common to all variants.
    pub fn timestamp_ns(&self) -> u64 {
        match self {
            Record::AxisFrame(r) => r.timestamp_ns,
            Record::BusEvent(r) => r.timestamp_ns,
            Record::TimingMark(r) => r.timestamp_ns,
            Record::Annotation(r) => r.timestamp_ns,
        }
    }
}

// ── Codec errors ─────────────────────────────────────────────────────

/// Errors returned by the codec.
#[derive(Debug, Clone, PartialEq)]
pub enum CodecError {
    /// The buffer is too small to hold the encoded record.
    BufferTooSmall,
    /// The input buffer is too short to decode a full record.
    UnexpectedEof,
    /// Unknown record tag byte.
    UnknownTag(u8),
    /// Unsupported codec version.
    UnsupportedVersion(u8),
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::BufferTooSmall => write!(f, "output buffer too small"),
            CodecError::UnexpectedEof => write!(f, "unexpected end of input"),
            CodecError::UnknownTag(t) => write!(f, "unknown record tag: {t:#04x}"),
            CodecError::UnsupportedVersion(v) => write!(f, "unsupported codec version: {v}"),
        }
    }
}

impl std::error::Error for CodecError {}

// ── Wire format ──────────────────────────────────────────────────────
//
// Every encoded record is:
//   [version: u8] [tag: u8] [payload...]
//
// Payload layouts (all little-endian):
//   AxisFrame:   timestamp_ns(8) axis_id(2) raw(8) processed(8)  = 26 bytes
//   BusEvent:    timestamp_ns(8) event_code(2) payload_len(1) payload(8) = 19 bytes
//   TimingMark:  timestamp_ns(8) sequence(4) delta_ns(4)        = 16 bytes
//   Annotation:  timestamp_ns(8) msg_len(1) msg(64)             = 73 bytes

/// Maximum encoded size of any single record (version + tag + largest payload).
pub const MAX_ENCODED_SIZE: usize = 2 + 73; // Annotation is largest

/// Encode a [`Record`] into `buf`. Returns the number of bytes written.
pub fn encode(record: &Record, buf: &mut [u8]) -> Result<usize, CodecError> {
    match record {
        Record::AxisFrame(r) => encode_axis_frame(r, buf),
        Record::BusEvent(r) => encode_bus_event(r, buf),
        Record::TimingMark(r) => encode_timing_mark(r, buf),
        Record::Annotation(r) => encode_annotation(r, buf),
    }
}

/// Decode a [`Record`] from `buf`. Returns the record and number of bytes consumed.
pub fn decode(buf: &[u8]) -> Result<(Record, usize), CodecError> {
    if buf.len() < 2 {
        return Err(CodecError::UnexpectedEof);
    }
    let version = buf[0];
    if version != CODEC_VERSION {
        return Err(CodecError::UnsupportedVersion(version));
    }
    let tag = buf[1];
    match tag {
        TAG_AXIS_FRAME => decode_axis_frame(buf),
        TAG_BUS_EVENT => decode_bus_event(buf),
        TAG_TIMING_MARK => decode_timing_mark(buf),
        TAG_ANNOTATION => decode_annotation(buf),
        other => Err(CodecError::UnknownTag(other)),
    }
}

// ── Encode helpers ───────────────────────────────────────────────────

fn write_header(buf: &mut [u8], tag: u8) {
    buf[0] = CODEC_VERSION;
    buf[1] = tag;
}

fn encode_axis_frame(r: &AxisFrame, buf: &mut [u8]) -> Result<usize, CodecError> {
    const SIZE: usize = 2 + 8 + 2 + 8 + 8; // 28
    if buf.len() < SIZE {
        return Err(CodecError::BufferTooSmall);
    }
    write_header(buf, TAG_AXIS_FRAME);
    buf[2..10].copy_from_slice(&r.timestamp_ns.to_le_bytes());
    buf[10..12].copy_from_slice(&r.axis_id.to_le_bytes());
    buf[12..20].copy_from_slice(&r.raw.to_le_bytes());
    buf[20..28].copy_from_slice(&r.processed.to_le_bytes());
    Ok(SIZE)
}

fn encode_bus_event(r: &BusEvent, buf: &mut [u8]) -> Result<usize, CodecError> {
    const SIZE: usize = 2 + 8 + 2 + 1 + 8; // 21
    if buf.len() < SIZE {
        return Err(CodecError::BufferTooSmall);
    }
    write_header(buf, TAG_BUS_EVENT);
    buf[2..10].copy_from_slice(&r.timestamp_ns.to_le_bytes());
    buf[10..12].copy_from_slice(&r.event_code.to_le_bytes());
    buf[12] = r.payload_len;
    buf[13..21].copy_from_slice(&r.payload);
    Ok(SIZE)
}

fn encode_timing_mark(r: &TimingMark, buf: &mut [u8]) -> Result<usize, CodecError> {
    const SIZE: usize = 2 + 8 + 4 + 4; // 18
    if buf.len() < SIZE {
        return Err(CodecError::BufferTooSmall);
    }
    write_header(buf, TAG_TIMING_MARK);
    buf[2..10].copy_from_slice(&r.timestamp_ns.to_le_bytes());
    buf[10..14].copy_from_slice(&r.sequence.to_le_bytes());
    buf[14..18].copy_from_slice(&r.delta_ns.to_le_bytes());
    Ok(SIZE)
}

fn encode_annotation(r: &Annotation, buf: &mut [u8]) -> Result<usize, CodecError> {
    const SIZE: usize = 2 + 8 + 1 + ANNOTATION_MAX; // 75
    if buf.len() < SIZE {
        return Err(CodecError::BufferTooSmall);
    }
    write_header(buf, TAG_ANNOTATION);
    buf[2..10].copy_from_slice(&r.timestamp_ns.to_le_bytes());
    buf[10] = r.msg_len;
    buf[11..11 + ANNOTATION_MAX].copy_from_slice(&r.msg);
    Ok(SIZE)
}

// ── Decode helpers ───────────────────────────────────────────────────

fn decode_axis_frame(buf: &[u8]) -> Result<(Record, usize), CodecError> {
    const SIZE: usize = 28;
    if buf.len() < SIZE {
        return Err(CodecError::UnexpectedEof);
    }
    let timestamp_ns = u64::from_le_bytes(buf[2..10].try_into().unwrap());
    let axis_id = u16::from_le_bytes(buf[10..12].try_into().unwrap());
    let raw = f64::from_le_bytes(buf[12..20].try_into().unwrap());
    let processed = f64::from_le_bytes(buf[20..28].try_into().unwrap());
    Ok((
        Record::AxisFrame(AxisFrame {
            timestamp_ns,
            axis_id,
            raw,
            processed,
        }),
        SIZE,
    ))
}

fn decode_bus_event(buf: &[u8]) -> Result<(Record, usize), CodecError> {
    const SIZE: usize = 21;
    if buf.len() < SIZE {
        return Err(CodecError::UnexpectedEof);
    }
    let timestamp_ns = u64::from_le_bytes(buf[2..10].try_into().unwrap());
    let event_code = u16::from_le_bytes(buf[10..12].try_into().unwrap());
    let payload_len = buf[12];
    let mut payload = [0u8; 8];
    payload.copy_from_slice(&buf[13..21]);
    Ok((
        Record::BusEvent(BusEvent {
            timestamp_ns,
            event_code,
            payload_len,
            payload,
        }),
        SIZE,
    ))
}

fn decode_timing_mark(buf: &[u8]) -> Result<(Record, usize), CodecError> {
    const SIZE: usize = 18;
    if buf.len() < SIZE {
        return Err(CodecError::UnexpectedEof);
    }
    let timestamp_ns = u64::from_le_bytes(buf[2..10].try_into().unwrap());
    let sequence = u32::from_le_bytes(buf[10..14].try_into().unwrap());
    let delta_ns = u32::from_le_bytes(buf[14..18].try_into().unwrap());
    Ok((
        Record::TimingMark(TimingMark {
            timestamp_ns,
            sequence,
            delta_ns,
        }),
        SIZE,
    ))
}

fn decode_annotation(buf: &[u8]) -> Result<(Record, usize), CodecError> {
    const SIZE: usize = 2 + 8 + 1 + ANNOTATION_MAX; // 75
    if buf.len() < SIZE {
        return Err(CodecError::UnexpectedEof);
    }
    let timestamp_ns = u64::from_le_bytes(buf[2..10].try_into().unwrap());
    let msg_len = buf[10];
    let mut msg = [0u8; ANNOTATION_MAX];
    msg.copy_from_slice(&buf[11..11 + ANNOTATION_MAX]);
    Ok((
        Record::Annotation(Annotation {
            timestamp_ns,
            msg_len,
            msg,
        }),
        SIZE,
    ))
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_frame_roundtrip() {
        let record = Record::AxisFrame(AxisFrame {
            timestamp_ns: 1_000_000,
            axis_id: 3,
            raw: 0.5,
            processed: 0.75,
        });
        let mut buf = [0u8; MAX_ENCODED_SIZE];
        let n = encode(&record, &mut buf).unwrap();
        let (decoded, consumed) = decode(&buf[..n]).unwrap();
        assert_eq!(consumed, n);
        assert_eq!(decoded, record);
    }

    #[test]
    fn bus_event_roundtrip() {
        let mut payload = [0u8; 8];
        payload[0] = 0xDE;
        payload[1] = 0xAD;
        let record = Record::BusEvent(BusEvent {
            timestamp_ns: 2_000_000,
            event_code: 42,
            payload,
            payload_len: 2,
        });
        let mut buf = [0u8; MAX_ENCODED_SIZE];
        let n = encode(&record, &mut buf).unwrap();
        let (decoded, consumed) = decode(&buf[..n]).unwrap();
        assert_eq!(consumed, n);
        assert_eq!(decoded, record);
    }

    #[test]
    fn timing_mark_roundtrip() {
        let record = Record::TimingMark(TimingMark {
            timestamp_ns: 3_000_000,
            sequence: 100,
            delta_ns: 4000,
        });
        let mut buf = [0u8; MAX_ENCODED_SIZE];
        let n = encode(&record, &mut buf).unwrap();
        let (decoded, consumed) = decode(&buf[..n]).unwrap();
        assert_eq!(consumed, n);
        assert_eq!(decoded, record);
    }

    #[test]
    fn annotation_roundtrip() {
        let mut msg = [0u8; ANNOTATION_MAX];
        let text = b"profile changed";
        msg[..text.len()].copy_from_slice(text);
        let record = Record::Annotation(Annotation {
            timestamp_ns: 4_000_000,
            msg,
            msg_len: text.len() as u8,
        });
        let mut buf = [0u8; MAX_ENCODED_SIZE];
        let n = encode(&record, &mut buf).unwrap();
        let (decoded, consumed) = decode(&buf[..n]).unwrap();
        assert_eq!(consumed, n);
        assert_eq!(decoded, record);
        if let Record::Annotation(a) = decoded {
            assert_eq!(a.message(), "profile changed");
        } else {
            panic!("expected Annotation");
        }
    }

    #[test]
    fn version_byte_is_first() {
        let record = Record::TimingMark(TimingMark {
            timestamp_ns: 0,
            sequence: 0,
            delta_ns: 0,
        });
        let mut buf = [0u8; MAX_ENCODED_SIZE];
        encode(&record, &mut buf).unwrap();
        assert_eq!(buf[0], CODEC_VERSION);
    }

    #[test]
    fn unsupported_version_rejected() {
        let mut buf = [0u8; MAX_ENCODED_SIZE];
        buf[0] = 0xFF; // bad version
        buf[1] = TAG_AXIS_FRAME;
        let err = decode(&buf).unwrap_err();
        assert_eq!(err, CodecError::UnsupportedVersion(0xFF));
    }

    #[test]
    fn unknown_tag_rejected() {
        let mut buf = [0u8; MAX_ENCODED_SIZE];
        buf[0] = CODEC_VERSION;
        buf[1] = 0xFF; // bad tag
        let err = decode(&buf).unwrap_err();
        assert_eq!(err, CodecError::UnknownTag(0xFF));
    }

    #[test]
    fn buffer_too_small_on_encode() {
        let record = Record::AxisFrame(AxisFrame {
            timestamp_ns: 0,
            axis_id: 0,
            raw: 0.0,
            processed: 0.0,
        });
        let mut buf = [0u8; 2]; // too small
        let err = encode(&record, &mut buf).unwrap_err();
        assert_eq!(err, CodecError::BufferTooSmall);
    }

    #[test]
    fn unexpected_eof_on_decode() {
        let buf = [CODEC_VERSION, TAG_AXIS_FRAME, 0]; // truncated
        let err = decode(&buf).unwrap_err();
        assert_eq!(err, CodecError::UnexpectedEof);
    }

    #[test]
    fn empty_input_decode_error() {
        let err = decode(&[]).unwrap_err();
        assert_eq!(err, CodecError::UnexpectedEof);
    }

    #[test]
    fn all_record_types_have_distinct_tags() {
        let records = [
            Record::AxisFrame(AxisFrame {
                timestamp_ns: 0,
                axis_id: 0,
                raw: 0.0,
                processed: 0.0,
            }),
            Record::BusEvent(BusEvent {
                timestamp_ns: 0,
                event_code: 0,
                payload: [0; 8],
                payload_len: 0,
            }),
            Record::TimingMark(TimingMark {
                timestamp_ns: 0,
                sequence: 0,
                delta_ns: 0,
            }),
            Record::Annotation(Annotation {
                timestamp_ns: 0,
                msg: [0; ANNOTATION_MAX],
                msg_len: 0,
            }),
        ];
        let mut tags = Vec::new();
        for r in &records {
            let mut buf = [0u8; MAX_ENCODED_SIZE];
            encode(r, &mut buf).unwrap();
            tags.push(buf[1]);
        }
        tags.sort();
        tags.dedup();
        assert_eq!(tags.len(), 4, "each record type must have a unique tag");
    }

    #[test]
    fn record_timestamp_accessor() {
        let r = Record::BusEvent(BusEvent {
            timestamp_ns: 12345,
            event_code: 0,
            payload: [0; 8],
            payload_len: 0,
        });
        assert_eq!(r.timestamp_ns(), 12345);
    }
}
