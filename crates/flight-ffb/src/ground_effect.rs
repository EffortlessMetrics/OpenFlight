// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Ground effect force modifier for force feedback (REQ-805)
//!
//! Simulates the increased lift and reduced control force experienced when
//! flying close to the ground. Below one wingspan height AGL, aerodynamic
//! forces change due to the ground plane constraining wingtip vortices,
//! which this module models as a stick-force reduction.

/// Configuration for ground effect simulation.
#[derive(Debug, Clone)]
pub struct GroundEffectConfig {
    /// Aircraft wingspan in meters. Ground effect becomes significant
    /// below approximately one wingspan height AGL.
    pub wingspan_m: f32,
}

impl Default for GroundEffectConfig {
    fn default() -> Self {
        Self {
            wingspan_m: 10.0, // ~33 ft, typical light GA aircraft
        }
    }
}

/// Computes a ground-effect force modifier.
///
/// The ground effect reduces control forces as the aircraft descends
/// below one wingspan height AGL. The returned modifier ranges from
/// `0.0` (maximum ground effect, on the runway) to `1.0` (no ground
/// effect, well above ground).
///
/// # Formula
///
/// ```text
/// modifier = 1.0 - 1.0 / (1.0 + (agl / wingspan * 2.0)^2)
/// ```
///
/// # Arguments
///
/// * `agl_m` — Altitude above ground level in **meters**.
/// * `airspeed_kts` — Current indicated airspeed in **knots**.
///   The effect is suppressed when airspeed is below 20 kts to avoid
///   artefacts during taxi.
/// * `config` — Aircraft-specific [`GroundEffectConfig`].
///
/// # Returns
///
/// A force modifier in the range `[0.0, 1.0]`. Multiply the nominal
/// stick force by this value to obtain the ground-effect-adjusted force.
pub fn ground_effect_modifier(agl_m: f32, airspeed_kts: f32, config: &GroundEffectConfig) -> f32 {
    // Clamp negative AGL (shouldn't happen but be defensive)
    let agl = agl_m.max(0.0);
    let wingspan = config.wingspan_m.max(0.01); // avoid division by zero

    // No effect when too slow (taxi / parked)
    if airspeed_kts < 20.0 {
        return 1.0;
    }

    // Well above ground — no effect
    if agl > wingspan * 2.0 {
        return 1.0;
    }

    let ratio = agl / wingspan * 2.0;
    let modifier = 1.0 - 1.0 / (1.0 + ratio.powi(2));

    modifier.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cfg() -> GroundEffectConfig {
        GroundEffectConfig { wingspan_m: 10.0 }
    }

    #[test]
    fn on_ground_gives_maximum_effect() {
        let m = ground_effect_modifier(0.0, 60.0, &default_cfg());
        assert!(m < 0.05, "modifier on ground should be near 0, got {m}");
    }

    #[test]
    fn high_altitude_gives_no_effect() {
        let m = ground_effect_modifier(100.0, 120.0, &default_cfg());
        assert!(
            (m - 1.0).abs() < 1e-4,
            "modifier at high altitude should be ~1.0, got {m}"
        );
    }

    #[test]
    fn half_wingspan_gives_partial_effect() {
        let m = ground_effect_modifier(5.0, 80.0, &default_cfg());
        assert!(m > 0.3 && m < 0.9, "half-wingspan modifier should be intermediate, got {m}");
    }

    #[test]
    fn low_airspeed_suppresses_effect() {
        let m = ground_effect_modifier(1.0, 10.0, &default_cfg());
        assert!(
            (m - 1.0).abs() < 1e-6,
            "modifier below 20 kts should be 1.0, got {m}"
        );
    }

    #[test]
    fn modifier_is_monotonically_increasing_with_altitude() {
        let cfg = default_cfg();
        let mut prev = ground_effect_modifier(0.0, 80.0, &cfg);
        for alt in 1..=30 {
            let cur = ground_effect_modifier(alt as f32, 80.0, &cfg);
            assert!(
                cur >= prev - 1e-6,
                "modifier should increase with altitude: alt={alt}, prev={prev}, cur={cur}"
            );
            prev = cur;
        }
    }

    #[test]
    fn negative_agl_clamped_to_zero() {
        let m = ground_effect_modifier(-5.0, 80.0, &default_cfg());
        assert!(m < 0.05, "negative AGL should behave like 0 AGL, got {m}");
    }

    #[test]
    fn zero_wingspan_does_not_panic() {
        let cfg = GroundEffectConfig { wingspan_m: 0.0 };
        let m = ground_effect_modifier(5.0, 80.0, &cfg);
        assert!((0.0..=1.0).contains(&m), "modifier out of range: {m}");
    }
}
