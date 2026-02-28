// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! First-run detection and default profile creation.

use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Result of first-run check.
#[derive(Debug, Clone, PartialEq)]
pub enum FirstRunResult {
    /// First run: profile directory was absent; default profile created.
    Created { profile_path: PathBuf },
    /// Not first run: profile directory already existed.
    Existing,
    /// First run but failed to create default profile.
    CreationFailed { reason: String },
}

/// Check if this is a first run and create default profile if needed.
///
/// Returns `FirstRunResult::Created` if profile_dir was absent and default
/// profile was written. Non-destructive on subsequent runs.
pub fn check_and_initialize(profile_dir: &Path) -> FirstRunResult {
    if profile_dir.exists() {
        return FirstRunResult::Existing;
    }

    info!("First run detected: profile directory does not exist");

    if let Err(e) = std::fs::create_dir_all(profile_dir) {
        warn!("Failed to create profile directory: {}", e);
        return FirstRunResult::CreationFailed {
            reason: format!("Cannot create directory: {}", e),
        };
    }

    let profile_path = profile_dir.join("default.yaml");

    if let Err(e) = std::fs::write(&profile_path, default_profile_yaml()) {
        warn!("Failed to write default profile: {}", e);
        return FirstRunResult::CreationFailed {
            reason: format!("Cannot write profile: {}", e),
        };
    }

    info!("Created default profile at {}", profile_path.display());

    FirstRunResult::Created { profile_path }
}

/// Returns the default profile YAML content.
pub fn default_profile_yaml() -> &'static str {
    r#"schema: "flight.profile/1"
name: "Default Profile"
description: "Auto-created on first run"

axes:
  pitch:
    deadzone: 0.03
    expo: 0.20
    response: linear
  roll:
    deadzone: 0.03
    expo: 0.20
    response: linear
  yaw:
    deadzone: 0.05
    expo: 0.10
    response: linear
  throttle:
    deadzone: 0.01
    response: linear
  collective:
    deadzone: 0.02
    response: linear
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_first_run_creates_directory() {
        let base = TempDir::new().unwrap();
        let profile_dir = base.path().join("profiles");

        assert!(!profile_dir.exists());
        check_and_initialize(&profile_dir);
        assert!(profile_dir.exists());
        assert!(profile_dir.join("default.yaml").exists());
    }

    #[test]
    fn test_first_run_result_is_created() {
        let base = TempDir::new().unwrap();
        let profile_dir = base.path().join("profiles");

        let result = check_and_initialize(&profile_dir);
        assert_eq!(
            result,
            FirstRunResult::Created {
                profile_path: profile_dir.join("default.yaml")
            }
        );
    }

    #[test]
    fn test_second_run_returns_existing() {
        let base = TempDir::new().unwrap();
        let profile_dir = base.path().join("profiles");

        check_and_initialize(&profile_dir);
        let result = check_and_initialize(&profile_dir);
        assert_eq!(result, FirstRunResult::Existing);
    }

    #[test]
    fn test_profile_content_is_valid_yaml() {
        let yaml = default_profile_yaml();
        let value: serde_yaml::Value =
            serde_yaml::from_str(yaml).expect("default profile must be valid YAML");
        assert!(value.is_mapping());
    }

    #[test]
    fn test_default_profile_has_pitch_roll_yaw() {
        let yaml = default_profile_yaml();
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let axes = value["axes"].as_mapping().expect("axes must be a mapping");
        assert!(axes.contains_key(serde_yaml::Value::String("pitch".to_string())));
        assert!(axes.contains_key(serde_yaml::Value::String("roll".to_string())));
        assert!(axes.contains_key(serde_yaml::Value::String("yaw".to_string())));
    }

    #[test]
    fn test_creation_fails_on_readonly_path() {
        // On Windows, attempt to create a profile under a null-byte path which is invalid.
        // This exercises the CreationFailed branch without needing filesystem permissions.
        let invalid_dir = Path::new("\0invalid");
        let result = check_and_initialize(invalid_dir);
        assert!(matches!(result, FirstRunResult::CreationFailed { .. }));
    }
}
