// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! IPC authentication via session tokens (REQ-930).
//!
//! Generates 256-bit random session tokens and validates them using
//! constant-time comparison to prevent timing side-channels. Tokens are
//! persisted to `XDG_RUNTIME_DIR` (Linux) or `%TEMP%` (Windows) with
//! restrictive file permissions.

use std::path::{Path, PathBuf};

use rand::RngCore;
use subtle::ConstantTimeEq;

use crate::SecurityError;

/// Length of a session token in bytes (256 bits).
pub const TOKEN_LENGTH: usize = 32;

/// A 256-bit session token.
#[derive(Clone)]
pub struct SessionToken {
    bytes: [u8; TOKEN_LENGTH],
}

impl SessionToken {
    /// Generate a new cryptographically-random session token.
    pub fn generate() -> Self {
        let mut bytes = [0u8; TOKEN_LENGTH];
        rand::rng().fill_bytes(&mut bytes);
        Self { bytes }
    }

    /// Construct from raw bytes.
    pub fn from_bytes(bytes: [u8; TOKEN_LENGTH]) -> Self {
        Self { bytes }
    }

    /// View the raw bytes.
    pub fn as_bytes(&self) -> &[u8; TOKEN_LENGTH] {
        &self.bytes
    }

    /// Constant-time comparison with another token or byte slice.
    pub fn ct_eq(&self, other: &[u8]) -> bool {
        if other.len() != TOKEN_LENGTH {
            return false;
        }
        self.bytes.ct_eq(other).into()
    }
}

impl std::fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionToken([REDACTED])")
    }
}

/// Generate a fresh session token.
pub fn generate_session_token() -> SessionToken {
    SessionToken::generate()
}

/// Validate `candidate` against `expected` using constant-time comparison.
pub fn validate_token(candidate: &[u8], expected: &SessionToken) -> bool {
    expected.ct_eq(candidate)
}

/// Return the platform-appropriate runtime directory for storing tokens.
pub fn token_dir() -> PathBuf {
    #[cfg(unix)]
    {
        if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            return PathBuf::from(dir);
        }
        PathBuf::from("/tmp")
    }
    #[cfg(windows)]
    {
        if let Ok(dir) = std::env::var("TEMP") {
            return PathBuf::from(dir);
        }
        PathBuf::from(r"C:\Temp")
    }
}

/// Persist a token to disk with restrictive permissions.
pub fn write_token(token: &SessionToken, path: &Path) -> crate::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        // Atomically create with restrictive mode; fail if the file already exists
        // to prevent symlink attacks (O_CREAT | O_EXCL with mode 0o600).
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .or_else(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    // Overwrite: remove and recreate to keep atomic semantics.
                    std::fs::remove_file(path).map_err(|re| SecurityError::PolicyViolation {
                        reason: format!("failed to remove existing token file: {re}"),
                    })?;
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .open(path)
                        .map_err(|e2| SecurityError::PolicyViolation {
                            reason: format!("failed to create token file: {e2}"),
                        })
                } else {
                    Err(SecurityError::PolicyViolation {
                        reason: format!("failed to create token file: {e}"),
                    })
                }
            })?;
        file.write_all(token.as_bytes())
            .map_err(|e| SecurityError::PolicyViolation {
                reason: format!("failed to write token data: {e}"),
            })?;
    }

    #[cfg(not(unix))]
    {
        // On non-Unix, fall back to plain write.
        std::fs::write(path, token.as_bytes()).map_err(|e| SecurityError::PolicyViolation {
            reason: format!("failed to write token file: {e}"),
        })?;
    }

    Ok(())
}

/// Read a token back from disk.
pub fn read_token(path: &Path) -> crate::Result<SessionToken> {
    let data = std::fs::read(path).map_err(|e| SecurityError::PolicyViolation {
        reason: format!("failed to read token file: {e}"),
    })?;
    if data.len() != TOKEN_LENGTH {
        return Err(SecurityError::PolicyViolation {
            reason: format!("token file has invalid length: {}", data.len()),
        });
    }
    let mut bytes = [0u8; TOKEN_LENGTH];
    bytes.copy_from_slice(&data);
    Ok(SessionToken::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // --- Token generation uniqueness ---

    #[test]
    fn test_generate_token_length() {
        let token = generate_session_token();
        assert_eq!(token.as_bytes().len(), TOKEN_LENGTH);
    }

    #[test]
    fn test_ten_tokens_all_unique() {
        let tokens: Vec<SessionToken> = (0..10).map(|_| generate_session_token()).collect();
        let set: HashSet<[u8; TOKEN_LENGTH]> = tokens.iter().map(|t| *t.as_bytes()).collect();
        assert_eq!(set.len(), 10, "all 10 tokens must be unique");
    }

    #[test]
    fn test_token_not_all_zeros() {
        let token = generate_session_token();
        assert_ne!(token.as_bytes(), &[0u8; TOKEN_LENGTH]);
    }

    // --- Token validation ---

    #[test]
    fn test_validate_valid_token() {
        let token = generate_session_token();
        assert!(validate_token(token.as_bytes(), &token));
    }

    #[test]
    fn test_validate_invalid_token() {
        let token = generate_session_token();
        let other = generate_session_token();
        assert!(!validate_token(other.as_bytes(), &token));
    }

    #[test]
    fn test_validate_wrong_length_short() {
        let token = generate_session_token();
        assert!(!validate_token(&[0u8; 16], &token));
    }

    #[test]
    fn test_validate_wrong_length_long() {
        let token = generate_session_token();
        assert!(!validate_token(&[0u8; 64], &token));
    }

    #[test]
    fn test_validate_empty_slice() {
        let token = generate_session_token();
        assert!(!validate_token(&[], &token));
    }

    #[test]
    fn test_validate_single_bit_difference() {
        let token = generate_session_token();
        let mut tampered = *token.as_bytes();
        tampered[0] ^= 1; // flip one bit
        assert!(!validate_token(&tampered, &token));
    }

    // --- Constant-time comparison (correctness, not timing) ---

    #[test]
    fn test_ct_eq_identical() {
        let token = SessionToken::from_bytes([0xAA; TOKEN_LENGTH]);
        assert!(token.ct_eq(&[0xAA; TOKEN_LENGTH]));
    }

    #[test]
    fn test_ct_eq_different() {
        let token = SessionToken::from_bytes([0xAA; TOKEN_LENGTH]);
        assert!(!token.ct_eq(&[0xBB; TOKEN_LENGTH]));
    }

    #[test]
    fn test_ct_eq_wrong_length_returns_false() {
        let token = SessionToken::from_bytes([0xAA; TOKEN_LENGTH]);
        assert!(!token.ct_eq(&[0xAA; 16]));
    }

    // --- Token persistence ---

    #[test]
    fn test_write_and_read_token() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("session.tok");
        let token = generate_session_token();
        write_token(&token, &path).unwrap();
        let loaded = read_token(&path).unwrap();
        assert_eq!(loaded.as_bytes(), token.as_bytes());
    }

    #[test]
    fn test_read_token_invalid_length() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("bad.tok");
        std::fs::write(&path, b"short").unwrap();
        assert!(read_token(&path).is_err());
    }

    #[test]
    fn test_read_token_missing_file() {
        let path = std::path::Path::new("nonexistent_token_file.tok");
        assert!(read_token(path).is_err());
    }

    #[test]
    fn test_token_debug_redacted() {
        let token = generate_session_token();
        let debug = format!("{token:?}");
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("0x"));
    }

    #[test]
    fn test_token_dir_returns_path() {
        let dir = token_dir();
        assert!(!dir.as_os_str().is_empty());
    }
}
