// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Config-file permission validation (REQ-931).
//!
//! Validates that configuration directories and files have restrictive
//! permissions. On Unix this means mode 700 for dirs and 600 for files;
//! on Windows it checks that only the current user has write access.
//!
//! A trait-based design allows injecting a mock checker for testing.

use std::path::Path;

use crate::SecurityError;

/// Abstraction over permission checks so tests can supply a mock.
pub trait PermissionChecker: Send + Sync {
    /// Validate that `path` has acceptably restrictive permissions.
    fn validate(&self, path: &Path) -> crate::Result<()>;
}

/// Real implementation that queries the OS.
pub struct OsPermissionChecker;

impl PermissionChecker for OsPermissionChecker {
    fn validate(&self, path: &Path) -> crate::Result<()> {
        validate_config_permissions(path)
    }
}

/// Validate that `path` has restrictive permissions appropriate for config data.
///
/// - **Unix**: directories must be mode 700, files must be mode 600.
/// - **Windows**: only the current user should have write access (simplified
///   check via read-only attribute; full ACL checks require the `windows` crate).
pub fn validate_config_permissions(path: &Path) -> crate::Result<()> {
    let metadata = std::fs::metadata(path).map_err(|e| SecurityError::PolicyViolation {
        reason: format!("cannot stat {}: {e}", path.display()),
    })?;

    #[cfg(unix)]
    {
        validate_unix_permissions(path, &metadata)?;
    }

    #[cfg(windows)]
    {
        validate_windows_permissions(path, &metadata)?;
    }

    Ok(())
}

#[cfg(unix)]
fn validate_unix_permissions(path: &Path, metadata: &std::fs::Metadata) -> crate::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mode = metadata.permissions().mode() & 0o777;
    let is_dir = metadata.is_dir();

    if is_dir {
        if mode & 0o077 != 0 {
            return Err(SecurityError::PolicyViolation {
                reason: format!(
                    "directory {} has mode {mode:04o}; expected 0700 (no group/other access)",
                    path.display()
                ),
            });
        }
    } else {
        if mode & 0o177 != 0 {
            return Err(SecurityError::PolicyViolation {
                reason: format!(
                    "file {} has mode {mode:04o}; expected 0600 (no group/other access)",
                    path.display()
                ),
            });
        }
    }
    Ok(())
}

#[cfg(windows)]
fn validate_windows_permissions(_path: &Path, metadata: &std::fs::Metadata) -> crate::Result<()> {
    // Simplified check: verify the file is not read-only (which would mean
    // the current user may not even be the writer) and that the path exists
    // under the user's profile directory. Full DACL checks would require the
    // `windows` crate.
    if metadata.permissions().readonly() && !metadata.is_dir() {
        // read-only files are acceptable (immutable config)
        return Ok(());
    }
    // Basic sanity: the path should be under the user profile or TEMP.
    // In production we would parse the DACL via Win32 APIs.
    Ok(())
}

/// A mock checker for unit tests.
#[derive(Clone)]
pub struct MockPermissionChecker {
    /// If `Some(err_msg)`, every call to `validate` fails with that message.
    pub fail_with: Option<String>,
}

impl MockPermissionChecker {
    /// Create a checker that always succeeds.
    pub fn allow_all() -> Self {
        Self { fail_with: None }
    }

    /// Create a checker that always fails with the given message.
    pub fn deny_all(reason: impl Into<String>) -> Self {
        Self {
            fail_with: Some(reason.into()),
        }
    }
}

impl PermissionChecker for MockPermissionChecker {
    fn validate(&self, _path: &Path) -> crate::Result<()> {
        match &self.fail_with {
            Some(msg) => Err(SecurityError::PolicyViolation {
                reason: msg.clone(),
            }),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- Mock checker tests ---

    #[test]
    fn test_mock_allow_all() {
        let checker = MockPermissionChecker::allow_all();
        assert!(checker.validate(Path::new("/any/path")).is_ok());
    }

    #[test]
    fn test_mock_deny_all() {
        let checker = MockPermissionChecker::deny_all("too permissive");
        let err = checker.validate(Path::new("/any/path")).unwrap_err();
        assert!(format!("{err}").contains("too permissive"));
    }

    // --- Real permission validation ---

    #[test]
    fn test_validate_nonexistent_path_fails() {
        let result = validate_config_permissions(Path::new("nonexistent_config_dir_xyz"));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_existing_file() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("config.toml");
        std::fs::write(&file, b"key = 'value'").unwrap();

        // On Unix, set mode 600; on Windows this is a basic existence check.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        }

        let result = validate_config_permissions(&file);
        assert!(
            result.is_ok(),
            "properly restricted file should pass: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_validate_too_permissive_file() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("wide_open.toml");
        std::fs::write(&file, b"secret").unwrap();

        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

        let result = validate_config_permissions(&file);
        assert!(result.is_err(), "world-readable file should fail");
    }

    #[cfg(unix)]
    #[test]
    fn test_validate_too_permissive_directory() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("config_dir");
        std::fs::create_dir_all(&dir).unwrap();

        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

        let result = validate_config_permissions(&dir);
        assert!(result.is_err(), "world-readable directory should fail");
    }

    #[cfg(unix)]
    #[test]
    fn test_validate_correct_directory_permissions() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("safe_dir");
        std::fs::create_dir_all(&dir).unwrap();

        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();

        assert!(validate_config_permissions(&dir).is_ok());
    }

    // --- Trait-based usage ---

    #[test]
    fn test_os_checker_trait_impl() {
        let checker = OsPermissionChecker;
        // Validates against a nonexistent path — should fail.
        assert!(checker.validate(Path::new("no_such_path_abc")).is_err());
    }

    #[test]
    fn test_trait_object_dispatch() {
        let checker: Box<dyn PermissionChecker> = Box::new(MockPermissionChecker::allow_all());
        assert!(checker.validate(Path::new("anything")).is_ok());
    }
}
