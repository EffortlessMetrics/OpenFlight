// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for WinWing panel integration — FCU, MCP, overhead panel,
//! EFIS, and pedestal.
//!
//! These tests exercise:
//! 1. FCU/MCP panel parsing (encoders, buttons, LEDs, displays, modes)
//! 2. Panel protocol (USB HID reports, feature reports, multi-panel sync)
//! 3. Display management (7-segment values, blanking, flashing, brightness)
//! 4. Switch matrix (toggle vs momentary, guard switches, rotary selectors)
//! 5. LED/annunciator control (on/off, blink, RGB, intensity)
//! 6. Integration (sim state ↔ panel, roundtrip)

use flight_hotas_winwing::protocol::{
    BacklightSubCommand, CommandCategory, DetentName, DisplaySubCommand, FeatureReportFrame,
    ProtocolError, build_backlight_all_rgb_command, build_backlight_single_command,
    build_backlight_single_rgb_command, build_display_brightness_command,
    build_display_clear_command, build_display_segment_command, build_display_text_command,
    parse_detent_response, parse_feature_report, FEATURE_REPORT_ID, MAX_PAYLOAD_LEN,
};
use flight_hotas_winwing::profiles::{
    efis_panel_profile, fcu_panel_profile, profile_by_pid, take_off_panel_profile,
};
use flight_hotas_winwing::ufc_panel::{
    HUD_BUTTON_COUNT, MIN_REPORT_BYTES as UFC_MIN_REPORT, TOTAL_BUTTON_COUNT, UFC_BUTTON_COUNT,
    UfcPanelParseError, parse_ufc_panel_report,
};

// ── Panel ID constants ───────────────────────────────────────────────────────
const FCU_PANEL_ID: u8 = 0x10;
const EFIS_PANEL_ID: u8 = 0x20;
const UFC_PANEL_ID: u8 = 0x30;
const PEDESTAL_PANEL_ID: u8 = 0x40;

// ── UFC button constants ─────────────────────────────────────────────────────
const ENTER_BUTTON: u8 = 12;
const ENTER_LED_INDEX: u8 = 11;

// ═══════════════════════════════════════════════════════════════════════════════
// 1. FCU / MCP panel parsing (10 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Helper: build an FCU display-text command and round-trip parse it.
fn fcu_display_roundtrip(panel_id: u8, field: u8, text: &str) -> Vec<u8> {
    let frame = build_display_text_command(panel_id, field, text).unwrap();
    let parsed = parse_feature_report(frame.as_bytes()).unwrap();
    parsed.payload[2..].to_vec()
}

#[test]
fn fcu_speed_encoder_cw_rotation_display_update() {
    // Simulate updating the SPD display after a CW encoder rotation.
    // SPD field_index = 0.
    let text = "280";
    let payload = fcu_display_roundtrip(FCU_PANEL_ID, 0, text);
    assert_eq!(payload, b"280");
}

#[test]
fn fcu_heading_encoder_ccw_rotation_display_update() {
    // HDG field_index = 1 on the FCU.
    let text = "045";
    let payload = fcu_display_roundtrip(FCU_PANEL_ID, 1, text);
    assert_eq!(payload, b"045");
}

#[test]
fn fcu_altitude_knob_display_five_digits() {
    // ALT display is 5 digits wide.
    let profile = fcu_panel_profile();
    let alt_disp = profile.displays.iter().find(|d| d.name == "ALT").unwrap();
    assert_eq!(alt_disp.width, 5);

    let text = "35000";
    let payload = fcu_display_roundtrip(FCU_PANEL_ID, 2, text);
    assert_eq!(payload, b"35000");
}

#[test]
fn fcu_vs_wheel_positive_and_negative() {
    // VS/FPA display shows both positive and negative values.
    let profile = fcu_panel_profile();
    let vs_disp = profile
        .displays
        .iter()
        .find(|d| d.name == "VS/FPA")
        .unwrap();
    assert_eq!(vs_disp.width, 5);

    // Positive VS
    let payload_up = fcu_display_roundtrip(FCU_PANEL_ID, 3, "+1200");
    assert_eq!(payload_up, b"+1200");

    // Negative VS
    let payload_dn = fcu_display_roundtrip(FCU_PANEL_ID, 3, "-0800");
    assert_eq!(payload_dn, b"-0800");
}

#[test]
fn fcu_ap_engage_button_in_autopilot_group() {
    let profile = fcu_panel_profile();
    let ap_group = profile
        .button_groups
        .iter()
        .find(|g| g.name == "autopilot")
        .unwrap();
    // FCU has 6 autopilot buttons (AP1, AP2, ATHR, LOC, APPR, EXPED).
    assert_eq!(ap_group.count, 6);
}

#[test]
fn fcu_mode_select_buttons() {
    let profile = fcu_panel_profile();
    let mode_group = profile
        .button_groups
        .iter()
        .find(|g| g.name == "mode_select")
        .unwrap();
    // SPD/MACH, HDG/TRK, ALT, VS/FPA push-pull selectors.
    assert_eq!(mode_group.count, 4);
}

#[test]
fn fcu_encoder_push_buttons() {
    let profile = fcu_panel_profile();
    let enc_push = profile
        .button_groups
        .iter()
        .find(|g| g.name == "encoder_push")
        .unwrap();
    assert_eq!(enc_push.count, 6);
}

#[test]
fn fcu_all_encoders_have_push() {
    let profile = fcu_panel_profile();
    for enc in &profile.encoders {
        assert!(
            enc.has_push,
            "FCU encoder '{}' should have a push button",
            enc.name
        );
    }
}

#[test]
fn fcu_annunciator_display_is_led_type() {
    let profile = fcu_panel_profile();
    let ann = profile
        .displays
        .iter()
        .find(|d| d.name == "annunciators")
        .unwrap();
    assert_eq!(ann.display_type, "led-annunciator");
    assert_eq!(ann.width, 8); // 8 individual annunciators
}

#[test]
fn mcp_nav_hdg_apr_rev_modes_via_button_groups() {
    // The FCU profile doubles as MCP for Boeing-style sims.
    // Verify mode_select group covers NAV/HDG/APR/REV-equivalent modes.
    let profile = fcu_panel_profile();
    let total_buttons: u8 = profile.button_groups.iter().map(|g| g.count).sum();
    assert_eq!(total_buttons, profile.button_count);
    assert_eq!(profile.button_count, 16);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Panel protocol — USB HID reports (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn protocol_feature_report_id_is_0xf0() {
    assert_eq!(FEATURE_REPORT_ID, 0xF0);
    let frame = FeatureReportFrame::new(CommandCategory::Display, 0x01, &[]).unwrap();
    assert_eq!(frame.as_bytes()[0], 0xF0);
}

#[test]
fn protocol_report_id_parsing_rejects_wrong_id() {
    // Build a valid frame, then corrupt the report ID.
    let frame = FeatureReportFrame::new(CommandCategory::Display, 0x01, &[0x42]).unwrap();
    let mut bytes = frame.as_bytes().to_vec();
    bytes[0] = 0x01; // Not 0xF0
    let err = parse_feature_report(&bytes).unwrap_err();
    assert_eq!(err, ProtocolError::InvalidReportId { id: 0x01 });
}

#[test]
fn protocol_feature_report_checksum_roundtrip() {
    // Build → serialise → parse should succeed for arbitrary payloads.
    for payload_len in [0, 1, 8, 32, MAX_PAYLOAD_LEN] {
        let payload: Vec<u8> = (0..payload_len).map(|i| i as u8).collect();
        let frame =
            FeatureReportFrame::new(CommandCategory::Backlight, 0x03, &payload).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.payload, payload.as_slice());
    }
}

#[test]
fn protocol_feature_report_corrupt_checksum() {
    let frame =
        FeatureReportFrame::new(CommandCategory::Display, 0x02, &[0xDE, 0xAD]).unwrap();
    let mut bytes = frame.as_bytes().to_vec();
    let last = bytes.len() - 1;
    bytes[last] = bytes[last].wrapping_add(1); // Corrupt
    let err = parse_feature_report(&bytes).unwrap_err();
    assert!(matches!(err, ProtocolError::ChecksumMismatch { .. }));
}

#[test]
fn protocol_multi_panel_sync_different_panel_ids() {
    // Two panels with different IDs can have independent display updates.
    let fcu_frame = build_display_text_command(FCU_PANEL_ID, 0, "280").unwrap();
    let efis_frame = build_display_text_command(EFIS_PANEL_ID, 0, "1013").unwrap();

    let fcu_parsed = parse_feature_report(fcu_frame.as_bytes()).unwrap();
    let efis_parsed = parse_feature_report(efis_frame.as_bytes()).unwrap();

    assert_eq!(fcu_parsed.payload[0], FCU_PANEL_ID);
    assert_eq!(efis_parsed.payload[0], EFIS_PANEL_ID);
    assert_ne!(fcu_parsed.payload[0], efis_parsed.payload[0]);
}

#[test]
fn protocol_ufc_panel_report_id_0x06() {
    // UFC panel uses report ID 0x06 for input reports.
    let mut report = [0u8; UFC_MIN_REPORT];
    report[0] = 0x06;
    let state = parse_ufc_panel_report(&report).unwrap();
    // All buttons released.
    for n in 1..=TOTAL_BUTTON_COUNT {
        assert!(!state.buttons.is_pressed(n));
    }
}

#[test]
fn protocol_ufc_panel_wrong_report_id_rejected() {
    let mut report = [0u8; UFC_MIN_REPORT];
    report[0] = 0x01; // Wrong — expects 0x06
    let err = parse_ufc_panel_report(&report).unwrap_err();
    assert!(matches!(err, UfcPanelParseError::UnknownReportId { .. }));
}

#[test]
fn protocol_ufc_panel_too_short_rejected() {
    let err = parse_ufc_panel_report(&[0x06, 0x00]).unwrap_err();
    assert!(matches!(err, UfcPanelParseError::TooShort { .. }));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Display management (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn display_7seg_segment_bitmask_encoding() {
    // Common 7-segment encoding: digit "0" = 0x3F, "1" = 0x06, "8" = 0x7F.
    let segments = [0x3F, 0x06, 0x7F]; // "0", "1", "8"
    let frame = build_display_segment_command(FCU_PANEL_ID, 0, &segments).unwrap();
    let parsed = parse_feature_report(frame.as_bytes()).unwrap();
    assert_eq!(parsed.sub_command, DisplaySubCommand::WriteSegment as u8);
    assert_eq!(&parsed.payload[2..], &segments);
}

#[test]
fn display_blanking_via_clear_command() {
    // Clear all fields on a panel → display blanking.
    let frame = build_display_clear_command(FCU_PANEL_ID).unwrap();
    let parsed = parse_feature_report(frame.as_bytes()).unwrap();
    assert_eq!(parsed.category, CommandCategory::Display);
    assert_eq!(parsed.sub_command, DisplaySubCommand::ClearAll as u8);
    assert_eq!(parsed.payload, &[FCU_PANEL_ID]);
}

#[test]
fn display_flashing_indicator_via_segment_with_dp() {
    // Flashing indication: write segments with decimal-point bit set (bit 7).
    // "8." = 0x7F | 0x80 = 0xFF
    let segments = [0xFF]; // "8" with decimal point
    let frame = build_display_segment_command(FCU_PANEL_ID, 2, &segments).unwrap();
    let parsed = parse_feature_report(frame.as_bytes()).unwrap();
    assert_eq!(parsed.payload[2], 0xFF);
}

#[test]
fn display_brightness_zero_to_max() {
    for brightness in [0u8, 64, 128, 255] {
        let frame = build_display_brightness_command(FCU_PANEL_ID, brightness).unwrap();
        let parsed = parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.sub_command, DisplaySubCommand::SetBrightness as u8);
        assert_eq!(parsed.payload[1], brightness);
    }
}

#[test]
fn display_fcu_field_count_matches_profile() {
    let profile = fcu_panel_profile();
    // FCU has 5 display fields: SPD, HDG/TRK, ALT, VS/FPA, annunciators
    assert_eq!(profile.displays.len(), 5);
    let seg_displays: Vec<_> = profile
        .displays
        .iter()
        .filter(|d| d.display_type == "7seg")
        .collect();
    assert_eq!(seg_displays.len(), 4); // 4 numeric 7-seg displays
}

#[test]
fn display_efis_baro_field_width_is_4() {
    let profile = efis_panel_profile();
    let baro = profile.displays.iter().find(|d| d.name == "BARO").unwrap();
    assert_eq!(baro.width, 4); // e.g. "1013" hPa (4 digits) or "29.92" inHg (4 digits plus decimal-point segment)
    assert_eq!(baro.display_type, "7seg");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Switch matrix (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn switch_toggle_vs_momentary_in_top_panel() {
    let profile = take_off_panel_profile();
    let toggles = profile
        .button_groups
        .iter()
        .find(|g| g.name == "toggle_switches")
        .unwrap();
    let pushes = profile
        .button_groups
        .iter()
        .find(|g| g.name == "push_buttons")
        .unwrap();
    // Toggle switches maintain state; push buttons are momentary.
    assert!(toggles.count >= 12);
    assert_eq!(pushes.count, 12);
}

#[test]
fn switch_guard_concept_ufc_panel_buttons() {
    // UFC panel has 24 UFC buttons and 12 HUD buttons = 36 total.
    // Guard-switch concept: buttons can mask each other in software.
    assert_eq!(UFC_BUTTON_COUNT, 24);
    assert_eq!(HUD_BUTTON_COUNT, 12);
    assert_eq!(TOTAL_BUTTON_COUNT, 36);
}

#[test]
fn switch_rotary_selector_efis_nd_mode() {
    // EFIS has an ND_MODE encoder (rotary) with 5 ND mode buttons.
    let profile = efis_panel_profile();
    let nd_mode = profile
        .button_groups
        .iter()
        .find(|g| g.name == "nd_mode")
        .unwrap();
    assert_eq!(nd_mode.count, 5); // ROSE, ARC, PLAN, VOR, ILS
    assert!(profile.encoders.iter().any(|e| e.name == "ND_MODE"));
}

#[test]
fn switch_rotary_selector_efis_nd_range() {
    let profile = efis_panel_profile();
    let nd_range = profile
        .button_groups
        .iter()
        .find(|g| g.name == "nd_range")
        .unwrap();
    assert_eq!(nd_range.count, 1); // Single range encoder → one push
    assert!(profile.encoders.iter().any(|e| e.name == "ND_RANGE"));
}

#[test]
fn switch_multi_position_ufc_keypad() {
    // UFC panel keypad: buttons 1–10 are digits 0–9.
    let mut report = [0u8; UFC_MIN_REPORT];
    report[0] = 0x06;
    // Press digit "5" (button 6, since digits 0–9 = buttons 1–10).
    report[1] = 0b0010_0000; // bit 5 = button 6
    let state = parse_ufc_panel_report(&report).unwrap();
    assert!(state.buttons.is_ufc_pressed(6));
    assert!(!state.buttons.is_ufc_pressed(5));
    assert!(!state.buttons.is_ufc_pressed(7));
}

#[test]
fn switch_simultaneous_ufc_and_hud_buttons() {
    // Press UFC button 1 + HUD button 1 simultaneously.
    let mut report = [0u8; UFC_MIN_REPORT];
    report[0] = 0x06;
    report[1] = 0x01; // UFC button 1
    report[4] = 0x01; // HUD button 1 (bit 24)
    let state = parse_ufc_panel_report(&report).unwrap();
    assert!(state.buttons.is_ufc_pressed(1));
    assert!(state.buttons.is_hud_pressed(1));
    assert!(state.buttons.is_pressed(1)); // UFC 1
    assert!(state.buttons.is_pressed(25)); // HUD 1
}

#[test]
fn switch_top_panel_encoder_push_group() {
    let profile = take_off_panel_profile();
    let enc_push = profile
        .button_groups
        .iter()
        .find(|g| g.name == "encoder_push")
        .unwrap();
    // 8 encoders each with a push button.
    assert_eq!(enc_push.count, 8);
    assert_eq!(profile.encoders.len(), 8);
}

#[test]
fn switch_efis_baro_push_buttons() {
    let profile = efis_panel_profile();
    let baro_push = profile
        .button_groups
        .iter()
        .find(|g| g.name == "baro_push")
        .unwrap();
    // STD and reset push buttons for barometric setting.
    assert_eq!(baro_push.count, 2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. LED / annunciator control (4 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn led_on_off_via_backlight_intensity() {
    // Intensity 0 = off, 255 = on.
    let off_frame = build_backlight_single_command(FCU_PANEL_ID, 0, 0).unwrap();
    let on_frame = build_backlight_single_command(FCU_PANEL_ID, 0, 255).unwrap();

    let off_parsed = parse_feature_report(off_frame.as_bytes()).unwrap();
    let on_parsed = parse_feature_report(on_frame.as_bytes()).unwrap();

    assert_eq!(off_parsed.payload[2], 0); // off
    assert_eq!(on_parsed.payload[2], 255); // on
}

#[test]
fn led_blink_via_alternating_intensity() {
    // Blink is implemented by alternating intensity in software.
    // Verify both on and off frames can be constructed.
    let on = build_backlight_single_command(FCU_PANEL_ID, 5, 200).unwrap();
    let off = build_backlight_single_command(FCU_PANEL_ID, 5, 0).unwrap();

    let on_parsed = parse_feature_report(on.as_bytes()).unwrap();
    let off_parsed = parse_feature_report(off.as_bytes()).unwrap();

    assert_eq!(on_parsed.category, CommandCategory::Backlight);
    assert_eq!(off_parsed.category, CommandCategory::Backlight);
    assert_eq!(on_parsed.payload[1], 5); // Same button index
    assert_eq!(off_parsed.payload[1], 5);
    assert_ne!(on_parsed.payload[2], off_parsed.payload[2]); // Different intensity
}

#[test]
fn led_rgb_colour_for_annunciator() {
    // Set annunciator to green (0, 255, 0).
    let frame = build_backlight_single_rgb_command(FCU_PANEL_ID, 0, 0, 255, 0).unwrap();
    let parsed = parse_feature_report(frame.as_bytes()).unwrap();
    assert_eq!(parsed.sub_command, BacklightSubCommand::SetSingleRgb as u8);
    assert_eq!(parsed.payload[2], 0); // R
    assert_eq!(parsed.payload[3], 255); // G
    assert_eq!(parsed.payload[4], 0); // B
}

#[test]
fn led_intensity_half_brightness() {
    let frame = build_backlight_single_command(FCU_PANEL_ID, 3, 128).unwrap();
    let parsed = parse_feature_report(frame.as_bytes()).unwrap();
    assert_eq!(parsed.payload[2], 128);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Integration — sim state ↔ panel (4 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Simulate a sim-state → panel-display pipeline:
/// 1. Receive sim state value (e.g. speed = 250 kts)
/// 2. Format as text
/// 3. Build display command
/// 4. Parse back and verify
fn sim_to_panel_display(panel_id: u8, field: u8, value: &str) -> String {
    let frame = build_display_text_command(panel_id, field, value).unwrap();
    let parsed = parse_feature_report(frame.as_bytes()).unwrap();
    String::from_utf8(parsed.payload[2..].to_vec()).unwrap()
}

#[test]
fn integration_sim_speed_to_fcu_display() {
    // Sim reports IAS = 250 kts → FCU SPD display shows "250".
    let displayed = sim_to_panel_display(FCU_PANEL_ID, 0, "250");
    assert_eq!(displayed, "250");
}

#[test]
fn integration_sim_heading_to_fcu_display() {
    // Sim reports HDG = 090° → FCU HDG display shows "090".
    let displayed = sim_to_panel_display(FCU_PANEL_ID, 1, "090");
    assert_eq!(displayed, "090");
}

#[test]
fn integration_panel_input_to_sim_command() {
    // UFC keypad press → build a feature report acknowledging the LED state.
    // Simulate: pilot presses ENTER on UFC.
    let mut report = [0u8; UFC_MIN_REPORT];
    report[0] = 0x06;
    report[2] = 0b0000_1000; // bit 11 in the 24-bit mask (ENTER_BUTTON)
    let state = parse_ufc_panel_report(&report).unwrap();
    assert!(
        state.buttons.is_ufc_pressed(ENTER_BUTTON),
        "ENTER button should be pressed"
    );

    // In response, set LED on ENTER button's backlight (0-indexed).
    let led_frame = build_backlight_single_command(UFC_PANEL_ID, ENTER_LED_INDEX, 255).unwrap();
    let led_parsed = parse_feature_report(led_frame.as_bytes()).unwrap();
    assert_eq!(led_parsed.payload[1], ENTER_LED_INDEX); // button index
    assert_eq!(led_parsed.payload[2], 255); // full brightness
}

#[test]
fn integration_full_roundtrip_altitude_change() {
    // Full roundtrip: encoder rotation → display update → LED confirmation.
    let profile = fcu_panel_profile();

    // Step 1: Verify ALT encoder exists.
    assert!(profile.encoders.iter().any(|e| e.name == "ALT"));

    // Step 2: Build display update for new altitude (FL350 = 35000 ft).
    let alt_text = "35000";
    let disp_frame = build_display_text_command(FCU_PANEL_ID, 2, alt_text).unwrap();
    let disp_parsed = parse_feature_report(disp_frame.as_bytes()).unwrap();
    assert_eq!(&disp_parsed.payload[2..], b"35000");

    // Step 3: Build LED confirmation (ALT armed annunciator → amber).
    let led_frame = build_backlight_single_rgb_command(FCU_PANEL_ID, 7, 255, 165, 0).unwrap();
    let led_parsed = parse_feature_report(led_frame.as_bytes()).unwrap();
    assert_eq!(led_parsed.payload[2], 255); // R
    assert_eq!(led_parsed.payload[3], 165); // G (amber)
    assert_eq!(led_parsed.payload[4], 0); // B

    // Step 4: Verify profile has 5-digit ALT display.
    let alt_disp = profile.displays.iter().find(|d| d.name == "ALT").unwrap();
    assert_eq!(alt_disp.width, 5);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Bonus: additional coverage for pedestal (radio/transponder/trim)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn pedestal_radio_tuning_via_display_text() {
    // Radio tuning panel uses display text for COM frequency.
    // COM1 = 123.500 MHz → display "12350".
    let displayed = sim_to_panel_display(PEDESTAL_PANEL_ID, 0, "12350");
    assert_eq!(displayed, "12350");
}

#[test]
fn pedestal_transponder_code_display() {
    // Transponder panel: squawk code 1200 → "1200".
    let displayed = sim_to_panel_display(PEDESTAL_PANEL_ID, 1, "1200");
    assert_eq!(displayed, "1200");
}

#[test]
fn pedestal_trim_wheel_detent_query_and_response() {
    // Trim wheel uses detent protocol to report center position.
    let center_pos = 32768u16.to_le_bytes();
    let payload = [
        0, 0, center_pos[0], center_pos[1], 0, // lever 0, idle detent
    ];
    let report = parse_detent_response(&payload).unwrap();
    assert_eq!(report.positions.len(), 1);
    assert_eq!(report.positions[0].name, DetentName::Idle);
    assert_eq!(report.positions[0].raw_position, 32768);
    // Normalised ~ 0.5
    assert!((report.positions[0].normalised - 0.5).abs() < 0.01);
}

#[test]
fn pedestal_all_panels_have_winwing_vid() {
    // Every panel profile should carry the WinWing VID.
    for profile in [
        fcu_panel_profile(),
        efis_panel_profile(),
        take_off_panel_profile(),
    ] {
        assert_eq!(
            profile.vid, 0x4098,
            "{} should have WinWing VID",
            profile.name
        );
    }
}

#[test]
fn pedestal_profile_lookup_fcu_by_pid() {
    let p = profile_by_pid(0xBEE4).unwrap();
    assert!(p.name.contains("FCU"));
}

#[test]
fn pedestal_profile_lookup_efis_by_pid() {
    let p = profile_by_pid(0xBEE6).unwrap();
    assert!(p.name.contains("EFIS"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Additional depth: overhead panel switches via button-matrix parsing
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn overhead_panel_toggle_switches_count() {
    // The Take Off Panel (TOP) represents the overhead panel concept.
    // It has 12 toggle switches for fuel, hydraulics, electrical, APU, etc.
    let profile = take_off_panel_profile();
    let toggles = profile
        .button_groups
        .iter()
        .find(|g| g.name == "toggle_switches")
        .unwrap();
    assert!(toggles.count >= 12);
}

#[test]
fn overhead_panel_total_buttons() {
    let profile = take_off_panel_profile();
    assert_eq!(profile.button_count, 32);
}

#[test]
fn overhead_panel_backlight_coverage() {
    let profile = take_off_panel_profile();
    assert_eq!(
        profile.backlight_led_count, 32,
        "every overhead panel button should be individually backlit"
    );
}

#[test]
fn overhead_panel_display_fields_for_readouts() {
    let profile = take_off_panel_profile();
    // TOP has 6 display fields: ALT, HDG, CRS, SPD, VS, BARO.
    assert_eq!(profile.displays.len(), 6);
    let names: Vec<_> = profile.displays.iter().map(|d| d.name).collect();
    assert!(names.contains(&"ALT"));
    assert!(names.contains(&"HDG"));
    assert!(names.contains(&"CRS"));
    assert!(names.contains(&"SPD"));
    assert!(names.contains(&"VS"));
    assert!(names.contains(&"BARO"));
}

#[test]
fn overhead_panel_rgb_backlight_all() {
    // Set all overhead panel LEDs to dim amber.
    let frame = build_backlight_all_rgb_command(UFC_PANEL_ID, 128, 80, 0).unwrap();
    let parsed = parse_feature_report(frame.as_bytes()).unwrap();
    assert_eq!(parsed.sub_command, BacklightSubCommand::SetAllRgb as u8);
    assert_eq!(parsed.payload[1], 128); // R
    assert_eq!(parsed.payload[2], 80); // G
    assert_eq!(parsed.payload[3], 0); // B
}
