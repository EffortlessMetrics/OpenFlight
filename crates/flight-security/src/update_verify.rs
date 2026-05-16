// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Update verification — Ed25519 signatures and SHA-256 checksums (REQ-933).
//!
//! Provides functions to verify update manifests using Ed25519 public-key
//! signatures and SHA-256 content checksums, with an optional certificate
//! pinning mechanism via a hard-coded public key.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use crate::update_signature::{sha256_hex, verify_digest};

/// Verify an Ed25519 signature over `manifest` bytes.
///
/// Returns `true` when the signature is valid for the given public key.
pub fn verify_update_signature(manifest: &[u8], signature: &[u8], pubkey: &[u8]) -> bool {
    let pubkey_bytes: &[u8; 32] = match pubkey.try_into() {
        Ok(b) => b,
        Err(_) => return false,
    };
    let Ok(vk) = VerifyingKey::from_bytes(pubkey_bytes) else {
        return false;
    };
    let Ok(sig) = Signature::from_slice(signature) else {
        return false;
    };
    vk.verify(manifest, &sig).is_ok()
}

/// Compute the SHA-256 checksum of `data` and return it as a hex string.
///
/// This is a thin wrapper around [`crate::update_signature::sha256_hex`].
pub fn sha256_checksum(data: &[u8]) -> String {
    sha256_hex(data)
}

/// Verify that `data` matches the expected hex-encoded SHA-256 checksum.
///
/// This is a thin wrapper around [`crate::update_signature::verify_digest`].
pub fn verify_checksum(data: &[u8], expected_hex: &str) -> crate::Result<()> {
    verify_digest(data, expected_hex)
}

/// Hard-coded pinned public key for the OpenFlight update channel.
///
/// In production this would be a real Ed25519 public key compiled into the
/// binary. For now we use a placeholder 32-byte value.
pub const PINNED_PUBLIC_KEY: [u8; 32] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
    0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20,
];

/// Convenience: verify a manifest+signature against the pinned key.
pub fn verify_with_pinned_key(manifest: &[u8], signature: &[u8]) -> bool {
    verify_update_signature(manifest, signature, &PINNED_PUBLIC_KEY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey, pkcs8::DecodePrivateKey};
    use uselesskey::{Ed25519FactoryExt, Ed25519Spec, Factory, Seed};

    /// Build a deterministic Ed25519 fixture without committing test keys.
    fn signing_key_fixture(label: &str) -> SigningKey {
        let seed =
            Seed::from_env_value("flight-security-update-verify").expect("seed must be valid");
        let factory = Factory::deterministic(seed);
        let keypair = factory.ed25519(label, Ed25519Spec::new());

        SigningKey::from_pkcs8_der(keypair.private_key_pkcs8_der().as_ref())
            .expect("uselesskey should emit valid Ed25519 PKCS#8")
    }

    /// Helper: deterministically derive an Ed25519 keypair and sign `message`.
    fn sign_message(label: &str, message: &[u8]) -> (SigningKey, Vec<u8>, Vec<u8>) {
        let sk = signing_key_fixture(label);
        let sig = sk.sign(message);
        let pubkey = sk.verifying_key().to_bytes().to_vec();
        (sk, sig.to_bytes().to_vec(), pubkey)
    }

    // --- Ed25519 signature verification ---

    #[test]
    fn test_valid_signature() {
        let manifest = b"OpenFlight update manifest v4.0";
        let (_sk, sig, pubkey) = sign_message("valid-signature", manifest);
        assert!(verify_update_signature(manifest, &sig, &pubkey));
    }

    #[test]
    fn test_tampered_manifest_fails() {
        let manifest = b"original manifest";
        let (_sk, sig, pubkey) = sign_message("tampered-manifest", manifest);
        assert!(!verify_update_signature(
            b"tampered manifest",
            &sig,
            &pubkey
        ));
    }

    #[test]
    fn test_wrong_pubkey_fails() {
        let manifest = b"manifest";
        let (_sk, sig, _pubkey) = sign_message("wrong-pubkey-source", manifest);
        let (_sk2, _sig2, other_pubkey) = sign_message("wrong-pubkey-target", b"other");
        assert!(!verify_update_signature(manifest, &sig, &other_pubkey));
    }

    #[test]
    fn test_truncated_signature_fails() {
        let manifest = b"manifest";
        let (_sk, sig, pubkey) = sign_message("truncated-signature", manifest);
        assert!(!verify_update_signature(manifest, &sig[..32], &pubkey));
    }

    #[test]
    fn test_empty_signature_fails() {
        let manifest = b"manifest";
        let (_sk, _sig, pubkey) = sign_message("empty-signature", manifest);
        assert!(!verify_update_signature(manifest, &[], &pubkey));
    }

    #[test]
    fn test_empty_pubkey_fails() {
        let manifest = b"manifest";
        let (_sk, sig, _pubkey) = sign_message("empty-pubkey", manifest);
        assert!(!verify_update_signature(manifest, &sig, &[]));
    }

    #[test]
    fn test_all_zeros_pubkey_fails() {
        let manifest = b"manifest";
        let (_sk, sig, _pubkey) = sign_message("zeros-pubkey", manifest);
        assert!(!verify_update_signature(manifest, &sig, &[0u8; 32]));
    }

    // --- SHA-256 checksum ---

    #[test]
    fn test_checksum_known_vector() {
        let checksum = sha256_checksum(b"");
        assert_eq!(
            checksum,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_verify_checksum_valid() {
        let data = b"update payload";
        let hex = sha256_checksum(data);
        assert!(verify_checksum(data, &hex).is_ok());
    }

    #[test]
    fn test_verify_checksum_case_insensitive() {
        let data = b"test";
        let hex = sha256_checksum(data).to_uppercase();
        assert!(verify_checksum(data, &hex).is_ok());
    }

    #[test]
    fn test_verify_checksum_mismatch() {
        let err = verify_checksum(
            b"data",
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap_err();
        assert!(format!("{err}").contains("SHA-256 mismatch"));
    }

    // --- Pinned key ---

    #[test]
    fn test_pinned_key_rejects_invalid_sig() {
        // The placeholder pinned key won't verify a random signature.
        assert!(!verify_with_pinned_key(b"manifest", &[0u8; 64]));
    }

    #[test]
    fn test_pinned_key_constant_length() {
        assert_eq!(PINNED_PUBLIC_KEY.len(), 32);
    }
}
