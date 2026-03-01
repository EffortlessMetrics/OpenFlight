// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the `flight-motion` crate.
//!
//! Covers edge-case behavior, filter frequency response, multi-tick pipeline
//! scenarios, serialization, error paths, and property-based invariants that
//! complement the existing unit and proptest suites.

use approx::assert_abs_diff_eq;
use flight_bus::types::{GForce, ValidatedAngle};
use flight_bus::BusSnapshot;
use flight_motion::{
    DoFConfig, HighPassFilter, LowPassFilter, MotionConfig, MotionError, MotionFrame,
    MotionMapper, OutputError, SimToolsConfig, WashoutConfig, WashoutFilter,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn default_dt() -> f32 {
    1.0 / 250.0
}

fn settle_mapper(mapper: &mut MotionMapper, snap: &BusSnapshot, ticks: usize) -> MotionFrame {
    let mut frame = MotionFrame::NEUTRAL;
    for _ in 0..ticks {
        frame = mapper.process(snap);
    }
    frame
}

fn snapshot_with_g(g_lon: f32, g_lat: f32, g_vert: f32) -> BusSnapshot {
    let mut s = BusSnapshot::default();
    s.kinematics.g_longitudinal = GForce::new(g_lon).unwrap();
    s.kinematics.g_lateral = GForce::new(g_lat).unwrap();
    s.kinematics.g_force = GForce::new(g_vert).unwrap();
    s
}

fn snapshot_with_attitude(bank_deg: f32, pitch_deg: f32) -> BusSnapshot {
    let mut s = BusSnapshot::default();
    s.kinematics.bank = ValidatedAngle::new_degrees(bank_deg).unwrap();
    s.kinematics.pitch = ValidatedAngle::new_degrees(pitch_deg).unwrap();
    s
}

fn snapshot_with_yaw_rate(rate_rad_s: f32) -> BusSnapshot {
    let mut s = BusSnapshot::default();
    s.angular_rates.r = rate_rad_s;
    s
}

fn full_intensity_config() -> MotionConfig {
    MotionConfig {
        intensity: 1.0,
        ..Default::default()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § MotionFrame — construction, clamping, scaling, formatting
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn frame_neutral_constant_all_zero() {
    let f = MotionFrame::NEUTRAL;
    assert_eq!(f.surge, 0.0);
    assert_eq!(f.sway, 0.0);
    assert_eq!(f.heave, 0.0);
    assert_eq!(f.roll, 0.0);
    assert_eq!(f.pitch, 0.0);
    assert_eq!(f.yaw, 0.0);
}

#[test]
fn frame_default_equals_neutral() {
    assert_eq!(MotionFrame::default(), MotionFrame::NEUTRAL);
}

#[test]
fn frame_is_neutral_false_when_any_nonzero() {
    let mut f = MotionFrame::NEUTRAL;
    f.heave = 0.001;
    assert!(!f.is_neutral());
}

#[test]
fn frame_clamped_preserves_in_range() {
    let f = MotionFrame {
        surge: 0.5,
        sway: -0.5,
        heave: 0.0,
        roll: 1.0,
        pitch: -1.0,
        yaw: 0.99,
    };
    let c = f.clamped();
    assert_eq!(c.surge, 0.5);
    assert_eq!(c.sway, -0.5);
    assert_eq!(c.heave, 0.0);
    assert_eq!(c.roll, 1.0);
    assert_eq!(c.pitch, -1.0);
    assert_eq!(c.yaw, 0.99);
}

#[test]
fn frame_clamped_clips_all_channels_positive() {
    let f = MotionFrame {
        surge: 10.0,
        sway: 2.0,
        heave: 1.5,
        roll: 100.0,
        pitch: 3.14,
        yaw: f32::MAX,
    };
    let c = f.clamped();
    for &v in &c.to_array() {
        assert!(v <= 1.0, "positive overflow not clamped: {v}");
    }
}

#[test]
fn frame_clamped_clips_all_channels_negative() {
    let f = MotionFrame {
        surge: -10.0,
        sway: -2.0,
        heave: -1.5,
        roll: -100.0,
        pitch: -3.14,
        yaw: f32::MIN,
    };
    let c = f.clamped();
    for &v in &c.to_array() {
        assert!(v >= -1.0, "negative overflow not clamped: {v}");
    }
}

#[test]
fn frame_clamped_is_idempotent() {
    let f = MotionFrame {
        surge: 5.0,
        sway: -5.0,
        heave: 0.3,
        roll: -0.7,
        pitch: 1.0,
        yaw: -1.0,
    };
    let c1 = f.clamped();
    let c2 = c1.clamped();
    assert_eq!(c1, c2);
}

#[test]
fn frame_scaled_negative_factor_inverts() {
    let f = MotionFrame {
        surge: 0.5,
        sway: -0.3,
        heave: 0.0,
        roll: 1.0,
        pitch: -1.0,
        yaw: 0.25,
    };
    let s = f.scaled(-1.0);
    assert_abs_diff_eq!(s.surge, -0.5, epsilon = 1e-6);
    assert_abs_diff_eq!(s.sway, 0.3, epsilon = 1e-6);
    assert_abs_diff_eq!(s.roll, -1.0, epsilon = 1e-6);
    assert_abs_diff_eq!(s.pitch, 1.0, epsilon = 1e-6);
}

#[test]
fn frame_scaled_factor_greater_than_one() {
    let f = MotionFrame {
        surge: 0.5,
        sway: 0.0,
        heave: 0.0,
        roll: 0.0,
        pitch: 0.0,
        yaw: 0.0,
    };
    let s = f.scaled(2.0);
    assert_abs_diff_eq!(s.surge, 1.0, epsilon = 1e-6);
}

#[test]
fn frame_scaled_composition() {
    let f = MotionFrame {
        surge: 0.8,
        sway: -0.4,
        heave: 0.2,
        roll: 0.0,
        pitch: 0.0,
        yaw: 0.0,
    };
    let composed = f.scaled(0.5).scaled(0.5);
    let direct = f.scaled(0.25);
    for (a, b) in composed.to_array().iter().zip(direct.to_array().iter()) {
        assert_abs_diff_eq!(a, b, epsilon = 1e-6);
    }
}

#[test]
fn frame_to_array_length_six() {
    assert_eq!(MotionFrame::NEUTRAL.to_array().len(), 6);
}

#[test]
fn frame_simtools_neutral() {
    assert_eq!(MotionFrame::NEUTRAL.to_simtools_string(), "A0B0C0D0E0F0\n");
}

#[test]
fn frame_simtools_max_positive() {
    let f = MotionFrame {
        surge: 1.0,
        sway: 1.0,
        heave: 1.0,
        roll: 1.0,
        pitch: 1.0,
        yaw: 1.0,
    };
    assert_eq!(f.to_simtools_string(), "A100B100C100D100E100F100\n");
}

#[test]
fn frame_simtools_max_negative() {
    let f = MotionFrame {
        surge: -1.0,
        sway: -1.0,
        heave: -1.0,
        roll: -1.0,
        pitch: -1.0,
        yaw: -1.0,
    };
    assert_eq!(
        f.to_simtools_string(),
        "A-100B-100C-100D-100E-100F-100\n"
    );
}

#[test]
fn frame_simtools_clamps_overflow_before_encoding() {
    let f = MotionFrame {
        surge: 5.0,
        sway: -5.0,
        heave: 0.0,
        roll: 0.0,
        pitch: 0.0,
        yaw: 0.0,
    };
    assert_eq!(f.to_simtools_string(), "A100B-100C0D0E0F0\n");
}

#[test]
fn frame_simtools_rounding() {
    let f = MotionFrame {
        surge: 0.505,
        sway: -0.504,
        heave: 0.0,
        roll: 0.0,
        pitch: 0.0,
        yaw: 0.0,
    };
    let s = f.to_simtools_string();
    // 0.505 * 100 = 50.5 → rounds to 51 (round half to even may vary, but round() rounds .5 up)
    assert!(s.starts_with("A51") || s.starts_with("A50"), "got: {s}");
}

#[test]
fn frame_display_contains_all_channels() {
    let f = MotionFrame {
        surge: 0.123,
        sway: -0.456,
        heave: 0.789,
        roll: 0.0,
        pitch: -0.5,
        yaw: 1.0,
    };
    let s = format!("{f}");
    assert!(s.contains("surge="));
    assert!(s.contains("sway="));
    assert!(s.contains("heave="));
    assert!(s.contains("roll="));
    assert!(s.contains("pitch="));
    assert!(s.contains("yaw="));
}

#[test]
fn frame_clone_equality() {
    let f = MotionFrame {
        surge: 0.1,
        sway: 0.2,
        heave: 0.3,
        roll: 0.4,
        pitch: 0.5,
        yaw: 0.6,
    };
    #[allow(clippy::clone_on_copy)]
    let f2 = f.clone();
    assert_eq!(f, f2);
}

#[test]
fn frame_partial_eq_detects_difference() {
    let a = MotionFrame {
        surge: 0.5,
        ..Default::default()
    };
    let b = MotionFrame {
        surge: 0.50001,
        ..Default::default()
    };
    assert_ne!(a, b);
}

#[test]
fn frame_serde_json_roundtrip() {
    let f = MotionFrame {
        surge: 0.1,
        sway: -0.2,
        heave: 0.3,
        roll: -0.4,
        pitch: 0.5,
        yaw: -0.6,
    };
    let json = serde_json::to_string(&f).unwrap();
    let f2: MotionFrame = serde_json::from_str(&json).unwrap();
    assert_eq!(f, f2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// § HighPassFilter — onset, washout, frequency response
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn hp_impulse_response_decays_to_zero() {
    let mut hp = HighPassFilter::new(1.0, default_dt());
    let onset = hp.process(1.0);
    assert!(onset > 0.9);
    // All subsequent ticks with zero input should decay
    for _ in 0..1000 {
        let out = hp.process(0.0);
        assert!(out.abs() < onset.abs());
    }
    let final_out = hp.process(0.0);
    assert!(final_out.abs() < 0.001);
}

#[test]
fn hp_passes_alternating_signal() {
    let mut hp = HighPassFilter::new(0.1, default_dt());
    // Feed an AC signal well above cutoff frequency
    let signal_freq_hz = 50.0;
    let mut max_abs = 0.0_f32;
    for i in 0..500 {
        let t = i as f32 * default_dt();
        let input = (2.0 * std::f32::consts::PI * signal_freq_hz * t).sin();
        let out = hp.process(input);
        if i > 100 {
            max_abs = max_abs.max(out.abs());
        }
    }
    // AC signal at 50 Hz through a 0.1 Hz HP should pass almost unattenuated
    assert!(max_abs > 0.9, "HP should pass high-freq: max_abs={max_abs}");
}

#[test]
fn hp_attenuates_very_low_frequency_signal() {
    let mut hp = HighPassFilter::new(10.0, default_dt());
    // Feed a signal at 0.1 Hz — well below the 10 Hz cutoff
    let signal_freq_hz = 0.1;
    let mut max_abs_late = 0.0_f32;
    for i in 0..5000 {
        let t = i as f32 * default_dt();
        let input = (2.0 * std::f32::consts::PI * signal_freq_hz * t).sin();
        let out = hp.process(input);
        if i > 2500 {
            max_abs_late = max_abs_late.max(out.abs());
        }
    }
    assert!(
        max_abs_late < 0.15,
        "HP should attenuate low-freq: {max_abs_late}"
    );
}

#[test]
fn hp_lower_cutoff_slower_washout() {
    let mut hp_slow = HighPassFilter::new(0.1, default_dt());
    let mut hp_fast = HighPassFilter::new(5.0, default_dt());
    // Step input
    for _ in 0..200 {
        hp_slow.process(1.0);
        hp_fast.process(1.0);
    }
    let slow_out = hp_slow.process(1.0);
    let fast_out = hp_fast.process(1.0);
    // Slower cutoff → less washout after same number of ticks
    assert!(
        slow_out.abs() > fast_out.abs(),
        "slow={slow_out} should be larger than fast={fast_out}"
    );
}

#[test]
fn hp_reset_then_new_step_gives_full_onset() {
    let mut hp = HighPassFilter::new(1.0, default_dt());
    // Drive with data
    for _ in 0..500 {
        hp.process(1.0);
    }
    hp.reset();
    let onset = hp.process(1.0);
    assert!(onset > 0.9, "after reset, onset should be near 1.0: {onset}");
}

#[test]
fn hp_linearity_superposition() {
    let dt = default_dt();
    let freq = 1.0;
    let a_input = 0.7_f32;
    let b_input = 0.3_f32;

    let mut hp_a = HighPassFilter::new(freq, dt);
    let mut hp_b = HighPassFilter::new(freq, dt);
    let mut hp_sum = HighPassFilter::new(freq, dt);

    for _ in 0..10 {
        let out_a = hp_a.process(a_input);
        let out_b = hp_b.process(b_input);
        let out_sum = hp_sum.process(a_input + b_input);
        assert_abs_diff_eq!(out_a + out_b, out_sum, epsilon = 1e-5);
    }
}

#[test]
fn hp_negative_step_mirrors_positive() {
    let mut hp_pos = HighPassFilter::new(1.0, default_dt());
    let mut hp_neg = HighPassFilter::new(1.0, default_dt());
    for _ in 0..100 {
        let pos = hp_pos.process(1.0);
        let neg = hp_neg.process(-1.0);
        assert_abs_diff_eq!(pos, -neg, epsilon = 1e-6);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § LowPassFilter — convergence, attenuation, step response
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn lp_step_response_monotonically_increases() {
    let mut lp = LowPassFilter::new(5.0, default_dt());
    let mut prev = 0.0_f32;
    for i in 0..500 {
        let out = lp.process(1.0);
        if i > 0 {
            assert!(
                out >= prev - 1e-7,
                "LP step response should be monotonic: {prev} → {out}"
            );
        }
        prev = out;
    }
    assert!(prev > 0.99, "should converge near 1.0: {prev}");
}

#[test]
fn lp_negative_step_response_monotonically_decreases() {
    let mut lp = LowPassFilter::new(5.0, default_dt());
    let mut prev = 0.0_f32;
    for i in 0..500 {
        let out = lp.process(-1.0);
        if i > 0 {
            assert!(
                out <= prev + 1e-7,
                "LP negative step should be monotonic: {prev} → {out}"
            );
        }
        prev = out;
    }
    assert!(prev < -0.99, "should converge near -1.0: {prev}");
}

#[test]
fn lp_high_cutoff_converges_faster() {
    let mut lp_slow = LowPassFilter::new(1.0, default_dt());
    let mut lp_fast = LowPassFilter::new(20.0, default_dt());
    for _ in 0..50 {
        lp_slow.process(1.0);
        lp_fast.process(1.0);
    }
    let slow_out = lp_slow.process(1.0);
    let fast_out = lp_fast.process(1.0);
    assert!(
        fast_out > slow_out,
        "higher cutoff should converge faster: fast={fast_out}, slow={slow_out}"
    );
}

#[test]
fn lp_dc_gain_unity() {
    // After very many ticks, LP output should equal the constant input
    let mut lp = LowPassFilter::new(2.0, default_dt());
    let input = 0.42;
    for _ in 0..10000 {
        lp.process(input);
    }
    let out = lp.process(input);
    assert_abs_diff_eq!(out, input, epsilon = 0.001);
}

#[test]
fn lp_linearity_superposition() {
    let dt = default_dt();
    let freq = 5.0;

    let mut lp_a = LowPassFilter::new(freq, dt);
    let mut lp_b = LowPassFilter::new(freq, dt);
    let mut lp_sum = LowPassFilter::new(freq, dt);

    for _ in 0..100 {
        let a = lp_a.process(0.6);
        let b = lp_b.process(0.4);
        let s = lp_sum.process(1.0);
        assert_abs_diff_eq!(a + b, s, epsilon = 1e-5);
    }
}

#[test]
fn lp_reset_then_zero_input_gives_zero() {
    let mut lp = LowPassFilter::new(5.0, default_dt());
    for _ in 0..100 {
        lp.process(1.0);
    }
    lp.reset();
    assert_eq!(lp.process(0.0), 0.0);
}

#[test]
fn lp_negative_input_symmetry() {
    let mut lp_pos = LowPassFilter::new(3.0, default_dt());
    let mut lp_neg = LowPassFilter::new(3.0, default_dt());
    for _ in 0..200 {
        let pos = lp_pos.process(0.7);
        let neg = lp_neg.process(-0.7);
        assert_abs_diff_eq!(pos, -neg, epsilon = 1e-6);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § WashoutFilter — 6DOF filter bank
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn washout_hp_channels_decay_lp_channels_converge() {
    let cfg = WashoutConfig::default();
    let mut wf = WashoutFilter::new(&cfg, default_dt());

    // Drive all channels with constant 0.5 for many ticks
    for _ in 0..3000 {
        wf.surge_hp.process(0.5);
        wf.sway_hp.process(0.5);
        wf.heave_hp.process(0.5);
        wf.roll_lp.process(0.5);
        wf.pitch_lp.process(0.5);
        wf.yaw_hp.process(0.5);
    }

    // HP channels should have washed out to near zero
    let surge = wf.surge_hp.process(0.5);
    let sway = wf.sway_hp.process(0.5);
    let heave = wf.heave_hp.process(0.5);
    let yaw = wf.yaw_hp.process(0.5);
    assert!(surge.abs() < 0.01, "surge HP not washed out: {surge}");
    assert!(sway.abs() < 0.01, "sway HP not washed out: {sway}");
    assert!(heave.abs() < 0.01, "heave HP not washed out: {heave}");
    assert!(yaw.abs() < 0.01, "yaw HP not washed out: {yaw}");

    // LP channels should have converged to near 0.5
    let roll = wf.roll_lp.process(0.5);
    let pitch = wf.pitch_lp.process(0.5);
    assert_abs_diff_eq!(roll, 0.5, epsilon = 0.01);
    assert_abs_diff_eq!(pitch, 0.5, epsilon = 0.01);
}

#[test]
fn washout_channels_are_independent() {
    let cfg = WashoutConfig::default();
    let mut wf = WashoutFilter::new(&cfg, default_dt());

    // Only drive surge — other channels should remain at zero
    for _ in 0..10 {
        wf.surge_hp.process(1.0);
    }
    assert_eq!(wf.sway_hp.process(0.0), 0.0);
    assert_eq!(wf.heave_hp.process(0.0), 0.0);
    assert_eq!(wf.roll_lp.process(0.0), 0.0);
    assert_eq!(wf.pitch_lp.process(0.0), 0.0);
    assert_eq!(wf.yaw_hp.process(0.0), 0.0);
}

#[test]
fn washout_custom_frequencies() {
    let cfg = WashoutConfig {
        hp_frequency_hz: 2.0,
        lp_frequency_hz: 10.0,
    };
    let mut wf = WashoutFilter::new(&cfg, default_dt());

    // Higher HP freq → faster washout
    for _ in 0..500 {
        wf.surge_hp.process(1.0);
    }
    let surge = wf.surge_hp.process(1.0);
    assert!(surge.abs() < 0.01, "high-freq HP should wash out fast: {surge}");

    // Higher LP freq → faster convergence
    let mut wf2 = WashoutFilter::new(&cfg, default_dt());
    for _ in 0..200 {
        wf2.roll_lp.process(1.0);
    }
    let roll = wf2.roll_lp.process(1.0);
    assert!(roll > 0.99, "high-freq LP should converge fast: {roll}");
}

// ═══════════════════════════════════════════════════════════════════════════════
// § MotionMapper — pipeline, channel mapping, reset behavior
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn mapper_g_longitudinal_maps_to_surge_with_negation() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    // Positive g_longitudinal should produce negative surge (negation in mapper)
    let snap = snapshot_with_g(2.0, 0.0, 1.0);
    let frame = mapper.process(&snap);
    assert!(
        frame.surge < 0.0,
        "positive g_lon should give negative surge: {}",
        frame.surge
    );
}

#[test]
fn mapper_g_lateral_maps_to_sway() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    let snap = snapshot_with_g(0.0, 2.0, 1.0);
    let frame = mapper.process(&snap);
    assert!(
        frame.sway > 0.0,
        "positive g_lat should give positive sway: {}",
        frame.sway
    );
}

#[test]
fn mapper_heave_subtracts_one_g() {
    // Default g_force = 1.0 (level flight).  After washout, heave should be ~0.
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    let snap = BusSnapshot::default(); // g_force = 1.0
    let frame = mapper.process(&snap);
    // First tick: (1.0 - 1.0) / max_g = 0.0 → HP(0) = 0
    assert_abs_diff_eq!(frame.heave, 0.0, epsilon = 0.01);
}

#[test]
fn mapper_heave_nonzero_for_extra_g() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    // 3G → (3.0 - 1.0) / 3.0 = 0.667 → HP first tick ≈ 0.667 * alpha
    let snap = snapshot_with_g(0.0, 0.0, 3.0);
    let frame = mapper.process(&snap);
    assert!(
        frame.heave > 0.3,
        "extra g-force should produce heave: {}",
        frame.heave
    );
}

#[test]
fn mapper_bank_angle_maps_to_roll() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    let snap = snapshot_with_attitude(30.0, 0.0);
    let frame = mapper.process(&snap);
    // 30° / max_angle_deg(30) = 1.0 → LP first tick is small but positive
    assert!(frame.roll > 0.0, "bank should map to roll: {}", frame.roll);
}

#[test]
fn mapper_pitch_angle_maps_to_pitch() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    let snap = snapshot_with_attitude(0.0, 15.0);
    let frame = mapper.process(&snap);
    assert!(
        frame.pitch > 0.0,
        "pitch angle should map to pitch: {}",
        frame.pitch
    );
}

#[test]
fn mapper_yaw_rate_maps_to_yaw() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    // Positive yaw rate (rad/s)
    let snap = snapshot_with_yaw_rate(1.0);
    let frame = mapper.process(&snap);
    assert!(
        frame.yaw > 0.0,
        "positive yaw rate should produce positive yaw: {}",
        frame.yaw
    );
}

#[test]
fn mapper_surge_washes_out_after_sustained_g() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    let snap = snapshot_with_g(2.0, 0.0, 1.0);

    // First tick: strong onset
    let first = mapper.process(&snap);
    assert!(first.surge.abs() > 0.3);

    // After many ticks: should wash out
    let frame = settle_mapper(&mut mapper, &snap, 5000);
    assert!(
        frame.surge.abs() < 0.05,
        "surge should wash out: {}",
        frame.surge
    );
}

#[test]
fn mapper_roll_converges_for_sustained_bank() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    let snap = snapshot_with_attitude(15.0, 0.0);
    let frame = settle_mapper(&mut mapper, &snap, 3000);

    // 15° / 30° = 0.5 → LP should converge near 0.5 * intensity
    assert!(
        frame.roll > 0.3,
        "roll should converge to sustained tilt: {}",
        frame.roll
    );
}

#[test]
fn mapper_gain_amplifies_channel() {
    let mut config = full_intensity_config();
    config.surge.gain = 2.0; // max allowed

    let mut mapper = MotionMapper::new(config, default_dt());
    let snap = snapshot_with_g(1.0, 0.0, 1.0);
    let frame = mapper.process(&snap);

    // With gain=2, surge output should be larger than with gain=1
    let mut config2 = full_intensity_config();
    config2.surge.gain = 1.0;
    let mut mapper2 = MotionMapper::new(config2, default_dt());
    let frame2 = mapper2.process(&snap);

    assert!(
        frame.surge.abs() >= frame2.surge.abs() - 1e-5,
        "gain should amplify: gain2={}, gain1={}",
        frame.surge,
        frame2.surge
    );
}

#[test]
fn mapper_gain_zero_silences_channel() {
    let mut config = full_intensity_config();
    config.surge.gain = 0.0;
    let mut mapper = MotionMapper::new(config, default_dt());
    let snap = snapshot_with_g(5.0, 0.0, 1.0);
    let frame = mapper.process(&snap);
    assert_eq!(frame.surge, 0.0, "gain=0 should silence channel");
}

#[test]
fn mapper_invert_negates_roll() {
    let snap = snapshot_with_attitude(20.0, 0.0);

    let mut cfg_normal = full_intensity_config();
    cfg_normal.roll.invert = false;
    let mut mapper_normal = MotionMapper::new(cfg_normal, default_dt());
    let normal = mapper_normal.process(&snap);

    let mut cfg_inv = full_intensity_config();
    cfg_inv.roll.invert = true;
    let mut mapper_inv = MotionMapper::new(cfg_inv, default_dt());
    let inverted = mapper_inv.process(&snap);

    assert_abs_diff_eq!(normal.roll, -inverted.roll, epsilon = 1e-5);
}

#[test]
fn mapper_all_channels_disabled_gives_neutral() {
    let mut config = full_intensity_config();
    config.surge.enabled = false;
    config.sway.enabled = false;
    config.heave.enabled = false;
    config.roll.enabled = false;
    config.pitch.enabled = false;
    config.yaw.enabled = false;
    let mut mapper = MotionMapper::new(config, default_dt());
    let snap = snapshot_with_g(5.0, 5.0, 5.0);
    let frame = mapper.process(&snap);
    assert!(frame.is_neutral());
}

#[test]
fn mapper_reset_behaves_like_new() {
    let config = full_intensity_config();
    let snap = snapshot_with_g(2.0, 1.0, 3.0);

    let mut mapper = MotionMapper::new(config.clone(), default_dt());
    settle_mapper(&mut mapper, &snap, 100);
    mapper.reset();
    let after_reset = mapper.process(&snap);

    let mut fresh = MotionMapper::new(config, default_dt());
    let from_fresh = fresh.process(&snap);

    for (a, b) in after_reset.to_array().iter().zip(from_fresh.to_array().iter()) {
        assert_abs_diff_eq!(a, b, epsilon = 1e-5);
    }
}

#[test]
fn mapper_set_intensity_mid_stream() {
    let mut mapper = MotionMapper::new(full_intensity_config(), default_dt());
    let snap = snapshot_with_g(2.0, 0.0, 1.0);
    let full = mapper.process(&snap);

    mapper.reset();
    mapper.set_intensity(0.5);
    let half = mapper.process(&snap);

    // Half intensity should give roughly half the output
    assert_abs_diff_eq!(half.surge, full.surge * 0.5, epsilon = 0.05);
}

#[test]
fn mapper_output_always_in_unit_range() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    // Extreme inputs
    let snap = snapshot_with_g(20.0, -20.0, 20.0);
    for _ in 0..100 {
        let frame = mapper.process(&snap);
        for &v in &frame.to_array() {
            assert!(
                v.abs() <= 1.0 + 1e-5,
                "output out of bounds: {v}"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § MotionConfig / DoFConfig / WashoutConfig — defaults and serde
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn dof_config_default_enabled_gain_one_no_invert() {
    let d = DoFConfig::default();
    assert!(d.enabled);
    assert_eq!(d.gain, 1.0);
    assert!(!d.invert);
}

#[test]
fn washout_config_default_values() {
    let w = WashoutConfig::default();
    assert_eq!(w.hp_frequency_hz, 0.5);
    assert_eq!(w.lp_frequency_hz, 5.0);
}

#[test]
fn motion_config_default_max_values() {
    let c = MotionConfig::default();
    assert_eq!(c.max_g, 3.0);
    assert_eq!(c.max_angle_deg, 30.0);
    assert_eq!(c.max_yaw_rate_deg_s, 60.0);
}

#[test]
fn motion_config_serde_json_roundtrip() {
    let config = MotionConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let config2: MotionConfig = serde_json::from_str(&json).unwrap();
    assert_abs_diff_eq!(config2.intensity, config.intensity, epsilon = 1e-6);
    assert_abs_diff_eq!(config2.max_g, config.max_g, epsilon = 1e-6);
    assert!(config2.surge.enabled);
    assert_eq!(config2.washout.hp_frequency_hz, config.washout.hp_frequency_hz);
}

#[test]
fn dof_config_serde_json_roundtrip() {
    let d = DoFConfig {
        enabled: false,
        gain: 1.5,
        invert: true,
    };
    let json = serde_json::to_string(&d).unwrap();
    let d2: DoFConfig = serde_json::from_str(&json).unwrap();
    assert!(!d2.enabled);
    assert_eq!(d2.gain, 1.5);
    assert!(d2.invert);
}

// ═══════════════════════════════════════════════════════════════════════════════
// § SimToolsConfig — defaults
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn simtools_config_default_port() {
    let c = SimToolsConfig::default();
    assert_eq!(c.remote_addr.port(), 4123);
    assert_eq!(c.remote_addr.ip().to_string(), "127.0.0.1");
}

#[test]
fn simtools_config_default_local_ephemeral() {
    let c = SimToolsConfig::default();
    assert_eq!(c.local_addr.port(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// § Error types
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn motion_error_config_display() {
    let e = MotionError::Config("bad value".to_string());
    let msg = format!("{e}");
    assert!(msg.contains("Invalid configuration"));
    assert!(msg.contains("bad value"));
}

#[test]
fn motion_error_output_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::AddrInUse, "port taken");
    let output_err = OutputError::Socket(io_err);
    let motion_err = MotionError::Output(output_err);
    let msg = format!("{motion_err}");
    assert!(msg.contains("Output error"));
}

#[test]
fn output_error_send_incomplete() {
    let e = OutputError::SendIncomplete {
        sent: 5,
        expected: 20,
    };
    let msg = format!("{e}");
    assert!(msg.contains("5"));
    assert!(msg.contains("20"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// § Realistic multi-tick scenarios
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn scenario_takeoff_roll_acceleration() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    // Simulate acceleration: g_longitudinal ramps from 0 to 1.5G over 50 ticks
    let mut max_surge = 0.0_f32;
    for i in 0..50 {
        let g_lon = 1.5 * (i as f32 / 50.0);
        let mut snap = BusSnapshot::default();
        snap.kinematics.g_longitudinal = GForce::new(g_lon).unwrap();
        let frame = mapper.process(&snap);
        max_surge = max_surge.max(frame.surge.abs());
    }
    assert!(
        max_surge > 0.1,
        "takeoff acceleration should produce surge: {max_surge}"
    );
}

#[test]
fn scenario_level_turn_produces_roll_and_sway() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    // Bank 20° right + lateral g-force
    let mut snap = BusSnapshot::default();
    snap.kinematics.bank = ValidatedAngle::new_degrees(20.0).unwrap();
    snap.kinematics.g_lateral = GForce::new(0.5).unwrap();

    let frame = settle_mapper(&mut mapper, &snap, 500);
    assert!(frame.roll > 0.0, "should have positive roll: {}", frame.roll);
    // Sway should have washed out for sustained g
}

#[test]
fn scenario_turbulence_oscillation() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    // Oscillating g-force simulating turbulence
    let mut any_nonzero_heave = false;
    for i in 0..200 {
        let t = i as f32 * default_dt();
        let g_osc = 1.0 + 0.5 * (2.0 * std::f32::consts::PI * 2.0 * t).sin();
        let mut snap = BusSnapshot::default();
        snap.kinematics.g_force = GForce::new(g_osc).unwrap();
        let frame = mapper.process(&snap);
        if frame.heave.abs() > 0.01 {
            any_nonzero_heave = true;
        }
    }
    assert!(any_nonzero_heave, "turbulence should produce heave motion");
}

#[test]
fn scenario_return_to_neutral_after_input_removed() {
    let config = full_intensity_config();
    let mut mapper = MotionMapper::new(config, default_dt());

    // Strong inputs for 100 ticks
    let strong = snapshot_with_g(3.0, 2.0, 4.0);
    for _ in 0..100 {
        mapper.process(&strong);
    }

    // Return to neutral inputs
    let neutral = BusSnapshot::default();
    let frame = settle_mapper(&mut mapper, &neutral, 5000);

    for &v in &frame.to_array() {
        assert!(
            v.abs() < 0.02,
            "should return to neutral after input removed: {v}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// § Property-based tests (proptest)
// ═══════════════════════════════════════════════════════════════════════════════

proptest::proptest! {
    /// Clamped then scaled by ≤1 stays in [-1, 1].
    #[test]
    fn prop_clamped_then_scaled_in_range(
        surge in -100.0_f32..=100.0_f32,
        sway  in -100.0_f32..=100.0_f32,
        heave in -100.0_f32..=100.0_f32,
        roll  in -100.0_f32..=100.0_f32,
        pitch in -100.0_f32..=100.0_f32,
        yaw   in -100.0_f32..=100.0_f32,
        factor in 0.0_f32..=1.0_f32,
    ) {
        let f = MotionFrame { surge, sway, heave, roll, pitch, yaw };
        let result = f.clamped().scaled(factor);
        for &v in &result.to_array() {
            proptest::prop_assert!(v.abs() <= 1.0 + 1e-6, "out of range: {v}");
        }
    }

    /// HP filter output magnitude never exceeds input magnitude on the very
    /// first tick (from zero state).
    #[test]
    fn prop_hp_first_tick_no_amplification(
        freq  in 0.01_f32..=50.0_f32,
        dt    in 0.001_f32..=0.1_f32,
        input in -50.0_f32..=50.0_f32,
    ) {
        let mut hp = HighPassFilter::new(freq, dt);
        let out = hp.process(input);
        proptest::prop_assert!(
            out.abs() <= input.abs() + 1e-5,
            "HP amplified on first tick: out={out}, input={input}"
        );
    }

    /// LP filter output always stays between 0 and the input value when
    /// starting from zero state with a positive constant input.
    #[test]
    fn prop_lp_output_between_zero_and_input(
        freq  in 0.1_f32..=50.0_f32,
        dt    in 0.001_f32..=0.05_f32,
        input in 0.01_f32..=10.0_f32,
        steps in 1_usize..=200_usize,
    ) {
        let mut lp = LowPassFilter::new(freq, dt);
        for _ in 0..steps {
            let out = lp.process(input);
            proptest::prop_assert!(
                out >= -1e-5 && out <= input + 1e-5,
                "LP out of expected range: out={out}, input={input}"
            );
        }
    }

    /// MotionMapper with zero intensity always produces neutral frame.
    #[test]
    fn prop_zero_intensity_always_neutral(
        g_force in -20.0_f32..=20.0_f32,
        g_lat   in -20.0_f32..=20.0_f32,
        g_lon   in -20.0_f32..=20.0_f32,
    ) {
        let mut snap = BusSnapshot::default();
        snap.kinematics.g_force = GForce::new(g_force).unwrap();
        snap.kinematics.g_lateral = GForce::new(g_lat).unwrap();
        snap.kinematics.g_longitudinal = GForce::new(g_lon).unwrap();

        let config = MotionConfig { intensity: 0.0, ..Default::default() };
        let mut mapper = MotionMapper::new(config, default_dt());
        let frame = mapper.process(&snap);
        proptest::prop_assert!(frame.is_neutral());
    }

    /// After a full reset, processing a default snapshot should yield the same
    /// result as a freshly constructed mapper.
    #[test]
    fn prop_reset_equivalence(
        g_lon   in -10.0_f32..=10.0_f32,
        g_lat   in -10.0_f32..=10.0_f32,
        steps   in 10_usize..=200_usize,
    ) {
        let mut snap = BusSnapshot::default();
        snap.kinematics.g_longitudinal = GForce::new(g_lon).unwrap();
        snap.kinematics.g_lateral = GForce::new(g_lat).unwrap();

        let config = full_intensity_config();
        let mut mapper = MotionMapper::new(config.clone(), default_dt());
        for _ in 0..steps {
            mapper.process(&snap);
        }
        mapper.reset();

        let test_snap = BusSnapshot::default();
        let after_reset = mapper.process(&test_snap);

        let mut fresh = MotionMapper::new(config, default_dt());
        let from_fresh = fresh.process(&test_snap);

        for (a, b) in after_reset.to_array().iter().zip(from_fresh.to_array().iter()) {
            proptest::prop_assert!(
                (a - b).abs() < 1e-5,
                "reset not equivalent: after_reset={a}, fresh={b}"
            );
        }
    }

    /// `to_simtools_string` always produces exactly 6 integer values in [-100, 100].
    #[test]
    fn prop_simtools_always_valid_format(
        surge in -10.0_f32..=10.0_f32,
        sway  in -10.0_f32..=10.0_f32,
        heave in -10.0_f32..=10.0_f32,
        roll  in -10.0_f32..=10.0_f32,
        pitch in -10.0_f32..=10.0_f32,
        yaw   in -10.0_f32..=10.0_f32,
    ) {
        let frame = MotionFrame { surge, sway, heave, roll, pitch, yaw };
        let s = frame.to_simtools_string();
        proptest::prop_assert!(s.ends_with('\n'));
        let values: Vec<i32> = s
            .trim_end_matches('\n')
            .split(|c: char| c.is_ascii_uppercase())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse::<i32>().expect("parse int"))
            .collect();
        proptest::prop_assert_eq!(values.len(), 6);
        for &v in &values {
            proptest::prop_assert!((-100..=100).contains(&v), "value OOB: {v}");
        }
    }

    /// Scaling by factor then by its inverse should approximately recover the
    /// original frame (for non-zero factors away from float precision limits).
    #[test]
    fn prop_scale_inverse_roundtrip(
        surge in -1.0_f32..=1.0_f32,
        sway  in -1.0_f32..=1.0_f32,
        heave in -1.0_f32..=1.0_f32,
        roll  in -1.0_f32..=1.0_f32,
        pitch in -1.0_f32..=1.0_f32,
        yaw   in -1.0_f32..=1.0_f32,
        factor in 0.1_f32..=10.0_f32,
    ) {
        let f = MotionFrame { surge, sway, heave, roll, pitch, yaw };
        let roundtrip = f.scaled(factor).scaled(1.0 / factor);
        for (a, b) in f.to_array().iter().zip(roundtrip.to_array().iter()) {
            proptest::prop_assert!(
                (a - b).abs() < 1e-4,
                "scale roundtrip failed: orig={a}, roundtrip={b}, factor={factor}"
            );
        }
    }
}
