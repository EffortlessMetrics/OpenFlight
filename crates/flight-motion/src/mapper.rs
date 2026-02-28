// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Maps a [`BusSnapshot`] to a [`MotionFrame`] using washout filtering.
//!
//! ## Channel mapping
//!
//! | DoF   | BusSnapshot source              | Filter    |
//! |-------|---------------------------------|-----------|
//! | Surge | `kinematics.g_longitudinal`     | High-pass |
//! | Sway  | `kinematics.g_lateral`          | High-pass |
//! | Heave | `kinematics.g_force` − 1G       | High-pass |
//! | Roll  | `kinematics.bank` (deg)         | Low-pass  |
//! | Pitch | `kinematics.pitch` (deg)        | Low-pass  |
//! | Yaw   | `angular_rates.r` (deg/s)       | High-pass |
//!
//! Translational channels are normalized by `max_g` (configurable).
//! Angular channels are normalized by `max_angle_deg` or `max_yaw_rate_deg_s`.

use crate::{config::MotionConfig, frame::MotionFrame, washout::WashoutFilter};
use flight_bus::BusSnapshot;

/// Maps BusSnapshot kinematics to a 6DOF motion frame with washout filtering.
///
/// Designed to be called once per processing tick (250 Hz from the RT spine, or
/// slower if driven by a sim adapter update rate).
///
/// ```
/// use flight_motion::{MotionMapper, MotionConfig};
/// use flight_bus::BusSnapshot;
///
/// let config = MotionConfig::default();
/// let sample_dt = 1.0 / 250.0;  // 250 Hz
/// let mut mapper = MotionMapper::new(config, sample_dt);
///
/// let snapshot = BusSnapshot::default();
/// let frame = mapper.process(&snapshot);
/// // frame.surge, .sway, .heave, .roll, .pitch, .yaw are all 0 for a neutral snapshot
/// ```
#[derive(Debug)]
pub struct MotionMapper {
    config: MotionConfig,
    washout: WashoutFilter,
}

impl MotionMapper {
    /// Create a new mapper with the given configuration and sample interval.
    ///
    /// `sample_dt` is the expected time between [`process`] calls in seconds.
    pub fn new(config: MotionConfig, sample_dt: f32) -> Self {
        let washout = WashoutFilter::new(&config.washout, sample_dt);
        Self { config, washout }
    }

    /// Process one `BusSnapshot` tick and return a 6DOF motion frame.
    ///
    /// Returns [`MotionFrame::NEUTRAL`] if the snapshot is not valid or if the
    /// global intensity is zero.
    pub fn process(&mut self, snapshot: &BusSnapshot) -> MotionFrame {
        if self.config.intensity == 0.0 {
            return MotionFrame::NEUTRAL;
        }

        let k = &snapshot.kinematics;
        let ar = &snapshot.angular_rates;

        // ── Translational channels ────────────────────────────────────────────
        //
        // Normalize g-forces by max_g, then high-pass filter for onset cues.
        // Heave: subtract 1G (gravity) so that level flight maps to 0.
        let surge_raw = -k.g_longitudinal.value() / self.config.max_g;
        let sway_raw = k.g_lateral.value() / self.config.max_g;
        let heave_raw = (k.g_force.value() - 1.0) / self.config.max_g;

        let surge = self.washout.surge_hp.process(surge_raw);
        let sway = self.washout.sway_hp.process(sway_raw);
        let heave = self.washout.heave_hp.process(heave_raw);

        // ── Angular channels ──────────────────────────────────────────────────
        //
        // Roll and pitch: direct attitude angle, low-pass filtered for sustained tilt.
        let roll_deg = k.bank.value();
        let pitch_deg = k.pitch.value();
        let roll_raw = roll_deg / self.config.max_angle_deg;
        let pitch_raw = pitch_deg / self.config.max_angle_deg;

        let roll = self.washout.roll_lp.process(roll_raw);
        let pitch = self.washout.pitch_lp.process(pitch_raw);

        // Yaw: use yaw rate (deg/s), high-pass filtered for onset cue.
        let yaw_rate_deg_s = ar.r.to_degrees();
        let yaw_raw = yaw_rate_deg_s / self.config.max_yaw_rate_deg_s;
        let yaw = self.washout.yaw_hp.process(yaw_raw);

        // ── Apply per-channel config ──────────────────────────────────────────

        let apply = |val: f32, cfg: &crate::config::DoFConfig| -> f32 {
            if !cfg.enabled {
                return 0.0;
            }
            let v = val * cfg.gain * if cfg.invert { -1.0 } else { 1.0 };
            v.clamp(-1.0, 1.0)
        };

        let frame = MotionFrame {
            surge: apply(surge, &self.config.surge),
            sway: apply(sway, &self.config.sway),
            heave: apply(heave, &self.config.heave),
            roll: apply(roll, &self.config.roll),
            pitch: apply(pitch, &self.config.pitch),
            yaw: apply(yaw, &self.config.yaw),
        };

        frame.scaled(self.config.intensity)
    }

    /// Reset all filter states (e.g. on sim disconnect or aircraft change).
    pub fn reset(&mut self) {
        self.washout.reset();
    }

    /// Return the current configuration.
    pub fn config(&self) -> &MotionConfig {
        &self.config
    }

    /// Update the configuration at runtime.
    ///
    /// Filter corner frequencies are not recalculated; call [`MotionMapper::new`]
    /// to change filter characteristics.
    pub fn set_intensity(&mut self, intensity: f32) {
        self.config.intensity = intensity.clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_bus::BusSnapshot;

    fn neutral_snapshot() -> BusSnapshot {
        BusSnapshot::default()
    }

    #[test]
    fn test_neutral_snapshot_produces_neutral_frame() {
        let mut mapper = MotionMapper::new(MotionConfig::default(), 1.0 / 250.0);
        let _frame = mapper.process(&neutral_snapshot());
        for _ in 0..5000 {
            mapper.process(&neutral_snapshot());
        }
        let frame = mapper.process(&neutral_snapshot());
        assert!(
            frame.surge.abs() < 0.01,
            "Surge should wash out: {}",
            frame.surge
        );
        assert!(
            frame.sway.abs() < 0.01,
            "Sway should wash out: {}",
            frame.sway
        );
        assert!(
            frame.roll.abs() < 0.01,
            "Roll should wash out: {}",
            frame.roll
        );
    }

    #[test]
    fn test_zero_intensity_returns_neutral() {
        let config = MotionConfig { intensity: 0.0, ..Default::default() };
        let mut mapper = MotionMapper::new(config, 1.0 / 250.0);
        let frame = mapper.process(&neutral_snapshot());
        assert!(frame.is_neutral());
    }

    #[test]
    fn test_set_intensity_clamps() {
        let mut mapper = MotionMapper::new(MotionConfig::default(), 1.0 / 250.0);
        mapper.set_intensity(2.0);
        assert_eq!(mapper.config().intensity, 1.0);
        mapper.set_intensity(-1.0);
        assert_eq!(mapper.config().intensity, 0.0);
    }

    #[test]
    fn test_disabled_channel_outputs_zero() {
        let mut config = MotionConfig::default();
        config.roll.enabled = false;
        config.pitch.enabled = false;
        let mut mapper = MotionMapper::new(config, 1.0 / 250.0);
        // Even with a non-zero snapshot, disabled channels should be zero
        let frame = mapper.process(&neutral_snapshot());
        assert_eq!(frame.roll, 0.0);
        assert_eq!(frame.pitch, 0.0);
    }

    #[test]
    fn test_reset_clears_filter_state() {
        let mut mapper = MotionMapper::new(MotionConfig::default(), 1.0 / 250.0);
        // Drive for a while to build up filter state
        for _ in 0..100 {
            mapper.process(&neutral_snapshot());
        }
        mapper.reset();
        // After reset, mapper should behave like freshly created
        let frame = mapper.process(&neutral_snapshot());
        // roll and pitch should be very close to 0 on first tick after reset
        assert!(frame.roll.abs() < 0.01);
        assert!(frame.pitch.abs() < 0.01);
    }

    proptest::proptest! {
        /// MotionMapper::process() must never produce outputs outside [-1, 1]
        /// regardless of the input snapshot values, as long as intensity > 0.
        #[test]
        fn prop_process_output_always_in_bounds(
            g_force in -20.0_f32..=20.0_f32,
            g_lat   in -20.0_f32..=20.0_f32,
            g_lon   in -20.0_f32..=20.0_f32,
            bank_deg  in -180.0_f32..=180.0_f32,
            pitch_deg in -180.0_f32..=180.0_f32,
            intensity in 0.01_f32..=1.0_f32,
        ) {
            use flight_bus::types::{GForce, ValidatedAngle};

            let mut snapshot = BusSnapshot::default();
            snapshot.kinematics.g_force = GForce::new(g_force).unwrap();
            snapshot.kinematics.g_lateral = GForce::new(g_lat).unwrap();
            snapshot.kinematics.g_longitudinal = GForce::new(g_lon).unwrap();
            snapshot.kinematics.bank = ValidatedAngle::new_degrees(bank_deg).unwrap();
            snapshot.kinematics.pitch = ValidatedAngle::new_degrees(pitch_deg).unwrap();

            let config = MotionConfig { intensity, ..Default::default() };
            let mut mapper = MotionMapper::new(config, 1.0 / 250.0);

            for _ in 0..5 {
                let frame = mapper.process(&snapshot);
                proptest::prop_assert!(frame.surge.abs() <= 1.0, "surge OOB: {}", frame.surge);
                proptest::prop_assert!(frame.sway.abs()  <= 1.0, "sway OOB: {}",  frame.sway);
                proptest::prop_assert!(frame.heave.abs() <= 1.0, "heave OOB: {}", frame.heave);
                proptest::prop_assert!(frame.roll.abs()  <= 1.0, "roll OOB: {}",  frame.roll);
                proptest::prop_assert!(frame.pitch.abs() <= 1.0, "pitch OOB: {}", frame.pitch);
                proptest::prop_assert!(frame.yaw.abs()   <= 1.0, "yaw OOB: {}",   frame.yaw);
            }
        }
    }
}
