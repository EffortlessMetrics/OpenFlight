// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Update manifest system for version management, channel selection,
//! and integrity verification (REQ-911 through REQ-914).

use sha2::{Digest, Sha256};
use std::cmp::Ordering;

/// An update channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateChannel {
    Stable,
    Beta,
    Canary,
}

/// A single version entry in the update manifest.
#[derive(Debug, Clone)]
pub struct VersionEntry {
    pub version: String,
    pub channel: UpdateChannel,
    pub release_date: String,
    pub download_url: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub signature: Option<String>,
    pub release_notes: String,
    pub min_version: Option<String>,
    pub delta_url: Option<String>,
    pub delta_size_bytes: Option<u64>,
    pub delta_sha256: Option<String>,
}

/// The full update manifest.
#[derive(Debug, Clone)]
pub struct UpdateManifest {
    pub schema_version: u32,
    pub entries: Vec<VersionEntry>,
    pub manifest_sha256: String,
    pub manifest_signature: Option<String>,
}

/// Manages update channels, version checking, and update history.
pub struct ManifestUpdateManager {
    current_version: String,
    channel: UpdateChannel,
    manifest: Option<UpdateManifest>,
    update_history: Vec<UpdateRecord>,
}

/// Record of a completed (or failed) update attempt.
#[derive(Debug, Clone)]
pub struct UpdateRecord {
    pub from_version: String,
    pub to_version: String,
    pub timestamp: u64,
    pub success: bool,
    pub was_delta: bool,
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Parse a semver string `"major.minor.patch"` into its numeric components.
pub fn parse_semver(version: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let major = parts[0].parse::<u32>().ok()?;
    let minor = parts[1].parse::<u32>().ok()?;
    let patch = parts[2].parse::<u32>().ok()?;
    Some((major, minor, patch))
}

/// Compare two semver strings. Non-parseable versions are treated as equal.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    match (parse_semver(a), parse_semver(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        _ => Ordering::Equal,
    }
}

// ---------------------------------------------------------------------------
// ManifestUpdateManager
// ---------------------------------------------------------------------------

impl ManifestUpdateManager {
    /// Create a new manager for the given installed version and channel.
    pub fn new(current_version: &str, channel: UpdateChannel) -> Self {
        Self {
            current_version: current_version.to_string(),
            channel,
            manifest: None,
            update_history: Vec::new(),
        }
    }

    /// Switch to a different update channel.
    pub fn set_channel(&mut self, channel: UpdateChannel) {
        self.channel = channel;
    }

    /// Return the currently selected channel.
    pub fn current_channel(&self) -> UpdateChannel {
        self.channel
    }

    /// Load (or replace) the update manifest.
    pub fn load_manifest(&mut self, manifest: UpdateManifest) {
        self.manifest = Some(manifest);
    }

    /// Find the latest available update for the current channel that is newer
    /// than the installed version.
    pub fn check_for_update(&self) -> Option<&VersionEntry> {
        let manifest = self.manifest.as_ref()?;
        manifest
            .entries
            .iter()
            .filter(|e| e.channel == self.channel)
            .filter(|e| compare_versions(&e.version, &self.current_version) == Ordering::Greater)
            .max_by(|a, b| compare_versions(&a.version, &b.version))
    }

    /// Convenience: returns `true` when an update is available.
    pub fn is_update_available(&self) -> bool {
        self.check_for_update().is_some()
    }

    /// Check whether a delta update from the current version exists.
    pub fn can_delta_update(&self) -> bool {
        match self.check_for_update() {
            Some(entry) => {
                entry.delta_url.is_some()
                    && entry
                        .min_version
                        .as_ref()
                        .is_some_and(|mv| mv == &self.current_version)
            }
            None => false,
        }
    }

    /// Verify the SHA-256 integrity of downloaded data against a version entry.
    pub fn verify_integrity(entry: &VersionEntry, data: &[u8]) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let digest = hasher.finalize();
        let hex_digest = hex::encode(digest);
        hex_digest == entry.sha256
    }

    /// Append an update record to the history log.
    pub fn record_update(&mut self, record: UpdateRecord) {
        self.update_history.push(record);
    }

    /// Return the full update history.
    pub fn update_history(&self) -> &[UpdateRecord] {
        &self.update_history
    }

    /// Fraction of recorded updates that succeeded (0.0 when history is empty).
    pub fn success_rate(&self) -> f64 {
        if self.update_history.is_empty() {
            return 0.0;
        }
        let successes = self.update_history.iter().filter(|r| r.success).count();
        successes as f64 / self.update_history.len() as f64
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal `VersionEntry`.
    fn make_entry(version: &str, channel: UpdateChannel) -> VersionEntry {
        VersionEntry {
            version: version.to_string(),
            channel,
            release_date: "2025-01-01".to_string(),
            download_url: format!("https://example.com/{version}"),
            size_bytes: 1024,
            sha256: String::new(),
            signature: None,
            release_notes: "release notes".to_string(),
            min_version: None,
            delta_url: None,
            delta_size_bytes: None,
            delta_sha256: None,
        }
    }

    fn make_manifest(entries: Vec<VersionEntry>) -> UpdateManifest {
        UpdateManifest {
            schema_version: 1,
            entries,
            manifest_sha256: String::new(),
            manifest_signature: None,
        }
    }

    // 1. Create manager with version and channel
    #[test]
    fn test_update_manifest_create_manager() {
        let mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
        assert_eq!(mgr.current_channel(), UpdateChannel::Stable);
    }

    // 2. No update when no manifest loaded
    #[test]
    fn test_update_manifest_no_update_without_manifest() {
        let mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
        assert!(!mgr.is_update_available());
        assert!(mgr.check_for_update().is_none());
    }

    // 3. Find latest update for channel
    #[test]
    fn test_update_manifest_find_latest_for_channel() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
        let manifest = make_manifest(vec![
            make_entry("1.1.0", UpdateChannel::Stable),
            make_entry("1.2.0", UpdateChannel::Stable),
        ]);
        mgr.load_manifest(manifest);

        let update = mgr.check_for_update().unwrap();
        assert_eq!(update.version, "1.2.0");
    }

    // 4. Skip updates for other channels
    #[test]
    fn test_update_manifest_skip_other_channels() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
        let manifest = make_manifest(vec![
            make_entry("2.0.0", UpdateChannel::Beta),
            make_entry("3.0.0", UpdateChannel::Canary),
        ]);
        mgr.load_manifest(manifest);

        assert!(!mgr.is_update_available());
    }

    // 5. Delta update available when min_version matches
    #[test]
    fn test_update_manifest_delta_available() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
        let mut entry = make_entry("1.1.0", UpdateChannel::Stable);
        entry.min_version = Some("1.0.0".to_string());
        entry.delta_url = Some("https://example.com/delta".to_string());
        entry.delta_size_bytes = Some(512);
        entry.delta_sha256 = Some("abc".to_string());

        mgr.load_manifest(make_manifest(vec![entry]));
        assert!(mgr.can_delta_update());
    }

    // 6. Integrity verification with valid hash
    #[test]
    fn test_update_manifest_integrity_valid() {
        let data = b"hello world";
        let expected = sha2_hex(data);

        let mut entry = make_entry("1.0.0", UpdateChannel::Stable);
        entry.sha256 = expected;

        assert!(ManifestUpdateManager::verify_integrity(&entry, data));
    }

    // 7. Integrity verification fails with wrong hash
    #[test]
    fn test_update_manifest_integrity_invalid() {
        let mut entry = make_entry("1.0.0", UpdateChannel::Stable);
        entry.sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string();

        assert!(!ManifestUpdateManager::verify_integrity(
            &entry,
            b"hello world"
        ));
    }

    // 8. Version comparison
    #[test]
    fn test_update_manifest_version_comparison() {
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "1.0.1"), Ordering::Less);
        assert_eq!(compare_versions("2.0.0", "1.9.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.10.0", "1.9.0"), Ordering::Greater);
    }

    // 9. Update history tracking
    #[test]
    fn test_update_manifest_history_tracking() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
        assert!(mgr.update_history().is_empty());

        mgr.record_update(UpdateRecord {
            from_version: "1.0.0".to_string(),
            to_version: "1.1.0".to_string(),
            timestamp: 1000,
            success: true,
            was_delta: false,
        });

        assert_eq!(mgr.update_history().len(), 1);
        assert_eq!(mgr.update_history()[0].to_version, "1.1.0");
    }

    // 10. Success rate calculation
    #[test]
    fn test_update_manifest_success_rate() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
        assert_eq!(mgr.success_rate(), 0.0);

        mgr.record_update(UpdateRecord {
            from_version: "1.0.0".to_string(),
            to_version: "1.1.0".to_string(),
            timestamp: 1000,
            success: true,
            was_delta: false,
        });
        mgr.record_update(UpdateRecord {
            from_version: "1.1.0".to_string(),
            to_version: "1.2.0".to_string(),
            timestamp: 2000,
            success: false,
            was_delta: true,
        });

        let rate = mgr.success_rate();
        assert!((rate - 0.5).abs() < f64::EPSILON);
    }

    // 11. Channel switching
    #[test]
    fn test_update_manifest_channel_switching() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
        assert_eq!(mgr.current_channel(), UpdateChannel::Stable);

        mgr.set_channel(UpdateChannel::Beta);
        assert_eq!(mgr.current_channel(), UpdateChannel::Beta);

        mgr.set_channel(UpdateChannel::Canary);
        assert_eq!(mgr.current_channel(), UpdateChannel::Canary);
    }

    // 12. Semver parsing
    #[test]
    fn test_update_manifest_semver_parsing() {
        assert_eq!(parse_semver("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("0.0.0"), Some((0, 0, 0)));
        assert_eq!(parse_semver("10.20.30"), Some((10, 20, 30)));
        assert!(parse_semver("1.2").is_none());
        assert!(parse_semver("abc").is_none());
        assert!(parse_semver("1.2.x").is_none());
    }

    /// Compute hex-encoded SHA-256 for test data.
    fn sha2_hex(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }
}
