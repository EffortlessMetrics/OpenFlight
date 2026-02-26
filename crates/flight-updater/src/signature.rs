// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Digital signature verification for updates

use anyhow::anyhow;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Update signature containing the signature and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSignature {
    /// Ed25519 signature bytes (hex encoded)
    pub signature: String,
    /// SHA256 hash of the signed content (hex encoded)
    pub content_hash: String,
    /// Signature algorithm version
    pub algorithm: String,
    /// Timestamp when signature was created
    pub timestamp: u64,
    /// Signer identity
    pub signer: String,
}

impl UpdateSignature {
    /// Create a new update signature
    pub fn new(signature: String, content_hash: String, signer: String) -> Self {
        Self {
            signature,
            content_hash,
            algorithm: "Ed25519".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signer,
        }
    }
}

/// Signature verifier for update packages
#[derive(Debug)]
pub struct SignatureVerifier {
    verifying_key: VerifyingKey,
}

impl SignatureVerifier {
    /// Helper function to verify signature with proper length checks
    pub fn verify_signature_bytes(
        verifying_key: &VerifyingKey,
        message: &[u8],
        signature_bytes: &[u8],
    ) -> crate::Result<bool> {
        // Convert Vec<u8> to [u8; 64] with proper error handling
        let sig_array: [u8; 64] = signature_bytes.try_into().map_err(|_| {
            crate::UpdateError::InvalidSignature(anyhow!("Invalid signature length").to_string())
        })?;

        // Handle Signature::from_bytes conversion
        let signature = Signature::from_bytes(&sig_array);

        // Verify signature
        match verifying_key.verify(message, &signature) {
            Ok(()) => Ok(true),
            Err(e) => {
                tracing::warn!("Signature verification failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Helper function to create VerifyingKey from bytes with proper length checks
    pub fn verifying_key_from_bytes(key_bytes: &[u8]) -> crate::Result<VerifyingKey> {
        // Convert Vec<u8> to [u8; 32] with proper error handling
        let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| {
            crate::UpdateError::InvalidSignature(anyhow!("Invalid public key length").to_string())
        })?;

        // Handle VerifyingKey::from_bytes conversion
        VerifyingKey::from_bytes(&key_array)
            .map_err(|e| crate::UpdateError::InvalidSignature(format!("Invalid public key: {}", e)))
    }

    /// Create a new signature verifier with the given public key
    pub fn new(public_key_hex: &str) -> crate::Result<Self> {
        let key_bytes = hex::decode(public_key_hex).map_err(|e| {
            crate::UpdateError::InvalidSignature(format!("Invalid public key hex: {}", e))
        })?;

        let verifying_key = Self::verifying_key_from_bytes(&key_bytes)?;

        Ok(Self { verifying_key })
    }

    /// Verify a signature against file content
    pub async fn verify_file(
        &self,
        file_path: &Path,
        signature: &UpdateSignature,
    ) -> crate::Result<bool> {
        // Read and hash the file
        let content = tokio::fs::read(file_path).await?;
        let content_hash = self.hash_content(&content);

        // Verify content hash matches
        if content_hash != signature.content_hash {
            tracing::warn!(
                "Content hash mismatch: expected {}, got {}",
                signature.content_hash,
                content_hash
            );
            return Ok(false);
        }

        self.verify_content(&content, signature).await
    }

    /// Verify a signature against raw content
    pub async fn verify_content(
        &self,
        content: &[u8],
        signature: &UpdateSignature,
    ) -> crate::Result<bool> {
        // Verify algorithm
        if signature.algorithm != "Ed25519" {
            return Err(crate::UpdateError::InvalidSignature(format!(
                "Unsupported algorithm: {}",
                signature.algorithm
            )));
        }

        // Decode signature
        let sig_bytes = hex::decode(&signature.signature).map_err(|e| {
            crate::UpdateError::InvalidSignature(format!("Invalid signature hex: {}", e))
        })?;

        // Use helper function for verification
        let result = Self::verify_signature_bytes(&self.verifying_key, content, &sig_bytes)?;

        if result {
            tracing::info!(
                "Signature verification successful for signer: {}",
                signature.signer
            );
        }

        Ok(result)
    }

    /// Hash content using SHA256
    pub fn hash_content(&self, content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }

    /// Verify signature timestamp is within acceptable range
    pub fn verify_timestamp(&self, signature: &UpdateSignature, max_age_hours: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let age_seconds = now.saturating_sub(signature.timestamp);
        let max_age_seconds = max_age_hours * 3600;

        if age_seconds > max_age_seconds {
            tracing::warn!(
                "Signature too old: {} hours (max: {} hours)",
                age_seconds / 3600,
                max_age_hours
            );
            return false;
        }

        true
    }
}

/// Signature manifest for update packages
#[derive(Debug, Serialize, Deserialize)]
pub struct SignatureManifest {
    /// Version of the manifest format
    pub version: u32,
    /// Signatures for each file in the update
    pub files: std::collections::HashMap<String, UpdateSignature>,
    /// Overall package signature
    pub package_signature: UpdateSignature,
}

impl SignatureManifest {
    /// Create a new signature manifest
    pub fn new() -> Self {
        Self {
            version: 1,
            files: std::collections::HashMap::new(),
            package_signature: UpdateSignature::new(
                String::new(),
                String::new(),
                "flight-hub-updater".to_string(),
            ),
        }
    }

    /// Add a file signature to the manifest
    pub fn add_file_signature(&mut self, file_path: &str, signature: UpdateSignature) {
        self.files.insert(file_path.to_string(), signature);
    }

    /// Get signature for a specific file
    pub fn get_file_signature(&self, file_path: &str) -> Option<&UpdateSignature> {
        self.files.get(file_path)
    }

    /// Verify all file signatures in the manifest
    pub async fn verify_all_files(
        &self,
        verifier: &SignatureVerifier,
        base_path: &Path,
    ) -> crate::Result<bool> {
        for (file_path, signature) in &self.files {
            let full_path = base_path.join(file_path);

            if !full_path.exists() {
                tracing::error!("File not found: {}", full_path.display());
                return Ok(false);
            }

            if !verifier.verify_file(&full_path, signature).await? {
                tracing::error!("Signature verification failed for: {}", file_path);
                return Ok(false);
            }
        }

        Ok(true)
    }
}

impl Default for SignatureManifest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand_core::OsRng;

    #[tokio::test]
    async fn test_signature_verification() {
        // Generate a test signing key
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        // Create test content
        let content = b"test update content";

        // Sign the content
        let signature_bytes = signing_key.sign(content);
        let signature_hex = hex::encode(signature_bytes.to_bytes());

        // Create verifier
        let public_key_hex = hex::encode(verifying_key.to_bytes());
        let verifier = SignatureVerifier::new(&public_key_hex).unwrap();

        // Create signature
        let content_hash = verifier.hash_content(content);
        let update_sig =
            UpdateSignature::new(signature_hex, content_hash, "test-signer".to_string());

        // Verify signature
        let result = verifier.verify_content(content, &update_sig).await.unwrap();
        assert!(result);
    }

    #[test]
    fn test_signature_timestamp_validation() {
        let verifier = SignatureVerifier::new(&hex::encode([0u8; 32])).unwrap();

        // Current timestamp should be valid
        let current_sig = UpdateSignature::new(String::new(), String::new(), "test".to_string());
        assert!(verifier.verify_timestamp(&current_sig, 24));

        // Old timestamp should be invalid
        let old_sig = UpdateSignature {
            timestamp: 0, // Unix epoch
            ..current_sig
        };
        assert!(!verifier.verify_timestamp(&old_sig, 24));
    }

    #[test]
    fn test_signature_manifest() {
        let mut manifest = SignatureManifest::new();

        let signature = UpdateSignature::new(
            "test_sig".to_string(),
            "test_hash".to_string(),
            "test_signer".to_string(),
        );

        manifest.add_file_signature("test.bin", signature.clone());

        let retrieved = manifest.get_file_signature("test.bin").unwrap();
        assert_eq!(retrieved.signature, signature.signature);
        assert_eq!(retrieved.content_hash, signature.content_hash);
    }

    /// Flipping one byte of the content must cause verify_content to return false
    /// (the Ed25519 signature no longer matches the modified message).
    #[tokio::test]
    async fn test_tampered_content_fails_verification() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let content = b"authentic update payload";
        let signature_bytes = signing_key.sign(content);
        let signature_hex = hex::encode(signature_bytes.to_bytes());

        let public_key_hex = hex::encode(verifying_key.to_bytes());
        let verifier = SignatureVerifier::new(&public_key_hex).unwrap();

        let content_hash = verifier.hash_content(content);
        let update_sig = UpdateSignature::new(signature_hex, content_hash, "test".to_string());

        let mut tampered = content.to_vec();
        tampered[0] ^= 0xFF; // flip one byte

        let result = verifier
            .verify_content(&tampered, &update_sig)
            .await
            .unwrap();
        assert!(!result, "tampered content must not verify");
    }

    /// Flipping one byte of the signature itself must also fail verification.
    #[tokio::test]
    async fn test_tampered_signature_bytes_fails_verification() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let content = b"authentic update payload";
        let mut sig_bytes = signing_key.sign(content).to_bytes();
        sig_bytes[0] ^= 0xFF; // corrupt the first byte
        let tampered_sig_hex = hex::encode(sig_bytes);

        let public_key_hex = hex::encode(verifying_key.to_bytes());
        let verifier = SignatureVerifier::new(&public_key_hex).unwrap();

        let content_hash = verifier.hash_content(content);
        let update_sig = UpdateSignature::new(tampered_sig_hex, content_hash, "test".to_string());

        let result = verifier.verify_content(content, &update_sig).await.unwrap();
        assert!(!result, "corrupted signature must not verify");
    }

    /// verify_content with an empty payload must succeed when the signature is valid
    /// (i.e. it must not panic or return an unexpected error).
    #[tokio::test]
    async fn test_empty_payload_verifies_gracefully() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let signature_bytes = signing_key.sign(b"");
        let signature_hex = hex::encode(signature_bytes.to_bytes());

        let public_key_hex = hex::encode(verifying_key.to_bytes());
        let verifier = SignatureVerifier::new(&public_key_hex).unwrap();

        let content_hash = verifier.hash_content(b"");
        let update_sig = UpdateSignature::new(signature_hex, content_hash, "test".to_string());

        let result = verifier.verify_content(b"", &update_sig).await.unwrap();
        assert!(result, "valid signature for empty payload must verify");
    }

    /// A signature produced by a *different* key must not verify against an empty payload.
    #[tokio::test]
    async fn test_wrong_key_rejects_empty_payload() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let wrong_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let signature_bytes = wrong_key.sign(b"");
        let signature_hex = hex::encode(signature_bytes.to_bytes());

        let public_key_hex = hex::encode(verifying_key.to_bytes());
        let verifier = SignatureVerifier::new(&public_key_hex).unwrap();

        let content_hash = verifier.hash_content(b"");
        let update_sig = UpdateSignature::new(signature_hex, content_hash, "test".to_string());

        let result = verifier.verify_content(b"", &update_sig).await.unwrap();
        assert!(!result, "signature from wrong key must not verify");
    }

    /// An unsupported algorithm field must cause verify_content to return an Err.
    #[tokio::test]
    async fn test_unsupported_algorithm_returns_error() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let public_key_hex = hex::encode(verifying_key.to_bytes());
        let verifier = SignatureVerifier::new(&public_key_hex).unwrap();

        let content = b"some content";
        let sig_bytes = signing_key.sign(content);
        let mut update_sig = UpdateSignature::new(
            hex::encode(sig_bytes.to_bytes()),
            verifier.hash_content(content),
            "test".to_string(),
        );
        update_sig.algorithm = "RSA".to_string();

        let result = verifier.verify_content(content, &update_sig).await;
        assert!(
            result.is_err(),
            "unsupported algorithm must return an error"
        );
    }

    /// Passing non-hex characters as the signature must return an Err rather than panic.
    #[tokio::test]
    async fn test_invalid_hex_signature_returns_error() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let public_key_hex = hex::encode(verifying_key.to_bytes());
        let verifier = SignatureVerifier::new(&public_key_hex).unwrap();

        let content = b"some content";
        let update_sig = UpdateSignature::new(
            "not-valid-hex!!!".to_string(),
            verifier.hash_content(content),
            "test".to_string(),
        );

        let result = verifier.verify_content(content, &update_sig).await;
        assert!(
            result.is_err(),
            "invalid hex signature must return an error"
        );
    }
}
