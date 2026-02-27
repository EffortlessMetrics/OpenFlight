// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive integration tests for flight-updater.
//!
//! Covers: manifest parsing, version comparison, channel selection, checksum
//! validation, update-state fields, delta-patch size accounting, and property-
//! based ordering invariants.

use flight_updater::{
    Channel,
    channels::ChannelManager,
    delta::{DeltaOperation, DeltaPatch, FileDelta},
    rollback::VersionInfo,
    signature::SignatureVerifier,
    updater::{UpdateConfig, UpdateResult},
};
use proptest::prelude::*;
use std::path::PathBuf;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Build a `VersionInfo` with an explicit `build_timestamp` so that tests are
/// deterministic regardless of wall-clock time.
fn make_version(version: &str, build_timestamp: u64) -> VersionInfo {
    VersionInfo {
        version: version.to_string(),
        build_timestamp,
        commit_hash: "deadbeef".to_string(),
        channel: Channel::Stable,
        install_timestamp: build_timestamp,
        install_path: PathBuf::from("/tmp/test"),
        backup_path: None,
    }
}

// ── 1. Update manifest (AvailableUpdate) parses valid JSON ───────────────────

/// The `AvailableUpdate` type must deserialise from well-formed JSON, preserving
/// every field exactly.  This acts as a regression guard for the manifest wire
/// format.
#[test]
fn available_update_parses_valid_json() {
    let json = r#"{
        "version": "1.2.0",
        "channel": "Stable",
        "size": 4096,
        "release_notes_url": "https://example.com/notes/1.2.0",
        "is_security_update": false,
        "priority": 3
    }"#;

    let update: flight_updater::updater::AvailableUpdate =
        serde_json::from_str(json).expect("must parse valid JSON");

    assert_eq!(update.version, "1.2.0");
    assert_eq!(update.channel, Channel::Stable);
    assert_eq!(update.size, 4096);
    assert_eq!(
        update.release_notes_url.as_deref(),
        Some("https://example.com/notes/1.2.0")
    );
    assert!(!update.is_security_update);
    assert_eq!(update.priority, 3);
}

/// An `AvailableUpdate` must survive a full JSON serialise → deserialise
/// round-trip with all fields intact.
#[test]
fn available_update_json_roundtrip() {
    let original = flight_updater::updater::AvailableUpdate {
        version: "2.0.0-beta.1".to_string(),
        channel: Channel::Beta,
        size: 8192,
        release_notes_url: Some("https://example.com/beta".to_string()),
        is_security_update: true,
        priority: 5,
    };

    let json = serde_json::to_string(&original).expect("serialisation must succeed");
    let parsed: flight_updater::updater::AvailableUpdate =
        serde_json::from_str(&json).expect("deserialisation must succeed");

    assert_eq!(parsed.version, original.version);
    assert_eq!(parsed.channel, original.channel);
    assert_eq!(parsed.size, original.size);
    assert_eq!(parsed.release_notes_url, original.release_notes_url);
    assert_eq!(parsed.is_security_update, original.is_security_update);
    assert_eq!(parsed.priority, original.priority);
}

// ── 2 & 3. Version comparison and semver-style ordering ─────────────────────

/// `is_newer_than` must return `true` when the caller's `build_timestamp` is
/// strictly greater than the argument's.
#[test]
fn version_is_newer_than_returns_true_for_greater_timestamp() {
    let newer = make_version("1.2.0", 2000);
    let older = make_version("1.1.0", 1000);
    assert!(
        newer.is_newer_than(&older),
        "1.2.0 (ts=2000) must be newer than 1.1.0 (ts=1000)"
    );
}

/// `is_newer_than` must return `false` when the caller's timestamp is strictly
/// less than the argument's.
#[test]
fn version_is_newer_than_returns_false_for_lesser_timestamp() {
    let newer = make_version("1.2.0", 2000);
    let older = make_version("1.1.0", 1000);
    assert!(
        !older.is_newer_than(&newer),
        "1.1.0 (ts=1000) must not be newer than 1.2.0 (ts=2000)"
    );
}

/// When two `VersionInfo` objects share the same `build_timestamp` neither is
/// considered newer than the other (strict `>` comparison).
#[test]
fn version_is_newer_than_returns_false_for_equal_timestamps() {
    let v1 = make_version("1.0.0", 1000);
    let v2 = make_version("1.0.0", 1000);
    assert!(
        !v1.is_newer_than(&v2),
        "equal timestamps must not be considered newer"
    );
    assert!(
        !v2.is_newer_than(&v1),
        "symmetrically, equal timestamps must not be considered newer"
    );
}

/// When versions are recorded in semver order their timestamps should reflect
/// the same ordering.  Validate the four-level chain 1.0.0 < 1.1.0 < 1.2.2 <
/// 1.2.3, including transitivity.
#[test]
fn version_timestamps_reflect_semver_ordering() {
    let v100 = make_version("1.0.0", 1000);
    let v110 = make_version("1.1.0", 2000);
    let v122 = make_version("1.2.2", 3000);
    let v123 = make_version("1.2.3", 4000);

    assert!(v110.is_newer_than(&v100), "1.1.0 > 1.0.0");
    assert!(v122.is_newer_than(&v110), "1.2.2 > 1.1.0");
    assert!(v123.is_newer_than(&v122), "1.2.3 > 1.2.2");

    // Transitivity: 1.2.3 > 1.0.0
    assert!(
        v123.is_newer_than(&v100),
        "1.2.3 must be newer than 1.0.0 transitively"
    );
}

// ── 4–6. Channel selection: accept_prerelease flags ─────────────────────────

/// The stable channel must never accept prerelease builds.
#[test]
fn stable_channel_does_not_accept_prerelease() {
    let mgr = ChannelManager::new();
    let cfg = mgr
        .get_config(Channel::Stable)
        .expect("stable config must exist");
    assert!(
        !cfg.accept_prerelease,
        "stable channel must not accept prerelease builds"
    );
}

/// The beta channel must accept prerelease builds.
#[test]
fn beta_channel_accepts_prerelease() {
    let mgr = ChannelManager::new();
    let cfg = mgr
        .get_config(Channel::Beta)
        .expect("beta config must exist");
    assert!(
        cfg.accept_prerelease,
        "beta channel must accept prerelease builds"
    );
}

/// The canary channel must accept prerelease builds (most permissive channel).
#[test]
fn canary_channel_accepts_prerelease() {
    let mgr = ChannelManager::new();
    let cfg = mgr
        .get_config(Channel::Canary)
        .expect("canary config must exist");
    assert!(
        cfg.accept_prerelease,
        "canary channel must accept prerelease builds"
    );
}

// ── 8–9. SHA-256 checksum validation ────────────────────────────────────────

/// `hash_content(b"")` must produce the well-known SHA-256 digest of the empty
/// string, proving the hash function is wired up correctly.
#[test]
fn sha256_hash_of_empty_content_is_correct() {
    // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb924…
    let verifier = SignatureVerifier::new(&hex::encode([0u8; 32])).unwrap();
    let digest = verifier.hash_content(b"");
    assert_eq!(
        digest, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "SHA-256 of the empty string must match the known constant"
    );
}

/// Two distinct byte strings must produce different digests (collision
/// resistance sanity check).
#[test]
fn sha256_hash_differs_for_different_content() {
    let verifier = SignatureVerifier::new(&hex::encode([0u8; 32])).unwrap();
    let h1 = verifier.hash_content(b"flight-hub");
    let h2 = verifier.hash_content(b"flight-hub-update");
    assert_ne!(h1, h2, "distinct content must produce distinct digests");
}

/// Hashing the same content twice must produce identical digests (determinism).
#[test]
fn sha256_hash_is_deterministic() {
    let verifier = SignatureVerifier::new(&hex::encode([0u8; 32])).unwrap();
    let content = b"stable-release-1.0.0";
    assert_eq!(
        verifier.hash_content(content),
        verifier.hash_content(content)
    );
}

// ── 10. UpdateResult reflects a completed update ────────────────────────────

/// A successfully completed (non-rollback) update must set `updated = true`,
/// `rollback_occurred = false`, and carry both `previous_version` and
/// `new_version`.
#[test]
fn update_result_completed_state_has_expected_fields() {
    let result = UpdateResult {
        updated: true,
        previous_version: Some("1.0.0".to_string()),
        new_version: Some("1.1.0".to_string()),
        rollback_occurred: false,
        channel: Channel::Stable,
        update_size: 2048,
        duration_seconds: 10,
    };

    assert!(
        result.updated,
        "updated must be true for a completed update"
    );
    assert!(
        !result.rollback_occurred,
        "rollback_occurred must be false for a normal update"
    );
    assert_eq!(result.previous_version.as_deref(), Some("1.0.0"));
    assert_eq!(result.new_version.as_deref(), Some("1.1.0"));
    assert_eq!(result.update_size, 2048);
}

/// An `UpdateResult` that records a rollback must set `rollback_occurred = true`
/// and `updated = true`.
#[test]
fn update_result_rollback_state_has_expected_fields() {
    let result = UpdateResult {
        updated: true,
        previous_version: Some("1.1.0".to_string()),
        new_version: Some("1.0.0".to_string()),
        rollback_occurred: true,
        channel: Channel::Stable,
        update_size: 0,
        duration_seconds: 2,
    };

    assert!(result.updated);
    assert!(
        result.rollback_occurred,
        "rollback_occurred must be true for a rollback result"
    );
    // After rollback new_version is the *older* version we rolled back to
    assert_eq!(result.new_version.as_deref(), Some("1.0.0"));
}

// ── UpdateConfig serialisation round-trip ───────────────────────────────────

/// `UpdateConfig` must survive a full JSON round-trip, preserving every field.
#[test]
fn update_config_json_roundtrip() {
    let config = UpdateConfig {
        install_dir: PathBuf::from("/opt/flight-hub"),
        update_dir: PathBuf::from("/var/lib/flight-hub/updates"),
        current_version: "1.5.2".to_string(),
        channel: Channel::Beta,
        auto_check: true,
        auto_install: false,
        max_rollback_versions: 5,
        startup_timeout_seconds: 120,
    };

    let json = serde_json::to_string(&config).expect("serialisation must succeed");
    let parsed: UpdateConfig = serde_json::from_str(&json).expect("deserialisation must succeed");

    assert_eq!(parsed.current_version, config.current_version);
    assert_eq!(parsed.channel, config.channel);
    assert_eq!(parsed.auto_check, config.auto_check);
    assert_eq!(parsed.auto_install, config.auto_install);
    assert_eq!(parsed.max_rollback_versions, config.max_rollback_versions);
    assert_eq!(
        parsed.startup_timeout_seconds,
        config.startup_timeout_seconds
    );
}

// ── 11. DeltaPatch size calculation ─────────────────────────────────────────

/// `calculate_size` must count only `Insert` operation data and new-file bytes;
/// `Copy` and `Delete` operations have zero weight in the patch size.
#[test]
fn delta_patch_calculate_size_counts_insert_and_new_files_only() {
    let mut patch = DeltaPatch::new("1.0.0".to_string(), "1.1.0".to_string());

    let file_delta = FileDelta {
        source_path: "app.bin".to_string(),
        target_path: "app.bin".to_string(),
        source_hash: "ignored".to_string(),
        target_hash: "ignored".to_string(),
        operations: vec![
            DeltaOperation::Insert {
                data: vec![0u8; 100],
            }, // +100 bytes
            DeltaOperation::Copy {
                src_offset: 0,
                length: 50,
            }, // no patch bytes
            DeltaOperation::Delete { length: 20 }, // no patch bytes
        ],
        compression: "none".to_string(),
    };
    patch.add_file_delta(file_delta);

    // A brand-new file of 200 bytes
    patch.add_new_file("new.bin".to_string(), vec![0u8; 200]);

    patch.calculate_size();

    assert_eq!(
        patch.patch_size, 300,
        "patch_size must be Insert(100) + new_file(200) = 300"
    );
}

/// A patch with no Insert operations and no new files must have a patch_size of 0.
#[test]
fn delta_patch_size_is_zero_with_no_inserts_or_new_files() {
    let mut patch = DeltaPatch::new("1.0.0".to_string(), "1.1.0".to_string());

    let file_delta = FileDelta {
        source_path: "config.bin".to_string(),
        target_path: "config.bin".to_string(),
        source_hash: "h1".to_string(),
        target_hash: "h2".to_string(),
        operations: vec![
            DeltaOperation::Copy {
                src_offset: 0,
                length: 512,
            },
            DeltaOperation::Delete { length: 64 },
        ],
        compression: "none".to_string(),
    };
    patch.add_file_delta(file_delta);
    patch.add_deleted_file("old.bin".to_string());

    patch.calculate_size();

    assert_eq!(
        patch.patch_size, 0,
        "patch with only Copy/Delete/deleted files must have size 0"
    );
}

// ── 12. Proptest: version ordering is antisymmetric and consistent ───────────

proptest! {
    /// For any two distinct timestamps the ordering produced by `is_newer_than`
    /// must be antisymmetric: if A > B then B must not be > A, and vice versa.
    #[test]
    fn prop_version_ordering_is_antisymmetric(
        ts_a in 1u64..100_000u64,
        ts_b in 1u64..100_000u64,
    ) {
        let a = make_version("va", ts_a);
        let b = make_version("vb", ts_b);

        if ts_a == ts_b {
            prop_assert!(!a.is_newer_than(&b), "equal timestamps: a must not be newer than b");
            prop_assert!(!b.is_newer_than(&a), "equal timestamps: b must not be newer than a");
        } else if ts_a > ts_b {
            prop_assert!(a.is_newer_than(&b), "a(ts={ts_a}) must be newer than b(ts={ts_b})");
            prop_assert!(!b.is_newer_than(&a), "b must not be newer than a when ts_a > ts_b");
        } else {
            prop_assert!(!a.is_newer_than(&b), "a must not be newer than b when ts_a < ts_b");
            prop_assert!(b.is_newer_than(&a), "b(ts={ts_b}) must be newer than a(ts={ts_a})");
        }
    }

    /// For any three timestamps, if ts_a > ts_b > ts_c then `is_newer_than` must
    /// be transitive: a > c must also hold.
    #[test]
    fn prop_version_ordering_is_transitive(
        ts_a in 2u64..100_000u64,
        ts_b in 1u64..100_000u64,
        ts_c in 1u64..100_000u64,
    ) {
        // Only assert transitivity when we have a strict ordering
        if ts_a > ts_b && ts_b > ts_c {
            let a = make_version("va", ts_a);
            let b = make_version("vb", ts_b);
            let c = make_version("vc", ts_c);
            prop_assert!(a.is_newer_than(&b));
            prop_assert!(b.is_newer_than(&c));
            prop_assert!(a.is_newer_than(&c), "ordering must be transitive: a > b > c → a > c");
        }
    }
}
