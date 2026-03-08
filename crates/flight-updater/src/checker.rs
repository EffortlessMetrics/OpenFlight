// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Update checker — compares the installed version against a release manifest
//! and determines whether an update is available.

use crate::channels::Channel;
use crate::manifest::{ReleaseManifest, SemVer};

/// Result of an update check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheckResult {
    /// A newer version is available.
    UpdateAvailable { from: SemVer, to: SemVer },
    /// The installed version is up to date.
    UpToDate,
    /// The manifest targets a different channel.
    ChannelMismatch { expected: Channel, actual: Channel },
}

/// Compares the running version/channel against release manifests.
#[derive(Debug, Clone)]
pub struct UpdateChecker {
    current_version: SemVer,
    current_channel: Channel,
}

impl UpdateChecker {
    pub fn new(current_version: SemVer, channel: Channel) -> Self {
        Self {
            current_version,
            current_channel: channel,
        }
    }

    /// Check whether the given release manifest represents an available update.
    pub fn check(&self, manifest: &ReleaseManifest) -> UpdateCheckResult {
        if manifest.channel != self.current_channel {
            return UpdateCheckResult::ChannelMismatch {
                expected: self.current_channel,
                actual: manifest.channel,
            };
        }
        if manifest.version > self.current_version {
            UpdateCheckResult::UpdateAvailable {
                from: self.current_version.clone(),
                to: manifest.version.clone(),
            }
        } else {
            UpdateCheckResult::UpToDate
        }
    }

    /// Convenience: returns `true` when an update is available.
    pub fn is_update_available(&self, manifest: &ReleaseManifest) -> bool {
        matches!(
            self.check(manifest),
            UpdateCheckResult::UpdateAvailable { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Architecture, Platform, PlatformArtifact};

    fn sample_artifact() -> PlatformArtifact {
        PlatformArtifact {
            url: "https://dl.example.com/win-x64.zip".into(),
            platform: Platform::Windows,
            architecture: Architecture::X64,
            sha256: "aa".repeat(32),
            size_bytes: 10_000,
        }
    }

    fn manifest_with_version(
        major: u32,
        minor: u32,
        patch: u32,
        channel: Channel,
    ) -> ReleaseManifest {
        ReleaseManifest {
            version: SemVer::new(major, minor, patch),
            channel,
            release_date: "2025-07-10".into(),
            artifacts: vec![sample_artifact()],
            release_notes: None,
        }
    }

    #[test]
    fn update_available_when_newer() {
        let checker = UpdateChecker::new(SemVer::new(1, 0, 0), Channel::Stable);
        let manifest = manifest_with_version(2, 0, 0, Channel::Stable);
        assert_eq!(
            checker.check(&manifest),
            UpdateCheckResult::UpdateAvailable {
                from: SemVer::new(1, 0, 0),
                to: SemVer::new(2, 0, 0),
            }
        );
    }

    #[test]
    fn no_update_when_same_version() {
        let checker = UpdateChecker::new(SemVer::new(1, 0, 0), Channel::Stable);
        let manifest = manifest_with_version(1, 0, 0, Channel::Stable);
        assert_eq!(checker.check(&manifest), UpdateCheckResult::UpToDate);
    }

    #[test]
    fn no_update_when_older_version() {
        let checker = UpdateChecker::new(SemVer::new(2, 0, 0), Channel::Stable);
        let manifest = manifest_with_version(1, 0, 0, Channel::Stable);
        assert_eq!(checker.check(&manifest), UpdateCheckResult::UpToDate);
    }

    #[test]
    fn channel_mismatch_detected() {
        let checker = UpdateChecker::new(SemVer::new(1, 0, 0), Channel::Stable);
        let manifest = manifest_with_version(2, 0, 0, Channel::Beta);
        assert_eq!(
            checker.check(&manifest),
            UpdateCheckResult::ChannelMismatch {
                expected: Channel::Stable,
                actual: Channel::Beta,
            }
        );
    }

    #[test]
    fn update_available_minor_bump() {
        let checker = UpdateChecker::new(SemVer::new(1, 0, 0), Channel::Stable);
        let manifest = manifest_with_version(1, 1, 0, Channel::Stable);
        assert!(checker.is_update_available(&manifest));
    }

    #[test]
    fn update_available_patch_bump() {
        let checker = UpdateChecker::new(SemVer::new(1, 0, 0), Channel::Stable);
        let manifest = manifest_with_version(1, 0, 1, Channel::Stable);
        assert!(checker.is_update_available(&manifest));
    }

    #[test]
    fn beta_channel_update() {
        let checker = UpdateChecker::new(SemVer::new(1, 0, 0), Channel::Beta);
        let manifest = manifest_with_version(2, 0, 0, Channel::Beta);
        assert!(checker.is_update_available(&manifest));
    }
}
