// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Body-zone mapping and flight-event routing for tactile feedback
//!
//! [`TactileZone`] represents where on the pilot's body a vibration is felt.
//! [`TactileRouter`] maps incoming flight events to the appropriate zones
//! with pre-configured [`HapticPattern`]s.

use crate::effects::EffectType;
use crate::haptic_effect::{self, HapticPattern};
use serde::{Deserialize, Serialize};

// ── Zone enum ────────────────────────────────────────────────────────

/// Physical location where tactile feedback is delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TactileZone {
    /// Stick / yoke grip.
    Grip,
    /// Trigger / button finger.
    Trigger,
    /// Throttle handle.
    ThrottleHandle,
    /// Left rudder pedal.
    PedalLeft,
    /// Right rudder pedal.
    PedalRight,
    /// Seat-back / seat-pan transducer.
    Seat,
    /// User-defined zone identified by index.
    Custom(u8),
}

// ── Zone config ──────────────────────────────────────────────────────

/// Per-zone configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneConfig {
    /// Target zone.
    pub zone: TactileZone,
    /// Index of the motor driving this zone.
    pub motor_index: u8,
    /// Intensity multiplier applied to patterns routed here (0.0–2.0).
    pub intensity_scale: f64,
    /// Whether this zone is active.
    pub enabled: bool,
}

impl ZoneConfig {
    /// Create a default-enabled zone config.
    pub fn new(zone: TactileZone, motor_index: u8) -> Self {
        Self {
            zone,
            motor_index,
            intensity_scale: 1.0,
            enabled: true,
        }
    }
}

// ── Router ───────────────────────────────────────────────────────────

/// Routes flight events to body zones with appropriate haptic patterns.
pub struct TactileRouter {
    zones: Vec<ZoneConfig>,
}

impl TactileRouter {
    /// Create a router with the given zone configurations.
    pub fn new(zones: Vec<ZoneConfig>) -> Self {
        Self { zones }
    }

    /// Create a router pre-loaded with sensible defaults for a typical setup.
    pub fn with_defaults() -> Self {
        Self::new(vec![
            ZoneConfig::new(TactileZone::Grip, 0),
            ZoneConfig::new(TactileZone::Trigger, 1),
            ZoneConfig::new(TactileZone::ThrottleHandle, 2),
            ZoneConfig::new(TactileZone::PedalLeft, 3),
            ZoneConfig::new(TactileZone::PedalRight, 4),
            ZoneConfig::new(TactileZone::Seat, 5),
        ])
    }

    /// Route a flight event to the appropriate zones and patterns.
    ///
    /// Returns a list of `(zone, pattern)` pairs. Disabled zones are
    /// excluded from the result. Intensity scales are baked into each
    /// pulse of the returned pattern.
    pub fn route_event(&self, event: &EffectType) -> Vec<(TactileZone, HapticPattern)> {
        let zone_pattern_pairs = match event {
            EffectType::Touchdown => vec![
                (TactileZone::Seat, haptic_effect::touchdown()),
                (TactileZone::PedalLeft, haptic_effect::touchdown()),
                (TactileZone::PedalRight, haptic_effect::touchdown()),
            ],
            EffectType::GroundRoll => vec![
                (TactileZone::Seat, haptic_effect::turbulence(0.3)),
                (TactileZone::PedalLeft, haptic_effect::turbulence(0.2)),
                (TactileZone::PedalRight, haptic_effect::turbulence(0.2)),
            ],
            EffectType::StallBuffet => vec![
                (TactileZone::Grip, haptic_effect::stall_warning()),
                (TactileZone::Seat, haptic_effect::stall_warning()),
            ],
            EffectType::EngineVibration => vec![
                (
                    TactileZone::ThrottleHandle,
                    haptic_effect::engine_vibration(2000.0),
                ),
                (TactileZone::Seat, haptic_effect::engine_vibration(2000.0)),
            ],
            EffectType::GearWarning => vec![
                (TactileZone::Grip, haptic_effect::landing_gear()),
                (TactileZone::PedalLeft, haptic_effect::landing_gear()),
                (TactileZone::PedalRight, haptic_effect::landing_gear()),
            ],
            EffectType::RotorVibration => vec![
                (TactileZone::Grip, haptic_effect::engine_vibration(3500.0)),
                (TactileZone::Seat, haptic_effect::engine_vibration(3500.0)),
            ],
        };

        zone_pattern_pairs
            .into_iter()
            .filter_map(|(zone, pattern)| {
                let cfg = self.zone_config(zone)?;
                if !cfg.enabled {
                    return None;
                }
                Some((zone, scale_pattern(pattern, cfg.intensity_scale)))
            })
            .collect()
    }

    /// Look up the configuration for a zone (first match).
    fn zone_config(&self, zone: TactileZone) -> Option<&ZoneConfig> {
        self.zones.iter().find(|z| z.zone == zone)
    }

    /// Get all zone configurations.
    pub fn zones(&self) -> &[ZoneConfig] {
        &self.zones
    }

    /// Replace the zone list.
    pub fn set_zones(&mut self, zones: Vec<ZoneConfig>) {
        self.zones = zones;
    }
}

/// Scale every pulse intensity in a pattern by `scale`.
fn scale_pattern(mut pattern: HapticPattern, scale: f64) -> HapticPattern {
    for (pulse, _) in &mut pattern.steps {
        pulse.intensity = (pulse.intensity * scale).clamp(0.0, 1.0);
    }
    pattern
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ZoneConfig ──────────────────────────────────────────────────

    #[test]
    fn test_zone_config_defaults() {
        let cfg = ZoneConfig::new(TactileZone::Seat, 5);
        assert_eq!(cfg.zone, TactileZone::Seat);
        assert_eq!(cfg.motor_index, 5);
        assert!((cfg.intensity_scale - 1.0).abs() < 1e-9);
        assert!(cfg.enabled);
    }

    // ── TactileRouter basics ────────────────────────────────────────

    #[test]
    fn test_default_router_has_six_zones() {
        let router = TactileRouter::with_defaults();
        assert_eq!(router.zones().len(), 6);
    }

    // ── Touchdown routing ───────────────────────────────────────────

    #[test]
    fn test_route_touchdown() {
        let router = TactileRouter::with_defaults();
        let results = router.route_event(&EffectType::Touchdown);
        assert!(!results.is_empty());

        let zones: Vec<_> = results.iter().map(|(z, _)| *z).collect();
        assert!(zones.contains(&TactileZone::Seat));
        assert!(zones.contains(&TactileZone::PedalLeft));
        assert!(zones.contains(&TactileZone::PedalRight));

        for (_, pat) in &results {
            assert!(!pat.is_empty());
        }
    }

    // ── Stall routing ───────────────────────────────────────────────

    #[test]
    fn test_route_stall_buffet() {
        let router = TactileRouter::with_defaults();
        let results = router.route_event(&EffectType::StallBuffet);
        assert!(!results.is_empty());

        let zones: Vec<_> = results.iter().map(|(z, _)| *z).collect();
        assert!(zones.contains(&TactileZone::Grip));
        assert!(zones.contains(&TactileZone::Seat));
    }

    // ── Engine vibration routing ────────────────────────────────────

    #[test]
    fn test_route_engine_vibration() {
        let router = TactileRouter::with_defaults();
        let results = router.route_event(&EffectType::EngineVibration);
        let zones: Vec<_> = results.iter().map(|(z, _)| *z).collect();
        assert!(zones.contains(&TactileZone::ThrottleHandle));
        assert!(zones.contains(&TactileZone::Seat));
    }

    // ── Gear warning routing ────────────────────────────────────────

    #[test]
    fn test_route_gear_warning() {
        let router = TactileRouter::with_defaults();
        let results = router.route_event(&EffectType::GearWarning);
        let zones: Vec<_> = results.iter().map(|(z, _)| *z).collect();
        assert!(zones.contains(&TactileZone::Grip));
    }

    // ── Disabled zone is skipped ────────────────────────────────────

    #[test]
    fn test_disabled_zone_skipped() {
        let mut zones = vec![
            ZoneConfig::new(TactileZone::Grip, 0),
            ZoneConfig::new(TactileZone::Seat, 5),
        ];
        zones[1].enabled = false;

        let router = TactileRouter::new(zones);
        let results = router.route_event(&EffectType::StallBuffet);

        let zone_list: Vec<_> = results.iter().map(|(z, _)| *z).collect();
        assert!(zone_list.contains(&TactileZone::Grip));
        assert!(
            !zone_list.contains(&TactileZone::Seat),
            "disabled zone must be excluded"
        );
    }

    // ── Unknown zone not configured → skipped ───────────────────────

    #[test]
    fn test_unconfigured_zone_skipped() {
        // Router with only Grip configured
        let router = TactileRouter::new(vec![ZoneConfig::new(TactileZone::Grip, 0)]);
        let results = router.route_event(&EffectType::Touchdown);
        // Touchdown targets Seat + Pedals, none of which are configured
        assert!(results.is_empty());
    }

    // ── Intensity scaling ───────────────────────────────────────────

    #[test]
    fn test_intensity_scaling() {
        let mut cfg = ZoneConfig::new(TactileZone::Seat, 5);
        cfg.intensity_scale = 0.5;

        let router = TactileRouter::new(vec![cfg]);
        let results = router.route_event(&EffectType::EngineVibration);
        assert!(!results.is_empty());

        for (_, pat) in &results {
            for (pulse, _) in &pat.steps {
                assert!(
                    pulse.intensity <= 0.5 + 1e-9,
                    "scaling should halve intensity"
                );
            }
        }
    }

    #[test]
    fn test_intensity_scale_clamped_to_one() {
        let mut cfg = ZoneConfig::new(TactileZone::Seat, 5);
        cfg.intensity_scale = 5.0; // Very high scale

        let router = TactileRouter::new(vec![cfg]);
        let results = router.route_event(&EffectType::Touchdown);

        for (_, pat) in &results {
            for (pulse, _) in &pat.steps {
                assert!(pulse.intensity <= 1.0, "intensity must not exceed 1.0");
            }
        }
    }

    // ── Custom zone ─────────────────────────────────────────────────

    #[test]
    fn test_custom_zone() {
        let zone = TactileZone::Custom(42);
        let cfg = ZoneConfig::new(zone, 10);
        assert_eq!(cfg.zone, TactileZone::Custom(42));
    }

    // ── scale_pattern helper ────────────────────────────────────────

    #[test]
    fn test_scale_pattern_zero() {
        let pat = haptic_effect::touchdown();
        let scaled = scale_pattern(pat, 0.0);
        for (pulse, _) in &scaled.steps {
            assert_eq!(pulse.intensity, 0.0);
        }
    }

    // ── set_zones ───────────────────────────────────────────────────

    #[test]
    fn test_set_zones() {
        let mut router = TactileRouter::with_defaults();
        assert_eq!(router.zones().len(), 6);

        router.set_zones(vec![ZoneConfig::new(TactileZone::Grip, 0)]);
        assert_eq!(router.zones().len(), 1);
    }

    // ── all EffectType variants routed ──────────────────────────────

    #[test]
    fn test_all_effect_types_produce_results() {
        let router = TactileRouter::with_defaults();
        let variants = [
            EffectType::Touchdown,
            EffectType::GroundRoll,
            EffectType::StallBuffet,
            EffectType::EngineVibration,
            EffectType::GearWarning,
            EffectType::RotorVibration,
        ];
        for variant in &variants {
            let results = router.route_event(variant);
            assert!(
                !results.is_empty(),
                "{:?} should produce at least one routed zone",
                variant
            );
        }
    }
}
