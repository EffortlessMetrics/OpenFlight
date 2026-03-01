// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for WinWing device protocols.
//!
//! Covers HOTAS report formats, panel displays, panel knobs/encoders,
//! LED/backlight control, and profile validation for all WinWing products
//! including Orion HOTAS, FCU, EFIS, and MCP panels.

use flight_hotas_winwing::input::{
    self, RUDDER_REPORT_LEN, STICK_REPORT_LEN, THROTTLE_REPORT_LEN,
};
use flight_hotas_winwing::orion2_stick;
use flight_hotas_winwing::orion2_throttle;
use flight_hotas_winwing::profiles::{self, DeviceProfile};
use flight_hotas_winwing::protocol::{
    self, BacklightSubCommand, CommandCategory, DetentName, DisplaySubCommand, FeatureReportFrame,
    FEATURE_REPORT_ID, MAX_FRAME_LEN, MAX_PAYLOAD_LEN, MIN_FRAME_LEN, ProtocolError,
};
use flight_hotas_winwing::WINWING_VID;

// ═══════════════════════════════════════════════════════════════════════════════
// § 1 — HOTAS report format (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

mod hotas_report_format {
    use super::*;

    // ── Stick axis format ────────────────────────────────────────────────

    #[test]
    fn stick_axis_format_bipolar_signed_i16() {
        // Verify roll/pitch use signed i16 LE → [-1.0, 1.0] normalisation.
        let mut report = [0u8; STICK_REPORT_LEN];
        report[0] = 0x02; // stick report ID
        // roll = -16384 (half left), pitch = +16384 (half forward)
        report[1..3].copy_from_slice(&(-16384i16).to_le_bytes());
        report[3..5].copy_from_slice(&16384i16.to_le_bytes());

        let state = input::parse_stick_report(&report).unwrap();
        assert!(state.axes.roll < -0.45 && state.axes.roll > -0.55,
            "half-left roll should be ~-0.5, got {}", state.axes.roll);
        assert!(state.axes.pitch > 0.45 && state.axes.pitch < 0.55,
            "half-forward pitch should be ~0.5, got {}", state.axes.pitch);
    }

    // ── Throttle axis format ─────────────────────────────────────────────

    #[test]
    fn throttle_axis_format_unipolar_u16() {
        // Verify throttle levers use unsigned u16 LE → [0.0, 1.0].
        let mut report = [0u8; THROTTLE_REPORT_LEN];
        report[0] = 0x01;
        // Left = 32768 (midpoint), Right = 49152 (75%)
        report[1..3].copy_from_slice(&32768u16.to_le_bytes());
        report[3..5].copy_from_slice(&49152u16.to_le_bytes());

        let state = input::parse_throttle_report(&report).unwrap();
        assert!((state.axes.throttle_left - 0.5).abs() < 0.01,
            "midpoint throttle should be ~0.5, got {}", state.axes.throttle_left);
        assert!((state.axes.throttle_right - 0.75).abs() < 0.01,
            "75% throttle should be ~0.75, got {}", state.axes.throttle_right);
    }

    // ── Button byte mapping ──────────────────────────────────────────────

    #[test]
    fn throttle_button_byte_mapping_50_buttons() {
        // Verify buttons 1-50 are mapped across bytes 11-18 of the throttle report.
        let mut report = [0u8; THROTTLE_REPORT_LEN];
        report[0] = 0x01;
        // Set button 50 → bit 49 in the u64 at offset 11
        let mask: u64 = 1u64 << 49;
        report[11..19].copy_from_slice(&mask.to_le_bytes());

        let state = input::parse_throttle_report(&report).unwrap();
        assert!(state.buttons.is_pressed(50), "button 50 should be pressed");
        assert!(!state.buttons.is_pressed(49), "button 49 should not be pressed");
    }

    // ── Encoder data ─────────────────────────────────────────────────────

    #[test]
    fn throttle_encoder_data_signed_deltas() {
        // Orion 2 throttle: 5 encoder bytes at offsets 19–23, interpreted as i8 deltas.
        let mut report = [0u8; orion2_throttle::MIN_REPORT_BYTES];
        report[0] = 0x01;
        report[19] = 3u8;           // encoder 0: +3 CW
        report[20] = (-2i8) as u8;  // encoder 1: -2 CCW
        report[21] = 0;             // encoder 2: no movement
        report[22] = 127u8;         // encoder 3: max CW
        report[23] = (-128i8) as u8; // encoder 4: max CCW

        let state = orion2_throttle::parse_orion2_throttle_report(&report).unwrap();
        assert_eq!(state.buttons.encoders[0], 3);
        assert_eq!(state.buttons.encoders[1], -2);
        assert_eq!(state.buttons.encoders[2], 0);
        assert_eq!(state.buttons.encoders[3], 127);
        assert_eq!(state.buttons.encoders[4], -128);
    }

    // ── LED output format ────────────────────────────────────────────────

    #[test]
    fn led_output_uses_feature_report_frame() {
        // LED/backlight commands use the proprietary feature report protocol.
        let frame = protocol::build_backlight_single_command(0x01, 0, 128).unwrap();
        let bytes = frame.as_bytes();
        assert_eq!(bytes[0], FEATURE_REPORT_ID, "LED commands use feature report ID 0xF0");
        assert_eq!(bytes[1], CommandCategory::Backlight as u8);
        assert_eq!(bytes[2], BacklightSubCommand::SetSingle as u8);
    }

    // ── Device identification ────────────────────────────────────────────

    #[test]
    fn device_identification_via_report_id() {
        // Different devices use different report IDs for input reports.
        // Stick = 0x02, Throttle = 0x01, Rudder = 0x03
        let stick_report = {
            let mut r = [0u8; STICK_REPORT_LEN];
            r[0] = 0x02;
            r
        };
        let throttle_report = {
            let mut r = [0u8; THROTTLE_REPORT_LEN];
            r[0] = 0x01;
            r
        };
        let rudder_report = {
            let mut r = [0u8; RUDDER_REPORT_LEN];
            r[0] = 0x03;
            r
        };

        assert!(input::parse_stick_report(&stick_report).is_ok());
        assert!(input::parse_throttle_report(&throttle_report).is_ok());
        assert!(input::parse_rudder_report(&rudder_report).is_ok());

        // Cross-device report ID rejection
        assert!(input::parse_stick_report(&throttle_report).is_err());
        assert!(input::parse_throttle_report(&rudder_report).is_err());
    }

    // ── Orion 2 stick full report layout ─────────────────────────────────

    #[test]
    fn orion2_stick_full_report_layout_12_bytes() {
        // Verify the complete 12-byte layout: ID + roll(2) + pitch(2) + buttons(4) + hatA(1) + hatB(1) + reserved(1)
        let mut r = [0u8; orion2_stick::MIN_REPORT_BYTES];
        r[0] = 0x02;
        r[1..3].copy_from_slice(&i16::MAX.to_le_bytes());   // full right roll
        r[3..5].copy_from_slice(&i16::MIN.to_le_bytes());   // full aft pitch
        r[5..9].copy_from_slice(&0x000F_FFFFu32.to_le_bytes()); // all 20 buttons
        r[9] = 0x00;  // HAT A = North
        r[10] = 0x04; // HAT B = South

        let s = orion2_stick::parse_orion2_stick_report(&r).unwrap();
        assert!((s.axes.roll - 1.0).abs() < 1e-4);
        assert!(s.axes.pitch <= -1.0 + 1e-3);
        for btn in 1..=20 {
            assert!(s.buttons.is_pressed(btn), "button {btn} should be pressed");
        }
        assert_eq!(s.buttons.hat_a, 0x00);
        assert_eq!(s.buttons.hat_b, 0x04);
    }

    // ── WinWing VID constant ─────────────────────────────────────────────

    #[test]
    fn winwing_vid_is_0x4098() {
        assert_eq!(WINWING_VID, 0x4098);
        assert_eq!(input::WINWING_VENDOR_ID, 0x4098);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 2 — Panel displays (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

mod panel_displays {
    use super::*;

    // ── FCU LCD data ─────────────────────────────────────────────────────

    #[test]
    fn fcu_display_text_round_trip() {
        // Write "250" to the FCU speed display (panel_id=0x04, field=0).
        let frame = protocol::build_display_text_command(0x04, 0x00, "250").unwrap();
        let parsed = protocol::parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Display);
        assert_eq!(parsed.sub_command, DisplaySubCommand::WriteText as u8);
        assert_eq!(parsed.payload[0], 0x04); // panel_id for FCU
        assert_eq!(parsed.payload[1], 0x00); // field 0 = SPD
        assert_eq!(&parsed.payload[2..], b"250");
    }

    // ── EFIS LCD data ────────────────────────────────────────────────────

    #[test]
    fn efis_baro_display_segment_data() {
        // Write raw 7-segment data for barometric display "1013".
        let segments = [0x06, 0x3F, 0x06, 0x4F]; // stylised "1013" in 7-seg
        let frame = protocol::build_display_segment_command(0x06, 0x00, &segments).unwrap();
        let parsed = protocol::parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Display);
        assert_eq!(parsed.sub_command, DisplaySubCommand::WriteSegment as u8);
        assert_eq!(parsed.payload[0], 0x06); // EFIS panel_id
        assert_eq!(&parsed.payload[2..], &segments);
    }

    // ── MCP LCD data ─────────────────────────────────────────────────────

    #[test]
    fn mcp_altitude_display_text() {
        // MCP altitude display can show 5 digits like "37000".
        let frame = protocol::build_display_text_command(0x05, 0x02, "37000").unwrap();
        let parsed = protocol::parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(&parsed.payload[2..], b"37000");
        assert_eq!(parsed.payload.len(), 2 + 5); // panel_id + field + 5 chars
    }

    // ── Display refresh (clear all) ──────────────────────────────────────

    #[test]
    fn display_refresh_clear_all_fields() {
        // Clear all displays on a panel — used during refresh cycle.
        let frame = protocol::build_display_clear_command(0x04).unwrap();
        let parsed = protocol::parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Display);
        assert_eq!(parsed.sub_command, DisplaySubCommand::ClearAll as u8);
        assert_eq!(parsed.payload, &[0x04]);
    }

    // ── Backlight control (display brightness) ───────────────────────────

    #[test]
    fn display_backlight_brightness_levels() {
        // Verify brightness command at min, mid, and max levels.
        for (panel, brightness) in [(0x04, 0u8), (0x05, 128u8), (0x06, 255u8)] {
            let frame = protocol::build_display_brightness_command(panel, brightness).unwrap();
            let parsed = protocol::parse_feature_report(frame.as_bytes()).unwrap();
            assert_eq!(parsed.sub_command, DisplaySubCommand::SetBrightness as u8);
            assert_eq!(parsed.payload[0], panel);
            assert_eq!(parsed.payload[1], brightness);
        }
    }

    // ── Contrast / display text truncation ───────────────────────────────

    #[test]
    fn display_text_truncated_to_16_chars() {
        // WinWing protocol caps text at 16 characters.
        let long_text = "ABCDEFGHIJKLMNOPQRSTUVWXYZ012345";
        let frame = protocol::build_display_text_command(0x04, 0x00, long_text).unwrap();
        let parsed = protocol::parse_feature_report(frame.as_bytes()).unwrap();
        // payload = panel_id(1) + field(1) + text(16 max)
        assert_eq!(parsed.payload.len(), 2 + 16);
        assert_eq!(&parsed.payload[2..], b"ABCDEFGHIJKLMNOP");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 3 — Panel knobs (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

mod panel_knobs {
    use super::*;

    // ── Rotary encoder increments ────────────────────────────────────────

    #[test]
    fn rotary_encoder_cw_and_ccw_deltas() {
        // Throttle encoders report signed i8 deltas per tick.
        let mut report = [0u8; orion2_throttle::MIN_REPORT_BYTES];
        report[0] = 0x01;
        report[19] = 5u8;           // ENC1: +5 CW
        report[20] = (-4i8) as u8;  // ENC2: -4 CCW

        let state = orion2_throttle::parse_orion2_throttle_report(&report).unwrap();
        assert!(state.buttons.encoders[0] > 0, "CW should be positive");
        assert!(state.buttons.encoders[1] < 0, "CCW should be negative");
        assert_eq!(state.buttons.encoders[0], 5);
        assert_eq!(state.buttons.encoders[1], -4);
    }

    // ── Push-pull knob (encoder with push) ───────────────────────────────

    #[test]
    fn push_pull_knob_encoder_has_push_button() {
        // FCU and throttle encoders all have push buttons.
        let throttle = profiles::orion2_throttle_profile();
        assert!(throttle.encoders.iter().all(|e| e.has_push),
            "all throttle encoders should have push buttons");

        let fcu = profiles::fcu_panel_profile();
        assert!(fcu.encoders.iter().all(|e| e.has_push),
            "all FCU encoders should have push buttons");
    }

    // ── Detent positions ─────────────────────────────────────────────────

    #[test]
    fn detent_positions_idle_and_afterburner() {
        // Query detent positions and verify idle < afterburner.
        let idle_pos = 1000u16.to_le_bytes();
        let ab_pos = 62000u16.to_le_bytes();
        let payload = [
            0, 0, idle_pos[0], idle_pos[1], 0,  // left idle
            0, 1, ab_pos[0], ab_pos[1], 0,       // left afterburner
        ];

        let report = protocol::parse_detent_response(&payload).unwrap();
        assert_eq!(report.positions.len(), 2);
        assert_eq!(report.positions[0].name, DetentName::Idle);
        assert_eq!(report.positions[1].name, DetentName::Afterburner);
        assert!(report.positions[0].normalised < report.positions[1].normalised,
            "idle position should be below afterburner");
    }

    // ── Acceleration (encoder delta magnitude) ───────────────────────────

    #[test]
    fn encoder_acceleration_via_larger_deltas() {
        // Fast rotation produces larger delta magnitudes per report.
        let mut slow_report = [0u8; orion2_throttle::MIN_REPORT_BYTES];
        slow_report[0] = 0x01;
        slow_report[19] = 1u8; // slow: +1

        let mut fast_report = [0u8; orion2_throttle::MIN_REPORT_BYTES];
        fast_report[0] = 0x01;
        fast_report[19] = 10u8; // fast: +10

        let slow = orion2_throttle::parse_orion2_throttle_report(&slow_report).unwrap();
        let fast = orion2_throttle::parse_orion2_throttle_report(&fast_report).unwrap();
        assert!(fast.buttons.encoders[0].abs() > slow.buttons.encoders[0].abs(),
            "fast rotation should yield larger delta magnitude");
    }

    // ── Wrap-around (i8 min/max boundaries) ──────────────────────────────

    #[test]
    fn encoder_wraparound_i8_boundaries() {
        // Encoder deltas are i8: verify boundary values parse correctly.
        let mut report = [0u8; orion2_throttle::MIN_REPORT_BYTES];
        report[0] = 0x01;
        report[19] = 127u8;         // i8::MAX
        report[20] = 128u8;         // wraps to i8::MIN (-128)

        let state = orion2_throttle::parse_orion2_throttle_report(&report).unwrap();
        assert_eq!(state.buttons.encoders[0], i8::MAX);
        assert_eq!(state.buttons.encoders[1], i8::MIN);
    }

    // ── Detent set command round-trip ─────────────────────────────────────

    #[test]
    fn detent_set_command_position_round_trip() {
        // Build a detent-set command and verify it round-trips through parse.
        let frame = protocol::build_detent_set_command(1, 2, 45000).unwrap();
        let parsed = protocol::parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.category, CommandCategory::Detent);
        assert_eq!(parsed.payload[0], 1);  // right lever
        assert_eq!(parsed.payload[1], 2);  // custom detent #2
        let pos = u16::from_le_bytes([parsed.payload[2], parsed.payload[3]]);
        assert_eq!(pos, 45000);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 4 — LED control (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

mod led_control {
    use super::*;

    // ── Individual LED on/off ────────────────────────────────────────────

    #[test]
    fn individual_led_on_off() {
        // Turn on button 5 LED at full intensity, then off.
        let on_frame = protocol::build_backlight_single_command(0x01, 5, 255).unwrap();
        let off_frame = protocol::build_backlight_single_command(0x01, 5, 0).unwrap();

        let on_parsed = protocol::parse_feature_report(on_frame.as_bytes()).unwrap();
        let off_parsed = protocol::parse_feature_report(off_frame.as_bytes()).unwrap();

        assert_eq!(on_parsed.payload, &[0x01, 5, 255]);
        assert_eq!(off_parsed.payload, &[0x01, 5, 0]);
        assert_eq!(on_parsed.category, CommandCategory::Backlight);
        assert_eq!(off_parsed.category, CommandCategory::Backlight);
    }

    // ── LED brightness levels ────────────────────────────────────────────

    #[test]
    fn led_brightness_full_range() {
        // Verify brightness at 0, 64, 128, 192, 255.
        for intensity in [0u8, 64, 128, 192, 255] {
            let frame = protocol::build_backlight_single_command(0x01, 0, intensity).unwrap();
            let parsed = protocol::parse_feature_report(frame.as_bytes()).unwrap();
            assert_eq!(parsed.payload[2], intensity,
                "intensity {intensity} should round-trip exactly");
        }
    }

    // ── RGB colour ───────────────────────────────────────────────────────

    #[test]
    fn led_rgb_colour_channels() {
        // Verify per-button RGB control with distinct channels.
        let frame = protocol::build_backlight_single_rgb_command(
            0x01, 3, 255, 0, 128
        ).unwrap();
        let parsed = protocol::parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.sub_command, BacklightSubCommand::SetSingleRgb as u8);
        assert_eq!(parsed.payload[0], 0x01); // panel_id
        assert_eq!(parsed.payload[1], 3);    // button_index
        assert_eq!(parsed.payload[2], 255);  // R
        assert_eq!(parsed.payload[3], 0);    // G
        assert_eq!(parsed.payload[4], 128);  // B
    }

    // ── Pattern: blink/steady via all-LEDs command ───────────────────────

    #[test]
    fn led_pattern_all_buttons_steady() {
        // "Steady on" pattern: set all LEDs to the same intensity.
        let frame = protocol::build_backlight_all_command(0x02, 180).unwrap();
        let parsed = protocol::parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.sub_command, BacklightSubCommand::SetAll as u8);
        assert_eq!(parsed.payload, &[0x02, 180]);

        // "All off" pattern:
        let off_frame = protocol::build_backlight_all_command(0x02, 0).unwrap();
        let off_parsed = protocol::parse_feature_report(off_frame.as_bytes()).unwrap();
        assert_eq!(off_parsed.payload[1], 0);
    }

    // ── Annunciator (all-RGB) ────────────────────────────────────────────

    #[test]
    fn annunciator_all_rgb_command() {
        // Set all LEDs to amber (annunciator colour) for warning state.
        let frame = protocol::build_backlight_all_rgb_command(0x04, 255, 191, 0).unwrap();
        let parsed = protocol::parse_feature_report(frame.as_bytes()).unwrap();
        assert_eq!(parsed.sub_command, BacklightSubCommand::SetAllRgb as u8);
        assert_eq!(parsed.payload, &[0x04, 255, 191, 0]); // amber
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 5 — Profile (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

mod profile_tests {
    use super::*;

    // ── A320 default profile (FCU) ───────────────────────────────────────

    #[test]
    fn a320_fcu_default_profile_structure() {
        let fcu = profiles::fcu_panel_profile();
        assert_eq!(fcu.vid, WINWING_VID);
        assert!(fcu.name.contains("FCU") || fcu.name.contains("Airbus"));
        // FCU has SPD, HDG, ALT, VS/FPA encoders
        let enc_names: Vec<_> = fcu.encoders.iter().map(|e| e.name).collect();
        assert!(enc_names.contains(&"SPD"));
        assert!(enc_names.contains(&"HDG"));
        assert!(enc_names.contains(&"ALT"));
        assert!(enc_names.contains(&"VS/FPA"));
        // FCU has displays for these parameters
        let disp_names: Vec<_> = fcu.displays.iter().map(|d| d.name).collect();
        assert!(disp_names.contains(&"SPD"));
        assert!(disp_names.contains(&"ALT"));
        assert!(disp_names.contains(&"VS/FPA"));
        // FCU has annunciator LEDs
        assert!(fcu.displays.iter().any(|d| d.display_type == "led-annunciator"));
    }

    // ── Custom bindings (button groups match button count) ───────────────

    #[test]
    fn custom_bindings_button_groups_consistent() {
        // For every profile, button_groups.sum(count) == button_count.
        for profile in profiles::all_profiles() {
            let sum: usize = profile
                .button_groups
                .iter()
                .map(|g| g.count as usize)
                .sum();
            assert_eq!(
                sum,
                profile.button_count as usize,
                "{}: button groups sum ({sum}) != button_count ({})",
                profile.name,
                profile.button_count
            );
        }
    }

    // ── Display variable mapping (panels with displays) ──────────────────

    #[test]
    fn display_variable_mapping_panel_displays_have_width() {
        // Every display field must have a positive width.
        let panels_with_displays: Vec<DeviceProfile> = profiles::all_profiles()
            .into_iter()
            .filter(|p| !p.displays.is_empty())
            .collect();

        assert!(!panels_with_displays.is_empty(), "at least one panel should have displays");
        for profile in &panels_with_displays {
            for display in &profile.displays {
                assert!(display.width > 0,
                    "{}: display '{}' has zero width", profile.name, display.name);
                assert!(!display.name.is_empty(),
                    "{}: display has empty name", profile.name);
                assert!(!display.display_type.is_empty(),
                    "{}: display '{}' has empty type", profile.name, display.name);
            }
        }
    }

    // ── LED state mapping (panels with backlights) ───────────────────────

    #[test]
    fn led_state_mapping_backlit_panels() {
        // Panels with backlight_led_count > 0 should be identifiable by PID.
        let backlit: Vec<DeviceProfile> = profiles::all_profiles()
            .into_iter()
            .filter(|p| p.backlight_led_count > 0)
            .collect();

        // Ensure there is at least one backlit panel, without hardcoding a product set.
        assert!(
            !backlit.is_empty(),
            "expected at least one backlit panel, got none",
        );

        for profile in &backlit {
            assert!(profile.backlight_led_count > 0);
            assert!(profiles::profile_by_pid(profile.pid).is_some(),
                "backlit panel {} (PID 0x{:04X}) should be findable by PID",
                profile.name, profile.pid);
        }
    }

    // ── EFIS panel profile ───────────────────────────────────────────────

    #[test]
    fn efis_panel_profile_structure() {
        let efis = profiles::efis_panel_profile();
        assert_eq!(efis.vid, WINWING_VID);
        assert!(efis.name.contains("EFIS"));
        // EFIS has BARO encoder with push, ND_RANGE, ND_MODE
        assert_eq!(efis.encoders.len(), 3);
        let baro_enc = efis.encoders.iter().find(|e| e.name == "BARO").unwrap();
        assert!(baro_enc.has_push, "BARO encoder should have push (STD toggle)");
        // EFIS has BARO display and annunciators
        assert_eq!(efis.displays.len(), 2);
        let baro_disp = efis.displays.iter().find(|d| d.name == "BARO").unwrap();
        assert_eq!(baro_disp.display_type, "7seg");
        assert_eq!(baro_disp.width, 4); // e.g. "1013" or "29.92"
        // No axes on EFIS panel
        assert!(efis.axes.is_empty());
        // Button count should include ND mode/range/filter buttons
        assert!(efis.button_count >= 10);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § 6 — Protocol wire format depth tests (bonus coverage)
// ═══════════════════════════════════════════════════════════════════════════════

mod protocol_wire_format {
    use super::*;

    #[test]
    fn feature_report_checksum_is_xor_of_header_and_payload() {
        // Manually verify XOR checksum calculation.
        let payload = [0x01, 0x02, 0x03];
        let frame = FeatureReportFrame::new(CommandCategory::Display, 0x01, &payload).unwrap();
        let bytes = frame.as_bytes();

        // XOR of bytes[1..total-1] should equal bytes[total-1]
        let total = bytes.len();
        let mut expected_cksum: u8 = 0;
        for &b in &bytes[1..total - 1] {
            expected_cksum ^= b;
        }
        assert_eq!(bytes[total - 1], expected_cksum,
            "checksum mismatch: expected 0x{expected_cksum:02X}, got 0x{:02X}", bytes[total - 1]);
    }

    #[test]
    fn payload_length_field_is_u16_le() {
        let payload = [0xAA; 10];
        let frame = FeatureReportFrame::new(CommandCategory::Backlight, 0x02, &payload).unwrap();
        let bytes = frame.as_bytes();
        let stored_len = u16::from_le_bytes([bytes[3], bytes[4]]);
        assert_eq!(stored_len, 10, "payload length field should be 10");
    }

    #[test]
    fn frame_size_bounds() {
        // Empty payload → MIN_FRAME_LEN
        let min = FeatureReportFrame::new(CommandCategory::DeviceInfo, 0x01, &[]).unwrap();
        assert_eq!(min.len(), MIN_FRAME_LEN);

        // Max payload → MAX_FRAME_LEN
        let max = FeatureReportFrame::new(
            CommandCategory::DeviceInfo, 0x01, &[0u8; MAX_PAYLOAD_LEN]
        ).unwrap();
        assert_eq!(max.len(), MAX_FRAME_LEN);

        // Over max → error
        let err = FeatureReportFrame::new(
            CommandCategory::DeviceInfo, 0x01, &[0u8; MAX_PAYLOAD_LEN + 1]
        ).unwrap_err();
        assert!(matches!(err, ProtocolError::PayloadTooLarge { .. }));
    }

    #[test]
    fn all_command_categories_build_and_parse() {
        // Smoke test: every category + sub-command builds a valid frame.
        let tests: Vec<(CommandCategory, u8)> = vec![
            (CommandCategory::Display, DisplaySubCommand::WriteText as u8),
            (CommandCategory::Display, DisplaySubCommand::SetBrightness as u8),
            (CommandCategory::Backlight, BacklightSubCommand::SetSingle as u8),
            (CommandCategory::Backlight, BacklightSubCommand::SetAllRgb as u8),
            (CommandCategory::Detent, 0x01),
            (CommandCategory::DeviceInfo, 0x01),
        ];

        for (cat, sub) in tests {
            let frame = FeatureReportFrame::new(cat, sub, &[0x42]).unwrap();
            let parsed = protocol::parse_feature_report(frame.as_bytes()).unwrap();
            assert_eq!(parsed.category, cat);
            assert_eq!(parsed.sub_command, sub);
            assert_eq!(parsed.payload, &[0x42]);
        }
    }
}
