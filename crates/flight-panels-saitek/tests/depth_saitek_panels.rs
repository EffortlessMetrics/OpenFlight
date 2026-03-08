// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for Logitech/Saitek panel integration.
//!
//! Covers Radio Panel, Multi Panel, Switch Panel, and BIP across five
//! categories: display management, encoder handling, switch state,
//! profile binding, and USB HID protocol.

use std::time::{Duration, Instant};

use flight_panels_saitek::bip::{BipLedColor, BipState, BIP_LEDS_PER_STRIP};
use flight_panels_saitek::multi_panel::{
    LcdDisplay, ModeStateMachine, MultiPanelButtonState, MultiPanelLedMask, MultiPanelMode,
    MultiPanelProtocol, MultiPanelState, encode_segment, led_bits, parse_multi_panel_input,
    MULTI_PANEL_INPUT_MIN_BYTES, MULTI_PANEL_OUTPUT_BYTES,
};
use flight_panels_saitek::radio_panel::{
    EncoderDelta, RadioDisplay, RadioPanelButtonState, RadioPanelProtocol,
    parse_radio_panel_input, RADIO_PANEL_INPUT_MIN_BYTES, RADIO_PANEL_OUTPUT_BYTES,
};
use flight_panels_saitek::switch_panel::{
    GearLedColor, MagnetoPosition, SwitchDebounce, SwitchPanelGearLeds, SwitchPanelProtocol,
    SwitchPanelSwitchState, gear_led_bits, parse_switch_panel_input,
    SWITCH_PANEL_INPUT_MIN_BYTES, SWITCH_PANEL_OUTPUT_BYTES,
};

use flight_panels_core::display;
use flight_panels_core::protocol::{PanelEvent, PanelProtocol};

// ═══════════════════════════════════════════════════════════════════════════════
//  1. DISPLAY MANAGEMENT (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// 7-segment encoding covers all digits and special chars with correct bit-patterns.
#[test]
fn depth_seven_segment_encoding_completeness() {
    // Digits 0–9 each produce a unique non-zero pattern
    let mut seen = std::collections::HashSet::new();
    for d in '0'..='9' {
        let enc = encode_segment(d);
        assert_ne!(enc, 0x00, "digit '{d}' must not encode to blank");
        assert!(seen.insert(enc), "digit '{d}' must have a unique encoding");
    }
    // Dash uses only the middle segment (bit 6)
    assert_eq!(encode_segment('-'), 0x40);
    // Space and unknown chars are blank
    assert_eq!(encode_segment(' '), 0x00);
    assert_eq!(encode_segment('A'), 0x00);
}

/// Radio display refresh: modifying active/standby frequencies updates the HID
/// report correctly without cross-talk between rows.
#[test]
fn depth_radio_display_refresh_no_crosstalk() {
    let display = RadioDisplay {
        active: LcdDisplay::encode_str("11800"),
        ..Default::default()
    };
    let r1 = display.to_hid_report();
    // Only active row should be non-zero
    assert_ne!(&r1[1..6], &[0u8; 5], "active row must be populated");
    assert_eq!(&r1[6..11], &[0u8; 5], "standby row must remain blank");

    // Now set standby and confirm active is untouched
    let display2 = RadioDisplay {
        active: LcdDisplay::encode_str("11800"),
        standby: LcdDisplay::encode_str("12350"),
    };
    let r2 = display2.to_hid_report();
    assert_eq!(&r2[1..6], &r1[1..6], "active row must be unchanged");
    assert_ne!(&r2[6..11], &[0u8; 5], "standby row must now be populated");
}

/// Display blanking: blank display produces an all-zero HID report.
#[test]
fn depth_display_blanking_all_panels() {
    // Radio panel
    let radio = RadioDisplay::default().to_hid_report();
    assert!(radio.iter().all(|&b| b == 0), "blank radio report");

    // Multi panel
    let multi = MultiPanelState::default().to_hid_report();
    assert!(multi.iter().all(|&b| b == 0), "blank multi report");
}

/// Multi Panel LCD integer display: negative values, zero, and max all format correctly.
#[test]
fn depth_multi_panel_lcd_integer_edge_values() {
    // Zero: right-justified "    0"
    let z = LcdDisplay::from_integer(0);
    assert_eq!(z.raw(4), encode_segment('0'));
    for i in 0..4 {
        assert_eq!(z.raw(i), encode_segment(' '), "pos {i} should be blank");
    }

    // Max positive (99999)
    let max = LcdDisplay::from_integer(99999);
    for i in 0..5 {
        assert_eq!(max.raw(i), encode_segment('9'), "pos {i} should be '9'");
    }

    // Negative: "-  42"
    let neg = LcdDisplay::from_integer(-42);
    assert_eq!(neg.raw(0), encode_segment('-'));
    assert_eq!(neg.raw(3), encode_segment('4'));
    assert_eq!(neg.raw(4), encode_segment('2'));

    // Negative clamp: -10000 → "-9999"
    let neg_clamp = LcdDisplay::from_integer(-10000);
    assert_eq!(neg_clamp.raw(0), encode_segment('-'));
    assert_eq!(neg_clamp.raw(1), encode_segment('9'));
}

/// Multi Panel LED mask + display combined HID report preserves both fields.
#[test]
fn depth_multi_display_led_sync() {
    let lcd = LcdDisplay::encode_str("25000");
    let mask = MultiPanelLedMask::NONE
        .set(led_bits::ALT, true)
        .set(led_bits::VS, true)
        .set(led_bits::HDG, true);
    let report = lcd.to_hid_report(mask);

    // Display bytes correct
    assert_eq!(report[1], encode_segment('2'));
    assert_eq!(report[5], encode_segment('0'));
    // LED byte correct
    assert_eq!(
        report[11],
        led_bits::ALT | led_bits::VS | led_bits::HDG
    );
    // Reserved bytes zero
    for &b in &report[6..11] {
        assert_eq!(b, 0x00);
    }
}

/// BIP per-strip encoding produces independent LED reports.
#[test]
fn depth_bip_dual_strip_independence() {
    let mut bip = BipState::new();
    // Set strip 0 all green
    for pos in 0..BIP_LEDS_PER_STRIP {
        bip.set_led(0, pos, BipLedColor::Green);
    }
    // Strip 1 all red
    for pos in 0..BIP_LEDS_PER_STRIP {
        bip.set_led(1, pos, BipLedColor::Red);
    }

    let s0 = bip.encode_strip(0);
    let s1 = bip.encode_strip(1);
    assert!(s0.iter().all(|&b| b == BipLedColor::Green as u8));
    assert!(s1.iter().all(|&b| b == BipLedColor::Red as u8));
}

/// BIP brightness levels: all four colour states encode to distinct byte values.
#[test]
fn depth_bip_color_encoding_values() {
    assert_eq!(BipLedColor::Off as u8, 0);
    assert_eq!(BipLedColor::Green as u8, 1);
    assert_eq!(BipLedColor::Amber as u8, 2);
    assert_eq!(BipLedColor::Red as u8, 3);

    // Roundtrip through encode_strip
    let mut bip = BipState::new();
    bip.set_led(0, 0, BipLedColor::Off);
    bip.set_led(0, 1, BipLedColor::Green);
    bip.set_led(0, 2, BipLedColor::Amber);
    bip.set_led(0, 3, BipLedColor::Red);
    let encoded = bip.encode_strip(0);
    assert_eq!(encoded[0], 0);
    assert_eq!(encoded[1], 1);
    assert_eq!(encoded[2], 2);
    assert_eq!(encoded[3], 3);
}

/// Multi Panel flashing simulation: toggling LED mask produces alternating reports.
#[test]
fn depth_multi_panel_led_flash_toggle() {
    let lcd = LcdDisplay::encode_str("  AP ");
    let on_mask = MultiPanelLedMask(led_bits::ALT);
    let off_mask = MultiPanelLedMask::NONE;

    let report_on = lcd.to_hid_report(on_mask);
    let report_off = lcd.to_hid_report(off_mask);

    assert_ne!(report_on[11], report_off[11], "LED byte must differ");
    assert_eq!(
        &report_on[1..6], &report_off[1..6],
        "display bytes must be identical"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
//  2. ENCODER HANDLING (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Radio encoder: CW and CCW ticks accumulate independently for inner/outer.
#[test]
fn depth_radio_encoder_independent_axes() {
    let mut delta = EncoderDelta::default();

    // 5 outer CW, 3 inner CCW
    for _ in 0..5 {
        delta.update(&RadioPanelButtonState {
            mode: None,
            buttons: 0b0000_0010, // outer CW
        });
    }
    for _ in 0..3 {
        delta.update(&RadioPanelButtonState {
            mode: None,
            buttons: 0b0001_0000, // inner CCW
        });
    }

    assert_eq!(delta.outer, 5);
    assert_eq!(delta.inner, -3);
}

/// Encoder acceleration: rapid CW ticks accumulate linearly (no hardware accel).
#[test]
fn depth_encoder_accumulation_linear() {
    let mut delta = EncoderDelta::default();
    let cw = RadioPanelButtonState {
        mode: None,
        buttons: 0b0000_0010,
    };
    for i in 1..=100 {
        delta.update(&cw);
        assert_eq!(delta.outer, i, "tick {i} must accumulate linearly");
    }
}

/// Encoder wrap-around: bidirectional ticks cancel out to zero.
#[test]
fn depth_encoder_bidirectional_cancel() {
    let mut delta = EncoderDelta::default();
    let cw = RadioPanelButtonState {
        mode: None,
        buttons: 0b0000_0010,
    };
    let ccw = RadioPanelButtonState {
        mode: None,
        buttons: 0b0000_0100,
    };

    for _ in 0..50 {
        delta.update(&cw);
    }
    for _ in 0..50 {
        delta.update(&ccw);
    }
    assert_eq!(delta.outer, 0, "50 CW + 50 CCW must cancel");
}

/// Encoder drain returns accumulated value and resets atomically.
#[test]
fn depth_encoder_drain_atomic_reset() {
    let mut delta = EncoderDelta::default();
    delta.update(&RadioPanelButtonState {
        mode: None,
        buttons: 0b0000_1010, // outer CW + inner CW
    });
    delta.update(&RadioPanelButtonState {
        mode: None,
        buttons: 0b0000_1010,
    });

    let (outer, inner) = delta.drain();
    assert_eq!(outer, 2);
    assert_eq!(inner, 2);
    // Post-drain must be zero
    assert_eq!(delta.outer, 0);
    assert_eq!(delta.inner, 0);

    // Second drain returns zero
    let (o2, i2) = delta.drain();
    assert_eq!(o2, 0);
    assert_eq!(i2, 0);
}

/// Multi Panel encoder events parsed from HID report.
#[test]
fn depth_multi_encoder_parse_both_directions() {
    let proto = MultiPanelProtocol;

    // CW tick
    let cw_data = [0x00u8, 0b0100_0000, 0x00];
    let cw_events = proto.parse_input(&cw_data).unwrap();
    assert!(cw_events.iter().any(|e| matches!(
        e,
        PanelEvent::EncoderTick {
            name: "ENCODER",
            delta: 1
        }
    )));

    // CCW tick
    let ccw_data = [0x00u8, 0b0010_0000, 0x00];
    let ccw_events = proto.parse_input(&ccw_data).unwrap();
    assert!(ccw_events.iter().any(|e| matches!(
        e,
        PanelEvent::EncoderTick {
            name: "ENCODER",
            delta: -1
        }
    )));
}

/// Radio encoder: simultaneous CW on both encoders in one report.
#[test]
fn depth_radio_encoder_simultaneous_both() {
    let proto = RadioPanelProtocol;
    // Outer CW (bit 1) + Inner CW (bit 3) = 0b0000_1010
    let data = [0x00u8, 0x00, 0b0000_1010];
    let events = proto.parse_input(&data).unwrap();

    let outer_cw = events.iter().filter(|e| matches!(
        e,
        PanelEvent::EncoderTick {
            name: "OUTER",
            delta: 1
        }
    )).count();
    let inner_cw = events.iter().filter(|e| matches!(
        e,
        PanelEvent::EncoderTick {
            name: "INNER",
            delta: 1
        }
    )).count();

    assert_eq!(outer_cw, 1, "one outer CW event");
    assert_eq!(inner_cw, 1, "one inner CW event");
}

// ═══════════════════════════════════════════════════════════════════════════════
//  3. SWITCH STATE (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Switch panel: each individual toggle switch maps to the correct bit.
#[test]
fn depth_switch_individual_toggle_mapping() {
    type SwitchAccessor = fn(&SwitchPanelSwitchState) -> bool;
    let switches: &[(u8, SwitchAccessor, &str)] = &[
        (0b0000_0001, SwitchPanelSwitchState::master_battery, "MASTER_BAT"),
        (0b0000_0010, SwitchPanelSwitchState::master_alternator, "MASTER_ALT"),
        (0b0000_0100, SwitchPanelSwitchState::avionics_master, "AVIONICS"),
        (0b0000_1000, SwitchPanelSwitchState::fuel_pump, "FUEL_PUMP"),
        (0b0001_0000, SwitchPanelSwitchState::de_ice, "DE_ICE"),
        (0b0010_0000, SwitchPanelSwitchState::pitot_heat, "PITOT_HEAT"),
        (0b0100_0000, SwitchPanelSwitchState::cowl_flaps_closed, "COWL_FLAPS"),
        (0b1000_0000, SwitchPanelSwitchState::panel_light, "PANEL_LIGHT"),
    ];

    for &(byte1, accessor, name) in switches {
        let state = SwitchPanelSwitchState { byte1, byte2: 0 };
        assert!(
            accessor(&state),
            "{name} should be ON with byte1={byte1:#010b}"
        );
        // All other switches should be off
        let all_off = SwitchPanelSwitchState { byte1: !byte1, byte2: 0 };
        assert!(
            !accessor(&all_off),
            "{name} should be OFF when only others are set"
        );
    }
}

/// Multi Panel AP buttons: each button maps to a unique bit in byte 2.
#[test]
fn depth_multi_panel_ap_buttons_individual() {
    type BtnAccessor = fn(&MultiPanelButtonState) -> bool;
    let buttons: &[(u8, BtnAccessor, &str)] = &[
        (0b0000_0001, MultiPanelButtonState::btn_ap, "AP"),
        (0b0000_0010, MultiPanelButtonState::btn_hdg, "HDG"),
        (0b0000_0100, MultiPanelButtonState::btn_nav, "NAV"),
        (0b0000_1000, MultiPanelButtonState::btn_ias, "IAS"),
        (0b0001_0000, MultiPanelButtonState::btn_alt, "ALT"),
        (0b0010_0000, MultiPanelButtonState::btn_vs, "VS"),
        (0b0100_0000, MultiPanelButtonState::btn_apr, "APR"),
        (0b1000_0000, MultiPanelButtonState::btn_rev, "REV"),
    ];

    for &(byte2, accessor, name) in buttons {
        let state = MultiPanelButtonState { byte1: 0, byte2 };
        assert!(accessor(&state), "{name} should be pressed with byte2={byte2:#010b}");
    }
}

/// Gear LED feedback: individual gear colours set independently.
#[test]
fn depth_gear_led_individual_colors() {
    // Left green, Nose red, Right off
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

/// Switch panel magneto: all 5 positions decode correctly from HID bits.
#[test]
fn depth_magneto_all_positions_roundtrip() {
    let positions = [
        (0b0000_0000, MagnetoPosition::Off),
        (0b0000_0010, MagnetoPosition::Right),
        (0b0000_0100, MagnetoPosition::Left),
        (0b0000_0110, MagnetoPosition::Both),
        (0b0000_1000, MagnetoPosition::Start),
    ];

    for (byte2, expected) in positions {
        let state = SwitchPanelSwitchState { byte1: 0, byte2 };
        assert_eq!(
            state.magneto(),
            Some(expected),
            "byte2={byte2:#010b} should decode to {expected:?}"
        );
    }
}

/// Debounce: rapid transitions within the debounce window are rejected.
#[test]
fn depth_debounce_rapid_transitions() {
    let mut debounce = SwitchDebounce::new(Duration::from_millis(50));
    let t0 = Instant::now();

    // First change accepted
    assert!(debounce.accept(0, t0));
    // Rapid changes within 50ms rejected
    assert!(!debounce.accept(0, t0 + Duration::from_millis(10)));
    assert!(!debounce.accept(0, t0 + Duration::from_millis(30)));
    assert!(!debounce.accept(0, t0 + Duration::from_millis(49)));
    // After period, accepted again
    assert!(debounce.accept(0, t0 + Duration::from_millis(51)));
}

/// Switch diff: toggling multiple switches in one report generates events for each.
#[test]
fn depth_switch_diff_multiple_simultaneous_changes() {
    let mut proto = SwitchPanelProtocol::new(Duration::ZERO);
    let now = Instant::now();

    // All off
    let s0 = SwitchPanelSwitchState {
        byte1: 0x00,
        byte2: 0x00,
    };
    let _ = proto.diff_with_debounce(&s0, now);

    // Master Bat + Avionics + Gear all change at once
    let s1 = SwitchPanelSwitchState {
        byte1: 0b0000_0101, // MASTER_BAT + AVIONICS
        byte2: 0b0000_0001, // GEAR down
    };
    let events = proto.diff_with_debounce(&s1, now + Duration::from_millis(10));

    assert!(events.iter().any(|e| matches!(
        e,
        PanelEvent::SwitchChange { name: "MASTER_BAT", on: true }
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        PanelEvent::SwitchChange { name: "AVIONICS", on: true }
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        PanelEvent::SwitchChange { name: "GEAR", on: true }
    )));
    // MASTER_ALT was not changed
    assert!(!events.iter().any(|e| matches!(
        e,
        PanelEvent::SwitchChange { name: "MASTER_ALT", .. }
    )));
}

/// BIP annunciator states: LED colour can be overwritten in-place.
#[test]
fn depth_bip_annunciator_overwrite() {
    let mut bip = BipState::new();
    bip.set_led(0, 5, BipLedColor::Green);
    assert_eq!(bip.get_led(0, 5), Some(BipLedColor::Green));

    // Overwrite to Amber
    bip.set_led(0, 5, BipLedColor::Amber);
    assert_eq!(bip.get_led(0, 5), Some(BipLedColor::Amber));

    // Overwrite to Off
    bip.set_led(0, 5, BipLedColor::Off);
    assert_eq!(bip.get_led(0, 5), Some(BipLedColor::Off));
}

/// Gear transition sequence: UP → transit (red) → DOWN (green).
#[test]
fn depth_gear_led_transition_sequence() {
    // Gear up: all off
    let up = SwitchPanelGearLeds::ALL_OFF;
    assert_eq!(up.raw(), 0x00);

    // In transit: all red
    let transit = up.set_all(GearLedColor::Red);
    assert_eq!(transit, SwitchPanelGearLeds::ALL_RED);

    // Down and locked: all green
    let down = transit.set_all(GearLedColor::Green);
    assert_eq!(down, SwitchPanelGearLeds::ALL_GREEN);

    // Asymmetric transit: left green (locked), nose red (moving), right off (retracted)
    let asym = SwitchPanelGearLeds::ALL_OFF
        .set_left(GearLedColor::Green)
        .set_nose(GearLedColor::Red)
        .set_right(GearLedColor::Off);
    let report = asym.to_hid_report();
    assert_eq!(report[0], 0x00); // report ID
    assert_ne!(report[1], 0x00); // some LED bits set
}

// ═══════════════════════════════════════════════════════════════════════════════
//  4. PROFILE BINDING (4 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Variable → display: COM frequency renders correctly on Radio Panel display.
#[test]
fn depth_profile_com_freq_to_display() {
    let freq_str = display::format_com_freq(121_500);
    let lcd = LcdDisplay::encode_str(&freq_str);
    // "12150" → 1-2-1-5-0
    assert_eq!(lcd.raw(0), encode_segment('1'));
    assert_eq!(lcd.raw(1), encode_segment('2'));
    assert_eq!(lcd.raw(2), encode_segment('1'));
    assert_eq!(lcd.raw(3), encode_segment('5'));
    assert_eq!(lcd.raw(4), encode_segment('0'));

    // Edge: max COM freq 136.975 → "13697"
    let max_str = display::format_com_freq(136_975);
    let max_lcd = LcdDisplay::encode_str(&max_str);
    assert_eq!(max_lcd.raw(0), encode_segment('1'));
    assert_eq!(max_lcd.raw(1), encode_segment('3'));
}

/// Button → sim command: AP buttons produce named ButtonPress events.
#[test]
fn depth_profile_button_to_sim_command() {
    let proto = MultiPanelProtocol;
    let expected_names = ["AP", "HDG", "NAV", "IAS", "ALT", "VS", "APR", "REV"];

    for (i, &name) in expected_names.iter().enumerate() {
        let data = [0x00u8, 0x00, 1 << i];
        let events = proto.parse_input(&data).unwrap();
        assert!(
            events.iter().any(|e| matches!(e, PanelEvent::ButtonPress { name: n } if *n == name)),
            "bit {i} should produce ButtonPress for {name}"
        );
    }
}

/// LED → sim state: Multi Panel LED mask reflects active autopilot modes.
#[test]
fn depth_profile_led_reflects_ap_state() {
    // Simulated AP state: ALT hold + NAV active
    let mut mask = MultiPanelLedMask::NONE;
    mask = mask.set(led_bits::ALT, true);
    // NAV doesn't have a dedicated LED; the closest is IAS (bit 2)
    // But HDG does: bit 3
    mask = mask.set(led_bits::HDG, true);

    assert!(mask.is_set(led_bits::ALT));
    assert!(mask.is_set(led_bits::HDG));
    assert!(!mask.is_set(led_bits::VS));
    assert!(!mask.is_set(led_bits::CRS));

    // Render into report
    let state = MultiPanelState {
        display: LcdDisplay::from_integer(5000), // ALT hold at 5000ft
        leds: mask,
        ..Default::default()
    };
    let report = state.to_hid_report();
    assert_eq!(report[11], led_bits::ALT | led_bits::HDG);
}

/// Mode selector → display context: mode change determines what value is shown.
#[test]
fn depth_profile_mode_context_switch() {
    let mut sm = ModeStateMachine::new();

    // Simulate: ALT mode → display shows altitude
    let alt_input = MultiPanelButtonState {
        byte1: 0b0000_0001, // SEL_ALT
        byte2: 0,
    };
    let new_mode = sm.update(&alt_input);
    assert_eq!(new_mode, Some(MultiPanelMode::Alt));

    // Switch to VS mode → display should show vertical speed
    let vs_input = MultiPanelButtonState {
        byte1: 0b0000_0010, // SEL_VS
        byte2: 0,
    };
    let new_mode = sm.update(&vs_input);
    assert_eq!(new_mode, Some(MultiPanelMode::Vs));
    assert_eq!(sm.current(), Some(MultiPanelMode::Vs));

    // Verify display update for VS
    let vs_display = LcdDisplay::from_integer(-500); // -500 fpm
    assert_eq!(vs_display.raw(0), encode_segment('-'));
}

// ═══════════════════════════════════════════════════════════════════════════════
//  5. PROTOCOL — USB HID REPORT FORMAT (4 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Radio Panel HID output report: exactly 23 bytes, correct field positions.
#[test]
fn depth_protocol_radio_hid_output_format() {
    let display = RadioDisplay {
        active: LcdDisplay::encode_str("11800"),
        standby: LcdDisplay::encode_str("12350"),
    };
    let report = display.to_hid_report();

    assert_eq!(report.len(), RADIO_PANEL_OUTPUT_BYTES);
    assert_eq!(report.len(), 23);
    assert_eq!(report[0], 0x00, "byte 0 = report ID");
    // Active: bytes 1-5
    assert_eq!(report[1], encode_segment('1'));
    assert_eq!(report[5], encode_segment('0'));
    // Standby: bytes 6-10
    assert_eq!(report[6], encode_segment('1'));
    assert_eq!(report[10], encode_segment('0'));
    // Reserved: bytes 11-22 all zero
    for (i, &byte) in report[11..23].iter().enumerate() {
        assert_eq!(byte, 0x00, "reserved byte {}", i + 11);
    }
}

/// Multi Panel HID output report: 12 bytes with display + LED mask.
#[test]
fn depth_protocol_multi_hid_output_format() {
    let state = MultiPanelState {
        display: LcdDisplay::encode_str("35000"),
        leds: MultiPanelLedMask(led_bits::ALT | led_bits::AUTO_THROTTLE),
        ..Default::default()
    };
    let report = state.to_hid_report();

    assert_eq!(report.len(), MULTI_PANEL_OUTPUT_BYTES);
    assert_eq!(report.len(), 12);
    assert_eq!(report[0], 0x00, "byte 0 = report ID");
    // Display: bytes 1-5
    assert_eq!(report[1], encode_segment('3'));
    assert_eq!(report[5], encode_segment('0'));
    // Lower row (unused): bytes 6-10
    for (i, &byte) in report[6..11].iter().enumerate() {
        assert_eq!(byte, 0x00, "lower row byte {}", i + 6);
    }
    // LED mask: byte 11
    assert_eq!(report[11], led_bits::ALT | led_bits::AUTO_THROTTLE);
}

/// Switch Panel HID output report: 2 bytes for gear LEDs.
#[test]
fn depth_protocol_switch_hid_output_format() {
    let leds = SwitchPanelGearLeds::ALL_GREEN;
    let report = leds.to_hid_report();

    assert_eq!(report.len(), SWITCH_PANEL_OUTPUT_BYTES);
    assert_eq!(report.len(), 2);
    assert_eq!(report[0], 0x00, "byte 0 = report ID");
    assert_eq!(
        report[1],
        gear_led_bits::LEFT_GREEN | gear_led_bits::NOSE_GREEN | gear_led_bits::RIGHT_GREEN
    );
}

/// HID input report parsing: minimum-length validation for all panels.
#[test]
fn depth_protocol_input_min_length_validation() {
    // Radio: needs 3 bytes
    assert_eq!(RADIO_PANEL_INPUT_MIN_BYTES, 3);
    assert!(parse_radio_panel_input(&[0x00, 0x00]).is_none());
    assert!(parse_radio_panel_input(&[0x00, 0x00, 0x00]).is_some());

    // Multi: needs 3 bytes
    assert_eq!(MULTI_PANEL_INPUT_MIN_BYTES, 3);
    assert!(parse_multi_panel_input(&[0x00, 0x00]).is_none());
    assert!(parse_multi_panel_input(&[0x00, 0x00, 0x00]).is_some());

    // Switch: needs 3 bytes
    assert_eq!(SWITCH_PANEL_INPUT_MIN_BYTES, 3);
    assert!(parse_switch_panel_input(&[0x00, 0x00]).is_none());
    assert!(parse_switch_panel_input(&[0x00, 0x00, 0x00]).is_some());

    // Extra bytes beyond minimum are silently accepted
    assert!(parse_radio_panel_input(&[0x00; 64]).is_some());
    assert!(parse_multi_panel_input(&[0x00; 64]).is_some());
    assert!(parse_switch_panel_input(&[0x00; 64]).is_some());
}
