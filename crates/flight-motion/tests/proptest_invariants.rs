// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Property-based tests for the `flight-motion` crate.
//!
//! Uses `proptest` to verify mathematical invariants of:
//! - [`HighPassFilter`] / [`LowPassFilter`] (filter algebra)
//! - [`WashoutFilter`] (6DOF filter bank)
//! - [`MotionFrame`] (normalised 6DOF value container)
//! - [`MotionMapper`] (BusSnapshot → MotionFrame pipeline)

use flight_bus::BusSnapshot;
use flight_motion::{
    HighPassFilter, LowPassFilter, MotionConfig, MotionFrame, MotionMapper, WashoutConfig,
    WashoutFilter,
};
use proptest::prelude::*;

// ─────────────────────────────────────────────────────────────────────────────
// HighPassFilter
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// On the very first tick from zero state the HP output is `α·input`,
    /// where `α = τ/(τ+dt) ∈ [0, 1)`.  Therefore `|output| ≤ |input|`.
    #[test]
    fn prop_hp_first_tick_bounded_by_input(
        freq  in 0.01_f32..=100.0_f32,
        dt    in 0.0001_f32..=0.1_f32,
        input in -100.0_f32..=100.0_f32,
    ) {
        let mut hp = HighPassFilter::new(freq, dt);
        let out = hp.process(input);
        prop_assert!(
            out.abs() <= input.abs() + 1e-5,
            "HP first-tick output {out} exceeds input magnitude {}",
            input.abs()
        );
    }

    /// Zero input must always produce zero output regardless of filter params.
    #[test]
    fn prop_hp_zero_input_zero_output(
        freq  in 0.01_f32..=100.0_f32,
        dt    in 0.0001_f32..=0.1_f32,
        steps in 1_usize..=50_usize,
    ) {
        let mut hp = HighPassFilter::new(freq, dt);
        for _ in 0..steps {
            let out = hp.process(0.0);
            prop_assert_eq!(out, 0.0, "HP output must be 0 for zero input");
        }
    }

    /// `reset()` restores zero state: immediately after a reset, zero input
    /// must return zero output, regardless of prior filter history.
    #[test]
    fn prop_hp_reset_restores_zero_state(
        freq  in 0.01_f32..=100.0_f32,
        dt    in 0.0001_f32..=0.1_f32,
        input in -10.0_f32..=10.0_f32,
        steps in 1_usize..=100_usize,
    ) {
        let mut hp = HighPassFilter::new(freq, dt);
        for _ in 0..steps {
            hp.process(input);
        }
        hp.reset();
        prop_assert_eq!(hp.process(0.0), 0.0);
    }

    /// **Washout property**: for a constant input the HP output must decay to
    /// near-zero over time — that is the whole point of the washout filter.
    ///
    /// We simulate 6 time constants (τ = 1/(2πf)) worth of ticks, ensuring
    /// ample decay regardless of frequency and timestep.
    #[test]
    fn prop_hp_constant_input_washes_out(
        freq  in 0.5_f32..=100.0_f32,
        dt    in 0.001_f32..=0.01_f32,
        input in -10.0_f32..=10.0_f32,
    ) {
        let tau = 1.0_f32 / (2.0 * std::f32::consts::PI * freq);
        let sim_time = 6.0 * tau;
        let ticks = ((sim_time / dt).ceil() as usize).max(100);
        let mut hp = HighPassFilter::new(freq, dt);
        let mut out = 0.0_f32;
        for _ in 0..ticks {
            out = hp.process(input);
        }
        let threshold = 0.01_f32 * input.abs().max(0.01_f32);
        prop_assert!(
            out.abs() < threshold,
            "HP did not wash out: out={out}, input={input}, threshold={threshold}, ticks={ticks}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LowPassFilter
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// Zero input must always produce zero output (filter starts at zero state).
    #[test]
    fn prop_lp_zero_input_zero_output(
        freq  in 0.01_f32..=100.0_f32,
        dt    in 0.0001_f32..=0.1_f32,
        steps in 1_usize..=50_usize,
    ) {
        let mut lp = LowPassFilter::new(freq, dt);
        for _ in 0..steps {
            let out = lp.process(0.0);
            prop_assert_eq!(out, 0.0);
        }
    }

    /// A LP filter starting at zero cannot amplify a constant input: every tick
    /// the output must be bounded by the input magnitude.
    #[test]
    fn prop_lp_cannot_amplify_constant_input(
        freq  in 0.01_f32..=50.0_f32,
        dt    in 0.0001_f32..=0.1_f32,
        input in -100.0_f32..=100.0_f32,
    ) {
        let mut lp = LowPassFilter::new(freq, dt);
        for _ in 0..100 {
            let out = lp.process(input);
            prop_assert!(
                out.abs() <= input.abs() + 1e-5,
                "LP amplified: out={out}, input={input}"
            );
        }
    }

    /// **DC gain = 1**: for a constant input at moderate frequencies the LP
    /// output must converge to within 2 % of the input after 2 000 ticks.
    ///
    /// Frequency 1–20 Hz, dt 1–10 ms gives at least 5 time constants in 2 000
    /// ticks (worst case: 1 Hz at 10 ms → 20 s / 0.159 s ≈ 126 τ).
    #[test]
    fn prop_lp_constant_input_converges(
        freq  in 1.0_f32..=20.0_f32,
        dt    in 0.001_f32..=0.01_f32,
        input in -10.0_f32..=10.0_f32,
    ) {
        let mut lp = LowPassFilter::new(freq, dt);
        let mut out = 0.0_f32;
        for _ in 0..2000 {
            out = lp.process(input);
        }
        prop_assert!(
            (out - input).abs() < 0.02,
            "LP did not converge: out={out}, input={input}"
        );
    }

    /// `reset()` restores zero state: zero input immediately after reset must
    /// produce zero output.
    #[test]
    fn prop_lp_reset_restores_zero_state(
        freq  in 0.1_f32..=20.0_f32,
        dt    in 0.001_f32..=0.05_f32,
        input in -10.0_f32..=10.0_f32,
        steps in 1_usize..=100_usize,
    ) {
        let mut lp = LowPassFilter::new(freq, dt);
        for _ in 0..steps {
            lp.process(input);
        }
        lp.reset();
        prop_assert_eq!(lp.process(0.0), 0.0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WashoutFilter (6DOF bank)
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// After `WashoutFilter::reset()`, every channel must return 0 for zero
    /// input, regardless of how many ticks of non-zero input preceded the reset.
    #[test]
    fn prop_washout_reset_clears_all_channels(
        hp_freq in 0.1_f32..=5.0_f32,
        lp_freq in 0.5_f32..=20.0_f32,
        dt      in 0.001_f32..=0.05_f32,
        input   in -10.0_f32..=10.0_f32,
        steps   in 1_usize..=200_usize,
    ) {
        let config = WashoutConfig { hp_frequency_hz: hp_freq, lp_frequency_hz: lp_freq };
        let mut wf = WashoutFilter::new(&config, dt);

        for _ in 0..steps {
            wf.surge_hp.process(input);
            wf.sway_hp.process(input);
            wf.heave_hp.process(input);
            wf.roll_lp.process(input);
            wf.pitch_lp.process(input);
            wf.yaw_hp.process(input);
        }

        wf.reset();

        prop_assert_eq!(wf.surge_hp.process(0.0), 0.0, "surge_hp not cleared");
        prop_assert_eq!(wf.sway_hp.process(0.0),  0.0, "sway_hp not cleared");
        prop_assert_eq!(wf.heave_hp.process(0.0), 0.0, "heave_hp not cleared");
        prop_assert_eq!(wf.roll_lp.process(0.0),  0.0, "roll_lp not cleared");
        prop_assert_eq!(wf.pitch_lp.process(0.0), 0.0, "pitch_lp not cleared");
        prop_assert_eq!(wf.yaw_hp.process(0.0),   0.0, "yaw_hp not cleared");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MotionFrame
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// `clamped()` always produces values in `[-1.0, 1.0]` for any finite input.
    #[test]
    fn prop_frame_clamped_always_in_unit_interval(
        surge in -1e30_f32..=1e30_f32,
        sway  in -1e30_f32..=1e30_f32,
        heave in -1e30_f32..=1e30_f32,
        roll  in -1e30_f32..=1e30_f32,
        pitch in -1e30_f32..=1e30_f32,
        yaw   in -1e30_f32..=1e30_f32,
    ) {
        let frame = MotionFrame { surge, sway, heave, roll, pitch, yaw };
        let c = frame.clamped();
        for &v in &[c.surge, c.sway, c.heave, c.roll, c.pitch, c.yaw] {
            prop_assert!((-1.0..=1.0).contains(&v), "clamped value out of range: {v}");
        }
    }

    /// `scaled(0.0)` always yields the neutral (all-zero) frame.
    #[test]
    fn prop_frame_scaled_zero_is_neutral(
        surge in -1.0_f32..=1.0_f32,
        sway  in -1.0_f32..=1.0_f32,
        heave in -1.0_f32..=1.0_f32,
        roll  in -1.0_f32..=1.0_f32,
        pitch in -1.0_f32..=1.0_f32,
        yaw   in -1.0_f32..=1.0_f32,
    ) {
        let frame = MotionFrame { surge, sway, heave, roll, pitch, yaw };
        prop_assert!(frame.scaled(0.0).is_neutral());
    }

    /// `scaled(1.0)` is the identity.
    #[test]
    fn prop_frame_scaled_one_is_identity(
        surge in -1.0_f32..=1.0_f32,
        sway  in -1.0_f32..=1.0_f32,
        heave in -1.0_f32..=1.0_f32,
        roll  in -1.0_f32..=1.0_f32,
        pitch in -1.0_f32..=1.0_f32,
        yaw   in -1.0_f32..=1.0_f32,
    ) {
        let frame = MotionFrame { surge, sway, heave, roll, pitch, yaw };
        prop_assert_eq!(frame.scaled(1.0), frame);
    }

    /// `to_simtools_string()` always produces six integers in `[-100, 100]`
    /// and ends with a newline.
    #[test]
    fn prop_frame_simtools_values_in_range(
        surge in -1e30_f32..=1e30_f32,
        sway  in -1e30_f32..=1e30_f32,
        heave in -1e30_f32..=1e30_f32,
        roll  in -1e30_f32..=1e30_f32,
        pitch in -1e30_f32..=1e30_f32,
        yaw   in -1e30_f32..=1e30_f32,
    ) {
        let frame = MotionFrame { surge, sway, heave, roll, pitch, yaw };
        let s = frame.to_simtools_string();
        prop_assert!(s.ends_with('\n'), "SimTools string must end with newline");
        // Format: "A{i}B{i}C{i}D{i}E{i}F{i}\n"
        // Split on uppercase letters to extract the six integer tokens.
        let values: Vec<i32> = s
            .trim_end_matches('\n')
            .split(|c: char| c.is_ascii_uppercase())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse::<i32>().expect("SimTools value must be a valid integer"))
            .collect();
        prop_assert_eq!(values.len(), 6, "expected 6 SimTools channel values");
        for &v in &values {
            prop_assert!((-100..=100).contains(&v), "SimTools value {v} out of [-100, 100]");
        }
    }

    /// `to_array()` returns channels in `[surge, sway, heave, roll, pitch, yaw]` order.
    #[test]
    fn prop_frame_array_order_matches_fields(
        surge in -1.0_f32..=1.0_f32,
        sway  in -1.0_f32..=1.0_f32,
        heave in -1.0_f32..=1.0_f32,
        roll  in -1.0_f32..=1.0_f32,
        pitch in -1.0_f32..=1.0_f32,
        yaw   in -1.0_f32..=1.0_f32,
    ) {
        let frame = MotionFrame { surge, sway, heave, roll, pitch, yaw };
        let arr = frame.to_array();
        prop_assert_eq!(arr[0], surge);
        prop_assert_eq!(arr[1], sway);
        prop_assert_eq!(arr[2], heave);
        prop_assert_eq!(arr[3], roll);
        prop_assert_eq!(arr[4], pitch);
        prop_assert_eq!(arr[5], yaw);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MotionMapper
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    /// `MotionMapper::process` must always produce finite (non-NaN, non-Inf)
    /// values for any valid input snapshot.
    #[test]
    fn prop_mapper_output_always_finite(
        g_force   in -20.0_f32..=20.0_f32,
        g_lat     in -20.0_f32..=20.0_f32,
        g_lon     in -20.0_f32..=20.0_f32,
        bank_deg  in -180.0_f32..=180.0_f32,
        pitch_deg in -180.0_f32..=180.0_f32,
        yaw_rate  in -10.0_f32..=10.0_f32,
        intensity in 0.0_f32..=1.0_f32,
    ) {
        use flight_bus::types::{GForce, ValidatedAngle};

        let mut snapshot = BusSnapshot::default();
        snapshot.kinematics.g_force        = GForce::new(g_force).unwrap();
        snapshot.kinematics.g_lateral      = GForce::new(g_lat).unwrap();
        snapshot.kinematics.g_longitudinal = GForce::new(g_lon).unwrap();
        snapshot.kinematics.bank           = ValidatedAngle::new_degrees(bank_deg).unwrap();
        snapshot.kinematics.pitch          = ValidatedAngle::new_degrees(pitch_deg).unwrap();
        snapshot.angular_rates.r           = yaw_rate;

        let config = MotionConfig { intensity, ..Default::default() };
        let mut mapper = MotionMapper::new(config, 1.0 / 60.0);

        let frame = mapper.process(&snapshot);
        for &v in &frame.to_array() {
            prop_assert!(v.is_finite(), "mapper produced non-finite value: {v}");
        }
    }

    /// Every output channel is bounded by `[-intensity, +intensity]`, because
    /// `apply()` clamps each pre-scaled channel to `[-1, 1]` and the frame is
    /// subsequently scaled by `intensity`.
    #[test]
    fn prop_mapper_output_bounded_by_intensity(
        g_force   in -20.0_f32..=20.0_f32,
        g_lat     in -20.0_f32..=20.0_f32,
        g_lon     in -20.0_f32..=20.0_f32,
        bank_deg  in -180.0_f32..=180.0_f32,
        pitch_deg in -180.0_f32..=180.0_f32,
        yaw_rate  in -10.0_f32..=10.0_f32,
        intensity in 0.01_f32..=1.0_f32,
    ) {
        use flight_bus::types::{GForce, ValidatedAngle};

        let mut snapshot = BusSnapshot::default();
        snapshot.kinematics.g_force        = GForce::new(g_force).unwrap();
        snapshot.kinematics.g_lateral      = GForce::new(g_lat).unwrap();
        snapshot.kinematics.g_longitudinal = GForce::new(g_lon).unwrap();
        snapshot.kinematics.bank           = ValidatedAngle::new_degrees(bank_deg).unwrap();
        snapshot.kinematics.pitch          = ValidatedAngle::new_degrees(pitch_deg).unwrap();
        snapshot.angular_rates.r           = yaw_rate;

        let config = MotionConfig { intensity, ..Default::default() };
        let mut mapper = MotionMapper::new(config, 1.0 / 60.0);

        for _ in 0..10 {
            let frame = mapper.process(&snapshot);
            for &v in &frame.to_array() {
                prop_assert!(
                    v.abs() <= intensity + 1e-5,
                    "output {v} exceeds intensity bound {intensity}"
                );
            }
        }
    }

    /// A disabled DoF channel must always output exactly `0.0`, regardless of
    /// the input snapshot.  We test the surge channel as a representative case.
    #[test]
    fn prop_disabled_surge_channel_always_zero(
        g_force   in -20.0_f32..=20.0_f32,
        g_lat     in -20.0_f32..=20.0_f32,
        g_lon     in -20.0_f32..=20.0_f32,
        intensity in 0.01_f32..=1.0_f32,
    ) {
        use flight_bus::types::GForce;

        let mut snapshot = BusSnapshot::default();
        snapshot.kinematics.g_force        = GForce::new(g_force).unwrap();
        snapshot.kinematics.g_lateral      = GForce::new(g_lat).unwrap();
        snapshot.kinematics.g_longitudinal = GForce::new(g_lon).unwrap();

        let mut config = MotionConfig { intensity, ..Default::default() };
        config.surge.enabled = false;
        let mut mapper = MotionMapper::new(config, 1.0 / 60.0);

        let frame = mapper.process(&snapshot);
        prop_assert_eq!(frame.surge, 0.0, "disabled surge must be 0");
    }

    /// Inverting a channel must negate its output sign relative to the
    /// non-inverted mapper when `gain = 1` and the value is not saturated.
    ///
    /// We isolate the roll channel (driven by bank angle through a LP filter
    /// from zero state) and use `max_angle_deg = 360°` so that the first-tick
    /// LP output never reaches the ±1 clamp boundary.
    #[test]
    fn prop_inverted_roll_negates_normal(
        bank_deg  in -170.0_f32..=170.0_f32,
        intensity in 0.01_f32..=1.0_f32,
    ) {
        use flight_bus::types::ValidatedAngle;

        let mut snapshot = BusSnapshot::default();
        snapshot.kinematics.bank = ValidatedAngle::new_degrees(bank_deg).unwrap();

        // Large max_angle_deg prevents saturation, isolating the sign relationship.
        let mut base = MotionConfig { intensity, max_angle_deg: 360.0, ..Default::default() };
        base.roll.gain = 1.0;
        // Disable all other channels so only roll contributes.
        base.surge.enabled = false;
        base.sway.enabled  = false;
        base.heave.enabled = false;
        base.pitch.enabled = false;
        base.yaw.enabled   = false;

        let mut normal_cfg   = base.clone();
        normal_cfg.roll.invert = false;

        let mut inverted_cfg = base.clone();
        inverted_cfg.roll.invert = true;

        let mut normal_mapper   = MotionMapper::new(normal_cfg,   1.0 / 60.0);
        let mut inverted_mapper = MotionMapper::new(inverted_cfg, 1.0 / 60.0);

        let nf = normal_mapper.process(&snapshot);
        let iv = inverted_mapper.process(&snapshot);

        prop_assert!(
            (nf.roll + iv.roll).abs() < 1e-5,
            "inverted roll should negate normal: normal={}, inverted={}",
            nf.roll, iv.roll
        );
    }
}
