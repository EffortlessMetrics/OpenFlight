// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Feature negotiation and version compatibility

use crate::{
    IpcError, PROTOCOL_VERSION,
    proto::{NegotiateFeaturesRequest, NegotiateFeaturesResponse, TransportType},
};
use anyhow::Result;
use std::collections::HashSet;

/// Semantic version parsing and comparison
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn parse(version_str: &str) -> Result<Self, IpcError> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() != 3 {
            return Err(IpcError::ConnectionFailed {
                reason: format!("Invalid version format: {}", version_str),
            });
        }

        let major = parts[0].parse().map_err(|_| IpcError::ConnectionFailed {
            reason: format!("Invalid major version: {}", parts[0]),
        })?;

        let minor = parts[1].parse().map_err(|_| IpcError::ConnectionFailed {
            reason: format!("Invalid minor version: {}", parts[1]),
        })?;

        let patch = parts[2].parse().map_err(|_| IpcError::ConnectionFailed {
            reason: format!("Invalid patch version: {}", parts[2]),
        })?;

        Ok(Version {
            major,
            minor,
            patch,
        })
    }

    /// Check if this version is compatible with another version
    /// Compatible if major versions match and this version >= other version
    pub fn is_compatible_with(&self, other: &Version) -> bool {
        self.major == other.major && self >= other
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Negotiate features between client and server
pub fn negotiate_features(
    request: &NegotiateFeaturesRequest,
    server_features: &[String],
) -> Result<NegotiateFeaturesResponse, IpcError> {
    // Parse versions
    let client_version = Version::parse(&request.client_version)?;
    let server_version = Version::parse(PROTOCOL_VERSION)?;

    // Check version compatibility
    if !server_version.is_compatible_with(&client_version) {
        return Ok(NegotiateFeaturesResponse {
            success: false,
            server_version: PROTOCOL_VERSION.to_string(),
            enabled_features: vec![],
            negotiated_transport: TransportType::Unspecified.into(),
            error_message: format!(
                "Version mismatch: client={}, server={}",
                request.client_version, PROTOCOL_VERSION
            ),
        });
    }

    // Negotiate features - intersection of client and server supported features
    let client_features: HashSet<String> = request.supported_features.iter().cloned().collect();
    let server_features: HashSet<String> = server_features.iter().cloned().collect();
    let enabled_features: Vec<String> = client_features
        .intersection(&server_features)
        .cloned()
        .collect();

    // Negotiate transport - prefer client's choice if supported
    let negotiated_transport = match request.preferred_transport() {
        TransportType::NamedPipes => {
            #[cfg(all(windows, feature = "named-pipes"))]
            {
                TransportType::NamedPipes
            }
            #[cfg(not(all(windows, feature = "named-pipes")))]
            {
                TransportType::UnixSockets
            }
        }
        TransportType::UnixSockets => {
            #[cfg(all(unix, feature = "unix-sockets"))]
            {
                TransportType::UnixSockets
            }
            #[cfg(not(all(unix, feature = "unix-sockets")))]
            {
                TransportType::NamedPipes
            }
        }
        _ => crate::default_transport_type(),
    };

    Ok(NegotiateFeaturesResponse {
        success: true,
        server_version: PROTOCOL_VERSION.to_string(),
        enabled_features,
        negotiated_transport: negotiated_transport.into(),
        error_message: String::new(),
    })
}

/// Validate that required features are enabled
pub fn validate_required_features(
    enabled_features: &[String],
    required_features: &[String],
) -> Result<(), IpcError> {
    let enabled: HashSet<String> = enabled_features.iter().cloned().collect();

    for required in required_features {
        if !enabled.contains(required) {
            return Err(IpcError::UnsupportedFeature {
                feature: required.clone(),
            });
        }
    }

    Ok(())
}

/// Check for breaking changes in protocol schema
/// This would be called during CI to detect breaking changes
pub fn detect_breaking_changes(
    old_schema: &str,
    new_schema: &str,
) -> Result<Vec<String>, IpcError> {
    // This is a simplified implementation
    // In practice, you'd use protobuf reflection or a dedicated tool
    let mut breaking_changes = Vec::new();

    // Check for removed fields (simplified check)
    let old_lines: HashSet<&str> = old_schema.lines().collect();
    let new_lines: HashSet<&str> = new_schema.lines().collect();

    for old_line in &old_lines {
        if old_line.trim().starts_with("rpc ") && !new_lines.contains(old_line) {
            breaking_changes.push(format!("Removed RPC: {}", old_line.trim()));
        }

        if old_line.trim().starts_with("message ") && !new_lines.contains(old_line) {
            breaking_changes.push(format!("Removed message: {}", old_line.trim()));
        }
    }

    Ok(breaking_changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        let version = Version::parse("1.2.3").unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
    }

    #[test]
    fn test_version_compatibility() {
        let v1_0_0 = Version::parse("1.0.0").unwrap();
        let v1_1_0 = Version::parse("1.1.0").unwrap();
        let v2_0_0 = Version::parse("2.0.0").unwrap();

        assert!(v1_1_0.is_compatible_with(&v1_0_0));
        assert!(!v1_0_0.is_compatible_with(&v1_1_0));
        assert!(!v2_0_0.is_compatible_with(&v1_0_0));
    }

    #[test]
    fn test_feature_negotiation() {
        let request = NegotiateFeaturesRequest {
            client_version: "1.0.0".to_string(),
            supported_features: vec![
                "device-management".to_string(),
                "health-monitoring".to_string(),
            ],
            preferred_transport: TransportType::NamedPipes.into(),
        };

        let server_features = vec![
            "device-management".to_string(),
            "profile-management".to_string(),
        ];

        let response = negotiate_features(&request, &server_features).unwrap();

        assert!(response.success);
        assert_eq!(response.enabled_features, vec!["device-management"]);
    }

    use proptest::prelude::*;

    proptest! {
        // Test version parsing with valid generated versions
        #[test]
        fn prop_parse_valid_version(major in 0u32..100, minor in 0u32..100, patch in 0u32..100) {
            let version_str = format!("{}.{}.{}", major, minor, patch);
            let version = Version::parse(&version_str).unwrap();

            prop_assert_eq!(version.major, major);
            prop_assert_eq!(version.minor, minor);
            prop_assert_eq!(version.patch, patch);
        }

        // Test version compatibility logic
        #[test]
        fn prop_version_compatibility(
            major in 1u32..100,
            minor1 in 0u32..100,
            minor2 in 0u32..100
        ) {
            let v1 = Version { major, minor: minor1, patch: 0 };
            let v2 = Version { major, minor: minor2, patch: 0 };

            // Same major version: compatible if v1 >= v2
            if minor1 >= minor2 {
                prop_assert!(v1.is_compatible_with(&v2));
            } else {
                prop_assert!(!v1.is_compatible_with(&v2));
            }
        }

        // Test feature negotiation intersection
        #[test]
        fn prop_feature_negotiation_intersection(
            ref client_feats in proptest::collection::vec("[a-z]+", 0..10),
            ref server_feats in proptest::collection::vec("[a-z]+", 0..10)
        ) {
            let request = NegotiateFeaturesRequest {
                client_version: PROTOCOL_VERSION.to_string(),
                supported_features: client_feats.clone(),
                preferred_transport: TransportType::Unspecified.into(),
            };

            let response = negotiate_features(&request, server_feats).unwrap();

            if response.success {
                // Enabled features should be subset of both
                for feat in &response.enabled_features {
                    prop_assert!(client_feats.contains(feat));
                    prop_assert!(server_feats.contains(feat));
                }

                // Check count matches intersection
                let client_set: HashSet<_> = client_feats.iter().collect();
                let server_set: HashSet<_> = server_feats.iter().collect();
                let intersection_count = client_set.intersection(&server_set).count();

                prop_assert_eq!(response.enabled_features.len(), intersection_count);
            }
        }
    }
}
