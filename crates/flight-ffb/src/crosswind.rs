// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Crosswind force generation for force feedback (REQ-850)
//!
//! Computes yaw and roll forces arising from crosswind components.
//! The effect is strongest at low airspeed (approach/landing) where
//! the crosswind-to-airspeed ratio is large.

/// Output of the crosswind force computation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrosswindForceOutput {
    /// Yaw force (−1.0 to 1.0). Positive = wind pushes nose right.
    pub yaw_force: f32,
    /// Roll force (−1.0 to 1.0). Positive = wind rolls aircraft right.
    pub roll_force: f32,
}

/// Computes crosswind-induced yaw and roll forces.
///
/// # Arguments
///
/// * `wind_speed_kts` — Total wind speed in **knots**.
/// * `wind_direction_deg` — Direction the wind is coming **from** in
///   degrees true (0–360).
/// * `aircraft_heading_deg` — Aircraft magnetic/true heading in degrees
///   (0–360).
/// * `airspeed_kts` — Indicated airspeed in **knots**.
///
/// # Returns
///
/// A [`CrosswindForceOutput`] with yaw and roll force values in
/// `[−1.0, 1.0]`. Forces are zero when airspeed is below 5 kts
/// (parked / stationary) to avoid divide-by-zero artefacts.
pub fn compute_crosswind_forces(
    wind_speed_kts: f32,
    wind_direction_deg: f32,
    aircraft_heading_deg: f32,
    airspeed_kts: f32,
) -> CrosswindForceOutput {
    let zero = CrosswindForceOutput {
        yaw_force: 0.0,
        roll_force: 0.0,
    };

    if airspeed_kts < 5.0 || wind_speed_kts <= 0.0 {
        return zero;
    }

    // Crosswind component: positive = wind from the right
    let relative_deg = (wind_direction_deg - aircraft_heading_deg).to_radians();
    let crosswind_component = wind_speed_kts * relative_deg.sin();

    // Ratio of crosswind to airspeed — effect is stronger at low airspeed
    let ratio = crosswind_component / airspeed_kts;

    // Yaw force — proportional to crosswind/airspeed ratio
    let yaw_force = ratio.clamp(-1.0, 1.0);

    // Roll force — weaker than yaw (roughly 40% coupling)
    let roll_force = (ratio * 0.4).clamp(-1.0, 1.0);

    CrosswindForceOutput {
        yaw_force,
        roll_force,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_wind_gives_zero_forces() {
        let out = compute_crosswind_forces(0.0, 270.0, 180.0, 120.0);
        assert_eq!(out.yaw_force, 0.0);
        assert_eq!(out.roll_force, 0.0);
    }

    #[test]
    fn headwind_gives_near_zero_crosswind() {
        // Wind from 360, heading 360 → pure headwind
        let out = compute_crosswind_forces(20.0, 360.0, 360.0, 100.0);
        assert!(
            out.yaw_force.abs() < 0.01,
            "headwind yaw should be ~0, got {}",
            out.yaw_force
        );
    }

    #[test]
    fn pure_crosswind_from_right() {
        // Heading 360, wind from 090 → full right crosswind
        let out = compute_crosswind_forces(30.0, 90.0, 0.0, 80.0);
        assert!(
            out.yaw_force > 0.0,
            "right crosswind should give positive yaw: {}",
            out.yaw_force
        );
        assert!(
            out.roll_force > 0.0,
            "right crosswind should give positive roll: {}",
            out.roll_force
        );
    }

    #[test]
    fn low_airspeed_amplifies_effect() {
        let fast = compute_crosswind_forces(20.0, 90.0, 0.0, 140.0);
        let slow = compute_crosswind_forces(20.0, 90.0, 0.0, 40.0);
        assert!(
            slow.yaw_force.abs() > fast.yaw_force.abs(),
            "low airspeed should amplify: slow={}, fast={}",
            slow.yaw_force,
            fast.yaw_force
        );
    }

    #[test]
    fn very_low_airspeed_suppressed() {
        let out = compute_crosswind_forces(30.0, 90.0, 0.0, 3.0);
        assert_eq!(out.yaw_force, 0.0);
        assert_eq!(out.roll_force, 0.0);
    }

    #[test]
    fn roll_is_weaker_than_yaw() {
        let out = compute_crosswind_forces(25.0, 90.0, 0.0, 80.0);
        assert!(
            out.roll_force.abs() < out.yaw_force.abs(),
            "roll should be weaker: roll={}, yaw={}",
            out.roll_force,
            out.yaw_force
        );
    }

    #[test]
    fn forces_clamped_to_range() {
        // Extreme crosswind with very low airspeed just above threshold
        let out = compute_crosswind_forces(200.0, 90.0, 0.0, 6.0);
        assert!(
            (-1.0..=1.0).contains(&out.yaw_force),
            "yaw out of range: {}",
            out.yaw_force
        );
        assert!(
            (-1.0..=1.0).contains(&out.roll_force),
            "roll out of range: {}",
            out.roll_force
        );
    }
}
