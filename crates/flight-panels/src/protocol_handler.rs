// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Generic panel communication protocol handler.
//!
//! Provides message framing, parsing, and construction for panel communication.
//! Wire format: `[type:u8][panel_id:u8][data_len:u16 LE][data...]`

/// Panel message types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// Display text update.
    DisplayUpdate = 0,
    /// LED on/off control.
    LedControl = 1,
    /// Button press/release event.
    ButtonEvent = 2,
    /// Rotary encoder event.
    EncoderEvent = 3,
    /// Connection keep-alive.
    KeepAlive = 4,
}

impl MessageType {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::DisplayUpdate),
            1 => Some(Self::LedControl),
            2 => Some(Self::ButtonEvent),
            3 => Some(Self::EncoderEvent),
            4 => Some(Self::KeepAlive),
            _ => None,
        }
    }
}

/// A parsed panel message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelMessage {
    /// The type of this message.
    pub message_type: MessageType,
    /// Target or source panel identifier.
    pub panel_id: u8,
    /// Message payload.
    pub data: Vec<u8>,
}

/// Header size in bytes: type(1) + panel_id(1) + data_len(2).
const HEADER_SIZE: usize = 4;

/// Protocol handler for panel communication.
///
/// Accumulates incoming bytes and parses complete messages using a simple
/// length-prefixed framing format.
pub struct ProtocolHandler {
    buffer: Vec<u8>,
    max_message_size: usize,
}

impl ProtocolHandler {
    /// Creates a new protocol handler with the given maximum data payload size.
    #[must_use]
    pub fn new(max_message_size: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_message_size,
        }
    }

    /// Appends raw bytes to the internal buffer.
    pub fn feed_bytes(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Attempts to parse the next complete message from the buffer.
    ///
    /// Returns `None` if insufficient data is available or the leading bytes
    /// are malformed (unknown type or oversized payload, which are discarded).
    pub fn parse_next(&mut self) -> Option<PanelMessage> {
        if self.buffer.len() < HEADER_SIZE {
            return None;
        }

        let type_byte = self.buffer[0];
        let panel_id = self.buffer[1];
        let data_len = u16::from_le_bytes([self.buffer[2], self.buffer[3]]) as usize;

        let message_type = match MessageType::from_u8(type_byte) {
            Some(mt) => mt,
            None => {
                self.buffer.drain(..HEADER_SIZE);
                return None;
            }
        };

        if data_len > self.max_message_size {
            self.buffer.drain(..HEADER_SIZE);
            return None;
        }

        let total_len = HEADER_SIZE + data_len;
        if self.buffer.len() < total_len {
            return None;
        }

        let data = self.buffer[HEADER_SIZE..total_len].to_vec();
        self.buffer.drain(..total_len);

        Some(PanelMessage {
            message_type,
            panel_id,
            data,
        })
    }

    /// Builds a display-update message.
    #[must_use]
    pub fn build_display_message(panel_id: u8, text: &str) -> Vec<u8> {
        Self::build_raw(MessageType::DisplayUpdate as u8, panel_id, text.as_bytes())
    }

    /// Builds an LED-control message from a slice of on/off states.
    #[must_use]
    pub fn build_led_message(panel_id: u8, leds: &[bool]) -> Vec<u8> {
        let led_bytes: Vec<u8> = leds.iter().map(|&on| u8::from(on)).collect();
        Self::build_raw(MessageType::LedControl as u8, panel_id, &led_bytes)
    }

    /// Returns the number of bytes currently buffered.
    #[must_use]
    pub fn pending_bytes(&self) -> usize {
        self.buffer.len()
    }

    fn build_raw(msg_type: u8, panel_id: u8, data: &[u8]) -> Vec<u8> {
        let data_len = data.len() as u16;
        let mut msg = Vec::with_capacity(HEADER_SIZE + data.len());
        msg.push(msg_type);
        msg.push(panel_id);
        msg.extend_from_slice(&data_len.to_le_bytes());
        msg.extend_from_slice(data);
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to construct a raw message for testing.
    fn make_raw(msg_type: u8, panel_id: u8, data: &[u8]) -> Vec<u8> {
        ProtocolHandler::build_raw(msg_type, panel_id, data)
    }

    #[test]
    fn test_new_handler() {
        let handler = ProtocolHandler::new(256);
        assert_eq!(handler.pending_bytes(), 0);
    }

    #[test]
    fn test_feed_and_parse_display_update() {
        let msg = ProtocolHandler::build_display_message(1, "HI");
        let mut handler = ProtocolHandler::new(256);
        handler.feed_bytes(&msg);
        let parsed = handler.parse_next().unwrap();
        assert_eq!(parsed.message_type, MessageType::DisplayUpdate);
        assert_eq!(parsed.panel_id, 1);
        assert_eq!(parsed.data, b"HI");
    }

    #[test]
    fn test_feed_and_parse_led_control() {
        let msg = ProtocolHandler::build_led_message(2, &[true, false, true]);
        let mut handler = ProtocolHandler::new(256);
        handler.feed_bytes(&msg);
        let parsed = handler.parse_next().unwrap();
        assert_eq!(parsed.message_type, MessageType::LedControl);
        assert_eq!(parsed.panel_id, 2);
        assert_eq!(parsed.data, vec![1, 0, 1]);
    }

    #[test]
    fn test_feed_and_parse_button_event() {
        let raw = make_raw(MessageType::ButtonEvent as u8, 3, &[0x42]);
        let mut handler = ProtocolHandler::new(256);
        handler.feed_bytes(&raw);
        let parsed = handler.parse_next().unwrap();
        assert_eq!(parsed.message_type, MessageType::ButtonEvent);
        assert_eq!(parsed.panel_id, 3);
        assert_eq!(parsed.data, vec![0x42]);
    }

    #[test]
    fn test_feed_and_parse_encoder_event() {
        let raw = make_raw(MessageType::EncoderEvent as u8, 4, &[0x01, 0xFF]);
        let mut handler = ProtocolHandler::new(256);
        handler.feed_bytes(&raw);
        let parsed = handler.parse_next().unwrap();
        assert_eq!(parsed.message_type, MessageType::EncoderEvent);
        assert_eq!(parsed.panel_id, 4);
        assert_eq!(parsed.data, vec![0x01, 0xFF]);
    }

    #[test]
    fn test_feed_and_parse_keepalive() {
        let raw = make_raw(MessageType::KeepAlive as u8, 0, &[]);
        let mut handler = ProtocolHandler::new(256);
        handler.feed_bytes(&raw);
        let parsed = handler.parse_next().unwrap();
        assert_eq!(parsed.message_type, MessageType::KeepAlive);
        assert_eq!(parsed.panel_id, 0);
        assert!(parsed.data.is_empty());
    }

    #[test]
    fn test_partial_message_returns_none() {
        let msg = ProtocolHandler::build_display_message(1, "HELLO");
        let mut handler = ProtocolHandler::new(256);
        // Feed only the header
        handler.feed_bytes(&msg[..HEADER_SIZE]);
        assert!(handler.parse_next().is_none());
        assert_eq!(handler.pending_bytes(), HEADER_SIZE);
    }

    #[test]
    fn test_build_display_message_format() {
        let msg = ProtocolHandler::build_display_message(5, "AB");
        assert_eq!(msg[0], MessageType::DisplayUpdate as u8);
        assert_eq!(msg[1], 5);
        assert_eq!(u16::from_le_bytes([msg[2], msg[3]]), 2);
        assert_eq!(&msg[4..], b"AB");
    }

    #[test]
    fn test_build_led_message_format() {
        let msg = ProtocolHandler::build_led_message(7, &[false, true]);
        assert_eq!(msg[0], MessageType::LedControl as u8);
        assert_eq!(msg[1], 7);
        assert_eq!(u16::from_le_bytes([msg[2], msg[3]]), 2);
        assert_eq!(&msg[4..], &[0, 1]);
    }

    #[test]
    fn test_pending_bytes_tracking() {
        let mut handler = ProtocolHandler::new(256);
        assert_eq!(handler.pending_bytes(), 0);
        handler.feed_bytes(&[1, 2, 3]);
        assert_eq!(handler.pending_bytes(), 3);
    }

    #[test]
    fn test_multiple_messages_sequential() {
        let msg1 = ProtocolHandler::build_display_message(1, "A");
        let msg2 = ProtocolHandler::build_led_message(2, &[true]);
        let mut handler = ProtocolHandler::new(256);
        handler.feed_bytes(&msg1);
        handler.feed_bytes(&msg2);

        let p1 = handler.parse_next().unwrap();
        assert_eq!(p1.message_type, MessageType::DisplayUpdate);
        assert_eq!(p1.data, b"A");

        let p2 = handler.parse_next().unwrap();
        assert_eq!(p2.message_type, MessageType::LedControl);
        assert_eq!(p2.data, vec![1]);

        assert!(handler.parse_next().is_none());
    }

    #[test]
    fn test_oversized_message_discarded() {
        // data_len = 300, but max is 256
        let mut raw = vec![MessageType::DisplayUpdate as u8, 1];
        raw.extend_from_slice(&300u16.to_le_bytes());
        raw.extend_from_slice(&vec![0u8; 300]);

        let mut handler = ProtocolHandler::new(256);
        handler.feed_bytes(&raw);
        // Oversized header is discarded, remaining bytes stay
        assert!(handler.parse_next().is_none());
        assert_eq!(handler.pending_bytes(), 300); // data bytes remain
    }

    #[test]
    fn test_unknown_type_discarded() {
        let raw = make_raw(0xFF, 1, &[0x01]);
        let mut handler = ProtocolHandler::new(256);
        handler.feed_bytes(&raw);
        // Unknown type: header discarded
        assert!(handler.parse_next().is_none());
        // Data byte + original data still in buffer (only header was drained)
        assert_eq!(handler.pending_bytes(), 1);
    }
}
