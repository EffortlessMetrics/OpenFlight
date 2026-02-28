// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Custom UDP protocol for OpenFlight ↔ X-Plane plugin communication
//!
//! When the OpenFlight X-Plane plugin is installed, it opens a dedicated UDP
//! channel that supports higher-frequency data (up to 250 Hz vs the default
//! 20 Hz), richer message types, and version negotiation. If the plugin is
//! not installed the adapter falls back to the standard X-Plane UDP interface.
//!
//! ## Wire format
//!
//! Every message starts with a 4-byte magic tag (`OFXP`) followed by:
//!
//! | Offset | Size | Description |
//! |--------|------|-------------|
//! | 0 | 4 | Magic: `OFXP` |
//! | 4 | 1 | Protocol version (currently `1`) |
//! | 5 | 1 | Message type (see [`MessageType`]) |
//! | 6 | 2 | Payload length (LE u16) |
//! | 8 | N | Payload bytes |

use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{info, warn};

// ── Constants ────────────────────────────────────────────────────────

/// Wire-format magic bytes.
pub const MAGIC: &[u8; 4] = b"OFXP";
/// Current protocol version.
pub const PROTOCOL_VERSION: u8 = 1;
/// Minimum supported plugin version string.
pub const MIN_PLUGIN_VERSION: &str = "1.0.0";
/// Header length: magic(4) + version(1) + type(1) + length(2).
pub const HEADER_LEN: usize = 8;
/// Maximum payload size (64 KiB minus header).
pub const MAX_PAYLOAD: usize = 65527;
/// Default plugin UDP port.
pub const DEFAULT_PLUGIN_PORT: u16 = 49100;

// ── Message types ────────────────────────────────────────────────────

/// Numeric message type identifiers on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    /// Version handshake request (client → plugin).
    Handshake = 0x01,
    /// Version handshake response (plugin → client).
    HandshakeAck = 0x02,
    /// High-frequency dataref batch (plugin → client).
    DatarefBatch = 0x10,
    /// Subscribe to dataref updates (client → plugin).
    Subscribe = 0x11,
    /// Unsubscribe from dataref updates (client → plugin).
    Unsubscribe = 0x12,
    /// Set a dataref (client → plugin).
    SetDataref = 0x20,
    /// Execute a command (client → plugin).
    ExecuteCommand = 0x21,
    /// Heartbeat / keep-alive (bidirectional).
    Heartbeat = 0x30,
    /// Error response (plugin → client).
    Error = 0xFF,
}

impl MessageType {
    /// Try to parse a `u8` into a [`MessageType`].
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Handshake),
            0x02 => Some(Self::HandshakeAck),
            0x10 => Some(Self::DatarefBatch),
            0x11 => Some(Self::Subscribe),
            0x12 => Some(Self::Unsubscribe),
            0x20 => Some(Self::SetDataref),
            0x21 => Some(Self::ExecuteCommand),
            0x30 => Some(Self::Heartbeat),
            0xFF => Some(Self::Error),
            _ => None,
        }
    }
}

// ── High-level message enum ──────────────────────────────────────────

/// A decoded plugin protocol message.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginProtoMessage {
    /// Version negotiation request.
    Handshake {
        client_version: String,
        requested_frequency_hz: u16,
    },
    /// Version negotiation response.
    HandshakeAck {
        plugin_version: String,
        granted_frequency_hz: u16,
        capabilities: Vec<String>,
    },
    /// Batch of dataref values at high frequency.
    DatarefBatch {
        sequence: u32,
        timestamp_us: u64,
        entries: Vec<DatarefEntry>,
    },
    /// Subscribe to one or more datarefs.
    Subscribe { datarefs: Vec<SubscriptionRequest> },
    /// Unsubscribe from datarefs.
    Unsubscribe { dataref_ids: Vec<u32> },
    /// Set a dataref to a float value.
    SetDataref { path: String, value: f32 },
    /// Execute a named X-Plane command.
    ExecuteCommand { path: String },
    /// Keep-alive ping/pong.
    Heartbeat { timestamp_us: u64 },
    /// Error from the plugin.
    Error { code: u16, message: String },
}

/// A single dataref value inside a [`PluginProtoMessage::DatarefBatch`].
#[derive(Debug, Clone, PartialEq)]
pub struct DatarefEntry {
    pub id: u32,
    pub value: f32,
}

/// A subscription request for a single dataref.
#[derive(Debug, Clone, PartialEq)]
pub struct SubscriptionRequest {
    pub id: u32,
    pub path: String,
    pub frequency_hz: u16,
}

// ── Errors ───────────────────────────────────────────────────────────

/// Protocol encoding/decoding errors.
#[derive(Error, Debug, PartialEq)]
pub enum ProtocolError {
    #[error("bad magic: expected OFXP, got {got:?}")]
    BadMagic { got: [u8; 4] },
    #[error("unsupported protocol version {version} (supported: {PROTOCOL_VERSION})")]
    UnsupportedVersion { version: u8 },
    #[error("unknown message type 0x{type_byte:02X}")]
    UnknownMessageType { type_byte: u8 },
    #[error("buffer too short: need {need} bytes, have {have}")]
    BufferTooShort { need: usize, have: usize },
    #[error("payload too large: {size} bytes (max {MAX_PAYLOAD})")]
    PayloadTooLarge { size: usize },
    #[error("invalid UTF-8 in payload at offset {offset}")]
    InvalidUtf8 { offset: usize },
    #[error("truncated payload: expected {expected} bytes in field, got {actual}")]
    TruncatedPayload { expected: usize, actual: usize },
}

// ── Encoding ─────────────────────────────────────────────────────────

/// Encode a [`PluginProtoMessage`] into a byte buffer.
pub fn encode(msg: &PluginProtoMessage) -> Result<Vec<u8>, ProtocolError> {
    let (msg_type, payload) = encode_payload(msg)?;

    if payload.len() > MAX_PAYLOAD {
        return Err(ProtocolError::PayloadTooLarge {
            size: payload.len(),
        });
    }

    let mut buf = Vec::with_capacity(HEADER_LEN + payload.len());
    buf.extend_from_slice(MAGIC);
    buf.push(PROTOCOL_VERSION);
    buf.push(msg_type as u8);
    buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    buf.extend_from_slice(&payload);
    Ok(buf)
}

fn encode_payload(msg: &PluginProtoMessage) -> Result<(MessageType, Vec<u8>), ProtocolError> {
    match msg {
        PluginProtoMessage::Handshake {
            client_version,
            requested_frequency_hz,
        } => {
            let mut p = Vec::new();
            write_string(&mut p, client_version);
            p.extend_from_slice(&requested_frequency_hz.to_le_bytes());
            Ok((MessageType::Handshake, p))
        }
        PluginProtoMessage::HandshakeAck {
            plugin_version,
            granted_frequency_hz,
            capabilities,
        } => {
            let mut p = Vec::new();
            write_string(&mut p, plugin_version);
            p.extend_from_slice(&granted_frequency_hz.to_le_bytes());
            p.extend_from_slice(&(capabilities.len() as u16).to_le_bytes());
            for cap in capabilities {
                write_string(&mut p, cap);
            }
            Ok((MessageType::HandshakeAck, p))
        }
        PluginProtoMessage::DatarefBatch {
            sequence,
            timestamp_us,
            entries,
        } => {
            let mut p = Vec::new();
            p.extend_from_slice(&sequence.to_le_bytes());
            p.extend_from_slice(&timestamp_us.to_le_bytes());
            p.extend_from_slice(&(entries.len() as u32).to_le_bytes());
            for e in entries {
                p.extend_from_slice(&e.id.to_le_bytes());
                p.extend_from_slice(&e.value.to_le_bytes());
            }
            Ok((MessageType::DatarefBatch, p))
        }
        PluginProtoMessage::Subscribe { datarefs } => {
            let mut p = Vec::new();
            p.extend_from_slice(&(datarefs.len() as u16).to_le_bytes());
            for sub in datarefs {
                p.extend_from_slice(&sub.id.to_le_bytes());
                write_string(&mut p, &sub.path);
                p.extend_from_slice(&sub.frequency_hz.to_le_bytes());
            }
            Ok((MessageType::Subscribe, p))
        }
        PluginProtoMessage::Unsubscribe { dataref_ids } => {
            let mut p = Vec::new();
            p.extend_from_slice(&(dataref_ids.len() as u16).to_le_bytes());
            for id in dataref_ids {
                p.extend_from_slice(&id.to_le_bytes());
            }
            Ok((MessageType::Unsubscribe, p))
        }
        PluginProtoMessage::SetDataref { path, value } => {
            let mut p = Vec::new();
            write_string(&mut p, path);
            p.extend_from_slice(&value.to_le_bytes());
            Ok((MessageType::SetDataref, p))
        }
        PluginProtoMessage::ExecuteCommand { path } => {
            let mut p = Vec::new();
            write_string(&mut p, path);
            Ok((MessageType::ExecuteCommand, p))
        }
        PluginProtoMessage::Heartbeat { timestamp_us } => {
            let p = timestamp_us.to_le_bytes().to_vec();
            Ok((MessageType::Heartbeat, p))
        }
        PluginProtoMessage::Error { code, message } => {
            let mut p = Vec::new();
            p.extend_from_slice(&code.to_le_bytes());
            write_string(&mut p, message);
            Ok((MessageType::Error, p))
        }
    }
}

// ── Decoding ─────────────────────────────────────────────────────────

/// Decode a byte buffer into a [`PluginProtoMessage`].
pub fn decode(buf: &[u8]) -> Result<PluginProtoMessage, ProtocolError> {
    if buf.len() < HEADER_LEN {
        return Err(ProtocolError::BufferTooShort {
            need: HEADER_LEN,
            have: buf.len(),
        });
    }

    let magic: [u8; 4] = [buf[0], buf[1], buf[2], buf[3]];
    if &magic != MAGIC {
        return Err(ProtocolError::BadMagic { got: magic });
    }

    let version = buf[4];
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion { version });
    }

    let msg_type = MessageType::from_u8(buf[5])
        .ok_or(ProtocolError::UnknownMessageType { type_byte: buf[5] })?;

    let payload_len = u16::from_le_bytes([buf[6], buf[7]]) as usize;
    let total = HEADER_LEN + payload_len;
    if buf.len() < total {
        return Err(ProtocolError::BufferTooShort {
            need: total,
            have: buf.len(),
        });
    }

    let payload = &buf[HEADER_LEN..total];
    decode_payload(msg_type, payload)
}

fn decode_payload(
    msg_type: MessageType,
    payload: &[u8],
) -> Result<PluginProtoMessage, ProtocolError> {
    match msg_type {
        MessageType::Handshake => {
            let (client_version, off) = read_string(payload, 0)?;
            ensure_remaining(payload, off, 2)?;
            let freq = u16::from_le_bytes([payload[off], payload[off + 1]]);
            Ok(PluginProtoMessage::Handshake {
                client_version,
                requested_frequency_hz: freq,
            })
        }
        MessageType::HandshakeAck => {
            let (plugin_version, mut off) = read_string(payload, 0)?;
            ensure_remaining(payload, off, 2)?;
            let freq = u16::from_le_bytes([payload[off], payload[off + 1]]);
            off += 2;
            ensure_remaining(payload, off, 2)?;
            let cap_count = u16::from_le_bytes([payload[off], payload[off + 1]]) as usize;
            off += 2;
            let mut capabilities = Vec::with_capacity(cap_count);
            for _ in 0..cap_count {
                let (s, new_off) = read_string(payload, off)?;
                capabilities.push(s);
                off = new_off;
            }
            Ok(PluginProtoMessage::HandshakeAck {
                plugin_version,
                granted_frequency_hz: freq,
                capabilities,
            })
        }
        MessageType::DatarefBatch => {
            ensure_remaining(payload, 0, 4 + 8 + 4)?;
            let sequence = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let timestamp_us = u64::from_le_bytes([
                payload[4],
                payload[5],
                payload[6],
                payload[7],
                payload[8],
                payload[9],
                payload[10],
                payload[11],
            ]);
            let count =
                u32::from_le_bytes([payload[12], payload[13], payload[14], payload[15]]) as usize;
            let mut entries = Vec::with_capacity(count);
            let mut off = 16;
            for _ in 0..count {
                ensure_remaining(payload, off, 8)?;
                let id = u32::from_le_bytes([
                    payload[off],
                    payload[off + 1],
                    payload[off + 2],
                    payload[off + 3],
                ]);
                let value = f32::from_le_bytes([
                    payload[off + 4],
                    payload[off + 5],
                    payload[off + 6],
                    payload[off + 7],
                ]);
                entries.push(DatarefEntry { id, value });
                off += 8;
            }
            Ok(PluginProtoMessage::DatarefBatch {
                sequence,
                timestamp_us,
                entries,
            })
        }
        MessageType::Subscribe => {
            ensure_remaining(payload, 0, 2)?;
            let count = u16::from_le_bytes([payload[0], payload[1]]) as usize;
            let mut datarefs = Vec::with_capacity(count);
            let mut off = 2;
            for _ in 0..count {
                ensure_remaining(payload, off, 4)?;
                let id = u32::from_le_bytes([
                    payload[off],
                    payload[off + 1],
                    payload[off + 2],
                    payload[off + 3],
                ]);
                off += 4;
                let (path, new_off) = read_string(payload, off)?;
                off = new_off;
                ensure_remaining(payload, off, 2)?;
                let freq = u16::from_le_bytes([payload[off], payload[off + 1]]);
                off += 2;
                datarefs.push(SubscriptionRequest {
                    id,
                    path,
                    frequency_hz: freq,
                });
            }
            Ok(PluginProtoMessage::Subscribe { datarefs })
        }
        MessageType::Unsubscribe => {
            ensure_remaining(payload, 0, 2)?;
            let count = u16::from_le_bytes([payload[0], payload[1]]) as usize;
            let mut ids = Vec::with_capacity(count);
            let mut off = 2;
            for _ in 0..count {
                ensure_remaining(payload, off, 4)?;
                let id = u32::from_le_bytes([
                    payload[off],
                    payload[off + 1],
                    payload[off + 2],
                    payload[off + 3],
                ]);
                ids.push(id);
                off += 4;
            }
            Ok(PluginProtoMessage::Unsubscribe { dataref_ids: ids })
        }
        MessageType::SetDataref => {
            let (path, off) = read_string(payload, 0)?;
            ensure_remaining(payload, off, 4)?;
            let value = f32::from_le_bytes([
                payload[off],
                payload[off + 1],
                payload[off + 2],
                payload[off + 3],
            ]);
            Ok(PluginProtoMessage::SetDataref { path, value })
        }
        MessageType::ExecuteCommand => {
            let (path, _) = read_string(payload, 0)?;
            Ok(PluginProtoMessage::ExecuteCommand { path })
        }
        MessageType::Heartbeat => {
            ensure_remaining(payload, 0, 8)?;
            let ts = u64::from_le_bytes([
                payload[0], payload[1], payload[2], payload[3], payload[4], payload[5], payload[6],
                payload[7],
            ]);
            Ok(PluginProtoMessage::Heartbeat { timestamp_us: ts })
        }
        MessageType::Error => {
            ensure_remaining(payload, 0, 2)?;
            let code = u16::from_le_bytes([payload[0], payload[1]]);
            let (message, _) = read_string(payload, 2)?;
            Ok(PluginProtoMessage::Error { code, message })
        }
    }
}

// ── Plugin discovery ─────────────────────────────────────────────────

/// State of plugin discovery and version negotiation.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginDiscoveryState {
    /// No plugin detected; using standard UDP fallback.
    NotDetected,
    /// Handshake sent, waiting for response.
    Discovering,
    /// Plugin connected with negotiated parameters.
    Connected {
        plugin_version: String,
        frequency_hz: u16,
        capabilities: Vec<String>,
    },
    /// Plugin was connected but is now unavailable.
    Disconnected { reason: String },
}

/// Manages plugin discovery, version negotiation, and fallback logic.
pub struct PluginDiscovery {
    state: PluginDiscoveryState,
    last_heartbeat_sent: Option<Instant>,
    last_heartbeat_recv: Option<Instant>,
    heartbeat_interval: Duration,
    heartbeat_timeout: Duration,
}

impl PluginDiscovery {
    pub fn new() -> Self {
        Self {
            state: PluginDiscoveryState::NotDetected,
            last_heartbeat_sent: None,
            last_heartbeat_recv: None,
            heartbeat_interval: Duration::from_secs(2),
            heartbeat_timeout: Duration::from_secs(10),
        }
    }

    /// Current state.
    pub fn state(&self) -> &PluginDiscoveryState {
        &self.state
    }

    /// Whether the plugin is actively connected.
    pub fn is_connected(&self) -> bool {
        matches!(self.state, PluginDiscoveryState::Connected { .. })
    }

    /// Whether we should fall back to standard UDP.
    pub fn should_use_standard_udp(&self) -> bool {
        matches!(
            self.state,
            PluginDiscoveryState::NotDetected | PluginDiscoveryState::Disconnected { .. }
        )
    }

    /// Build a handshake message to initiate plugin discovery.
    pub fn build_handshake(&mut self, version: &str, frequency_hz: u16) -> PluginProtoMessage {
        self.state = PluginDiscoveryState::Discovering;
        PluginProtoMessage::Handshake {
            client_version: version.to_owned(),
            requested_frequency_hz: frequency_hz,
        }
    }

    /// Process an incoming message and update discovery state.
    pub fn process_message(&mut self, msg: &PluginProtoMessage) {
        match msg {
            PluginProtoMessage::HandshakeAck {
                plugin_version,
                granted_frequency_hz,
                capabilities,
            } => {
                info!(
                    version = %plugin_version,
                    freq = granted_frequency_hz,
                    "plugin connected"
                );
                self.state = PluginDiscoveryState::Connected {
                    plugin_version: plugin_version.clone(),
                    frequency_hz: *granted_frequency_hz,
                    capabilities: capabilities.clone(),
                };
                self.last_heartbeat_recv = Some(Instant::now());
            }
            PluginProtoMessage::Heartbeat { .. } => {
                self.last_heartbeat_recv = Some(Instant::now());
            }
            PluginProtoMessage::Error { code, message } => {
                warn!(code, message = %message, "plugin error");
                if *code >= 0x8000 {
                    // Fatal error range — disconnect
                    self.state = PluginDiscoveryState::Disconnected {
                        reason: message.clone(),
                    };
                }
            }
            _ => {}
        }
    }

    /// Build a heartbeat message if the interval has elapsed.
    pub fn maybe_heartbeat(
        &mut self,
        now: Instant,
        timestamp_us: u64,
    ) -> Option<PluginProtoMessage> {
        if !self.is_connected() {
            return None;
        }
        let should_send = match self.last_heartbeat_sent {
            Some(t) => now.duration_since(t) >= self.heartbeat_interval,
            None => true,
        };
        if should_send {
            self.last_heartbeat_sent = Some(now);
            Some(PluginProtoMessage::Heartbeat { timestamp_us })
        } else {
            None
        }
    }

    /// Check if the plugin has timed out (no heartbeat response).
    pub fn check_timeout(&mut self, now: Instant) {
        if !self.is_connected() {
            return;
        }
        if let Some(last) = self.last_heartbeat_recv {
            if now.duration_since(last) > self.heartbeat_timeout {
                warn!("plugin heartbeat timeout");
                self.state = PluginDiscoveryState::Disconnected {
                    reason: "heartbeat timeout".to_owned(),
                };
            }
        }
    }

    /// Reset to initial state (e.g. for reconnection).
    pub fn reset(&mut self) {
        self.state = PluginDiscoveryState::NotDetected;
        self.last_heartbeat_sent = None;
        self.last_heartbeat_recv = None;
    }
}

impl Default for PluginDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

// ── Wire helpers ─────────────────────────────────────────────────────

/// Write a length-prefixed UTF-8 string (u16 LE length + bytes).
fn write_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(bytes);
}

/// Read a length-prefixed UTF-8 string starting at `offset`.
fn read_string(buf: &[u8], offset: usize) -> Result<(String, usize), ProtocolError> {
    ensure_remaining(buf, offset, 2)?;
    let len = u16::from_le_bytes([buf[offset], buf[offset + 1]]) as usize;
    let start = offset + 2;
    ensure_remaining(buf, start, len)?;
    let s = std::str::from_utf8(&buf[start..start + len])
        .map_err(|_| ProtocolError::InvalidUtf8 { offset: start })?;
    Ok((s.to_owned(), start + len))
}

/// Ensure at least `need` bytes are available at `offset`.
fn ensure_remaining(buf: &[u8], offset: usize, need: usize) -> Result<(), ProtocolError> {
    if offset + need > buf.len() {
        return Err(ProtocolError::TruncatedPayload {
            expected: need,
            actual: buf.len().saturating_sub(offset),
        });
    }
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Header / framing ─────────────────────────────────────────────

    #[test]
    fn header_starts_with_magic() {
        let msg = PluginProtoMessage::Heartbeat { timestamp_us: 42 };
        let buf = encode(&msg).unwrap();
        assert_eq!(&buf[..4], MAGIC);
    }

    #[test]
    fn header_contains_version() {
        let msg = PluginProtoMessage::Heartbeat { timestamp_us: 0 };
        let buf = encode(&msg).unwrap();
        assert_eq!(buf[4], PROTOCOL_VERSION);
    }

    #[test]
    fn header_contains_message_type() {
        let msg = PluginProtoMessage::Heartbeat { timestamp_us: 0 };
        let buf = encode(&msg).unwrap();
        assert_eq!(buf[5], MessageType::Heartbeat as u8);
    }

    #[test]
    fn header_payload_length_is_correct() {
        let msg = PluginProtoMessage::Heartbeat { timestamp_us: 0 };
        let buf = encode(&msg).unwrap();
        let payload_len = u16::from_le_bytes([buf[6], buf[7]]) as usize;
        assert_eq!(payload_len, buf.len() - HEADER_LEN);
    }

    // ── Round-trip encode/decode ─────────────────────────────────────

    #[test]
    fn roundtrip_handshake() {
        let msg = PluginProtoMessage::Handshake {
            client_version: "1.2.3".to_owned(),
            requested_frequency_hz: 250,
        };
        let buf = encode(&msg).unwrap();
        let decoded = decode(&buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn roundtrip_handshake_ack() {
        let msg = PluginProtoMessage::HandshakeAck {
            plugin_version: "1.0.0".to_owned(),
            granted_frequency_hz: 100,
            capabilities: vec!["datarefs".to_owned(), "commands".to_owned()],
        };
        let buf = encode(&msg).unwrap();
        let decoded = decode(&buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn roundtrip_dataref_batch() {
        let msg = PluginProtoMessage::DatarefBatch {
            sequence: 42,
            timestamp_us: 1_000_000,
            entries: vec![
                DatarefEntry { id: 1, value: 0.5 },
                DatarefEntry { id: 2, value: -1.0 },
                DatarefEntry {
                    id: 3,
                    value: 100.25,
                },
            ],
        };
        let buf = encode(&msg).unwrap();
        let decoded = decode(&buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn roundtrip_subscribe() {
        let msg = PluginProtoMessage::Subscribe {
            datarefs: vec![
                SubscriptionRequest {
                    id: 1,
                    path: "sim/airspeed".to_owned(),
                    frequency_hz: 50,
                },
                SubscriptionRequest {
                    id: 2,
                    path: "sim/altitude".to_owned(),
                    frequency_hz: 10,
                },
            ],
        };
        let buf = encode(&msg).unwrap();
        let decoded = decode(&buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn roundtrip_unsubscribe() {
        let msg = PluginProtoMessage::Unsubscribe {
            dataref_ids: vec![1, 2, 3],
        };
        let buf = encode(&msg).unwrap();
        let decoded = decode(&buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn roundtrip_set_dataref() {
        let msg = PluginProtoMessage::SetDataref {
            path: "sim/joystick/yoke_pitch_ratio".to_owned(),
            value: 0.75,
        };
        let buf = encode(&msg).unwrap();
        let decoded = decode(&buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn roundtrip_execute_command() {
        let msg = PluginProtoMessage::ExecuteCommand {
            path: "sim/flight_controls/flaps_down".to_owned(),
        };
        let buf = encode(&msg).unwrap();
        let decoded = decode(&buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn roundtrip_heartbeat() {
        let msg = PluginProtoMessage::Heartbeat {
            timestamp_us: 123_456_789,
        };
        let buf = encode(&msg).unwrap();
        let decoded = decode(&buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn roundtrip_error() {
        let msg = PluginProtoMessage::Error {
            code: 0x8001,
            message: "dataref not found".to_owned(),
        };
        let buf = encode(&msg).unwrap();
        let decoded = decode(&buf).unwrap();
        assert_eq!(decoded, msg);
    }

    // ── Decode error cases ───────────────────────────────────────────

    #[test]
    fn decode_too_short() {
        let err = decode(b"OFXP").unwrap_err();
        assert!(matches!(err, ProtocolError::BufferTooShort { .. }));
    }

    #[test]
    fn decode_bad_magic() {
        let buf = [b'X', b'X', b'X', b'X', 1, 0x30, 0, 0];
        let err = decode(&buf).unwrap_err();
        assert!(matches!(err, ProtocolError::BadMagic { .. }));
    }

    #[test]
    fn decode_unsupported_version() {
        let buf = [b'O', b'F', b'X', b'P', 99, 0x30, 0, 0];
        let err = decode(&buf).unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::UnsupportedVersion { version: 99 }
        ));
    }

    #[test]
    fn decode_unknown_message_type() {
        let buf = [b'O', b'F', b'X', b'P', 1, 0x99, 0, 0];
        let err = decode(&buf).unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::UnknownMessageType { type_byte: 0x99 }
        ));
    }

    #[test]
    fn decode_truncated_payload() {
        // Header says 8 bytes of payload, but only 2 provided
        let buf = [b'O', b'F', b'X', b'P', 1, 0x30, 8, 0, 0, 0];
        let err = decode(&buf).unwrap_err();
        assert!(matches!(err, ProtocolError::BufferTooShort { .. }));
    }

    // ── MessageType ──────────────────────────────────────────────────

    #[test]
    fn message_type_roundtrip() {
        let types = [
            MessageType::Handshake,
            MessageType::HandshakeAck,
            MessageType::DatarefBatch,
            MessageType::Subscribe,
            MessageType::Unsubscribe,
            MessageType::SetDataref,
            MessageType::ExecuteCommand,
            MessageType::Heartbeat,
            MessageType::Error,
        ];
        for t in types {
            assert_eq!(MessageType::from_u8(t as u8), Some(t));
        }
    }

    #[test]
    fn message_type_unknown_returns_none() {
        assert_eq!(MessageType::from_u8(0x00), None);
        assert_eq!(MessageType::from_u8(0x77), None);
    }

    // ── Empty collections ────────────────────────────────────────────

    #[test]
    fn roundtrip_empty_dataref_batch() {
        let msg = PluginProtoMessage::DatarefBatch {
            sequence: 0,
            timestamp_us: 0,
            entries: vec![],
        };
        let buf = encode(&msg).unwrap();
        assert_eq!(decode(&buf).unwrap(), msg);
    }

    #[test]
    fn roundtrip_empty_subscribe() {
        let msg = PluginProtoMessage::Subscribe { datarefs: vec![] };
        let buf = encode(&msg).unwrap();
        assert_eq!(decode(&buf).unwrap(), msg);
    }

    #[test]
    fn roundtrip_empty_unsubscribe() {
        let msg = PluginProtoMessage::Unsubscribe {
            dataref_ids: vec![],
        };
        let buf = encode(&msg).unwrap();
        assert_eq!(decode(&buf).unwrap(), msg);
    }

    #[test]
    fn roundtrip_handshake_ack_no_capabilities() {
        let msg = PluginProtoMessage::HandshakeAck {
            plugin_version: "1.0.0".to_owned(),
            granted_frequency_hz: 20,
            capabilities: vec![],
        };
        let buf = encode(&msg).unwrap();
        assert_eq!(decode(&buf).unwrap(), msg);
    }

    // ── Plugin discovery state machine ───────────────────────────────

    #[test]
    fn discovery_starts_not_detected() {
        let pd = PluginDiscovery::new();
        assert_eq!(*pd.state(), PluginDiscoveryState::NotDetected);
        assert!(!pd.is_connected());
        assert!(pd.should_use_standard_udp());
    }

    #[test]
    fn discovery_handshake_transitions_to_discovering() {
        let mut pd = PluginDiscovery::new();
        pd.build_handshake("1.0.0", 250);
        assert_eq!(*pd.state(), PluginDiscoveryState::Discovering);
    }

    #[test]
    fn discovery_ack_transitions_to_connected() {
        let mut pd = PluginDiscovery::new();
        pd.build_handshake("1.0.0", 250);

        let ack = PluginProtoMessage::HandshakeAck {
            plugin_version: "1.0.0".to_owned(),
            granted_frequency_hz: 100,
            capabilities: vec!["datarefs".to_owned()],
        };
        pd.process_message(&ack);
        assert!(pd.is_connected());
        assert!(!pd.should_use_standard_udp());
    }

    #[test]
    fn discovery_fatal_error_disconnects() {
        let mut pd = PluginDiscovery::new();
        pd.build_handshake("1.0.0", 250);

        let ack = PluginProtoMessage::HandshakeAck {
            plugin_version: "1.0.0".to_owned(),
            granted_frequency_hz: 100,
            capabilities: vec![],
        };
        pd.process_message(&ack);
        assert!(pd.is_connected());

        let err = PluginProtoMessage::Error {
            code: 0x8001,
            message: "fatal".to_owned(),
        };
        pd.process_message(&err);
        assert!(!pd.is_connected());
        assert!(pd.should_use_standard_udp());
    }

    #[test]
    fn discovery_heartbeat_timeout() {
        let mut pd = PluginDiscovery::new();
        pd.build_handshake("1.0.0", 250);

        let ack = PluginProtoMessage::HandshakeAck {
            plugin_version: "1.0.0".to_owned(),
            granted_frequency_hz: 100,
            capabilities: vec![],
        };
        pd.process_message(&ack);
        assert!(pd.is_connected());

        // Simulate time passing beyond timeout
        pd.last_heartbeat_recv = Some(Instant::now() - Duration::from_secs(20));
        pd.check_timeout(Instant::now());
        assert!(!pd.is_connected());
    }

    #[test]
    fn discovery_reset() {
        let mut pd = PluginDiscovery::new();
        pd.build_handshake("1.0.0", 250);
        pd.reset();
        assert_eq!(*pd.state(), PluginDiscoveryState::NotDetected);
    }

    #[test]
    fn discovery_maybe_heartbeat_when_connected() {
        let mut pd = PluginDiscovery::new();
        pd.build_handshake("1.0.0", 250);

        let ack = PluginProtoMessage::HandshakeAck {
            plugin_version: "1.0.0".to_owned(),
            granted_frequency_hz: 100,
            capabilities: vec![],
        };
        pd.process_message(&ack);

        // First call should produce a heartbeat
        let hb = pd.maybe_heartbeat(Instant::now(), 999);
        assert!(hb.is_some());
        assert!(matches!(
            hb.unwrap(),
            PluginProtoMessage::Heartbeat { timestamp_us: 999 }
        ));

        // Immediately after — interval not elapsed
        let hb2 = pd.maybe_heartbeat(Instant::now(), 1000);
        assert!(hb2.is_none());
    }

    #[test]
    fn discovery_no_heartbeat_when_not_connected() {
        let mut pd = PluginDiscovery::new();
        let hb = pd.maybe_heartbeat(Instant::now(), 0);
        assert!(hb.is_none());
    }

    #[test]
    fn default_discovery() {
        let pd = PluginDiscovery::default();
        assert_eq!(*pd.state(), PluginDiscoveryState::NotDetected);
    }

    // ── Wire helper tests ────────────────────────────────────────────

    #[test]
    fn write_read_string_roundtrip() {
        let mut buf = Vec::new();
        write_string(&mut buf, "hello world");
        let (s, off) = read_string(&buf, 0).unwrap();
        assert_eq!(s, "hello world");
        assert_eq!(off, buf.len());
    }

    #[test]
    fn write_read_empty_string() {
        let mut buf = Vec::new();
        write_string(&mut buf, "");
        let (s, off) = read_string(&buf, 0).unwrap();
        assert_eq!(s, "");
        assert_eq!(off, 2); // just the length prefix
    }

    #[test]
    fn read_string_truncated_length() {
        let buf = [0u8; 1]; // only 1 byte, need 2 for length
        let err = read_string(&buf, 0).unwrap_err();
        assert!(matches!(err, ProtocolError::TruncatedPayload { .. }));
    }

    #[test]
    fn read_string_truncated_body() {
        let buf = [5, 0, b'h', b'i']; // says 5 bytes but only 2
        let err = read_string(&buf, 0).unwrap_err();
        assert!(matches!(err, ProtocolError::TruncatedPayload { .. }));
    }
}
