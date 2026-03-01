// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Panel protocol message framing, encoding, and dispatch engine.
//!
//! Provides a standardized wire format for panel communication:
//!
//! - [`PanelMessage`] — typed panel events and commands
//! - [`MessageFrame`] — wire-level framing with CRC-16 checksum
//! - [`MessageDispatcher`] — pre-allocated handler routing table

/// Maximum payload size in bytes.
pub const MAX_PAYLOAD: usize = 256;

/// Maximum number of distinct message types the dispatcher supports.
const MAX_MESSAGE_TYPES: usize = 16;

// Wire message type identifiers.
const MSG_LED_UPDATE: u8 = 0x01;
const MSG_DISPLAY_UPDATE: u8 = 0x02;
const MSG_BUTTON_EVENT: u8 = 0x03;
const MSG_ENCODER_EVENT: u8 = 0x04;
const MSG_SWITCH_EVENT: u8 = 0x05;
const MSG_HEARTBEAT: u8 = 0x06;

/// Frame header size: frame_id(2) + message_type(1) + payload_len(2).
const FRAME_HEADER_SIZE: usize = 5;

/// CRC-16 checksum field size.
const CHECKSUM_SIZE: usize = 2;

/// Errors produced during frame encoding, decoding, or dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// Payload exceeds [`MAX_PAYLOAD`] bytes.
    PayloadTooLarge,
    /// CRC-16 checksum mismatch.
    InvalidChecksum,
    /// Unrecognised message type identifier.
    InvalidMessageType(u8),
    /// Not enough bytes to decode a complete frame.
    InsufficientData,
    /// Payload bytes do not match the expected layout.
    InvalidPayload,
    /// No handler registered for the message type.
    NoHandler(u8),
}

// ─── LedColor ────────────────────────────────────────────────────────────────

/// LED colour state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedColor {
    Off,
    Green,
    Amber,
    Red,
}

impl LedColor {
    fn as_u8(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Green => 1,
            Self::Amber => 2,
            Self::Red => 3,
        }
    }

    fn from_u8(val: u8) -> Result<Self, FrameError> {
        match val {
            0 => Ok(Self::Off),
            1 => Ok(Self::Green),
            2 => Ok(Self::Amber),
            3 => Ok(Self::Red),
            _ => Err(FrameError::InvalidPayload),
        }
    }
}

// ─── DisplayFormat ───────────────────────────────────────────────────────────

/// Display formatting hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayFormat {
    /// Raw character data.
    Raw,
    /// Numeric with a decimal-point position.
    Numeric { decimal_pos: u8 },
    /// Frequency display (e.g. 118.50).
    Frequency,
}

impl DisplayFormat {
    fn encode(self) -> (u8, u8) {
        match self {
            Self::Raw => (0x00, 0x00),
            Self::Numeric { decimal_pos } => (0x01, decimal_pos),
            Self::Frequency => (0x02, 0x00),
        }
    }

    fn decode(fmt_type: u8, fmt_param: u8) -> Result<Self, FrameError> {
        match fmt_type {
            0x00 => Ok(Self::Raw),
            0x01 => Ok(Self::Numeric {
                decimal_pos: fmt_param,
            }),
            0x02 => Ok(Self::Frequency),
            _ => Err(FrameError::InvalidPayload),
        }
    }
}

// ─── PanelMessage ────────────────────────────────────────────────────────────

/// Standardized message types for panel communication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelMessage {
    /// Update an LED indicator.
    LedUpdate {
        panel_id: u8,
        led_id: u8,
        state: bool,
        color: LedColor,
    },
    /// Update a segment display.
    DisplayUpdate {
        panel_id: u8,
        display_id: u8,
        value: [u8; 8],
        format: DisplayFormat,
    },
    /// Button press / release event.
    ButtonEvent {
        panel_id: u8,
        button_id: u8,
        pressed: bool,
        timestamp: u32,
    },
    /// Rotary encoder tick event.
    EncoderEvent {
        panel_id: u8,
        encoder_id: u8,
        delta: i16,
        timestamp: u32,
    },
    /// Switch position change event.
    SwitchEvent {
        panel_id: u8,
        switch_id: u8,
        position: u8,
        timestamp: u32,
    },
    /// Connection heartbeat / keep-alive.
    Heartbeat { panel_id: u8, sequence: u16 },
}

impl PanelMessage {
    /// Returns the wire message-type identifier.
    #[must_use]
    pub fn message_type_id(&self) -> u8 {
        match self {
            Self::LedUpdate { .. } => MSG_LED_UPDATE,
            Self::DisplayUpdate { .. } => MSG_DISPLAY_UPDATE,
            Self::ButtonEvent { .. } => MSG_BUTTON_EVENT,
            Self::EncoderEvent { .. } => MSG_ENCODER_EVENT,
            Self::SwitchEvent { .. } => MSG_SWITCH_EVENT,
            Self::Heartbeat { .. } => MSG_HEARTBEAT,
        }
    }

    /// Encodes the message fields into `buf`, returning the byte count written.
    pub fn encode_payload(&self, buf: &mut [u8; MAX_PAYLOAD]) -> u16 {
        match self {
            Self::LedUpdate {
                panel_id,
                led_id,
                state,
                color,
            } => {
                buf[0] = *panel_id;
                buf[1] = *led_id;
                buf[2] = u8::from(*state);
                buf[3] = color.as_u8();
                4
            }
            Self::DisplayUpdate {
                panel_id,
                display_id,
                value,
                format,
            } => {
                buf[0] = *panel_id;
                buf[1] = *display_id;
                buf[2..10].copy_from_slice(value);
                let (ft, fp) = format.encode();
                buf[10] = ft;
                buf[11] = fp;
                12
            }
            Self::ButtonEvent {
                panel_id,
                button_id,
                pressed,
                timestamp,
            } => {
                buf[0] = *panel_id;
                buf[1] = *button_id;
                buf[2] = u8::from(*pressed);
                buf[3..7].copy_from_slice(&timestamp.to_le_bytes());
                7
            }
            Self::EncoderEvent {
                panel_id,
                encoder_id,
                delta,
                timestamp,
            } => {
                buf[0] = *panel_id;
                buf[1] = *encoder_id;
                buf[2..4].copy_from_slice(&delta.to_le_bytes());
                buf[4..8].copy_from_slice(&timestamp.to_le_bytes());
                8
            }
            Self::SwitchEvent {
                panel_id,
                switch_id,
                position,
                timestamp,
            } => {
                buf[0] = *panel_id;
                buf[1] = *switch_id;
                buf[2] = *position;
                buf[3..7].copy_from_slice(&timestamp.to_le_bytes());
                7
            }
            Self::Heartbeat { panel_id, sequence } => {
                buf[0] = *panel_id;
                buf[1..3].copy_from_slice(&sequence.to_le_bytes());
                3
            }
        }
    }

    /// Decodes a message from its type ID and payload bytes.
    pub fn decode_payload(message_type: u8, payload: &[u8], len: u16) -> Result<Self, FrameError> {
        let len = len as usize;
        if payload.len() < len {
            return Err(FrameError::InvalidPayload);
        }
        match message_type {
            MSG_LED_UPDATE => {
                if len < 4 {
                    return Err(FrameError::InvalidPayload);
                }
                Ok(Self::LedUpdate {
                    panel_id: payload[0],
                    led_id: payload[1],
                    state: payload[2] != 0,
                    color: LedColor::from_u8(payload[3])?,
                })
            }
            MSG_DISPLAY_UPDATE => {
                if len < 12 {
                    return Err(FrameError::InvalidPayload);
                }
                let mut value = [0u8; 8];
                value.copy_from_slice(&payload[2..10]);
                Ok(Self::DisplayUpdate {
                    panel_id: payload[0],
                    display_id: payload[1],
                    value,
                    format: DisplayFormat::decode(payload[10], payload[11])?,
                })
            }
            MSG_BUTTON_EVENT => {
                if len < 7 {
                    return Err(FrameError::InvalidPayload);
                }
                Ok(Self::ButtonEvent {
                    panel_id: payload[0],
                    button_id: payload[1],
                    pressed: payload[2] != 0,
                    timestamp: u32::from_le_bytes([payload[3], payload[4], payload[5], payload[6]]),
                })
            }
            MSG_ENCODER_EVENT => {
                if len < 8 {
                    return Err(FrameError::InvalidPayload);
                }
                Ok(Self::EncoderEvent {
                    panel_id: payload[0],
                    encoder_id: payload[1],
                    delta: i16::from_le_bytes([payload[2], payload[3]]),
                    timestamp: u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]),
                })
            }
            MSG_SWITCH_EVENT => {
                if len < 7 {
                    return Err(FrameError::InvalidPayload);
                }
                Ok(Self::SwitchEvent {
                    panel_id: payload[0],
                    switch_id: payload[1],
                    position: payload[2],
                    timestamp: u32::from_le_bytes([payload[3], payload[4], payload[5], payload[6]]),
                })
            }
            MSG_HEARTBEAT => {
                if len < 3 {
                    return Err(FrameError::InvalidPayload);
                }
                Ok(Self::Heartbeat {
                    panel_id: payload[0],
                    sequence: u16::from_le_bytes([payload[1], payload[2]]),
                })
            }
            other => Err(FrameError::InvalidMessageType(other)),
        }
    }
}

// ─── MessageFrame ────────────────────────────────────────────────────────────

/// Wire-format message frame with header, payload, and CRC-16 checksum.
///
/// Wire layout: `[frame_id:u16 LE][msg_type:u8][payload_len:u16 LE][payload…][crc16:u16 LE]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageFrame {
    /// Monotonically increasing frame identifier.
    frame_id: u16,
    /// Message type byte (see [`PanelMessage::message_type_id`]).
    message_type: u8,
    /// Number of valid bytes in `payload`.
    payload_len: u16,
    /// Fixed-size payload buffer.
    payload: [u8; MAX_PAYLOAD],
    /// CRC-16/CCITT-FALSE computed over header + payload.
    checksum: u16,
}

impl MessageFrame {
    /// Creates a new frame with validated payload.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError::PayloadTooLarge`] if `payload` exceeds [`MAX_PAYLOAD`].
    pub fn new(frame_id: u16, message_type: u8, payload: &[u8]) -> Result<Self, FrameError> {
        if payload.len() > MAX_PAYLOAD {
            return Err(FrameError::PayloadTooLarge);
        }
        let mut buf = [0u8; MAX_PAYLOAD];
        buf[..payload.len()].copy_from_slice(payload);
        let mut frame = Self {
            frame_id,
            message_type,
            payload_len: payload.len() as u16,
            payload: buf,
            checksum: 0,
        };
        frame.checksum = frame.compute_checksum();
        Ok(frame)
    }

    /// Returns the frame identifier.
    #[must_use]
    pub fn frame_id(&self) -> u16 {
        self.frame_id
    }

    /// Returns the message type byte.
    #[must_use]
    pub fn message_type(&self) -> u8 {
        self.message_type
    }

    /// Returns the number of valid payload bytes.
    #[must_use]
    pub fn payload_len(&self) -> u16 {
        self.payload_len
    }

    /// Returns the payload buffer.
    #[must_use]
    pub fn payload(&self) -> &[u8; MAX_PAYLOAD] {
        &self.payload
    }

    /// Returns the checksum value.
    #[must_use]
    pub fn checksum(&self) -> u16 {
        self.checksum
    }

    /// Creates a frame from a [`PanelMessage`], encoding payload and computing the checksum.
    #[must_use]
    pub fn from_message(frame_id: u16, msg: &PanelMessage) -> Self {
        let mut payload = [0u8; MAX_PAYLOAD];
        let payload_len = msg.encode_payload(&mut payload);
        let message_type = msg.message_type_id();

        let mut frame = Self {
            frame_id,
            message_type,
            payload_len,
            payload,
            checksum: 0,
        };
        frame.checksum = frame.compute_checksum();
        frame
    }

    /// Serialises the frame to wire bytes.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError::PayloadTooLarge`] if `payload_len` exceeds [`MAX_PAYLOAD`].
    pub fn encode(&self) -> Result<Vec<u8>, FrameError> {
        if self.payload_len as usize > MAX_PAYLOAD {
            return Err(FrameError::PayloadTooLarge);
        }
        let total = FRAME_HEADER_SIZE + self.payload_len as usize + CHECKSUM_SIZE;
        let mut buf = Vec::with_capacity(total);
        buf.extend_from_slice(&self.frame_id.to_le_bytes());
        buf.push(self.message_type);
        buf.extend_from_slice(&self.payload_len.to_le_bytes());
        buf.extend_from_slice(&self.payload[..self.payload_len as usize]);
        buf.extend_from_slice(&self.checksum.to_le_bytes());
        Ok(buf)
    }

    /// Deserialises a frame from wire bytes, validating the checksum.
    pub fn decode(data: &[u8]) -> Result<Self, FrameError> {
        if data.len() < FRAME_HEADER_SIZE + CHECKSUM_SIZE {
            return Err(FrameError::InsufficientData);
        }

        let frame_id = u16::from_le_bytes([data[0], data[1]]);
        let message_type = data[2];
        let payload_len = u16::from_le_bytes([data[3], data[4]]);

        if payload_len as usize > MAX_PAYLOAD {
            return Err(FrameError::PayloadTooLarge);
        }

        let expected = FRAME_HEADER_SIZE + payload_len as usize + CHECKSUM_SIZE;
        if data.len() < expected {
            return Err(FrameError::InsufficientData);
        }

        let mut payload = [0u8; MAX_PAYLOAD];
        let plen = payload_len as usize;
        payload[..plen].copy_from_slice(&data[FRAME_HEADER_SIZE..FRAME_HEADER_SIZE + plen]);

        let ck_off = FRAME_HEADER_SIZE + plen;
        let checksum = u16::from_le_bytes([data[ck_off], data[ck_off + 1]]);

        let frame = Self {
            frame_id,
            message_type,
            payload_len,
            payload,
            checksum,
        };

        if frame.compute_checksum() != checksum {
            return Err(FrameError::InvalidChecksum);
        }

        Ok(frame)
    }

    /// Returns `true` if the stored checksum matches the computed value.
    #[must_use]
    pub fn verify_checksum(&self) -> bool {
        self.compute_checksum() == self.checksum
    }

    /// Decodes the payload into a [`PanelMessage`].
    pub fn to_message(&self) -> Result<PanelMessage, FrameError> {
        PanelMessage::decode_payload(self.message_type, &self.payload, self.payload_len)
    }

    /// CRC-16/CCITT-FALSE over header + payload.
    fn compute_checksum(&self) -> u16 {
        let plen = self.payload_len as usize;
        // Safety: clamp to MAX_PAYLOAD to prevent OOB if payload_len is invalid.
        let plen = if plen > MAX_PAYLOAD { MAX_PAYLOAD } else { plen };
        let total = FRAME_HEADER_SIZE + plen;
        let mut buf = [0u8; FRAME_HEADER_SIZE + MAX_PAYLOAD];
        buf[0..2].copy_from_slice(&self.frame_id.to_le_bytes());
        buf[2] = self.message_type;
        buf[3..5].copy_from_slice(&self.payload_len.to_le_bytes());
        buf[5..5 + plen].copy_from_slice(&self.payload[..plen]);
        crc16_ccitt(&buf[..total])
    }
}

// ─── MessageDispatcher ───────────────────────────────────────────────────────

/// Handler function pointer type.
pub type MessageHandler = fn(&PanelMessage);

/// Routes decoded messages to registered handlers by message type.
///
/// The handler table is pre-allocated at construction — dispatch performs no
/// dynamic allocation.
pub struct MessageDispatcher {
    handlers: [Option<MessageHandler>; MAX_MESSAGE_TYPES],
}

impl MessageDispatcher {
    /// Creates a dispatcher with no registered handlers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: [None; MAX_MESSAGE_TYPES],
        }
    }

    /// Registers a handler for a message type.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError::InvalidMessageType`] if `message_type` ≥ 16.
    pub fn register(
        &mut self,
        message_type: u8,
        handler: MessageHandler,
    ) -> Result<(), FrameError> {
        let idx = message_type as usize;
        if idx >= MAX_MESSAGE_TYPES {
            return Err(FrameError::InvalidMessageType(message_type));
        }
        self.handlers[idx] = Some(handler);
        Ok(())
    }

    /// Dispatches a message to its registered handler.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError::NoHandler`] if no handler is registered for the type.
    pub fn dispatch(&self, msg: &PanelMessage) -> Result<(), FrameError> {
        let idx = msg.message_type_id() as usize;
        match self.handlers[idx] {
            Some(handler) => {
                handler(msg);
                Ok(())
            }
            None => Err(FrameError::NoHandler(msg.message_type_id())),
        }
    }

    /// Returns `true` if a handler is registered for the given type.
    #[must_use]
    pub fn has_handler(&self, message_type: u8) -> bool {
        let idx = message_type as usize;
        idx < MAX_MESSAGE_TYPES && self.handlers[idx].is_some()
    }
}

impl Default for MessageDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ─── CRC-16 ──────────────────────────────────────────────────────────────────

/// CRC-16/CCITT-FALSE: polynomial 0x1021, init 0xFFFF.
fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    // ── Round-trip helpers ────────────────────────────────────────────────────

    fn roundtrip(msg: &PanelMessage) {
        let frame = MessageFrame::from_message(1, msg);
        let wire = frame.encode().unwrap();
        let decoded_frame = MessageFrame::decode(&wire).unwrap();
        let decoded_msg = decoded_frame.to_message().unwrap();
        assert_eq!(*msg, decoded_msg);
    }

    // ── Round-trip encode/decode for every message type ──────────────────────

    #[test]
    fn roundtrip_led_update() {
        roundtrip(&PanelMessage::LedUpdate {
            panel_id: 1,
            led_id: 5,
            state: true,
            color: LedColor::Green,
        });
    }

    #[test]
    fn roundtrip_display_update() {
        roundtrip(&PanelMessage::DisplayUpdate {
            panel_id: 2,
            display_id: 0,
            value: [0x31, 0x31, 0x38, 0x2E, 0x35, 0x30, 0x00, 0x00],
            format: DisplayFormat::Frequency,
        });
    }

    #[test]
    fn roundtrip_button_event() {
        roundtrip(&PanelMessage::ButtonEvent {
            panel_id: 3,
            button_id: 7,
            pressed: true,
            timestamp: 123_456,
        });
    }

    #[test]
    fn roundtrip_encoder_event() {
        roundtrip(&PanelMessage::EncoderEvent {
            panel_id: 4,
            encoder_id: 1,
            delta: -3,
            timestamp: 999_999,
        });
    }

    #[test]
    fn roundtrip_switch_event() {
        roundtrip(&PanelMessage::SwitchEvent {
            panel_id: 5,
            switch_id: 2,
            position: 3,
            timestamp: 500_000,
        });
    }

    #[test]
    fn roundtrip_heartbeat() {
        roundtrip(&PanelMessage::Heartbeat {
            panel_id: 0,
            sequence: 42,
        });
    }

    // ── Checksum tests ───────────────────────────────────────────────────────

    #[test]
    fn checksum_is_valid() {
        let msg = PanelMessage::Heartbeat {
            panel_id: 1,
            sequence: 100,
        };
        let frame = MessageFrame::from_message(1, &msg);
        assert!(frame.verify_checksum());
    }

    #[test]
    fn corrupted_payload_detected() {
        let msg = PanelMessage::ButtonEvent {
            panel_id: 1,
            button_id: 2,
            pressed: true,
            timestamp: 1000,
        };
        let frame = MessageFrame::from_message(1, &msg);
        let mut wire = frame.encode().unwrap();
        wire[FRAME_HEADER_SIZE] ^= 0xFF;
        assert_eq!(
            MessageFrame::decode(&wire),
            Err(FrameError::InvalidChecksum)
        );
    }

    #[test]
    fn corrupted_checksum_detected() {
        let msg = PanelMessage::Heartbeat {
            panel_id: 1,
            sequence: 1,
        };
        let frame = MessageFrame::from_message(1, &msg);
        let mut wire = frame.encode().unwrap();
        let last = wire.len() - 1;
        wire[last] ^= 0x01;
        assert_eq!(
            MessageFrame::decode(&wire),
            Err(FrameError::InvalidChecksum)
        );
    }

    // ── Error handling ───────────────────────────────────────────────────────

    #[test]
    fn unknown_message_type_rejected() {
        let result = PanelMessage::decode_payload(0xFF, &[0; 8], 8);
        assert_eq!(result, Err(FrameError::InvalidMessageType(0xFF)));
    }

    #[test]
    fn max_payload_size_enforced() {
        let mut data = vec![0u8; FRAME_HEADER_SIZE + CHECKSUM_SIZE];
        data[2] = MSG_HEARTBEAT;
        let too_large = (MAX_PAYLOAD as u16) + 1;
        data[3..5].copy_from_slice(&too_large.to_le_bytes());
        assert_eq!(
            MessageFrame::decode(&data),
            Err(FrameError::PayloadTooLarge)
        );
    }

    #[test]
    fn insufficient_data_rejected() {
        assert_eq!(
            MessageFrame::decode(&[0x00, 0x01]),
            Err(FrameError::InsufficientData)
        );
    }

    #[test]
    fn zero_length_heartbeat_payload_rejected() {
        let result = PanelMessage::decode_payload(MSG_HEARTBEAT, &[], 0);
        assert_eq!(result, Err(FrameError::InvalidPayload));
    }

    // ── Frame identity ───────────────────────────────────────────────────────

    #[test]
    fn frame_id_preserved() {
        let msg = PanelMessage::Heartbeat {
            panel_id: 0,
            sequence: 1,
        };
        let frame = MessageFrame::from_message(0xBEEF, &msg);
        let wire = frame.encode().unwrap();
        let decoded = MessageFrame::decode(&wire).unwrap();
        assert_eq!(decoded.frame_id(), 0xBEEF);
    }

    // ── Enum variant coverage ────────────────────────────────────────────────

    #[test]
    fn all_led_colors_roundtrip() {
        for color in [
            LedColor::Off,
            LedColor::Green,
            LedColor::Amber,
            LedColor::Red,
        ] {
            let msg = PanelMessage::LedUpdate {
                panel_id: 1,
                led_id: 0,
                state: true,
                color,
            };
            roundtrip(&msg);
        }
    }

    #[test]
    fn display_format_variants_roundtrip() {
        for format in [
            DisplayFormat::Raw,
            DisplayFormat::Numeric { decimal_pos: 3 },
            DisplayFormat::Frequency,
        ] {
            let msg = PanelMessage::DisplayUpdate {
                panel_id: 1,
                display_id: 0,
                value: [0; 8],
                format,
            };
            roundtrip(&msg);
        }
    }

    // ── Dispatcher tests ─────────────────────────────────────────────────────

    fn noop_handler(_msg: &PanelMessage) {}

    #[test]
    fn dispatcher_routes_registered_handler() {
        let mut d = MessageDispatcher::new();
        d.register(MSG_HEARTBEAT, noop_handler).unwrap();
        let msg = PanelMessage::Heartbeat {
            panel_id: 0,
            sequence: 1,
        };
        assert!(d.dispatch(&msg).is_ok());
    }

    #[test]
    fn dispatcher_no_handler_returns_error() {
        let d = MessageDispatcher::new();
        let msg = PanelMessage::Heartbeat {
            panel_id: 0,
            sequence: 1,
        };
        assert_eq!(d.dispatch(&msg), Err(FrameError::NoHandler(MSG_HEARTBEAT)));
    }

    static LED_CALLED: AtomicBool = AtomicBool::new(false);
    static BTN_CALLED: AtomicBool = AtomicBool::new(false);

    fn led_handler(_: &PanelMessage) {
        LED_CALLED.store(true, Ordering::SeqCst);
    }
    fn btn_handler(_: &PanelMessage) {
        BTN_CALLED.store(true, Ordering::SeqCst);
    }

    #[test]
    fn dispatcher_routes_to_correct_handler() {
        LED_CALLED.store(false, Ordering::SeqCst);
        BTN_CALLED.store(false, Ordering::SeqCst);

        let mut d = MessageDispatcher::new();
        d.register(MSG_LED_UPDATE, led_handler).unwrap();
        d.register(MSG_BUTTON_EVENT, btn_handler).unwrap();

        let msg = PanelMessage::LedUpdate {
            panel_id: 1,
            led_id: 0,
            state: true,
            color: LedColor::Green,
        };
        d.dispatch(&msg).unwrap();

        assert!(LED_CALLED.load(Ordering::SeqCst));
        assert!(!BTN_CALLED.load(Ordering::SeqCst));
    }

    #[test]
    fn dispatcher_has_handler_check() {
        let mut d = MessageDispatcher::new();
        assert!(!d.has_handler(MSG_LED_UPDATE));
        d.register(MSG_LED_UPDATE, noop_handler).unwrap();
        assert!(d.has_handler(MSG_LED_UPDATE));
        assert!(!d.has_handler(MSG_HEARTBEAT));
    }

    #[test]
    fn dispatcher_register_out_of_range() {
        let mut d = MessageDispatcher::new();
        assert_eq!(
            d.register(250, noop_handler),
            Err(FrameError::InvalidMessageType(250))
        );
    }

    // ── Payload bounds-check tests ───────────────────────────────────────────

    #[test]
    fn test_decode_short_payload_returns_error() {
        // Payload slice is shorter than what `len` claims — must not panic.
        let result = PanelMessage::decode_payload(MSG_BUTTON_EVENT, &[1, 2], 7);
        assert_eq!(result, Err(FrameError::InvalidPayload));
    }

    #[test]
    fn test_decode_empty_payload_returns_error() {
        let result = PanelMessage::decode_payload(MSG_LED_UPDATE, &[], 4);
        assert_eq!(result, Err(FrameError::InvalidPayload));
    }

    #[test]
    fn test_decode_inflated_len_returns_error() {
        // 3-byte payload but len says 12 — must be caught before indexing.
        let result = PanelMessage::decode_payload(MSG_DISPLAY_UPDATE, &[0, 0, 0], 12);
        assert_eq!(result, Err(FrameError::InvalidPayload));
    }
}
