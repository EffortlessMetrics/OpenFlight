//! Digital signature verification for updates

use ed25519_dalek::{PublicKey, Signature, Verifier};
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
    pub fn new(
        signature: String,
        content_hash: String,
        signer: String,
    ) -> Self {
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
    public_key: PublicKey,
}

impl SignatureVerifier {
    /// Create a new signature verifier with the given public key
    pub fn new(public_key_hex: &str) -> crate::Result<Self> {
        let key_bytes = hex::decode(public_key_hex)
            .map_err(|e| crate::UpdateError::InvalidSignature(
                format!("Invalid public key hex: {}", e)
            ))?;
        
        let public_key = PublicKey::from_bytes(&key_bytes)
            .map_err(|e| crate::UpdateError::InvalidSignature(
                format!("Invalid public key: {}", e)
            ))?;
        
        Ok(Self { public_key })
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
            return Err(crate::UpdateError::InvalidSignature(
                format!("Unsupported algorithm: {}", signature.algorithm)
            ));
        }
        
        // Decode signature
        let sig_bytes = hex::decode(&signature.signature)
            .map_err(|e| crate::UpdateError::InvalidSignature(
                format!("Invalid signature hex: {}", e)
            ))?;
        
        let sig = Signature::from_bytes(&sig_bytes)
            .map_err(|e| crate::UpdateError::InvalidSignature(
                format!("Invalid signature format: {}", e)
            ))?;
        
        // Verify signature
        match self.public_key.verify(content, &sig) {
            Ok(()) => {
                tracing::info!("Signature verification successful for signer: {}", signature.signer);
                Ok(true)
            }
            Err(e) => {
                tracing::warn!("Signature verification failed: {}", e);
                Ok(false)
            }
        }
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
    use ed25519_dalek::{Keypair, Signer};
    use rand::rngs::OsRng;

    #[tokio::test]
    async fn test_signature_verification() {
        // Generate a test keypair
        let mut csprng = OsRng {};
        let keypair = Keypair::generate(&mut csprng);
        
        // Create test content
        let content = b"test update content";
        
        // Sign the content
        let signature_bytes = keypair.sign(content);
        let signature_hex = hex::encode(signature_bytes.to_bytes());
        
        // Create verifier
        let public_key_hex = hex::encode(keypair.public.to_bytes());
        let verifier = SignatureVerifier::new(&public_key_hex).unwrap();
        
        // Create signature
        let content_hash = verifier.hash_content(content);
        let update_sig = UpdateSignature::new(
            signature_hex,
            content_hash,
            "test-signer".to_string(),
        );
        
        // Verify signature
        let result = verifier.verify_content(content, &update_sig).await.unwrap();
        assert!(result);
    }

    #[test]
    fn test_signature_timestamp_validation() {
        let verifier = SignatureVerifier::new(&hex::encode([0u8; 32])).unwrap();
        
        // Current timestamp should be valid
        let current_sig = UpdateSignature::new(
            String::new(),
            String::new(),
            "test".to_string(),
        );
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
}