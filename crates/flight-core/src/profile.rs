//! Profile management and validation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use indexmap::IndexMap;
use sha2::{Sha256, Digest};
use crate::{FlightError, Result};

/// Flight profile schema version 1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub schema: String,
    pub sim: Option<String>,
    pub aircraft: Option<AircraftId>,
    pub axes: HashMap<String, AxisConfig>,
    pub pof_overrides: Option<HashMap<String, PofOverride>>,
}

/// Capability enforcement modes for safety
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityMode {
    /// Full capabilities - no restrictions
    Full,
    /// Demo mode - reduced force feedback and limited axis response
    Demo,
    /// Kid mode - heavily restricted for safety
    Kid,
}

impl Default for CapabilityMode {
    fn default() -> Self {
        Self::Full
    }
}

/// Capability limits for different modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityLimits {
    /// Maximum axis output magnitude (0.0-1.0)
    pub max_axis_output: f32,
    /// Maximum force feedback torque (Nm)
    pub max_ffb_torque: f32,
    /// Maximum slew rate (units/second)
    pub max_slew_rate: f32,
    /// Maximum curve expo magnitude
    pub max_curve_expo: f32,
    /// Whether high torque mode is allowed
    pub allow_high_torque: bool,
    /// Whether custom curves are allowed
    pub allow_custom_curves: bool,
}

impl CapabilityLimits {
    /// Get limits for a specific capability mode
    pub fn for_mode(mode: CapabilityMode) -> Self {
        match mode {
            CapabilityMode::Full => Self {
                max_axis_output: 1.0,
                max_ffb_torque: 50.0, // Reasonable max for consumer devices
                max_slew_rate: 100.0,
                max_curve_expo: 1.0,
                allow_high_torque: true,
                allow_custom_curves: true,
            },
            CapabilityMode::Demo => Self {
                max_axis_output: 0.8, // 80% max output
                max_ffb_torque: 15.0, // Reduced torque
                max_slew_rate: 5.0,   // Slower response
                max_curve_expo: 0.5,  // Limited curve strength
                allow_high_torque: false,
                allow_custom_curves: true,
            },
            CapabilityMode::Kid => Self {
                max_axis_output: 0.5, // 50% max output
                max_ffb_torque: 5.0,  // Very low torque
                max_slew_rate: 2.0,   // Very slow response
                max_curve_expo: 0.2,  // Minimal curve strength
                allow_high_torque: false,
                allow_custom_curves: false,
            },
        }
    }
}

/// Capability enforcement context
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CapabilityContext {
    pub mode: CapabilityMode,
    pub limits: CapabilityLimits,
    pub audit_enabled: bool,
}

impl Default for CapabilityContext {
    fn default() -> Self {
        Self {
            mode: CapabilityMode::Full,
            limits: CapabilityLimits::for_mode(CapabilityMode::Full),
            audit_enabled: true,
        }
    }
}

impl CapabilityContext {
    /// Create context for specific mode
    pub fn for_mode(mode: CapabilityMode) -> Self {
        Self {
            mode,
            limits: CapabilityLimits::for_mode(mode),
            audit_enabled: true,
        }
    }

    /// Set audit logging enabled/disabled
    pub fn with_audit(mut self, enabled: bool) -> Self {
        self.audit_enabled = enabled;
        self
    }

    /// Get human-readable mode name
    pub fn mode_name(&self) -> &'static str {
        match self.mode {
            CapabilityMode::Full => "full",
            CapabilityMode::Demo => "demo",
            CapabilityMode::Kid => "kid",
        }
    }
}

/// Canonical representation of a profile for deterministic hashing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalProfile {
    pub schema: String,
    pub sim: Option<String>,
    pub aircraft: Option<AircraftId>,
    pub axes: IndexMap<String, CanonicalAxisConfig>,
    pub pof_overrides: Option<IndexMap<String, CanonicalPofOverride>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AircraftId {
    pub icao: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisConfig {
    pub deadzone: Option<f32>,
    pub expo: Option<f32>,
    pub slew_rate: Option<f32>,
    pub detents: Vec<DetentZone>,
    pub curve: Option<Vec<CurvePoint>>,
}

/// Canonical axis configuration with normalized values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalAxisConfig {
    pub deadzone: Option<f64>,
    pub expo: Option<f64>,
    pub slew_rate: Option<f64>,
    pub detents: Vec<CanonicalDetentZone>,
    pub curve: Option<Vec<CanonicalCurvePoint>>,
}

/// A point on a response curve
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurvePoint {
    pub input: f32,
    pub output: f32,
}

/// Canonical curve point with normalized precision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalCurvePoint {
    pub input: f64,
    pub output: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetentZone {
    pub position: f32,
    pub width: f32,
    pub role: String,
}

/// Canonical detent zone with normalized precision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalDetentZone {
    pub position: f64,
    pub width: f64,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PofOverride {
    pub axes: Option<HashMap<String, AxisConfig>>,
    pub hysteresis: Option<HashMap<String, HysteresisConfig>>,
}

/// Canonical phase of flight override
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalPofOverride {
    pub axes: Option<IndexMap<String, CanonicalAxisConfig>>,
    pub hysteresis: Option<IndexMap<String, CanonicalHysteresisConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HysteresisConfig {
    pub enter: HashMap<String, f32>,
    pub exit: HashMap<String, f32>,
}

/// Canonical hysteresis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalHysteresisConfig {
    pub enter: IndexMap<String, f64>,
    pub exit: IndexMap<String, f64>,
}

impl Profile {
    /// Validate profile schema and constraints
    pub fn validate(&self) -> Result<()> {
        self.validate_with_location().map(|_| ())
    }

    /// Validate profile with capability enforcement
    pub fn validate_with_capabilities(&self, context: &CapabilityContext) -> Result<()> {
        self.validate_with_location_and_capabilities(context).map(|_| ())
    }

    /// Validate profile with line/column error reporting
    pub fn validate_with_location(&self) -> Result<ValidationResult> {
        self.validate_with_location_and_capabilities(&CapabilityContext::default())
    }

    /// Validate profile with line/column error reporting and capability enforcement
    pub fn validate_with_location_and_capabilities(&self, context: &CapabilityContext) -> Result<ValidationResult> {
        let mut result = ValidationResult::new();

        if self.schema != "flight.profile/1" {
            result.add_error(ValidationError {
                line: 1,
                column: 1,
                message: format!("Unsupported schema version: {}", self.schema),
            });
            return Ok(result);
        }

        // Validate axis configurations
        for (axis_name, config) in &self.axes {
            if let Err(errors) = self.validate_axis_config_with_capabilities(axis_name, config, context) {
                result.errors.extend(errors);
            }
        }

        if result.has_errors() {
            Err(FlightError::ProfileValidation(result.to_string()))
        } else {
            Ok(result)
        }
    }

    fn validate_axis_config(&self, name: &str, config: &AxisConfig) -> std::result::Result<(), Vec<ValidationError>> {
        self.validate_axis_config_with_capabilities(name, config, &CapabilityContext::default())
    }

    fn validate_axis_config_with_capabilities(&self, name: &str, config: &AxisConfig, context: &CapabilityContext) -> std::result::Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Validate deadzone range
        if let Some(deadzone) = config.deadzone {
            if !(0.0..=1.0).contains(&deadzone) {
                errors.push(ValidationError {
                    line: 0, // TODO: Extract from JSON parsing
                    column: 0,
                    message: format!("Axis '{}': deadzone must be between 0.0 and 1.0", name),
                });
            }
        }

        // Validate expo range and capability limits
        if let Some(expo) = config.expo {
            if !(-1.0..=1.0).contains(&expo) {
                errors.push(ValidationError {
                    line: 0,
                    column: 0,
                    message: format!("Axis '{}': expo must be between -1.0 and 1.0", name),
                });
            }
            
            // Check capability limits
            if expo.abs() > context.limits.max_curve_expo {
                errors.push(ValidationError {
                    line: 0,
                    column: 0,
                    message: format!(
                        "Axis '{}': expo magnitude {} exceeds {} mode limit of {}",
                        name, expo.abs(), context.mode_name(), context.limits.max_curve_expo
                    ),
                });
            }
        }

        // Validate slew rate capability limits
        if let Some(slew_rate) = config.slew_rate {
            if slew_rate > context.limits.max_slew_rate {
                errors.push(ValidationError {
                    line: 0,
                    column: 0,
                    message: format!(
                        "Axis '{}': slew rate {} exceeds {} mode limit of {}",
                        name, slew_rate, context.mode_name(), context.limits.max_slew_rate
                    ),
                });
            }
        }

        // Validate curve capability limits
        if let Some(curve) = &config.curve {
            if !context.limits.allow_custom_curves {
                errors.push(ValidationError {
                    line: 0,
                    column: 0,
                    message: format!(
                        "Axis '{}': custom curves not allowed in {} mode",
                        name, context.mode_name()
                    ),
                });
            } else if let Err(msg) = validate_curve_monotonic(curve) {
                errors.push(ValidationError {
                    line: 0,
                    column: 0,
                    message: format!("Axis '{}': {}", name, msg),
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Convert to canonical form for deterministic hashing
    pub fn canonicalize(&self) -> CanonicalProfile {
        let mut canonical_axes = IndexMap::new();
        for (name, config) in &self.axes {
            canonical_axes.insert(name.clone(), config.canonicalize());
        }
        canonical_axes.sort_keys();

        let canonical_pof_overrides = self.pof_overrides.as_ref().map(|overrides| {
            let mut canonical_overrides = IndexMap::new();
            for (name, override_config) in overrides {
                canonical_overrides.insert(name.clone(), override_config.canonicalize());
            }
            canonical_overrides.sort_keys();
            canonical_overrides
        });

        CanonicalProfile {
            schema: self.schema.clone(),
            sim: self.sim.clone(),
            aircraft: self.aircraft.clone(),
            axes: canonical_axes,
            pof_overrides: canonical_pof_overrides,
        }
    }

    /// Compute effective profile hash
    pub fn effective_hash(&self) -> String {
        let canonical = self.canonicalize();
        let json = serde_json::to_string(&canonical).expect("Canonical profile should serialize");
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Merge profiles with deterministic hierarchy: Global → Sim → Aircraft → Phase of Flight
    pub fn merge(profiles: &[&Profile]) -> Result<Profile> {
        if profiles.is_empty() {
            return Err(FlightError::ProfileValidation("No profiles to merge".to_string()));
        }

        let mut merged = profiles[0].clone();

        for profile in profiles.iter().skip(1) {
            merged = merged.merge_with(profile)?;
        }

        Ok(merged)
    }

    /// Merge this profile with another (other takes precedence for scalars)
    pub fn merge_with(&self, other: &Profile) -> Result<Profile> {
        let mut result = self.clone();

        // Schema must match
        if self.schema != other.schema {
            return Err(FlightError::ProfileValidation(
                "Cannot merge profiles with different schema versions".to_string()
            ));
        }

        // Last-writer-wins for scalars
        if other.sim.is_some() {
            result.sim = other.sim.clone();
        }
        if other.aircraft.is_some() {
            result.aircraft = other.aircraft.clone();
        }

        // Keyed merge for axes
        for (axis_name, other_config) in &other.axes {
            if let Some(existing_config) = result.axes.get(axis_name) {
                result.axes.insert(axis_name.clone(), existing_config.merge_with(other_config));
            } else {
                result.axes.insert(axis_name.clone(), other_config.clone());
            }
        }

        // Merge POF overrides
        if let Some(other_pof) = &other.pof_overrides {
            let mut merged_pof = result.pof_overrides.unwrap_or_default();
            for (pof_name, other_override) in other_pof {
                if let Some(existing_override) = merged_pof.get(pof_name) {
                    merged_pof.insert(pof_name.clone(), existing_override.merge_with(other_override));
                } else {
                    merged_pof.insert(pof_name.clone(), other_override.clone());
                }
            }
            result.pof_overrides = Some(merged_pof);
        }

        Ok(result)
    }
}

impl AxisConfig {
    /// Convert to canonical form with normalized precision
    pub fn canonicalize(&self) -> CanonicalAxisConfig {
        CanonicalAxisConfig {
            deadzone: self.deadzone.map(normalize_float),
            expo: self.expo.map(normalize_float),
            slew_rate: self.slew_rate.map(normalize_float),
            detents: self.detents.iter().map(|d| d.canonicalize()).collect(),
            curve: self.curve.as_ref().map(|curve| {
                curve.iter().map(|p| p.canonicalize()).collect()
            }),
        }
    }

    /// Merge with another axis config (other takes precedence for scalars)
    pub fn merge_with(&self, other: &AxisConfig) -> AxisConfig {
        AxisConfig {
            deadzone: other.deadzone.or(self.deadzone),
            expo: other.expo.or(self.expo),
            slew_rate: other.slew_rate.or(self.slew_rate),
            detents: if other.detents.is_empty() {
                self.detents.clone()
            } else {
                other.detents.clone()
            },
            curve: other.curve.clone().or_else(|| self.curve.clone()),
        }
    }
}

impl DetentZone {
    /// Convert to canonical form
    pub fn canonicalize(&self) -> CanonicalDetentZone {
        CanonicalDetentZone {
            position: normalize_float(self.position),
            width: normalize_float(self.width),
            role: self.role.clone(),
        }
    }
}

impl CurvePoint {
    /// Convert to canonical form
    pub fn canonicalize(&self) -> CanonicalCurvePoint {
        CanonicalCurvePoint {
            input: normalize_float(self.input),
            output: normalize_float(self.output),
        }
    }
}

impl PofOverride {
    /// Convert to canonical form
    pub fn canonicalize(&self) -> CanonicalPofOverride {
        let canonical_axes = self.axes.as_ref().map(|axes| {
            let mut canonical = IndexMap::new();
            for (name, config) in axes {
                canonical.insert(name.clone(), config.canonicalize());
            }
            canonical.sort_keys();
            canonical
        });

        let canonical_hysteresis = self.hysteresis.as_ref().map(|hyst| {
            let mut canonical = IndexMap::new();
            for (name, config) in hyst {
                canonical.insert(name.clone(), config.canonicalize());
            }
            canonical.sort_keys();
            canonical
        });

        CanonicalPofOverride {
            axes: canonical_axes,
            hysteresis: canonical_hysteresis,
        }
    }

    /// Merge with another POF override
    pub fn merge_with(&self, other: &PofOverride) -> PofOverride {
        let merged_axes = match (&self.axes, &other.axes) {
            (Some(self_axes), Some(other_axes)) => {
                let mut merged = self_axes.clone();
                for (axis_name, other_config) in other_axes {
                    if let Some(existing_config) = merged.get(axis_name) {
                        merged.insert(axis_name.clone(), existing_config.merge_with(other_config));
                    } else {
                        merged.insert(axis_name.clone(), other_config.clone());
                    }
                }
                Some(merged)
            }
            (None, Some(other_axes)) => Some(other_axes.clone()),
            (Some(self_axes), None) => Some(self_axes.clone()),
            (None, None) => None,
        };

        let merged_hysteresis = match (&self.hysteresis, &other.hysteresis) {
            (Some(self_hyst), Some(other_hyst)) => {
                let mut merged = self_hyst.clone();
                for (name, other_config) in other_hyst {
                    if let Some(existing_config) = merged.get(name) {
                        merged.insert(name.clone(), existing_config.merge_with(other_config));
                    } else {
                        merged.insert(name.clone(), other_config.clone());
                    }
                }
                Some(merged)
            }
            (None, Some(other_hyst)) => Some(other_hyst.clone()),
            (Some(self_hyst), None) => Some(self_hyst.clone()),
            (None, None) => None,
        };

        PofOverride {
            axes: merged_axes,
            hysteresis: merged_hysteresis,
        }
    }
}

impl HysteresisConfig {
    /// Convert to canonical form
    pub fn canonicalize(&self) -> CanonicalHysteresisConfig {
        let mut canonical_enter = IndexMap::new();
        for (key, value) in &self.enter {
            canonical_enter.insert(key.clone(), normalize_float(*value));
        }
        canonical_enter.sort_keys();

        let mut canonical_exit = IndexMap::new();
        for (key, value) in &self.exit {
            canonical_exit.insert(key.clone(), normalize_float(*value));
        }
        canonical_exit.sort_keys();

        CanonicalHysteresisConfig {
            enter: canonical_enter,
            exit: canonical_exit,
        }
    }

    /// Merge with another hysteresis config
    pub fn merge_with(&self, other: &HysteresisConfig) -> HysteresisConfig {
        let mut merged_enter = self.enter.clone();
        for (key, value) in &other.enter {
            merged_enter.insert(key.clone(), *value);
        }

        let mut merged_exit = self.exit.clone();
        for (key, value) in &other.exit {
            merged_exit.insert(key.clone(), *value);
        }

        HysteresisConfig {
            enter: merged_enter,
            exit: merged_exit,
        }
    }
}

/// Validation result with line/column error reporting
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
}

/// Validation error with location information
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
        }
    }

    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

impl std::fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for error in &self.errors {
            writeln!(f, "Line {}, Column {}: {}", error.line, error.column, error.message)?;
        }
        Ok(())
    }
}

/// Normalize float to 1e-6 precision for deterministic hashing
fn normalize_float(value: f32) -> f64 {
    let normalized = (value as f64 * 1_000_000.0).round() / 1_000_000.0;
    normalized
}

/// Validate that a curve is monotonic
fn validate_curve_monotonic(curve: &[CurvePoint]) -> std::result::Result<(), String> {
    if curve.len() < 2 {
        return Ok(());
    }

    for window in curve.windows(2) {
        let current = &window[0];
        let next = &window[1];
        
        if next.input <= current.input {
            return Err(format!(
                "Curve is not monotonic: input {} <= {} at points ({}, {}) -> ({}, {})",
                next.input, current.input, current.input, current.output, next.input, next.output
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_profile_validation() {
        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId { icao: "C172".to_string() }),
            axes: HashMap::new(),
            pof_overrides: None,
        };

        assert!(profile.validate().is_ok());
    }

    #[test]
    fn test_invalid_schema_version() {
        let profile = Profile {
            schema: "flight.profile/2".to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };

        let result = profile.validate_with_location();
        assert!(result.is_ok()); // validate_with_location returns Ok(ValidationResult) even with errors
        let validation_result = result.unwrap();
        assert!(validation_result.has_errors());
    }

    #[test]
    fn test_curve_monotonicity_validation() {
        let monotonic_curve = vec![
            CurvePoint { input: 0.0, output: 0.0 },
            CurvePoint { input: 0.5, output: 0.3 },
            CurvePoint { input: 1.0, output: 1.0 },
        ];

        let non_monotonic_curve = vec![
            CurvePoint { input: 0.0, output: 0.0 },
            CurvePoint { input: 0.7, output: 0.3 },
            CurvePoint { input: 0.5, output: 0.8 }, // Input decreases
        ];

        assert!(validate_curve_monotonic(&monotonic_curve).is_ok());
        assert!(validate_curve_monotonic(&non_monotonic_curve).is_err());
    }

    #[test]
    fn test_profile_canonicalization() {
        let mut axes = HashMap::new();
        axes.insert("pitch".to_string(), AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.2),
            slew_rate: Some(1.2),
            detents: vec![],
            curve: None,
        });

        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId { icao: "C172".to_string() }),
            axes,
            pof_overrides: None,
        };

        let canonical = profile.canonicalize();
        assert_eq!(canonical.schema, "flight.profile/1");
        assert!(canonical.axes.contains_key("pitch"));
    }

    #[test]
    fn test_deterministic_hash() {
        let mut axes = HashMap::new();
        axes.insert("pitch".to_string(), AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.2),
            slew_rate: Some(1.2),
            detents: vec![],
            curve: None,
        });

        let profile1 = Profile {
            schema: "flight.profile/1".to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId { icao: "C172".to_string() }),
            axes: axes.clone(),
            pof_overrides: None,
        };

        let profile2 = Profile {
            schema: "flight.profile/1".to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId { icao: "C172".to_string() }),
            axes,
            pof_overrides: None,
        };

        assert_eq!(profile1.effective_hash(), profile2.effective_hash());
    }

    #[test]
    fn test_profile_merge() {
        let mut base_axes = HashMap::new();
        base_axes.insert("pitch".to_string(), AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.2),
            slew_rate: None,
            detents: vec![],
            curve: None,
        });

        let base_profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: Some("msfs".to_string()),
            aircraft: None,
            axes: base_axes,
            pof_overrides: None,
        };

        let mut override_axes = HashMap::new();
        override_axes.insert("pitch".to_string(), AxisConfig {
            deadzone: None,
            expo: Some(0.3), // Override expo
            slew_rate: Some(1.5), // Add slew rate
            detents: vec![],
            curve: None,
        });

        let override_profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: None,
            aircraft: Some(AircraftId { icao: "C172".to_string() }),
            axes: override_axes,
            pof_overrides: None,
        };

        let merged = base_profile.merge_with(&override_profile).unwrap();
        
        assert_eq!(merged.sim, Some("msfs".to_string())); // From base
        assert_eq!(merged.aircraft, Some(AircraftId { icao: "C172".to_string() })); // From override
        
        let pitch_config = merged.axes.get("pitch").unwrap();
        assert_eq!(pitch_config.deadzone, Some(0.03)); // From base
        assert_eq!(pitch_config.expo, Some(0.3)); // From override
        assert_eq!(pitch_config.slew_rate, Some(1.5)); // From override
    }

    #[test]
    fn test_capability_limits() {
        let full_limits = CapabilityLimits::for_mode(CapabilityMode::Full);
        let demo_limits = CapabilityLimits::for_mode(CapabilityMode::Demo);
        let kid_limits = CapabilityLimits::for_mode(CapabilityMode::Kid);

        // Full mode should have highest limits
        assert_eq!(full_limits.max_axis_output, 1.0);
        assert!(full_limits.allow_high_torque);
        assert!(full_limits.allow_custom_curves);

        // Demo mode should be more restrictive
        assert!(demo_limits.max_axis_output < full_limits.max_axis_output);
        assert!(demo_limits.max_ffb_torque < full_limits.max_ffb_torque);
        assert!(!demo_limits.allow_high_torque);

        // Kid mode should be most restrictive
        assert!(kid_limits.max_axis_output < demo_limits.max_axis_output);
        assert!(kid_limits.max_ffb_torque < demo_limits.max_ffb_torque);
        assert!(!kid_limits.allow_high_torque);
        assert!(!kid_limits.allow_custom_curves);
    }

    #[test]
    fn test_capability_enforcement_validation() {
        let mut axes = HashMap::new();
        axes.insert("pitch".to_string(), AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.8), // High expo that should be rejected in kid mode
            slew_rate: Some(10.0), // High slew rate that should be rejected
            detents: vec![],
            curve: Some(vec![
                CurvePoint { input: 0.0, output: 0.0 },
                CurvePoint { input: 1.0, output: 1.0 },
            ]),
        });

        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId { icao: "C172".to_string() }),
            axes,
            pof_overrides: None,
        };

        // Should pass in full mode
        let full_context = CapabilityContext::for_mode(CapabilityMode::Full);
        assert!(profile.validate_with_capabilities(&full_context).is_ok());

        // Should fail in kid mode due to high expo, slew rate, and custom curve
        let kid_context = CapabilityContext::for_mode(CapabilityMode::Kid);
        let result = profile.validate_with_location_and_capabilities(&kid_context);
        match &result {
            Ok(validation_result) => {
                if !validation_result.has_errors() {
                    panic!("Expected validation errors in kid mode, but got none");
                }
            }
            Err(e) => {
                // This is actually what we expect - validation should fail
                println!("Validation failed as expected: {}", e);
                return; // Test passes
            }
        }
        let validation_result = result.unwrap();
        
        // Should have multiple errors
        let errors = &validation_result.errors;
        assert!(errors.len() >= 3); // expo, slew_rate, and custom curve errors
        
        // Check that error messages mention the capability mode
        let error_messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        assert!(error_messages.iter().any(|msg| msg.contains("kid mode")));
    }

    #[test]
    fn test_capability_context_creation() {
        let context = CapabilityContext::for_mode(CapabilityMode::Demo);
        assert_eq!(context.mode, CapabilityMode::Demo);
        assert_eq!(context.mode_name(), "demo");
        assert!(context.audit_enabled);

        let context_no_audit = context.with_audit(false);
        assert!(!context_no_audit.audit_enabled);
    }

    proptest! {
        #[test]
        fn test_canonicalization_determinism(
            deadzone in prop::option::of(0.0f32..1.0),
            expo in prop::option::of(-1.0f32..1.0),
            slew_rate in prop::option::of(0.1f32..10.0)
        ) {
            let mut axes = HashMap::new();
            axes.insert("test".to_string(), AxisConfig {
                deadzone,
                expo,
                slew_rate,
                detents: vec![],
                curve: None,
            });

            let profile1 = Profile {
                schema: "flight.profile/1".to_string(),
                sim: Some("msfs".to_string()),
                aircraft: Some(AircraftId { icao: "TEST".to_string() }),
                axes: axes.clone(),
                pof_overrides: None,
            };

            let profile2 = Profile {
                schema: "flight.profile/1".to_string(),
                sim: Some("msfs".to_string()),
                aircraft: Some(AircraftId { icao: "TEST".to_string() }),
                axes,
                pof_overrides: None,
            };

            // Same inputs should produce identical hashes
            prop_assert_eq!(profile1.effective_hash(), profile2.effective_hash());
        }

        #[test]
        fn test_merge_determinism(
            base_deadzone in prop::option::of(0.0f32..0.5),
            override_deadzone in prop::option::of(0.5f32..1.0),
            base_expo in prop::option::of(-0.5f32..0.0),
            override_expo in prop::option::of(0.0f32..0.5)
        ) {
            let mut base_axes = HashMap::new();
            base_axes.insert("test".to_string(), AxisConfig {
                deadzone: base_deadzone,
                expo: base_expo,
                slew_rate: None,
                detents: vec![],
                curve: None,
            });

            let base_profile = Profile {
                schema: "flight.profile/1".to_string(),
                sim: Some("msfs".to_string()),
                aircraft: None,
                axes: base_axes,
                pof_overrides: None,
            };

            let mut override_axes = HashMap::new();
            override_axes.insert("test".to_string(), AxisConfig {
                deadzone: override_deadzone,
                expo: override_expo,
                slew_rate: Some(1.0),
                detents: vec![],
                curve: None,
            });

            let override_profile = Profile {
                schema: "flight.profile/1".to_string(),
                sim: None,
                aircraft: Some(AircraftId { icao: "TEST".to_string() }),
                axes: override_axes,
                pof_overrides: None,
            };

            // Multiple merges should be deterministic
            let merged1 = base_profile.merge_with(&override_profile).unwrap();
            let merged2 = base_profile.merge_with(&override_profile).unwrap();
            
            prop_assert_eq!(merged1.effective_hash(), merged2.effective_hash());
        }
    }
}