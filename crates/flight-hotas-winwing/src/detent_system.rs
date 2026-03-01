// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Throttle detent handling for WinWing devices.
//!
//! WinWing throttles feature configurable magnetic detent positions
//! (Idle, Afterburner, and custom positions).  This module provides:
//!
//! - [`WinwingDetentConfig`] — per-lever detent configuration with
//!   magnetic strength and snap zones.
//! - [`detect_detent`] — determine which detent zone (if any) an axis
//!   value falls within, based on the current configuration.
//!
//! # Magnetic detent model
//!
//! Each detent has a *centre position* and a *snap radius*.  When the
//! raw axis value is within the snap radius of a detent centre, the
//! detent is considered active and the output value snaps to the centre.
//! The `strength` parameter (0.0–1.0) controls how aggressively the
//! value is pulled toward the centre within the snap zone.

use crate::protocol::DetentName;

// ── Detent position ───────────────────────────────────────────────────────────

/// A single detent position with magnetic snap behaviour.
#[derive(Debug, Clone, PartialEq)]
pub struct MagneticDetent {
    /// Which detent this represents.
    pub name: DetentName,
    /// Centre position on the normalised \[0.0, 1.0\] axis.
    pub centre: f32,
    /// Half-width of the snap zone (normalised units).
    ///
    /// A raw axis value within `centre ± snap_radius` is considered
    /// inside this detent zone.
    pub snap_radius: f32,
    /// Magnetic strength 0.0 (no pull) to 1.0 (hard snap to centre).
    pub strength: f32,
}

impl MagneticDetent {
    /// Returns `true` if `value` is within the snap zone of this detent.
    pub fn contains(&self, value: f32) -> bool {
        (value - self.centre).abs() <= self.snap_radius
    }

    /// Apply the magnetic detent effect to `value`.
    ///
    /// If `value` is within the snap zone, it is pulled toward `centre`
    /// by `strength`.  If outside the zone, `value` is returned unchanged.
    pub fn apply(&self, value: f32) -> f32 {
        if !self.contains(value) {
            return value;
        }
        let delta = self.centre - value;
        value + delta * self.strength
    }
}

// ── Active detent result ──────────────────────────────────────────────────────

/// Result of a detent detection query.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveDetent {
    /// Which detent is active.
    pub name: DetentName,
    /// The original raw axis value (normalised 0.0–1.0).
    pub raw_value: f32,
    /// The snapped output value after magnetic pull.
    pub snapped_value: f32,
}

// ── Detent configuration ──────────────────────────────────────────────────────

/// Per-lever detent configuration for a WinWing throttle.
///
/// Holds an ordered list of [`MagneticDetent`]s.  Detent zones must
/// not overlap; behaviour is undefined if they do (the first match
/// wins).
#[derive(Debug, Clone)]
pub struct WinwingDetentConfig {
    detents: Vec<MagneticDetent>,
}

impl WinwingDetentConfig {
    /// Create a new configuration with no detents.
    pub fn new() -> Self {
        Self {
            detents: Vec::new(),
        }
    }

    /// Create a default military-aircraft detent layout.
    ///
    /// Two detents: Idle at 0.0 and Afterburner at 1.0.
    pub fn military_default() -> Self {
        Self {
            detents: vec![
                MagneticDetent {
                    name: DetentName::Idle,
                    centre: 0.0,
                    snap_radius: 0.03,
                    strength: 0.8,
                },
                MagneticDetent {
                    name: DetentName::Afterburner,
                    centre: 1.0,
                    snap_radius: 0.03,
                    strength: 0.8,
                },
            ],
        }
    }

    /// Create a civilian-aircraft detent layout.
    ///
    /// Two detents: Idle at 0.0 and a custom "Climb" detent at 0.85.
    pub fn civilian_default() -> Self {
        Self {
            detents: vec![
                MagneticDetent {
                    name: DetentName::Idle,
                    centre: 0.0,
                    snap_radius: 0.03,
                    strength: 0.8,
                },
                MagneticDetent {
                    name: DetentName::Custom(2),
                    centre: 0.85,
                    snap_radius: 0.04,
                    strength: 0.7,
                },
            ],
        }
    }

    /// Add a detent position.
    pub fn add_detent(&mut self, detent: MagneticDetent) {
        self.detents.push(detent);
    }

    /// The configured detents.
    pub fn detents(&self) -> &[MagneticDetent] {
        &self.detents
    }

    /// Number of configured detents.
    pub fn len(&self) -> usize {
        self.detents.len()
    }

    /// Returns `true` if no detents are configured.
    pub fn is_empty(&self) -> bool {
        self.detents.is_empty()
    }
}

impl Default for WinwingDetentConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect which detent (if any) a normalised axis value falls within.
///
/// Returns `None` if the value is not within any detent zone.
pub fn detect_detent(config: &WinwingDetentConfig, value: f32) -> Option<ActiveDetent> {
    for detent in &config.detents {
        if detent.contains(value) {
            return Some(ActiveDetent {
                name: detent.name,
                raw_value: value,
                snapped_value: detent.apply(value),
            });
        }
    }
    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn idle_detent() -> MagneticDetent {
        MagneticDetent {
            name: DetentName::Idle,
            centre: 0.0,
            snap_radius: 0.05,
            strength: 1.0,
        }
    }

    fn afterburner_detent() -> MagneticDetent {
        MagneticDetent {
            name: DetentName::Afterburner,
            centre: 1.0,
            snap_radius: 0.05,
            strength: 1.0,
        }
    }

    // ── MagneticDetent ────────────────────────────────────────────────────

    #[test]
    fn test_magnetic_detent_contains_at_centre() {
        let d = idle_detent();
        assert!(d.contains(0.0));
    }

    #[test]
    fn test_magnetic_detent_contains_within_zone() {
        let d = idle_detent();
        assert!(d.contains(0.03));
        assert!(d.contains(-0.03));
    }

    #[test]
    fn test_magnetic_detent_contains_at_boundary() {
        let d = idle_detent();
        assert!(d.contains(0.05));
    }

    #[test]
    fn test_magnetic_detent_outside_zone() {
        let d = idle_detent();
        assert!(!d.contains(0.06));
        assert!(!d.contains(0.5));
    }

    #[test]
    fn test_magnetic_detent_apply_full_strength() {
        let d = idle_detent(); // strength = 1.0
        let snapped = d.apply(0.03);
        assert!(
            (snapped - 0.0).abs() < 1e-6,
            "full strength should snap to centre"
        );
    }

    #[test]
    fn test_magnetic_detent_apply_partial_strength() {
        let d = MagneticDetent {
            name: DetentName::Idle,
            centre: 0.0,
            snap_radius: 0.05,
            strength: 0.5,
        };
        let snapped = d.apply(0.04);
        // 0.04 + (0.0 - 0.04) * 0.5 = 0.04 - 0.02 = 0.02
        assert!((snapped - 0.02).abs() < 1e-6);
    }

    #[test]
    fn test_magnetic_detent_apply_zero_strength() {
        let d = MagneticDetent {
            name: DetentName::Idle,
            centre: 0.0,
            snap_radius: 0.05,
            strength: 0.0,
        };
        let snapped = d.apply(0.03);
        assert!(
            (snapped - 0.03).abs() < 1e-6,
            "zero strength should not move value"
        );
    }

    #[test]
    fn test_magnetic_detent_apply_outside_zone() {
        let d = idle_detent();
        let snapped = d.apply(0.5);
        assert!(
            (snapped - 0.5).abs() < 1e-6,
            "outside zone should pass through"
        );
    }

    // ── WinwingDetentConfig ───────────────────────────────────────────────

    #[test]
    fn test_config_new_is_empty() {
        let c = WinwingDetentConfig::new();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn test_config_default_is_empty() {
        let c = WinwingDetentConfig::default();
        assert!(c.is_empty());
    }

    #[test]
    fn test_config_add_detent() {
        let mut c = WinwingDetentConfig::new();
        c.add_detent(idle_detent());
        assert_eq!(c.len(), 1);
        assert!(!c.is_empty());
    }

    #[test]
    fn test_config_military_default() {
        let c = WinwingDetentConfig::military_default();
        assert_eq!(c.len(), 2);
        assert_eq!(c.detents()[0].name, DetentName::Idle);
        assert_eq!(c.detents()[1].name, DetentName::Afterburner);
    }

    #[test]
    fn test_config_civilian_default() {
        let c = WinwingDetentConfig::civilian_default();
        assert_eq!(c.len(), 2);
        assert_eq!(c.detents()[0].name, DetentName::Idle);
        assert_eq!(c.detents()[1].name, DetentName::Custom(2));
    }

    #[test]
    fn test_config_military_default_positions() {
        let c = WinwingDetentConfig::military_default();
        assert!((c.detents()[0].centre - 0.0).abs() < 1e-6);
        assert!((c.detents()[1].centre - 1.0).abs() < 1e-6);
    }

    // ── detect_detent ─────────────────────────────────────────────────────

    #[test]
    fn test_detect_detent_idle() {
        let c = WinwingDetentConfig::military_default();
        let result = detect_detent(&c, 0.01).unwrap();
        assert_eq!(result.name, DetentName::Idle);
        assert!((result.raw_value - 0.01).abs() < 1e-6);
    }

    #[test]
    fn test_detect_detent_afterburner() {
        let c = WinwingDetentConfig::military_default();
        let result = detect_detent(&c, 0.99).unwrap();
        assert_eq!(result.name, DetentName::Afterburner);
    }

    #[test]
    fn test_detect_detent_none_midrange() {
        let c = WinwingDetentConfig::military_default();
        assert!(detect_detent(&c, 0.5).is_none());
    }

    #[test]
    fn test_detect_detent_none_empty_config() {
        let c = WinwingDetentConfig::new();
        assert!(detect_detent(&c, 0.5).is_none());
    }

    #[test]
    fn test_detect_detent_snapped_value() {
        let mut c = WinwingDetentConfig::new();
        c.add_detent(MagneticDetent {
            name: DetentName::Idle,
            centre: 0.0,
            snap_radius: 0.05,
            strength: 1.0,
        });
        let result = detect_detent(&c, 0.03).unwrap();
        assert!((result.snapped_value - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_detect_detent_partial_snap() {
        let mut c = WinwingDetentConfig::new();
        c.add_detent(MagneticDetent {
            name: DetentName::Afterburner,
            centre: 1.0,
            snap_radius: 0.05,
            strength: 0.5,
        });
        let result = detect_detent(&c, 0.97).unwrap();
        // 0.97 + (1.0 - 0.97) * 0.5 = 0.97 + 0.015 = 0.985
        assert!((result.snapped_value - 0.985).abs() < 1e-5);
    }

    #[test]
    fn test_detect_detent_custom_position() {
        let mut c = WinwingDetentConfig::new();
        c.add_detent(MagneticDetent {
            name: DetentName::Custom(5),
            centre: 0.5,
            snap_radius: 0.02,
            strength: 0.9,
        });
        let result = detect_detent(&c, 0.51).unwrap();
        assert_eq!(result.name, DetentName::Custom(5));
    }

    #[test]
    fn test_detect_detent_just_outside_zone() {
        let mut c = WinwingDetentConfig::new();
        c.add_detent(MagneticDetent {
            name: DetentName::Idle,
            centre: 0.0,
            snap_radius: 0.03,
            strength: 1.0,
        });
        // 0.031 is just outside the snap radius
        assert!(detect_detent(&c, 0.031).is_none());
    }

    #[test]
    fn test_detect_detent_at_exact_boundary() {
        let mut c = WinwingDetentConfig::new();
        c.add_detent(MagneticDetent {
            name: DetentName::Idle,
            centre: 0.0,
            snap_radius: 0.03,
            strength: 1.0,
        });
        // Exactly at boundary should be inside
        let result = detect_detent(&c, 0.03);
        assert!(result.is_some());
    }

    #[test]
    fn test_multiple_detents_first_match_wins() {
        let mut c = WinwingDetentConfig::new();
        c.add_detent(idle_detent());
        c.add_detent(afterburner_detent());

        // Value 0.0 is in idle zone
        let result = detect_detent(&c, 0.0).unwrap();
        assert_eq!(result.name, DetentName::Idle);

        // Value 1.0 is in afterburner zone
        let result = detect_detent(&c, 1.0).unwrap();
        assert_eq!(result.name, DetentName::Afterburner);
    }

    #[test]
    fn test_detent_config_detents_accessor() {
        let c = WinwingDetentConfig::military_default();
        let detents = c.detents();
        assert_eq!(detents.len(), 2);
        assert!(detents[0].strength > 0.0);
        assert!(detents[1].strength > 0.0);
    }
}
