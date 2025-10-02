//! Version compatibility management for StreamDeck app integration
//!
//! Implements supported app version ranges with graceful degradation behavior.
//! Provides detection and warning for out-of-range versions without crashing.

use crate::{AppVersion, VersionError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn, error};

/// Version range specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRange {
    pub min_version: AppVersion,
    pub max_version: AppVersion,
    pub features: Vec<String>,
    pub degraded_features: Vec<String>,
}

impl VersionRange {
    pub fn new(min_version: AppVersion, max_version: AppVersion) -> Self {
        Self {
            min_version,
            max_version,
            features: Vec::new(),
            degraded_features: Vec::new(),
        }
    }

    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }

    pub fn with_degraded_features(mut self, degraded_features: Vec<String>) -> Self {
        self.degraded_features = degraded_features;
        self
    }

    /// Check if a version falls within this range
    pub fn contains(&self, version: &AppVersion) -> bool {
        self.is_greater_or_equal(version, &self.min_version) &&
        self.is_less_or_equal(version, &self.max_version)
    }

    /// Check if version is greater than or equal to reference
    fn is_greater_or_equal(&self, version: &AppVersion, reference: &AppVersion) -> bool {
        if version.major != reference.major {
            return version.major > reference.major;
        }
        if version.minor != reference.minor {
            return version.minor > reference.minor;
        }
        if version.patch != reference.patch {
            return version.patch > reference.patch;
        }
        
        match (version.build, reference.build) {
            (Some(v_build), Some(r_build)) => v_build >= r_build,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => true,
        }
    }

    /// Check if version is less than or equal to reference
    fn is_less_or_equal(&self, version: &AppVersion, reference: &AppVersion) -> bool {
        if version.major != reference.major {
            return version.major < reference.major;
        }
        if version.minor != reference.minor {
            return version.minor < reference.minor;
        }
        if version.patch != reference.patch {
            return version.patch < reference.patch;
        }
        
        match (version.build, reference.build) {
            (Some(v_build), Some(r_build)) => v_build <= r_build,
            (Some(_), None) => false,
            (None, Some(_)) => true,
            (None, None) => true,
        }
    }
}

/// Compatibility status for a version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompatibilityStatus {
    FullySupported,
    PartiallySupported { missing_features: Vec<String> },
    Deprecated { warning_message: String },
    Unsupported { error_message: String },
}

/// Compatibility matrix mapping version ranges to support status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityMatrix {
    pub supported_ranges: Vec<VersionRange>,
    pub deprecated_ranges: Vec<VersionRange>,
    pub unsupported_ranges: Vec<VersionRange>,
    pub feature_matrix: HashMap<String, VersionRange>,
}

impl CompatibilityMatrix {
    pub fn new() -> Self {
        Self {
            supported_ranges: Vec::new(),
            deprecated_ranges: Vec::new(),
            unsupported_ranges: Vec::new(),
            feature_matrix: HashMap::new(),
        }
    }

    /// Create default compatibility matrix for StreamDeck app versions
    pub fn default_streamdeck() -> Self {
        let mut matrix = Self::new();

        // Fully supported versions (StreamDeck 6.0.0 - 6.4.x)
        matrix.supported_ranges.push(
            VersionRange::new(
                AppVersion::new(6, 0, 0),
                AppVersion::new(6, 4, 999)
            ).with_features(vec![
                "basic_actions".to_string(),
                "multi_actions".to_string(),
                "profiles".to_string(),
                "property_inspector".to_string(),
                "websocket_api".to_string(),
            ])
        );

        // Partially supported versions (StreamDeck 5.3.0 - 5.9.x)
        matrix.deprecated_ranges.push(
            VersionRange::new(
                AppVersion::new(5, 3, 0),
                AppVersion::new(5, 9, 999)
            ).with_features(vec![
                "basic_actions".to_string(),
                "profiles".to_string(),
            ]).with_degraded_features(vec![
                "multi_actions".to_string(),
                "property_inspector".to_string(),
            ])
        );

        // Unsupported versions (< 5.3.0 or >= 7.0.0)
        matrix.unsupported_ranges.push(
            VersionRange::new(
                AppVersion::new(0, 0, 0),
                AppVersion::new(5, 2, 999)
            )
        );
        
        matrix.unsupported_ranges.push(
            VersionRange::new(
                AppVersion::new(7, 0, 0),
                AppVersion::new(999, 999, 999)
            )
        );

        // Feature-specific version requirements
        matrix.feature_matrix.insert(
            "websocket_api".to_string(),
            VersionRange::new(AppVersion::new(6, 0, 0), AppVersion::new(6, 4, 999))
        );
        
        matrix.feature_matrix.insert(
            "property_inspector".to_string(),
            VersionRange::new(AppVersion::new(5, 5, 0), AppVersion::new(6, 4, 999))
        );

        matrix
    }

    /// Check compatibility status for a given version
    pub fn check_compatibility(&self, version: &AppVersion) -> CompatibilityStatus {
        // Check if fully supported
        for range in &self.supported_ranges {
            if range.contains(version) {
                return CompatibilityStatus::FullySupported;
            }
        }

        // Check if deprecated but partially supported
        for range in &self.deprecated_ranges {
            if range.contains(version) {
                let missing_features = range.degraded_features.clone();
                return CompatibilityStatus::PartiallySupported { missing_features };
            }
        }

        // Check if explicitly unsupported
        for range in &self.unsupported_ranges {
            if range.contains(version) {
                return CompatibilityStatus::Unsupported {
                    error_message: format!(
                        "StreamDeck app version {} is not supported. Please upgrade to version 6.0.0 or later.",
                        version
                    ),
                };
            }
        }

        // Unknown version - treat as deprecated
        CompatibilityStatus::Deprecated {
            warning_message: format!(
                "StreamDeck app version {} is unknown. Some features may not work correctly.",
                version
            ),
        }
    }

    /// Get available features for a version
    pub fn get_available_features(&self, version: &AppVersion) -> Vec<String> {
        let mut available_features = Vec::new();

        for (feature, range) in &self.feature_matrix {
            if range.contains(version) {
                available_features.push(feature.clone());
            }
        }

        available_features
    }
}

impl Default for CompatibilityMatrix {
    fn default() -> Self {
        Self::default_streamdeck()
    }
}

/// Version compatibility manager
pub struct VersionCompatibility {
    matrix: CompatibilityMatrix,
    current_app_version: Option<AppVersion>,
    compatibility_status: Option<CompatibilityStatus>,
}

impl VersionCompatibility {
    pub fn new() -> Self {
        Self {
            matrix: CompatibilityMatrix::default(),
            current_app_version: None,
            compatibility_status: None,
        }
    }

    pub fn with_matrix(matrix: CompatibilityMatrix) -> Self {
        Self {
            matrix,
            current_app_version: None,
            compatibility_status: None,
        }
    }

    /// Check if a version is compatible
    pub fn is_compatible(&self, version: &AppVersion) -> Result<bool, VersionError> {
        let status = self.matrix.check_compatibility(version);
        
        match &status {
            CompatibilityStatus::FullySupported => {
                info!("StreamDeck app version {} is fully supported", version);
                Ok(true)
            }
            CompatibilityStatus::PartiallySupported { missing_features } => {
                warn!(
                    "StreamDeck app version {} is partially supported. Missing features: {:?}",
                    version, missing_features
                );
                Ok(true)
            }
            CompatibilityStatus::Deprecated { warning_message } => {
                warn!("StreamDeck app version {}: {}", version, warning_message);
                Ok(true)
            }
            CompatibilityStatus::Unsupported { error_message } => {
                error!("StreamDeck app version {}: {}", version, error_message);
                Err(VersionError::NotSupported {
                    version: version.to_string(),
                    min_version: "6.0.0".to_string(),
                    max_version: "6.4.x".to_string(),
                })
            }
        }
    }

    /// Set the current app version and check compatibility
    pub fn set_app_version(&mut self, version: AppVersion) -> Result<(), VersionError> {
        let is_compatible = self.is_compatible(&version)?;
        
        if is_compatible {
            self.compatibility_status = Some(self.matrix.check_compatibility(&version));
            self.current_app_version = Some(version);
            Ok(())
        } else {
            Err(VersionError::CompatibilityCheckFailed(
                "Version is not compatible".to_string()
            ))
        }
    }

    /// Get the current compatibility status
    pub fn get_compatibility_status(&self) -> Option<&CompatibilityStatus> {
        self.compatibility_status.as_ref()
    }

    /// Get available features for current version
    pub fn get_available_features(&self) -> Vec<String> {
        if let Some(version) = &self.current_app_version {
            self.matrix.get_available_features(version)
        } else {
            Vec::new()
        }
    }

    /// Check if a specific feature is available
    pub fn is_feature_available(&self, feature: &str) -> bool {
        self.get_available_features().contains(&feature.to_string())
    }

    /// Get the compatibility matrix
    pub fn get_matrix(&self) -> &CompatibilityMatrix {
        &self.matrix
    }

    /// Get user guidance for version management
    pub fn get_user_guidance(&self) -> String {
        match &self.compatibility_status {
            Some(CompatibilityStatus::FullySupported) => {
                "Your StreamDeck app version is fully supported. All features are available.".to_string()
            }
            Some(CompatibilityStatus::PartiallySupported { missing_features }) => {
                format!(
                    "Your StreamDeck app version is supported but some features are unavailable: {}. \
                    Consider upgrading to StreamDeck app version 6.0.0 or later for full functionality.",
                    missing_features.join(", ")
                )
            }
            Some(CompatibilityStatus::Deprecated { warning_message }) => {
                format!(
                    "{}. Consider upgrading to a newer version for better compatibility.",
                    warning_message
                )
            }
            Some(CompatibilityStatus::Unsupported { error_message }) => {
                format!(
                    "{}. Please visit https://www.elgato.com/gaming/stream-deck to download the latest version.",
                    error_message
                )
            }
            None => {
                "StreamDeck app version not detected. Please ensure StreamDeck app is running.".to_string()
            }
        }
    }
}

impl Default for VersionCompatibility {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_range_contains() {
        let range = VersionRange::new(
            AppVersion::new(6, 0, 0),
            AppVersion::new(6, 4, 999)
        );

        assert!(range.contains(&AppVersion::new(6, 0, 0)));
        assert!(range.contains(&AppVersion::new(6, 2, 5)));
        assert!(range.contains(&AppVersion::new(6, 4, 999)));
        assert!(!range.contains(&AppVersion::new(5, 9, 999)));
        assert!(!range.contains(&AppVersion::new(7, 0, 0)));
    }

    #[test]
    fn test_version_from_string() {
        let version = AppVersion::from_string("6.2.1").unwrap();
        assert_eq!(version.major, 6);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 1);
        assert_eq!(version.build, None);

        let version_with_build = AppVersion::from_string("6.2.1.123").unwrap();
        assert_eq!(version_with_build.build, Some(123));

        assert!(AppVersion::from_string("invalid").is_err());
        assert!(AppVersion::from_string("6.2").is_err());
        assert!(AppVersion::from_string("6.2.1.2.3").is_err());
    }

    #[test]
    fn test_compatibility_matrix() {
        let matrix = CompatibilityMatrix::default_streamdeck();
        
        // Test fully supported version
        let supported_version = AppVersion::new(6, 2, 0);
        match matrix.check_compatibility(&supported_version) {
            CompatibilityStatus::FullySupported => (),
            _ => panic!("Expected fully supported"),
        }

        // Test deprecated version
        let deprecated_version = AppVersion::new(5, 5, 0);
        match matrix.check_compatibility(&deprecated_version) {
            CompatibilityStatus::PartiallySupported { .. } => (),
            _ => panic!("Expected partially supported"),
        }

        // Test unsupported version
        let unsupported_version = AppVersion::new(4, 0, 0);
        match matrix.check_compatibility(&unsupported_version) {
            CompatibilityStatus::Unsupported { .. } => (),
            _ => panic!("Expected unsupported"),
        }
    }

    #[test]
    fn test_version_compatibility() {
        let mut compat = VersionCompatibility::new();
        
        // Test compatible version
        let compatible_version = AppVersion::new(6, 2, 0);
        assert!(compat.is_compatible(&compatible_version).unwrap());
        
        compat.set_app_version(compatible_version).unwrap();
        assert!(compat.is_feature_available("websocket_api"));
        
        // Test incompatible version
        let incompatible_version = AppVersion::new(4, 0, 0);
        assert!(compat.is_compatible(&incompatible_version).is_err());
    }

    #[test]
    fn test_user_guidance() {
        let mut compat = VersionCompatibility::new();
        
        // Test guidance for supported version
        compat.set_app_version(AppVersion::new(6, 2, 0)).unwrap();
        let guidance = compat.get_user_guidance();
        assert!(guidance.contains("fully supported"));
        
        // Test guidance for deprecated version
        compat.set_app_version(AppVersion::new(5, 5, 0)).unwrap();
        let guidance = compat.get_user_guidance();
        assert!(guidance.contains("supported but some features"));
    }
}