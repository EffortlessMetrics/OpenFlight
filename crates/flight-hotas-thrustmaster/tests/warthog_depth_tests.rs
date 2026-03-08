// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive depth tests for the Thrustmaster HOTAS Warthog.
//!
//! Covers grip axes, grip buttons/hats, throttle axes, throttle buttons/toggles,
//! profile generation, and device identification for both the stick (VID 044F,
//! PID 0402) and throttle (VID 044F, PID 0404).

use flight_hotas_thrustmaster::profiles::{device_profile, AxisNormalization};
use flight_hotas_thrustmaster::protocol::{
    build_led_report, identify_device, is_pinkie_held, is_throttle_split,
    resolve_shifted_button, toggles, LedState, ThrustmasterDevice, VENDOR_ID,
    WARTHOG_LED_REPORT_ID, WARTHOG_STICK_PHYSICAL_BUTTONS, WARTHOG_THROTTLE_PHYSICAL_BUTTONS,
};
use flight_hotas_thrustmaster::{
    parse_warthog_stick, parse_warthog_throttle, WarthogHat, WarthogStickButtons,
    WarthogThrottleButtons, WARTHOG_JOYSTICK_PID, WARTHOG_THROTTLE_PID,
    WARTHOG_STICK_MIN_REPORT_BYTES, WARTHOG_THROTTLE_MIN_REPORT_BYTES,
};

// ─── Report builders ────────────────────────────────────────────────────────

fn stick_report(x: u16, y: u16, rz: u16, btn_low: u16, btn_high: u8, hat: u8) -> [u8; WARTHOG_STICK_MIN_REPORT_BYTES] {
    let mut r = [0u8; WARTHOG_STICK_MIN_REPORT_BYTES];
    r[0..2].copy_from_slice(&x.to_le_bytes());
    r[2..4].copy_from_slice(&y.to_le_bytes());
    r[4..6].copy_from_slice(&rz.to_le_bytes());
    r[6..8].copy_from_slice(&btn_low.to_le_bytes());
    r[8] = btn_high;
    r[9] = hat;
    r
}

#[allow(clippy::too_many_arguments)]
fn throttle_report(
    scx: u16,
    scy: u16,
    tl: u16,
    tr: u16,
    tc: u16,
    btn_low: u16,
    btn_mid: u16,
    btn_high: u8,
    toggle_bits: u8,
    hat_dms: u8,
    hat_csl: u8,
) -> Vec<u8> {
    let mut r = vec![0u8; WARTHOG_THROTTLE_MIN_REPORT_BYTES];
    r[0..2].copy_from_slice(&scx.to_le_bytes());
    r[2..4].copy_from_slice(&scy.to_le_bytes());
    r[4..6].copy_from_slice(&tl.to_le_bytes());
    r[6..8].copy_from_slice(&tr.to_le_bytes());
    r[8..10].copy_from_slice(&tc.to_le_bytes());
    r[10..12].copy_from_slice(&btn_low.to_le_bytes());
    r[12..14].copy_from_slice(&btn_mid.to_le_bytes());
    r[14] = btn_high;
    r[15] = toggle_bits;
    r[16] = hat_dms;
    r[17] = hat_csl;
    r
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. GRIP AXES (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Stick X/Y full range: extremes map to ±1.0.
#[test]
fn grip_axes_xy_full_range() {
    let left = parse_warthog_stick(&stick_report(0, 0, 32768, 0, 0, 0xFF)).unwrap();
    assert!(left.axes.x < -0.99, "full left X: {}", left.axes.x);
    assert!(left.axes.y < -0.99, "full forward Y: {}", left.axes.y);

    let right = parse_warthog_stick(&stick_report(65535, 65535, 32768, 0, 0, 0xFF)).unwrap();
    assert!(right.axes.x > 0.99, "full right X: {}", right.axes.x);
    assert!(right.axes.y > 0.99, "full back Y: {}", right.axes.y);
}

/// Micro-stick / RZ axis full range.
#[test]
fn grip_axes_microstick_range() {
    let left = parse_warthog_stick(&stick_report(32768, 32768, 0, 0, 0, 0xFF)).unwrap();
    assert!(left.axes.rz < -0.99, "rz full left: {}", left.axes.rz);

    let right = parse_warthog_stick(&stick_report(32768, 32768, 65535, 0, 0, 0xFF)).unwrap();
    assert!(right.axes.rz > 0.99, "rz full right: {}", right.axes.rz);
}

/// 16-bit axis resolution: adjacent raw values produce distinct outputs.
#[test]
fn grip_axes_resolution_16bit() {
    let a = parse_warthog_stick(&stick_report(32768, 32768, 32768, 0, 0, 0xFF)).unwrap();
    let b = parse_warthog_stick(&stick_report(32769, 32768, 32768, 0, 0, 0xFF)).unwrap();
    assert_ne!(
        a.axes.x, b.axes.x,
        "adjacent raw values must produce different normalised outputs"
    );
}

/// Center calibration: midpoint raw value normalises to near zero.
#[test]
fn grip_axes_center_calibration() {
    let s = parse_warthog_stick(&stick_report(32768, 32768, 32768, 0, 0, 0xFF)).unwrap();
    assert!(s.axes.x.abs() < 0.001, "center x: {}", s.axes.x);
    assert!(s.axes.y.abs() < 0.001, "center y: {}", s.axes.y);
    assert!(s.axes.rz.abs() < 0.001, "center rz: {}", s.axes.rz);
}

/// Linearity: monotonically increasing raw values produce monotonically increasing outputs.
#[test]
fn grip_axes_linearity() {
    let low = parse_warthog_stick(&stick_report(40000, 32768, 32768, 0, 0, 0xFF)).unwrap();
    let mid = parse_warthog_stick(&stick_report(50000, 32768, 32768, 0, 0, 0xFF)).unwrap();
    let high = parse_warthog_stick(&stick_report(60000, 32768, 32768, 0, 0, 0xFF)).unwrap();

    assert!(low.axes.x < mid.axes.x, "low < mid");
    assert!(mid.axes.x < high.axes.x, "mid < high");
    // Linear normalization: equal raw increments produce equal output increments
    let delta1 = mid.axes.x - low.axes.x;
    let delta2 = high.axes.x - mid.axes.x;
    assert!(
        (delta1 - delta2).abs() < 0.01,
        "deltas should be nearly equal: {} vs {}",
        delta1,
        delta2
    );
}

/// Deadzone: very small deflections from center are detectable.
#[test]
fn grip_axes_deadzone_sensitivity() {
    let center = parse_warthog_stick(&stick_report(32768, 32768, 32768, 0, 0, 0xFF)).unwrap();
    let tiny = parse_warthog_stick(&stick_report(32868, 32768, 32768, 0, 0, 0xFF)).unwrap();
    assert!(
        (tiny.axes.x - center.axes.x).abs() > 0.001,
        "tiny deflection must be detectable"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. GRIP BUTTONS (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// All 19 individual buttons can be set and read back in isolation.
#[test]
fn grip_buttons_all_19_individual() {
    for n in 1u8..=19 {
        let (btn_low, btn_high) = if n <= 16 {
            (1u16 << (n - 1), 0u8)
        } else {
            (0u16, 1u8 << (n - 17))
        };
        let state =
            parse_warthog_stick(&stick_report(32768, 32768, 32768, btn_low, btn_high, 0xFF))
                .unwrap();
        assert!(state.buttons.button(n), "button {} not detected", n);
        for m in 1u8..=19 {
            if m != n {
                assert!(
                    !state.buttons.button(m),
                    "button {} should not be set when only {} pressed",
                    m,
                    n
                );
            }
        }
    }
}

/// Hat positions: all 8-way + center directions on the stick hat.
#[test]
fn grip_buttons_hat_all_positions() {
    let cases = [
        (0x00u8, WarthogHat::North),
        (0x20, WarthogHat::East),
        (0x40, WarthogHat::South),
        (0x60, WarthogHat::West),
        (0x10, WarthogHat::NorthEast),
        (0x30, WarthogHat::SouthEast),
        (0x50, WarthogHat::SouthWest),
        (0x70, WarthogHat::NorthWest),
        (0xF0, WarthogHat::Center),
    ];
    for (hat_byte, expected) in &cases {
        let state =
            parse_warthog_stick(&stick_report(32768, 32768, 32768, 0, 0, *hat_byte)).unwrap();
        assert_eq!(
            state.buttons.hat, *expected,
            "hat byte 0x{:02X} -> {:?}, expected {:?}",
            hat_byte, state.buttons.hat, expected
        );
    }
}

/// Pinky switch (button 2) detection via `is_pinkie_held`.
#[test]
fn grip_buttons_pinky_switch() {
    let b_set = WarthogStickButtons {
        buttons_low: 0x0002,
        buttons_high: 0,
        hat: WarthogHat::Center,
    };
    assert!(is_pinkie_held(&b_set));

    let b_clear = WarthogStickButtons {
        buttons_low: 0x0000,
        buttons_high: 0,
        hat: WarthogHat::Center,
    };
    assert!(!is_pinkie_held(&b_clear));
}

/// Weapon release trigger (button 1) and mid-range button verification.
#[test]
fn grip_buttons_weapon_release_and_nws() {
    let state =
        parse_warthog_stick(&stick_report(32768, 32768, 32768, 0x0001, 0, 0xFF)).unwrap();
    assert!(state.buttons.button(1), "trigger / weapon release");

    let state2 =
        parse_warthog_stick(&stick_report(32768, 32768, 32768, 0x4000, 0, 0xFF)).unwrap();
    assert!(state2.buttons.button(15), "button 15");
}

/// Paddle / button 19 (highest stick button), out-of-range returns false.
#[test]
fn grip_buttons_paddle_highest() {
    let state = parse_warthog_stick(&stick_report(32768, 32768, 32768, 0, 0x04, 0xFF)).unwrap();
    assert!(state.buttons.button(19), "paddle / button 19");
    assert!(!state.buttons.button(20), "button 20 out of range");
}

/// All 19 buttons pressed simultaneously plus hat north.
#[test]
fn grip_buttons_all_pressed() {
    let state =
        parse_warthog_stick(&stick_report(32768, 32768, 32768, 0xFFFF, 0x07, 0x00)).unwrap();
    for n in 1u8..=19 {
        assert!(state.buttons.button(n), "button {} should be pressed", n);
    }
    assert_eq!(state.buttons.hat, WarthogHat::North);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. THROTTLE AXES (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Left and right throttles operate independently.
#[test]
fn throttle_axes_dual_independent() {
    let state = parse_warthog_throttle(&throttle_report(
        32768, 32768, 65535, 0, 32768, 0, 0, 0, 0, 0xFF, 0xFF,
    ))
    .unwrap();
    assert!(state.axes.throttle_left > 0.99, "left full: {}", state.axes.throttle_left);
    assert!(state.axes.throttle_right < 0.001, "right idle: {}", state.axes.throttle_right);

    let state2 = parse_warthog_throttle(&throttle_report(
        32768, 32768, 0, 65535, 32768, 0, 0, 0, 0, 0xFF, 0xFF,
    ))
    .unwrap();
    assert!(state2.axes.throttle_left < 0.001, "left idle: {}", state2.axes.throttle_left);
    assert!(state2.axes.throttle_right > 0.99, "right full: {}", state2.axes.throttle_right);
}

/// Combined throttle axis idle / full / mid-range.
#[test]
fn throttle_axes_combined_friction() {
    let idle = parse_warthog_throttle(&throttle_report(
        32768, 32768, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF,
    ))
    .unwrap();
    assert!(idle.axes.throttle_combined < 0.001, "combined idle: {}", idle.axes.throttle_combined);

    let full = parse_warthog_throttle(&throttle_report(
        32768, 32768, 0, 0, 65535, 0, 0, 0, 0, 0xFF, 0xFF,
    ))
    .unwrap();
    assert!(full.axes.throttle_combined > 0.99, "combined full: {}", full.axes.throttle_combined);

    let mid = parse_warthog_throttle(&throttle_report(
        32768, 32768, 0, 0, 32768, 0, 0, 0, 0, 0xFF, 0xFF,
    ))
    .unwrap();
    assert!(
        (0.4..=0.6).contains(&mid.axes.throttle_combined),
        "combined mid: {}",
        mid.axes.throttle_combined
    );
}

/// Slew control X/Y full bipolar range.
#[test]
fn throttle_axes_slew_control_xy() {
    let left = parse_warthog_throttle(&throttle_report(
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF,
    ))
    .unwrap();
    assert!(left.axes.slew_x < -0.99, "slew_x full left: {}", left.axes.slew_x);
    assert!(left.axes.slew_y < -0.99, "slew_y full up: {}", left.axes.slew_y);

    let right = parse_warthog_throttle(&throttle_report(
        65535, 65535, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF,
    ))
    .unwrap();
    assert!(right.axes.slew_x > 0.99, "slew_x full right: {}", right.axes.slew_x);
    assert!(right.axes.slew_y > 0.99, "slew_y full down: {}", right.axes.slew_y);
}

/// Micro-stick on throttle centered.
#[test]
fn throttle_axes_microstick_centered() {
    let state = parse_warthog_throttle(&throttle_report(
        32768, 32768, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF,
    ))
    .unwrap();
    assert!(state.axes.slew_x.abs() < 0.01, "slew_x centered: {}", state.axes.slew_x);
    assert!(state.axes.slew_y.abs() < 0.01, "slew_y centered: {}", state.axes.slew_y);
}

/// Reverse range: throttle at zero produces 0.0.
#[test]
fn throttle_axes_reverse_range_zero() {
    let state = parse_warthog_throttle(&throttle_report(
        32768, 32768, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF,
    ))
    .unwrap();
    assert!(state.axes.throttle_left < 0.001, "left zero: {}", state.axes.throttle_left);
    assert!(state.axes.throttle_right < 0.001, "right zero: {}", state.axes.throttle_right);
    assert!(
        state.axes.throttle_combined < 0.001,
        "combined zero: {}",
        state.axes.throttle_combined
    );
}

/// Throttle axes unipolar range: all values 0.0..=1.0 across sweep.
#[test]
fn throttle_axes_unipolar_range_sweep() {
    for raw in [0u16, 1, 16383, 32767, 32768, 49151, 65534, 65535] {
        let state = parse_warthog_throttle(&throttle_report(
            32768, 32768, raw, raw, raw, 0, 0, 0, 0, 0xFF, 0xFF,
        ))
        .unwrap();
        assert!(
            (0.0..=1.0).contains(&state.axes.throttle_left),
            "tl raw={}: {}",
            raw,
            state.axes.throttle_left
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.throttle_right),
            "tr raw={}: {}",
            raw,
            state.axes.throttle_right
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.throttle_combined),
            "tc raw={}: {}",
            raw,
            state.axes.throttle_combined
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. THROTTLE BUTTONS (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Toggle switches: all 8 toggle bits can be set and read individually.
#[test]
fn throttle_buttons_toggle_switches() {
    let all_set = WarthogThrottleButtons {
        buttons_low: 0,
        buttons_mid: 0,
        buttons_high: 0,
        toggles: 0xFF,
        hat_dms: WarthogHat::Center,
        hat_csl: WarthogHat::Center,
    };
    assert!(toggles::is_set(all_set.toggles, toggles::EFL_NORM));
    assert!(toggles::is_set(all_set.toggles, toggles::EFR_NORM));
    assert!(toggles::is_set(all_set.toggles, toggles::EOL_NORM));
    assert!(toggles::is_set(all_set.toggles, toggles::EOR_NORM));
    assert!(toggles::is_set(all_set.toggles, toggles::APU_START));
    assert!(toggles::is_set(all_set.toggles, toggles::LGSIL));
    assert!(toggles::is_set(all_set.toggles, toggles::SPDF));
    assert!(toggles::is_set(all_set.toggles, toggles::SPDB));

    let apu_only = WarthogThrottleButtons {
        toggles: 1 << toggles::APU_START,
        ..Default::default()
    };
    assert!(toggles::is_set(apu_only.toggles, toggles::APU_START));
    assert!(!toggles::is_set(apu_only.toggles, toggles::EFL_NORM));
}

/// All 40 momentary buttons can be individually addressed.
#[test]
fn throttle_buttons_momentary_all_40() {
    for n in 1u8..=40 {
        let (btn_low, btn_mid, btn_high) = match n {
            1..=16 => (1u16 << (n - 1), 0u16, 0u8),
            17..=32 => (0u16, 1u16 << (n - 17), 0u8),
            33..=40 => (0u16, 0u16, 1u8 << (n - 33)),
            _ => unreachable!(),
        };
        let state = parse_warthog_throttle(&throttle_report(
            32768, 32768, 0, 0, 0, btn_low, btn_mid, btn_high, 0, 0xFF, 0xFF,
        ))
        .unwrap();
        assert!(state.buttons.button(n), "throttle button {} not detected", n);
        if n > 1 {
            assert!(
                !state.buttons.button(n - 1),
                "button {} should not be set when only {} pressed",
                n - 1,
                n
            );
        }
    }
}

/// China hat (DMS hat) all 4 directions + center.
#[test]
fn throttle_buttons_china_hat_dms() {
    let directions = [
        (0x00u8, WarthogHat::North),
        (0x02, WarthogHat::East),
        (0x04, WarthogHat::South),
        (0x06, WarthogHat::West),
        (0x0F, WarthogHat::Center),
    ];
    for (byte, expected) in &directions {
        let state = parse_warthog_throttle(&throttle_report(
            32768, 32768, 0, 0, 0, 0, 0, 0, 0, *byte, 0xFF,
        ))
        .unwrap();
        assert_eq!(
            state.buttons.hat_dms, *expected,
            "DMS hat byte 0x{:02X} -> {:?}",
            byte, state.buttons.hat_dms
        );
    }
}

/// Boat switch / CSL hat all directions + center.
#[test]
fn throttle_buttons_boat_switch_csl() {
    let directions = [
        (0x00u8, WarthogHat::North),
        (0x02, WarthogHat::East),
        (0x04, WarthogHat::South),
        (0x06, WarthogHat::West),
        (0x0F, WarthogHat::Center),
    ];
    for (byte, expected) in &directions {
        let state = parse_warthog_throttle(&throttle_report(
            32768, 32768, 0, 0, 0, 0, 0, 0, 0, 0xFF, *byte,
        ))
        .unwrap();
        assert_eq!(
            state.buttons.hat_csl, *expected,
            "CSL hat byte 0x{:02X} -> {:?}",
            byte, state.buttons.hat_csl
        );
    }
}

/// Speed brake forward/back toggle positions.
#[test]
fn throttle_buttons_flaps_speed_brake() {
    let spdf = WarthogThrottleButtons {
        toggles: 1 << toggles::SPDF,
        ..Default::default()
    };
    assert!(toggles::is_set(spdf.toggles, toggles::SPDF));
    assert!(!toggles::is_set(spdf.toggles, toggles::SPDB));

    let spdb = WarthogThrottleButtons {
        toggles: 1 << toggles::SPDB,
        ..Default::default()
    };
    assert!(toggles::is_set(spdb.toggles, toggles::SPDB));
    assert!(!toggles::is_set(spdb.toggles, toggles::SPDF));

    let center = WarthogThrottleButtons {
        toggles: 0,
        ..Default::default()
    };
    assert!(!toggles::is_set(center.toggles, toggles::SPDF));
    assert!(!toggles::is_set(center.toggles, toggles::SPDB));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. PROFILE GENERATION (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// DCS A-10C profile: Warthog axis counts match hardware.
#[test]
fn profile_dcs_a10c_stick_and_throttle() {
    let stick = device_profile(ThrustmasterDevice::WarthogJoystick).unwrap();
    let throttle = device_profile(ThrustmasterDevice::WarthogThrottle).unwrap();

    assert_eq!(stick.axes.len(), 2, "stick axes");
    assert_eq!(throttle.axes.len(), 5, "throttle axes");
    assert_eq!(stick.button_count, WARTHOG_STICK_PHYSICAL_BUTTONS);
    assert_eq!(throttle.button_count, WARTHOG_THROTTLE_PHYSICAL_BUTTONS);
    assert_eq!(stick.hat_count, 1);
    assert_eq!(throttle.hat_count, 2);
}

/// Default axis normalization types match expected encoding.
#[test]
fn profile_default_axes_normalization() {
    let stick = device_profile(ThrustmasterDevice::WarthogJoystick).unwrap();
    for ax in &stick.axes {
        match ax.normalization {
            AxisNormalization::Bipolar { center, half_span } => {
                assert!(
                    (center - 32767.5).abs() < 0.01,
                    "stick {} center: {}",
                    ax.id,
                    center
                );
                assert!(
                    (half_span - 32767.5).abs() < 0.01,
                    "stick {} half_span: {}",
                    ax.id,
                    half_span
                );
            }
            _ => panic!("stick axis {} should be bipolar", ax.id),
        }
    }

    let throttle = device_profile(ThrustmasterDevice::WarthogThrottle).unwrap();
    for ax in &throttle.axes {
        match ax.id {
            "throttle_left" | "throttle_right" | "throttle_combined" => {
                assert!(
                    matches!(ax.normalization, AxisNormalization::Unipolar { .. }),
                    "throttle axis {} should be unipolar",
                    ax.id
                );
            }
            "slew_x" | "slew_y" => {
                assert!(
                    matches!(ax.normalization, AxisNormalization::Bipolar { .. }),
                    "slew axis {} should be bipolar",
                    ax.id
                );
            }
            _ => {}
        }
    }
}

/// Button-to-command mapping: pinky shift resolves correctly.
#[test]
fn profile_button_to_command_mapping_shift() {
    for phys in 1..=WARTHOG_STICK_PHYSICAL_BUTTONS {
        assert_eq!(resolve_shifted_button(phys, false), Some(phys));
    }

    assert_eq!(resolve_shifted_button(2, true), Some(2));
    assert_eq!(resolve_shifted_button(1, true), Some(20));
    assert_eq!(resolve_shifted_button(3, true), Some(21));
    assert_eq!(resolve_shifted_button(19, true), Some(37));

    let mut shifted: Vec<u8> = (1..=WARTHOG_STICK_PHYSICAL_BUTTONS)
        .filter_map(|n| resolve_shifted_button(n, true))
        .collect();
    let count = shifted.len();
    shifted.sort();
    shifted.dedup();
    assert_eq!(shifted.len(), count, "shifted mapping has collisions");
}

/// Mode layers: LED control for Warthog throttle backlight.
#[test]
fn profile_mode_layers_led() {
    let off = build_led_report(LedState::Off);
    assert_eq!(off, [WARTHOG_LED_REPORT_ID, 0x00]);

    for level in 1..=5u8 {
        let report = build_led_report(LedState::Brightness(level));
        assert_eq!(report[0], WARTHOG_LED_REPORT_ID);
        assert_eq!(report[1], level);
    }

    let clamped = build_led_report(LedState::Brightness(255));
    assert_eq!(clamped[1], LedState::MAX_BRIGHTNESS);
}

/// Profile metadata: names, notes, and LED capability flags.
#[test]
fn profile_metadata_completeness() {
    let stick = device_profile(ThrustmasterDevice::WarthogJoystick).unwrap();
    assert_eq!(stick.name, "HOTAS Warthog Joystick");
    assert!(!stick.has_leds, "stick has no LEDs");
    assert!(stick.notes.contains("Metal"), "stick notes mention Metal gimbal");

    let throttle = device_profile(ThrustmasterDevice::WarthogThrottle).unwrap();
    assert_eq!(throttle.name, "HOTAS Warthog Throttle");
    assert!(throttle.has_leds, "throttle has LEDs");
    assert!(
        throttle.notes.contains("interlock") || throttle.notes.contains("split"),
        "throttle notes mention split/interlock"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. DEVICE IDENTIFICATION (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// VID/PID matching for Warthog stick (044F:0402).
#[test]
fn device_id_vid_pid_stick() {
    assert_eq!(WARTHOG_JOYSTICK_PID, 0x0402);
    assert_eq!(
        identify_device(VENDOR_ID, WARTHOG_JOYSTICK_PID),
        Some(ThrustmasterDevice::WarthogJoystick)
    );
}

/// VID/PID matching for Warthog throttle (044F:0404).
#[test]
fn device_id_vid_pid_throttle() {
    assert_eq!(WARTHOG_THROTTLE_PID, 0x0404);
    assert_eq!(
        identify_device(VENDOR_ID, WARTHOG_THROTTLE_PID),
        Some(ThrustmasterDevice::WarthogThrottle)
    );
}

/// Stick vs throttle discrimination: same VID, different PIDs produce distinct variants.
#[test]
fn device_id_stick_vs_throttle_discrimination() {
    let stick = identify_device(VENDOR_ID, 0x0402);
    let throttle = identify_device(VENDOR_ID, 0x0404);

    assert_ne!(stick, throttle, "stick and throttle must be different devices");
    assert_eq!(stick, Some(ThrustmasterDevice::WarthogJoystick));
    assert_eq!(throttle, Some(ThrustmasterDevice::WarthogThrottle));
}

/// Combined HOTAS setup: both devices identified from a single enumeration pass.
#[test]
fn device_id_combined_hotas_setup() {
    let devices = [(VENDOR_ID, 0x0402u16), (VENDOR_ID, 0x0404u16)];
    let identified: Vec<_> = devices
        .iter()
        .filter_map(|&(vid, pid)| identify_device(vid, pid))
        .collect();
    assert_eq!(identified.len(), 2, "both devices identified");
    assert!(identified.contains(&ThrustmasterDevice::WarthogJoystick));
    assert!(identified.contains(&ThrustmasterDevice::WarthogThrottle));
}

/// Wrong VID or unknown PID returns None.
#[test]
fn device_id_wrong_vid_unknown_pid() {
    assert_eq!(identify_device(0x1234, 0x0402), None, "wrong VID");
    assert_eq!(identify_device(VENDOR_ID, 0xFFFF), None, "unknown PID");
    assert_eq!(identify_device(0x0000, 0x0000), None, "zero VID/PID");
}

// ═══════════════════════════════════════════════════════════════════════════════
// BONUS: Additional depth tests
// ═══════════════════════════════════════════════════════════════════════════════

/// Throttle split detection when levers diverge.
#[test]
fn throttle_split_merge_detection() {
    assert!(!is_throttle_split(0.5, 0.5), "merged");
    assert!(!is_throttle_split(0.5, 0.51), "within tolerance");
    assert!(is_throttle_split(0.2, 0.8), "clearly split");
    assert!(is_throttle_split(0.0, 0.05), "small divergence past threshold");
}

/// Report length validation: too-short reports are rejected.
#[test]
fn report_length_validation() {
    assert!(parse_warthog_stick(&[0u8; 0]).is_err());
    assert!(parse_warthog_stick(&[0u8; WARTHOG_STICK_MIN_REPORT_BYTES - 1]).is_err());
    assert!(parse_warthog_stick(&[0u8; WARTHOG_STICK_MIN_REPORT_BYTES]).is_ok());

    assert!(parse_warthog_throttle(&[0u8; 0]).is_err());
    assert!(parse_warthog_throttle(&[0u8; WARTHOG_THROTTLE_MIN_REPORT_BYTES - 1]).is_err());
    assert!(parse_warthog_throttle(&[0u8; WARTHOG_THROTTLE_MIN_REPORT_BYTES]).is_ok());
}

/// Oversized reports parse without panic (extra bytes ignored).
#[test]
fn oversized_reports_accepted() {
    let stick_big = [0u8; 64];
    assert!(parse_warthog_stick(&stick_big).is_ok());

    let throttle_big = [0u8; 128];
    assert!(parse_warthog_throttle(&throttle_big).is_ok());
}

/// Device name lookup returns correct names for Warthog variants.
#[test]
fn device_names_correct() {
    assert_eq!(ThrustmasterDevice::WarthogJoystick.name(), "HOTAS Warthog Joystick");
    assert_eq!(ThrustmasterDevice::WarthogThrottle.name(), "HOTAS Warthog Throttle");
}
