// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Signed update manifest with per-platform artifacts, policy control,
//! and integrity validation.

use crate::channels::Channel;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Target operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Windows,
    Linux,
    Macos,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Windows => write!(f, "windows"),
            Platform::Linux => write!(f, "linux"),
            Platform::Macos => write!(f, "macos"),
        }
    }
}

/// CPU architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    X86_64,
    Aarch64,
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arch::X86_64 => write!(f, "x86_64"),
            Arch::Aarch64 => write!(f, "aarch64"),
        }
    }
}

/// Installer / package format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallerType {
    Msi,
    Deb,
    TarGz,
}

impl fmt::Display for InstallerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallerType::Msi => write!(f, "msi"),
            InstallerType::Deb => write!(f, "deb"),
            InstallerType::TarGz => write!(f, "tar_gz"),
        }
    }
}

// ---------------------------------------------------------------------------
// Core structs
// ---------------------------------------------------------------------------

/// A downloadable artifact for a specific platform/arch combination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactEntry {
    pub platform: Platform,
    pub arch: Arch,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub installer_type: InstallerType,
}

/// Signed update manifest describing a release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    pub channel: Channel,
    pub release_date: String,
    pub artifacts: Vec<ArtifactEntry>,
    pub min_version: String,
    pub release_notes: String,
}

/// Policy governing how the updater behaves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatePolicy {
    pub auto_update: bool,
    pub allowed_channels: Vec<Channel>,
    pub rollback_on_failure: bool,
    pub max_download_retries: u8,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Errors produced by manifest validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestValidationError {
    InvalidSemver(String),
    InvalidSha256 { field: String, value: String },
    EmptyUrl { artifact_index: usize },
    EmptyArtifacts,
    EmptyReleaseNotes,
}

impl fmt::Display for ManifestValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSemver(v) => write!(f, "invalid semver: {v}"),
            Self::InvalidSha256 { field, value } => {
                write!(f, "invalid sha256 in {field}: {value}")
            }
            Self::EmptyUrl { artifact_index } => {
                write!(f, "empty url in artifact index {artifact_index}")
            }
            Self::EmptyArtifacts => write!(f, "artifacts list is empty"),
            Self::EmptyReleaseNotes => write!(f, "release notes are empty"),
        }
    }
}

fn is_valid_sha256(s: &str) -> bool {
    crate::is_valid_sha256(s)
}

fn parse_semver(version: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let major = parts[0].parse::<u32>().ok()?;
    let minor = parts[1].parse::<u32>().ok()?;
    let patch = parts[2].parse::<u32>().ok()?;
    Some((major, minor, patch))
}

fn compare_versions(a: &str, b: &str) -> Option<Ordering> {
    Some(parse_semver(a)?.cmp(&parse_semver(b)?))
}

impl UpdateManifest {
    /// Validate the manifest for structural correctness.
    pub fn validate(&self) -> Vec<ManifestValidationError> {
        let mut errors = Vec::new();

        if parse_semver(&self.version).is_none() {
            errors.push(ManifestValidationError::InvalidSemver(self.version.clone()));
        }
        if parse_semver(&self.min_version).is_none() {
            errors.push(ManifestValidationError::InvalidSemver(
                self.min_version.clone(),
            ));
        }
        if self.artifacts.is_empty() {
            errors.push(ManifestValidationError::EmptyArtifacts);
        }
        if self.release_notes.is_empty() {
            errors.push(ManifestValidationError::EmptyReleaseNotes);
        }
        for (i, a) in self.artifacts.iter().enumerate() {
            if !is_valid_sha256(&a.sha256) {
                errors.push(ManifestValidationError::InvalidSha256 {
                    field: format!("artifacts[{i}].sha256"),
                    value: a.sha256.clone(),
                });
            }
            if a.url.is_empty() {
                errors.push(ManifestValidationError::EmptyUrl { artifact_index: i });
            }
        }
        errors
    }

    /// Returns `true` when `current_version` satisfies `min_version`
    /// (i.e. current ≥ min_version) and the manifest version is strictly
    /// newer than `current_version`.
    pub fn is_applicable(&self, current_version: &str) -> bool {
        let cur_ge_min = compare_versions(current_version, &self.min_version)
            .is_some_and(|o| o != Ordering::Less);
        let manifest_gt_cur = compare_versions(&self.version, current_version)
            .is_some_and(|o| o == Ordering::Greater);
        cur_ge_min && manifest_gt_cur
    }
}

impl UpdatePolicy {
    /// Check whether the policy allows a given channel.
    pub fn allows_channel(&self, channel: Channel) -> bool {
        self.allowed_channels.contains(&channel)
    }
}

impl Default for UpdatePolicy {
    fn default() -> Self {
        Self {
            auto_update: false,
            allowed_channels: vec![Channel::Stable],
            rollback_on_failure: true,
            max_download_retries: 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_sha256() -> String {
        "a".repeat(64)
    }

    fn sample_artifact() -> ArtifactEntry {
        ArtifactEntry {
            platform: Platform::Windows,
            arch: Arch::X86_64,
            url: "https://dl.example.com/flight-1.2.0-win-x64.msi".into(),
            sha256: valid_sha256(),
            size_bytes: 50_000_000,
            installer_type: InstallerType::Msi,
        }
    }

    fn sample_manifest() -> UpdateManifest {
        UpdateManifest {
            version: "1.2.0".into(),
            channel: Channel::Stable,
            release_date: "2025-06-01T00:00:00Z".into(),
            artifacts: vec![sample_artifact()],
            min_version: "1.0.0".into(),
            release_notes: "Bug fixes and performance improvements.".into(),
        }
    }

    // 1. Serialization round-trip
    #[test]
    fn test_manifest_json_round_trip() {
        let manifest = sample_manifest();
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let deserialized: UpdateManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, deserialized);
    }

    // 2. ArtifactEntry round-trip
    #[test]
    fn test_artifact_entry_json_round_trip() {
        let artifact = sample_artifact();
        let json = serde_json::to_string(&artifact).unwrap();
        let deserialized: ArtifactEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(artifact, deserialized);
    }

    // 3. UpdatePolicy round-trip and defaults
    #[test]
    fn test_policy_json_round_trip_and_defaults() {
        let policy = UpdatePolicy::default();
        assert!(!policy.auto_update);
        assert!(policy.rollback_on_failure);
        assert_eq!(policy.max_download_retries, 3);
        assert_eq!(policy.allowed_channels, vec![Channel::Stable]);

        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: UpdatePolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, deserialized);
    }

    // 4. Valid manifest passes validation
    #[test]
    fn test_valid_manifest_passes_validation() {
        let manifest = sample_manifest();
        let errors = manifest.validate();
        assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
    }

    // 5. Invalid SHA256 detected
    #[test]
    fn test_invalid_sha256_detected() {
        let mut manifest = sample_manifest();
        manifest.artifacts[0].sha256 = "not-a-valid-hash".into();
        let errors = manifest.validate();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ManifestValidationError::InvalidSha256 { .. })),
            "expected InvalidSha256 error"
        );
    }

    // 6. Invalid semver detected
    #[test]
    fn test_invalid_semver_detected() {
        let mut manifest = sample_manifest();
        manifest.version = "not-semver".into();
        let errors = manifest.validate();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ManifestValidationError::InvalidSemver(_))),
            "expected InvalidSemver error"
        );
    }

    // 7. Empty URL detected
    #[test]
    fn test_empty_url_detected() {
        let mut manifest = sample_manifest();
        manifest.artifacts[0].url = String::new();
        let errors = manifest.validate();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ManifestValidationError::EmptyUrl { .. })),
            "expected EmptyUrl error"
        );
    }

    // 8. is_applicable — current version in range
    #[test]
    fn test_is_applicable_in_range() {
        let manifest = sample_manifest(); // version 1.2.0, min_version 1.0.0
        assert!(manifest.is_applicable("1.0.0"));
        assert!(manifest.is_applicable("1.1.0"));
    }

    // 9. is_applicable — current version too old
    #[test]
    fn test_is_applicable_below_min() {
        let manifest = sample_manifest();
        assert!(!manifest.is_applicable("0.9.0"));
    }

    // 10. is_applicable — current version already at manifest version
    #[test]
    fn test_is_applicable_already_current() {
        let manifest = sample_manifest();
        assert!(!manifest.is_applicable("1.2.0"));
    }

    // 11. Channel filtering via policy
    #[test]
    fn test_policy_channel_filtering() {
        let policy = UpdatePolicy {
            auto_update: true,
            allowed_channels: vec![Channel::Stable, Channel::Beta],
            rollback_on_failure: true,
            max_download_retries: 5,
        };
        assert!(policy.allows_channel(Channel::Stable));
        assert!(policy.allows_channel(Channel::Beta));
        assert!(!policy.allows_channel(Channel::Canary));
    }

    // 12. Multiple validation errors reported at once
    #[test]
    fn test_multiple_validation_errors() {
        let manifest = UpdateManifest {
            version: "bad".into(),
            channel: Channel::Stable,
            release_date: "2025-06-01".into(),
            artifacts: vec![],
            min_version: "also-bad".into(),
            release_notes: String::new(),
        };
        let errors = manifest.validate();
        assert!(errors.len() >= 4, "expected ≥4 errors, got {errors:?}");
    }
}
