// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Artifact verification and trust policy (REQ-930).
//!
//! Provides checksum-based integrity verification for update payloads and plugin
//! binaries, manifest structure validation, and configurable trust policies that
//! control which artifacts are accepted.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::SecurityError;

/// Result of a checksum verification operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChecksumStatus {
    /// Checksum matched the expected value.
    Verified,
    /// Checksum did not match.
    Mismatch { expected: String, actual: String },
    /// An I/O error prevented reading the file.
    IoError(String),
    /// The target file was not found.
    FileNotFound,
}

/// Outcome of a checksum verification, including the status and the path checked.
#[derive(Debug, Clone)]
pub struct ChecksumResult {
    pub path: String,
    pub status: ChecksumStatus,
}

/// Artifact integrity verifier.
///
/// Verifies file checksums and validates manifest structure before accepting
/// artifacts into the system.
pub struct ArtifactVerifier;

impl ArtifactVerifier {
    /// Verify the SHA-256 checksum of a file on disk.
    ///
    /// Returns `Ok(true)` when the checksum matches, `Ok(false)` when it does
    /// not, and `Err` on I/O failure or missing file.
    pub fn verify_checksum(path: &Path, expected_sha256: &str) -> crate::Result<bool> {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SecurityError::SignatureVerificationFailed {
                    reason: format!("file not found: {}", path.display()),
                });
            }
            Err(e) => {
                return Err(SecurityError::SignatureVerificationFailed {
                    reason: format!("cannot read {}: {e}", path.display()),
                });
            }
        };

        let hash = Sha256::digest(&data);
        let actual = hex::encode(hash);
        Ok(actual == expected_sha256.to_ascii_lowercase())
    }

    /// Return a detailed [`ChecksumResult`] for a file.
    pub fn verify_checksum_detailed(path: &Path, expected_sha256: &str) -> ChecksumResult {
        let path_str = path.display().to_string();
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return ChecksumResult {
                    path: path_str,
                    status: ChecksumStatus::FileNotFound,
                };
            }
            Err(e) => {
                return ChecksumResult {
                    path: path_str,
                    status: ChecksumStatus::IoError(e.to_string()),
                };
            }
        };

        let hash = Sha256::digest(&data);
        let actual = hex::encode(hash);
        let expected_lower = expected_sha256.to_ascii_lowercase();

        let status = if actual == expected_lower {
            ChecksumStatus::Verified
        } else {
            ChecksumStatus::Mismatch {
                expected: expected_lower,
                actual,
            }
        };

        ChecksumResult {
            path: path_str,
            status,
        }
    }

    /// Validate the *structure* of a signed manifest.
    ///
    /// This checks that:
    /// - `manifest_bytes` is valid JSON containing at least `name` and `version`.
    /// - `signature_bytes` is non-empty and looks like hex or base64.
    /// - `public_key` is non-empty and at least 32 bytes (minimum key material).
    ///
    /// **No actual cryptographic verification is performed** — this is a
    /// structural pre-check before handing off to a real PKI layer.
    pub fn verify_manifest(
        manifest_bytes: &[u8],
        signature_bytes: &[u8],
        public_key: &[u8],
    ) -> crate::Result<bool> {
        // --- manifest structure ---
        let value: serde_json::Value =
            serde_json::from_slice(manifest_bytes).map_err(|e| SecurityError::InvalidManifest {
                reason: format!("manifest is not valid JSON: {e}"),
            })?;

        let obj = value
            .as_object()
            .ok_or_else(|| SecurityError::InvalidManifest {
                reason: "manifest root must be a JSON object".to_string(),
            })?;

        if !obj.contains_key("name") {
            return Err(SecurityError::InvalidManifest {
                reason: "manifest missing required field 'name'".to_string(),
            });
        }
        if !obj.contains_key("version") {
            return Err(SecurityError::InvalidManifest {
                reason: "manifest missing required field 'version'".to_string(),
            });
        }

        // --- signature format ---
        if signature_bytes.is_empty() {
            return Err(SecurityError::SignatureVerificationFailed {
                reason: "signature is empty".to_string(),
            });
        }
        if !is_hex_or_base64(signature_bytes) {
            return Err(SecurityError::SignatureVerificationFailed {
                reason: "signature is not valid hex or base64".to_string(),
            });
        }

        // --- public key format ---
        if public_key.is_empty() {
            return Err(SecurityError::SignatureVerificationFailed {
                reason: "public key is empty".to_string(),
            });
        }
        if public_key.len() < 32 {
            return Err(SecurityError::SignatureVerificationFailed {
                reason: format!(
                    "public key too short ({} bytes, minimum 32)",
                    public_key.len()
                ),
            });
        }

        // Structure looks valid — actual crypto would happen here.
        Ok(true)
    }
}

/// Check whether `bytes` (interpreted as ASCII) is plausible hex or base64.
fn is_hex_or_base64(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let s = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Hex: all chars in [0-9a-fA-F]
    let is_hex = trimmed.chars().all(|c| c.is_ascii_hexdigit());
    // Base64: chars in [A-Za-z0-9+/=]
    let is_b64 = trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=');
    is_hex || is_b64
}

// ---------------------------------------------------------------------------
// Trust policy
// ---------------------------------------------------------------------------

/// Release channel from which artifacts may originate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReleaseChannel {
    Stable,
    Beta,
    Nightly,
    Custom(String),
}

/// Configurable policy that gates artifact acceptance.
#[derive(Debug, Clone)]
pub struct TrustPolicy {
    /// Require a cryptographic signature on every artifact.
    pub require_signature: bool,
    /// Require a SHA-256 checksum for every artifact.
    pub require_checksum: bool,
    /// Set of release channels from which artifacts are accepted.
    pub allowed_channels: Vec<ReleaseChannel>,
}

impl Default for TrustPolicy {
    fn default() -> Self {
        Self {
            require_signature: true,
            require_checksum: true,
            allowed_channels: vec![ReleaseChannel::Stable],
        }
    }
}

impl TrustPolicy {
    /// Check whether `channel` is allowed by this policy.
    pub fn is_channel_allowed(&self, channel: &ReleaseChannel) -> bool {
        self.allowed_channels.contains(channel)
    }

    /// Evaluate whether an artifact satisfies this policy.
    ///
    /// `has_signature` and `has_checksum` indicate the presence of the
    /// respective verification material. `channel` is the release channel the
    /// artifact claims to originate from.
    pub fn evaluate(
        &self,
        has_signature: bool,
        has_checksum: bool,
        channel: &ReleaseChannel,
    ) -> crate::Result<()> {
        if self.require_signature && !has_signature {
            return Err(SecurityError::PolicyViolation {
                reason: "artifact is missing a required signature".to_string(),
            });
        }
        if self.require_checksum && !has_checksum {
            return Err(SecurityError::PolicyViolation {
                reason: "artifact is missing a required checksum".to_string(),
            });
        }
        if !self.is_channel_allowed(channel) {
            return Err(SecurityError::PolicyViolation {
                reason: format!("release channel {channel:?} is not in the allowed list"),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- Checksum verification ---

    #[test]
    fn test_verify_checksum_matching() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("payload.bin");
        let content = b"hello checksum";
        std::fs::write(&file, content).unwrap();

        let expected = hex::encode(Sha256::digest(content));
        assert!(ArtifactVerifier::verify_checksum(&file, &expected).unwrap());
    }

    #[test]
    fn test_verify_checksum_mismatch() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("payload.bin");
        std::fs::write(&file, b"original").unwrap();

        let wrong = "0".repeat(64);
        assert!(!ArtifactVerifier::verify_checksum(&file, &wrong).unwrap());
    }

    #[test]
    fn test_verify_checksum_file_not_found() {
        let err = ArtifactVerifier::verify_checksum(Path::new("nonexistent_file.bin"), "abc")
            .unwrap_err();
        assert!(format!("{err}").contains("file not found"));
    }

    #[test]
    fn test_verify_checksum_case_insensitive() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("payload.bin");
        std::fs::write(&file, b"data").unwrap();
        let expected = hex::encode(Sha256::digest(b"data")).to_uppercase();
        assert!(ArtifactVerifier::verify_checksum(&file, &expected).unwrap());
    }

    #[test]
    fn test_verify_checksum_detailed_verified() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("ok.bin");
        std::fs::write(&file, b"ok").unwrap();
        let expected = hex::encode(Sha256::digest(b"ok"));
        let result = ArtifactVerifier::verify_checksum_detailed(&file, &expected);
        assert_eq!(result.status, ChecksumStatus::Verified);
    }

    #[test]
    fn test_verify_checksum_detailed_mismatch() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("bad.bin");
        std::fs::write(&file, b"bad").unwrap();
        let result = ArtifactVerifier::verify_checksum_detailed(&file, &"0".repeat(64));
        assert!(matches!(result.status, ChecksumStatus::Mismatch { .. }));
    }

    #[test]
    fn test_verify_checksum_detailed_not_found() {
        let result = ArtifactVerifier::verify_checksum_detailed(Path::new("missing.bin"), "abc");
        assert_eq!(result.status, ChecksumStatus::FileNotFound);
    }

    // --- Manifest structure validation ---

    #[test]
    fn test_verify_manifest_valid() {
        let manifest = br#"{"name": "my-plugin", "version": "1.0.0"}"#;
        let sig = b"aabbccdd";
        let key = &[0u8; 32];
        assert!(ArtifactVerifier::verify_manifest(manifest, sig, key).unwrap());
    }

    #[test]
    fn test_verify_manifest_invalid_json() {
        let err = ArtifactVerifier::verify_manifest(b"not json", b"aabb", &[0u8; 32]).unwrap_err();
        assert!(format!("{err}").contains("not valid JSON"));
    }

    #[test]
    fn test_verify_manifest_missing_name() {
        let manifest = br#"{"version": "1.0.0"}"#;
        let err = ArtifactVerifier::verify_manifest(manifest, b"aabb", &[0u8; 32]).unwrap_err();
        assert!(format!("{err}").contains("missing required field 'name'"));
    }

    #[test]
    fn test_verify_manifest_missing_version() {
        let manifest = br#"{"name": "plugin"}"#;
        let err = ArtifactVerifier::verify_manifest(manifest, b"aabb", &[0u8; 32]).unwrap_err();
        assert!(format!("{err}").contains("missing required field 'version'"));
    }

    #[test]
    fn test_verify_manifest_empty_signature() {
        let manifest = br#"{"name": "p", "version": "1"}"#;
        let err = ArtifactVerifier::verify_manifest(manifest, b"", &[0u8; 32]).unwrap_err();
        assert!(format!("{err}").contains("signature is empty"));
    }

    #[test]
    fn test_verify_manifest_invalid_signature_format() {
        let manifest = br#"{"name": "p", "version": "1"}"#;
        // Non-hex, non-base64 characters
        let err = ArtifactVerifier::verify_manifest(manifest, "!!!???".as_bytes(), &[0u8; 32])
            .unwrap_err();
        assert!(format!("{err}").contains("not valid hex or base64"));
    }

    #[test]
    fn test_verify_manifest_empty_public_key() {
        let manifest = br#"{"name": "p", "version": "1"}"#;
        let err = ArtifactVerifier::verify_manifest(manifest, b"aabb", &[]).unwrap_err();
        assert!(format!("{err}").contains("public key is empty"));
    }

    #[test]
    fn test_verify_manifest_short_public_key() {
        let manifest = br#"{"name": "p", "version": "1"}"#;
        let err = ArtifactVerifier::verify_manifest(manifest, b"aabb", &[0u8; 16]).unwrap_err();
        assert!(format!("{err}").contains("public key too short"));
    }

    #[test]
    fn test_verify_manifest_base64_signature() {
        let manifest = br#"{"name": "p", "version": "1"}"#;
        let sig = b"SGVsbG8gV29ybGQ="; // "Hello World" in base64
        assert!(ArtifactVerifier::verify_manifest(manifest, sig, &[0u8; 32]).unwrap());
    }

    #[test]
    fn test_verify_manifest_not_object() {
        let manifest = br#"[1, 2, 3]"#;
        let err = ArtifactVerifier::verify_manifest(manifest, b"aabb", &[0u8; 32]).unwrap_err();
        assert!(format!("{err}").contains("root must be a JSON object"));
    }

    // --- Trust policy ---

    #[test]
    fn test_trust_policy_default() {
        let policy = TrustPolicy::default();
        assert!(policy.require_signature);
        assert!(policy.require_checksum);
        assert!(policy.is_channel_allowed(&ReleaseChannel::Stable));
        assert!(!policy.is_channel_allowed(&ReleaseChannel::Nightly));
    }

    #[test]
    fn test_trust_policy_evaluate_all_satisfied() {
        let policy = TrustPolicy::default();
        assert!(policy.evaluate(true, true, &ReleaseChannel::Stable).is_ok());
    }

    #[test]
    fn test_trust_policy_evaluate_missing_signature() {
        let policy = TrustPolicy::default();
        let err = policy
            .evaluate(false, true, &ReleaseChannel::Stable)
            .unwrap_err();
        assert!(format!("{err}").contains("missing a required signature"));
    }

    #[test]
    fn test_trust_policy_evaluate_missing_checksum() {
        let policy = TrustPolicy::default();
        let err = policy
            .evaluate(true, false, &ReleaseChannel::Stable)
            .unwrap_err();
        assert!(format!("{err}").contains("missing a required checksum"));
    }

    #[test]
    fn test_trust_policy_evaluate_disallowed_channel() {
        let policy = TrustPolicy::default();
        let err = policy
            .evaluate(true, true, &ReleaseChannel::Nightly)
            .unwrap_err();
        assert!(format!("{err}").contains("not in the allowed list"));
    }

    #[test]
    fn test_trust_policy_lenient() {
        let policy = TrustPolicy {
            require_signature: false,
            require_checksum: false,
            allowed_channels: vec![
                ReleaseChannel::Stable,
                ReleaseChannel::Beta,
                ReleaseChannel::Nightly,
            ],
        };
        assert!(
            policy
                .evaluate(false, false, &ReleaseChannel::Nightly)
                .is_ok()
        );
    }

    #[test]
    fn test_trust_policy_custom_channel() {
        let policy = TrustPolicy {
            require_signature: false,
            require_checksum: false,
            allowed_channels: vec![ReleaseChannel::Custom("internal".to_string())],
        };
        assert!(
            policy
                .evaluate(
                    false,
                    false,
                    &ReleaseChannel::Custom("internal".to_string())
                )
                .is_ok()
        );
        assert!(
            policy
                .evaluate(false, false, &ReleaseChannel::Stable)
                .is_err()
        );
    }
}
