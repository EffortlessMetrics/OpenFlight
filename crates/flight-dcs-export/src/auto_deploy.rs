// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS export script auto-deployment (REQ-694)
//!
//! Automatically finds DCS World installations and deploys the Flight Hub
//! Export.lua script, comparing versions before overwriting.

use std::path::{Path, PathBuf};

/// Result of a deployment attempt.
#[derive(Debug, Clone, PartialEq)]
pub enum DeployResult {
    /// Script was successfully deployed.
    Deployed,
    /// Existing script is already up to date.
    AlreadyUpToDate,
    /// Deployment skipped because the existing version differs.
    VersionSkipped {
        old_version: String,
        new_version: String,
    },
}

/// Extract the version string from a script's content.
///
/// Looks for a line containing `-- Version: X.Y.Z` and returns the version part.
fn extract_version(content: &str) -> Option<String> {
    content
        .lines()
        .find(|line| line.contains("Version:"))
        .and_then(|line| line.split("Version:").nth(1))
        .map(|v| v.trim().to_string())
}

/// Attempt to find the DCS World Saved Games directory.
///
/// On Windows this checks the registry first, then falls back to common paths
/// under `%USERPROFILE%\Saved Games`. On non-Windows platforms returns `None`.
pub fn find_dcs_install() -> Option<PathBuf> {
    // Try registry on Windows
    #[cfg(windows)]
    {
        if let Some(path) = find_dcs_via_registry() {
            return Some(path);
        }
    }

    // Fallback: common Saved Games paths
    if let Some(home) = dirs::home_dir() {
        let candidates = [
            home.join("Saved Games").join("DCS.openbeta"),
            home.join("Saved Games").join("DCS"),
        ];
        for candidate in &candidates {
            if candidate.is_dir() {
                return Some(candidate.clone());
            }
        }
    }

    None
}

#[cfg(windows)]
fn find_dcs_via_registry() -> Option<PathBuf> {
    use std::process::Command;

    // Query the registry for DCS install path
    let output = Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Eagle Dynamics\DCS World",
            "/v",
            "Path",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.contains("Path") && trimmed.contains("REG_SZ") {
            let path_str = trimmed.split("REG_SZ").nth(1)?.trim();
            let path = PathBuf::from(path_str);
            if path.is_dir() {
                return Some(path);
            }
        }
    }

    None
}

/// Deploy the export script into the DCS Scripts directory.
///
/// Compares version strings before overwriting. If the existing script has the
/// same version the deployment is skipped as `AlreadyUpToDate`. If versions
/// differ and the existing script is a Flight Hub script, returns
/// `VersionSkipped` so the caller can decide whether to force-overwrite.
pub fn deploy_export_script(
    dcs_dir: &Path,
    script_content: &str,
) -> anyhow::Result<DeployResult> {
    let scripts_dir = dcs_dir.join("Scripts");
    std::fs::create_dir_all(&scripts_dir)?;

    let target = scripts_dir.join("Export.lua");

    if target.exists() {
        let existing = std::fs::read_to_string(&target)?;
        let existing_version = extract_version(&existing);
        let new_version = extract_version(script_content);

        match (existing_version, new_version) {
            (Some(old), Some(new)) if old == new => {
                return Ok(DeployResult::AlreadyUpToDate);
            }
            (Some(old), Some(new)) => {
                return Ok(DeployResult::VersionSkipped {
                    old_version: old,
                    new_version: new,
                });
            }
            _ => {
                // No version info — overwrite
            }
        }
    }

    std::fs::write(&target, script_content)?;
    Ok(DeployResult::Deployed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_deploy_to_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let content = "-- Flight Hub DCS Export Script\n-- Version: 1.0.0\nprint('hello')";
        let result = deploy_export_script(tmp.path(), content).unwrap();
        assert_eq!(result, DeployResult::Deployed);

        let written = std::fs::read_to_string(tmp.path().join("Scripts").join("Export.lua")).unwrap();
        assert_eq!(written, content);
    }

    #[test]
    fn test_deploy_already_up_to_date() {
        let tmp = TempDir::new().unwrap();
        let content = "-- Flight Hub DCS Export Script\n-- Version: 1.0.0\nprint('hello')";

        // Deploy once
        deploy_export_script(tmp.path(), content).unwrap();

        // Deploy again with same version
        let result = deploy_export_script(tmp.path(), content).unwrap();
        assert_eq!(result, DeployResult::AlreadyUpToDate);
    }

    #[test]
    fn test_deploy_version_skipped() {
        let tmp = TempDir::new().unwrap();
        let old_content = "-- Flight Hub DCS Export Script\n-- Version: 1.0.0\nprint('old')";
        let new_content = "-- Flight Hub DCS Export Script\n-- Version: 2.0.0\nprint('new')";

        deploy_export_script(tmp.path(), old_content).unwrap();
        let result = deploy_export_script(tmp.path(), new_content).unwrap();

        assert_eq!(
            result,
            DeployResult::VersionSkipped {
                old_version: "1.0.0".to_string(),
                new_version: "2.0.0".to_string(),
            }
        );
    }

    #[test]
    fn test_extract_version_found() {
        let content = "-- Flight Hub DCS Export Script\n-- Version: 3.2.1\ncode()";
        assert_eq!(extract_version(content), Some("3.2.1".to_string()));
    }

    #[test]
    fn test_extract_version_missing() {
        let content = "-- no version here\ncode()";
        assert_eq!(extract_version(content), None);
    }

    #[test]
    fn test_deploy_overwrites_when_no_version() {
        let tmp = TempDir::new().unwrap();
        let old_content = "-- some old script with no version";
        let new_content = "-- new script with no version";

        // Create Scripts dir and old file
        std::fs::create_dir_all(tmp.path().join("Scripts")).unwrap();
        std::fs::write(tmp.path().join("Scripts").join("Export.lua"), old_content).unwrap();

        let result = deploy_export_script(tmp.path(), new_content).unwrap();
        assert_eq!(result, DeployResult::Deployed);

        let written = std::fs::read_to_string(tmp.path().join("Scripts").join("Export.lua")).unwrap();
        assert_eq!(written, new_content);
    }
}
