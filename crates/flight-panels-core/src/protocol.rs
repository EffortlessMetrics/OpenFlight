// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Panel protocol trait and common event types.
//!
//! Defines a hardware-agnostic interface that every panel driver implements,
//! plus shared types for input events and panel identification.

use std::fmt;

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
}
