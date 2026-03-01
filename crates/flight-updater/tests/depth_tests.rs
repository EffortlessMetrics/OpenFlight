// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the flight-updater crate.
//!
//! Groups:
//!   1. Manifest parsing & validation (12 tests)
//!   2. SemVer & version comparison  (10 tests)
//!   3. Signature verification       (8 tests)
//!   4. Channel system               (7 tests)
//!   5. Policy engine                (8 tests)
//!   6. Rollback & state machine     (9 tests)
//!   7. Delta & compression          (6 tests)
//!   8. Error handling               (4 tests)
//!   9. Property-based (proptest)    (5 tests)

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use ed25519_dalek::{Signer, SigningKey};
use rand_core::OsRng;
use sha2::{Digest, Sha256};

use flight_updater::channels::{Channel, ChannelConfig, ChannelManager};
use flight_updater::delta::{
    calculate_delta, DeltaApplier, DeltaOperation, DeltaPatch, FileDelta,
};
use flight_updater::manifest::{
    parse as parse_manifest, verify_signature as verify_manifest_signature, FileOperation,
    FileUpdate, SemVer, UpdateManifest as SignedUpdateManifest,
};
use flight_updater::policy::{
    should_apply, CurrentState, UpdateDecision, UpdatePolicy as ManifestUpdatePolicy,
};
use flight_updater::rollback::{
    ArtifactFile, FileSystem, UpdateJournal, UpdateRollbackConfig, UpdateRollbackManager,
    UpdateState, VersionInfo,
};
use flight_updater::signed_manifest::{
    Arch, ArtifactEntry as SmArtifactEntry, InstallerType, ManifestValidationError, Platform,
    UpdateManifest as SmUpdateManifest, UpdatePolicy as SmUpdatePolicy,
};
use flight_updater::update_manifest::{
    compare_versions, parse_semver, ManifestUpdateManager, UpdateChannel,
    UpdateManifest as UmUpdateManifest, UpdateRecord, VersionEntry,
};
use flight_updater::updater::{UpdateConfig, UpdateResult};

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn make_signed_manifest_with_key() -> (SignedUpdateManifest, SigningKey) {
    let sk = SigningKey::generate(&mut OsRng);
    let mut manifest = SignedUpdateManifest {
        version: SemVer::new(2, 0, 0),
        channel: Channel::Stable,
        files: vec![
            FileUpdate {
                path: "bin/app.exe".into(),
                hash_before: "aa".repeat(32),
                hash_after: "bb".repeat(32),
                size: 2048,
                operation: FileOperation::Modify,
            },
            FileUpdate {
                path: "lib/core.dll".into(),
                hash_before: String::new(),
                hash_after: "cc".repeat(32),
                size: 4096,
                operation: FileOperation::Add,
            },
        ],
        signature: String::new(),
        min_version: Some(SemVer::new(1, 0, 0)),
    };
    let msg = manifest.canonical_bytes();
    let sig = sk.sign(&msg);
    manifest.signature = hex::encode(sig.to_bytes());
    (manifest, sk)
}

fn make_version_entry(version: &str, channel: UpdateChannel) -> VersionEntry {
    VersionEntry {
        version: version.to_string(),
        channel,
        release_date: "2025-01-15".to_string(),
        download_url: format!("https://dl.example.com/{version}"),
        size_bytes: 4096,
        sha256: String::new(),
        signature: None,
        release_notes: "Bug fixes.".to_string(),
        min_version: None,
        delta_url: None,
        delta_size_bytes: None,
        delta_sha256: None,
    }
}

fn make_um_manifest(entries: Vec<VersionEntry>) -> UmUpdateManifest {
    UmUpdateManifest {
        schema_version: 1,
        entries,
        manifest_sha256: String::new(),
        manifest_signature: None,
    }
}

fn valid_sha256() -> String {
    "a".repeat(64)
}

fn sample_sm_artifact() -> SmArtifactEntry {
    SmArtifactEntry {
        platform: Platform::Windows,
        arch: Arch::X86_64,
        url: "https://dl.example.com/flight-2.0.0-win-x64.msi".into(),
        sha256: valid_sha256(),
        size_bytes: 50_000_000,
        installer_type: InstallerType::Msi,
    }
}

fn sample_sm_manifest() -> SmUpdateManifest {
    SmUpdateManifest {
        version: "2.0.0".into(),
        channel: Channel::Stable,
        release_date: "2025-06-01T00:00:00Z".into(),
        artifacts: vec![sample_sm_artifact()],
        min_version: "1.0.0".into(),
        release_notes: "Major update with stability improvements.".into(),
    }
}

// ── MockFs ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct MockFs {
    files: Rc<RefCell<HashMap<PathBuf, Vec<u8>>>>,
    dirs: Rc<RefCell<HashSet<PathBuf>>>,
}

impl MockFs {
    fn new_mock() -> Self {
        Self {
            files: Rc::new(RefCell::new(HashMap::new())),
            dirs: Rc::new(RefCell::new(HashSet::new())),
        }
    }

    fn add_file(&self, path: &str, data: &[u8]) {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            self.ensure_parents(parent);
        }
        self.files.borrow_mut().insert(path, data.to_vec());
    }

    fn ensure_parents(&self, path: &Path) {
        let mut current = PathBuf::new();
        for component in path.components() {
            current.push(component);
            self.dirs.borrow_mut().insert(current.clone());
        }
    }
}

impl FileSystem for MockFs {
    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.files
            .borrow()
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, path.display().to_string()))
    }

    fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            self.ensure_parents(parent);
        }
        self.files
            .borrow_mut()
            .insert(path.to_path_buf(), data.to_vec());
        Ok(())
    }

    fn append_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let mut files = self.files.borrow_mut();
        let entry = files.entry(path.to_path_buf()).or_default();
        entry.extend_from_slice(data);
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.files.borrow_mut().remove(path);
        Ok(())
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.ensure_parents(path);
        self.dirs.borrow_mut().insert(path.to_path_buf());
        Ok(())
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        let path_buf = path.to_path_buf();
        self.files
            .borrow_mut()
            .retain(|k, _| !k.starts_with(&path_buf));
        self.dirs
            .borrow_mut()
            .retain(|d| !d.starts_with(&path_buf));
        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        let pb = path.to_path_buf();
        self.files.borrow().contains_key(&pb) || self.dirs.borrow().contains(&pb)
    }

    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let pb = path.to_path_buf();
        let mut entries = HashSet::new();
        for key in self.files.borrow().keys() {
            if let Ok(rel) = key.strip_prefix(&pb)
                && let Some(first) = rel.components().next()
            {
                entries.insert(pb.join(first));
            }
        }
        for dir in self.dirs.borrow().iter() {
            if let Ok(rel) = dir.strip_prefix(&pb)
                && !rel.as_os_str().is_empty()
                && let Some(first) = rel.components().next()
            {
                entries.insert(pb.join(first));
            }
        }
        Ok(entries.into_iter().collect())
    }

    fn is_dir(&self, path: &Path) -> bool {
        self.dirs.borrow().contains(&path.to_path_buf())
    }
}

fn mock_config(base: &str) -> UpdateRollbackConfig {
    UpdateRollbackConfig {
        backup_dir: PathBuf::from(format!("{base}/backups")),
        install_dir: PathBuf::from(format!("{base}/install")),
        journal_path: PathBuf::from(format!("{base}/journal.log")),
        max_backups: 3,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. MANIFEST PARSING & VALIDATION (12 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// 1.1 Parse valid signed manifest preserves all fields.
#[test]
fn manifest_parse_valid_preserves_fields() {
    let (manifest, _sk) = make_signed_manifest_with_key();
    let json = serde_json::to_vec(&manifest).unwrap();
    let parsed = parse_manifest(&json).unwrap();

    assert_eq!(parsed.version, SemVer::new(2, 0, 0));
    assert_eq!(parsed.channel, Channel::Stable);
    assert_eq!(parsed.files.len(), 2);
    assert_eq!(parsed.files[0].operation, FileOperation::Modify);
    assert_eq!(parsed.files[1].operation, FileOperation::Add);
    assert_eq!(parsed.min_version, Some(SemVer::new(1, 0, 0)));
}

/// 1.2 Parse empty bytes returns error.
#[test]
fn manifest_parse_empty_bytes_error() {
    assert!(parse_manifest(b"").is_err());
}

/// 1.3 Parse malformed JSON returns serialization error.
#[test]
fn manifest_parse_malformed_json_error() {
    assert!(parse_manifest(b"{{{{not json").is_err());
}

/// 1.4 Parse JSON with missing required fields returns error.
#[test]
fn manifest_parse_missing_fields_error() {
    let incomplete = r#"{"version":{"major":1,"minor":0,"patch":0}}"#;
    assert!(parse_manifest(incomplete.as_bytes()).is_err());
}

/// 1.5 Manifest with no files parses successfully (files is empty vec).
#[test]
fn manifest_parse_empty_files_list() {
    let manifest = SignedUpdateManifest {
        version: SemVer::new(1, 0, 0),
        channel: Channel::Beta,
        files: vec![],
        signature: String::new(),
        min_version: None,
    };
    let json = serde_json::to_vec(&manifest).unwrap();
    let parsed = parse_manifest(&json).unwrap();
    assert!(parsed.files.is_empty());
    assert!(parsed.min_version.is_none());
}

/// 1.6 Manifest with min_version=None round-trips correctly.
#[test]
fn manifest_parse_no_min_version() {
    let manifest = SignedUpdateManifest {
        version: SemVer::new(3, 1, 0),
        channel: Channel::Canary,
        files: vec![],
        signature: "deadbeef".into(),
        min_version: None,
    };
    let json = serde_json::to_vec(&manifest).unwrap();
    let parsed = parse_manifest(&json).unwrap();
    assert!(parsed.min_version.is_none());
    assert_eq!(parsed.signature, "deadbeef");
}

/// 1.7 SmUpdateManifest validation catches invalid semver in version.
#[test]
fn sm_manifest_validate_invalid_version() {
    let mut m = sample_sm_manifest();
    m.version = "not-a-version".into();
    let errors = m.validate();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ManifestValidationError::InvalidSemver(_)))
    );
}

/// 1.8 SmUpdateManifest validation catches invalid semver in min_version.
#[test]
fn sm_manifest_validate_invalid_min_version() {
    let mut m = sample_sm_manifest();
    m.min_version = "xyz".into();
    let errors = m.validate();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ManifestValidationError::InvalidSemver(s) if s == "xyz"))
    );
}

/// 1.9 SmUpdateManifest validation catches empty artifacts list.
#[test]
fn sm_manifest_validate_empty_artifacts() {
    let mut m = sample_sm_manifest();
    m.artifacts.clear();
    let errors = m.validate();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ManifestValidationError::EmptyArtifacts))
    );
}

/// 1.10 SmUpdateManifest validation catches empty release notes.
#[test]
fn sm_manifest_validate_empty_release_notes() {
    let mut m = sample_sm_manifest();
    m.release_notes = String::new();
    let errors = m.validate();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ManifestValidationError::EmptyReleaseNotes))
    );
}

/// 1.11 SmUpdateManifest validation catches invalid SHA256 in artifact.
#[test]
fn sm_manifest_validate_bad_artifact_sha256() {
    let mut m = sample_sm_manifest();
    m.artifacts[0].sha256 = "too-short".into();
    let errors = m.validate();
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, ManifestValidationError::InvalidSha256 { .. }))
    );
}

/// 1.12 SmUpdateManifest with multiple errors reports them all at once.
#[test]
fn sm_manifest_validate_multiple_errors() {
    let m = SmUpdateManifest {
        version: "bad".into(),
        channel: Channel::Stable,
        release_date: "2025-01-01".into(),
        artifacts: vec![],
        min_version: "also-bad".into(),
        release_notes: String::new(),
    };
    let errors = m.validate();
    // bad version + bad min_version + empty artifacts + empty release notes
    assert!(errors.len() >= 4, "expected ≥4 errors, got {errors:?}");
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. SEMVER & VERSION COMPARISON (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// 2.1 SemVer::parse valid triple.
#[test]
fn semver_parse_valid_triple() {
    let v = SemVer::parse("10.20.30").unwrap();
    assert_eq!(v, SemVer::new(10, 20, 30));
}

/// 2.2 SemVer::parse rejects two-part.
#[test]
fn semver_parse_rejects_two_part() {
    assert!(SemVer::parse("1.2").is_none());
}

/// 2.3 SemVer::parse rejects four-part.
#[test]
fn semver_parse_rejects_four_part() {
    assert!(SemVer::parse("1.2.3.4").is_none());
}

/// 2.4 SemVer::parse rejects non-numeric components.
#[test]
fn semver_parse_rejects_non_numeric() {
    assert!(SemVer::parse("a.b.c").is_none());
    assert!(SemVer::parse("1.x.0").is_none());
}

/// 2.5 SemVer ordering: major takes precedence.
#[test]
fn semver_ordering_major_precedence() {
    assert!(SemVer::new(2, 0, 0) > SemVer::new(1, 99, 99));
}

/// 2.6 SemVer ordering: minor takes precedence over patch.
#[test]
fn semver_ordering_minor_precedence() {
    assert!(SemVer::new(1, 2, 0) > SemVer::new(1, 1, 99));
}

/// 2.7 SemVer equality.
#[test]
fn semver_equality() {
    assert_eq!(SemVer::new(0, 0, 0), SemVer::new(0, 0, 0));
    assert_eq!(SemVer::new(u32::MAX, u32::MAX, u32::MAX), SemVer::new(u32::MAX, u32::MAX, u32::MAX));
}

/// 2.8 SemVer Display formatting.
#[test]
fn semver_display_formatting() {
    assert_eq!(SemVer::new(0, 0, 1).to_string(), "0.0.1");
    assert_eq!(SemVer::new(100, 200, 300).to_string(), "100.200.300");
}

/// 2.9 parse_semver from update_manifest module matches SemVer::parse.
#[test]
fn parse_semver_consistency() {
    assert_eq!(parse_semver("1.2.3"), Some((1, 2, 3)));
    assert_eq!(parse_semver("0.0.0"), Some((0, 0, 0)));
    assert!(parse_semver("abc").is_none());
    assert!(parse_semver("").is_none());
}

/// 2.10 compare_versions ordering across various pairs.
#[test]
fn compare_versions_full_ordering() {
    use std::cmp::Ordering;
    assert_eq!(compare_versions("1.0.0", "1.0.0"), Ordering::Equal);
    assert_eq!(compare_versions("1.0.0", "1.0.1"), Ordering::Less);
    assert_eq!(compare_versions("2.0.0", "1.9.9"), Ordering::Greater);
    assert_eq!(compare_versions("1.10.0", "1.9.0"), Ordering::Greater);
    // Non-parseable versions treated as equal
    assert_eq!(compare_versions("bad", "worse"), Ordering::Equal);
    assert_eq!(compare_versions("1.0.0", "bad"), Ordering::Equal);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. SIGNATURE VERIFICATION (8 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// 3.1 Valid signature verifies successfully.
#[test]
fn sig_valid_keypair_verifies() {
    let (manifest, sk) = make_signed_manifest_with_key();
    let pk_hex = hex::encode(sk.verifying_key().to_bytes());
    assert!(verify_manifest_signature(&manifest, &pk_hex).is_ok());
}

/// 3.2 Wrong key rejects signature.
#[test]
fn sig_wrong_key_rejects() {
    let (manifest, _sk) = make_signed_manifest_with_key();
    let wrong_sk = SigningKey::generate(&mut OsRng);
    let wrong_pk_hex = hex::encode(wrong_sk.verifying_key().to_bytes());
    assert!(verify_manifest_signature(&manifest, &wrong_pk_hex).is_err());
}

/// 3.3 Tampered version field invalidates signature.
#[test]
fn sig_tampered_version_fails() {
    let (mut manifest, sk) = make_signed_manifest_with_key();
    let pk_hex = hex::encode(sk.verifying_key().to_bytes());
    manifest.version = SemVer::new(99, 0, 0);
    assert!(verify_manifest_signature(&manifest, &pk_hex).is_err());
}

/// 3.4 Tampered channel field invalidates signature.
#[test]
fn sig_tampered_channel_fails() {
    let (mut manifest, sk) = make_signed_manifest_with_key();
    let pk_hex = hex::encode(sk.verifying_key().to_bytes());
    manifest.channel = Channel::Canary;
    assert!(verify_manifest_signature(&manifest, &pk_hex).is_err());
}

/// 3.5 Tampered files list invalidates signature.
#[test]
fn sig_tampered_files_fails() {
    let (mut manifest, sk) = make_signed_manifest_with_key();
    let pk_hex = hex::encode(sk.verifying_key().to_bytes());
    manifest.files.push(FileUpdate {
        path: "extra.dll".into(),
        hash_before: String::new(),
        hash_after: "dd".repeat(32),
        size: 512,
        operation: FileOperation::Add,
    });
    assert!(verify_manifest_signature(&manifest, &pk_hex).is_err());
}

/// 3.6 Non-hex public key returns error (not panic).
#[test]
fn sig_non_hex_public_key_error() {
    let (manifest, _sk) = make_signed_manifest_with_key();
    assert!(verify_manifest_signature(&manifest, "not-hex!!!").is_err());
}

/// 3.7 Too-short public key returns error.
#[test]
fn sig_short_public_key_error() {
    let (manifest, _sk) = make_signed_manifest_with_key();
    assert!(verify_manifest_signature(&manifest, "aabb").is_err());
}

/// 3.8 Empty signature field fails verification.
#[test]
fn sig_empty_signature_field_fails() {
    let sk = SigningKey::generate(&mut OsRng);
    let pk_hex = hex::encode(sk.verifying_key().to_bytes());
    let manifest = SignedUpdateManifest {
        version: SemVer::new(1, 0, 0),
        channel: Channel::Stable,
        files: vec![],
        signature: String::new(),
        min_version: None,
    };
    assert!(verify_manifest_signature(&manifest, &pk_hex).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. CHANNEL SYSTEM (7 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// 4.1 ChannelManager initializes with three channels.
#[test]
fn channel_manager_has_three_channels() {
    let mgr = ChannelManager::new();
    let channels = mgr.available_channels();
    assert_eq!(channels.len(), 3);
}

/// 4.2 Channel Display formatting.
#[test]
fn channel_display() {
    assert_eq!(Channel::Stable.to_string(), "stable");
    assert_eq!(Channel::Beta.to_string(), "beta");
    assert_eq!(Channel::Canary.to_string(), "canary");
}

/// 4.3 Channel from_str is case-insensitive.
#[test]
fn channel_from_str_case_insensitive() {
    assert_eq!("STABLE".parse::<Channel>().unwrap(), Channel::Stable);
    assert_eq!("Beta".parse::<Channel>().unwrap(), Channel::Beta);
    assert_eq!("canary".parse::<Channel>().unwrap(), Channel::Canary);
}

/// 4.4 Channel from_str rejects unknown channel names.
#[test]
fn channel_from_str_rejects_unknown() {
    let err = "nightly".parse::<Channel>().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("nightly"), "error must mention the bad name: {msg}");
}

/// 4.5 Validate_config rejects zero check frequency.
#[test]
fn channel_validate_rejects_zero_frequency() {
    let mut mgr = ChannelManager::new();
    let mut cfg = mgr.get_config(Channel::Stable).unwrap().clone();
    cfg.check_frequency_hours = 0;
    mgr.update_config(Channel::Stable, cfg);
    assert!(mgr.validate_config(Channel::Stable).is_err());
}

/// 4.6 Validate_config rejects empty URL.
#[test]
fn channel_validate_rejects_empty_url() {
    let mut mgr = ChannelManager::new();
    let mut cfg = mgr.get_config(Channel::Stable).unwrap().clone();
    cfg.update_url = String::new();
    mgr.update_config(Channel::Stable, cfg);
    assert!(mgr.validate_config(Channel::Stable).is_err());
}

/// 4.7 Validate_config rejects empty public key.
#[test]
fn channel_validate_rejects_empty_public_key() {
    let mut mgr = ChannelManager::new();
    let cfg = ChannelConfig {
        channel: Channel::Stable,
        check_frequency_hours: 24,
        auto_install: false,
        accept_prerelease: false,
        update_url: "https://example.com".into(),
        public_key: String::new(),
    };
    mgr.update_config(Channel::Stable, cfg);
    assert!(mgr.validate_config(Channel::Stable).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. POLICY ENGINE (8 tests)
// ═══════════════════════════════════════════════════════════════════════════

fn auto_policy() -> ManifestUpdatePolicy {
    ManifestUpdatePolicy {
        auto_apply: true,
        ..Default::default()
    }
}

fn default_state() -> CurrentState {
    CurrentState {
        installed_version: SemVer::new(1, 0, 0),
        sim_running: false,
        update_channel: Channel::Stable,
        update_version: SemVer::new(2, 0, 0),
        update_min_version: None,
    }
}

/// 5.1 Apply when auto_apply on and channel allowed.
#[test]
fn policy_apply_when_all_conditions_met() {
    assert_eq!(should_apply(&auto_policy(), &default_state()), UpdateDecision::Apply);
}

/// 5.2 Skip when channel not allowed.
#[test]
fn policy_skip_disallowed_channel() {
    let state = CurrentState {
        update_channel: Channel::Canary,
        ..default_state()
    };
    assert!(matches!(should_apply(&auto_policy(), &state), UpdateDecision::Skip(_)));
}

/// 5.3 Skip when already at offered version.
#[test]
fn policy_skip_same_version() {
    let state = CurrentState {
        installed_version: SemVer::new(2, 0, 0),
        update_version: SemVer::new(2, 0, 0),
        ..default_state()
    };
    assert!(matches!(should_apply(&auto_policy(), &state), UpdateDecision::Skip(_)));
}

/// 5.4 Skip when installed version ahead of update.
#[test]
fn policy_skip_ahead_of_update() {
    let state = CurrentState {
        installed_version: SemVer::new(3, 0, 0),
        ..default_state()
    };
    assert!(matches!(should_apply(&auto_policy(), &state), UpdateDecision::Skip(_)));
}

/// 5.5 Skip when installed version below min_version.
#[test]
fn policy_skip_below_min_version() {
    let state = CurrentState {
        installed_version: SemVer::new(0, 5, 0),
        update_min_version: Some(SemVer::new(1, 0, 0)),
        ..default_state()
    };
    assert!(matches!(should_apply(&auto_policy(), &state), UpdateDecision::Skip(_)));
}

/// 5.6 Defer when sim running.
#[test]
fn policy_defer_sim_running() {
    let state = CurrentState {
        sim_running: true,
        ..default_state()
    };
    let decision = should_apply(&auto_policy(), &state);
    assert!(matches!(decision, UpdateDecision::Defer(ref r) if r.contains("simulator")));
}

/// 5.7 Defer when auto_apply is off.
#[test]
fn policy_defer_auto_apply_off() {
    let policy = ManifestUpdatePolicy::default();
    assert!(matches!(should_apply(&policy, &default_state()), UpdateDecision::Defer(_)));
}

/// 5.8 Apply with multiple channels allowed including beta.
#[test]
fn policy_apply_beta_when_allowed() {
    let policy = ManifestUpdatePolicy {
        auto_apply: true,
        allowed_channels: vec![Channel::Stable, Channel::Beta],
        ..Default::default()
    };
    let state = CurrentState {
        update_channel: Channel::Beta,
        ..default_state()
    };
    assert_eq!(should_apply(&policy, &state), UpdateDecision::Apply);
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. ROLLBACK & STATE MACHINE (9 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// 6.1 Initial state is Idle.
#[test]
fn rollback_initial_state_idle() {
    let fs = MockFs::new_mock();
    fs.add_file("/app/install/bin.exe", b"v1");
    let config = mock_config("/app");
    let mgr = UpdateRollbackManager::new(config, fs).unwrap();
    assert_eq!(*mgr.state(), UpdateState::Idle);
}

/// 6.2 Successful apply_update transitions to Complete.
#[test]
fn rollback_apply_update_success_completes() {
    let fs = MockFs::new_mock();
    fs.add_file("/app/install/bin.exe", b"v1");

    let data = b"v2-binary-content";
    let hash = sha256_hex(data);
    fs.add_file("/tmp/v2.bin", data);

    let config = mock_config("/app");
    let mut mgr = UpdateRollbackManager::new(config, fs).unwrap();

    let artifacts = vec![ArtifactFile {
        path: PathBuf::from("/tmp/v2.bin"),
        expected_sha256: hash,
    }];
    mgr.apply_update(&artifacts, "2.0.0").unwrap();
    assert_eq!(*mgr.state(), UpdateState::Complete);
}

/// 6.3 apply_update from non-Idle state returns error.
#[test]
fn rollback_apply_from_non_idle_fails() {
    let fs = MockFs::new_mock();
    fs.add_file("/app/install/bin.exe", b"v1");

    let data = b"v2";
    let hash = sha256_hex(data);
    fs.add_file("/tmp/v2.bin", data);

    let config = mock_config("/app");
    let mut mgr = UpdateRollbackManager::new(config, fs).unwrap();
    let artifacts = vec![ArtifactFile {
        path: PathBuf::from("/tmp/v2.bin"),
        expected_sha256: hash,
    }];
    mgr.apply_update(&artifacts, "2.0.0").unwrap();
    // Now in Complete state — trying again must fail
    let result = mgr.apply_update(&artifacts, "3.0.0");
    assert!(result.is_err());
}

/// 6.4 verify_update_integrity catches hash mismatch.
#[test]
fn rollback_verify_integrity_mismatch() {
    let fs = MockFs::new_mock();
    fs.add_file("/tmp/art.bin", b"real-content");
    let config = mock_config("/app");
    let mgr = UpdateRollbackManager::new(config, fs).unwrap();

    let artifacts = vec![ArtifactFile {
        path: PathBuf::from("/tmp/art.bin"),
        expected_sha256: "0".repeat(64),
    }];
    assert!(mgr.verify_update_integrity(&artifacts).is_err());
}

/// 6.5 verify_update_integrity passes with correct hash.
#[test]
fn rollback_verify_integrity_correct() {
    let fs = MockFs::new_mock();
    let data = b"good-content";
    fs.add_file("/tmp/art.bin", data);
    let config = mock_config("/app");
    let mgr = UpdateRollbackManager::new(config, fs).unwrap();

    let artifacts = vec![ArtifactFile {
        path: PathBuf::from("/tmp/art.bin"),
        expected_sha256: sha256_hex(data),
    }];
    assert!(mgr.verify_update_integrity(&artifacts).is_ok());
}

/// 6.6 Journal records state transitions in order.
#[test]
fn rollback_journal_state_ordering() {
    let fs = MockFs::new_mock();
    let journal = UpdateJournal::new(PathBuf::from("/j.log"), fs);

    journal.record(&UpdateState::Downloading, "1.0.0", "start").unwrap();
    journal.record(&UpdateState::Verifying, "1.0.0", "verify").unwrap();
    journal.record(&UpdateState::Installing, "1.0.0", "install").unwrap();
    journal.record(&UpdateState::Complete, "1.0.0", "done").unwrap();

    let entries = journal.entries().unwrap();
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[0].state, UpdateState::Downloading);
    assert_eq!(entries[3].state, UpdateState::Complete);
}

/// 6.7 Journal check_incomplete detects non-terminal state.
#[test]
fn rollback_journal_incomplete_detected() {
    let fs = MockFs::new_mock();
    let journal = UpdateJournal::new(PathBuf::from("/j.log"), fs);
    journal.record(&UpdateState::Installing, "2.0.0", "stuck").unwrap();
    let incomplete = journal.check_incomplete().unwrap();
    assert!(incomplete.is_some());
    assert_eq!(incomplete.unwrap().version, "2.0.0");
}

/// 6.8 Journal check_incomplete returns None for terminal states.
#[test]
fn rollback_journal_terminal_not_incomplete() {
    let fs = MockFs::new_mock();
    let journal = UpdateJournal::new(PathBuf::from("/j.log"), fs);

    for terminal in [UpdateState::Complete, UpdateState::Failed, UpdateState::Idle] {
        journal.clear().unwrap();
        journal.record(&terminal, "1.0.0", "terminal").unwrap();
        assert!(
            journal.check_incomplete().unwrap().is_none(),
            "{terminal:?} should be terminal"
        );
    }
}

/// 6.9 Recovery on startup with available backup restores and returns to Idle.
#[test]
fn rollback_recover_on_startup_with_backup() {
    let fs = MockFs::new_mock();
    fs.add_file("/app/install/bin.exe", b"broken-v2");
    fs.add_file("/app/backups/backup_001/bin.exe", b"good-v1");

    let config = mock_config("/app");
    let mut mgr = UpdateRollbackManager::new(config, fs).unwrap();

    mgr.journal()
        .record(&UpdateState::Installing, "2.0.0", "interrupted")
        .unwrap();

    let recovered = mgr.recover_on_startup().unwrap();
    assert!(recovered);
    assert_eq!(*mgr.state(), UpdateState::Idle);
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. DELTA & COMPRESSION (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// 7.1 calculate_delta detects added, modified, and removed files.
#[test]
fn delta_calculate_all_operations() {
    let mut old: HashMap<String, Vec<u8>> = HashMap::new();
    old.insert("keep.txt".into(), b"same-content".to_vec());
    old.insert("modify.txt".into(), b"old-content".to_vec());
    old.insert("remove.txt".into(), b"will-be-removed".to_vec());

    let mut new: HashMap<String, Vec<u8>> = HashMap::new();
    new.insert("keep.txt".into(), b"same-content".to_vec());
    new.insert("modify.txt".into(), b"new-content".to_vec());
    new.insert("add.txt".into(), b"brand-new".to_vec());

    let updates = calculate_delta(&old, &new);

    let ops: HashMap<String, FileOperation> = updates
        .iter()
        .map(|u| (u.path.clone(), u.operation))
        .collect();

    assert_eq!(ops.get("add.txt"), Some(&FileOperation::Add));
    assert_eq!(ops.get("modify.txt"), Some(&FileOperation::Modify));
    assert_eq!(ops.get("remove.txt"), Some(&FileOperation::Remove));
    // "keep.txt" unchanged — should not appear
    assert!(!ops.contains_key("keep.txt"));
}

/// 7.2 Identical files produce no delta.
#[test]
fn delta_identical_files_no_updates() {
    let mut old: HashMap<String, Vec<u8>> = HashMap::new();
    old.insert("a.bin".into(), b"same".to_vec());
    let new = old.clone();
    let updates = calculate_delta(&old, &new);
    assert!(updates.is_empty());
}

/// 7.3 DeltaPatch::calculate_size counts only Insert data and new files.
#[test]
fn delta_patch_calculate_size_correct() {
    let mut patch = DeltaPatch::new("1.0.0".into(), "1.1.0".into());
    let delta = FileDelta {
        source_path: "f.bin".into(),
        target_path: "f.bin".into(),
        source_hash: "s".into(),
        target_hash: "t".into(),
        operations: vec![
            DeltaOperation::Copy { src_offset: 0, length: 500 },
            DeltaOperation::Insert { data: vec![0; 100] },
            DeltaOperation::Delete { length: 50 },
        ],
        compression: "none".into(),
    };
    patch.add_file_delta(delta);
    patch.add_new_file("new.bin".into(), vec![0; 200]);
    patch.calculate_size();
    assert_eq!(patch.patch_size, 300); // 100 insert + 200 new file
}

/// 7.4 Compress → decompress round-trip preserves content.
#[test]
fn delta_compression_roundtrip() {
    let original = b"Flight Hub update delta payload for compression test";
    let compressed = DeltaApplier::compress_patch_data(original).unwrap();
    assert_ne!(compressed, original.to_vec(), "compressed should differ from original");
    let decompressed = DeltaApplier::decompress_patch_data(&compressed).unwrap();
    assert_eq!(decompressed, original);
}

/// 7.5 Compress empty data round-trips.
#[test]
fn delta_compression_empty_data() {
    let compressed = DeltaApplier::compress_patch_data(b"").unwrap();
    let decompressed = DeltaApplier::decompress_patch_data(&compressed).unwrap();
    assert!(decompressed.is_empty());
}

/// 7.6 DeltaPatch::new initializes with correct defaults.
#[test]
fn delta_patch_new_defaults() {
    let patch = DeltaPatch::new("1.0.0".into(), "2.0.0".into());
    assert_eq!(patch.version, 1);
    assert_eq!(patch.source_version, "1.0.0");
    assert_eq!(patch.target_version, "2.0.0");
    assert!(patch.files.is_empty());
    assert!(patch.deleted_files.is_empty());
    assert!(patch.new_files.is_empty());
    assert_eq!(patch.patch_size, 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. ERROR HANDLING (4 tests)
// ═══════════════════════════════════════════════════════════════════════════

/// 8.1 UpdateError::InvalidSignature formats correctly.
#[test]
fn error_invalid_signature_display() {
    let err = flight_updater::UpdateError::InvalidSignature("bad key".into());
    let msg = err.to_string();
    assert!(msg.contains("bad key"));
}

/// 8.2 UpdateError::ChannelNotFound preserves channel name.
#[test]
fn error_channel_not_found_display() {
    let err = flight_updater::UpdateError::ChannelNotFound("nightly".into());
    assert!(err.to_string().contains("nightly"));
}

/// 8.3 UpdateError::DeltaPatch formats correctly.
#[test]
fn error_delta_patch_display() {
    let err = flight_updater::UpdateError::DeltaPatch("hash mismatch".into());
    assert!(err.to_string().contains("hash mismatch"));
}

/// 8.4 UpdateError::Rollback formats correctly.
#[test]
fn error_rollback_display() {
    let err = flight_updater::UpdateError::Rollback("no backup".into());
    assert!(err.to_string().contains("no backup"));
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. PROPERTY-BASED TESTS (proptest) (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// 9.1 SemVer round-trip through Display + parse is identity.
        #[test]
        fn semver_display_parse_roundtrip(
            major in 0u32..1000,
            minor in 0u32..1000,
            patch in 0u32..1000,
        ) {
            let v = SemVer::new(major, minor, patch);
            let s = v.to_string();
            let parsed = SemVer::parse(&s).unwrap();
            prop_assert_eq!(v, parsed);
        }

        /// 9.2 SemVer ordering is consistent: a < b ⟹ ¬(b < a).
        #[test]
        fn semver_ordering_antisymmetric(
            a_major in 0u32..100, a_minor in 0u32..100, a_patch in 0u32..100,
            b_major in 0u32..100, b_minor in 0u32..100, b_patch in 0u32..100,
        ) {
            let a = SemVer::new(a_major, a_minor, a_patch);
            let b = SemVer::new(b_major, b_minor, b_patch);
            if a < b {
                prop_assert!(!(b < a));
            }
        }

        /// 9.3 SignedUpdateManifest JSON round-trip preserves all fields.
        #[test]
        fn manifest_json_roundtrip(
            major in 0u32..100, minor in 0u32..100, patch_v in 0u32..100,
        ) {
            let manifest = SignedUpdateManifest {
                version: SemVer::new(major, minor, patch_v),
                channel: Channel::Stable,
                files: vec![],
                signature: "test-sig".into(),
                min_version: None,
            };
            let json = serde_json::to_vec(&manifest).unwrap();
            let parsed = parse_manifest(&json).unwrap();
            prop_assert_eq!(parsed.version, manifest.version);
        }

        /// 9.4 Compression round-trip preserves arbitrary byte sequences.
        #[test]
        fn compression_roundtrip_arbitrary(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
            let compressed = DeltaApplier::compress_patch_data(&data).unwrap();
            let decompressed = DeltaApplier::decompress_patch_data(&compressed).unwrap();
            prop_assert_eq!(data, decompressed);
        }

        /// 9.5 Policy: version strictly greater always yields non-Skip for allowed channel.
        #[test]
        fn policy_newer_version_not_skipped_for_version_reason(
            installed_patch in 0u32..50,
            update_patch in 51u32..100,
        ) {
            let policy = ManifestUpdatePolicy {
                auto_apply: true,
                allowed_channels: vec![Channel::Stable],
                defer_while_sim_running: false,
                ..Default::default()
            };
            let state = CurrentState {
                installed_version: SemVer::new(1, 0, installed_patch),
                sim_running: false,
                update_channel: Channel::Stable,
                update_version: SemVer::new(1, 0, update_patch),
                update_min_version: None,
            };
            let decision = should_apply(&policy, &state);
            // Must be Apply (not Skip or Defer for version reasons)
            prop_assert_eq!(decision, UpdateDecision::Apply);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. ADDITIONAL INTEGRATION TESTS (bonus for 60+ count)
// ═══════════════════════════════════════════════════════════════════════════

/// ManifestUpdateManager finds latest update for correct channel.
#[test]
fn update_manager_finds_latest_for_channel() {
    let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
    let manifest = make_um_manifest(vec![
        make_version_entry("1.1.0", UpdateChannel::Stable),
        make_version_entry("1.5.0", UpdateChannel::Beta),
        make_version_entry("1.3.0", UpdateChannel::Stable),
    ]);
    mgr.load_manifest(manifest);

    let update = mgr.check_for_update().unwrap();
    assert_eq!(update.version, "1.3.0"); // latest Stable, not Beta 1.5.0
}

/// ManifestUpdateManager reports no update when no manifest loaded.
#[test]
fn update_manager_no_manifest_no_update() {
    let mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
    assert!(!mgr.is_update_available());
}

/// ManifestUpdateManager success_rate with mixed history.
#[test]
fn update_manager_success_rate_mixed() {
    let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
    mgr.record_update(UpdateRecord {
        from_version: "1.0.0".into(),
        to_version: "1.1.0".into(),
        timestamp: 100,
        success: true,
        was_delta: false,
    });
    mgr.record_update(UpdateRecord {
        from_version: "1.1.0".into(),
        to_version: "1.2.0".into(),
        timestamp: 200,
        success: false,
        was_delta: true,
    });
    mgr.record_update(UpdateRecord {
        from_version: "1.1.0".into(),
        to_version: "1.2.0".into(),
        timestamp: 300,
        success: true,
        was_delta: false,
    });
    let rate = mgr.success_rate();
    assert!((rate - 2.0 / 3.0).abs() < f64::EPSILON);
}

/// UpdateResult serde round-trip preserves rollback flag.
#[test]
fn update_result_rollback_flag_roundtrip() {
    let result = UpdateResult {
        updated: true,
        previous_version: Some("1.0.0".into()),
        new_version: Some("0.9.0".into()),
        rollback_occurred: true,
        channel: Channel::Stable,
        update_size: 0,
        duration_seconds: 2,
    };
    let json = serde_json::to_string(&result).unwrap();
    let parsed: UpdateResult = serde_json::from_str(&json).unwrap();
    assert!(parsed.rollback_occurred);
}

/// SmUpdateManifest is_applicable with edge cases.
#[test]
fn sm_manifest_is_applicable_edge_cases() {
    let m = sample_sm_manifest(); // version 2.0.0, min_version 1.0.0
    assert!(m.is_applicable("1.0.0"), "at min version");
    assert!(m.is_applicable("1.9.9"), "between min and target");
    assert!(!m.is_applicable("0.9.9"), "below min");
    assert!(!m.is_applicable("2.0.0"), "at target");
    assert!(!m.is_applicable("2.0.1"), "above target");
}

/// SmUpdatePolicy allows_channel filtering.
#[test]
fn sm_policy_allows_channel() {
    let policy = SmUpdatePolicy {
        auto_update: true,
        allowed_channels: vec![Channel::Stable, Channel::Beta],
        rollback_on_failure: true,
        max_download_retries: 3,
    };
    assert!(policy.allows_channel(Channel::Stable));
    assert!(policy.allows_channel(Channel::Beta));
    assert!(!policy.allows_channel(Channel::Canary));
}

/// SmUpdatePolicy default values.
#[test]
fn sm_policy_default_values() {
    let policy = SmUpdatePolicy::default();
    assert!(!policy.auto_update);
    assert!(policy.rollback_on_failure);
    assert_eq!(policy.max_download_retries, 3);
    assert_eq!(policy.allowed_channels, vec![Channel::Stable]);
}

/// Journal clear empties entries.
#[test]
fn journal_clear_works() {
    let fs = MockFs::new_mock();
    let journal = UpdateJournal::new(PathBuf::from("/j.log"), fs);
    journal.record(&UpdateState::Downloading, "1.0.0", "dl").unwrap();
    assert_eq!(journal.entries().unwrap().len(), 1);
    journal.clear().unwrap();
    assert!(journal.entries().unwrap().is_empty());
}

/// VersionInfo::is_newer_than is strictly greater-than by timestamp.
#[test]
fn version_info_newer_than_strict() {
    let v1 = VersionInfo {
        version: "1.0.0".into(),
        build_timestamp: 1000,
        commit_hash: "aaa".into(),
        channel: Channel::Stable,
        install_timestamp: 1000,
        install_path: PathBuf::from("/tmp"),
        backup_path: None,
    };
    let v2 = VersionInfo {
        build_timestamp: 2000,
        ..v1.clone()
    };
    assert!(v2.is_newer_than(&v1));
    assert!(!v1.is_newer_than(&v2));
    assert!(!v1.is_newer_than(&v1));
}

/// ManifestUpdateManager can_delta_update when delta_url and min_version match.
#[test]
fn update_manager_can_delta_update() {
    let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
    let mut entry = make_version_entry("1.1.0", UpdateChannel::Stable);
    entry.min_version = Some("1.0.0".into());
    entry.delta_url = Some("https://example.com/delta".into());
    mgr.load_manifest(make_um_manifest(vec![entry]));
    assert!(mgr.can_delta_update());
}

/// ManifestUpdateManager can_delta_update false when min_version doesn't match.
#[test]
fn update_manager_cannot_delta_wrong_min_version() {
    let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
    let mut entry = make_version_entry("1.1.0", UpdateChannel::Stable);
    entry.min_version = Some("0.9.0".into()); // doesn't match current 1.0.0
    entry.delta_url = Some("https://example.com/delta".into());
    mgr.load_manifest(make_um_manifest(vec![entry]));
    assert!(!mgr.can_delta_update());
}

/// ManifestUpdateManager verify_integrity with correct SHA256.
#[test]
fn update_manager_verify_integrity_correct() {
    let data = b"test-update-content";
    let hash = sha256_hex(data);
    let mut entry = make_version_entry("1.0.0", UpdateChannel::Stable);
    entry.sha256 = hash;
    assert!(ManifestUpdateManager::verify_integrity(&entry, data));
}

/// ManifestUpdateManager verify_integrity with wrong SHA256.
#[test]
fn update_manager_verify_integrity_wrong_hash() {
    let mut entry = make_version_entry("1.0.0", UpdateChannel::Stable);
    entry.sha256 = "0".repeat(64);
    assert!(!ManifestUpdateManager::verify_integrity(&entry, b"anything"));
}

/// ArtifactEntry JSON round-trip.
#[test]
fn sm_artifact_entry_roundtrip() {
    let a = sample_sm_artifact();
    let json = serde_json::to_string(&a).unwrap();
    let parsed: SmArtifactEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(a, parsed);
}

/// canonical_bytes excludes signature field.
#[test]
fn canonical_bytes_excludes_signature() {
    let (manifest, _sk) = make_signed_manifest_with_key();
    let bytes1 = manifest.canonical_bytes();
    let mut m2 = manifest.clone();
    m2.signature = "completely-different-signature".into();
    assert_eq!(bytes1, m2.canonical_bytes());
}

/// FileOperation serde round-trip for all variants.
#[test]
fn file_operation_serde_all_variants() {
    for op in [FileOperation::Add, FileOperation::Modify, FileOperation::Remove] {
        let json = serde_json::to_string(&op).unwrap();
        let parsed: FileOperation = serde_json::from_str(&json).unwrap();
        assert_eq!(op, parsed);
    }
}
