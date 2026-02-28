// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Update and plugin signature verification.
//!
//! Uses SHA-256 digests to verify the integrity of update payloads and plugin
//! binaries before installation. This is a first-layer check; a full PKI chain
//! (e.g. ed25519-dalek) can be layered on top.

use sha2::{Digest, Sha256};
use std::path::Path;

use crate::SecurityError;

/// A hex-encoded SHA-256 digest.
pub type HexDigest = String;

/// Compute the SHA-256 digest of `data` and return it as a lowercase hex string.
pub fn sha256_hex(data: &[u8]) -> HexDigest {
    let hash = Sha256::digest(data);
    hex::encode(hash)
}

/// Verify that `data` matches the expected hex-encoded SHA-256 `digest`.
pub fn verify_digest(data: &[u8], expected_hex: &str) -> crate::Result<()> {
    let actual = sha256_hex(data);
    if actual == expected_hex.to_ascii_lowercase() {
        Ok(())
    } else {
        Err(SecurityError::SignatureVerificationFailed {
            reason: format!(
                "SHA-256 mismatch: expected {expected_hex}, got {actual}"
            ),
        })
    }
}

/// Read a file from disk and verify its SHA-256 digest.
pub fn verify_file_digest(path: &Path, expected_hex: &str) -> crate::Result<()> {
    let data = std::fs::read(path).map_err(|e| SecurityError::SignatureVerificationFailed {
        reason: format!("cannot read {}: {e}", path.display()),
    })?;
    verify_digest(&data, expected_hex)
}

/// Represents a signed payload with its expected digest.
#[derive(Debug, Clone)]
pub struct SignedPayload {
    pub name: String,
    pub version: String,
    pub expected_digest: HexDigest,
}

impl SignedPayload {
    /// Verify that `data` matches this payload's expected digest.
    pub fn verify(&self, data: &[u8]) -> crate::Result<()> {
        verify_digest(data, &self.expected_digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sha256_hex_known_vector() {
        // SHA-256 of empty string
        let digest = sha256_hex(b"");
        assert_eq!(
            digest,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_verify_digest_success() {
        let data = b"hello world";
        let digest = sha256_hex(data);
        assert!(verify_digest(data, &digest).is_ok());
    }

    #[test]
    fn test_verify_digest_mismatch() {
        let data = b"hello world";
        let wrong = "0000000000000000000000000000000000000000000000000000000000000000";
        let err = verify_digest(data, wrong).unwrap_err();
        assert!(format!("{err}").contains("SHA-256 mismatch"));
    }

    #[test]
    fn test_verify_digest_case_insensitive() {
        let data = b"test";
        let digest = sha256_hex(data).to_uppercase();
        assert!(verify_digest(data, &digest).is_ok());
    }

    #[test]
    fn test_verify_file_digest_success() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("update.bin");
        let content = b"update payload v2.0";
        std::fs::write(&file, content).unwrap();

        let digest = sha256_hex(content);
        assert!(verify_file_digest(&file, &digest).is_ok());
    }

    #[test]
    fn test_verify_file_digest_tampered() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("update.bin");
        std::fs::write(&file, b"original").unwrap();

        let expected = sha256_hex(b"original");
        // Tamper with the file
        std::fs::write(&file, b"tampered").unwrap();

        let err = verify_file_digest(&file, &expected).unwrap_err();
        assert!(format!("{err}").contains("SHA-256 mismatch"));
    }

    #[test]
    fn test_verify_file_digest_missing_file() {
        let err = verify_file_digest(Path::new("nonexistent_update.bin"), "abc123").unwrap_err();
        assert!(format!("{err}").contains("cannot read"));
    }

    #[test]
    fn test_signed_payload_verify() {
        let data = b"plugin binary content";
        let payload = SignedPayload {
            name: "my-plugin".to_string(),
            version: "1.0.0".to_string(),
            expected_digest: sha256_hex(data),
        };
        assert!(payload.verify(data).is_ok());
        assert!(payload.verify(b"different content").is_err());
    }
}
