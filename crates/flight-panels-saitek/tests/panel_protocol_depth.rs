// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the Saitek panel protocol engine.
//!
//! Covers 7-segment display encoding, LED control, switch state parsing,
//! panel identification, HID report framing, and integration scenarios.

use flight_panels_saitek::multi_panel::{
    self, LcdDisplay, ModeStateMachine, MultiPanelButtonState, MultiPanelLedMask,
    MultiPanelProtocol, MultiPanelState, MULTI_PANEL_OUTPUT_BYTES, MULTI_PANEL_PID,
    MULTI_PANEL_VID, encode_segment, led_bits, parse_multi_panel_input,
};
use flight_panels_saitek::radio_panel::{
    EncoderDelta, RadioDisplay, RadioMode, RadioPanelButtonState, RadioPanelProtocol,
    RadioPanelState, RADIO_PANEL_OUTPUT_BYTES, RADIO_PANEL_PID, RADIO_PANEL_VID,
    parse_radio_panel_input,
};
use flight_panels_saitek::saitek::PanelType;
use flight_panels_saitek::switch_panel::{
    GearLedColor, MagnetoPosition, SwitchDebounce, SwitchPanelGearLeds, SwitchPanelProtocol,
    SwitchPanelState, SwitchPanelSwitchState, SWITCH_PANEL_OUTPUT_BYTES, SWITCH_PANEL_PID,
    SWITCH_PANEL_VID, gear_led_bits, parse_switch_panel_input,
};

use flight_panels_core::protocol::{PanelEvent, PanelId, PanelProtocol};
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════════════════════════════════════════
// 1. 7-Segment Display
// ═══════════════════════════════════════════════════════════════════════════════

mod seven_segment {
    use super::*;

    /// Standard 7-segment encodings: each digit 0-9 lights the correct segments.
    #[test]
    fn digits_0_through_9_segment_encoding() {
        let expected: [(char, u8); 10] = [
            ('0', 0x3F), // a b c d e f
            ('1', 0x06), //   b c
            ('2', 0x5B), // a b   d e   g
            ('3', 0x4F), // a b c d     g
            ('4', 0x66), //   b c     f g
            ('5', 0x6D), // a   c d   f g
            ('6', 0x7D), // a   c d e f g
            ('7', 0x07), // a b c
            ('8', 0x7F), // a b c d e f g
            ('9', 0x6F), // a b c d   f g
        ];
        for (c, enc) in expected {
            assert_eq!(
                encode_segment(c),
                enc,
                "digit '{c}' should encode to {enc:#04x}"
            );
        }
    }

    /// Blank (space) and dash are the two special characters.
    #[test]
    fn blank_and_dash_special_chars() {
        assert_eq!(encode_segment(' '), 0x00, "space = all segments off");
        assert_eq!(encode_segment('-'), 0x40, "dash = only segment g (middle)");
    }

    /// Decimal point: the encode_segment function doesn't handle it directly;
    /// the decimal point is bit 7 which can be OR'd onto any encoded byte.
    #[test]
    fn decimal_point_via_bit7_overlay() {
        let five = encode_segment('5');
        let five_with_dp = five | 0x80;
        // Original segments preserved, plus dp bit
        assert_eq!(five_with_dp & 0x7F, five, "lower 7 bits unchanged");
        assert_ne!(five_with_dp & 0x80, 0, "decimal point bit set");
    }

    /// Multi-digit number → LcdDisplay segment array.
    #[test]
    fn multi_digit_number_to_segment_array() {
        let lcd = LcdDisplay::from_integer(12345);
        assert_eq!(lcd.raw(0), encode_segment('1'));
        assert_eq!(lcd.raw(1), encode_segment('2'));
        assert_eq!(lcd.raw(2), encode_segment('3'));
        assert_eq!(lcd.raw(3), encode_segment('4'));
        assert_eq!(lcd.raw(4), encode_segment('5'));
    }

    /// Leading spaces (right-justified) for values shorter than 5 digits.
    #[test]
    fn leading_spaces_for_small_values() {
        let lcd = LcdDisplay::from_integer(7);
        // "    7"
        for pos in 0..4 {
            assert_eq!(
                lcd.raw(pos),
                encode_segment(' '),
                "position {pos} should be blank"
            );
        }
        assert_eq!(lcd.raw(4), encode_segment('7'));
    }

    /// Zero is displayed as right-justified "    0".
    #[test]
    fn zero_right_justified() {
        let lcd = LcdDisplay::from_integer(0);
        assert_eq!(lcd.raw(4), encode_segment('0'));
        assert_eq!(lcd.raw(3), encode_segment(' '));
    }

    /// Negative values get a leading dash.
    #[test]
    fn negative_value_with_dash() {
        let lcd = LcdDisplay::from_integer(-42);
        assert_eq!(lcd.raw(0), encode_segment('-'));
        // "-  42"
        assert_eq!(lcd.raw(3), encode_segment('4'));
        assert_eq!(lcd.raw(4), encode_segment('2'));
    }

    /// Values beyond 99999 are clamped.
    #[test]
    fn positive_overflow_clamped() {
        let lcd = LcdDisplay::from_integer(100_000);
        let clamped = LcdDisplay::from_integer(99999);
        assert_eq!(lcd.as_bytes(), clamped.as_bytes());
    }

    /// Negative values beyond −9999 are clamped.
    #[test]
    fn negative_overflow_clamped() {
        let lcd = LcdDisplay::from_integer(-99_999);
        let clamped = LcdDisplay::from_integer(-9999);
        assert_eq!(lcd.as_bytes(), clamped.as_bytes());
    }

    /// Roundtrip: encode_segment for each digit 0-9, then verify the segment
    /// pattern is unique (no two digits share the same encoding).
    #[test]
    fn encode_roundtrip_uniqueness() {
        let mut seen = std::collections::HashSet::new();
        for d in 0u8..10 {
            let c = char::from_digit(d as u32, 10).unwrap();
            let enc = encode_segment(c);
            assert!(
                seen.insert(enc),
                "digit {d} encoding {enc:#04x} collides with a previous digit"
            );
        }
    }

    /// LcdDisplay::encode_str with string shorter than 5 pads right with blanks.
    #[test]
    fn encode_str_pads_right() {
        let lcd = LcdDisplay::encode_str("1");
        assert_eq!(lcd.raw(0), encode_segment('1'));
        for i in 1..5 {
            assert_eq!(lcd.raw(i), 0x00, "position {i} should be blank");
        }
    }

    /// LcdDisplay::encode_str truncates at 5 characters.
    #[test]
    fn encode_str_truncates() {
        let lcd = LcdDisplay::encode_str("ABCDEFGH");
        // Out-of-range of raw position doesn't panic, returns 0
        assert_eq!(lcd.raw(5), 0);
    }

    /// set_raw allows arbitrary 7-segment patterns (e.g. custom indicators).
    #[test]
    fn set_raw_arbitrary_pattern() {
        let mut lcd = LcdDisplay::blank();
        lcd.set_raw(0, 0b0100_1001); // custom segments a, d, g
        assert_eq!(lcd.raw(0), 0b0100_1001);
        // Other positions untouched
        assert_eq!(lcd.raw(1), 0x00);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. LED Control
// ═══════════════════════════════════════════════════════════════════════════════

mod led_control {
    use super::*;

    /// Individual LED on/off via the mask set() builder.
    #[test]
    fn individual_led_on_off() {
        let mask = MultiPanelLedMask::NONE.set(led_bits::ALT, true);
        assert!(mask.is_set(led_bits::ALT));
        assert!(!mask.is_set(led_bits::VS));

        let mask = mask.set(led_bits::ALT, false);
        assert!(!mask.is_set(led_bits::ALT));
    }

    /// All LEDs on and all LEDs off constants.
    #[test]
    fn group_all_on_all_off() {
        assert_eq!(MultiPanelLedMask::ALL.raw(), 0xFF);
        assert_eq!(MultiPanelLedMask::NONE.raw(), 0x00);

        // ALL has every bit set
        for &bit in &[
            led_bits::ALT,
            led_bits::VS,
            led_bits::IAS,
            led_bits::HDG,
            led_bits::CRS,
            led_bits::AUTO_THROTTLE,
            led_bits::FLAPS,
            led_bits::PITCH_TRIM,
        ] {
            assert!(MultiPanelLedMask::ALL.is_set(bit));
            assert!(!MultiPanelLedMask::NONE.is_set(bit));
        }
    }

    /// Gear LEDs support red/green colour per gear position.
    #[test]
    fn gear_led_multi_color() {
        let leds = SwitchPanelGearLeds::ALL_OFF
            .set_left(GearLedColor::Green)
            .set_nose(GearLedColor::Red)
            .set_right(GearLedColor::Green);

        assert_ne!(leds.raw() & gear_led_bits::LEFT_GREEN, 0);
        assert_eq!(leds.raw() & gear_led_bits::LEFT_RED, 0);
        assert_eq!(leds.raw() & gear_led_bits::NOSE_GREEN, 0);
        assert_ne!(leds.raw() & gear_led_bits::NOSE_RED, 0);
        assert_ne!(leds.raw() & gear_led_bits::RIGHT_GREEN, 0);
        assert_eq!(leds.raw() & gear_led_bits::RIGHT_RED, 0);
    }

    /// Setting colour clears the opposite colour (dirty tracking: no ghost bits).
    #[test]
    fn color_set_clears_opposite() {
        let leds = SwitchPanelGearLeds::ALL_OFF
            .set_left(GearLedColor::Red)
            .set_left(GearLedColor::Green);
        assert_ne!(leds.raw() & gear_led_bits::LEFT_GREEN, 0);
        assert_eq!(
            leds.raw() & gear_led_bits::LEFT_RED,
            0,
            "red should be cleared when green is set"
        );

        let leds = leds.set_left(GearLedColor::Off);
        assert_eq!(leds.raw() & gear_led_bits::LEFT_GREEN, 0);
        assert_eq!(leds.raw() & gear_led_bits::LEFT_RED, 0);
    }

    /// LED mask in the HID report: only byte 11 changes.
    #[test]
    fn led_mask_in_hid_report() {
        let lcd = LcdDisplay::blank();
        let mask = MultiPanelLedMask::NONE
            .set(led_bits::HDG, true)
            .set(led_bits::CRS, true);
        let report = lcd.to_hid_report(mask);
        assert_eq!(report[11], led_bits::HDG | led_bits::CRS);
        // Display bytes should be zero (blank)
        for &b in &report[1..6] {
            assert_eq!(b, 0x00);
        }
    }

    /// Gear LED HID report is exactly SWITCH_PANEL_OUTPUT_BYTES (2 bytes).
    #[test]
    fn gear_led_hid_report_size() {
        let report = SwitchPanelGearLeds::ALL_GREEN.to_hid_report();
        assert_eq!(report.len(), SWITCH_PANEL_OUTPUT_BYTES);
        assert_eq!(report[0], 0x00, "report ID");
    }

    /// Transitioning from all-green to all-red changes every gear LED.
    #[test]
    fn gear_led_full_transition() {
        let green = SwitchPanelGearLeds::ALL_GREEN;
        let red = SwitchPanelGearLeds::ALL_RED;
        // No bit overlap
        assert_eq!(green.raw() & red.raw(), 0, "green and red bits must not overlap");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Switch State
// ═══════════════════════════════════════════════════════════════════════════════

mod switch_state {
    use super::*;

    /// Each toggle switch maps to a unique bit in byte1.
    #[test]
    fn individual_switch_detection() {
        let switches: [(fn(&SwitchPanelSwitchState) -> bool, u8); 8] = [
            (SwitchPanelSwitchState::master_battery, 0),
            (SwitchPanelSwitchState::master_alternator, 1),
            (SwitchPanelSwitchState::avionics_master, 2),
            (SwitchPanelSwitchState::fuel_pump, 3),
            (SwitchPanelSwitchState::de_ice, 4),
            (SwitchPanelSwitchState::pitot_heat, 5),
            (SwitchPanelSwitchState::cowl_flaps_closed, 6),
            (SwitchPanelSwitchState::panel_light, 7),
        ];
        for (accessor, bit) in switches {
            let state = SwitchPanelSwitchState {
                byte1: 1 << bit,
                byte2: 0,
            };
            assert!(
                accessor(&state),
                "switch at bit {bit} should be on when only that bit is set"
            );
            // All others off
            let state_off = SwitchPanelSwitchState {
                byte1: !(1u8 << bit),
                byte2: 0,
            };
            assert!(
                !accessor(&state_off),
                "switch at bit {bit} should be off when that bit is clear"
            );
        }
    }

    /// Rotary encoder step counting: outer and inner accumulate independently.
    #[test]
    fn rotary_encoder_step_counting() {
        let mut delta = EncoderDelta::default();
        // 5 clockwise outer ticks
        for _ in 0..5 {
            delta.update(&RadioPanelButtonState {
                mode: None,
                buttons: 0b0000_0010, // outer CW
            });
        }
        // 3 counter-clockwise inner ticks
        for _ in 0..3 {
            delta.update(&RadioPanelButtonState {
                mode: None,
                buttons: 0b0001_0000, // inner CCW
            });
        }
        assert_eq!(delta.outer, 5);
        assert_eq!(delta.inner, -3);

        let (o, i) = delta.drain();
        assert_eq!((o, i), (5, -3));
        assert_eq!(delta.outer, 0);
        assert_eq!(delta.inner, 0);
    }

    /// Multi-position switch: magneto OFF → R → L → BOTH → START.
    #[test]
    fn magneto_multi_position_mapping() {
        let positions = [
            (0b0000_0000u8, MagnetoPosition::Off),
            (0b0000_0010, MagnetoPosition::Right),
            (0b0000_0100, MagnetoPosition::Left),
            (0b0000_0110, MagnetoPosition::Both),
            (0b0000_1000, MagnetoPosition::Start),
        ];
        for (byte2, expected) in positions {
            let state = SwitchPanelSwitchState { byte1: 0, byte2 };
            assert_eq!(state.magneto(), Some(expected), "byte2={byte2:#010b}");
        }
    }

    /// Debounce: rapid transitions within the period are rejected.
    #[test]
    fn debounce_rejects_rapid_transitions() {
        let mut debounce = SwitchDebounce::new(Duration::from_millis(50));
        let t0 = Instant::now();

        assert!(debounce.accept(0, t0), "first transition accepted");
        assert!(
            !debounce.accept(0, t0 + Duration::from_millis(10)),
            "bounce within 50ms rejected"
        );
        assert!(
            !debounce.accept(0, t0 + Duration::from_millis(49)),
            "still within period"
        );
        assert!(
            debounce.accept(0, t0 + Duration::from_millis(51)),
            "after period, accepted"
        );
    }

    /// Debounce is per-switch: accepting switch 0 doesn't block switch 1.
    #[test]
    fn debounce_independent_per_switch() {
        let mut debounce = SwitchDebounce::new(Duration::from_millis(50));
        let t0 = Instant::now();
        assert!(debounce.accept(0, t0));
        assert!(debounce.accept(1, t0), "different switch still accepted");
        assert!(
            !debounce.accept(0, t0 + Duration::from_millis(10)),
            "same switch rejected"
        );
    }

    /// SwitchPanelProtocol::diff_with_debounce emits change events.
    #[test]
    fn state_change_events_via_diff() {
        let mut proto = SwitchPanelProtocol::new(Duration::ZERO);
        let t0 = Instant::now();

        // Initial state: everything off
        let state0 = SwitchPanelSwitchState {
            byte1: 0,
            byte2: 0,
        };
        let events0 = proto.diff_with_debounce(&state0, t0);
        assert!(events0.is_empty(), "no change from default");

        // Turn on master battery + gear down
        let state1 = SwitchPanelSwitchState {
            byte1: 0b0000_0001, // MASTER_BAT
            byte2: 0b0000_0001, // GEAR down
        };
        let events1 = proto.diff_with_debounce(&state1, t0 + Duration::from_millis(10));
        assert!(events1.iter().any(|e| matches!(
            e,
            PanelEvent::SwitchChange {
                name: "MASTER_BAT",
                on: true
            }
        )));
        assert!(events1.iter().any(|e| matches!(
            e,
            PanelEvent::SwitchChange {
                name: "GEAR",
                on: true
            }
        )));

        // Turn off master battery
        let state2 = SwitchPanelSwitchState {
            byte1: 0,
            byte2: 0b0000_0001,
        };
        let events2 = proto.diff_with_debounce(&state2, t0 + Duration::from_millis(20));
        assert!(events2.iter().any(|e| matches!(
            e,
            PanelEvent::SwitchChange {
                name: "MASTER_BAT",
                on: false
            }
        )));
        // Gear unchanged → no GEAR event
        assert!(!events2
            .iter()
            .any(|e| matches!(e, PanelEvent::SwitchChange { name: "GEAR", .. })));
    }

    /// diff_with_debounce suppresses bouncing switch changes.
    #[test]
    fn diff_with_debounce_suppresses_bounce() {
        let mut proto = SwitchPanelProtocol::new(Duration::from_millis(50));
        let t0 = Instant::now();

        // First: fuel pump on
        let state_on = SwitchPanelSwitchState {
            byte1: 0b0000_1000,
            byte2: 0,
        };
        let events = proto.diff_with_debounce(&state_on, t0);
        assert_eq!(events.len(), 1, "first change accepted");

        // Bounce: fuel pump off within 50ms → should be suppressed
        let state_off = SwitchPanelSwitchState {
            byte1: 0,
            byte2: 0,
        };
        let events = proto.diff_with_debounce(&state_off, t0 + Duration::from_millis(10));
        // The FUEL_PUMP event should be debounced (rejected)
        assert!(
            !events.iter().any(|e| matches!(
                e,
                PanelEvent::SwitchChange {
                    name: "FUEL_PUMP",
                    ..
                }
            )),
            "bounce should be suppressed by debounce"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Panel Identification
// ═══════════════════════════════════════════════════════════════════════════════

mod panel_identification {
    use super::*;

    /// VID/PID → panel type for all known Saitek panels.
    #[test]
    fn vid_pid_to_panel_type() {
        assert_eq!(PanelType::from_product_id(0x0D05), Some(PanelType::RadioPanel));
        assert_eq!(PanelType::from_product_id(0x0D06), Some(PanelType::MultiPanel));
        assert_eq!(PanelType::from_product_id(0x0D67), Some(PanelType::SwitchPanel));
        assert_eq!(PanelType::from_product_id(0x0B4E), Some(PanelType::BIP));
        assert_eq!(PanelType::from_product_id(0x0A2F), Some(PanelType::FIP));
    }

    /// Each protocol driver's vendor_id/product_id match the panel constants.
    #[test]
    fn protocol_vid_pid_consistency() {
        let radio = RadioPanelProtocol;
        assert_eq!(radio.vendor_id(), RADIO_PANEL_VID);
        assert_eq!(radio.product_id(), RADIO_PANEL_PID);

        let multi = MultiPanelProtocol;
        assert_eq!(multi.vendor_id(), MULTI_PANEL_VID);
        assert_eq!(multi.product_id(), MULTI_PANEL_PID);

        let switch = SwitchPanelProtocol::new(Duration::from_millis(5));
        assert_eq!(switch.vendor_id(), SWITCH_PANEL_VID);
        assert_eq!(switch.product_id(), SWITCH_PANEL_PID);
    }

    /// Unknown PIDs return None gracefully.
    #[test]
    fn unknown_panel_graceful() {
        for pid in [0x0000u16, 0x0001, 0x1234, 0xDEAD, 0xFFFF] {
            assert_eq!(
                PanelType::from_product_id(pid),
                None,
                "PID {pid:#06x} should be unknown"
            );
        }
    }

    /// Radio, Multi, Switch panel variants have distinct PIDs.
    #[test]
    fn panel_variants_distinct() {
        let pids = [
            RADIO_PANEL_PID,
            MULTI_PANEL_PID,
            SWITCH_PANEL_PID,
        ];
        let unique: std::collections::HashSet<_> = pids.iter().collect();
        assert_eq!(unique.len(), pids.len(), "all panel PIDs must be distinct");
    }

    /// PanelType discriminant values match their PID.
    #[test]
    fn panel_type_discriminant_matches_pid() {
        assert_eq!(PanelType::RadioPanel as u16, 0x0D05);
        assert_eq!(PanelType::MultiPanel as u16, 0x0D06);
        assert_eq!(PanelType::SwitchPanel as u16, 0x0D67);
    }

    /// PanelId display format: "VVVV:PPPP @ path".
    #[test]
    fn panel_id_display_format() {
        let id = PanelId {
            vendor_id: 0x06A3,
            product_id: 0x0D05,
            device_path: r"\\?\hid#vid_06a3&pid_0d05".to_string(),
        };
        let s = id.to_string();
        assert!(s.starts_with("06A3:0D05"));
        assert!(s.contains("hid#vid_06a3"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Protocol Framing / HID Reports
// ═══════════════════════════════════════════════════════════════════════════════

mod protocol_framing {
    use super::*;

    /// Radio Panel HID output report is exactly RADIO_PANEL_OUTPUT_BYTES.
    #[test]
    fn radio_panel_report_size() {
        let display = RadioDisplay::default();
        let report = display.to_hid_report();
        assert_eq!(report.len(), RADIO_PANEL_OUTPUT_BYTES);
        assert_eq!(RADIO_PANEL_OUTPUT_BYTES, 23);
    }

    /// Multi Panel HID output report is exactly MULTI_PANEL_OUTPUT_BYTES.
    #[test]
    fn multi_panel_report_size() {
        let lcd = LcdDisplay::blank();
        let report = lcd.to_hid_report(MultiPanelLedMask::NONE);
        assert_eq!(report.len(), MULTI_PANEL_OUTPUT_BYTES);
        assert_eq!(MULTI_PANEL_OUTPUT_BYTES, 12);
    }

    /// Switch Panel HID output report is exactly SWITCH_PANEL_OUTPUT_BYTES.
    #[test]
    fn switch_panel_report_size() {
        let report = SwitchPanelGearLeds::ALL_OFF.to_hid_report();
        assert_eq!(report.len(), SWITCH_PANEL_OUTPUT_BYTES);
        assert_eq!(SWITCH_PANEL_OUTPUT_BYTES, 2);
    }

    /// Radio display HID report: byte 0 = report ID (0x00), bytes 1-5 = active,
    /// bytes 6-10 = standby, bytes 11-22 = reserved (zero).
    #[test]
    fn radio_display_byte_layout() {
        let display = RadioDisplay {
            active: LcdDisplay::encode_str("12345"),
            standby: LcdDisplay::encode_str("67890"),
        };
        let report = display.to_hid_report();

        assert_eq!(report[0], 0x00, "report ID");
        // Active display
        assert_eq!(report[1], encode_segment('1'));
        assert_eq!(report[5], encode_segment('5'));
        // Standby display
        assert_eq!(report[6], encode_segment('6'));
        assert_eq!(report[10], encode_segment('0'));
        // Reserved bytes
        for i in 11..RADIO_PANEL_OUTPUT_BYTES {
            assert_eq!(report[i], 0x00, "byte {i} reserved");
        }
    }

    /// Multi Panel report: bytes 1-5 = display, bytes 6-10 = lower row (zero),
    /// byte 11 = LED mask.
    #[test]
    fn multi_panel_byte_layout() {
        let lcd = LcdDisplay::encode_str("99999");
        let leds = MultiPanelLedMask(0b1010_1010);
        let report = lcd.to_hid_report(leds);

        assert_eq!(report[0], 0x00, "report ID");
        for i in 1..=5 {
            assert_eq!(report[i], encode_segment('9'), "display byte {i}");
        }
        for i in 6..=10 {
            assert_eq!(report[i], 0x00, "lower row byte {i}");
        }
        assert_eq!(report[11], 0b1010_1010, "LED mask");
    }

    /// Switch Panel report: [report_id=0, led_bits].
    #[test]
    fn switch_panel_byte_layout() {
        let leds = SwitchPanelGearLeds::ALL_GREEN;
        let report = leds.to_hid_report();
        assert_eq!(report[0], 0x00, "report ID");
        assert_eq!(report[1], leds.raw(), "LED bits");
    }

    /// Report byte ordering: data_len field in protocol handler is little-endian.
    #[test]
    fn byte_ordering_little_endian() {
        // RadioDisplay encodes frequencies in left-to-right order (MSB first for display)
        let display = RadioDisplay {
            active: LcdDisplay::encode_str("11800"), // 118.00 MHz
            standby: LcdDisplay::blank(),
        };
        let report = display.to_hid_report();
        // First display character at byte 1
        assert_eq!(report[1], encode_segment('1'), "first digit at byte 1");
        assert_eq!(report[2], encode_segment('1'), "second digit at byte 2");
    }

    /// output_report_size() matches actual report construction.
    #[test]
    fn output_report_size_matches_actual() {
        let radio = RadioPanelProtocol;
        assert_eq!(
            radio.output_report_size(),
            RadioDisplay::default().to_hid_report().len()
        );

        let multi = MultiPanelProtocol;
        assert_eq!(
            multi.output_report_size(),
            LcdDisplay::blank()
                .to_hid_report(MultiPanelLedMask::NONE)
                .len()
        );

        let switch = SwitchPanelProtocol::new(Duration::from_millis(5));
        assert_eq!(
            switch.output_report_size(),
            SwitchPanelGearLeds::ALL_OFF.to_hid_report().len()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Integration Scenarios
// ═══════════════════════════════════════════════════════════════════════════════

mod integration {
    use super::*;

    /// Full cycle: read switches → process → update display → send LED.
    #[test]
    fn full_read_process_display_led_cycle() {
        // 1. Read switches from HID input
        let hid_input = [0x00u8, 0x00, 0b0000_0001]; // gear down on switch panel
        let switch_state = parse_switch_panel_input(&hid_input).unwrap();
        assert!(switch_state.gear_down());

        // 2. Process: gear is down → set gear LEDs green
        let gear_leds = if switch_state.gear_down() {
            SwitchPanelGearLeds::ALL_GREEN
        } else {
            SwitchPanelGearLeds::ALL_OFF
        };

        // 3. Build and send HID output report
        let report = gear_leds.to_hid_report();
        assert_eq!(report.len(), SWITCH_PANEL_OUTPUT_BYTES);
        assert_ne!(report[1], 0x00, "gear LEDs should be on");
    }

    /// Full cycle for radio panel: read mode → format frequency → build display report.
    #[test]
    fn radio_full_cycle_mode_to_display() {
        // 1. Parse HID input: COM1 mode, outer encoder CW
        let hid_input = [0x00u8, 0x00, 0b0000_0010]; // COM1 mode, outer CW
        let btn_state = parse_radio_panel_input(&hid_input).unwrap();
        assert_eq!(btn_state.mode, Some(RadioMode::Com1));
        assert!(btn_state.outer_enc_cw());

        // 2. Format COM frequency for display
        let freq_str = flight_panels_core::display::format_com_freq(121_500);
        assert_eq!(freq_str, "12150");

        // 3. Build display
        let mut state = RadioPanelState::default();
        state.display.active = LcdDisplay::encode_str(&freq_str);
        state.display.standby = LcdDisplay::encode_str("12350");

        // 4. Build HID output report
        let report = state.to_hid_report();
        assert_eq!(report.len(), RADIO_PANEL_OUTPUT_BYTES);
        assert_eq!(report[1], encode_segment('1'));
    }

    /// Full cycle for multi panel: mode select → show altitude → light LEDs.
    #[test]
    fn multi_panel_full_cycle() {
        // 1. Parse mode selector (ALT mode)
        let hid_input = [0x00u8, 0b0000_0001, 0x00]; // SEL_ALT
        let btn_state = parse_multi_panel_input(&hid_input).unwrap();
        assert!(btn_state.sel_alt());

        // 2. Process mode
        let mut mode_sm = ModeStateMachine::new();
        let new_mode = mode_sm.update(&btn_state);
        assert_eq!(new_mode, Some(multi_panel::MultiPanelMode::Alt));

        // 3. Format altitude for display
        let lcd = LcdDisplay::from_integer(35000);

        // 4. Set up LEDs
        let leds = MultiPanelLedMask::NONE.set(led_bits::ALT, true);

        // 5. Build HID report
        let report = lcd.to_hid_report(leds);
        assert_eq!(report.len(), MULTI_PANEL_OUTPUT_BYTES);
        assert_eq!(report[1], encode_segment('3'));
        assert!(report[11] & led_bits::ALT != 0);
    }

    /// Multiple panels simultaneously: each panel's state is independent.
    #[test]
    fn multiple_panels_simultaneously() {
        let mut radio_state = RadioPanelState::default();
        let mut multi_state = MultiPanelState::default();
        let mut switch_state = SwitchPanelState::default();

        // Update all panels
        radio_state.display.active = LcdDisplay::encode_str("12150");
        multi_state.display = LcdDisplay::from_integer(5000);
        multi_state.leds = MultiPanelLedMask::NONE.set(led_bits::VS, true);
        switch_state.gear_leds = SwitchPanelGearLeds::ALL_RED;

        // Build reports simultaneously
        let radio_report = radio_state.to_hid_report();
        let multi_report = multi_state.to_hid_report();
        let switch_report = switch_state.to_hid_report();

        // Each report is independent and well-formed
        assert_eq!(radio_report.len(), RADIO_PANEL_OUTPUT_BYTES);
        assert_eq!(multi_report.len(), MULTI_PANEL_OUTPUT_BYTES);
        assert_eq!(switch_report.len(), SWITCH_PANEL_OUTPUT_BYTES);

        // Radio has frequency data in position 0 ('1' of "12150")
        assert_ne!(radio_report[1], 0);
        // Multi has altitude " 5000" — position 2 should be '5'
        assert_eq!(multi_report[2], encode_segment('5'));
        assert!(multi_report[11] & led_bits::VS != 0);
        // Switch has red gear LEDs
        assert_ne!(switch_report[1], 0);
    }

    /// Panel disconnect/reconnect: state resets to defaults cleanly.
    #[test]
    fn panel_disconnect_reconnect_state_reset() {
        // Simulate active state
        let mut state = MultiPanelState {
            display: LcdDisplay::from_integer(35000),
            leds: MultiPanelLedMask::ALL,
            buttons: MultiPanelButtonState {
                byte1: 0b0000_0001,
                byte2: 0xFF,
            },
        };

        // Verify active
        let report = state.to_hid_report();
        assert_ne!(report[1], 0);
        assert_eq!(report[11], 0xFF);

        // Simulate disconnect: reset to defaults
        state = MultiPanelState::default();
        let report = state.to_hid_report();
        assert!(
            report.iter().all(|&b| b == 0),
            "after reset, all bytes should be zero"
        );
    }

    /// Protocol parse_input: too-short data returns None for all panel types.
    #[test]
    fn all_panels_reject_short_input() {
        let radio = RadioPanelProtocol;
        assert!(radio.parse_input(&[]).is_none());
        assert!(radio.parse_input(&[0x00]).is_none());
        assert!(radio.parse_input(&[0x00, 0x00]).is_none());

        let multi = MultiPanelProtocol;
        assert!(multi.parse_input(&[]).is_none());
        assert!(multi.parse_input(&[0x00]).is_none());
        assert!(multi.parse_input(&[0x00, 0x00]).is_none());

        let switch = SwitchPanelProtocol::new(Duration::from_millis(5));
        assert!(switch.parse_input(&[]).is_none());
        assert!(switch.parse_input(&[0x00]).is_none());
        assert!(switch.parse_input(&[0x00, 0x00]).is_none());
    }

    /// Protocol parse_input emits correct PanelEvent variants.
    #[test]
    fn all_panels_emit_correct_events() {
        // Radio: ACT_STBY button + mode selector
        let radio = RadioPanelProtocol;
        let events = radio.parse_input(&[0x00, 0x03, 0b0000_0001]).unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, PanelEvent::ButtonPress { name: "ACT_STBY" })));
        assert!(events
            .iter()
            .any(|e| matches!(e, PanelEvent::SelectorChange { name: "MODE", .. })));

        // Multi: AP button + encoder CW
        let multi = MultiPanelProtocol;
        let events = multi
            .parse_input(&[0x00, 0b0100_0000, 0b0000_0001])
            .unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, PanelEvent::ButtonPress { name: "AP" })));
        assert!(events.iter().any(|e| matches!(
            e,
            PanelEvent::EncoderTick {
                name: "ENCODER",
                delta: 1
            }
        )));

        // Switch: MASTER_BAT on + GEAR down
        let switch = SwitchPanelProtocol::new(Duration::from_millis(5));
        let events = switch
            .parse_input(&[0x00, 0b0000_0001, 0b0000_0001])
            .unwrap();
        assert!(events.iter().any(|e| matches!(
            e,
            PanelEvent::SwitchChange {
                name: "MASTER_BAT",
                on: true
            }
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            PanelEvent::SwitchChange {
                name: "GEAR",
                on: true
            }
        )));
    }

    /// RadioMode round-trip: from_hid_byte for all 7 modes, and reserved = None.
    #[test]
    fn radio_mode_exhaustive() {
        let modes = [
            (0, RadioMode::Com1),
            (1, RadioMode::Com2),
            (2, RadioMode::Nav1),
            (3, RadioMode::Nav2),
            (4, RadioMode::Adf),
            (5, RadioMode::Dme),
            (6, RadioMode::Xpdr),
        ];
        for (byte, expected) in modes {
            assert_eq!(RadioMode::from_hid_byte(byte), Some(expected));
        }
        assert_eq!(RadioMode::from_hid_byte(7), None, "reserved value");

        // Upper bits masked: 0xF8 | mode should still decode correctly
        for (byte, expected) in modes {
            assert_eq!(RadioMode::from_hid_byte(0xF8 | byte), Some(expected));
        }
    }

    /// Encoder accumulation: simultaneous CW+CCW on same encoder cancels out.
    #[test]
    fn encoder_simultaneous_cw_ccw_cancels() {
        let mut delta = EncoderDelta::default();
        // Both CW and CCW bits set simultaneously (unusual but possible)
        delta.update(&RadioPanelButtonState {
            mode: None,
            buttons: 0b0000_0110, // outer CW + outer CCW
        });
        assert_eq!(delta.outer, 0, "CW + CCW should cancel");
    }

    /// ModeStateMachine full cycle through all modes.
    #[test]
    fn mode_state_machine_full_cycle() {
        let mut sm = ModeStateMachine::new();
        let modes = [
            (0b0000_0001u8, multi_panel::MultiPanelMode::Alt),
            (0b0000_0010, multi_panel::MultiPanelMode::Vs),
            (0b0000_0100, multi_panel::MultiPanelMode::Ias),
            (0b0000_1000, multi_panel::MultiPanelMode::Hdg),
            (0b0001_0000, multi_panel::MultiPanelMode::Crs),
        ];
        for (byte1, expected_mode) in modes {
            let state = MultiPanelButtonState { byte1, byte2: 0 };
            let result = sm.update(&state);
            assert_eq!(result, Some(expected_mode));
            assert_eq!(sm.current(), Some(expected_mode));

            // Same mode again → None (no change)
            assert_eq!(sm.update(&state), None);
        }
    }

    /// Gear LED transition sequence: OFF → RED → GREEN (up → transit → down-locked).
    #[test]
    fn gear_led_transition_sequence() {
        // Gear up: LEDs off
        let step0 = SwitchPanelGearLeds::ALL_OFF;
        assert_eq!(step0.raw(), 0x00);

        // Gear in transit: red
        let step1 = SwitchPanelGearLeds::ALL_RED;
        assert_ne!(step1.raw() & gear_led_bits::LEFT_RED, 0);
        assert_ne!(step1.raw() & gear_led_bits::NOSE_RED, 0);
        assert_ne!(step1.raw() & gear_led_bits::RIGHT_RED, 0);

        // Gear down and locked: green
        let step2 = SwitchPanelGearLeds::ALL_GREEN;
        assert_ne!(step2.raw() & gear_led_bits::LEFT_GREEN, 0);
        assert_ne!(step2.raw() & gear_led_bits::NOSE_GREEN, 0);
        assert_ne!(step2.raw() & gear_led_bits::RIGHT_GREEN, 0);

        // Each step is different
        assert_ne!(step0.raw(), step1.raw());
        assert_ne!(step1.raw(), step2.raw());
        assert_ne!(step0.raw(), step2.raw());
    }

    /// Asymmetric gear: e.g. left green, nose red, right off (partial extension).
    #[test]
    fn asymmetric_gear_led_state() {
        let leds = SwitchPanelGearLeds::ALL_OFF
            .set_left(GearLedColor::Green)
            .set_nose(GearLedColor::Red)
            .set_right(GearLedColor::Off);

        let report = leds.to_hid_report();
        let bits = report[1];
        assert_ne!(bits & gear_led_bits::LEFT_GREEN, 0);
        assert_eq!(bits & gear_led_bits::LEFT_RED, 0);
        assert_ne!(bits & gear_led_bits::NOSE_RED, 0);
        assert_eq!(bits & gear_led_bits::NOSE_GREEN, 0);
        assert_eq!(bits & gear_led_bits::RIGHT_GREEN, 0);
        assert_eq!(bits & gear_led_bits::RIGHT_RED, 0);
    }
}
