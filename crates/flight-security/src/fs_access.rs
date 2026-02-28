// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! File system access control.
//!
//! Restricts plugin and service file access to approved configuration
//! directories. All paths are canonicalized before comparison to prevent
//! traversal attacks (`../`, symlink escapes, etc.).

use std::path::{Path, PathBuf};

use crate::SecurityError;

/// File-system access policy.
///
/// Maintains a whitelist of allowed directory roots. Every access request is
/// validated against this whitelist after path canonicalization.
#[derive(Debug, Clone)]
pub struct FsAccessPolicy {
    /// Allowed directory roots (canonical, absolute paths).
    allowed_roots: Vec<PathBuf>,
}

impl FsAccessPolicy {
    /// Create a policy that allows access to `roots`.
    ///
    /// Each root is canonicalized at construction time so that later
    /// comparisons are reliable. Roots that cannot be canonicalized (e.g. they
    /// do not exist yet) are stored as-is after normalising separators.
    pub fn new(roots: &[PathBuf]) -> Self {
        let allowed_roots = roots
            .iter()
            .map(|r| std::fs::canonicalize(r).unwrap_or_else(|_| normalize_path(r)))
            .collect();
        Self { allowed_roots }
    }

    /// Validate that `requested` falls inside one of the allowed roots.
    ///
    /// The path is canonicalized first; if canonicalization fails (the path
    /// does not exist) the raw normalised form is checked instead.
    pub fn validate(&self, requested: &Path) -> crate::Result<PathBuf> {
        let canonical = std::fs::canonicalize(requested)
            .unwrap_or_else(|_| normalize_path(requested));

        // Reject paths that contain traversal components even after
        // normalisation (defence-in-depth).
        if has_traversal_components(requested) {
            return Err(SecurityError::PathTraversal {
                path: requested.to_path_buf(),
            });
        }

        for root in &self.allowed_roots {
            if canonical.starts_with(root) {
                return Ok(canonical);
            }
        }

        Err(SecurityError::UnauthorizedPath {
            path: requested.to_path_buf(),
            allowed_roots: self.allowed_roots.clone(),
        })
    }

    /// Return the set of allowed roots.
    pub fn allowed_roots(&self) -> &[PathBuf] {
        &self.allowed_roots
    }
}

/// Check whether a path contains `..` or other traversal components.
fn has_traversal_components(path: &Path) -> bool {
    use std::path::Component;
    path.components().any(|c| matches!(c, Component::ParentDir))
}

/// Normalise a path without touching the filesystem (no symlink resolution).
/// Collapses `.` and strips trailing separators.
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {} // skip `.`
            _ => out.push(component),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_access_within_allowed_root_succeeds() {
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let file = config_dir.join("profile.json");
        std::fs::write(&file, b"{}").unwrap();

        let policy = FsAccessPolicy::new(&[config_dir.clone()]);
        assert!(policy.validate(&file).is_ok());
    }

    #[test]
    fn test_access_outside_root_rejected() {
        let tmp = TempDir::new().unwrap();
        let allowed = tmp.path().join("config");
        std::fs::create_dir_all(&allowed).unwrap();

        let outside = tmp.path().join("secrets").join("key.pem");
        std::fs::create_dir_all(outside.parent().unwrap()).unwrap();
        std::fs::write(&outside, b"secret").unwrap();

        let policy = FsAccessPolicy::new(&[allowed]);
        assert!(policy.validate(&outside).is_err());
    }

    #[test]
    fn test_path_traversal_rejected() {
        let tmp = TempDir::new().unwrap();
        let allowed = tmp.path().join("config");
        std::fs::create_dir_all(&allowed).unwrap();

        // Attempt to escape with `..`
        let traversal = allowed.join("..").join("secrets").join("key.pem");
        let policy = FsAccessPolicy::new(&[allowed]);
        assert!(policy.validate(&traversal).is_err());
    }

    #[test]
    fn test_dot_dot_in_middle_of_path() {
        let tmp = TempDir::new().unwrap();
        let allowed = tmp.path().join("config");
        std::fs::create_dir_all(&allowed).unwrap();

        let sneaky = allowed.join("subdir").join("..").join("..").join("etc").join("passwd");
        let policy = FsAccessPolicy::new(&[allowed]);
        assert!(policy.validate(&sneaky).is_err());
    }

    #[test]
    fn test_multiple_allowed_roots() {
        let tmp = TempDir::new().unwrap();
        let root_a = tmp.path().join("a");
        let root_b = tmp.path().join("b");
        std::fs::create_dir_all(&root_a).unwrap();
        std::fs::create_dir_all(&root_b).unwrap();

        let file_a = root_a.join("ok.txt");
        let file_b = root_b.join("ok.txt");
        std::fs::write(&file_a, b"a").unwrap();
        std::fs::write(&file_b, b"b").unwrap();

        let policy = FsAccessPolicy::new(&[root_a, root_b]);
        assert!(policy.validate(&file_a).is_ok());
        assert!(policy.validate(&file_b).is_ok());
    }

    #[test]
    fn test_empty_policy_rejects_everything() {
        let policy = FsAccessPolicy::new(&[]);
        let path = PathBuf::from("anything.txt");
        assert!(policy.validate(&path).is_err());
    }

    #[test]
    fn test_allowed_roots_getter() {
        let roots = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        let policy = FsAccessPolicy::new(&roots);
        assert_eq!(policy.allowed_roots().len(), 2);
    }
}
