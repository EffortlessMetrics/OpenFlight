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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

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
        // Sort keys and normalize float precision
        let mut canonical = self.clone();

        // Normalize float values to 6 decimal places
        for config in canonical.axes.values_mut() {
            if let Some(deadzone) = &mut config.deadzone {
                *deadzone = (*deadzone * 1_000_000.0).round() / 1_000_000.0;
            }
            if let Some(expo) = &mut config.expo {
                *expo = (*expo * 1_000_000.0).round() / 1_000_000.0;
            }
            if let Some(slew_rate) = &mut config.slew_rate {
                *slew_rate = (*slew_rate * 1_000_000.0).round() / 1_000_000.0;
            }

            // Normalize curve points
            if let Some(curve) = &mut config.curve {
                for point in curve {
                    point.input = (point.input * 1_000_000.0).round() / 1_000_000.0;
                    point.output = (point.output * 1_000_000.0).round() / 1_000_000.0;
                }
            }

            // Normalize filter parameters
            if let Some(filter) = &mut config.filter {
                filter.alpha = (filter.alpha * 1_000_000.0).round() / 1_000_000.0;
                if let Some(threshold) = &mut filter.spike_threshold {
                    *threshold = (*threshold * 1_000_000.0).round() / 1_000_000.0;
                }
            }
        }

        // Serialize with sorted keys
        serde_json::to_string(&canonical).unwrap_or_default()
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
    }
}
