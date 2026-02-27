// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile Management System
//!
//! This module provides comprehensive profile management for Flight Hub, including
//! JSON Schema validation, canonicalization, merging, and capability enforcement.
//!
//! # Overview
//!
//! Profiles define how flight controls behave for specific aircraft and flight phases.
//! The system supports:
//!
//! - **Hierarchical Merging**: Global → Sim → Aircraft → Phase of Flight
//! - **Deterministic Canonicalization**: Ensures identical profiles produce identical hashes
//! - **Capability Enforcement**: Kid/Demo mode restrictions
//! - **Validation**: JSON Schema compliance with detailed error reporting
//!
//! # Examples
//!
//! ## Basic Profile Creation
//!
//! ```rust
//! use flight_profile::{Profile, AxisConfig, AircraftId};
//! use std::collections::HashMap;
//!
//! let mut axes = HashMap::new();
//! axes.insert("pitch".to_string(), AxisConfig {
//!     deadzone: Some(0.03),
//!     expo: Some(0.2),
//!     slew_rate: Some(1.2),
//!     detents: vec![],
//!     curve: None,
//!     filter: None,
//! });
//!
//! let profile = Profile {
//!     schema: "flight.profile/1".to_string(),
//!     sim: Some("msfs".to_string()),
//!     aircraft: Some(AircraftId { icao: "C172".to_string() }),
//!     axes,
//!     pof_overrides: None,
//! };
//!
//! // Validate the profile
//! profile.validate().expect("Profile should be valid");
//! ```
//!
//! ## Profile Merging
//!
//! ```rust
//! # use flight_profile::{Profile, AxisConfig, AircraftId};
//! # use std::collections::HashMap;
//! # let base_profile = Profile {
//! #     schema: "flight.profile/1".to_string(),
//! #     sim: Some("msfs".to_string()),
//! #     aircraft: Some(AircraftId { icao: "C172".to_string() }),
//! #     axes: HashMap::new(),
//! #     pof_overrides: None,
//! # };
//! # let override_profile = base_profile.clone();
//!
//! // Merge profiles with last-writer-wins semantics
//! let merged = base_profile.merge_with(&override_profile)?;
//!
//! // Verify deterministic behavior
//! let hash1 = merged.effective_hash();
//! let hash2 = merged.effective_hash();
//! assert_eq!(hash1, hash2);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod profile_compare;
pub mod profile_linter;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Recursively normalise all JSON numbers to 6 decimal places (f64).
///
/// This is used by [`Profile::canonicalize`] to produce a stable hash that
/// survives JSON roundtrips.  Working in f64 avoids the rounding instability
/// that occurs when rounding f32 values with f32 arithmetic.
fn normalize_json_floats(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                let rounded = (f * 1_000_000.0_f64).round() / 1_000_000.0_f64;
                serde_json::Number::from_f64(rounded)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Number(n))
            } else {
                serde_json::Value::Number(n)
            }
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(normalize_json_floats).collect())
        }
        serde_json::Value::Object(map) => {
            let normalized = map
                .into_iter()
                .map(|(k, v)| (k, normalize_json_floats(v)))
                .collect();
            serde_json::Value::Object(normalized)
        }
        other => other,
    }
}

#[derive(Error, Debug)]
pub enum ProfileError {
    #[error("Validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, ProfileError>;

/// Flight profile schema version
pub const PROFILE_SCHEMA_VERSION: &str = "flight.profile/1";

/// Maximum allowed values for validation
pub const MAX_DEADZONE: f32 = 0.5;
pub const MAX_EXPO: f32 = 1.0;
pub const MAX_SLEW_RATE: f32 = 100.0;

/// Complete flight profile definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    /// Schema version for compatibility checking
    pub schema: String,

    /// Target simulator (optional for global profiles)
    pub sim: Option<String>,

    /// Target aircraft (optional for sim-wide profiles)
    pub aircraft: Option<AircraftId>,

    /// Axis configurations by name
    pub axes: HashMap<String, AxisConfig>,

    /// Phase of Flight overrides
    pub pof_overrides: Option<HashMap<String, PofOverrides>>,
}

/// Aircraft identification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AircraftId {
    /// ICAO aircraft type code
    pub icao: String,
}

/// Configuration for a single axis
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AxisConfig {
    /// Deadzone size (0.0 to 0.5)
    pub deadzone: Option<f32>,

    /// Exponential curve factor (0.0 to 1.0)
    pub expo: Option<f32>,

    /// Slew rate limit in units per second
    pub slew_rate: Option<f32>,

    /// Detent zones for this axis
    pub detents: Vec<DetentZone>,

    /// Custom response curve points
    pub curve: Option<Vec<CurvePoint>>,

    /// EMA filter configuration for potentiometer noise reduction
    pub filter: Option<FilterConfig>,
}

/// EMA filter configuration for axis noise reduction.
///
/// Used primarily for noisy potentiometers like the B104 in T.Flight HOTAS 4.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FilterConfig {
    /// Smoothing factor [0.0, 1.0] - lower = more smoothing
    pub alpha: f32,

    /// Spike rejection threshold in normalized units (optional)
    pub spike_threshold: Option<f32>,

    /// Maximum consecutive spikes before accepting as real change
    pub max_spike_count: Option<u8>,
}

/// Detent zone definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetentZone {
    /// Center position of detent (-1.0 to 1.0)
    pub position: f32,

    /// Width of detent zone
    pub width: f32,

    /// Semantic role of this detent
    pub role: String,
}

/// Point on a custom response curve
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CurvePoint {
    /// Input value (0.0 to 1.0)
    pub input: f32,

    /// Output value (0.0 to 1.0)
    pub output: f32,
}

/// Phase of Flight overrides
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PofOverrides {
    /// Axis overrides for this phase
    pub axes: Option<HashMap<String, AxisConfig>>,

    /// Hysteresis configuration for phase transitions
    pub hysteresis: Option<HashMap<String, HashMap<String, f32>>>,
}

/// Capability enforcement modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityMode {
    /// Full functionality (default)
    Full,
    /// Demo mode with reduced authority
    Demo,
    /// Kid mode with strict limits
    Kid,
}

/// Capability limits for different modes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityLimits {
    /// Maximum axis output (0.0 to 1.0)
    pub max_axis_output: f32,
    /// Maximum FFB torque in Nm
    pub max_ffb_torque: f32,
    /// Allow high torque mode
    pub allow_high_torque: bool,
    /// Maximum expo value
    pub max_expo: f32,
    /// Maximum slew rate
    pub max_slew_rate: f32,
}

/// Context for capability enforcement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityContext {
    /// Current capability mode
    pub mode: CapabilityMode,
    /// Applied limits
    pub limits: CapabilityLimits,
    /// Enable audit logging
    pub audit_enabled: bool,
}

impl Profile {
    /// Validate the profile against schema and constraints
    pub fn validate(&self) -> Result<()> {
        self.validate_with_capabilities(&CapabilityContext::for_mode(CapabilityMode::Full))
    }

    /// Validate with capability enforcement
    pub fn validate_with_capabilities(&self, context: &CapabilityContext) -> Result<()> {
        // Check schema version
        if self.schema != PROFILE_SCHEMA_VERSION {
            return Err(ProfileError::Validation(format!(
                "Unsupported schema version: {}",
                self.schema
            )));
        }

        // Validate each axis
        for (axis_name, config) in &self.axes {
            self.validate_axis_config(axis_name, config, context)?;
        }

        // Validate PoF overrides if present
        if let Some(pof_overrides) = &self.pof_overrides {
            for (phase_name, overrides) in pof_overrides {
                self.validate_pof_overrides(phase_name, overrides, context)?;
            }
        }

        Ok(())
    }

    /// Canonicalize profile for deterministic hashing
    pub fn canonicalize(&self) -> String {
        // Serialize to a serde_json::Value first (uses f64 internally),
        // then normalize all numeric values to 6 decimal places using f64
        // arithmetic.  This avoids f32 rounding instability that would cause
        // the canonical form to differ across JSON roundtrips.
        let value = serde_json::to_value(self).unwrap_or_default();
        let normalized = normalize_json_floats(value);
        serde_json::to_string(&normalized).unwrap_or_default()
    }

    /// Compute effective profile hash
    pub fn effective_hash(&self) -> String {
        use sha2::{Digest, Sha256};

        let canonical = self.canonicalize();
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Merge this profile with another using last-writer-wins semantics
    pub fn merge_with(&self, other: &Profile) -> Result<Profile> {
        let mut merged = self.clone();

        // Override sim and aircraft if specified in other
        if other.sim.is_some() {
            merged.sim = other.sim.clone();
        }
        if other.aircraft.is_some() {
            merged.aircraft = other.aircraft.clone();
        }

        // Merge axes with last-writer-wins for scalars
        for (axis_name, other_config) in &other.axes {
            let merged_config = if let Some(base_config) = merged.axes.get(axis_name) {
                merge_axis_configs(base_config, other_config)
            } else {
                other_config.clone()
            };
            merged.axes.insert(axis_name.clone(), merged_config);
        }

        // Merge PoF overrides
        if let Some(other_pof) = &other.pof_overrides {
            let merged_pof = merged.pof_overrides.get_or_insert_with(HashMap::new);
            for (phase, overrides) in other_pof {
                merged_pof.insert(phase.clone(), overrides.clone());
            }
        }

        Ok(merged)
    }

    fn validate_axis_config(
        &self,
        axis_name: &str,
        config: &AxisConfig,
        context: &CapabilityContext,
    ) -> Result<()> {
        // Validate deadzone
        if let Some(deadzone) = config.deadzone
            && !(0.0..=MAX_DEADZONE).contains(&deadzone)
        {
            return Err(ProfileError::Validation(format!(
                "axes.{}.deadzone: Deadzone must be between 0.0 and {}",
                axis_name, MAX_DEADZONE
            )));
        }

        // Validate expo with capability limits
        if let Some(expo) = config.expo
            && (expo < 0.0 || expo > context.limits.max_expo)
        {
            return Err(ProfileError::Validation(format!(
                "axes.{}.expo: Expo must be between 0.0 and {} in {:?} mode",
                axis_name, context.limits.max_expo, context.mode
            )));
        }

        // Validate slew rate with capability limits
        if let Some(slew_rate) = config.slew_rate
            && (slew_rate < 0.0 || slew_rate > context.limits.max_slew_rate)
        {
            return Err(ProfileError::Validation(format!(
                "axes.{}.slew_rate: Slew rate must be between 0.0 and {} in {:?} mode",
                axis_name, context.limits.max_slew_rate, context.mode
            )));
        }

        // Validate curve monotonicity
        if let Some(curve) = &config.curve {
            self.validate_curve_monotonic(axis_name, curve)?;
        }

        // Validate detents
        for (i, detent) in config.detents.iter().enumerate() {
            self.validate_detent(axis_name, i, detent)?;
        }

        // Validate filter configuration
        if let Some(filter) = &config.filter {
            self.validate_filter_config(axis_name, filter)?;
        }

        Ok(())
    }

    fn validate_curve_monotonic(&self, axis_name: &str, curve: &[CurvePoint]) -> Result<()> {
        if curve.len() < 2 {
            return Err(ProfileError::Validation(format!(
                "axes.{}.curve: Curve must have at least 2 points",
                axis_name
            )));
        }

        for i in 1..curve.len() {
            if curve[i].input <= curve[i - 1].input {
                return Err(ProfileError::Validation(format!(
                    "axes.{}.curve[{}]: Curve input values must be strictly increasing (monotonic)",
                    axis_name, i
                )));
            }
        }

        Ok(())
    }

    fn validate_detent(&self, axis_name: &str, index: usize, detent: &DetentZone) -> Result<()> {
        if detent.position < -1.0 || detent.position > 1.0 {
            return Err(ProfileError::Validation(format!(
                "axes.{}.detents[{}].position: Detent position must be between -1.0 and 1.0",
                axis_name, index
            )));
        }

        if detent.width <= 0.0 || detent.width > 0.5 {
            return Err(ProfileError::Validation(format!(
                "axes.{}.detents[{}].width: Detent width must be between 0.0 and 0.5",
                axis_name, index
            )));
        }

        Ok(())
    }

    fn validate_filter_config(&self, axis_name: &str, filter: &FilterConfig) -> Result<()> {
        // Validate alpha is in [0.0, 1.0]
        if !(0.0..=1.0).contains(&filter.alpha) {
            return Err(ProfileError::Validation(format!(
                "axes.{}.filter.alpha: Alpha must be between 0.0 and 1.0",
                axis_name
            )));
        }

        // Validate spike_threshold is positive if set
        if let Some(threshold) = filter.spike_threshold
            && (threshold <= 0.0 || threshold > 1.0)
        {
            return Err(ProfileError::Validation(format!(
                "axes.{}.filter.spike_threshold: Spike threshold must be between 0.0 and 1.0",
                axis_name
            )));
        }

        // Validate max_spike_count is reasonable if set
        if let Some(count) = filter.max_spike_count
            && (count == 0 || count > 10)
        {
            return Err(ProfileError::Validation(format!(
                "axes.{}.filter.max_spike_count: Max spike count must be between 1 and 10",
                axis_name
            )));
        }

        Ok(())
    }

    fn validate_pof_overrides(
        &self,
        phase_name: &str,
        overrides: &PofOverrides,
        context: &CapabilityContext,
    ) -> Result<()> {
        if let Some(axes) = &overrides.axes {
            for (axis_name, config) in axes {
                self.validate_axis_config(
                    &format!("pof_overrides.{}.axes.{}", phase_name, axis_name),
                    config,
                    context,
                )?;
            }
        }

        Ok(())
    }

    /// Export the profile to a pretty-printed JSON string.
    pub fn export_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| ProfileError::Validation(e.to_string()))
    }
}

impl CapabilityContext {
    /// Create context for a specific capability mode
    pub fn for_mode(mode: CapabilityMode) -> Self {
        let limits = match mode {
            CapabilityMode::Full => CapabilityLimits {
                max_axis_output: 1.0,
                max_ffb_torque: 50.0,
                allow_high_torque: true,
                max_expo: 1.0,
                max_slew_rate: 100.0,
            },
            CapabilityMode::Demo => CapabilityLimits {
                max_axis_output: 0.8,
                max_ffb_torque: 20.0,
                allow_high_torque: false,
                max_expo: 0.6,
                max_slew_rate: 50.0,
            },
            CapabilityMode::Kid => CapabilityLimits {
                max_axis_output: 0.5,
                max_ffb_torque: 5.0,
                allow_high_torque: false,
                max_expo: 0.3,
                max_slew_rate: 20.0,
            },
        };

        Self {
            mode,
            limits,
            audit_enabled: false,
        }
    }
}

pub fn merge_axis_configs(base: &AxisConfig, override_config: &AxisConfig) -> AxisConfig {
    AxisConfig {
        deadzone: override_config.deadzone.or(base.deadzone),
        expo: override_config.expo.or(base.expo),
        slew_rate: override_config.slew_rate.or(base.slew_rate),
        detents: if override_config.detents.is_empty() {
            base.detents.clone()
        } else {
            override_config.detents.clone()
        },
        curve: override_config.curve.clone().or_else(|| base.curve.clone()),
        filter: override_config
            .filter
            .clone()
            .or_else(|| base.filter.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_validation() {
        let profile = create_valid_profile();
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn test_capability_enforcement() {
        let mut profile = create_valid_profile();

        // Add high expo that should be rejected in kid mode
        if let Some(pitch_config) = profile.axes.get_mut("pitch") {
            pitch_config.expo = Some(0.8);
        }

        let kid_context = CapabilityContext::for_mode(CapabilityMode::Kid);
        assert!(profile.validate_with_capabilities(&kid_context).is_err());

        let full_context = CapabilityContext::for_mode(CapabilityMode::Full);
        assert!(profile.validate_with_capabilities(&full_context).is_ok());
    }

    #[test]
    fn test_profile_canonicalization() {
        let profile = create_valid_profile();
        let canonical1 = profile.canonicalize();
        let canonical2 = profile.canonicalize();
        assert_eq!(canonical1, canonical2);
    }

    #[test]
    fn test_profile_merging() {
        let base = create_valid_profile();
        let override_profile = create_override_profile();

        let merged = base.merge_with(&override_profile).unwrap();

        // Check that override values are applied
        let pitch_config = merged.axes.get("pitch").unwrap();
        assert_eq!(pitch_config.expo, Some(0.5)); // From override
        assert_eq!(pitch_config.deadzone, Some(0.03)); // From base
    }

    fn create_valid_profile() -> Profile {
        let mut axes = HashMap::new();
        axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.03),
                expo: Some(0.2),
                slew_rate: Some(1.2),
                detents: vec![],
                curve: None,
                filter: None,
            },
        );

        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId {
                icao: "C172".to_string(),
            }),
            axes,
            pof_overrides: None,
        }
    }

    fn create_override_profile() -> Profile {
        let mut axes = HashMap::new();
        axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: None,  // Keep base value
                expo: Some(0.5), // Override
                slew_rate: None, // Keep base value
                detents: vec![],
                curve: None,
                filter: None,
            },
        );

        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes,
            pof_overrides: None,
        }
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_profile_roundtrip(
            deadzone in 0.0f32..0.5,
            expo in 0.0f32..1.0,
            slew_rate in 0.0f32..100.0,
            axis_name in "[a-z]+"
        ) {
            let mut axes = HashMap::new();
            axes.insert(
                axis_name,
                AxisConfig {
                    deadzone: Some(deadzone),
                    expo: Some(expo),
                    slew_rate: Some(slew_rate),
                    detents: vec![],
                    curve: None,
                    filter: None,
                },
            );

            let profile = Profile {
                schema: PROFILE_SCHEMA_VERSION.to_string(),
                sim: Some("msfs".to_string()),
                aircraft: Some(AircraftId {
                    icao: "C172".to_string(),
                }),
                axes,
                pof_overrides: None,
            };

            // Should be valid
            prop_assert!(profile.validate().is_ok());

            // Canonicalization round trip
            let canonical = profile.canonicalize();
            let parsed: Profile = serde_json::from_str(&canonical).unwrap();

            // Note: Floating point comparison issues may arise if we compare directly
            // but the canonicalize fn rounds to 6 decimals, so it should be stable.
            // We verify the hash is stable.
            prop_assert_eq!(profile.effective_hash(), parsed.effective_hash());
        }

        #[test]
        fn prop_capability_enforcement_kid_mode(
            expo in 0.31f32..1.0, // Values > 0.3 should fail in Kid mode
        ) {
            let mut axes = HashMap::new();
            axes.insert(
                "test_axis".to_string(),
                AxisConfig {
                    deadzone: None,
                    expo: Some(expo),
                    slew_rate: None,
                    detents: vec![],
                    curve: None,
                    filter: None,
                },
            );

            let profile = Profile {
                schema: PROFILE_SCHEMA_VERSION.to_string(),
                sim: None,
                aircraft: None,
                axes,
                pof_overrides: None,
            };

            let kid_context = CapabilityContext::for_mode(CapabilityMode::Kid);
            prop_assert!(profile.validate_with_capabilities(&kid_context).is_err());
        }

        // Prop: merging a profile with itself is idempotent (result equals the original)
        #[test]
        fn prop_merge_with_self_is_idempotent(
            deadzone in 0.0f32..0.5f32,
            expo in 0.0f32..1.0f32,
            axis_name in "[a-z]+",
        ) {
            let mut axes = HashMap::new();
            axes.insert(
                axis_name,
                AxisConfig {
                    deadzone: Some(deadzone),
                    expo: Some(expo),
                    slew_rate: None,
                    detents: vec![],
                    curve: None,
                    filter: None,
                },
            );
            let profile = Profile {
                schema: PROFILE_SCHEMA_VERSION.to_string(),
                sim: Some("msfs".to_string()),
                aircraft: None,
                axes,
                pof_overrides: None,
            };
            let merged = profile.merge_with(&profile).unwrap();
            prop_assert_eq!(profile, merged);
        }

        // Prop: any normal f32 deadzone that passes validation is within [0.0, 0.5]
        #[test]
        fn prop_deadzone_clamped_by_validation(d in proptest::num::f32::NORMAL) {
            let mut axes = HashMap::new();
            axes.insert(
                "test".to_string(),
                AxisConfig {
                    deadzone: Some(d),
                    expo: None,
                    slew_rate: None,
                    detents: vec![],
                    curve: None,
                    filter: None,
                },
            );
            let profile = Profile {
                schema: PROFILE_SCHEMA_VERSION.to_string(),
                sim: None,
                aircraft: None,
                axes,
                pof_overrides: None,
            };
            if profile.validate().is_ok() {
                prop_assert!(d >= 0.0 && d <= MAX_DEADZONE,
                    "validation passed but deadzone {} is outside [0.0, {}]", d, MAX_DEADZONE);
            }
        }

        // Prop: any normal f32 expo that passes validation is within [0.0, 1.0]
        #[test]
        fn prop_expo_clamped_by_validation(e in proptest::num::f32::NORMAL) {
            let mut axes = HashMap::new();
            axes.insert(
                "test".to_string(),
                AxisConfig {
                    deadzone: None,
                    expo: Some(e),
                    slew_rate: None,
                    detents: vec![],
                    curve: None,
                    filter: None,
                },
            );
            let profile = Profile {
                schema: PROFILE_SCHEMA_VERSION.to_string(),
                sim: None,
                aircraft: None,
                axes,
                pof_overrides: None,
            };
            if profile.validate().is_ok() {
                prop_assert!(e >= 0.0 && e <= MAX_EXPO,
                    "validation passed but expo {} is outside [0.0, {}]", e, MAX_EXPO);
            }
        }
    }

    // ── validation edge cases ─────────────────────────────────────────────────

    #[test]
    fn test_deadzone_out_of_range_rejected() {
        let mut profile = create_valid_profile();
        profile.axes.get_mut("pitch").unwrap().deadzone = Some(0.6); // > MAX_DEADZONE (0.5)
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_negative_deadzone_rejected() {
        let mut profile = create_valid_profile();
        profile.axes.get_mut("pitch").unwrap().deadzone = Some(-0.01);
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_negative_slew_rate_rejected() {
        let mut profile = create_valid_profile();
        profile.axes.get_mut("pitch").unwrap().slew_rate = Some(-1.0);
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_curve_too_few_points_rejected() {
        let mut profile = create_valid_profile();
        profile.axes.get_mut("pitch").unwrap().curve = Some(vec![CurvePoint {
            input: 0.0,
            output: 0.0,
        }]);
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_curve_non_monotonic_rejected() {
        let mut profile = create_valid_profile();
        profile.axes.get_mut("pitch").unwrap().curve = Some(vec![
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 0.5,
                output: 0.5,
            },
            CurvePoint {
                input: 0.3,
                output: 0.8,
            }, // non-monotonic input
        ]);
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_curve_valid_accepted() {
        let mut profile = create_valid_profile();
        profile.axes.get_mut("pitch").unwrap().curve = Some(vec![
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 0.5,
                output: 0.4,
            },
            CurvePoint {
                input: 1.0,
                output: 1.0,
            },
        ]);
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn test_detent_out_of_range_rejected() {
        let mut profile = create_valid_profile();
        profile.axes.get_mut("pitch").unwrap().detents = vec![DetentZone {
            position: 1.5, // > 1.0
            width: 0.1,
            role: "idle".to_string(),
        }];
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_detent_zero_width_rejected() {
        let mut profile = create_valid_profile();
        profile.axes.get_mut("pitch").unwrap().detents = vec![DetentZone {
            position: 0.0,
            width: 0.0, // must be > 0
            role: "idle".to_string(),
        }];
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_detent_valid_accepted() {
        let mut profile = create_valid_profile();
        profile.axes.get_mut("pitch").unwrap().detents = vec![DetentZone {
            position: 0.0,
            width: 0.1,
            role: "idle".to_string(),
        }];
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn test_filter_alpha_out_of_range_rejected() {
        let mut profile = create_valid_profile();
        profile.axes.get_mut("pitch").unwrap().filter = Some(FilterConfig {
            alpha: 1.5, // > 1.0
            spike_threshold: None,
            max_spike_count: None,
        });
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_filter_valid_accepted() {
        let mut profile = create_valid_profile();
        profile.axes.get_mut("pitch").unwrap().filter = Some(FilterConfig {
            alpha: 0.3,
            spike_threshold: Some(0.05),
            max_spike_count: Some(3),
        });
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn test_effective_hash_deterministic() {
        let profile = create_valid_profile();
        let hash1 = profile.effective_hash();
        let hash2 = profile.effective_hash();
        assert_eq!(hash1, hash2, "hash must be deterministic");
        assert_eq!(hash1.len(), 64, "SHA-256 hex string must be 64 chars");
    }

    #[test]
    fn test_different_profiles_different_hashes() {
        let profile1 = create_valid_profile();
        let mut profile2 = create_valid_profile();
        profile2.axes.get_mut("pitch").unwrap().expo = Some(0.9);
        assert_ne!(profile1.effective_hash(), profile2.effective_hash());
    }

    #[test]
    fn test_pof_override_merged_on_merge() {
        let base = create_valid_profile();
        let mut overr = create_valid_profile();

        let phase_axes = {
            let mut m = HashMap::new();
            m.insert(
                "pitch".to_string(),
                AxisConfig {
                    deadzone: Some(0.05),
                    expo: Some(0.1),
                    slew_rate: None,
                    detents: vec![],
                    curve: None,
                    filter: None,
                },
            );
            m
        };
        overr.pof_overrides = Some({
            let mut pof = HashMap::new();
            pof.insert(
                "climb".to_string(),
                PofOverrides {
                    axes: Some(phase_axes),
                    hysteresis: None,
                },
            );
            pof
        });

        let merged = base.merge_with(&overr).unwrap();
        assert!(
            merged.pof_overrides.is_some(),
            "PoF overrides should be present after merge"
        );
        assert!(merged.pof_overrides.as_ref().unwrap().contains_key("climb"));
    }

    #[test]
    fn test_merge_axis_configs_direct() {
        let base = AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.2),
            slew_rate: Some(1.0),
            detents: vec![],
            curve: None,
            filter: None,
        };
        let overr = AxisConfig {
            deadzone: None,  // should keep base
            expo: Some(0.5), // should override
            slew_rate: None, // should keep base
            detents: vec![],
            curve: None,
            filter: None,
        };
        let merged = merge_axis_configs(&base, &overr);
        assert_eq!(merged.deadzone, Some(0.03), "base deadzone preserved");
        assert_eq!(merged.expo, Some(0.5), "override expo applied");
        assert_eq!(merged.slew_rate, Some(1.0), "base slew_rate preserved");
    }

    #[test]
    fn snapshot_canonical_form() {
        let profile = create_valid_profile();
        insta::assert_snapshot!("profile_canonical_form", profile.canonicalize());
    }

    #[test]
    fn snapshot_merged_profile() {
        let base = create_valid_profile();
        let override_profile = create_override_profile();
        let merged = base.merge_with(&override_profile).unwrap();
        insta::assert_snapshot!("profile_merged", merged.canonicalize());
    }

    #[test]
    fn snapshot_validation_error_bad_schema() {
        let mut profile = create_valid_profile();
        profile.schema = "flight.profile/99".to_string();
        let err = profile.validate().unwrap_err();
        insta::assert_debug_snapshot!("profile_validation_error_schema", err);
    }

    /// Snapshot a Profile deserialized from minimal valid YAML.
    #[test]
    fn snapshot_profile_debug_from_yaml() {
        let yaml = "schema: \"flight.profile/1\"\nsim: msfs\naxes:\n  pitch:\n    deadzone: 0.05\n    expo: 0.3\n    slew_rate: 2.0\n    detents: []\n";
        let profile: Profile = serde_yaml::from_str(yaml).expect("YAML should deserialize");
        insta::assert_debug_snapshot!("profile_debug_from_yaml", profile);
    }

    /// Snapshot the deserialization error when the required `schema` field is absent.
    #[test]
    fn snapshot_validation_error_missing_schema_field() {
        let yaml = "sim: msfs\naxes: {}\n";
        let err = serde_yaml::from_str::<Profile>(yaml).unwrap_err();
        insta::assert_snapshot!("validation_error_missing_schema_field", err.to_string());
    }

    /// YAML round-trip snapshot: Profile → YAML output shape.
    /// Catches regressions in field names and serialization format.
    #[test]
    fn snapshot_profile_yaml_round_trip() {
        let profile = create_valid_profile();
        insta::assert_yaml_snapshot!("profile_yaml_round_trip", profile);
    }

    /// YAML snapshot of a Profile with all optional fields populated.
    #[test]
    fn snapshot_profile_full_fields_yaml() {
        let mut axes = HashMap::new();
        axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.03),
                expo: Some(0.2),
                slew_rate: Some(1.2),
                detents: vec![DetentZone {
                    position: 0.0,
                    width: 0.05,
                    role: "center".to_string(),
                }],
                curve: Some(vec![
                    CurvePoint {
                        input: 0.0,
                        output: 0.0,
                    },
                    CurvePoint {
                        input: 0.5,
                        output: 0.35,
                    },
                    CurvePoint {
                        input: 1.0,
                        output: 1.0,
                    },
                ]),
                filter: Some(FilterConfig {
                    alpha: 0.3,
                    spike_threshold: Some(0.05),
                    max_spike_count: Some(3),
                }),
            },
        );
        let profile = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId {
                icao: "A320".to_string(),
            }),
            axes,
            pof_overrides: None,
        };
        insta::assert_yaml_snapshot!("profile_full_fields_yaml", profile);
    }

    /// JSON snapshot of the capability manifest for each mode.
    /// Catches regressions in limit values and field structure.
    #[test]
    fn snapshot_capability_manifest_full() {
        let ctx = CapabilityContext::for_mode(CapabilityMode::Full);
        insta::assert_json_snapshot!("capability_manifest_full", ctx);
    }

    #[test]
    fn snapshot_capability_manifest_demo() {
        let ctx = CapabilityContext::for_mode(CapabilityMode::Demo);
        insta::assert_json_snapshot!("capability_manifest_demo", ctx);
    }

    #[test]
    fn snapshot_capability_manifest_kid() {
        let ctx = CapabilityContext::for_mode(CapabilityMode::Kid);
        insta::assert_json_snapshot!("capability_manifest_kid", ctx);
    }

    // ── export_json tests ─────────────────────────────────────────────────────

    #[test]
    fn test_profile_export_produces_valid_json() {
        let profile = create_valid_profile();
        let json = profile.export_json().expect("export_json should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("exported JSON must be valid");
        assert!(parsed.is_object());
    }

    #[test]
    fn test_profile_export_roundtrip() {
        let profile = create_valid_profile();
        let json = profile.export_json().expect("export_json should succeed");
        let restored: Profile =
            serde_json::from_str(&json).expect("should deserialize back to Profile");
        assert_eq!(profile, restored);
    }

    #[test]
    fn test_profile_export_snapshot() {
        let profile = create_valid_profile();
        let json = profile.export_json().expect("export_json should succeed");
        insta::assert_snapshot!("profile_export_snapshot", json);
    }
}
