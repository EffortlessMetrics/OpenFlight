// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the flight-updater crate.
//!
//! Groups:
//!   1. Manifest parsing & validation
//!   2. SemVer & version comparison
//!   3. Signature verification
//!   4. Channel system
//!   5. Policy engine
//!   6. Rollback & state machine
//!   7. Delta & compression
//!   8. Error handling & Misc
//!   9. Progress tracking
//!   10. Property-based tests (proptest)
//!   11. Integration Scenarios

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use ed25519_dalek::{Signer, SigningKey};
use proptest::prelude::*;
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
    UpdateState, VersionInfo, RealFileSystem,
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

fn sign_manifest(manifest: &mut SignedUpdateManifest, signing_key: &SigningKey) {
    let msg = manifest.canonical_bytes();
    let sig = signing_key.sign(&msg);
    manifest.signature = hex::encode(sig.to_bytes());
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
            if let Ok(rel) = key.strip_prefix(&pb) {
                if let Some(first) = rel.components().next() {
                    entries.insert(pb.join(first));
                }
            }
        }
        for dir in self.dirs.borrow().iter() {
            if let Ok(rel) = dir.strip_prefix(&pb) {
                if !rel.as_os_str().is_empty() {
                    if let Some(first) = rel.components().next() {
                        entries.insert(pb.join(first));
                    }
                }
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
// 1. MANIFEST PARSING & VALIDATION
// ═══════════════════════════════════════════════════════════════════════════

mod manifest_parsing {
    use super::*;

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

    #[test]
    fn manifest_parse_empty_bytes_error() {
        assert!(parse_manifest(b"").is_err());
    }

    #[test]
    fn manifest_parse_malformed_json_error() {
        assert!(parse_manifest(b"{{{{not json").is_err());
    }

    #[test]
    fn manifest_parse_missing_fields_error() {
        let incomplete = r#"{"version":{"major":1,"minor":0,"patch":0}}"#;
        assert!(parse_manifest(incomplete.as_bytes()).is_err());
    }

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

    #[test]
    fn sm_manifest_validate_invalid_version() {
        let mut m = sample_sm_manifest();
        m.version = "not-a-version".into();
        let errors = m.validate();
        assert!(errors.iter().any(|e| matches!(e, ManifestValidationError::InvalidSemver(_))));
    }

    #[test]
    fn sm_manifest_validate_empty_artifacts() {
        let mut m = sample_sm_manifest();
        m.artifacts.clear();
        let errors = m.validate();
        assert!(errors.iter().any(|e| matches!(e, ManifestValidationError::EmptyArtifacts)));
    }

    #[test]
    fn sm_manifest_validate_bad_artifact_sha256() {
        let mut m = sample_sm_manifest();
        m.artifacts[0].sha256 = "too-short".into();
        let errors = m.validate();
        assert!(errors.iter().any(|e| matches!(e, ManifestValidationError::InvalidSha256 { .. })));
    }

    #[test]
    fn future_version_format_graceful_degradation() {
        let (manifest, sk) = make_signed_manifest_with_key();
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());

        let mut json_value: serde_json::Value = serde_json::to_value(&manifest).unwrap();
        json_value["new_future_field"] = serde_json::json!("some_value");
        json_value["metadata"] = serde_json::json!({"build": 42});

        let json_bytes = serde_json::to_vec(&json_value).unwrap();
        let parsed = parse_manifest(&json_bytes).unwrap();
        assert!(verify_manifest_signature(&parsed, &pk_hex).is_ok());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. SEMVER & VERSION COMPARISON
// ═══════════════════════════════════════════════════════════════════════════

mod semver_tests {
    use super::*;

    #[test]
    fn semver_parse_valid_triple() {
        let v = SemVer::parse("10.20.30").unwrap();
        assert_eq!(v, SemVer::new(10, 20, 30));
    }

    #[test]
    fn semver_ordering_major_precedence() {
        assert!(SemVer::new(2, 0, 0) > SemVer::new(1, 99, 99));
    }

    #[test]
    fn semver_display_formatting() {
        assert_eq!(SemVer::new(100, 200, 300).to_string(), "100.200.300");
    }

    #[test]
    fn parse_semver_consistency() {
        assert_eq!(parse_semver("1.2.3"), Some((1, 2, 3)));
        assert!(parse_semver("abc").is_none());
    }

    #[test]
    fn compare_versions_full_ordering() {
        use std::cmp::Ordering;
        assert_eq!(compare_versions("1.0.0", "1.0.1"), Ordering::Less);
        assert_eq!(compare_versions("2.0.0", "1.9.9"), Ordering::Greater);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. SIGNATURE VERIFICATION
// ═══════════════════════════════════════════════════════════════════════════

mod signature_verification {
    use super::*;

    #[test]
    fn sig_valid_keypair_verifies() {
        let (manifest, sk) = make_signed_manifest_with_key();
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());
        assert!(verify_manifest_signature(&manifest, &pk_hex).is_ok());
    }

    #[test]
    fn sig_wrong_key_rejects() {
        let (manifest, _sk) = make_signed_manifest_with_key();
        let wrong_sk = SigningKey::generate(&mut OsRng);
        let wrong_pk_hex = hex::encode(wrong_sk.verifying_key().to_bytes());
        assert!(verify_manifest_signature(&manifest, &wrong_pk_hex).is_err());
    }

    #[test]
    fn tampered_version_is_rejected() {
        let (mut manifest, sk) = make_signed_manifest_with_key();
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());
        manifest.version = SemVer::new(9, 9, 9);
        assert!(verify_manifest_signature(&manifest, &pk_hex).is_err());
    }

    #[test]
    fn canonical_bytes_excludes_signature() {
        let (manifest, _sk) = make_signed_manifest_with_key();
        let bytes1 = manifest.canonical_bytes();
        let mut m2 = manifest.clone();
        m2.signature = "completely-different-signature".into();
        assert_eq!(bytes1, m2.canonical_bytes());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. CHANNEL SYSTEM
// ═══════════════════════════════════════════════════════════════════════════

mod channel_system {
    use super::*;

    #[test]
    fn channel_manager_has_three_channels() {
        let mgr = ChannelManager::new();
        let channels = mgr.available_channels();
        assert!(channels.contains(&Channel::Stable));
        assert!(channels.contains(&Channel::Beta));
        assert!(channels.contains(&Channel::Canary));
    }

    #[test]
    fn channel_switching_works() {
        let mut mgr = ChannelManager::new();
        mgr.switch_channel(Channel::Beta).unwrap();
        assert_eq!(mgr.current_channel(), Channel::Beta);
    }

    #[test]
    fn channel_from_str_case_insensitive() {
        assert_eq!("STABLE".parse::<Channel>().unwrap(), Channel::Stable);
    }

    #[test]
    fn channel_check_frequency_priority() {
        let mgr = ChannelManager::new();
        let stable = mgr.get_config(Channel::Stable).unwrap();
        let canary = mgr.get_config(Channel::Canary).unwrap();
        assert!(canary.check_frequency_hours < stable.check_frequency_hours);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. POLICY ENGINE
// ═══════════════════════════════════════════════════════════════════════════

mod policy_engine {
    use super::*;

    fn auto_policy() -> ManifestUpdatePolicy {
        ManifestUpdatePolicy { auto_apply: true, ..Default::default() }
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

    #[test]
    fn policy_apply_when_all_conditions_met() {
        assert_eq!(should_apply(&auto_policy(), &default_state()), UpdateDecision::Apply);
    }

    #[test]
    fn policy_skip_disallowed_channel() {
        let state = CurrentState { update_channel: Channel::Canary, ..default_state() };
        assert!(matches!(should_apply(&auto_policy(), &state), UpdateDecision::Skip(_)));
    }

    #[test]
    fn mid_flight_protection_defers_update() {
        let state = CurrentState { sim_running: true, ..default_state() };
        let decision = should_apply(&auto_policy(), &state);
        assert!(matches!(decision, UpdateDecision::Defer(ref r) if r.contains("simulator")));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. ROLLBACK & STATE MACHINE
// ═══════════════════════════════════════════════════════════════════════════

mod rollback_system {
    use super::*;

    #[test]
    fn rollback_initial_state_idle() {
        let fs = MockFs::new_mock();
        let config = mock_config("/app");
        let mgr = UpdateRollbackManager::new(config, fs).unwrap();
        assert_eq!(*mgr.state(), UpdateState::Idle);
    }

    #[test]
    fn failed_update_sets_failed_state() {
        let fs = MockFs::new_mock();
        let config = mock_config("/app");
        let mut mgr = UpdateRollbackManager::new(config, fs).unwrap();
        let artifacts = vec![ArtifactFile {
            path: PathBuf::from("/tmp/bad.bin"),
            expected_sha256: "wrong".into(),
        }];
        let _ = mgr.apply_update(&artifacts, "2.0.0");
        assert_eq!(*mgr.state(), UpdateState::Failed);
    }

    #[test]
    fn rollback_journal_state_ordering() {
        let fs = MockFs::new_mock();
        let journal = UpdateJournal::new(PathBuf::from("/j.log"), fs);
        journal.record(&UpdateState::Downloading, "1.0.0", "start").unwrap();
        journal.record(&UpdateState::Complete, "1.0.0", "done").unwrap();
        let entries = journal.entries().unwrap();
        assert_eq!(entries[0].state, UpdateState::Downloading);
        assert_eq!(entries[1].state, UpdateState::Complete);
    }

    #[test]
    fn rollback_recover_on_startup_with_backup() {
        let fs = MockFs::new_mock();
        fs.add_file("/app/install/bin.exe", b"broken-v2");
        fs.add_file("/app/backups/backup_001/bin.exe", b"good-v1");
        let config = mock_config("/app");
        let mut mgr = UpdateRollbackManager::new(config, fs).unwrap();
        mgr.journal().record(&UpdateState::Installing, "2.0.0", "stuck").unwrap();
        let recovered = mgr.recover_on_startup().unwrap();
        assert!(recovered);
        assert_eq!(*mgr.state(), UpdateState::Idle);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. DELTA & COMPRESSION
// ═══════════════════════════════════════════════════════════════════════════

mod delta_compression {
    use super::*;

    #[test]
    fn delta_calculate_all_operations() {
        let mut old = HashMap::new();
        old.insert("modify.txt".to_string(), b"old".to_vec());
        let mut new = HashMap::new();
        new.insert("modify.txt".to_string(), b"new".to_vec());
        let updates = calculate_delta(&old, &new);
        assert_eq!(updates[0].operation, FileOperation::Modify);
    }

    #[test]
    fn delta_compression_roundtrip() {
        let original = b"Flight Hub update delta payload";
        let compressed = DeltaApplier::compress_patch_data(original).unwrap();
        let decompressed = DeltaApplier::decompress_patch_data(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn delta_patch_calculate_size_correct() {
        let mut patch = DeltaPatch::new("1.0.0".into(), "1.1.0".into());
        patch.add_new_file("new.bin".into(), vec![0; 200]);
        patch.calculate_size();
        assert_eq!(patch.patch_size, 200);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. ERROR HANDLING & MISC
// ═══════════════════════════════════════════════════════════════════════════

mod error_handling {
    use super::*;

    #[test]
    fn error_invalid_signature_display() {
        let err = flight_updater::UpdateError::InvalidSignature("bad key".into());
        assert!(err.to_string().contains("bad key"));
    }

    #[test]
    fn update_result_roundtrip() {
        let result = UpdateResult {
            updated: true,
            previous_version: Some("1.0.0".into()),
            new_version: Some("2.0.0".into()),
            rollback_occurred: false,
            channel: Channel::Stable,
            update_size: 50_000,
            duration_seconds: 15,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: UpdateResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.new_version, result.new_version);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. PROGRESS TRACKING
// ═══════════════════════════════════════════════════════════════════════════

mod progress_tracking {
    use super::*;

    #[test]
    fn update_history_tracks_success_rate() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
        mgr.record_update(UpdateRecord {
            from_version: "1.0.0".into(), to_version: "1.1.0".into(),
            timestamp: 1000, success: true, was_delta: false,
        });
        mgr.record_update(UpdateRecord {
            from_version: "1.1.0".into(), to_version: "1.2.0".into(),
            timestamp: 2000, success: false, was_delta: false,
        });
        let rate = mgr.success_rate();
        assert!((rate - 0.5).abs() < 1e-10);
    }

    #[test]
    fn journal_clear_works() {
        let fs = MockFs::new_mock();
        let journal = UpdateJournal::new(PathBuf::from("/j.log"), fs);
        journal.record(&UpdateState::Downloading, "1.0.0", "dl").unwrap();
        journal.clear().unwrap();
        assert!(journal.entries().unwrap().is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. PROPERTY-BASED TESTS (proptest)
// ═══════════════════════════════════════════════════════════════════════════

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn semver_display_parse_roundtrip(
            major in 0u32..1000, minor in 0u32..1000, patch in 0u32..1000,
        ) {
            let v = SemVer::new(major, minor, patch);
            let s = v.to_string();
            let parsed = SemVer::parse(&s).unwrap();
            prop_assert_eq!(v, parsed);
        }

        #[test]
        fn compression_roundtrip_arbitrary(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
            let compressed = DeltaApplier::compress_patch_data(&data).unwrap();
            let decompressed = DeltaApplier::decompress_patch_data(&compressed).unwrap();
            prop_assert_eq!(data, decompressed);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. INTEGRATION SCENARIOS
// ═══════════════════════════════════════════════════════════════════════════

mod integration_scenarios {
    use super::*;

    #[test]
    fn full_update_lifecycle_real_fs() {
        let temp = tempfile::tempdir().unwrap();
        let install_dir = temp.path().join("install");
        let backup_dir = temp.path().join("backups");
        let journal_path = temp.path().join("journal.log");

        std::fs::create_dir_all(&install_dir).unwrap();
        std::fs::write(install_dir.join("app.bin"), b"v1").unwrap();

        let config = UpdateRollbackConfig {
            backup_dir, install_dir: install_dir.clone(),
            journal_path, max_backups: 3,
        };

        let fs = RealFileSystem;
        let mut mgr = UpdateRollbackManager::new(config, fs).unwrap();

        let artifact_data = b"v2";
        let artifact_path = temp.path().join("v2.bin");
        std::fs::write(&artifact_path, artifact_data).unwrap();

        let artifacts = vec![ArtifactFile {
            path: artifact_path, expected_sha256: sha256_hex(artifact_data),
        }];

        mgr.apply_update(&artifacts, "2.0.0").unwrap();
        assert_eq!(*mgr.state(), UpdateState::Complete);
    }

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
        assert_eq!(update.version, "1.3.0");
    }
}
