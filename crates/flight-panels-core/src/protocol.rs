// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Panel protocol trait and common event types.
//!
//! Defines a hardware-agnostic interface that every panel driver implements,
//! plus shared types for input events, panel identification, and a structured
//! message/response protocol for panel communication.

use std::fmt;
use std::time::Duration;

// ─── Panel identification ────────────────────────────────────────────────────

/// Unique identifier for a connected panel device.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PanelId {
    /// USB vendor ID.
    pub vendor_id: u16,
    /// USB product ID.
    pub product_id: u16,
    /// OS-level device path (e.g. HID path).
    pub device_path: String,
}

impl fmt::Display for PanelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04X}:{:04X} @ {}",
            self.vendor_id, self.product_id, self.device_path
        )
    }
}

// ─── Panel events ────────────────────────────────────────────────────────────

/// An input event produced by a panel.
#[derive(Debug, Clone, PartialEq)]
pub enum PanelEvent {
    /// A button was pressed.
    ButtonPress {
        /// Human-readable button name (e.g. `"AP"`, `"HDG"`).
        name: &'static str,
    },
    /// A button was released.
    ButtonRelease {
        /// Human-readable button name.
        name: &'static str,
    },
    /// A rotary encoder was turned.
    EncoderTick {
        /// Encoder name (e.g. `"OUTER"`, `"INNER"`).
        name: &'static str,
        /// Positive = clockwise, negative = counter-clockwise.
        delta: i8,
    },
    /// A toggle switch changed position.
    SwitchChange {
        /// Switch name (e.g. `"MASTER_BAT"`, `"GEAR"`).
        name: &'static str,
        /// `true` = on / engaged, `false` = off / disengaged.
        on: bool,
    },
    /// A multi-position selector changed.
    SelectorChange {
        /// Selector name (e.g. `"MODE"`, `"MAGNETO"`).
        name: &'static str,
        /// New position index (0-based).
        position: u8,
    },
}

// ─── Panel message protocol ──────────────────────────────────────────────────

/// A command message sent to a panel device.
#[derive(Debug, Clone, PartialEq)]
pub enum PanelMessage {
    /// Request the current switch/button/encoder state.
    ReadState,
    /// Update a segment display with up to 5 characters.
    WriteDisplay {
        /// Display row index (0 = primary, 1 = secondary).
        row: u8,
        /// Display content (will be 7-segment encoded by the codec).
        text: [u8; 5],
    },
    /// Set LED state for a named indicator.
    SetLed {
        /// LED index within the panel's LED mapping.
        led_index: u8,
        /// `true` = on, `false` = off.
        on: bool,
    },
    /// Set panel backlight brightness.
    SetBacklight {
        /// Brightness level 0–255.
        brightness: u8,
    },
    /// Request a calibration / self-test cycle.
    Calibrate,
}

/// A response received from a panel device.
#[derive(Debug, Clone, PartialEq)]
pub enum PanelResponse {
    /// Raw HID state data returned by a [`PanelMessage::ReadState`] request.
    StateData {
        /// Raw report bytes.
        data: Vec<u8>,
    },
    /// Acknowledgement that a command was accepted.
    Ack,
    /// The panel reported an error.
    Error {
        /// Machine-readable error code.
        code: u16,
        /// Human-readable description.
        message: String,
    },
}

/// Encode / decode [`PanelMessage`] and [`PanelResponse`] to / from HID report bytes.
///
/// Each panel type provides a concrete implementation that maps the generic
/// message types to its specific HID report layout.
pub trait PanelCodec: Send {
    /// Encode a [`PanelMessage`] into a byte buffer suitable for an HID output report.
    ///
    /// Returns `None` if the message type is not supported by this panel.
    fn encode(&self, msg: &PanelMessage) -> Option<Vec<u8>>;

    /// Decode raw HID input report bytes into a [`PanelResponse`].
    ///
    /// Returns `None` for malformed or unrecognised reports.
    fn decode(&self, data: &[u8]) -> Option<PanelResponse>;
}

/// Connection-level interface for sending and receiving panel messages.
pub trait PanelConnection: Send {
    /// Send a message to the panel, blocking until the write completes.
    fn send(&mut self, msg: &PanelMessage) -> Result<(), String>;

    /// Receive the next response, blocking up to `timeout`.
    ///
    /// Returns `None` if no response arrives before the timeout expires.
    fn receive(&mut self, timeout: Duration) -> Option<PanelResponse>;

    /// Non-blocking poll: returns `true` if data is available to [`receive`][Self::receive].
    fn poll(&self) -> bool;
}

// ─── Panel protocol trait ────────────────────────────────────────────────────

/// Hardware-agnostic interface implemented by every panel driver.
///
/// Each panel type (Radio Panel, Multi Panel, Switch Panel, …) provides a
/// concrete implementation that knows how to parse HID input reports and
/// build HID output reports.
pub trait PanelProtocol: Send {
    /// Human-readable panel name (e.g. `"Saitek Radio Panel"`).
    fn name(&self) -> &str;

    /// USB vendor ID for this panel type.
    fn vendor_id(&self) -> u16;

    /// USB product ID for this panel type.
    fn product_id(&self) -> u16;

    /// Names of the LEDs / indicators this panel exposes.
    fn led_names(&self) -> &[&'static str];

    /// Expected HID output report size in bytes.
    fn output_report_size(&self) -> usize;

    /// Parse a raw HID input report into zero or more [`PanelEvent`]s.
    ///
    /// Returns `None` when the report is too short or malformed.
    fn parse_input(&self, data: &[u8]) -> Option<Vec<PanelEvent>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PanelId ──────────────────────────────────────────────────────────────

    #[test]
    fn test_panel_id_display() {
        let id = PanelId {
            vendor_id: 0x06A3,
            product_id: 0x0D05,
            device_path: "/dev/hidraw0".to_string(),
        };
        assert_eq!(id.to_string(), "06A3:0D05 @ /dev/hidraw0");
    }

    #[test]
    fn test_panel_id_equality() {
        let a = PanelId {
            vendor_id: 0x06A3,
            product_id: 0x0D05,
            device_path: "/dev/hidraw0".to_string(),
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_panel_id_inequality_different_path() {
        let a = PanelId {
            vendor_id: 0x06A3,
            product_id: 0x0D05,
            device_path: "/dev/hidraw0".to_string(),
        };
        let b = PanelId {
            vendor_id: 0x06A3,
            product_id: 0x0D05,
            device_path: "/dev/hidraw1".to_string(),
        };
        assert_ne!(a, b);
    }

    // ── PanelEvent ───────────────────────────────────────────────────────────

    #[test]
    fn test_button_press_event() {
        let event = PanelEvent::ButtonPress { name: "AP" };
        assert_eq!(event, PanelEvent::ButtonPress { name: "AP" });
    }

    #[test]
    fn test_encoder_tick_event() {
        let cw = PanelEvent::EncoderTick {
            name: "OUTER",
            delta: 1,
        };
        let ccw = PanelEvent::EncoderTick {
            name: "OUTER",
            delta: -1,
        };
        assert_ne!(cw, ccw);
    }

    #[test]
    fn test_switch_change_event() {
        let on = PanelEvent::SwitchChange {
            name: "GEAR",
            on: true,
        };
        let off = PanelEvent::SwitchChange {
            name: "GEAR",
            on: false,
        };
        assert_ne!(on, off);
    }

    #[test]
    fn test_selector_change_event() {
        let event = PanelEvent::SelectorChange {
            name: "MODE",
            position: 3,
        };
        if let PanelEvent::SelectorChange { name, position } = event {
            assert_eq!(name, "MODE");
            assert_eq!(position, 3);
        } else {
            panic!("wrong variant");
        }
    }

    // ── Mock panel for trait testing ─────────────────────────────────────────

    struct MockPanel;

    impl PanelProtocol for MockPanel {
        fn name(&self) -> &str {
            "Mock Panel"
        }
        fn vendor_id(&self) -> u16 {
            0x1234
        }
        fn product_id(&self) -> u16 {
            0x5678
        }
        fn led_names(&self) -> &[&'static str] {
            &["LED_A", "LED_B"]
        }
        fn output_report_size(&self) -> usize {
            8
        }
        fn parse_input(&self, data: &[u8]) -> Option<Vec<PanelEvent>> {
            if data.len() < 2 {
                return None;
            }
            let mut events = Vec::new();
            if data[1] & 0x01 != 0 {
                events.push(PanelEvent::ButtonPress { name: "BTN_A" });
            }
            Some(events)
        }
    }

    #[test]
    fn test_panel_protocol_trait() {
        let panel = MockPanel;
        assert_eq!(panel.name(), "Mock Panel");
        assert_eq!(panel.vendor_id(), 0x1234);
        assert_eq!(panel.product_id(), 0x5678);
        assert_eq!(panel.led_names(), &["LED_A", "LED_B"]);
        assert_eq!(panel.output_report_size(), 8);
    }

    #[test]
    fn test_panel_protocol_parse_input_button() {
        let panel = MockPanel;
        let events = panel.parse_input(&[0x00, 0x01]).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], PanelEvent::ButtonPress { name: "BTN_A" });
    }

    #[test]
    fn test_panel_protocol_parse_input_no_buttons() {
        let panel = MockPanel;
        let events = panel.parse_input(&[0x00, 0x00]).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_panel_protocol_parse_input_too_short() {
        let panel = MockPanel;
        assert!(panel.parse_input(&[0x00]).is_none());
    }

    #[test]
    fn test_panel_protocol_is_object_safe() {
        // Verify PanelProtocol can be used as a trait object
        let panel: Box<dyn PanelProtocol> = Box::new(MockPanel);
        assert_eq!(panel.name(), "Mock Panel");
        assert_eq!(panel.led_names().len(), 2);
    }

    // ── PanelMessage ─────────────────────────────────────────────────────────

    #[test]
    fn test_panel_message_variants() {
        let read = PanelMessage::ReadState;
        assert_eq!(read, PanelMessage::ReadState);

        let write = PanelMessage::WriteDisplay {
            row: 0,
            text: [0x3F, 0x06, 0x5B, 0x4F, 0x66],
        };
        if let PanelMessage::WriteDisplay { row, text } = &write {
            assert_eq!(*row, 0);
            assert_eq!(text.len(), 5);
        } else {
            panic!("wrong variant");
        }

        let led = PanelMessage::SetLed {
            led_index: 3,
            on: true,
        };
        assert_eq!(
            led,
            PanelMessage::SetLed {
                led_index: 3,
                on: true
            }
        );

        let bl = PanelMessage::SetBacklight { brightness: 128 };
        assert_eq!(bl, PanelMessage::SetBacklight { brightness: 128 });

        let cal = PanelMessage::Calibrate;
        assert_eq!(cal, PanelMessage::Calibrate);
    }

    #[test]
    fn test_panel_message_clone() {
        let msg = PanelMessage::SetLed {
            led_index: 5,
            on: false,
        };
        let cloned = msg.clone();
        assert_eq!(msg, cloned);
    }

    // ── PanelResponse ────────────────────────────────────────────────────────

    #[test]
    fn test_panel_response_state_data() {
        let resp = PanelResponse::StateData {
            data: vec![0x00, 0xFF, 0x42],
        };
        if let PanelResponse::StateData { data } = &resp {
            assert_eq!(data.len(), 3);
            assert_eq!(data[2], 0x42);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn test_panel_response_ack() {
        let resp = PanelResponse::Ack;
        assert_eq!(resp, PanelResponse::Ack);
    }

    #[test]
    fn test_panel_response_error() {
        let resp = PanelResponse::Error {
            code: 0x01,
            message: "Device busy".to_string(),
        };
        if let PanelResponse::Error { code, message } = &resp {
            assert_eq!(*code, 0x01);
            assert_eq!(message, "Device busy");
        } else {
            panic!("wrong variant");
        }
    }

    // ── PanelCodec mock ──────────────────────────────────────────────────────

    struct MockCodec;

    impl PanelCodec for MockCodec {
        fn encode(&self, msg: &PanelMessage) -> Option<Vec<u8>> {
            match msg {
                PanelMessage::ReadState => Some(vec![0x01]),
                PanelMessage::WriteDisplay { row, text } => {
                    let mut buf = vec![0x02, *row];
                    buf.extend_from_slice(text);
                    Some(buf)
                }
                PanelMessage::SetLed { led_index, on } => {
                    Some(vec![0x03, *led_index, u8::from(*on)])
                }
                PanelMessage::SetBacklight { brightness } => Some(vec![0x04, *brightness]),
                PanelMessage::Calibrate => Some(vec![0x05]),
            }
        }

        fn decode(&self, data: &[u8]) -> Option<PanelResponse> {
            if data.is_empty() {
                return None;
            }
            match data[0] {
                0x80 => Some(PanelResponse::StateData {
                    data: data[1..].to_vec(),
                }),
                0x81 => Some(PanelResponse::Ack),
                0xFF => Some(PanelResponse::Error {
                    code: if data.len() > 1 { data[1] as u16 } else { 0 },
                    message: "error".to_string(),
                }),
                _ => None,
            }
        }
    }

    #[test]
    fn test_codec_encode_read_state() {
        let codec = MockCodec;
        let encoded = codec.encode(&PanelMessage::ReadState).unwrap();
        assert_eq!(encoded, vec![0x01]);
    }

    #[test]
    fn test_codec_encode_write_display() {
        let codec = MockCodec;
        let msg = PanelMessage::WriteDisplay {
            row: 0,
            text: [0x3F, 0x06, 0x5B, 0x4F, 0x66],
        };
        let encoded = codec.encode(&msg).unwrap();
        assert_eq!(encoded[0], 0x02);
        assert_eq!(encoded[1], 0x00);
        assert_eq!(encoded[2..7], [0x3F, 0x06, 0x5B, 0x4F, 0x66]);
    }

    #[test]
    fn test_codec_encode_set_led() {
        let codec = MockCodec;
        let msg = PanelMessage::SetLed {
            led_index: 3,
            on: true,
        };
        let encoded = codec.encode(&msg).unwrap();
        assert_eq!(encoded, vec![0x03, 3, 1]);
    }

    #[test]
    fn test_codec_encode_set_backlight() {
        let codec = MockCodec;
        let msg = PanelMessage::SetBacklight { brightness: 200 };
        let encoded = codec.encode(&msg).unwrap();
        assert_eq!(encoded, vec![0x04, 200]);
    }

    #[test]
    fn test_codec_encode_calibrate() {
        let codec = MockCodec;
        let encoded = codec.encode(&PanelMessage::Calibrate).unwrap();
        assert_eq!(encoded, vec![0x05]);
    }

    #[test]
    fn test_codec_decode_state_data() {
        let codec = MockCodec;
        let resp = codec.decode(&[0x80, 0xAA, 0xBB]).unwrap();
        assert_eq!(
            resp,
            PanelResponse::StateData {
                data: vec![0xAA, 0xBB]
            }
        );
    }

    #[test]
    fn test_codec_decode_ack() {
        let codec = MockCodec;
        let resp = codec.decode(&[0x81]).unwrap();
        assert_eq!(resp, PanelResponse::Ack);
    }

    #[test]
    fn test_codec_decode_error() {
        let codec = MockCodec;
        let resp = codec.decode(&[0xFF, 0x01]).unwrap();
        if let PanelResponse::Error { code, .. } = resp {
            assert_eq!(code, 1);
        } else {
            panic!("expected error");
        }
    }

    #[test]
    fn test_codec_decode_empty_returns_none() {
        let codec = MockCodec;
        assert!(codec.decode(&[]).is_none());
    }

    #[test]
    fn test_codec_decode_unknown_returns_none() {
        let codec = MockCodec;
        assert!(codec.decode(&[0x42]).is_none());
    }

    #[test]
    fn test_codec_encode_decode_roundtrip() {
        let codec = MockCodec;
        // Encode a SetLed, then decode an Ack response
        let msg = PanelMessage::SetLed {
            led_index: 2,
            on: true,
        };
        let encoded = codec.encode(&msg).unwrap();
        assert!(!encoded.is_empty());
        let ack = codec.decode(&[0x81]).unwrap();
        assert_eq!(ack, PanelResponse::Ack);
    }

    #[test]
    fn test_codec_is_object_safe() {
        let codec: Box<dyn PanelCodec> = Box::new(MockCodec);
        assert!(codec.encode(&PanelMessage::ReadState).is_some());
    }

    #[test]
    fn test_codec_roundtrip_all_message_types() {
        let codec = MockCodec;
        let messages = [
            PanelMessage::ReadState,
            PanelMessage::WriteDisplay {
                row: 0,
                text: [0x3F, 0x06, 0x5B, 0x4F, 0x66],
            },
            PanelMessage::WriteDisplay {
                row: 1,
                text: [0x00; 5],
            },
            PanelMessage::SetLed {
                led_index: 0,
                on: true,
            },
            PanelMessage::SetLed {
                led_index: 7,
                on: false,
            },
            PanelMessage::SetBacklight { brightness: 0 },
            PanelMessage::SetBacklight { brightness: 255 },
            PanelMessage::Calibrate,
        ];
        for msg in &messages {
            let encoded = codec.encode(msg);
            assert!(
                encoded.is_some(),
                "encode should succeed for {:?}",
                msg
            );
            assert!(!encoded.unwrap().is_empty());
        }
    }

    #[test]
    fn test_codec_decode_malformed_single_byte() {
        let codec = MockCodec;
        // Single byte that doesn't match any response prefix
        assert!(codec.decode(&[0x42]).is_none());
        assert!(codec.decode(&[0x00]).is_none());
        assert!(codec.decode(&[0x7F]).is_none());
    }

    #[test]
    fn test_codec_decode_error_without_code_byte() {
        let codec = MockCodec;
        // Error response with only prefix byte → code defaults to 0
        let resp = codec.decode(&[0xFF]).unwrap();
        if let PanelResponse::Error { code, .. } = resp {
            assert_eq!(code, 0);
        } else {
            panic!("expected error response");
        }
    }

    #[test]
    fn test_codec_decode_state_data_empty_payload() {
        let codec = MockCodec;
        // State data with just the prefix and no payload bytes
        let resp = codec.decode(&[0x80]).unwrap();
        if let PanelResponse::StateData { data } = resp {
            assert!(data.is_empty());
        } else {
            panic!("expected state data");
        }
    }

    // ── PanelConnection mock ─────────────────────────────────────────────────

    struct MockConnection {
        pending: Vec<PanelResponse>,
    }

    impl PanelConnection for MockConnection {
        fn send(&mut self, _msg: &PanelMessage) -> Result<(), String> {
            self.pending.push(PanelResponse::Ack);
            Ok(())
        }

        fn receive(&mut self, _timeout: Duration) -> Option<PanelResponse> {
            if self.pending.is_empty() {
                None
            } else {
                Some(self.pending.remove(0))
            }
        }

        fn poll(&self) -> bool {
            !self.pending.is_empty()
        }
    }

    #[test]
    fn test_connection_send_receive() {
        let mut conn = MockConnection {
            pending: Vec::new(),
        };
        assert!(!conn.poll());
        conn.send(&PanelMessage::ReadState).unwrap();
        assert!(conn.poll());
        let resp = conn.receive(Duration::from_millis(100)).unwrap();
        assert_eq!(resp, PanelResponse::Ack);
        assert!(!conn.poll());
    }

    #[test]
    fn test_connection_receive_timeout_returns_none() {
        let mut conn = MockConnection {
            pending: Vec::new(),
        };
        assert!(conn.receive(Duration::from_millis(10)).is_none());
    }

    #[test]
    fn test_connection_multiple_sends() {
        let mut conn = MockConnection {
            pending: Vec::new(),
        };
        conn.send(&PanelMessage::SetLed {
            led_index: 0,
            on: true,
        })
        .unwrap();
        conn.send(&PanelMessage::SetBacklight { brightness: 100 })
            .unwrap();
        assert!(conn.poll());
        let r1 = conn.receive(Duration::from_millis(100)).unwrap();
        let r2 = conn.receive(Duration::from_millis(100)).unwrap();
        assert_eq!(r1, PanelResponse::Ack);
        assert_eq!(r2, PanelResponse::Ack);
        assert!(!conn.poll());
    }

    #[test]
    fn test_connection_is_object_safe() {
        let conn: Box<dyn PanelConnection> = Box::new(MockConnection {
            pending: Vec::new(),
        });
        assert!(!conn.poll());
    }

    // ── State machine lifecycle tests ────────────────────────────────────────

    /// Simulates the full panel lifecycle:
    /// connect → configure (calibrate + set backlight) → update loop → disconnect
    #[test]
    fn test_panel_lifecycle_connect_configure_update_disconnect() {
        let codec = MockCodec;
        let mut conn = MockConnection {
            pending: Vec::new(),
        };

        // Phase 1: Connect — read initial state
        conn.send(&PanelMessage::ReadState).unwrap();
        let resp = conn.receive(Duration::from_millis(100)).unwrap();
        assert_eq!(resp, PanelResponse::Ack);

        // Phase 2: Configure — calibrate and set backlight
        conn.send(&PanelMessage::Calibrate).unwrap();
        let resp = conn.receive(Duration::from_millis(100)).unwrap();
        assert_eq!(resp, PanelResponse::Ack);

        conn.send(&PanelMessage::SetBacklight { brightness: 200 })
            .unwrap();
        let resp = conn.receive(Duration::from_millis(100)).unwrap();
        assert_eq!(resp, PanelResponse::Ack);

        // Phase 3: Update loop — send display + LED updates
        for i in 0..10 {
            let text = [
                0x3F,
                0x06,
                0x5B,
                0x4F,
                (0x60 + i as u8) & 0x7F,
            ];
            conn.send(&PanelMessage::WriteDisplay { row: 0, text })
                .unwrap();
            let resp = conn.receive(Duration::from_millis(100)).unwrap();
            assert_eq!(resp, PanelResponse::Ack);

            conn.send(&PanelMessage::SetLed {
                led_index: (i % 8) as u8,
                on: i % 2 == 0,
            })
            .unwrap();
            let resp = conn.receive(Duration::from_millis(100)).unwrap();
            assert_eq!(resp, PanelResponse::Ack);
        }

        // Phase 4: Disconnect — blank display and LEDs
        conn.send(&PanelMessage::WriteDisplay {
            row: 0,
            text: [0x00; 5],
        })
        .unwrap();
        let resp = conn.receive(Duration::from_millis(100)).unwrap();
        assert_eq!(resp, PanelResponse::Ack);

        // All messages consumed
        assert!(!conn.poll());
    }

    #[test]
    fn test_panel_protocol_parse_empty_report() {
        let panel = MockPanel;
        assert!(panel.parse_input(&[]).is_none());
    }

    #[test]
    fn test_panel_protocol_parse_all_zeros() {
        let panel = MockPanel;
        let events = panel.parse_input(&[0x00, 0x00]).unwrap();
        assert!(events.is_empty(), "all-zeros report should produce no events");
    }

    #[test]
    fn test_panel_protocol_parse_max_bytes() {
        let panel = MockPanel;
        // Large report — only first 2 bytes matter for MockPanel
        let data = vec![0x00; 256];
        let events = panel.parse_input(&data).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_connection_interleaved_send_receive() {
        let mut conn = MockConnection {
            pending: Vec::new(),
        };

        // Send 3 messages, receive after each one
        for _ in 0..3 {
            conn.send(&PanelMessage::ReadState).unwrap();
            assert!(conn.poll());
            let resp = conn.receive(Duration::from_millis(10)).unwrap();
            assert_eq!(resp, PanelResponse::Ack);
            assert!(!conn.poll());
        }
    }

    #[test]
    fn test_connection_drain_all_pending() {
        let mut conn = MockConnection {
            pending: Vec::new(),
        };

        // Queue up several messages
        for _ in 0..5 {
            conn.send(&PanelMessage::ReadState).unwrap();
        }

        // Drain all
        let mut count = 0;
        while conn.poll() {
            let _ = conn.receive(Duration::from_millis(10)).unwrap();
            count += 1;
        }
        assert_eq!(count, 5);
        assert!(conn.receive(Duration::from_millis(10)).is_none());
    }
}
