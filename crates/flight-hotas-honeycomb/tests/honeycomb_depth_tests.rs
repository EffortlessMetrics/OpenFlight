// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for Honeycomb Alpha/Bravo devices — yoke parsing, throttle
//! quadrant, annunciator panel, calibration, profiles, and property tests.

use flight_hotas_honeycomb::alpha::ALPHA_REPORT_LEN;
use flight_hotas_honeycomb::bravo::BRAVO_REPORT_LEN;
use flight_hotas_honeycomb::bravo_leds::{BravoLedState, serialize_led_report};
use flight_hotas_honeycomb::profiles::{
    ALPHA_AXES, ALPHA_PROFILE, BRAVO_AXES, BRAVO_PROFILE, CHARLIE_PROFILE,
};
use flight_hotas_honeycomb::protocol::{
    self, GearIndicatorState, MagnetoPosition, decode_toggle_switch, ToggleSwitchState,
};
use flight_hotas_honeycomb::{
    parse_alpha_report, parse_bravo_report, HONEYCOMB_ALPHA_YOKE_PID, HONEYCOMB_BRAVO_PID,
    HONEYCOMB_VENDOR_ID,
};

// ── Report builders ──────────────────────────────────────────────────────────

fn alpha_report(roll: u16, pitch: u16, buttons: u64, hat: u8) -> [u8; ALPHA_REPORT_LEN] {
    let mut r = [0u8; ALPHA_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5] = (buttons & 0xFF) as u8;
    r[6] = ((buttons >> 8) & 0xFF) as u8;
    r[7] = ((buttons >> 16) & 0xFF) as u8;
    r[8] = ((buttons >> 24) & 0xFF) as u8;
    r[9] = ((buttons >> 32) & 0xFF) as u8;
    r[10] = hat & 0x0F;
    r
}

fn bravo_report(throttles: [u16; 7], buttons: u64) -> [u8; BRAVO_REPORT_LEN] {
    let mut r = [0u8; BRAVO_REPORT_LEN];
    r[0] = 0x01;
    for (i, &t) in throttles.iter().enumerate() {
        let off = 1 + i * 2;
        r[off..off + 2].copy_from_slice(&t.to_le_bytes());
    }
    r[15..23].copy_from_slice(&buttons.to_le_bytes());
    r
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Alpha Yoke Tests (8)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn alpha_pitch_full_range_forward_and_back() {
    let forward = parse_alpha_report(&alpha_report(2048, 0, 0, 15)).unwrap();
    let back = parse_alpha_report(&alpha_report(2048, 4095, 0, 15)).unwrap();

    assert!(forward.axes.pitch < -0.99, "full forward pitch ≈ -1.0");
    assert!(back.axes.pitch > 0.99, "full back pitch ≈ +1.0");
    // Verify monotonicity: increasing raw → increasing normalised
    assert!(back.axes.pitch > forward.axes.pitch);
}

#[test]
fn alpha_roll_full_range_left_and_right() {
    let left = parse_alpha_report(&alpha_report(0, 2048, 0, 15)).unwrap();
    let right = parse_alpha_report(&alpha_report(4095, 2048, 0, 15)).unwrap();

    assert!(left.axes.roll < -0.99, "full left roll ≈ -1.0");
    assert!(right.axes.roll > 0.99, "full right roll ≈ +1.0");
    assert!(right.axes.roll > left.axes.roll);
}

/// The Alpha Yoke doesn't have a dedicated trim axis, but trim wheel buttons
/// are mapped as momentary button presses. Verify button detection for typical
/// trim-wheel-equivalent buttons (buttons in the high range).
#[test]
fn alpha_trim_wheel_buttons() {
    // Buttons 33–36 are in the upper range of the Alpha's 36-button space.
    // Verify that high-numbered buttons parse correctly.
    for btn_num in 33u8..=36 {
        let mask: u64 = 1u64 << (btn_num - 1);
        let state = parse_alpha_report(&alpha_report(2048, 2048, mask, 15)).unwrap();
        assert!(
            state.buttons.is_pressed(btn_num),
            "button {btn_num} should be pressed"
        );
        // Adjacent buttons should NOT be pressed
        if btn_num > 1 {
            assert!(!state.buttons.is_pressed(btn_num - 1));
        }
    }
}

/// Verify the AP-related button mapping on the Alpha Yoke.
/// Buttons 1 (PTT) and 2 (AP Disconnect) are the primary yoke buttons.
#[test]
fn alpha_button_mapping_ptt_and_ap_disconnect() {
    // Button 1 = PTT, Button 2 = AP Disconnect (from profiles)
    let ptt_mask: u64 = 1 << 0;
    let ap_disc_mask: u64 = 1 << 1;
    let both: u64 = ptt_mask | ap_disc_mask;

    let state = parse_alpha_report(&alpha_report(2048, 2048, both, 15)).unwrap();
    assert!(state.buttons.is_pressed(1), "PTT (button 1) pressed");
    assert!(
        state.buttons.is_pressed(2),
        "AP Disconnect (button 2) pressed"
    );
    assert!(!state.buttons.is_pressed(3), "button 3 not pressed");
}

/// Verify magneto switch positions decode correctly from the Alpha button mask.
/// Buttons 25 (R), 26 (L), 27 (Start) encode the magneto position.
#[test]
fn alpha_magneto_switch_all_positions() {
    let positions = [
        (0u64, MagnetoPosition::Off),
        (1u64 << 24, MagnetoPosition::Right),
        (1u64 << 25, MagnetoPosition::Left),
        ((1u64 << 24) | (1u64 << 25), MagnetoPosition::Both),
        (1u64 << 26, MagnetoPosition::Start),
    ];
    for (mask, expected) in positions {
        let decoded = protocol::decode_magneto(mask);
        assert_eq!(decoded, expected, "mask=0x{mask:X} → {expected:?}");
    }
}

/// Verify rocker-style switches on the Alpha, which occupy consecutive button
/// pairs. Pressing the upper rocker = one button, lower rocker = another.
#[test]
fn alpha_rocker_switch_pairs() {
    // Simulate a rocker switch pair: buttons 3/4 (hat up/right on the profile,
    // but in terms of raw mask they're just consecutive buttons)
    let upper: u64 = 1 << 2; // button 3
    let lower: u64 = 1 << 3; // button 4

    let state_up = parse_alpha_report(&alpha_report(2048, 2048, upper, 15)).unwrap();
    assert!(state_up.buttons.is_pressed(3));
    assert!(!state_up.buttons.is_pressed(4));

    let state_down = parse_alpha_report(&alpha_report(2048, 2048, lower, 15)).unwrap();
    assert!(!state_down.buttons.is_pressed(3));
    assert!(state_down.buttons.is_pressed(4));
}

/// Verify all 8 hat directions plus centre are decoded correctly.
#[test]
fn alpha_hat_all_directions() {
    let expected: [(u8, u8, &str); 9] = [
        (15, 0, "center"), // raw 15 → centred
        (0, 1, "N"),
        (1, 2, "NE"),
        (2, 3, "E"),
        (3, 4, "SE"),
        (4, 5, "S"),
        (5, 6, "SW"),
        (6, 7, "W"),
        (7, 8, "NW"),
    ];
    for (raw, hat_val, direction) in expected {
        let state = parse_alpha_report(&alpha_report(2048, 2048, 0, raw)).unwrap();
        assert_eq!(state.buttons.hat, hat_val, "raw={raw} → hat={hat_val}");
        assert_eq!(
            state.buttons.hat_direction(),
            direction,
            "raw={raw} → {direction}"
        );
    }
}

/// Verify that buttons outside the valid 1–36 range return false.
#[test]
fn alpha_button_out_of_range() {
    let state = parse_alpha_report(&alpha_report(2048, 2048, u64::MAX, 15)).unwrap();
    assert!(!state.buttons.is_pressed(0), "button 0 is out of range");
    assert!(!state.buttons.is_pressed(37), "button 37 is out of range");
    assert!(!state.buttons.is_pressed(255), "button 255 is out of range");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Bravo Throttle Tests (8)
// ═══════════════════════════════════════════════════════════════════════════════

/// Each of the 5 throttle levers and flap/spoiler axes should be independently
/// controllable.
#[test]
fn bravo_six_levers_independent() {
    // Set each lever to a distinct value, others to 0
    let values: [u16; 7] = [4095, 3000, 2000, 1000, 500, 250, 100];
    let state = parse_bravo_report(&bravo_report(values, 0)).unwrap();

    let axes = [
        state.axes.throttle1,
        state.axes.throttle2,
        state.axes.throttle3,
        state.axes.throttle4,
        state.axes.throttle5,
        state.axes.flap_lever,
        state.axes.spoiler,
    ];
    // Each axis should be different
    for i in 0..axes.len() {
        for j in (i + 1)..axes.len() {
            assert!(
                (axes[i] - axes[j]).abs() > 0.01,
                "axes[{i}]={} should differ from axes[{j}]={}",
                axes[i],
                axes[j]
            );
        }
    }
    // First should be max, last should be near 100/4095
    assert!((axes[0] - 1.0).abs() < 1e-4, "throttle1 at max ≈ 1.0");
    assert!(axes[6] < 0.03, "spoiler at 100/4095 ≈ 0.024");
}

/// The flap lever has ~6 detent positions. Verify that evenly-spaced raw
/// values produce distinct normalised outputs.
#[test]
fn bravo_flap_selector_6_positions() {
    let detents: [u16; 6] = [0, 819, 1638, 2457, 3276, 4095];
    let mut prev = -1.0f32;
    for raw in detents {
        let state = parse_bravo_report(&bravo_report([0, 0, 0, 0, 0, raw, 0], 0)).unwrap();
        assert!(
            state.axes.flap_lever > prev,
            "flap={raw} → {} should be > prev={prev}",
            state.axes.flap_lever
        );
        prev = state.axes.flap_lever;
    }
    // First detent = 0.0, last = 1.0
    let first = parse_bravo_report(&bravo_report([0, 0, 0, 0, 0, 0, 0], 0)).unwrap();
    let last = parse_bravo_report(&bravo_report([0, 0, 0, 0, 0, 4095, 0], 0)).unwrap();
    assert!(first.axes.flap_lever < 0.001);
    assert!((last.axes.flap_lever - 1.0).abs() < 1e-4);
}

/// Gear lever has three states derived from buttons 31 (gear up) and 32 (gear down).
#[test]
fn bravo_gear_lever_three_states() {
    let gear_up_mask: u64 = 1 << 30;
    let gear_down_mask: u64 = 1 << 31;

    let up = GearIndicatorState::from_button_mask(gear_up_mask);
    let down = GearIndicatorState::from_button_mask(gear_down_mask);
    let transit = GearIndicatorState::from_button_mask(0);

    assert_eq!(up, GearIndicatorState::Up);
    assert_eq!(down, GearIndicatorState::Down);
    assert_eq!(transit, GearIndicatorState::Transit);

    // LED colors should match
    assert_eq!(up.led_colors(), (false, false), "gear up: no LEDs");
    assert_eq!(down.led_colors(), (true, false), "gear down: green");
    assert_eq!(transit.led_colors(), (false, true), "transit: red");
}

/// Trim wheel is reported via buttons 22 (trim down) and 23 (trim up).
#[test]
fn bravo_trim_buttons() {
    let trim_down: u64 = 1 << 21; // bit 21 = button 22 (trim down)
    let trim_up: u64 = 1 << 22; // bit 22 = button 23 (trim up)

    let state_down = parse_bravo_report(&bravo_report([0; 7], trim_down)).unwrap();
    assert!(state_down.buttons.is_pressed(22), "trim down button");
    assert!(!state_down.buttons.is_pressed(23));

    let state_up = parse_bravo_report(&bravo_report([0; 7], trim_up)).unwrap();
    assert!(state_up.buttons.is_pressed(23), "trim up button");
    assert!(!state_up.buttons.is_pressed(22));
}

/// All 8 autopilot mode buttons (HDG, NAV, APR, REV, ALT, VS, IAS, AP Master).
#[test]
fn bravo_autopilot_mode_buttons_all_eight() {
    // Bits 0–7 correspond to HDG, NAV, APR, REV, ALT, VS, IAS, AP Master
    let ap_names = ["HDG", "NAV", "APR", "REV", "ALT", "VS", "IAS", "AP Master"];
    for (i, name) in ap_names.iter().enumerate() {
        let mask: u64 = 1 << i;
        let state = parse_bravo_report(&bravo_report([0; 7], mask)).unwrap();
        assert!(
            state.buttons.is_pressed((i + 1) as u8),
            "{name} (button {}) should be pressed",
            i + 1
        );
    }
    // All pressed simultaneously
    let all_ap: u64 = 0xFF;
    let state = parse_bravo_report(&bravo_report([0; 7], all_ap)).unwrap();
    assert!(state.buttons.ap_master(), "AP master active with all AP buttons");
    for n in 1u8..=8 {
        assert!(state.buttons.is_pressed(n), "button {n} active");
    }
}

/// Reverse zone detection buttons (bits 23–27 and 32).
#[test]
fn bravo_reverse_zone_buttons() {
    // Reverse zone buttons: bits 23–27 (throttle 1–5 reverse zones)
    for bit in 23u32..=27 {
        let mask: u64 = 1 << bit;
        let state = parse_bravo_report(&bravo_report([0; 7], mask)).unwrap();
        assert!(
            state.buttons.is_pressed((bit + 1) as u8),
            "reverse zone bit {bit} should activate button {}",
            bit + 1
        );
    }
}

/// All 7 toggle switches should decode independently.
#[test]
fn bravo_toggle_switches_all_seven() {
    for sw in 1u8..=7 {
        let base_bit = 33 + (sw - 1) as u32 * 2;
        let up_mask: u64 = 1 << base_bit;
        let down_mask: u64 = 1 << (base_bit + 1);

        assert_eq!(
            decode_toggle_switch(up_mask, sw),
            ToggleSwitchState::Up,
            "switch {sw} up"
        );
        assert_eq!(
            decode_toggle_switch(down_mask, sw),
            ToggleSwitchState::Down,
            "switch {sw} down"
        );
        assert_eq!(
            decode_toggle_switch(0, sw),
            ToggleSwitchState::Center,
            "switch {sw} center"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Annunciator Panel Tests (6)
// ═══════════════════════════════════════════════════════════════════════════════

/// Master Warning LED occupies bit 6 of byte 2.
#[test]
fn annunciator_master_warning_led() {
    let mut leds = BravoLedState::all_off();
    leds.master_warning = true;
    let report = serialize_led_report(&leds);
    assert_eq!(report[2] & (1 << 6), 1 << 6, "master warning = bit 6 of byte 2");
    // No other bits in byte 2 should be set
    assert_eq!(report[2] & !(1 << 6), 0);
    assert_eq!(report[1], 0);
    assert_eq!(report[3], 0);
    assert_eq!(report[4], 0);
}

/// Master Caution LED occupies bit 5 of byte 3.
#[test]
fn annunciator_master_caution_led() {
    let mut leds = BravoLedState::all_off();
    leds.master_caution = true;
    let report = serialize_led_report(&leds);
    assert_eq!(
        report[3] & (1 << 5),
        1 << 5,
        "master caution = bit 5 of byte 3"
    );
    assert_eq!(report[3] & !(1 << 5), 0);
}

/// Engine Fire LED occupies bit 7 of byte 2.
#[test]
fn annunciator_engine_fire_led() {
    let mut leds = BravoLedState::all_off();
    leds.engine_fire = true;
    let report = serialize_led_report(&leds);
    assert_eq!(report[2] & (1 << 7), 1 << 7, "engine fire = bit 7 of byte 2");
    assert_eq!(report[2] & !(1 << 7), 0);
}

/// Each annunciator LED should independently set exactly its expected bit.
#[test]
fn annunciator_individual_leds_byte3() {
    let fields_byte3: [(fn(&mut BravoLedState), u8); 8] = [
        (|l| l.low_oil_pressure = true, 0),
        (|l| l.low_fuel_pressure = true, 1),
        (|l| l.anti_ice = true, 2),
        (|l| l.starter_engaged = true, 3),
        (|l| l.apu = true, 4),
        (|l| l.master_caution = true, 5),
        (|l| l.vacuum = true, 6),
        (|l| l.low_hyd_pressure = true, 7),
    ];
    for (setter, bit) in fields_byte3 {
        let mut leds = BravoLedState::all_off();
        setter(&mut leds);
        let report = serialize_led_report(&leds);
        assert_eq!(
            report[3],
            1 << bit,
            "annunciator byte3 bit {bit} should be exclusively set"
        );
    }
}

/// Byte 4 annunciator LEDs: aux fuel pump, parking brake, low volts, door.
#[test]
fn annunciator_individual_leds_byte4() {
    let fields_byte4: [(fn(&mut BravoLedState), u8); 4] = [
        (|l| l.aux_fuel_pump = true, 0),
        (|l| l.parking_brake = true, 1),
        (|l| l.low_volts = true, 2),
        (|l| l.door = true, 3),
    ];
    for (setter, bit) in fields_byte4 {
        let mut leds = BravoLedState::all_off();
        setter(&mut leds);
        let report = serialize_led_report(&leds);
        assert_eq!(
            report[4],
            1 << bit,
            "annunciator byte4 bit {bit} should be exclusively set"
        );
        // High nibble must remain 0
        assert_eq!(report[4] & 0xF0, 0);
    }
}

/// Simulated flash pattern: toggling a warning LED between on/off states
/// produces alternating report bytes (brightness simulation via toggle).
#[test]
fn annunciator_flash_pattern_toggle() {
    let mut leds = BravoLedState::all_off();

    // Frame 1: warning ON
    leds.master_warning = true;
    leds.engine_fire = true;
    let on_report = serialize_led_report(&leds);
    assert_eq!(on_report[2] & 0xC0, 0xC0, "both warning + fire on");

    // Frame 2: warning OFF (flash off-phase)
    leds.master_warning = false;
    leds.engine_fire = false;
    let off_report = serialize_led_report(&leds);
    assert_eq!(off_report[2] & 0xC0, 0x00, "both warning + fire off");

    // Reports should differ in byte 2
    assert_ne!(on_report[2], off_report[2]);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Calibration Tests (4)
// ═══════════════════════════════════════════════════════════════════════════════

/// Centre detection: raw value 2048 (12-bit midpoint) maps to exactly 0.0
/// for bipolar axes.
#[test]
fn calibration_center_detection_alpha() {
    let state = parse_alpha_report(&alpha_report(2048, 2048, 0, 15)).unwrap();
    assert!(
        state.axes.roll.abs() < 1e-6,
        "roll centre should be exactly 0.0, got {}",
        state.axes.roll
    );
    assert!(
        state.axes.pitch.abs() < 1e-6,
        "pitch centre should be exactly 0.0, got {}",
        state.axes.pitch
    );
}

/// Range mapping: verify that the full 12-bit range [0, 4095] maps to the
/// expected normalised range for both bipolar and unipolar axes.
#[test]
fn calibration_range_mapping_12bit() {
    // Bipolar: 0 → -1.0, 2048 → 0.0, 4095 → +1.0
    let low = parse_alpha_report(&alpha_report(0, 2048, 0, 15)).unwrap();
    let mid = parse_alpha_report(&alpha_report(2048, 2048, 0, 15)).unwrap();
    let high = parse_alpha_report(&alpha_report(4095, 2048, 0, 15)).unwrap();
    assert!(low.axes.roll <= -1.0);
    assert!(mid.axes.roll.abs() < 1e-6);
    assert!(high.axes.roll > 0.999);

    // Unipolar (Bravo): 0 → 0.0, 4095 → 1.0
    let bravo_low = parse_bravo_report(&bravo_report([0; 7], 0)).unwrap();
    let bravo_high = parse_bravo_report(&bravo_report([4095; 7], 0)).unwrap();
    assert!(bravo_low.axes.throttle1 < 0.001);
    assert!((bravo_high.axes.throttle1 - 1.0).abs() < 1e-4);
}

/// Linearity check: verify that evenly-spaced raw inputs produce
/// approximately evenly-spaced normalised outputs (bipolar axis).
#[test]
fn calibration_linearity_bipolar() {
    let samples: Vec<u16> = (0..=10).map(|i| (i * 4095) / 10).collect();
    let normalised: Vec<f32> = samples
        .iter()
        .map(|&raw| {
            parse_alpha_report(&alpha_report(raw, 2048, 0, 15))
                .unwrap()
                .axes
                .roll
        })
        .collect();

    // Check that differences between adjacent samples are roughly equal
    let deltas: Vec<f32> = normalised.windows(2).map(|w| w[1] - w[0]).collect();
    let mean_delta: f32 = deltas.iter().sum::<f32>() / deltas.len() as f32;
    for (i, &d) in deltas.iter().enumerate() {
        assert!(
            (d - mean_delta).abs() < 0.02,
            "step {i}: delta={d:.4} vs mean={mean_delta:.4} — deviation too large"
        );
    }
}

/// Linearity check for unipolar axis (Bravo throttle).
#[test]
fn calibration_linearity_unipolar() {
    let samples: Vec<u16> = (0..=10).map(|i| (i * 4095) / 10).collect();
    let normalised: Vec<f32> = samples
        .iter()
        .map(|&raw| {
            parse_bravo_report(&bravo_report([raw, 0, 0, 0, 0, 0, 0], 0))
                .unwrap()
                .axes
                .throttle1
        })
        .collect();

    let deltas: Vec<f32> = normalised.windows(2).map(|w| w[1] - w[0]).collect();
    let mean_delta: f32 = deltas.iter().sum::<f32>() / deltas.len() as f32;
    for (i, &d) in deltas.iter().enumerate() {
        assert!(
            (d - mean_delta).abs() < 0.02,
            "step {i}: delta={d:.4} vs mean={mean_delta:.4} — deviation too large"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Profile Generation Tests (4)
// ═══════════════════════════════════════════════════════════════════════════════

/// GA (General Aviation) profile: Alpha Yoke should have 2 bipolar axes,
/// a magneto switch, and appropriate deadzone/expo presets.
#[test]
fn profile_ga_alpha_defaults() {
    assert_eq!(ALPHA_PROFILE.axes.len(), 2, "Alpha has 2 axes");
    assert!(ALPHA_PROFILE.has_magneto, "Alpha has magneto switch");
    assert!(!ALPHA_PROFILE.has_leds, "Alpha has no LEDs");

    // Both axes should be bipolar (centred yoke)
    for axis in ALPHA_PROFILE.axes {
        assert!(axis.bipolar, "{} should be bipolar", axis.name);
        assert!(axis.deadzone > 0.0, "{} should have non-zero deadzone", axis.name);
        assert!(axis.expo > 0.0, "{} should have non-zero expo for GA", axis.name);
    }

    // Verify sim var hints
    assert_eq!(ALPHA_PROFILE.axes[0].sim_var_hint, "AILERON");
    assert_eq!(ALPHA_PROFILE.axes[1].sim_var_hint, "ELEVATOR");
}

/// Airliner profile: Bravo should have 6 unipolar axes, LEDs, and encoders.
#[test]
fn profile_airliner_bravo_defaults() {
    assert_eq!(BRAVO_PROFILE.axes.len(), 6, "Bravo has 6 axes");
    assert!(BRAVO_PROFILE.has_leds, "Bravo has LEDs");
    assert!(BRAVO_PROFILE.has_encoders, "Bravo has encoders");
    assert!(!BRAVO_PROFILE.has_magneto, "Bravo has no magneto");

    // All Bravo axes should be unipolar (throttle levers)
    for axis in BRAVO_PROFILE.axes {
        assert!(!axis.bipolar, "{} should be unipolar", axis.name);
    }

    // Verify key sim var hints for airliner config
    assert_eq!(BRAVO_PROFILE.axes[0].sim_var_hint, "THROTTLE 1");
    assert_eq!(BRAVO_PROFILE.axes[1].sim_var_hint, "THROTTLE 2");
}

/// Military configuration: verify axis names and indices are consistent.
#[test]
fn profile_military_axis_config() {
    // Verify that BRAVO_AXES indices are sequential and unique
    let indices: Vec<u8> = BRAVO_AXES.iter().map(|a| a.index).collect();
    for i in 0..indices.len() {
        for j in (i + 1)..indices.len() {
            assert_ne!(
                indices[i], indices[j],
                "duplicate axis index {} at positions {i} and {j}",
                indices[i]
            );
        }
    }

    // Alpha axes should also have unique indices
    let alpha_indices: Vec<u8> = ALPHA_AXES.iter().map(|a| a.index).collect();
    assert_eq!(alpha_indices, vec![0, 1]);
}

/// All profiles should reference the correct VID/PID.
#[test]
fn profile_vendor_product_ids() {
    let all = [&ALPHA_PROFILE, &BRAVO_PROFILE, &CHARLIE_PROFILE];
    for profile in all {
        assert_eq!(
            profile.vendor_id, HONEYCOMB_VENDOR_ID,
            "{} vendor ID mismatch",
            profile.name
        );
    }
    assert_eq!(ALPHA_PROFILE.product_id, HONEYCOMB_ALPHA_YOKE_PID);
    assert_eq!(BRAVO_PROFILE.product_id, HONEYCOMB_BRAVO_PID);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Property Tests (4)
// ═══════════════════════════════════════════════════════════════════════════════

mod proptest_depth {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Bipolar axis normalisation always stays within [-1.0, 1.0] for
        /// any 16-bit raw input (even values > 4095 are clamped).
        #[test]
        fn axis_normalization_bipolar_bounds(
            roll in 0u16..=u16::MAX,
            pitch in 0u16..=u16::MAX,
        ) {
            let state = parse_alpha_report(&super::alpha_report(roll, pitch, 0, 15)).unwrap();
            prop_assert!((-1.001..=1.001).contains(&state.axes.roll),
                "roll={} from raw={roll}", state.axes.roll);
            prop_assert!((-1.001..=1.001).contains(&state.axes.pitch),
                "pitch={} from raw={pitch}", state.axes.pitch);
            prop_assert!(state.axes.roll.is_finite());
            prop_assert!(state.axes.pitch.is_finite());
        }

        /// Unipolar axis normalisation always stays within [0.0, 1.0] for
        /// any 16-bit raw input.
        #[test]
        fn axis_normalization_unipolar_bounds(
            t1 in 0u16..=u16::MAX,
            t2 in 0u16..=u16::MAX,
            t3 in 0u16..=u16::MAX,
            flap in 0u16..=u16::MAX,
        ) {
            let state = parse_bravo_report(
                &super::bravo_report([t1, t2, t3, 0, 0, flap, 0], 0)
            ).unwrap();
            prop_assert!((0.0..=1.001).contains(&state.axes.throttle1));
            prop_assert!((0.0..=1.001).contains(&state.axes.throttle2));
            prop_assert!((0.0..=1.001).contains(&state.axes.throttle3));
            prop_assert!((0.0..=1.001).contains(&state.axes.flap_lever));
        }

        /// Button count: is_pressed(n) for n in 1–64 always returns a
        /// deterministic bool for any button mask.
        #[test]
        fn button_count_within_spec(mask in proptest::bits::u64::ANY) {
            let state = parse_bravo_report(&super::bravo_report([0; 7], mask)).unwrap();
            // Count pressed buttons — should match popcount of lower 64 bits
            let mut count = 0u32;
            for n in 1u8..=64 {
                if state.buttons.is_pressed(n) {
                    count += 1;
                }
            }
            // Count should equal the number of set bits in the mask
            let popcount = mask.count_ones();
            prop_assert_eq!(count, popcount);
        }

        /// LED report bytes are always 5 bytes with report ID 0x00, and
        /// byte 4 high nibble is always 0.
        #[test]
        fn led_indices_valid(
            ap in 0u8..=0xFF,
            gear in 0u8..=0xFF,
            ann1 in 0u8..=0xFF,
            ann2 in 0u8..=0x0F,
        ) {
            // Construct a LED state from arbitrary bits and verify serialization
            let mut leds = BravoLedState::all_off();
            leds.hdg = ap & 1 != 0;
            leds.nav = ap & 2 != 0;
            leds.apr = ap & 4 != 0;
            leds.rev = ap & 8 != 0;
            leds.alt = ap & 16 != 0;
            leds.vs = ap & 32 != 0;
            leds.ias = ap & 64 != 0;
            leds.autopilot = ap & 128 != 0;

            leds.gear_l_green = gear & 1 != 0;
            leds.gear_l_red = gear & 2 != 0;
            leds.gear_c_green = gear & 4 != 0;
            leds.gear_c_red = gear & 8 != 0;
            leds.gear_r_green = gear & 16 != 0;
            leds.gear_r_red = gear & 32 != 0;
            leds.master_warning = gear & 64 != 0;
            leds.engine_fire = gear & 128 != 0;

            leds.low_oil_pressure = ann1 & 1 != 0;
            leds.low_fuel_pressure = ann1 & 2 != 0;
            leds.anti_ice = ann1 & 4 != 0;
            leds.starter_engaged = ann1 & 8 != 0;
            leds.apu = ann1 & 16 != 0;
            leds.master_caution = ann1 & 32 != 0;
            leds.vacuum = ann1 & 64 != 0;
            leds.low_hyd_pressure = ann1 & 128 != 0;

            leds.aux_fuel_pump = ann2 & 1 != 0;
            leds.parking_brake = ann2 & 2 != 0;
            leds.low_volts = ann2 & 4 != 0;
            leds.door = ann2 & 8 != 0;

            let report = serialize_led_report(&leds);
            prop_assert_eq!(report[0], 0x00, "report ID must be 0");
            prop_assert_eq!(report[1], ap, "AP byte mismatch");
            prop_assert_eq!(report[2], gear, "gear byte mismatch");
            prop_assert_eq!(report[3], ann1, "ann1 byte mismatch");
            prop_assert_eq!(report[4], ann2, "ann2 byte mismatch");
            prop_assert_eq!(report[4] & 0xF0, 0, "byte 4 high nibble must be 0");
        }
    }
}
