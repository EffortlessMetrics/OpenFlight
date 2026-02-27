// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! CLI-Service version compatibility check (REQ-697)

use serde::Serialize;

/// Parsed semantic version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemVer {
    /// Major version — incremented on breaking changes.
    pub major: u32,
    /// Minor version — incremented on backwards-compatible additions.
    pub minor: u32,
    /// Patch version — incremented on backwards-compatible fixes.
    pub patch: u32,
}

impl SemVer {
    /// Parse a "major.minor.patch" string.
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.trim().split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(Self {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Version check input.
#[derive(Debug, Clone)]
pub struct VersionCheck {
    /// Semantic version of the CLI binary.
    pub cli_version: String,
    /// Semantic version reported by the running service.
    pub service_version: String,
}

/// Result of a version compatibility check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum VersionCheckResult {
    /// Versions are compatible.
    Compatible,
    /// Minor version mismatch — functionality may differ.
    MinorMismatch {
        cli_version: String,
        service_version: String,
    },
    /// Major version mismatch — incompatible.
    MajorMismatch {
        cli_version: String,
        service_version: String,
    },
}

/// Check compatibility between CLI and service versions.
pub fn check_compatibility(check: &VersionCheck) -> VersionCheckResult {
    let cli = match SemVer::parse(&check.cli_version) {
        Some(v) => v,
        None => {
            return VersionCheckResult::MajorMismatch {
                cli_version: check.cli_version.clone(),
                service_version: check.service_version.clone(),
            };
        }
    };

    let svc = match SemVer::parse(&check.service_version) {
        Some(v) => v,
        None => {
            return VersionCheckResult::MajorMismatch {
                cli_version: check.cli_version.clone(),
                service_version: check.service_version.clone(),
            };
        }
    };

    if cli.major != svc.major {
        VersionCheckResult::MajorMismatch {
            cli_version: check.cli_version.clone(),
            service_version: check.service_version.clone(),
        }
    } else if cli.minor != svc.minor {
        VersionCheckResult::MinorMismatch {
            cli_version: check.cli_version.clone(),
            service_version: check.service_version.clone(),
        }
    } else {
        VersionCheckResult::Compatible
    }
}

/// Format a human-readable warning for non-compatible results.
/// Returns `None` if versions are compatible.
pub fn format_warning(result: &VersionCheckResult) -> Option<String> {
    match result {
        VersionCheckResult::Compatible => None,
        VersionCheckResult::MinorMismatch {
            cli_version,
            service_version,
        } => Some(format!(
            "Warning: CLI version ({}) differs from service version ({}). \
             Some features may not be available.",
            cli_version, service_version,
        )),
        VersionCheckResult::MajorMismatch {
            cli_version,
            service_version,
        } => Some(format!(
            "Error: CLI version ({}) is incompatible with service version ({}). \
             Please update to a matching version.",
            cli_version, service_version,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatible_versions_return_compatible() {
        let check = VersionCheck {
            cli_version: "1.2.3".to_string(),
            service_version: "1.2.5".to_string(),
        };
        assert_eq!(check_compatibility(&check), VersionCheckResult::Compatible);
    }

    #[test]
    fn same_version_is_compatible() {
        let check = VersionCheck {
            cli_version: "2.0.0".to_string(),
            service_version: "2.0.0".to_string(),
        };
        assert_eq!(check_compatibility(&check), VersionCheckResult::Compatible);
    }

    #[test]
    fn minor_mismatch_warning() {
        let check = VersionCheck {
            cli_version: "1.3.0".to_string(),
            service_version: "1.2.0".to_string(),
        };
        let result = check_compatibility(&check);
        assert!(matches!(result, VersionCheckResult::MinorMismatch { .. }));
        let warning = format_warning(&result);
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("Warning"));
    }

    #[test]
    fn major_mismatch_error() {
        let check = VersionCheck {
            cli_version: "2.0.0".to_string(),
            service_version: "1.5.0".to_string(),
        };
        let result = check_compatibility(&check);
        assert!(matches!(result, VersionCheckResult::MajorMismatch { .. }));
        let warning = format_warning(&result);
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("Error"));
    }

    #[test]
    fn compatible_returns_no_warning() {
        let result = VersionCheckResult::Compatible;
        assert!(format_warning(&result).is_none());
    }

    #[test]
    fn invalid_version_treated_as_major_mismatch() {
        let check = VersionCheck {
            cli_version: "not-a-version".to_string(),
            service_version: "1.0.0".to_string(),
        };
        let result = check_compatibility(&check);
        assert!(matches!(result, VersionCheckResult::MajorMismatch { .. }));
    }

    #[test]
    fn semver_parse_valid() {
        let v = SemVer::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn semver_parse_invalid_returns_none() {
        assert!(SemVer::parse("1.2").is_none());
        assert!(SemVer::parse("abc").is_none());
        assert!(SemVer::parse("").is_none());
    }

    #[test]
    fn semver_display() {
        let v = SemVer {
            major: 3,
            minor: 1,
            patch: 4,
        };
        assert_eq!(v.to_string(), "3.1.4");
    }
}
