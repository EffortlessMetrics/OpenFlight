// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for Logitech/Saitek X52, X56, and Extreme 3D Pro HOTAS devices.
//!
//! These tests cover device-specific behaviour: axis ranges, button matrices,
//! MFD display protocol, LED/RGB control, mode switching, rotary encoders,
//! profile semantics, and VID/PID device identification.

use flight_hotas_logitech::protocol::{
    self, DeviceId, RgbColor, X52BlinkPattern, X52LedColor, X52LedId, X52Mode, X56RgbZone,
    identify_device, mfd_clear_all, mfd_encode_text, mfd_set_brightness, mfd_write_line,
    resolve_mode_button, x52_led_command, x56_rgb_set_all, x56_rgb_set_zone, LOGITECH_VID,
    MAD_CATZ_VID, MFD_LINE_COUNT, MFD_LINE_LENGTH, MFD_LINE_REPORT_SIZE, SAITEK_VID,
    X56_RGB_REPORT_SIZE,
};
use flight_hotas_logitech::profiles::{
    x52_profile, x56_profile, flight_yoke_profile, rudder_pedals_profile, AxisKind,
    DeviceProfile,
};
use flight_hotas_logitech::parse_extreme_3d_pro;

// ── Helper: Build Extreme 3D Pro reports ──────────────────────────────────────

fn build_e3dp_report(x: u16, y: u16, twist: u8, throttle: u8, buttons: u16, hat: u8) -> [u8; 7] {
    let x = x & 0x3FF;
    let y = y & 0x3FF;
    let throttle = throttle & 0x7F;
    let buttons = buttons & 0x0FFF;
    let hat = hat & 0x0F;
    let mut data = [0u8; 7];
    data[0] = x as u8;
    data[1] = ((x >> 8) as u8) | ((y as u8 & 0x3F) << 2);
    data[2] = ((y >> 6) as u8 & 0x0F) | ((twist & 0x0F) << 4);
    data[3] = (twist >> 4) | ((throttle & 0x0F) << 4);
    data[4] = (throttle >> 4) | (((buttons & 0x1F) as u8) << 3);
    data[5] = ((buttons >> 5) as u8 & 0x7F) | ((hat & 0x01) << 7);
    data[6] = (hat >> 1) & 0x07;
    data
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. X52 Pro depth tests (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// X52 profile axes span the documented range: 3 stick axes + throttle +
/// rotary + slider + mouse mini-stick = 9 total, all within valid resolution.
#[test]
fn x52_pro_axes_range_and_count() {
    let p = x52_profile();
    assert_eq!(p.axes.len(), 9, "X52 Pro should have 9 axes");
    for axis in &p.axes {
        let max = DeviceProfile::axis_max_raw(axis.resolution_bits);
        assert!(max > 0, "axis '{}' max_raw must be >0", axis.name);

        // Verify normalization at boundaries
        let at_zero = DeviceProfile::normalize_axis(0, axis);
        let at_max = DeviceProfile::normalize_axis(max, axis);
        match axis.kind {
            AxisKind::Bipolar => {
                assert!(at_zero < -0.99, "{} at 0 should be near -1.0", axis.name);
                assert!(at_max > 0.99, "{} at max should be near 1.0", axis.name);
            }
            AxisKind::Unipolar => {
                assert!(at_zero < 0.01, "{} at 0 should be near 0.0", axis.name);
                assert!(at_max > 0.99, "{} at max should be near 1.0", axis.name);
            }
        }
    }
}

/// X52 MFD display protocol produces correct reports for all 3 lines with
/// various text patterns.
#[test]
fn x52_pro_mfd_display_data() {
    // Line addressing
    for line in 0..MFD_LINE_COUNT {
        let buf = mfd_write_line(line, "TEST").unwrap();
        assert_eq!(buf[2], line, "line index byte");
        assert_eq!(buf.len(), MFD_LINE_REPORT_SIZE);
    }

    // Full-length text fills exactly 16 chars
    let full = mfd_write_line(0, "ABCDEFGHIJKLMNOP").unwrap();
    assert_eq!(&full[3..19], b"ABCDEFGHIJKLMNOP");

    // Empty text pads with spaces
    let empty = mfd_write_line(1, "").unwrap();
    assert!(empty[3..19].iter().all(|&b| b == b' '));

    // Non-ASCII replacement
    let encoded = mfd_encode_text("\u{00E9}\u{00F1}\u{00FC}test");
    assert_eq!(&encoded[..7], b"???test");

    // Brightness clamp
    let bright = mfd_set_brightness(255);
    assert_eq!(bright[2], 127, "brightness must clamp to 127");

    // Clear all produces 3 reports
    let cleared = mfd_clear_all();
    assert_eq!(cleared.len(), 3);
    for (i, report) in cleared.iter().enumerate() {
        assert_eq!(report[2], i as u8);
    }
}

/// X52 mode switch covers all 3 modes with correct index mapping.
#[test]
fn x52_pro_mode_switch_three_modes() {
    // Raw decode covers all input patterns
    assert_eq!(X52Mode::from_raw(0), X52Mode::Mode1);
    assert_eq!(X52Mode::from_raw(1), X52Mode::Mode1);
    assert_eq!(X52Mode::from_raw(2), X52Mode::Mode2);
    assert_eq!(X52Mode::from_raw(3), X52Mode::Mode3);

    // Index is contiguous 0-2
    assert_eq!(X52Mode::Mode1.index(), 0);
    assert_eq!(X52Mode::Mode2.index(), 1);
    assert_eq!(X52Mode::Mode3.index(), 2);

    // Upper bits masked off
    assert_eq!(X52Mode::from_raw(0b1111_1100), X52Mode::Mode1);
    assert_eq!(X52Mode::from_raw(0b1111_1110), X52Mode::Mode2);
    assert_eq!(X52Mode::from_raw(0b1111_1111), X52Mode::Mode3);

    // Profile carries 3 mode label sets
    let p = x52_profile();
    assert_eq!(p.mode_button_labels.len(), 3);
    for (i, labels) in p.mode_button_labels.iter().enumerate() {
        assert_eq!(
            labels.mode.index(),
            i,
            "mode labels out of order at {}",
            i
        );
        assert!(labels.labels.len() > 1, "mode {} should have labels", i);
    }
}

/// X52 button matrix: 34 buttons, mode-mapped via resolve_mode_button.
#[test]
fn x52_pro_button_matrix() {
    let p = x52_profile();
    assert_eq!(p.button_count, 34);

    // Build an identity button map
    let mut map = [[0u8; 32]; 3];
    for mode in 0..3 {
        for btn in 0..32 {
            map[mode][btn] = btn as u8;
        }
    }

    // All valid buttons resolve in all modes
    for mode in [X52Mode::Mode1, X52Mode::Mode2, X52Mode::Mode3] {
        for btn in 0u8..32 {
            assert!(
                resolve_mode_button(mode, btn, &map).is_some(),
                "button {} in {:?} should resolve",
                btn,
                mode
            );
        }
        // Out-of-range rejects
        assert!(resolve_mode_button(mode, 32, &map).is_none());
    }

    // Remapping works: in Mode2, button 5 -> logical 20
    map[1][5] = 20;
    assert_eq!(resolve_mode_button(X52Mode::Mode2, 5, &map), Some(20));
    assert_eq!(resolve_mode_button(X52Mode::Mode1, 5, &map), Some(5));
}

/// X52 POV hat supports all 8 cardinal directions plus center.
#[test]
fn x52_pro_pov_hat() {
    let p = x52_profile();
    assert_eq!(p.hat_count, 2, "X52 Pro has POV1 + POV2");

    // Mode labels include POV2 directions
    let m1 = &p.mode_button_labels[0];
    assert!(m1.labels.iter().any(|l| *l == "POV2 Up"));
    assert!(m1.labels.iter().any(|l| *l == "POV2 Right"));
    assert!(m1.labels.iter().any(|l| *l == "POV2 Down"));
    assert!(m1.labels.iter().any(|l| *l == "POV2 Left"));
}

/// X52 clutch button is labelled in mode button labels for all modes.
#[test]
fn x52_pro_clutch_button() {
    let p = x52_profile();
    for mode_labels in &p.mode_button_labels {
        assert!(
            mode_labels.labels.iter().any(|l| l.contains("Clutch")),
            "mode {:?} should label the clutch button",
            mode_labels.mode
        );
    }
}

/// X52 mouse mini-stick axes are bipolar with center detent.
#[test]
fn x52_pro_mouse_stick() {
    let p = x52_profile();
    let mouse_axes: Vec<_> = p
        .axes
        .iter()
        .filter(|a| a.name.contains("Mouse Mini-Stick"))
        .collect();
    assert_eq!(mouse_axes.len(), 2, "X52 has X+Y mouse mini-stick");
    for ax in &mouse_axes {
        assert_eq!(ax.kind, AxisKind::Bipolar);
        assert!(ax.center_detent, "{} should have center detent", ax.name);
        assert_eq!(ax.resolution_bits, 8);
    }
}

/// X52 LED color commands produce correct USB control transfer descriptors.
#[test]
fn x52_pro_led_color() {
    let leds = [
        X52LedId::Fire,
        X52LedId::ButtonA,
        X52LedId::ButtonB,
        X52LedId::ButtonD,
        X52LedId::ButtonE,
        X52LedId::Toggle1,
        X52LedId::Toggle2,
        X52LedId::Toggle3,
        X52LedId::Pov2,
        X52LedId::Clutch,
        X52LedId::Throttle,
    ];
    let colors = [
        X52LedColor::Off,
        X52LedColor::Green,
        X52LedColor::Amber,
        X52LedColor::Red,
    ];

    for (idx, led) in leds.iter().enumerate() {
        for color in &colors {
            let (req_type, req, wvalue, windex) = x52_led_command(*led, *color);
            assert_eq!(req_type, protocol::LED_REQUEST_TYPE);
            assert_eq!(req, protocol::LED_REQUEST);
            assert_eq!(wvalue, idx as u16, "LED index for {:?}", led);
            assert_eq!(
                windex,
                protocol::x52_led_color_code(*color) as u16,
                "color code for {:?}",
                color
            );
        }
    }

    // Default LED states are all green
    let p = x52_profile();
    for led_default in &p.led_defaults {
        assert_eq!(led_default.color, X52LedColor::Green);
    }

    // Blink pattern phase switching
    let blink = X52BlinkPattern::new(X52LedId::Fire, X52LedColor::Red, 500);
    let (_, _, _, on_color) = blink.command_for_phase(true);
    let (_, _, _, off_color) = blink.command_for_phase(false);
    assert_eq!(on_color, protocol::x52_led_color_code(X52LedColor::Red) as u16);
    assert_eq!(off_color, protocol::x52_led_color_code(X52LedColor::Off) as u16);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. X56 Rhino depth tests (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// X56 dual throttle axes are independent, unipolar, 16-bit.
#[test]
fn x56_rhino_dual_throttle_independent() {
    let p = x56_profile();
    let left = p.axes.iter().find(|a| a.name == "Left Throttle").unwrap();
    let right = p.axes.iter().find(|a| a.name == "Right Throttle").unwrap();

    // Both unipolar
    assert_eq!(left.kind, AxisKind::Unipolar);
    assert_eq!(right.kind, AxisKind::Unipolar);
    assert!(!left.center_detent);
    assert!(!right.center_detent);

    // Both 16-bit
    assert_eq!(left.resolution_bits, 16);
    assert_eq!(right.resolution_bits, 16);

    // Different HID usage codes ensure independence
    assert_ne!(
        left.hid_usage, right.hid_usage,
        "dual throttles must have distinct HID usages"
    );

    // Normalization at extremes
    let max = DeviceProfile::axis_max_raw(16);
    assert!(DeviceProfile::normalize_axis(0, left) < 0.01);
    assert!(DeviceProfile::normalize_axis(max, left) > 0.999);
    assert!(DeviceProfile::normalize_axis(0, right) < 0.01);
    assert!(DeviceProfile::normalize_axis(max, right) > 0.999);
}

/// X56 RGB LED control produces correct reports for all 4 zones.
#[test]
fn x56_rhino_rgb_led_control() {
    let zones = [
        X56RgbZone::StickBase,
        X56RgbZone::StickGrip,
        X56RgbZone::ThrottleBase,
        X56RgbZone::ThrottleGrip,
    ];
    let test_color = RgbColor::new(100, 200, 50);

    for (idx, zone) in zones.iter().enumerate() {
        let buf = x56_rgb_set_zone(*zone, test_color);
        assert_eq!(buf.len(), X56_RGB_REPORT_SIZE);
        assert_eq!(buf[0], protocol::X56_RGB_REPORT_ID);
        assert_eq!(buf[1], protocol::X56_RGB_CMD);
        assert_eq!(buf[2], idx as u8, "zone index for {:?}", zone);
        assert_eq!(buf[3], 100, "red");
        assert_eq!(buf[4], 200, "green");
        assert_eq!(buf[5], 50, "blue");
    }

    // Set-all covers all 4 zones
    let all = x56_rgb_set_all(RgbColor::WHITE);
    assert_eq!(all.len(), 4);
    for (i, report) in all.iter().enumerate() {
        assert_eq!(report[2], i as u8);
        assert_eq!(report[3], 255);
        assert_eq!(report[4], 255);
        assert_eq!(report[5], 255);
    }

    // OFF is truly black (all zeros for color bytes)
    let off = x56_rgb_set_zone(X56RgbZone::StickBase, RgbColor::OFF);
    assert_eq!(off[3..6], [0, 0, 0]);
}

/// X56 rotary encoders: 6 continuous 8-bit encoders.
#[test]
fn x56_rhino_rotary_encoders() {
    let p = x56_profile();
    assert_eq!(p.rotaries.len(), 6, "X56 has 6 rotary encoders");
    for rot in &p.rotaries {
        assert_eq!(rot.resolution_bits, 8, "{} should be 8-bit", rot.name);
        assert!(rot.continuous, "{} should be continuous", rot.name);
        assert!(!rot.name.is_empty());
    }

    // Verify throttle-mounted rotaries are present
    let throttle_rots: Vec<_> = p
        .rotaries
        .iter()
        .filter(|r| r.name.contains("Throttle"))
        .collect();
    assert!(
        throttle_rots.len() >= 2,
        "at least 2 rotaries on throttle unit"
    );
}

/// X56 button matrix: 32 buttons total.
#[test]
fn x56_rhino_button_matrix() {
    let p = x56_profile();
    assert_eq!(p.button_count, 32);
    // X56 has no mode selector, so no mode-remapped buttons
    assert!(!p.has_mode_selector);
    assert!(p.mode_button_labels.is_empty());
}

/// X56 hat switches: 4 hats total.
#[test]
fn x56_rhino_hat_switches() {
    let p = x56_profile();
    assert_eq!(p.hat_count, 4, "X56 has 4 hat switches");
}

/// X56 mode layer: no mode selector (software modes handled elsewhere).
#[test]
fn x56_rhino_mode_layer() {
    let p = x56_profile();
    assert!(!p.has_mode_selector);
    assert!(p.mode_button_labels.is_empty());
    // X56 has no MFD
    assert!(p.mfd_pages.is_empty());
    // X56 has no LED defaults (uses RGB instead)
    assert!(p.led_defaults.is_empty());
}

/// X56 mini-sticks: 2 mini-sticks, 4 axes total, bipolar with center detent.
#[test]
fn x56_rhino_ministick() {
    let p = x56_profile();
    let ministick_axes: Vec<_> = p
        .axes
        .iter()
        .filter(|a| a.name.contains("Mini-Stick"))
        .collect();
    assert_eq!(ministick_axes.len(), 4, "2 mini-sticks × 2 axes");

    for ax in &ministick_axes {
        assert_eq!(ax.kind, AxisKind::Bipolar, "{} bipolar", ax.name);
        assert!(ax.center_detent, "{} center detent", ax.name);
        assert_eq!(ax.resolution_bits, 8);
    }

    // Verify both sticks are present (Mini-Stick 1 and Mini-Stick 2)
    assert!(ministick_axes.iter().any(|a| a.name.contains("1 X")));
    assert!(ministick_axes.iter().any(|a| a.name.contains("1 Y")));
    assert!(ministick_axes.iter().any(|a| a.name.contains("2 X")));
    assert!(ministick_axes.iter().any(|a| a.name.contains("2 Y")));
}

/// X56 RGB presets cover all expected preset names and the Off preset is all-zero.
#[test]
fn x56_rhino_rgb_presets() {
    let p = x56_profile();
    assert!(p.rgb_presets.len() >= 5);

    let expected_presets = ["Default Blue", "Combat Red", "Night Green", "Amber Warm", "Off"];
    for name in &expected_presets {
        assert!(
            p.rgb_presets.iter().any(|pr| pr.name == *name),
            "missing preset '{}'",
            name
        );
    }

    // Off preset is truly all-zero
    let off = p.rgb_presets.iter().find(|pr| pr.name == "Off").unwrap();
    assert_eq!(off.stick_base, RgbColor::OFF);
    assert_eq!(off.stick_grip, RgbColor::OFF);
    assert_eq!(off.throttle_base, RgbColor::OFF);
    assert_eq!(off.throttle_grip, RgbColor::OFF);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Extreme 3D Pro depth tests (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Extreme 3D Pro: 4-axis joystick (X, Y, twist, throttle).
#[test]
fn extreme3dpro_basic_4_axis() {
    // Center position
    let data = build_e3dp_report(512, 512, 128, 0, 0, 8);
    let state = parse_extreme_3d_pro(&data).unwrap();
    assert!(state.axes.x.abs() < 0.01);
    assert!(state.axes.y.abs() < 0.01);
    assert!(state.axes.twist.abs() < 0.01);
    assert!(state.axes.throttle < 0.01);

    // Full deflection all axes
    let data = build_e3dp_report(1023, 0, 255, 127, 0, 8);
    let state = parse_extreme_3d_pro(&data).unwrap();
    assert!(state.axes.x > 0.99, "X full right");
    assert!(state.axes.y < -0.99, "Y full forward");
    assert!(state.axes.twist > 0.99, "twist full CW");
    assert!(state.axes.throttle > 0.99, "throttle max");

    // Opposite deflection
    let data = build_e3dp_report(0, 1023, 0, 0, 0, 8);
    let state = parse_extreme_3d_pro(&data).unwrap();
    assert!(state.axes.x < -0.99, "X full left");
    assert!(state.axes.y > 0.99, "Y full back");
    assert!(state.axes.twist < -0.99, "twist full CCW");
}

/// Extreme 3D Pro: twist rudder axis full range, center, and asymmetry.
#[test]
fn extreme3dpro_twist_rudder() {
    // Center
    let data = build_e3dp_report(512, 512, 128, 0, 0, 8);
    let center = parse_extreme_3d_pro(&data).unwrap().axes.twist;
    assert!(center.abs() < 0.02, "twist center: {}", center);

    // Full right twist
    let data = build_e3dp_report(512, 512, 255, 0, 0, 8);
    let right = parse_extreme_3d_pro(&data).unwrap().axes.twist;
    assert!(right > 0.99, "twist right: {}", right);

    // Full left twist
    let data = build_e3dp_report(512, 512, 0, 0, 0, 8);
    let left = parse_extreme_3d_pro(&data).unwrap().axes.twist;
    assert!(left < -0.99, "twist left: {}", left);

    // Quarter positions
    let data = build_e3dp_report(512, 512, 64, 0, 0, 8);
    let q1 = parse_extreme_3d_pro(&data).unwrap().axes.twist;
    assert!(q1 < -0.45 && q1 > -0.55, "twist quarter-left: {}", q1);

    let data = build_e3dp_report(512, 512, 192, 0, 0, 8);
    let q3 = parse_extreme_3d_pro(&data).unwrap().axes.twist;
    assert!(q3 > 0.45 && q3 < 0.55, "twist quarter-right: {}", q3);
}

/// Extreme 3D Pro: throttle slider 0..127 unipolar range.
#[test]
fn extreme3dpro_throttle_slider() {
    // Min
    let data = build_e3dp_report(512, 512, 128, 0, 0, 8);
    let min = parse_extreme_3d_pro(&data).unwrap().axes.throttle;
    assert!(min < 0.01, "throttle min: {}", min);

    // Max
    let data = build_e3dp_report(512, 512, 128, 127, 0, 8);
    let max = parse_extreme_3d_pro(&data).unwrap().axes.throttle;
    assert!(max > 0.99, "throttle max: {}", max);

    // Mid
    let data = build_e3dp_report(512, 512, 128, 64, 0, 8);
    let mid = parse_extreme_3d_pro(&data).unwrap().axes.throttle;
    assert!(
        (mid - 64.0 / 127.0).abs() < 0.02,
        "throttle mid: {}",
        mid
    );

    // Always unipolar (0.0..=1.0)
    for raw in (0..=127).step_by(10) {
        let data = build_e3dp_report(512, 512, 128, raw, 0, 8);
        let val = parse_extreme_3d_pro(&data).unwrap().axes.throttle;
        assert!(
            (0.0..=1.0).contains(&val),
            "throttle at raw {} = {} out of range",
            raw,
            val
        );
    }
}

/// Extreme 3D Pro: 12 buttons, individually addressable.
#[test]
fn extreme3dpro_12_buttons() {
    // Each button individually
    for b in 1u8..=12 {
        let mask = 1u16 << (b - 1);
        let data = build_e3dp_report(512, 512, 128, 0, mask, 8);
        let state = parse_extreme_3d_pro(&data).unwrap();
        assert!(state.buttons.button(b), "button {} should be ON", b);
        for other in 1u8..=12 {
            if other != b {
                assert!(
                    !state.buttons.button(other),
                    "button {} should be OFF when {} is ON",
                    other,
                    b
                );
            }
        }
    }

    // All pressed
    let data = build_e3dp_report(512, 512, 128, 0, 0x0FFF, 8);
    let state = parse_extreme_3d_pro(&data).unwrap();
    for b in 1u8..=12 {
        assert!(state.buttons.button(b));
    }

    // None pressed
    let data = build_e3dp_report(512, 512, 128, 0, 0, 8);
    let state = parse_extreme_3d_pro(&data).unwrap();
    for b in 1u8..=12 {
        assert!(!state.buttons.button(b));
    }

    // Out-of-range buttons always false
    let data = build_e3dp_report(512, 512, 128, 0, 0x0FFF, 8);
    let state = parse_extreme_3d_pro(&data).unwrap();
    assert!(!state.buttons.button(0));
    assert!(!state.buttons.button(13));
    assert!(!state.buttons.button(255));
}

/// Extreme 3D Pro: VID/PID device matching against Logitech Extreme 3D Pro.
#[test]
fn extreme3dpro_device_matching() {
    use flight_hotas_logitech::{EXTREME_3D_PRO_PID, LOGITECH_VENDOR_ID, is_extreme_3d_pro};

    assert_eq!(LOGITECH_VENDOR_ID, 0x046D);
    assert_eq!(EXTREME_3D_PRO_PID, 0xC215);
    assert!(is_extreme_3d_pro(LOGITECH_VENDOR_ID, EXTREME_3D_PRO_PID));
    assert!(!is_extreme_3d_pro(LOGITECH_VENDOR_ID, 0x0000));
    assert!(!is_extreme_3d_pro(0x0000, EXTREME_3D_PRO_PID));

    // Short reports rejected
    assert!(parse_extreme_3d_pro(&[0u8; 6]).is_err());
    assert!(parse_extreme_3d_pro(&[]).is_err());

    // 7+ byte reports accepted
    assert!(parse_extreme_3d_pro(&[0u8; 7]).is_ok());
    assert!(parse_extreme_3d_pro(&[0u8; 20]).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Profile tests (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// MSFS default profile suitability: X52 has MFD pages for nav/engine/radio.
#[test]
fn profile_msfs_default_x52() {
    let p = x52_profile();

    // MFD pages suitable for MSFS flight data
    assert_eq!(p.mfd_pages.len(), 3);
    let page_names: Vec<_> = p.mfd_pages.iter().map(|pg| pg.name).collect();
    assert!(page_names.contains(&"Navigation"));
    assert!(page_names.contains(&"Engine"));
    assert!(page_names.contains(&"Radio"));

    // All MFD lines within 16-char limit
    for page in &p.mfd_pages {
        for line in &page.lines {
            assert!(
                line.len() <= MFD_LINE_LENGTH,
                "'{}' exceeds MFD line length in {}",
                line,
                page.name
            );
        }
    }

    // LED defaults suitable for normal operation
    assert!(!p.led_defaults.is_empty());
}

/// DCS default profile suitability: X56 dual throttle + many buttons + hat switches.
#[test]
fn profile_dcs_default_x56() {
    let p = x56_profile();

    // DCS benefits from dual throttle for twin-engine aircraft
    assert!(
        p.axes.iter().any(|a| a.name == "Left Throttle"),
        "need left throttle"
    );
    assert!(
        p.axes.iter().any(|a| a.name == "Right Throttle"),
        "need right throttle"
    );

    // Lots of buttons for DCS bindings
    assert!(p.button_count >= 32, "DCS needs many buttons");

    // Multiple hat switches for view/weapon/countermeasures
    assert!(p.hat_count >= 4, "DCS benefits from many hats");

    // RGB presets for different aircraft
    assert!(!p.rgb_presets.is_empty());
}

/// Model-specific axis counts are correct and distinct.
#[test]
fn profile_model_specific_axis_counts() {
    let x52 = x52_profile();
    let x56 = x56_profile();
    let yoke = flight_yoke_profile();
    let rudder = rudder_pedals_profile();

    assert_eq!(x52.axes.len(), 9, "X52: 3 stick + throttle + 2 rotary + 2 mouse + slider");
    assert_eq!(x56.axes.len(), 9, "X56: 3 stick + 4 mini-stick + 2 throttle");
    assert_eq!(yoke.axes.len(), 8, "Yoke: 5 yoke + 3 throttle quadrant");
    assert_eq!(rudder.axes.len(), 3, "Rudder: 2 brakes + rudder");

    // Button counts are device-specific
    assert_eq!(x52.button_count, 34);
    assert_eq!(x56.button_count, 32);
    assert_eq!(yoke.button_count, 18);
    assert_eq!(rudder.button_count, 0);
}

/// Combined HOTAS setup: profiles can coexist and don't conflict.
#[test]
fn profile_combined_hotas_setup() {
    let profiles = [
        x52_profile(),
        x56_profile(),
        flight_yoke_profile(),
        rudder_pedals_profile(),
    ];

    // All have distinct names
    let names: Vec<_> = profiles.iter().map(|p| p.name).collect();
    for (i, a) in names.iter().enumerate() {
        for b in &names[i + 1..] {
            assert_ne!(a, b, "profile names must be unique");
        }
    }

    // All axes have non-empty names and valid resolution
    for p in &profiles {
        for axis in &p.axes {
            assert!(!axis.name.is_empty());
            assert!(axis.resolution_bits > 0 && axis.resolution_bits <= 16);
        }
    }

    // Total combined axis count
    let total_axes: usize = profiles.iter().map(|p| p.axes.len()).sum();
    assert!(
        total_axes >= 25,
        "combined setup should have many axes: {}",
        total_axes
    );
}

/// Profiles serialize to JSON without panicking (ensures Serialize derive works).
#[test]
fn profile_serialization() {
    let profiles = [
        x52_profile(),
        x56_profile(),
        flight_yoke_profile(),
        rudder_pedals_profile(),
    ];
    for p in &profiles {
        let json = serde_json::to_string(p);
        assert!(json.is_ok(), "{} should serialize: {:?}", p.name, json.err());
        let s = json.unwrap();
        assert!(s.contains(p.name));
        assert!(!s.is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Device identification tests (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// VID/PID matching for X52 and X52 Pro (Saitek VID).
#[test]
fn device_id_x52_vid_pid() {
    // X52 standard
    let x52 = identify_device(SAITEK_VID, 0x075C).unwrap();
    assert_eq!(x52.id, DeviceId::X52);
    assert_eq!(x52.vid, SAITEK_VID);
    assert!(x52.name.contains("X52"));

    // X52 Pro
    let x52p = identify_device(SAITEK_VID, 0x0762).unwrap();
    assert_eq!(x52p.id, DeviceId::X52Pro);
    assert!(x52p.name.contains("X52 Pro"));
}

/// VID/PID matching for X55/X56 family (Saitek + Mad Catz VIDs).
#[test]
fn device_id_x56_vid_pid() {
    // X55 Rhino stick (Saitek)
    let x55s = identify_device(SAITEK_VID, 0x2215).unwrap();
    assert_eq!(x55s.id, DeviceId::X55Stick);

    // X55 Rhino throttle (Saitek)
    let x55t = identify_device(SAITEK_VID, 0xA215).unwrap();
    assert_eq!(x55t.id, DeviceId::X55Throttle);

    // X56 Mad Catz stick
    let x56s = identify_device(MAD_CATZ_VID, 0x2221).unwrap();
    assert_eq!(x56s.id, DeviceId::X56MadCatzStick);
    assert!(x56s.name.contains("X56"));

    // X56 Mad Catz throttle
    let x56t = identify_device(MAD_CATZ_VID, 0xA221).unwrap();
    assert_eq!(x56t.id, DeviceId::X56MadCatzThrottle);
    assert!(x56t.name.contains("X56"));
}

/// Model discrimination: similar PIDs don't cross-match.
#[test]
fn device_id_model_discrimination() {
    // X52 PID doesn't match under Mad Catz VID
    assert!(identify_device(MAD_CATZ_VID, 0x075C).is_none());

    // X56 Mad Catz PID doesn't match under Saitek VID
    assert!(identify_device(SAITEK_VID, 0x2221).is_none());

    // X52 Pro PID doesn't match under Logitech VID
    assert!(identify_device(LOGITECH_VID, 0x0762).is_none());

    // Totally unknown device
    assert!(identify_device(0xDEAD, 0xBEEF).is_none());
    assert!(identify_device(0x0000, 0x0000).is_none());
}

/// No duplicate VID/PID entries in the device table.
#[test]
fn device_id_no_duplicates() {
    let table = protocol::DEVICE_TABLE;
    for (i, a) in table.iter().enumerate() {
        for b in &table[i + 1..] {
            assert!(
                !(a.vid == b.vid && a.pid == b.pid),
                "duplicate {:04X}:{:04X} ({} vs {})",
                a.vid,
                a.pid,
                a.name,
                b.name,
            );
        }
    }
}

/// VID constants match documented vendor IDs.
#[test]
fn device_id_firmware_variants() {
    assert_eq!(SAITEK_VID, 0x06A3, "Saitek VID");
    assert_eq!(MAD_CATZ_VID, 0x0738, "Mad Catz VID");
    assert_eq!(LOGITECH_VID, 0x046D, "Logitech VID");

    // Logitech-branded devices use LOGITECH_VID
    let yoke = identify_device(LOGITECH_VID, 0xC259).unwrap();
    assert_eq!(yoke.id, DeviceId::GFlightYoke);

    let throttle = identify_device(LOGITECH_VID, 0xC25A).unwrap();
    assert_eq!(throttle.id, DeviceId::GFlightThrottle);

    let rudder = identify_device(LOGITECH_VID, 0xC264).unwrap();
    assert_eq!(rudder.id, DeviceId::FlightRudderPedals);

    // Saitek-branded yoke
    let saitek_yoke = identify_device(SAITEK_VID, 0x0BAC).unwrap();
    assert_eq!(saitek_yoke.id, DeviceId::ProFlightYoke);

    // Saitek rudder pedals
    let saitek_rudder = identify_device(SAITEK_VID, 0x0763).unwrap();
    assert_eq!(saitek_rudder.id, DeviceId::ProFlightRudderPedals);
}
