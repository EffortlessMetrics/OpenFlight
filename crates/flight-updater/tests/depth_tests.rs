// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the update system covering manifest verification, delta
//! updates, rollback, channels, progress tracking, and integration scenarios.

use std::collections::HashMap;

use ed25519_dalek::{Signer, SigningKey};
use proptest::prelude::*;
use rand_core::OsRng;
use sha2::{Digest, Sha256};

use flight_updater::channels::{Channel, ChannelManager};
use flight_updater::delta::{
    DeltaApplier, DeltaOperation, DeltaPatch, FileDelta, calculate_delta,
};
use flight_updater::manifest::{
    FileOperation, FileUpdate, SemVer, UpdateManifest as SignedUpdateManifest,
    parse as parse_manifest, verify_signature as verify_manifest_signature,
};
use flight_updater::policy::{
    CurrentState, UpdateDecision, UpdatePolicy as ManifestUpdatePolicy, should_apply,
};
use flight_updater::rollback::{
    ArtifactFile, RealFileSystem, UpdateJournal, UpdateRollbackConfig,
    UpdateRollbackManager, UpdateState,
};
use flight_updater::update_manifest::{
    ManifestUpdateManager, UpdateChannel, UpdateManifest, UpdateRecord, VersionEntry,
};
use flight_updater::updater::{UpdateConfig, UpdateResult};

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn unsigned_manifest() -> SignedUpdateManifest {
    SignedUpdateManifest {
        version: SemVer::new(2, 0, 0),
        channel: Channel::Stable,
        files: vec![
            FileUpdate {
                path: "bin/app.exe".into(),
                hash_before: String::new(),
                hash_after: "bb".repeat(32),
                size: 2048,
                operation: FileOperation::Add,
            },
            FileUpdate {
                path: "lib/core.dll".into(),
                hash_before: "aa".repeat(32),
                hash_after: "cc".repeat(32),
                size: 4096,
                operation: FileOperation::Modify,
            },
        ],
        signature: String::new(),
        min_version: Some(SemVer::new(1, 0, 0)),
    }
}

fn sign_manifest(manifest: &mut SignedUpdateManifest, signing_key: &SigningKey) {
    let msg = manifest.canonical_bytes();
    let sig = signing_key.sign(&msg);
    manifest.signature = hex::encode(sig.to_bytes());
}

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

fn make_update_manifest(entries: Vec<VersionEntry>) -> UpdateManifest {
    UpdateManifest {
        schema_version: 1,
        entries,
        manifest_sha256: String::new(),
        manifest_signature: None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Manifest Verification
// ═══════════════════════════════════════════════════════════════════════════

mod manifest_verification {
    use super::*;

    /// Valid manifest with correct checksums → accepted.
    #[test]
    fn valid_manifest_with_correct_signature_is_accepted() {
        let sk = SigningKey::generate(&mut OsRng);
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());

        let mut manifest = unsigned_manifest();
        sign_manifest(&mut manifest, &sk);

        assert!(
            verify_manifest_signature(&manifest, &pk_hex).is_ok(),
            "correctly signed manifest must pass verification"
        );
    }

    /// Tampered manifest (version changed) → rejected.
    #[test]
    fn tampered_version_is_rejected() {
        let sk = SigningKey::generate(&mut OsRng);
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());

        let mut manifest = unsigned_manifest();
        sign_manifest(&mut manifest, &sk);

        manifest.version = SemVer::new(9, 9, 9);
        assert!(
            verify_manifest_signature(&manifest, &pk_hex).is_err(),
            "tampered version must fail verification"
        );
    }

    /// Tampered manifest (files changed) → rejected.
    #[test]
    fn tampered_files_are_rejected() {
        let sk = SigningKey::generate(&mut OsRng);
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());

        let mut manifest = unsigned_manifest();
        sign_manifest(&mut manifest, &sk);

        manifest.files[0].size = 999_999;
        assert!(
            verify_manifest_signature(&manifest, &pk_hex).is_err(),
            "tampered file size must fail verification"
        );
    }

    /// Tampered manifest (channel changed) → rejected.
    #[test]
    fn tampered_channel_is_rejected() {
        let sk = SigningKey::generate(&mut OsRng);
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());

        let mut manifest = unsigned_manifest();
        sign_manifest(&mut manifest, &sk);

        manifest.channel = Channel::Canary;
        assert!(
            verify_manifest_signature(&manifest, &pk_hex).is_err(),
            "tampered channel must fail verification"
        );
    }

    /// Missing required fields → rejected during parsing.
    #[test]
    fn missing_required_fields_rejected_on_parse() {
        // JSON missing the `version` field entirely
        let json = r#"{"channel":"Stable","files":[],"signature":"","min_version":null}"#;
        assert!(
            parse_manifest(json.as_bytes()).is_err(),
            "manifest missing 'version' must fail parsing"
        );
    }

    /// Missing signature field → rejected on parse.
    #[test]
    fn missing_signature_field_rejected_on_parse() {
        let json = r#"{"version":{"major":1,"minor":0,"patch":0},"channel":"Stable","files":[],"min_version":null}"#;
        assert!(
            parse_manifest(json.as_bytes()).is_err(),
            "manifest missing 'signature' must fail parsing"
        );
    }

    /// Future version format (unknown fields) → graceful degradation (parsing
    /// still succeeds because serde ignores unknown fields by default in
    /// deserialization, but the signature over the canonical bytes still covers
    /// the known fields).
    #[test]
    fn future_version_format_graceful_degradation() {
        let sk = SigningKey::generate(&mut OsRng);
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());

        let mut manifest = unsigned_manifest();
        sign_manifest(&mut manifest, &sk);

        // Serialize, inject an unknown field, and re-parse
        let mut json_value: serde_json::Value =
            serde_json::to_value(&manifest).unwrap();
        json_value["new_future_field"] = serde_json::json!("some_value");
        json_value["metadata"] = serde_json::json!({"build": 42});

        let json_bytes = serde_json::to_vec(&json_value).unwrap();
        let parsed = parse_manifest(&json_bytes);
        assert!(
            parsed.is_ok(),
            "unknown fields should be silently ignored during parsing"
        );

        let parsed = parsed.unwrap();
        assert!(
            verify_manifest_signature(&parsed, &pk_hex).is_ok(),
            "signature must still verify after stripping unknown fields"
        );
    }

    /// Empty signature field causes verification to fail gracefully.
    #[test]
    fn empty_signature_field_fails_verification() {
        let sk = SigningKey::generate(&mut OsRng);
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());

        let manifest = unsigned_manifest(); // signature = ""
        assert!(
            verify_manifest_signature(&manifest, &pk_hex).is_err(),
            "empty signature must fail verification"
        );
    }

    /// Signature from wrong key is rejected.
    #[test]
    fn wrong_key_signature_rejected() {
        let sk = SigningKey::generate(&mut OsRng);
        let wrong_sk = SigningKey::generate(&mut OsRng);
        let wrong_pk_hex = hex::encode(wrong_sk.verifying_key().to_bytes());

        let mut manifest = unsigned_manifest();
        sign_manifest(&mut manifest, &sk);

        assert!(
            verify_manifest_signature(&manifest, &wrong_pk_hex).is_err(),
            "signature from wrong key must fail"
        );
    }

    /// Canonical bytes are deterministic regardless of signature field content.
    #[test]
    fn canonical_bytes_exclude_signature_field() {
        let mut m1 = unsigned_manifest();
        m1.signature = "aaa".into();
        let mut m2 = unsigned_manifest();
        m2.signature = "zzz".into();
        assert_eq!(
            m1.canonical_bytes(),
            m2.canonical_bytes(),
            "canonical bytes must be identical regardless of signature content"
        );
    }
}

// Property test: signed manifest always verifies with the correct key.
proptest! {
    #[test]
    fn prop_signed_manifest_always_verifies_with_correct_key(
        major in 0u32..100,
        minor in 0u32..100,
        patch in 0u32..100,
    ) {
        let sk = SigningKey::generate(&mut OsRng);
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());

        let mut manifest = unsigned_manifest();
        manifest.version = SemVer::new(major, minor, patch);
        sign_manifest(&mut manifest, &sk);

        prop_assert!(
            verify_manifest_signature(&manifest, &pk_hex).is_ok(),
            "signed manifest with version {}.{}.{} must always verify",
            major, minor, patch
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Update Channels
// ═══════════════════════════════════════════════════════════════════════════

mod update_channels {
    use super::*;

    /// All three channels exist.
    #[test]
    fn stable_beta_canary_channels_exist() {
        let mgr = ChannelManager::new();
        let channels = mgr.available_channels();
        assert!(channels.contains(&Channel::Stable));
        assert!(channels.contains(&Channel::Beta));
        assert!(channels.contains(&Channel::Canary));
    }

    /// Channel switching works round-trip.
    #[test]
    fn channel_switching_works() {
        let mut mgr = ChannelManager::new();
        assert_eq!(mgr.current_channel(), Channel::Stable);

        mgr.switch_channel(Channel::Beta).unwrap();
        assert_eq!(mgr.current_channel(), Channel::Beta);

        mgr.switch_channel(Channel::Canary).unwrap();
        assert_eq!(mgr.current_channel(), Channel::Canary);

        mgr.switch_channel(Channel::Stable).unwrap();
        assert_eq!(mgr.current_channel(), Channel::Stable);
    }

    /// Can't downgrade from stable to canary accidentally — the policy engine
    /// blocks it by default (only stable channel is allowed in default policy).
    #[test]
    fn default_policy_blocks_canary_when_on_stable() {
        let policy = ManifestUpdatePolicy::default();
        let state = CurrentState {
            installed_version: SemVer::new(1, 0, 0),
            sim_running: false,
            update_channel: Channel::Canary,
            update_version: SemVer::new(2, 0, 0),
            update_min_version: None,
        };

        let decision = should_apply(&policy, &state);
        assert!(
            matches!(decision, UpdateDecision::Skip(_)),
            "default policy must skip canary updates: got {decision:?}"
        );
    }

    /// Channel priority: canary checks more frequently than beta, beta more
    /// than stable.
    #[test]
    fn channel_check_frequency_priority() {
        let mgr = ChannelManager::new();
        let stable = mgr.get_config(Channel::Stable).unwrap();
        let beta = mgr.get_config(Channel::Beta).unwrap();
        let canary = mgr.get_config(Channel::Canary).unwrap();

        assert!(
            canary.check_frequency_hours < beta.check_frequency_hours,
            "canary ({}) must check more frequently than beta ({})",
            canary.check_frequency_hours,
            beta.check_frequency_hours
        );
        assert!(
            beta.check_frequency_hours < stable.check_frequency_hours,
            "beta ({}) must check more frequently than stable ({})",
            beta.check_frequency_hours,
            stable.check_frequency_hours
        );
    }

    /// Canary and beta accept prereleases; stable does not.
    #[test]
    fn prerelease_acceptance_per_channel() {
        let mgr = ChannelManager::new();

        assert!(
            !mgr.get_config(Channel::Stable)
                .unwrap()
                .accept_prerelease
        );
        assert!(mgr.get_config(Channel::Beta).unwrap().accept_prerelease);
        assert!(mgr.get_config(Channel::Canary).unwrap().accept_prerelease);
    }

    /// The update manifest manager only returns updates for the selected
    /// channel.
    #[test]
    fn manifest_manager_filters_by_channel() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
        mgr.load_manifest(make_update_manifest(vec![
            make_entry("2.0.0", UpdateChannel::Beta),
            make_entry("3.0.0", UpdateChannel::Canary),
        ]));

        assert!(
            mgr.check_for_update().is_none(),
            "stable channel must not see beta/canary updates"
        );
    }

    /// After switching channels, the correct update becomes visible.
    #[test]
    fn channel_switch_reveals_correct_update() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);
        mgr.load_manifest(make_update_manifest(vec![
            make_entry("1.5.0", UpdateChannel::Stable),
            make_entry("2.0.0", UpdateChannel::Beta),
        ]));

        assert_eq!(mgr.check_for_update().unwrap().version, "1.5.0");

        mgr.set_channel(UpdateChannel::Beta);
        assert_eq!(mgr.check_for_update().unwrap().version, "2.0.0");
    }

    /// Policy engine allows beta when explicitly configured.
    #[test]
    fn policy_allows_beta_when_configured() {
        let policy = ManifestUpdatePolicy {
            auto_apply: true,
            allowed_channels: vec![Channel::Stable, Channel::Beta],
            ..Default::default()
        };
        let state = CurrentState {
            installed_version: SemVer::new(1, 0, 0),
            sim_running: false,
            update_channel: Channel::Beta,
            update_version: SemVer::new(2, 0, 0),
            update_min_version: None,
        };
        assert_eq!(should_apply(&policy, &state), UpdateDecision::Apply);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Delta Updates
// ═══════════════════════════════════════════════════════════════════════════

mod delta_updates {
    use super::*;

    /// Delta patch using Copy + Insert operations is smaller than full content.
    #[test]
    fn delta_smaller_than_full_update() {
        let mut patch = DeltaPatch::new("1.0.0".into(), "1.1.0".into());

        // A file delta that copies 900 bytes and inserts 100 new bytes
        let delta = FileDelta {
            source_path: "app.bin".into(),
            target_path: "app.bin".into(),
            source_hash: "s".into(),
            target_hash: "t".into(),
            operations: vec![
                DeltaOperation::Copy {
                    src_offset: 0,
                    length: 900,
                },
                DeltaOperation::Insert {
                    data: vec![0xAB; 100],
                },
            ],
            compression: "none".into(),
        };
        patch.add_file_delta(delta);
        patch.calculate_size();

        // Full file would be 1000 bytes; delta only carries 100
        assert_eq!(patch.patch_size, 100);
        assert!(
            patch.patch_size < 1000,
            "delta (100) must be smaller than full file (1000)"
        );
    }

    /// Delta applies Copy and Insert correctly to produce expected output.
    #[tokio::test]
    async fn delta_applies_correctly() {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        let work_dir = temp.path().join("work");
        tokio::fs::create_dir_all(&source_dir).await.unwrap();
        tokio::fs::create_dir_all(&target_dir).await.unwrap();

        // Write source file
        let source_content = b"AAAA_original_content";
        tokio::fs::write(source_dir.join("app.bin"), source_content).await.unwrap();

        let source_hash = sha256_hex(source_content);
        let target_content_expected = b"AAAA_new_stuff";
        let target_hash = sha256_hex(target_content_expected);

        let patch = DeltaPatch {
            version: 1,
            source_version: "1.0.0".into(),
            target_version: "1.1.0".into(),
            files: {
                let mut m = HashMap::new();
                m.insert(
                    "app.bin".to_string(),
                    FileDelta {
                        source_path: "app.bin".into(),
                        target_path: "app.bin".into(),
                        source_hash,
                        target_hash,
                        operations: vec![
                            DeltaOperation::Copy {
                                src_offset: 0,
                                length: 4, // "AAAA"
                            },
                            DeltaOperation::Insert {
                                data: b"_new_stuff".to_vec(),
                            },
                        ],
                        compression: "none".into(),
                    },
                );
                m
            },
            deleted_files: vec![],
            new_files: HashMap::new(),
            created_at: 0,
            patch_size: 0,
        };

        let applier = DeltaApplier::new(&work_dir).unwrap();
        applier
            .apply_patch(&patch, &source_dir, &target_dir)
            .await
            .unwrap();

        let result = tokio::fs::read(target_dir.join("app.bin")).await.unwrap();
        assert_eq!(
            result, target_content_expected,
            "delta must produce expected output"
        );
    }

    /// Corrupted delta (bad target hash) → error when applying.
    #[tokio::test]
    async fn corrupted_delta_fails() {
        let temp = tempfile::tempdir().unwrap();
        let source_dir = temp.path().join("source");
        let target_dir = temp.path().join("target");
        let work_dir = temp.path().join("work");
        tokio::fs::create_dir_all(&source_dir).await.unwrap();
        tokio::fs::create_dir_all(&target_dir).await.unwrap();

        let source_content = b"original";
        tokio::fs::write(source_dir.join("f.bin"), source_content).await.unwrap();

        let source_hash = sha256_hex(source_content);

        let patch = DeltaPatch {
            version: 1,
            source_version: "1.0.0".into(),
            target_version: "1.1.0".into(),
            files: {
                let mut m = HashMap::new();
                m.insert(
                    "f.bin".to_string(),
                    FileDelta {
                        source_path: "f.bin".into(),
                        target_path: "f.bin".into(),
                        source_hash,
                        target_hash: "wrong_hash_value".into(), // deliberately wrong
                        operations: vec![DeltaOperation::Insert {
                            data: b"new_data".to_vec(),
                        }],
                        compression: "none".into(),
                    },
                );
                m
            },
            deleted_files: vec![],
            new_files: HashMap::new(),
            created_at: 0,
            patch_size: 0,
        };

        let applier = DeltaApplier::new(&work_dir).unwrap();
        let result = applier
            .apply_patch(&patch, &source_dir, &target_dir)
            .await;

        assert!(
            result.is_err(),
            "delta with wrong target hash must fail"
        );
    }

    /// Multiple sequential deltas compose correctly (chain 1.0→1.1→1.2).
    #[tokio::test]
    async fn sequential_deltas_compose() {
        let temp = tempfile::tempdir().unwrap();
        let work_dir = temp.path().join("work");

        // Phase 1: v1.0 source
        let dir_v1 = temp.path().join("v1");
        tokio::fs::create_dir_all(&dir_v1).await.unwrap();
        let content_v1 = b"version_one";
        tokio::fs::write(dir_v1.join("app.bin"), content_v1).await.unwrap();

        // Phase 2: apply delta v1.0 → v1.1
        let dir_v1_1 = temp.path().join("v1_1");
        tokio::fs::create_dir_all(&dir_v1_1).await.unwrap();
        let content_v1_1 = b"version_one_one";
        let hash_v1 = sha256_hex(content_v1);
        let hash_v1_1 = sha256_hex(content_v1_1);

        let patch1 = DeltaPatch {
            version: 1,
            source_version: "1.0.0".into(),
            target_version: "1.1.0".into(),
            files: {
                let mut m = HashMap::new();
                m.insert(
                    "app.bin".to_string(),
                    FileDelta {
                        source_path: "app.bin".into(),
                        target_path: "app.bin".into(),
                        source_hash: hash_v1,
                        target_hash: hash_v1_1.clone(),
                        operations: vec![DeltaOperation::Insert {
                            data: content_v1_1.to_vec(),
                        }],
                        compression: "none".into(),
                    },
                );
                m
            },
            deleted_files: vec![],
            new_files: HashMap::new(),
            created_at: 0,
            patch_size: 0,
        };

        let applier = DeltaApplier::new(&work_dir).unwrap();
        applier
            .apply_patch(&patch1, &dir_v1, &dir_v1_1)
            .await
            .unwrap();

        // Phase 3: apply delta v1.1 → v1.2
        let dir_v1_2 = temp.path().join("v1_2");
        tokio::fs::create_dir_all(&dir_v1_2).await.unwrap();
        let content_v1_2 = b"version_one_two";
        let hash_v1_2 = sha256_hex(content_v1_2);

        let patch2 = DeltaPatch {
            version: 1,
            source_version: "1.1.0".into(),
            target_version: "1.2.0".into(),
            files: {
                let mut m = HashMap::new();
                m.insert(
                    "app.bin".to_string(),
                    FileDelta {
                        source_path: "app.bin".into(),
                        target_path: "app.bin".into(),
                        source_hash: hash_v1_1,
                        target_hash: hash_v1_2,
                        operations: vec![DeltaOperation::Insert {
                            data: content_v1_2.to_vec(),
                        }],
                        compression: "none".into(),
                    },
                );
                m
            },
            deleted_files: vec![],
            new_files: HashMap::new(),
            created_at: 0,
            patch_size: 0,
        };

        applier
            .apply_patch(&patch2, &dir_v1_1, &dir_v1_2)
            .await
            .unwrap();

        let final_content = tokio::fs::read(dir_v1_2.join("app.bin")).await.unwrap();
        assert_eq!(
            final_content, content_v1_2,
            "chained deltas must produce the v1.2 content"
        );
    }

    /// `calculate_delta` detects additions, modifications, and removals.
    #[test]
    fn calculate_delta_detects_all_operations() {
        let mut old: HashMap<String, Vec<u8>> = HashMap::new();
        old.insert("keep.txt".into(), b"same".to_vec());
        old.insert("modify.txt".into(), b"old_data".to_vec());
        old.insert("remove.txt".into(), b"gone".to_vec());

        let mut new: HashMap<String, Vec<u8>> = HashMap::new();
        new.insert("keep.txt".into(), b"same".to_vec());
        new.insert("modify.txt".into(), b"new_data".to_vec());
        new.insert("added.txt".into(), b"brand_new".to_vec());

        let updates = calculate_delta(&old, &new);

        let ops: Vec<_> = updates.iter().map(|u| (&u.path, u.operation)).collect();
        assert!(
            ops.iter().any(|(p, op)| p.as_str() == "added.txt" && *op == FileOperation::Add),
            "must detect added file"
        );
        assert!(
            ops.iter()
                .any(|(p, op)| p.as_str() == "modify.txt" && *op == FileOperation::Modify),
            "must detect modified file"
        );
        assert!(
            ops.iter()
                .any(|(p, op)| p.as_str() == "remove.txt" && *op == FileOperation::Remove),
            "must detect removed file"
        );
        // "keep.txt" unchanged → should NOT appear
        assert!(
            !ops.iter().any(|(p, _)| p.as_str() == "keep.txt"),
            "unchanged file must not appear in delta"
        );
    }

    /// Compression/decompression round-trip preserves data.
    #[test]
    fn compression_roundtrip() {
        let data = b"Hello, Flight Hub updater! This is test data for compression.";
        let compressed = DeltaApplier::compress_patch_data(data).unwrap();
        let decompressed = DeltaApplier::decompress_patch_data(&compressed).unwrap();
        assert_eq!(decompressed, data, "round-trip must preserve data");
    }

    /// Compressed data is smaller than original for compressible input.
    #[test]
    fn compression_reduces_size() {
        let data = vec![0xAA; 10_000]; // highly compressible
        let compressed = DeltaApplier::compress_patch_data(&data).unwrap();
        assert!(
            compressed.len() < data.len(),
            "compressed ({}) must be smaller than original ({})",
            compressed.len(),
            data.len()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Rollback
// ═══════════════════════════════════════════════════════════════════════════

mod rollback {
    use super::*;

    /// Failed update with a bad artifact hash sets state to Failed.
    #[test]
    fn failed_update_sets_failed_state() {
        let temp = tempfile::tempdir().unwrap();
        let install_dir = temp.path().join("install");
        let backup_dir = temp.path().join("backups");
        let journal_path = temp.path().join("journal.log");

        // Set up initial installation
        std::fs::create_dir_all(&install_dir).unwrap();
        std::fs::write(install_dir.join("app.bin"), b"v1_data").unwrap();

        let config = UpdateRollbackConfig {
            backup_dir: backup_dir.clone(),
            install_dir: install_dir.clone(),
            journal_path,
            max_backups: 3,
        };

        let fs = RealFileSystem;
        let mut mgr = UpdateRollbackManager::new(config, fs).unwrap();

        // Apply with a bad artifact (wrong hash) — should fail
        let artifact_path = temp.path().join("artifact.bin");
        std::fs::write(&artifact_path, b"artifact_data").unwrap();

        let artifacts = vec![ArtifactFile {
            path: artifact_path,
            expected_sha256: "wrong_hash".to_string(),
        }];

        let result = mgr.apply_update(&artifacts, "2.0.0");
        assert!(result.is_err(), "update with bad hash must fail");

        // State must be Failed (verification failed before install)
        assert_eq!(
            *mgr.state(),
            UpdateState::Failed,
            "state must be Failed after bad artifact"
        );
    }

    /// Rollback preserves user config files.
    #[test]
    fn rollback_preserves_user_config() {
        let temp = tempfile::tempdir().unwrap();
        let install_dir = temp.path().join("install");
        let backup_dir = temp.path().join("backups");
        let journal_path = temp.path().join("journal.log");

        // Create install with a "config" file
        std::fs::create_dir_all(&install_dir).unwrap();
        std::fs::write(install_dir.join("app.bin"), b"app_v1").unwrap();
        std::fs::write(install_dir.join("user.cfg"), b"user_prefs").unwrap();

        let config = UpdateRollbackConfig {
            backup_dir: backup_dir.clone(),
            install_dir: install_dir.clone(),
            journal_path,
            max_backups: 3,
        };

        let fs = RealFileSystem;
        let mgr = UpdateRollbackManager::new(config, fs).unwrap();

        // Create backup
        let backup_path = mgr.backup_current_version().unwrap();

        // Verify the backup contains user config
        let backed_up =
            std::fs::read(backup_path.join("user.cfg")).unwrap();
        assert_eq!(
            backed_up, b"user_prefs",
            "backup must contain the user config file"
        );

        // Simulate overwrite, then restore
        std::fs::write(install_dir.join("user.cfg"), b"corrupted").unwrap();
        mgr.restore_backup(&backup_path).unwrap();

        let restored = std::fs::read(install_dir.join("user.cfg")).unwrap();
        assert_eq!(
            restored, b"user_prefs",
            "rollback must restore the original user config"
        );
    }

    /// Max rollback depth is bounded: cleanup removes excess backups.
    #[test]
    fn max_rollback_depth_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let install_dir = temp.path().join("install");
        let backup_dir = temp.path().join("backups");
        let journal_path = temp.path().join("journal.log");

        std::fs::create_dir_all(&install_dir).unwrap();
        std::fs::write(install_dir.join("app.bin"), b"data").unwrap();

        let max_backups = 2;
        let config = UpdateRollbackConfig {
            backup_dir: backup_dir.clone(),
            install_dir: install_dir.clone(),
            journal_path,
            max_backups,
        };

        let fs = RealFileSystem;
        let mgr = UpdateRollbackManager::new(config, fs.clone()).unwrap();

        // Create more backups than allowed
        std::fs::create_dir_all(&backup_dir).unwrap();
        for i in 0..4 {
            let bp = backup_dir.join(format!("backup_{}", 1000 + i));
            std::fs::create_dir_all(&bp).unwrap();
            std::fs::write(bp.join("app.bin"), format!("v{i}").as_bytes()).unwrap();
        }

        mgr.cleanup_backup().unwrap();

        let remaining: Vec<_> = std::fs::read_dir(&backup_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        assert!(
            remaining.len() <= max_backups,
            "must keep at most {} backups, got {}",
            max_backups,
            remaining.len()
        );
    }

    /// Rollback version tracking: journal records state transitions.
    #[test]
    fn rollback_version_tracking_via_journal() {
        let temp = tempfile::tempdir().unwrap();
        let journal_path = temp.path().join("journal.log");

        let fs = RealFileSystem;
        let journal = UpdateJournal::new(journal_path, fs);

        journal
            .record(&UpdateState::Downloading, "2.0.0", "Starting download")
            .unwrap();
        journal
            .record(&UpdateState::Verifying, "2.0.0", "Verifying integrity")
            .unwrap();
        journal
            .record(&UpdateState::Failed, "2.0.0", "Hash mismatch")
            .unwrap();

        let entries = journal.entries().unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].state, UpdateState::Downloading);
        assert_eq!(entries[1].state, UpdateState::Verifying);
        assert_eq!(entries[2].state, UpdateState::Failed);
        assert_eq!(entries[2].version, "2.0.0");
    }

    /// Mid-flight protection: policy defers updates while sim is running.
    #[test]
    fn mid_flight_protection_defers_update() {
        let policy = ManifestUpdatePolicy {
            auto_apply: true,
            defer_while_sim_running: true,
            ..Default::default()
        };

        let state = CurrentState {
            installed_version: SemVer::new(1, 0, 0),
            sim_running: true,
            update_channel: Channel::Stable,
            update_version: SemVer::new(2, 0, 0),
            update_min_version: None,
        };

        let decision = should_apply(&policy, &state);
        match decision {
            UpdateDecision::Defer(reason) => {
                assert!(
                    reason.contains("simulator"),
                    "defer reason must mention simulator: {reason}"
                );
            }
            other => panic!("expected Defer, got {other:?}"),
        }
    }

    /// Journal detects incomplete updates for startup recovery.
    #[test]
    fn journal_detects_incomplete_update() {
        let temp = tempfile::tempdir().unwrap();
        let journal_path = temp.path().join("journal.log");

        let fs = RealFileSystem;
        let journal = UpdateJournal::new(journal_path, fs);

        journal
            .record(&UpdateState::Installing, "2.0.0", "In progress")
            .unwrap();

        let incomplete = journal.check_incomplete().unwrap();
        assert!(
            incomplete.is_some(),
            "journal must detect incomplete update"
        );
        assert_eq!(incomplete.unwrap().state, UpdateState::Installing);
    }

    /// Completed updates are not flagged as incomplete.
    #[test]
    fn journal_complete_is_not_incomplete() {
        let temp = tempfile::tempdir().unwrap();
        let journal_path = temp.path().join("journal.log");

        let fs = RealFileSystem;
        let journal = UpdateJournal::new(journal_path, fs);

        journal
            .record(&UpdateState::Complete, "2.0.0", "Done")
            .unwrap();

        assert!(
            journal.check_incomplete().unwrap().is_none(),
            "completed update must not be flagged as incomplete"
        );
    }

    /// Recovery on startup rolls back when incomplete update detected.
    #[test]
    fn recover_on_startup_rolls_back() {
        let temp = tempfile::tempdir().unwrap();
        let install_dir = temp.path().join("install");
        let backup_dir = temp.path().join("backups");
        let journal_path = temp.path().join("journal.log");

        // Set up a "broken" install and a valid backup
        std::fs::create_dir_all(&install_dir).unwrap();
        std::fs::write(install_dir.join("app.bin"), b"broken_v2").unwrap();

        let backup_path = backup_dir.join("backup_1000");
        std::fs::create_dir_all(&backup_path).unwrap();
        std::fs::write(backup_path.join("app.bin"), b"good_v1").unwrap();

        let config = UpdateRollbackConfig {
            backup_dir,
            install_dir: install_dir.clone(),
            journal_path: journal_path.clone(),
            max_backups: 3,
        };

        let fs = RealFileSystem;

        // Write an incomplete journal entry (simulates crash during install)
        let journal = UpdateJournal::new(journal_path, fs.clone());
        journal
            .record(&UpdateState::Installing, "2.0.0", "Crashed mid-install")
            .unwrap();

        let mut mgr = UpdateRollbackManager::new(config, fs).unwrap();
        let recovered = mgr.recover_on_startup().unwrap();

        assert!(recovered, "must recover from incomplete update");
        let restored = std::fs::read(install_dir.join("app.bin")).unwrap();
        assert_eq!(
            restored, b"good_v1",
            "must restore the backup after crash recovery"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Progress Tracking
// ═══════════════════════════════════════════════════════════════════════════

mod progress_tracking {
    use super::*;

    /// Download progress: the state machine transitions through expected states.
    #[test]
    fn state_machine_transitions_download_through_verify_to_complete() {
        let temp = tempfile::tempdir().unwrap();
        let install_dir = temp.path().join("install");
        let backup_dir = temp.path().join("backups");
        let journal_path = temp.path().join("journal.log");

        std::fs::create_dir_all(&install_dir).unwrap();
        std::fs::write(install_dir.join("app.bin"), b"v1").unwrap();

        let config = UpdateRollbackConfig {
            backup_dir,
            install_dir: install_dir.clone(),
            journal_path,
            max_backups: 3,
        };

        let fs = RealFileSystem;
        let mut mgr = UpdateRollbackManager::new(config, fs).unwrap();
        assert_eq!(*mgr.state(), UpdateState::Idle);

        // Create a valid artifact
        let artifact_data = b"new_binary_v2";
        let artifact_path = temp.path().join("artifact.bin");
        std::fs::write(&artifact_path, artifact_data).unwrap();
        let expected_hash = sha256_hex(artifact_data);

        let artifacts = vec![ArtifactFile {
            path: artifact_path,
            expected_sha256: expected_hash,
        }];

        mgr.apply_update(&artifacts, "2.0.0").unwrap();
        assert_eq!(
            *mgr.state(),
            UpdateState::Complete,
            "state must be Complete after successful update"
        );

        // Verify journal recorded all transitions
        let entries = mgr.journal().entries().unwrap();
        let states: Vec<_> = entries.iter().map(|e| &e.state).collect();
        assert!(states.contains(&&UpdateState::Verifying));
        assert!(states.contains(&&UpdateState::Installing));
        assert!(states.contains(&&UpdateState::Complete));
    }

    /// Cancellation: state returns to Idle after journal clear.
    #[test]
    fn cancellation_resets_journal() {
        let temp = tempfile::tempdir().unwrap();
        let journal_path = temp.path().join("journal.log");

        let fs = RealFileSystem;
        let journal = UpdateJournal::new(journal_path, fs);

        journal
            .record(&UpdateState::Downloading, "2.0.0", "Started")
            .unwrap();
        assert!(!journal.entries().unwrap().is_empty());

        journal.clear().unwrap();
        assert!(
            journal.entries().unwrap().is_empty(),
            "journal must be empty after clear (simulating cancellation)"
        );
    }

    /// Update history tracking with success rate calculation.
    #[test]
    fn update_history_tracks_success_rate() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);

        mgr.record_update(UpdateRecord {
            from_version: "1.0.0".into(),
            to_version: "1.1.0".into(),
            timestamp: 1000,
            success: true,
            was_delta: false,
        });
        mgr.record_update(UpdateRecord {
            from_version: "1.1.0".into(),
            to_version: "1.2.0".into(),
            timestamp: 2000,
            success: true,
            was_delta: true,
        });
        mgr.record_update(UpdateRecord {
            from_version: "1.2.0".into(),
            to_version: "1.3.0".into(),
            timestamp: 3000,
            success: false,
            was_delta: false,
        });

        let rate = mgr.success_rate();
        assert!(
            (rate - 2.0 / 3.0).abs() < 1e-12,
            "success rate must be 2/3 ≈ 0.667, got {rate}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Integration Scenarios
// ═══════════════════════════════════════════════════════════════════════════

mod integration_scenarios {
    use super::*;

    /// Full lifecycle: check → download (simulated) → verify → apply → verify → complete.
    #[test]
    fn full_update_lifecycle() {
        let temp = tempfile::tempdir().unwrap();
        let install_dir = temp.path().join("install");
        let backup_dir = temp.path().join("backups");
        let journal_path = temp.path().join("journal.log");

        // 1. Initial installation
        std::fs::create_dir_all(&install_dir).unwrap();
        std::fs::write(install_dir.join("app.bin"), b"v1_binary").unwrap();
        std::fs::write(install_dir.join("config.toml"), b"[settings]\nkey=val").unwrap();

        let config = UpdateRollbackConfig {
            backup_dir,
            install_dir: install_dir.clone(),
            journal_path,
            max_backups: 3,
        };

        let fs = RealFileSystem;
        let mut mgr = UpdateRollbackManager::new(config, fs).unwrap();

        // 2. Simulate download by writing artifact to temp
        let artifact_data = b"v2_binary_new";
        let artifact_path = temp.path().join("download_v2.bin");
        std::fs::write(&artifact_path, artifact_data).unwrap();

        // 3. Verify + Apply
        let artifacts = vec![ArtifactFile {
            path: artifact_path,
            expected_sha256: sha256_hex(artifact_data),
        }];

        mgr.apply_update(&artifacts, "2.0.0").unwrap();

        // 4. Verify final state
        assert_eq!(*mgr.state(), UpdateState::Complete);

        // 5. Installed artifact is present
        let installed = std::fs::read(install_dir.join("download_v2.bin")).unwrap();
        assert_eq!(installed, artifact_data);
    }

    /// Offline mode: cached update manifest is usable without network.
    #[test]
    fn offline_cached_manifest() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);

        // Simulate a previously cached manifest
        let manifest = make_update_manifest(vec![make_entry("1.5.0", UpdateChannel::Stable)]);
        mgr.load_manifest(manifest);

        // Even "offline" (no network call), we can check for updates
        assert!(
            mgr.is_update_available(),
            "cached manifest must show available update"
        );
        assert_eq!(mgr.check_for_update().unwrap().version, "1.5.0");
    }

    /// Multiple pending updates → latest wins.
    #[test]
    fn multiple_pending_updates_latest_wins() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);

        let manifest = make_update_manifest(vec![
            make_entry("1.1.0", UpdateChannel::Stable),
            make_entry("1.2.0", UpdateChannel::Stable),
            make_entry("1.5.0", UpdateChannel::Stable),
            make_entry("1.3.0", UpdateChannel::Stable),
        ]);
        mgr.load_manifest(manifest);

        let update = mgr.check_for_update().unwrap();
        assert_eq!(
            update.version, "1.5.0",
            "must select the latest available version"
        );
    }

    /// Integrity verification catches mismatched SHA-256.
    #[test]
    fn integrity_verification_catches_mismatch() {
        let data = b"actual file content";
        let mut entry = make_entry("1.0.0", UpdateChannel::Stable);
        entry.sha256 = "0".repeat(64);

        assert!(
            !ManifestUpdateManager::verify_integrity(&entry, data),
            "wrong hash must fail integrity check"
        );
    }

    /// Integrity verification passes for correct hash.
    #[test]
    fn integrity_verification_passes_for_correct_hash() {
        let data = b"actual file content";
        let mut entry = make_entry("1.0.0", UpdateChannel::Stable);
        entry.sha256 = sha256_hex(data);

        assert!(
            ManifestUpdateManager::verify_integrity(&entry, data),
            "correct hash must pass integrity check"
        );
    }

    /// Version below min_version is skipped by policy.
    #[test]
    fn version_below_min_is_skipped() {
        let policy = ManifestUpdatePolicy {
            auto_apply: true,
            ..Default::default()
        };
        let state = CurrentState {
            installed_version: SemVer::new(0, 5, 0),
            sim_running: false,
            update_channel: Channel::Stable,
            update_version: SemVer::new(2, 0, 0),
            update_min_version: Some(SemVer::new(1, 0, 0)),
        };

        let decision = should_apply(&policy, &state);
        assert!(
            matches!(decision, UpdateDecision::Skip(_)),
            "version below min must be skipped: got {decision:?}"
        );
    }

    /// Already up-to-date → skip.
    #[test]
    fn already_up_to_date_is_skipped() {
        let policy = ManifestUpdatePolicy {
            auto_apply: true,
            ..Default::default()
        };
        let state = CurrentState {
            installed_version: SemVer::new(2, 0, 0),
            sim_running: false,
            update_channel: Channel::Stable,
            update_version: SemVer::new(2, 0, 0),
            update_min_version: None,
        };

        let decision = should_apply(&policy, &state);
        assert!(
            matches!(decision, UpdateDecision::Skip(_)),
            "already at version must be skipped"
        );
    }

    /// Delta update availability depends on min_version matching current.
    #[test]
    fn delta_update_requires_min_version_match() {
        let mut mgr = ManifestUpdateManager::new("1.0.0", UpdateChannel::Stable);

        // Entry with min_version matching current
        let mut entry = make_entry("1.1.0", UpdateChannel::Stable);
        entry.min_version = Some("1.0.0".to_string());
        entry.delta_url = Some("https://example.com/delta".to_string());
        mgr.load_manifest(make_update_manifest(vec![entry]));

        assert!(
            mgr.can_delta_update(),
            "delta must be available when min_version matches"
        );

        // Entry with min_version NOT matching current
        let mut mgr2 = ManifestUpdateManager::new("0.9.0", UpdateChannel::Stable);
        let mut entry2 = make_entry("1.1.0", UpdateChannel::Stable);
        entry2.min_version = Some("1.0.0".to_string());
        entry2.delta_url = Some("https://example.com/delta".to_string());
        mgr2.load_manifest(make_update_manifest(vec![entry2]));

        assert!(
            !mgr2.can_delta_update(),
            "delta must not be available when min_version doesn't match"
        );
    }

    /// UpdateResult serialization round-trip.
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
        assert_eq!(parsed.updated, result.updated);
        assert_eq!(parsed.new_version, result.new_version);
        assert_eq!(parsed.rollback_occurred, result.rollback_occurred);
    }

    /// UpdateConfig defaults are sensible.
    #[test]
    fn update_config_defaults() {
        let config = UpdateConfig::default();
        assert_eq!(config.channel, Channel::Stable);
        assert!(config.auto_check);
        assert!(!config.auto_install);
        assert!(config.max_rollback_versions > 0);
        assert!(config.startup_timeout_seconds > 0);
    }
}
