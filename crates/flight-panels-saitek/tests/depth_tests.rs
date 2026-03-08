// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the flight-panels-saitek crate.
//!
//! Exercises edge cases, cross-module interactions, and protocol-level
//! invariants that go beyond the per-module unit tests.

use flight_panels_core::protocol::{PanelEvent, PanelProtocol};
use flight_panels_saitek::bip::{BIP_LEDS_PER_STRIP, BIP_STRIP_COUNT, BipLedColor, BipState};
use flight_panels_saitek::fip::{
    FIP_HEIGHT, FIP_PID, FIP_VID, FIP_WIDTH, FipButton, FipButtonState, FipFrame, FipPageManager,
    FipProtocol, FipScrollWheel, FipSoftKeys,
};
use flight_panels_saitek::multi_panel::{
    LcdDisplay, MULTI_PANEL_INPUT_MIN_BYTES, MULTI_PANEL_OUTPUT_BYTES, MULTI_PANEL_PID,
    MULTI_PANEL_VID, ModeStateMachine, MultiPanelButtonState, MultiPanelLedMask, MultiPanelMode,
    MultiPanelProtocol, MultiPanelState, encode_segment, led_bits, parse_multi_panel_input,
};
use flight_panels_saitek::radio_panel::{
    EncoderDelta, RADIO_PANEL_INPUT_MIN_BYTES, RADIO_PANEL_OUTPUT_BYTES, RADIO_PANEL_PID,
    RADIO_PANEL_VID, RadioDisplay, RadioMode, RadioPanelButtonState, RadioPanelProtocol,
    RadioPanelState, parse_radio_panel_input,
};
use flight_panels_saitek::switch_panel::{
    GearLedColor, MagnetoPosition, SWITCH_PANEL_INPUT_MIN_BYTES, SWITCH_PANEL_OUTPUT_BYTES,
    SWITCH_PANEL_PID, SWITCH_PANEL_VID, SwitchDebounce, SwitchPanelGearLeds, SwitchPanelProtocol,
    SwitchPanelState, SwitchPanelSwitchState, gear_led_bits, parse_switch_panel_input,
};
use std::collections::HashSet;
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════════════════════════════════════════
//  Radio Panel — depth tests
// ═══════════════════════════════════════════════════════════════════════════════

/// RadioMode discriminant values must match their HID byte encoding.
#[test]
fn radio_mode_discriminant_matches_hid_encoding() {
    for val in 0u8..7 {
        let mode = RadioMode::from_hid_byte(val).unwrap();
        assert_eq!(
            mode as u8, val,
            "RadioMode discriminant should equal HID byte {val}"
        );
    }
}

/// Encoding 0xFF (all bits set) must still extract a valid mode from the low 3 bits.
#[test]
fn radio_mode_from_hid_byte_0xff() {
    // 0xFF & 0x07 = 7 → reserved
    assert!(RadioMode::from_hid_byte(0xFF).is_none());
}

/// RadioMode labels are unique across all variants.
#[test]
fn radio_mode_labels_are_unique() {
    let labels: HashSet<&str> = [
        RadioMode::Com1,
        RadioMode::Com2,
        RadioMode::Nav1,
        RadioMode::Nav2,
        RadioMode::Adf,
        RadioMode::Dme,
        RadioMode::Xpdr,
    ]
    .iter()
    .map(|m| m.label())
    .collect();
    assert_eq!(labels.len(), 7);
}

/// Simultaneous CW + CCW encoder bits should net to zero delta.
#[test]
fn encoder_delta_simultaneous_cw_ccw_nets_zero() {
    let mut delta = EncoderDelta::default();
    // Both outer CW (bit 1) and outer CCW (bit 2) set
    let state = RadioPanelButtonState {
        mode: Some(RadioMode::Com1),
        buttons: 0b0000_0110,
    };
    delta.update(&state);
    assert_eq!(delta.outer, 0, "CW + CCW in same tick should cancel out");
}

/// Simultaneous inner CW + CCW should also cancel.
#[test]
fn encoder_delta_simultaneous_inner_cw_ccw() {
    let mut delta = EncoderDelta::default();
    let state = RadioPanelButtonState {
        mode: None,
        buttons: 0b0001_1000, // inner CW (bit 3) + inner CCW (bit 4)
    };
    delta.update(&state);
    assert_eq!(delta.inner, 0);
}

/// Drain returns accumulated values and subsequent drain returns zero.
#[test]
fn encoder_delta_double_drain() {
    let mut delta = EncoderDelta {
        outer: 5,
        inner: -3,
    };
    let (o1, i1) = delta.drain();
    assert_eq!((o1, i1), (5, -3));
    let (o2, i2) = delta.drain();
    assert_eq!((o2, i2), (0, 0));
}

/// A long sequence of alternating encoder ticks accumulates correctly.
#[test]
fn encoder_delta_long_sequence() {
    let mut delta = EncoderDelta::default();
    let cw = RadioPanelButtonState {
        mode: None,
        buttons: 0b0000_0010, // outer CW
    };
    let ccw = RadioPanelButtonState {
        mode: None,
        buttons: 0b0000_0100, // outer CCW
    };

    for _ in 0..100 {
        delta.update(&cw);
    }
    for _ in 0..30 {
        delta.update(&ccw);
    }
    assert_eq!(delta.outer, 70);
}

/// RadioDisplay HID report byte 0 is always the report ID (0x00).
#[test]
fn radio_display_hid_report_id() {
    let display = RadioDisplay {
        active: LcdDisplay::encode_str("99999"),
        standby: LcdDisplay::encode_str("11111"),
    };
    let report = display.to_hid_report();
    assert_eq!(report[0], 0x00);
    assert_eq!(report.len(), RADIO_PANEL_OUTPUT_BYTES);
}

/// Radio panel state report matches display report.
#[test]
fn radio_panel_state_report_matches_display() {
    let state = RadioPanelState {
        display: RadioDisplay {
            active: LcdDisplay::encode_str("12345"),
            standby: LcdDisplay::encode_str("67890"),
        },
        buttons: RadioPanelButtonState {
            mode: Some(RadioMode::Nav1),
            buttons: 0xFF,
        },
    };
    assert_eq!(
        state.to_hid_report(),
        state.display.to_hid_report(),
        "state report should delegate entirely to display"
    );
}

/// parse_radio_panel_input with exactly RADIO_PANEL_INPUT_MIN_BYTES works.
#[test]
fn radio_parse_exact_min_bytes() {
    let data = vec![0u8; RADIO_PANEL_INPUT_MIN_BYTES];
    assert!(parse_radio_panel_input(&data).is_some());
}

/// parse_radio_panel_input with extra bytes still works (ignores trailing data).
#[test]
fn radio_parse_extra_bytes_ignored() {
    let data = [0x00u8, 0x03, 0b0000_0001, 0xFF, 0xFF, 0xFF];
    let state = parse_radio_panel_input(&data).unwrap();
    assert_eq!(state.mode, Some(RadioMode::Nav2));
    assert!(state.act_stby());
}

/// RadioPanelProtocol: empty HID report produces no events (returns None).
#[test]
fn radio_protocol_empty_returns_none() {
    let proto = RadioPanelProtocol;
    assert!(proto.parse_input(&[]).is_none());
}

/// RadioPanelProtocol: all buttons and encoder bits set produces multiple events.
#[test]
fn radio_protocol_all_bits_set() {
    let proto = RadioPanelProtocol;
    // All button bits (0..4) set, mode = Com1
    let data = [0x00u8, 0x00, 0b0001_1111];
    let events = proto.parse_input(&data).unwrap();
    // Should have: ACT_STBY + OUTER CW + OUTER CCW + INNER CW + INNER CCW + MODE
    assert!(
        events.len() >= 5,
        "expected at least 5 events, got {}",
        events.len()
    );
}

/// RadioPanelProtocol: metadata matches constants.
#[test]
fn radio_protocol_constants() {
    let proto = RadioPanelProtocol;
    assert_eq!(proto.vendor_id(), RADIO_PANEL_VID);
    assert_eq!(proto.product_id(), RADIO_PANEL_PID);
    assert_eq!(proto.output_report_size(), RADIO_PANEL_OUTPUT_BYTES);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Multi Panel — depth tests
// ═══════════════════════════════════════════════════════════════════════════════

/// Each digit 0–9 encodes to a unique, non-zero 7-segment byte.
#[test]
fn encode_segment_digits_are_unique_and_nonzero() {
    let mut seen = HashSet::new();
    for d in '0'..='9' {
        let enc = encode_segment(d);
        assert_ne!(enc, 0x00, "digit '{d}' must not encode to blank");
        assert!(seen.insert(enc), "digit '{d}' encoding must be unique");
    }
}

/// encode_segment('-') uses only the middle horizontal segment (bit 6).
#[test]
fn encode_segment_dash_only_bit6() {
    let dash = encode_segment('-');
    assert_eq!(
        dash, 0b0100_0000,
        "dash should be exactly bit 6 (segment g)"
    );
}

/// LcdDisplay::from_integer clamps values above 99999.
#[test]
fn lcd_from_integer_clamp_overflow() {
    let lcd = LcdDisplay::from_integer(100_000);
    // Should clamp to 99999
    assert_eq!(lcd.raw(0), encode_segment('9'));
    assert_eq!(lcd.raw(4), encode_segment('9'));
}

/// LcdDisplay::from_integer clamps negative values to -9999.
#[test]
fn lcd_from_integer_clamp_negative_overflow() {
    let lcd = LcdDisplay::from_integer(-99_999);
    // "-9999"
    assert_eq!(lcd.raw(0), encode_segment('-'));
    assert_eq!(lcd.raw(1), encode_segment('9'));
    assert_eq!(lcd.raw(4), encode_segment('9'));
}

/// LcdDisplay::from_integer for -1 shows "-   1".
#[test]
fn lcd_from_integer_minus_one() {
    let lcd = LcdDisplay::from_integer(-1);
    assert_eq!(lcd.raw(0), encode_segment('-'));
    assert_eq!(lcd.raw(4), encode_segment('1'));
}

/// LcdDisplay set_raw and raw round-trip for arbitrary bytes.
#[test]
fn lcd_set_raw_roundtrip() {
    let mut lcd = LcdDisplay::blank();
    for pos in 0..5 {
        let val = (pos as u8) * 50 + 7;
        lcd.set_raw(pos, val);
        assert_eq!(lcd.raw(pos), val);
    }
}

/// LcdDisplay::raw out of bounds returns 0.
#[test]
fn lcd_raw_out_of_bounds() {
    let lcd = LcdDisplay::encode_str("12345");
    assert_eq!(lcd.raw(5), 0);
    assert_eq!(lcd.raw(100), 0);
}

/// MultiPanelLedMask set and clear operations are idempotent.
#[test]
fn led_mask_set_clear_idempotent() {
    let mask = MultiPanelLedMask::NONE;
    let set_twice = mask.set(led_bits::ALT, true).set(led_bits::ALT, true);
    assert_eq!(set_twice.raw(), led_bits::ALT);

    let clear_twice = set_twice
        .set(led_bits::ALT, false)
        .set(led_bits::ALT, false);
    assert_eq!(clear_twice.raw(), 0);
}

/// MultiPanelLedMask: setting one bit doesn't affect others.
#[test]
fn led_mask_bit_independence() {
    let all_bits = [
        led_bits::ALT,
        led_bits::VS,
        led_bits::IAS,
        led_bits::HDG,
        led_bits::CRS,
        led_bits::AUTO_THROTTLE,
        led_bits::FLAPS,
        led_bits::PITCH_TRIM,
    ];
    for &target in &all_bits {
        let mask = MultiPanelLedMask::NONE.set(target, true);
        for &other in &all_bits {
            if other == target {
                assert!(mask.is_set(other));
            } else {
                assert!(
                    !mask.is_set(other),
                    "setting {target:#04x} should not affect {other:#04x}"
                );
            }
        }
    }
}

/// ModeStateMachine full cycle through all 5 modes.
#[test]
fn mode_state_machine_full_cycle() {
    let mut sm = ModeStateMachine::new();
    let modes = [
        (0b0000_0001u8, MultiPanelMode::Alt),
        (0b0000_0010, MultiPanelMode::Vs),
        (0b0000_0100, MultiPanelMode::Ias),
        (0b0000_1000, MultiPanelMode::Hdg),
        (0b0001_0000, MultiPanelMode::Crs),
    ];
    for (byte1, expected) in modes {
        let state = MultiPanelButtonState { byte1, byte2: 0 };
        let changed = sm.update(&state);
        assert_eq!(changed, Some(expected));
        assert_eq!(sm.current(), Some(expected));
    }
}

/// ModeStateMachine: clearing all mode bits returns None and updates current.
#[test]
fn mode_state_machine_clear_all_modes() {
    let mut sm = ModeStateMachine::new();
    let alt = MultiPanelButtonState {
        byte1: 0b0000_0001,
        byte2: 0,
    };
    sm.update(&alt);
    assert_eq!(sm.current(), Some(MultiPanelMode::Alt));

    let none = MultiPanelButtonState {
        byte1: 0b0000_0000,
        byte2: 0,
    };
    let _result = sm.update(&none);
    // After clearing all mode bits, current should be None
    assert_eq!(
        sm.current(),
        None,
        "mode should be cleared after zeroing all bits"
    );
}

/// MultiPanelState HID report has correct LED byte position.
#[test]
fn multi_panel_state_led_byte_position() {
    let state = MultiPanelState {
        display: LcdDisplay::blank(),
        leds: MultiPanelLedMask(0xAB),
        ..Default::default()
    };
    let report = state.to_hid_report();
    assert_eq!(report[11], 0xAB);
}

/// parse_multi_panel_input: exact minimum bytes.
#[test]
fn multi_parse_exact_min_bytes() {
    let data = vec![0u8; MULTI_PANEL_INPUT_MIN_BYTES];
    assert!(parse_multi_panel_input(&data).is_some());
}

/// MultiPanelProtocol: parse_input with mode + encoder + button simultaneously.
#[test]
fn multi_protocol_combined_events() {
    let proto = MultiPanelProtocol;
    // byte1: SEL_ALT (bit 0) + ENC_CW (bit 6)
    // byte2: AP (bit 0) + ALT (bit 4)
    let data = [0x00u8, 0b0100_0001, 0b0001_0001];
    let events = proto.parse_input(&data).unwrap();

    let has_mode = events
        .iter()
        .any(|e| matches!(e, PanelEvent::SelectorChange { name: "MODE", .. }));
    let has_enc = events.iter().any(|e| {
        matches!(
            e,
            PanelEvent::EncoderTick {
                name: "ENCODER",
                delta: 1
            }
        )
    });
    let has_ap = events
        .iter()
        .any(|e| matches!(e, PanelEvent::ButtonPress { name: "AP" }));
    let has_alt = events
        .iter()
        .any(|e| matches!(e, PanelEvent::ButtonPress { name: "ALT" }));

    assert!(has_mode, "should have MODE selector event");
    assert!(has_enc, "should have ENCODER tick event");
    assert!(has_ap, "should have AP button event");
    assert!(has_alt, "should have ALT button event");
}

/// MultiPanelProtocol: too-short input returns None.
#[test]
fn multi_protocol_too_short() {
    let proto = MultiPanelProtocol;
    assert!(proto.parse_input(&[0x00]).is_none());
    assert!(proto.parse_input(&[]).is_none());
}

/// Multi Panel output report size matches constant.
#[test]
fn multi_panel_output_report_size() {
    let proto = MultiPanelProtocol;
    assert_eq!(proto.output_report_size(), MULTI_PANEL_OUTPUT_BYTES);
    assert_eq!(proto.output_report_size(), 12);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Switch Panel — depth tests
// ═══════════════════════════════════════════════════════════════════════════════

/// MagnetoPosition labels are unique.
#[test]
fn magneto_position_labels_unique() {
    let labels: HashSet<&str> = [
        MagnetoPosition::Off,
        MagnetoPosition::Right,
        MagnetoPosition::Left,
        MagnetoPosition::Both,
        MagnetoPosition::Start,
    ]
    .iter()
    .map(|m| m.label())
    .collect();
    assert_eq!(labels.len(), 5);
}

/// MagnetoPosition discriminant values match their encoded HID positions.
#[test]
fn magneto_discriminant_matches_hid() {
    let mappings = [
        (0b0000_0000u8, MagnetoPosition::Off, 0u8),
        (0b0000_0010, MagnetoPosition::Right, 1),
        (0b0000_0100, MagnetoPosition::Left, 2),
        (0b0000_0110, MagnetoPosition::Both, 3),
        (0b0000_1000, MagnetoPosition::Start, 4),
    ];
    for (hid_byte, expected, disc) in mappings {
        let pos = MagnetoPosition::from_hid_bits(hid_byte).unwrap();
        assert_eq!(pos, expected);
        assert_eq!(pos as u8, disc);
    }
}

/// Switch state individual toggle bits are independent.
#[test]
fn switch_state_individual_bit_independence() {
    let accessors: &[fn(&SwitchPanelSwitchState) -> bool] = &[
        SwitchPanelSwitchState::master_battery,
        SwitchPanelSwitchState::master_alternator,
        SwitchPanelSwitchState::avionics_master,
        SwitchPanelSwitchState::fuel_pump,
        SwitchPanelSwitchState::de_ice,
        SwitchPanelSwitchState::pitot_heat,
        SwitchPanelSwitchState::cowl_flaps_closed,
        SwitchPanelSwitchState::panel_light,
    ];

    for (bit, accessor) in accessors.iter().enumerate() {
        let state = SwitchPanelSwitchState {
            byte1: 1 << bit,
            byte2: 0,
        };
        assert!(accessor(&state), "bit {bit} accessor should return true");
        // All other bits should be false
        for (other_bit, other_accessor) in accessors.iter().enumerate() {
            if other_bit != bit {
                assert!(
                    !other_accessor(&state),
                    "bit {other_bit} should be false when only bit {bit} is set"
                );
            }
        }
    }
}

/// Gear LED colors cycle correctly: Off → Green → Red → Off.
#[test]
fn gear_led_color_cycle() {
    let leds = SwitchPanelGearLeds::ALL_OFF;

    let green = leds.set_left(GearLedColor::Green);
    assert_ne!(green.raw() & gear_led_bits::LEFT_GREEN, 0);
    assert_eq!(green.raw() & gear_led_bits::LEFT_RED, 0);

    let red = green.set_left(GearLedColor::Red);
    assert_eq!(red.raw() & gear_led_bits::LEFT_GREEN, 0);
    assert_ne!(red.raw() & gear_led_bits::LEFT_RED, 0);

    let off = red.set_left(GearLedColor::Off);
    assert_eq!(off.raw() & gear_led_bits::LEFT_GREEN, 0);
    assert_eq!(off.raw() & gear_led_bits::LEFT_RED, 0);
}

/// Gear LEDs: mixed colors per gear leg (realistic transit scenario).
#[test]
fn gear_led_mixed_transit_scenario() {
    // Left gear locked (green), nose in transit (red), right gear up (off)
    let leds = SwitchPanelGearLeds::ALL_OFF
        .set_left(GearLedColor::Green)
        .set_nose(GearLedColor::Red)
        .set_right(GearLedColor::Off);

    assert_ne!(leds.raw() & gear_led_bits::LEFT_GREEN, 0);
    assert_eq!(leds.raw() & gear_led_bits::LEFT_RED, 0);
    assert_eq!(leds.raw() & gear_led_bits::NOSE_GREEN, 0);
    assert_ne!(leds.raw() & gear_led_bits::NOSE_RED, 0);
    assert_eq!(leds.raw() & gear_led_bits::RIGHT_GREEN, 0);
    assert_eq!(leds.raw() & gear_led_bits::RIGHT_RED, 0);
}

/// Gear LED HID report is always SWITCH_PANEL_OUTPUT_BYTES long.
#[test]
fn gear_led_hid_report_length() {
    for color in [GearLedColor::Off, GearLedColor::Green, GearLedColor::Red] {
        let leds = SwitchPanelGearLeds::ALL_OFF.set_all(color);
        let report = leds.to_hid_report();
        assert_eq!(report.len(), SWITCH_PANEL_OUTPUT_BYTES);
        assert_eq!(report[0], 0x00, "report ID must always be 0x00");
    }
}

/// SwitchDebounce with zero period always accepts.
#[test]
fn switch_debounce_zero_period_always_accepts() {
    let mut debounce = SwitchDebounce::new(Duration::ZERO);
    let now = Instant::now();
    for i in 0..10 {
        assert!(debounce.accept(0, now + Duration::from_nanos(i)));
    }
}

/// SwitchDebounce: acceptance at exact boundary.
#[test]
fn switch_debounce_exact_boundary() {
    let period = Duration::from_millis(50);
    let mut debounce = SwitchDebounce::new(period);
    let now = Instant::now();

    assert!(debounce.accept(0, now));
    // At exactly the boundary period — should still be rejected (< means strictly less)
    assert!(!debounce.accept(0, now + period - Duration::from_nanos(1)));
    // At exactly the period — should be accepted (duration_since == period, not < period)
    assert!(debounce.accept(0, now + period));
}

/// SwitchDebounce period getter.
#[test]
fn switch_debounce_period_getter() {
    let period = Duration::from_millis(42);
    let debounce = SwitchDebounce::new(period);
    assert_eq!(debounce.period(), period);
}

/// SwitchPanelProtocol: parse_input with all switches on.
#[test]
fn switch_protocol_parse_all_switches_on() {
    let proto = SwitchPanelProtocol::new(Duration::from_millis(5));
    let data = [0x00u8, 0xFF, 0xFF];
    let events = proto.parse_input(&data).unwrap();

    // Should have 8 SwitchChange events from byte1 + GEAR + MAGNETO selector
    let switch_count = events
        .iter()
        .filter(|e| matches!(e, PanelEvent::SwitchChange { .. }))
        .count();
    assert!(
        switch_count >= 9,
        "expected 9 switch events, got {switch_count}"
    );
}

/// SwitchPanelProtocol parse_input returns None for too-short data.
#[test]
fn switch_protocol_parse_too_short() {
    let proto = SwitchPanelProtocol::new(Duration::from_millis(5));
    assert!(proto.parse_input(&[0x00, 0x00]).is_none());
    assert!(proto.parse_input(&[]).is_none());
}

/// SwitchPanelProtocol metadata.
#[test]
fn switch_protocol_metadata() {
    let proto = SwitchPanelProtocol::new(Duration::from_millis(5));
    assert_eq!(proto.vendor_id(), SWITCH_PANEL_VID);
    assert_eq!(proto.product_id(), SWITCH_PANEL_PID);
    assert_eq!(proto.output_report_size(), SWITCH_PANEL_OUTPUT_BYTES);
    assert_eq!(proto.led_names().len(), 6);
}

/// SwitchPanelProtocol diff_with_debounce: multiple switches changing together.
#[test]
fn switch_diff_multiple_simultaneous_changes() {
    let mut proto = SwitchPanelProtocol::new(Duration::ZERO);
    let now = Instant::now();

    // Start with all off
    let off = SwitchPanelSwitchState {
        byte1: 0x00,
        byte2: 0x00,
    };
    let _ = proto.diff_with_debounce(&off, now);

    // Turn on master battery, avionics, and gear simultaneously
    let on = SwitchPanelSwitchState {
        byte1: 0b0000_0101, // master_battery + avionics
        byte2: 0b0000_0001, // gear down
    };
    let events = proto.diff_with_debounce(&on, now + Duration::from_millis(10));
    let names: HashSet<&str> = events
        .iter()
        .filter_map(|e| match e {
            PanelEvent::SwitchChange { name, on: true } => Some(*name),
            _ => None,
        })
        .collect();
    assert!(names.contains("MASTER_BAT"));
    assert!(names.contains("AVIONICS"));
    assert!(names.contains("GEAR"));
}

/// SwitchPanelState default report is all zeros.
#[test]
fn switch_panel_state_default_zeros() {
    let state = SwitchPanelState::default();
    let report = state.to_hid_report();
    assert_eq!(report, [0x00, 0x00]);
}

/// parse_switch_panel_input exact minimum size.
#[test]
fn switch_parse_exact_min_bytes() {
    let data = vec![0u8; SWITCH_PANEL_INPUT_MIN_BYTES];
    assert!(parse_switch_panel_input(&data).is_some());
}

// ═══════════════════════════════════════════════════════════════════════════════
//  BIP — depth tests
// ═══════════════════════════════════════════════════════════════════════════════

/// BIP default and new produce identical states.
#[test]
fn bip_default_equals_new() {
    let default_state = BipState::default();
    let new_state = BipState::new();
    for strip in 0..BIP_STRIP_COUNT {
        for pos in 0..BIP_LEDS_PER_STRIP {
            assert_eq!(
                default_state.get_led(strip, pos),
                new_state.get_led(strip, pos)
            );
        }
    }
}

/// Setting all LEDs to each color produces correct counts.
#[test]
fn bip_fill_strip_all_colors() {
    for color in [
        BipLedColor::Off,
        BipLedColor::Green,
        BipLedColor::Amber,
        BipLedColor::Red,
    ] {
        let mut state = BipState::new();
        for pos in 0..BIP_LEDS_PER_STRIP {
            state.set_led(0, pos, color);
        }
        assert_eq!(state.count_color(0, color), BIP_LEDS_PER_STRIP);
        // Strip 1 should be unaffected
        assert_eq!(state.count_color(1, BipLedColor::Off), BIP_LEDS_PER_STRIP);
    }
}

/// BipLedColor repr values are 0, 1, 2, 3.
#[test]
fn bip_led_color_repr_values() {
    assert_eq!(BipLedColor::Off as u8, 0);
    assert_eq!(BipLedColor::Green as u8, 1);
    assert_eq!(BipLedColor::Amber as u8, 2);
    assert_eq!(BipLedColor::Red as u8, 3);
}

/// encode_strip with a pattern produces correct byte output.
#[test]
fn bip_encode_strip_pattern() {
    let mut state = BipState::new();
    // Alternating green/red pattern
    for pos in 0..BIP_LEDS_PER_STRIP {
        let color = if pos % 2 == 0 {
            BipLedColor::Green
        } else {
            BipLedColor::Red
        };
        state.set_led(0, pos, color);
    }
    let report = state.encode_strip(0);
    for pos in 0..BIP_LEDS_PER_STRIP {
        let expected = if pos % 2 == 0 { 1u8 } else { 3u8 };
        assert_eq!(report[pos], expected, "pos {pos}");
    }
}

/// BIP strips are independent: setting one strip leaves the other untouched.
#[test]
fn bip_strips_independent() {
    let mut state = BipState::new();
    for pos in 0..BIP_LEDS_PER_STRIP {
        state.set_led(0, pos, BipLedColor::Amber);
    }
    // Strip 1 should be untouched
    let strip1 = state.encode_strip(1);
    assert_eq!(strip1, [0u8; BIP_LEDS_PER_STRIP]);
}

/// BIP set_led overwrites previous color.
#[test]
fn bip_set_led_overwrite() {
    let mut state = BipState::new();
    state.set_led(0, 5, BipLedColor::Green);
    assert_eq!(state.get_led(0, 5), Some(BipLedColor::Green));
    state.set_led(0, 5, BipLedColor::Red);
    assert_eq!(state.get_led(0, 5), Some(BipLedColor::Red));
    state.set_led(0, 5, BipLedColor::Off);
    assert_eq!(state.get_led(0, 5), Some(BipLedColor::Off));
}

// ═══════════════════════════════════════════════════════════════════════════════
//  FIP — depth tests
// ═══════════════════════════════════════════════════════════════════════════════

/// FipFrame pixel addressing at row boundaries.
#[test]
fn fip_frame_row_boundary_pixels() {
    let mut frame = FipFrame::new();
    // End of first row
    frame.set_pixel_rgb565(FIP_WIDTH - 1, 0, 0xAAAA);
    assert_eq!(frame.get_pixel_rgb565(FIP_WIDTH - 1, 0), 0xAAAA);
    // Start of second row
    frame.set_pixel_rgb565(0, 1, 0xBBBB);
    assert_eq!(frame.get_pixel_rgb565(0, 1), 0xBBBB);
    // They should not interfere
    assert_eq!(frame.get_pixel_rgb565(FIP_WIDTH - 1, 0), 0xAAAA);
}

/// FipFrame: writing all corners.
#[test]
fn fip_frame_all_corners() {
    let mut frame = FipFrame::new();
    let corners = [
        (0, 0, 0x0001u16),
        (FIP_WIDTH - 1, 0, 0x0002),
        (0, FIP_HEIGHT - 1, 0x0003),
        (FIP_WIDTH - 1, FIP_HEIGHT - 1, 0x0004),
    ];
    for &(x, y, val) in &corners {
        frame.set_pixel_rgb565(x, y, val);
    }
    for &(x, y, val) in &corners {
        assert_eq!(frame.get_pixel_rgb565(x, y), val, "corner ({x}, {y})");
    }
}

/// FipFrame default is the same as new.
#[test]
fn fip_frame_default_eq_new() {
    let default_frame = FipFrame::default();
    let new_frame = FipFrame::new();
    assert_eq!(default_frame.pixels, new_frame.pixels);
}

/// FipButtonState with combined page + rotary.
#[test]
fn fip_button_combined_page_and_rotary() {
    // Page2 (bit 1) + RotaryCw (bit 6)
    let state = FipButtonState(0b0100_0010);
    assert!(state.is_pressed(FipButton::Page2));
    assert!(state.is_pressed(FipButton::RotaryCw));
    assert!(!state.is_pressed(FipButton::Page1));
    assert!(!state.is_pressed(FipButton::RotaryCcw));
}

/// FipScrollWheel: CW and CCW in alternation.
#[test]
fn fip_scroll_wheel_alternating() {
    let mut sw = FipScrollWheel::default();
    let cw = FipButtonState(1 << 6);
    let ccw = FipButtonState(1 << 7);
    sw.update(&cw);
    sw.update(&ccw);
    sw.update(&cw);
    assert_eq!(sw.accumulated, 1); // +1, -1, +1 = 1
}

/// FipScrollWheel drain followed by more updates.
#[test]
fn fip_scroll_wheel_drain_then_update() {
    let mut sw = FipScrollWheel::default();
    let cw = FipButtonState(1 << 6);
    sw.update(&cw);
    sw.update(&cw);
    assert_eq!(sw.drain(), 2);
    assert_eq!(sw.accumulated, 0);
    sw.update(&cw);
    assert_eq!(sw.accumulated, 1);
}

/// FipSoftKeys: overwriting existing labels.
#[test]
fn fip_soft_keys_overwrite() {
    let mut keys = FipSoftKeys::new();
    keys.set_label(0, "NAV");
    assert_eq!(keys.label(0), "NAV");
    keys.set_label(0, "MAP");
    assert_eq!(keys.label(0), "MAP");
}

/// FipSoftKeys labels array length is always 6.
#[test]
fn fip_soft_keys_labels_len() {
    let keys = FipSoftKeys::default();
    assert_eq!(keys.labels().len(), 6);
}

/// FipPageManager: page beyond count is ignored.
#[test]
fn fip_page_manager_select_beyond_count() {
    let mut pm = FipPageManager::new(3);
    pm.select(2); // valid
    assert_eq!(pm.current(), 2);
    pm.select(3); // invalid, should be ignored
    assert_eq!(pm.current(), 2);
    pm.select(0); // back to page 0
    assert_eq!(pm.current(), 0);
}

/// FipPageManager: handle_button for pages beyond page_count.
#[test]
fn fip_page_manager_button_beyond_count() {
    let mut pm = FipPageManager::new(2); // only pages 0, 1
    assert!(!pm.handle_button(FipButton::Page3)); // page 2 is beyond count
    assert_eq!(pm.current(), 0);
    assert!(pm.handle_button(FipButton::Page2)); // page 1 is valid
    assert_eq!(pm.current(), 1);
}

/// FipProtocol: parse_input with multiple page buttons simultaneously.
#[test]
fn fip_protocol_multiple_pages_pressed() {
    let proto = FipProtocol;
    // Page1 + Page3 + RotaryCcw
    let events = proto.parse_input(&[0b1000_0101]).unwrap();
    let page_presses: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, PanelEvent::ButtonPress { .. }))
        .collect();
    // Should have PAGE1 and PAGE3
    assert!(page_presses.len() >= 2);
}

/// FipProtocol metadata matches constants.
#[test]
fn fip_protocol_metadata() {
    let proto = FipProtocol;
    assert_eq!(proto.vendor_id(), FIP_VID);
    assert_eq!(proto.product_id(), FIP_PID);
    assert!(proto.led_names().is_empty());
    assert_eq!(proto.output_report_size(), FIP_WIDTH * FIP_HEIGHT * 2);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Cross-module / protocol trait consistency
// ═══════════════════════════════════════════════════════════════════════════════

/// All PanelProtocol implementations have non-empty names.
#[test]
fn all_protocols_have_nonempty_names() {
    let protos: Vec<Box<dyn PanelProtocol>> = vec![
        Box::new(RadioPanelProtocol),
        Box::new(MultiPanelProtocol),
        Box::new(SwitchPanelProtocol::new(Duration::from_millis(5))),
        Box::new(FipProtocol),
    ];
    for proto in &protos {
        assert!(!proto.name().is_empty(), "protocol name must not be empty");
    }
}

/// All PanelProtocol implementations have non-zero vendor/product IDs.
#[test]
fn all_protocols_have_valid_ids() {
    let protos: Vec<Box<dyn PanelProtocol>> = vec![
        Box::new(RadioPanelProtocol),
        Box::new(MultiPanelProtocol),
        Box::new(SwitchPanelProtocol::new(Duration::from_millis(5))),
        Box::new(FipProtocol),
    ];
    for proto in &protos {
        assert_ne!(proto.vendor_id(), 0, "{}: VID must not be 0", proto.name());
        assert_ne!(proto.product_id(), 0, "{}: PID must not be 0", proto.name());
        assert!(
            proto.output_report_size() > 0,
            "{}: output size must be > 0",
            proto.name()
        );
    }
}

/// All protocols reject empty input.
#[test]
fn all_protocols_reject_empty_input() {
    let protos: Vec<Box<dyn PanelProtocol>> = vec![
        Box::new(RadioPanelProtocol),
        Box::new(MultiPanelProtocol),
        Box::new(SwitchPanelProtocol::new(Duration::from_millis(5))),
        Box::new(FipProtocol),
    ];
    for proto in &protos {
        assert!(
            proto.parse_input(&[]).is_none(),
            "{}: empty input must return None",
            proto.name()
        );
    }
}

/// USB VID/PID constants are consistent across modules.
#[test]
fn vid_pid_consistency() {
    // All Saitek panels share a consistent VID
    assert_eq!(RADIO_PANEL_VID, MULTI_PANEL_VID);
    assert_eq!(MULTI_PANEL_VID, SWITCH_PANEL_VID);
    assert_eq!(SWITCH_PANEL_VID, FIP_VID);

    // PIDs are all unique
    let pids: HashSet<u16> = [RADIO_PANEL_PID, MULTI_PANEL_PID, SWITCH_PANEL_PID, FIP_PID]
        .into_iter()
        .collect();
    assert_eq!(pids.len(), 4, "all panel PIDs must be distinct");
}

/// LcdDisplay is shared between Radio and Multi panels (used by RadioDisplay).
#[test]
fn lcd_display_shared_usage() {
    let lcd = LcdDisplay::encode_str("12345");
    // Used in RadioDisplay
    let radio_display = RadioDisplay {
        active: lcd.clone(),
        standby: LcdDisplay::blank(),
    };
    let radio_report = radio_display.to_hid_report();
    // Used in MultiPanelState
    let multi_state = MultiPanelState {
        display: lcd.clone(),
        leds: MultiPanelLedMask::NONE,
        ..Default::default()
    };
    let multi_report = multi_state.to_hid_report();
    // Active frequency bytes (1..6) in radio should match display bytes (1..6) in multi
    assert_eq!(radio_report[1..6], multi_report[1..6]);
}

/// Input report minimum sizes are consistent (all require at least report-ID + data).
#[test]
fn input_min_bytes_consistency() {
    assert!(RADIO_PANEL_INPUT_MIN_BYTES >= 2);
    assert!(MULTI_PANEL_INPUT_MIN_BYTES >= 2);
    assert!(SWITCH_PANEL_INPUT_MIN_BYTES >= 2);
}
