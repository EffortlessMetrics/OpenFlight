//! Artifact trust tests.
//!
//! Validates checksum generation/verification, Ed25519 signing/verification,
//! tamper detection, and expired-signature handling — all using in-process
//! mocks with no real network or PKI.

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::OsRng;
use sha2::{Digest, Sha256};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Compute the SHA-256 hex digest of `data`.
fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hex::encode(hash)
}

/// Generate a fresh Ed25519 keypair.
fn generate_keypair() -> (SigningKey, VerifyingKey) {
    let sk = SigningKey::generate(&mut OsRng);
    let vk = sk.verifying_key();
    (sk, vk)
}

/// Sign `message` and return the hex-encoded signature.
fn sign_hex(sk: &SigningKey, message: &[u8]) -> String {
    let sig = sk.sign(message);
    hex::encode(sig.to_bytes())
}

/// Verify a hex-encoded signature against `message`.
fn verify_hex(vk: &VerifyingKey, message: &[u8], sig_hex: &str) -> bool {
    let sig_bytes = hex::decode(sig_hex).expect("invalid hex");
    let sig = ed25519_dalek::Signature::from_bytes(
        sig_bytes.as_slice().try_into().expect("wrong sig length"),
    );
    vk.verify(message, &sig).is_ok()
}

// Simulate a simple expiry model: signature payload includes a "not_after"
// unix timestamp. Verification rejects if current time exceeds it.

#[derive(Debug)]
struct TimestampedSignature {
    sig_hex: String,
    not_after: u64,
}

fn sign_with_expiry(sk: &SigningKey, message: &[u8], not_after: u64) -> TimestampedSignature {
    // Signature covers message + expiry timestamp.
    let mut payload = message.to_vec();
    payload.extend_from_slice(&not_after.to_le_bytes());
    TimestampedSignature {
        sig_hex: sign_hex(sk, &payload),
        not_after,
    }
}

fn verify_with_expiry(
    vk: &VerifyingKey,
    message: &[u8],
    ts: &TimestampedSignature,
    current_time: u64,
) -> Result<(), &'static str> {
    if current_time > ts.not_after {
        return Err("signature expired");
    }
    let mut payload = message.to_vec();
    payload.extend_from_slice(&ts.not_after.to_le_bytes());
    if verify_hex(vk, &payload, &ts.sig_hex) {
        Ok(())
    } else {
        Err("signature invalid")
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Checksum generation + verification
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn checksum_deterministic_for_same_input() {
    let data = b"flight-hub-artifact-v0.2.0";
    let h1 = sha256_hex(data);
    let h2 = sha256_hex(data);
    assert_eq!(h1, h2);
}

#[test]
fn checksum_changes_on_different_input() {
    let h1 = sha256_hex(b"version-a");
    let h2 = sha256_hex(b"version-b");
    assert_ne!(h1, h2);
}

#[test]
fn checksum_is_64_hex_chars() {
    let h = sha256_hex(b"test");
    assert_eq!(h.len(), 64);
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn checksum_verification_succeeds_for_correct_data() {
    let data = b"installer-payload";
    let expected = sha256_hex(data);
    assert_eq!(sha256_hex(data), expected);
}

#[test]
fn checksum_verification_fails_for_tampered_data() {
    let original = b"installer-payload";
    let expected = sha256_hex(original);
    let tampered = b"installer-payload-tampered";
    assert_ne!(sha256_hex(tampered), expected);
}

// ═════════════════════════════════════════════════════════════════════════════
// Ed25519 signing + verification
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn ed25519_sign_and_verify_roundtrip() {
    let (sk, vk) = generate_keypair();
    let msg = b"update-manifest-v0.2.0";
    let sig = sign_hex(&sk, msg);
    assert!(verify_hex(&vk, msg, &sig));
}

#[test]
fn ed25519_verification_fails_with_wrong_key() {
    let (sk, _vk) = generate_keypair();
    let (_sk2, vk2) = generate_keypair();
    let msg = b"update-manifest";
    let sig = sign_hex(&sk, msg);
    assert!(!verify_hex(&vk2, msg, &sig));
}

#[test]
fn ed25519_signature_is_128_hex_chars() {
    let (sk, _vk) = generate_keypair();
    let sig = sign_hex(&sk, b"data");
    assert_eq!(sig.len(), 128, "ed25519 sig must be 64 bytes = 128 hex");
}

// ═════════════════════════════════════════════════════════════════════════════
// Tampered artifact detection
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn tampered_artifact_detected_by_checksum() {
    let original = b"flight-hub-0.2.0.tar.gz-contents";
    let checksum = sha256_hex(original);

    let mut tampered = original.to_vec();
    tampered[0] ^= 0xFF;
    assert_ne!(sha256_hex(&tampered), checksum);
}

#[test]
fn tampered_artifact_detected_by_signature() {
    let (sk, vk) = generate_keypair();
    let original = b"flight-hub-0.2.0.tar.gz-contents";
    let sig = sign_hex(&sk, original);

    let mut tampered = original.to_vec();
    tampered.push(0x42);
    assert!(!verify_hex(&vk, &tampered, &sig));
}

#[test]
fn single_bit_flip_detected() {
    let (sk, vk) = generate_keypair();
    let data = b"artifact-data-here";
    let sig = sign_hex(&sk, data);

    let mut flipped = data.to_vec();
    flipped[5] ^= 0x01;
    assert!(!verify_hex(&vk, &flipped, &sig));
    assert_ne!(sha256_hex(&flipped), sha256_hex(data));
}

#[test]
fn empty_artifact_has_valid_checksum() {
    let h = sha256_hex(b"");
    assert_eq!(h.len(), 64);
    // SHA-256 of empty input is a well-known constant.
    assert_eq!(
        h,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// Expired signature handling
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn valid_signature_within_expiry_accepted() {
    let (sk, vk) = generate_keypair();
    let msg = b"update-manifest-payload";
    let ts = sign_with_expiry(&sk, msg, 2_000_000_000);
    assert!(verify_with_expiry(&vk, msg, &ts, 1_700_000_000).is_ok());
}

#[test]
fn expired_signature_rejected() {
    let (sk, vk) = generate_keypair();
    let msg = b"update-manifest-payload";
    let ts = sign_with_expiry(&sk, msg, 1_600_000_000);
    let result = verify_with_expiry(&vk, msg, &ts, 1_700_000_000);
    assert_eq!(result, Err("signature expired"));
}

#[test]
fn signature_at_exact_expiry_boundary_accepted() {
    let (sk, vk) = generate_keypair();
    let msg = b"boundary-test";
    let ts = sign_with_expiry(&sk, msg, 1_700_000_000);
    // current_time == not_after → should be accepted (not > not_after)
    assert!(verify_with_expiry(&vk, msg, &ts, 1_700_000_000).is_ok());
}

#[test]
fn expired_signature_one_second_past() {
    let (sk, vk) = generate_keypair();
    let msg = b"boundary-test";
    let ts = sign_with_expiry(&sk, msg, 1_700_000_000);
    let result = verify_with_expiry(&vk, msg, &ts, 1_700_000_001);
    assert_eq!(result, Err("signature expired"));
}

#[test]
fn tampered_message_with_valid_expiry_rejected() {
    let (sk, vk) = generate_keypair();
    let msg = b"original-payload";
    let ts = sign_with_expiry(&sk, msg, 2_000_000_000);

    let tampered = b"tampered-payload";
    let result = verify_with_expiry(&vk, tampered, &ts, 1_700_000_000);
    assert_eq!(result, Err("signature invalid"));
}
