// SPDX-License-Identifier: Apache-2.0 OR MIT
// SPDX-FileCopyrightText: Copyright (c) 2024-2026 OpenFlight Contributors

//! Update manifest validation tests.
//!
//! Validates the JSON schema, channel progression, rollback metadata,
//! delta vs full-image entries, and manifest signature logic.

use std::path::Path;

use serde::{Deserialize, Serialize};

// ── Types mirroring the update manifest schema ───────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdateManifest {
    schema_version: u32,
    product: String,
    channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    generated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    entries: Vec<UpdateEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdateEntry {
    version: String,
    platforms: Vec<String>,
    #[serde(default = "default_update_type")]
    update_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    delta_from: Option<String>,
    checksums: Checksums,
    urls: Urls,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_bytes: Option<u64>,
    rollback: RollbackMeta,
    #[serde(skip_serializing_if = "Option::is_none")]
    release_notes: Option<String>,
}

fn default_update_type() -> String {
    "full".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Checksums {
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Urls {
    primary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mirror: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RollbackMeta {
    supported: bool,
    previous_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rollback_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rollback_checksum: Option<String>,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn load_schema() -> serde_json::Value {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let schema_path = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("installer/update-manifest-schema.json");
    let text = std::fs::read_to_string(&schema_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", schema_path.display()));
    serde_json::from_str(&text).expect("invalid JSON schema")
}

fn sample_manifest(channel: &str) -> UpdateManifest {
    UpdateManifest {
        schema_version: 1,
        product: "flight-hub".into(),
        channel: channel.into(),
        generated_at: Some("2025-01-15T10:00:00Z".into()),
        signature: None,
        entries: vec![
            UpdateEntry {
                version: "0.2.0".into(),
                platforms: vec!["windows-x86_64".into(), "linux-x86_64".into()],
                update_type: "full".into(),
                delta_from: None,
                checksums: Checksums {
                    sha256: "a".repeat(64),
                },
                urls: Urls {
                    primary: "https://releases.flight-hub.dev/v0.2.0/flight-hub-full.tar.gz".into(),
                    mirror: Some(
                        "https://mirror.flight-hub.dev/v0.2.0/flight-hub-full.tar.gz".into(),
                    ),
                },
                size_bytes: Some(15_000_000),
                rollback: RollbackMeta {
                    supported: true,
                    previous_version: "0.1.0".into(),
                    rollback_url: Some(
                        "https://releases.flight-hub.dev/v0.1.0/flight-hub-full.tar.gz".into(),
                    ),
                    rollback_checksum: Some("b".repeat(64)),
                },
                release_notes: Some("Initial beta release".into()),
            },
            UpdateEntry {
                version: "0.2.0".into(),
                platforms: vec!["windows-x86_64".into(), "linux-x86_64".into()],
                update_type: "delta".into(),
                delta_from: Some("0.1.0".into()),
                checksums: Checksums {
                    sha256: "c".repeat(64),
                },
                urls: Urls {
                    primary: "https://releases.flight-hub.dev/v0.2.0/flight-hub-delta-0.1.0.tar.gz"
                        .into(),
                    mirror: None,
                },
                size_bytes: Some(3_000_000),
                rollback: RollbackMeta {
                    supported: true,
                    previous_version: "0.1.0".into(),
                    rollback_url: None,
                    rollback_checksum: None,
                },
                release_notes: None,
            },
        ],
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Schema structure
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn update_schema_is_valid_json() {
    let schema = load_schema();
    assert!(schema.is_object());
}

#[test]
fn update_schema_has_required_top_level_fields() {
    let schema = load_schema();
    let required = schema["required"].as_array().expect("required array");
    let fields: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(fields.contains(&"schema_version"));
    assert!(fields.contains(&"product"));
    assert!(fields.contains(&"channel"));
    assert!(fields.contains(&"entries"));
}

#[test]
fn update_schema_channel_enum_values() {
    let schema = load_schema();
    let channels = schema["properties"]["channel"]["enum"]
        .as_array()
        .expect("channel enum");
    let vals: Vec<&str> = channels.iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(vals, vec!["canary", "beta", "stable"]);
}

#[test]
fn update_schema_entry_requires_rollback() {
    let schema = load_schema();
    let entry_required = schema["definitions"]["update_entry"]["required"]
        .as_array()
        .expect("entry required array");
    let fields: Vec<&str> = entry_required.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(fields.contains(&"rollback"));
}

// ═════════════════════════════════════════════════════════════════════════════
// Manifest round-trip serialization
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn update_manifest_roundtrips_through_json() {
    let m = sample_manifest("canary");
    let json = serde_json::to_string_pretty(&m).expect("serialize");
    let m2: UpdateManifest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(m.entries.len(), m2.entries.len());
    assert_eq!(m.channel, m2.channel);
    assert_eq!(m.product, m2.product);
}

// ═════════════════════════════════════════════════════════════════════════════
// Channel progression
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn channel_progression_canary_before_beta_before_stable() {
    let channels = ["canary", "beta", "stable"];
    for (i, &ch) in channels.iter().enumerate() {
        let m = sample_manifest(ch);
        let json = serde_json::to_string(&m).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["channel"].as_str().unwrap(), channels[i]);
    }
    // Ordering: canary < beta < stable
    fn channel_rank(ch: &str) -> u8 {
        match ch {
            "canary" => 0,
            "beta" => 1,
            "stable" => 2,
            _ => panic!("unknown channel: {ch}"),
        }
    }
    assert!(channel_rank("canary") < channel_rank("beta"));
    assert!(channel_rank("beta") < channel_rank("stable"));
}

#[test]
fn invalid_channel_not_in_schema_enum() {
    let schema = load_schema();
    let channels: Vec<&str> = schema["properties"]["channel"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(!channels.contains(&"nightly"));
    assert!(!channels.contains(&"rc"));
}

// ═════════════════════════════════════════════════════════════════════════════
// Rollback metadata
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn every_entry_has_rollback_metadata() {
    let m = sample_manifest("stable");
    for entry in &m.entries {
        assert!(
            !entry.rollback.previous_version.is_empty(),
            "entry {} missing rollback previous_version",
            entry.version
        );
    }
}

#[test]
fn rollback_previous_version_is_valid_semver() {
    let m = sample_manifest("stable");
    for entry in &m.entries {
        let v: semver::Version = entry.rollback.previous_version.parse().unwrap_or_else(|e| {
            panic!(
                "invalid semver in rollback.previous_version '{}': {e}",
                entry.rollback.previous_version
            )
        });
        assert!(
            v.major + v.minor + v.patch < 1000,
            "suspiciously large version"
        );
    }
}

#[test]
fn rollback_previous_version_differs_from_current() {
    let m = sample_manifest("stable");
    for entry in &m.entries {
        assert_ne!(
            entry.version, entry.rollback.previous_version,
            "rollback must reference a different version"
        );
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Delta vs full-image entries
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn full_entries_have_no_delta_from() {
    let m = sample_manifest("beta");
    for entry in m.entries.iter().filter(|e| e.update_type == "full") {
        assert!(
            entry.delta_from.is_none(),
            "full entry {} must not have delta_from",
            entry.version
        );
    }
}

#[test]
fn delta_entries_have_delta_from() {
    let m = sample_manifest("beta");
    for entry in m.entries.iter().filter(|e| e.update_type == "delta") {
        assert!(
            entry.delta_from.is_some(),
            "delta entry {} must have delta_from",
            entry.version
        );
    }
}

#[test]
fn delta_entry_size_smaller_than_full() {
    let m = sample_manifest("beta");
    let full_size = m
        .entries
        .iter()
        .find(|e| e.update_type == "full")
        .and_then(|e| e.size_bytes);
    let delta_size = m
        .entries
        .iter()
        .find(|e| e.update_type == "delta")
        .and_then(|e| e.size_bytes);

    if let (Some(full), Some(delta)) = (full_size, delta_size) {
        assert!(
            delta < full,
            "delta ({delta}) should be smaller than full ({full})"
        );
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Checksums and URLs
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn all_entries_have_sha256_checksums() {
    let m = sample_manifest("stable");
    for entry in &m.entries {
        assert_eq!(
            entry.checksums.sha256.len(),
            64,
            "sha256 must be 64 hex chars for version {}",
            entry.version
        );
    }
}

#[test]
fn all_entries_have_primary_url() {
    let m = sample_manifest("stable");
    for entry in &m.entries {
        assert!(
            entry.urls.primary.starts_with("https://"),
            "primary URL must be HTTPS for version {}",
            entry.version
        );
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Mock signature validation
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn manifest_signature_field_accepted() {
    let mut m = sample_manifest("canary");
    m.signature = Some("deadbeef".repeat(8));
    let json = serde_json::to_string(&m).expect("serialize with signature");
    let m2: UpdateManifest = serde_json::from_str(&json).expect("deserialize");
    assert!(m2.signature.is_some());
}

#[test]
fn manifest_without_signature_accepted() {
    let m = sample_manifest("canary");
    assert!(m.signature.is_none());
    let json = serde_json::to_string(&m).expect("serialize without signature");
    let m2: UpdateManifest = serde_json::from_str(&json).expect("deserialize");
    assert!(m2.signature.is_none());
}
