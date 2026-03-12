// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Signed update manifest with per-file deltas, Ed25519 signature
//! verification, and semantic versioning.

use crate::channels::Channel;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

// ---------------------------------------------------------------------------
// SemVer
// ---------------------------------------------------------------------------

/// Parsed semantic version triple.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse a `"major.minor.patch"` string.
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
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

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

// ---------------------------------------------------------------------------
// FileUpdate
// ---------------------------------------------------------------------------

/// The kind of change a file update represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileOperation {
    Add,
    Modify,
    Remove,
}

/// Describes a single file change within an update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileUpdate {
    /// Relative path inside the installation directory.
    pub path: String,
    /// SHA-256 of the file *before* the update (empty for `Add`).
    pub hash_before: String,
    /// SHA-256 of the file *after* the update (empty for `Remove`).
    pub hash_after: String,
    /// Size in bytes of the new file content (0 for `Remove`).
    pub size: u64,
    /// Kind of change.
    pub operation: FileOperation,
}

// ---------------------------------------------------------------------------
// UpdateManifest
// ---------------------------------------------------------------------------

/// A signed update descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateManifest {
    /// Target version this manifest installs.
    pub version: SemVer,
    /// Release channel.
    pub channel: Channel,
    /// List of file-level changes.
    pub files: Vec<FileUpdate>,
    /// Hex-encoded Ed25519 signature over the canonical JSON of all other
    /// fields (i.e. everything except `signature` itself).
    pub signature: String,
    /// If set, the update can only be applied when the current installed
    /// version is ≥ this value.
    pub min_version: Option<SemVer>,
}

// ---------------------------------------------------------------------------
// Signed content helper
// ---------------------------------------------------------------------------

/// Intermediate structure that mirrors `UpdateManifest` but without the
/// `signature` field — used to derive the canonical bytes that are signed.
#[derive(Serialize)]
struct ManifestContent<'a> {
    version: &'a SemVer,
    channel: &'a Channel,
    files: &'a [FileUpdate],
    min_version: &'a Option<SemVer>,
}

impl UpdateManifest {
    /// Return the canonical JSON bytes that are covered by the signature.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let content = ManifestContent {
            version: &self.version,
            channel: &self.channel,
            files: &self.files,
            min_version: &self.min_version,
        };
        serde_json::to_vec(&content).expect("canonical serialization cannot fail")
    }
}

// ---------------------------------------------------------------------------
// parse / verify
// ---------------------------------------------------------------------------

/// Deserialize a JSON-encoded `UpdateManifest`.
pub fn parse(json_bytes: &[u8]) -> crate::Result<UpdateManifest> {
    serde_json::from_slice(json_bytes).map_err(crate::UpdateError::Serialization)
}

/// Verify the Ed25519 signature embedded in `manifest` against
/// `public_key_hex` (hex-encoded 32-byte verifying key).
pub fn verify_signature(manifest: &UpdateManifest, public_key_hex: &str) -> crate::Result<()> {
    let key_bytes = hex::decode(public_key_hex)
        .map_err(|e| crate::UpdateError::InvalidSignature(format!("bad public key hex: {e}")))?;
    let key_array: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| crate::UpdateError::InvalidSignature("public key must be 32 bytes".into()))?;
    let verifying_key = VerifyingKey::from_bytes(&key_array)
        .map_err(|e| crate::UpdateError::InvalidSignature(format!("invalid public key: {e}")))?;

    let sig_bytes = hex::decode(&manifest.signature)
        .map_err(|e| crate::UpdateError::InvalidSignature(format!("bad signature hex: {e}")))?;
    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| crate::UpdateError::InvalidSignature("signature must be 64 bytes".into()))?;
    let signature = Signature::from_bytes(&sig_array);

    let message = manifest.canonical_bytes();
    verifying_key.verify(&message, &signature).map_err(|e| {
        crate::UpdateError::InvalidSignature(format!("signature verification failed: {e}"))
    })
}

// ---------------------------------------------------------------------------
// Platform-specific release manifest
// ---------------------------------------------------------------------------

/// Target platform for an artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Windows,
    Linux,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Windows => write!(f, "windows"),
            Platform::Linux => write!(f, "linux"),
        }
    }
}

/// CPU architecture for an artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    X64,
    Arm64,
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Architecture::X64 => write!(f, "x64"),
            Architecture::Arm64 => write!(f, "arm64"),
        }
    }
}

/// A downloadable artifact for a specific platform and architecture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformArtifact {
    /// Download URL for this artifact.
    pub url: String,
    /// Target platform.
    pub platform: Platform,
    /// Target CPU architecture.
    pub architecture: Architecture,
    /// SHA-256 checksum (64 hex characters).
    pub sha256: String,
    /// Size in bytes.
    pub size_bytes: u64,
}

/// A release manifest describing an available update with per-platform
/// artifacts, release date, and optional release notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifest {
    /// Target version this release installs.
    pub version: SemVer,
    /// Release channel.
    pub channel: Channel,
    /// ISO 8601 release date (e.g. `"2025-07-10"`).
    pub release_date: String,
    /// Platform-specific download artifacts.
    pub artifacts: Vec<PlatformArtifact>,
    /// Optional human-readable release notes.
    pub release_notes: Option<String>,
}

impl ReleaseManifest {
    /// Validate required fields and checksum format.
    ///
    /// Returns `Ok(())` when the manifest is well-formed, or an error
    /// string describing the first problem found.
    pub fn validate(&self) -> crate::Result<()> {
        if self.release_date.is_empty() {
            return Err(crate::UpdateError::VersionValidation(
                "release_date is required".into(),
            ));
        }
        if self.artifacts.is_empty() {
            return Err(crate::UpdateError::VersionValidation(
                "at least one artifact is required".into(),
            ));
        }
        for (i, artifact) in self.artifacts.iter().enumerate() {
            if artifact.url.is_empty() {
                return Err(crate::UpdateError::VersionValidation(format!(
                    "artifact[{i}]: url must not be empty"
                )));
            }
            if !crate::is_valid_sha256(&artifact.sha256) {
                return Err(crate::UpdateError::VersionValidation(format!(
                    "artifact[{i}]: sha256 must be 64 hex characters"
                )));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey, pkcs8::DecodePrivateKey};
    use uselesskey::{Ed25519FactoryExt, Ed25519Spec, Factory, Seed};

    // -- helpers ----------------------------------------------------------

    fn sample_file_update(op: FileOperation) -> FileUpdate {
        match op {
            FileOperation::Add => FileUpdate {
                path: "bin/new.exe".into(),
                hash_before: String::new(),
                hash_after: "bb".repeat(32),
                size: 2048,
                operation: FileOperation::Add,
            },
            FileOperation::Modify => FileUpdate {
                path: "lib/core.dll".into(),
                hash_before: "aa".repeat(32),
                hash_after: "cc".repeat(32),
                size: 4096,
                operation: FileOperation::Modify,
            },
            FileOperation::Remove => FileUpdate {
                path: "tmp/old.log".into(),
                hash_before: "dd".repeat(32),
                hash_after: String::new(),
                size: 0,
                operation: FileOperation::Remove,
            },
        }
    }

    fn unsigned_manifest() -> UpdateManifest {
        UpdateManifest {
            version: SemVer::new(2, 0, 0),
            channel: Channel::Stable,
            files: vec![
                sample_file_update(FileOperation::Add),
                sample_file_update(FileOperation::Modify),
                sample_file_update(FileOperation::Remove),
            ],
            signature: String::new(),
            min_version: Some(SemVer::new(1, 0, 0)),
        }
    }

    fn signing_key_fixture(label: &str) -> SigningKey {
        let seed =
            Seed::from_env_value("flight-updater-manifest-tests").expect("test seed must be valid");
        let factory = Factory::deterministic(seed);
        let keypair = factory.ed25519(label, Ed25519Spec::new());

        SigningKey::from_pkcs8_der(keypair.private_key_pkcs8_der().as_ref())
            .expect("uselesskey should emit valid Ed25519 PKCS#8")
    }

    fn sign_manifest(manifest: &mut UpdateManifest, signing_key: &SigningKey) {
        let msg = manifest.canonical_bytes();
        let sig = signing_key.sign(&msg);
        manifest.signature = hex::encode(sig.to_bytes());
    }

    // -- SemVer -----------------------------------------------------------

    #[test]
    fn semver_parse_valid() {
        let v = SemVer::parse("1.2.3").unwrap();
        assert_eq!(v, SemVer::new(1, 2, 3));
    }

    #[test]
    fn semver_parse_invalid_two_parts() {
        assert!(SemVer::parse("1.2").is_none());
    }

    #[test]
    fn semver_parse_invalid_non_numeric() {
        assert!(SemVer::parse("a.b.c").is_none());
    }

    #[test]
    fn semver_ordering() {
        assert!(SemVer::new(1, 0, 0) < SemVer::new(2, 0, 0));
        assert!(SemVer::new(1, 1, 0) < SemVer::new(1, 2, 0));
        assert!(SemVer::new(1, 2, 3) < SemVer::new(1, 2, 4));
        assert_eq!(
            SemVer::new(1, 0, 0).cmp(&SemVer::new(1, 0, 0)),
            Ordering::Equal
        );
    }

    #[test]
    fn semver_display() {
        assert_eq!(SemVer::new(10, 20, 30).to_string(), "10.20.30");
    }

    // -- Manifest parsing -------------------------------------------------

    #[test]
    fn parse_valid_manifest() {
        let manifest = unsigned_manifest();
        let json = serde_json::to_vec(&manifest).unwrap();
        let parsed = parse(&json).unwrap();
        assert_eq!(parsed.version, manifest.version);
        assert_eq!(parsed.files.len(), 3);
    }

    #[test]
    fn parse_empty_bytes_is_error() {
        assert!(parse(b"").is_err());
    }

    #[test]
    fn parse_invalid_json_is_error() {
        assert!(parse(b"{not json}").is_err());
    }

    #[test]
    fn parse_preserves_min_version_none() {
        let mut m = unsigned_manifest();
        m.min_version = None;
        let json = serde_json::to_vec(&m).unwrap();
        let parsed = parse(&json).unwrap();
        assert!(parsed.min_version.is_none());
    }

    #[test]
    fn parse_preserves_channel() {
        let mut m = unsigned_manifest();
        m.channel = Channel::Canary;
        let json = serde_json::to_vec(&m).unwrap();
        let parsed = parse(&json).unwrap();
        assert_eq!(parsed.channel, Channel::Canary);
    }

    // -- Signature verification -------------------------------------------

    #[test]
    fn verify_signature_valid_keypair() {
        let sk = signing_key_fixture("valid-keypair");
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());

        let mut manifest = unsigned_manifest();
        sign_manifest(&mut manifest, &sk);

        assert!(verify_signature(&manifest, &pk_hex).is_ok());
    }

    #[test]
    fn verify_signature_wrong_key_fails() {
        let sk = signing_key_fixture("wrong-key-source");
        let wrong_sk = signing_key_fixture("wrong-key-target");
        let wrong_pk_hex = hex::encode(wrong_sk.verifying_key().to_bytes());

        let mut manifest = unsigned_manifest();
        sign_manifest(&mut manifest, &sk);

        assert!(verify_signature(&manifest, &wrong_pk_hex).is_err());
    }

    #[test]
    fn verify_signature_tampered_version_fails() {
        let sk = signing_key_fixture("tampered-version");
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());

        let mut manifest = unsigned_manifest();
        sign_manifest(&mut manifest, &sk);

        manifest.version = SemVer::new(9, 9, 9);
        assert!(verify_signature(&manifest, &pk_hex).is_err());
    }

    #[test]
    fn verify_signature_tampered_files_fails() {
        let sk = signing_key_fixture("tampered-files");
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());

        let mut manifest = unsigned_manifest();
        sign_manifest(&mut manifest, &sk);

        manifest.files.push(sample_file_update(FileOperation::Add));
        assert!(verify_signature(&manifest, &pk_hex).is_err());
    }

    #[test]
    fn verify_signature_bad_hex_key() {
        let manifest = unsigned_manifest();
        assert!(verify_signature(&manifest, "not-hex!!").is_err());
    }

    #[test]
    fn verify_signature_short_key() {
        let manifest = unsigned_manifest();
        assert!(verify_signature(&manifest, "aabb").is_err());
    }

    #[test]
    fn verify_signature_empty_signature_field() {
        let sk = signing_key_fixture("empty-signature-field");
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());
        let manifest = unsigned_manifest(); // signature is ""
        assert!(verify_signature(&manifest, &pk_hex).is_err());
    }

    // -- FileUpdate / FileOperation ---------------------------------------

    #[test]
    fn file_operation_serde_roundtrip() {
        for op in [
            FileOperation::Add,
            FileOperation::Modify,
            FileOperation::Remove,
        ] {
            let json = serde_json::to_string(&op).unwrap();
            let back: FileOperation = serde_json::from_str(&json).unwrap();
            assert_eq!(op, back);
        }
    }

    #[test]
    fn file_update_serde_roundtrip() {
        let fu = sample_file_update(FileOperation::Modify);
        let json = serde_json::to_string(&fu).unwrap();
        let back: FileUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(fu, back);
    }

    // -- canonical_bytes determinism --------------------------------------

    #[test]
    fn canonical_bytes_deterministic() {
        let m = unsigned_manifest();
        assert_eq!(m.canonical_bytes(), m.canonical_bytes());
    }

    #[test]
    fn canonical_bytes_excludes_signature() {
        let mut m1 = unsigned_manifest();
        m1.signature = "aaa".into();
        let mut m2 = unsigned_manifest();
        m2.signature = "bbb".into();
        assert_eq!(m1.canonical_bytes(), m2.canonical_bytes());
    }

    // -- ReleaseManifest --------------------------------------------------

    fn sample_artifact(platform: Platform, arch: Architecture) -> PlatformArtifact {
        PlatformArtifact {
            url: format!("https://dl.example.com/{platform}-{arch}.zip"),
            platform,
            architecture: arch,
            sha256: "aa".repeat(32), // 64 hex chars
            size_bytes: 10_000,
        }
    }

    fn valid_release_manifest() -> ReleaseManifest {
        ReleaseManifest {
            version: SemVer::new(2, 0, 0),
            channel: Channel::Stable,
            release_date: "2025-07-10".into(),
            artifacts: vec![
                sample_artifact(Platform::Windows, Architecture::X64),
                sample_artifact(Platform::Linux, Architecture::X64),
            ],
            release_notes: Some("Bug fixes and improvements.".into()),
        }
    }

    #[test]
    fn release_manifest_validate_valid() {
        assert!(valid_release_manifest().validate().is_ok());
    }

    #[test]
    fn release_manifest_validate_empty_release_date() {
        let mut m = valid_release_manifest();
        m.release_date = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn release_manifest_validate_no_artifacts() {
        let mut m = valid_release_manifest();
        m.artifacts.clear();
        assert!(m.validate().is_err());
    }

    #[test]
    fn release_manifest_validate_bad_checksum_length() {
        let mut m = valid_release_manifest();
        m.artifacts[0].sha256 = "abcd".into();
        assert!(m.validate().is_err());
    }

    #[test]
    fn release_manifest_validate_bad_checksum_chars() {
        let mut m = valid_release_manifest();
        m.artifacts[0].sha256 = "zz".repeat(32); // 64 chars but not hex
        assert!(m.validate().is_err());
    }

    #[test]
    fn release_manifest_validate_empty_url() {
        let mut m = valid_release_manifest();
        m.artifacts[0].url = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn release_manifest_without_notes_is_valid() {
        let mut m = valid_release_manifest();
        m.release_notes = None;
        assert!(m.validate().is_ok());
    }

    #[test]
    fn architecture_x64_serde_roundtrip() {
        let arch = Architecture::X64;
        let json = serde_json::to_string(&arch).unwrap();
        assert_eq!(json, "\"x64\"");
        let back: Architecture = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Architecture::X64);
    }

    #[test]
    fn architecture_arm64_serde_roundtrip() {
        let arch = Architecture::Arm64;
        let json = serde_json::to_string(&arch).unwrap();
        assert_eq!(json, "\"arm64\"");
        let back: Architecture = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Architecture::Arm64);
    }

    #[test]
    fn release_manifest_serde_roundtrip() {
        let m = valid_release_manifest();
        let json = serde_json::to_string(&m).unwrap();
        let back: ReleaseManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }
}
