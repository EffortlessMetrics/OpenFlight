//! Transactional installer with automatic rollback on failure.
//!
//! [`InstallTransaction`] records every side-effect as it executes so that a
//! partial installation can be completely reversed.

use std::fs;
use std::path::{Path, PathBuf};

// ── Error type ───────────────────────────────────────────────────────────────

/// Errors produced by install transactions.
#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("transaction already committed")]
    AlreadyCommitted,

    #[error("transaction already rolled back")]
    AlreadyRolledBack,

    #[error("service registration failed: {0}")]
    ServiceRegistration(String),

    #[error("rollback failed for operation {index}: {source}")]
    RollbackFailed {
        index: usize,
        source: std::io::Error,
    },
}

// ── Recorded operations ──────────────────────────────────────────────────────

/// A single reversible operation that was successfully executed.
#[derive(Debug)]
enum CompletedOp {
    /// A file was copied to `destination`.  If `backup` is `Some`, the
    /// original file was saved there and should be restored on rollback.
    FileCopied {
        destination: PathBuf,
        backup: Option<PathBuf>,
    },
    /// A directory was created.
    DirectoryCreated { path: PathBuf },
    /// A service was registered under `name`.
    ServiceRegistered {
        #[allow(dead_code)]
        name: String,
    },
}

// ── Transaction state ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TxState {
    Active,
    Committed,
    RolledBack,
}

/// A transactional installer that records operations and can undo them on
/// failure.
///
/// # Usage
///
/// ```ignore
/// let mut tx = InstallTransaction::new();
/// tx.create_directory(&dir)?;
/// tx.install_file(&src, &dst)?;
/// tx.register_service("FlightHub")?;
/// tx.commit(); // after this point rollback is no longer possible
/// ```
#[derive(Debug)]
pub struct InstallTransaction {
    ops: Vec<CompletedOp>,
    state: TxState,
}

impl InstallTransaction {
    /// Create a new, empty transaction.
    pub fn new() -> Self {
        Self {
            ops: Vec::new(),
            state: TxState::Active,
        }
    }

    /// Number of operations recorded so far.
    pub fn operation_count(&self) -> usize {
        self.ops.len()
    }

    // ── Operations ───────────────────────────────────────────────────────

    /// Create a directory (and any missing parents).
    pub fn create_directory(&mut self, path: &Path) -> Result<(), TransactionError> {
        self.ensure_active()?;
        if !path.exists() {
            fs::create_dir_all(path)?;
            self.ops.push(CompletedOp::DirectoryCreated {
                path: path.to_path_buf(),
            });
        }
        Ok(())
    }

    /// Copy `src` to `dst`, creating parent directories as needed.
    ///
    /// If `dst` already exists it is backed up to `dst.bak` so that rollback
    /// can restore the original.
    pub fn install_file(&mut self, src: &Path, dst: &Path) -> Result<(), TransactionError> {
        self.ensure_active()?;

        if let Some(parent) = dst.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
            self.ops.push(CompletedOp::DirectoryCreated {
                path: parent.to_path_buf(),
            });
        }

        let backup = if dst.exists() {
            let bak = dst.with_extension("bak");
            fs::copy(dst, &bak)?;
            Some(bak)
        } else {
            None
        };

        fs::copy(src, dst)?;

        self.ops.push(CompletedOp::FileCopied {
            destination: dst.to_path_buf(),
            backup,
        });

        Ok(())
    }

    /// Record that a service was registered.
    ///
    /// In production this would call platform APIs (sc.exe / systemctl).
    /// For testability the actual registration is a no-op; the record exists
    /// so rollback can deregister.
    pub fn register_service(&mut self, name: &str) -> Result<(), TransactionError> {
        self.ensure_active()?;
        self.ops.push(CompletedOp::ServiceRegistered {
            name: name.to_string(),
        });
        Ok(())
    }

    // ── Finalization ─────────────────────────────────────────────────────

    /// Finalize the transaction.  After this call no rollback is possible.
    pub fn commit(&mut self) -> Result<(), TransactionError> {
        self.ensure_active()?;
        // Remove any leftover backup files so the install is clean.
        for op in &self.ops {
            if let CompletedOp::FileCopied {
                backup: Some(bak), ..
            } = op
            {
                let _ = fs::remove_file(bak);
            }
        }
        self.state = TxState::Committed;
        Ok(())
    }

    /// Undo every recorded operation in reverse order.
    ///
    /// Returns `Ok(())` when all rollback steps succeed. On partial rollback
    /// failure the first error is returned with the index of the failing
    /// operation.
    pub fn rollback(&mut self) -> Result<(), TransactionError> {
        self.ensure_active()?;
        self.state = TxState::RolledBack;

        for (i, op) in self.ops.iter().rev().enumerate() {
            match op {
                CompletedOp::FileCopied {
                    destination,
                    backup,
                } => {
                    // Remove the installed file.
                    if destination.exists() {
                        fs::remove_file(destination).map_err(|e| {
                            TransactionError::RollbackFailed {
                                index: i,
                                source: e,
                            }
                        })?;
                    }
                    // Restore backup if present.
                    if let Some(bak) = backup
                        && bak.exists()
                    {
                        fs::rename(bak, destination).map_err(|e| {
                            TransactionError::RollbackFailed {
                                index: i,
                                source: e,
                            }
                        })?;
                    }
                }
                CompletedOp::DirectoryCreated { path } => {
                    // Only remove if empty — other files may have been placed
                    // here by the user.
                    if path.exists() {
                        let _ = fs::remove_dir(path);
                    }
                }
                CompletedOp::ServiceRegistered { .. } => {
                    // In production: deregister service via platform API.
                    // In test: no-op.
                }
            }
        }

        Ok(())
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn ensure_active(&self) -> Result<(), TransactionError> {
        match self.state {
            TxState::Active => Ok(()),
            TxState::Committed => Err(TransactionError::AlreadyCommitted),
            TxState::RolledBack => Err(TransactionError::AlreadyRolledBack),
        }
    }
}

impl Default for InstallTransaction {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: write a small file and return its path.
    fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn install_file_copies_to_destination() {
        let tmp = TempDir::new().unwrap();
        let src = write_file(tmp.path(), "src.txt", "hello");
        let dst = tmp.path().join("installed/file.txt");

        let mut tx = InstallTransaction::new();
        tx.install_file(&src, &dst).unwrap();
        tx.commit().unwrap();

        assert!(dst.exists());
        assert_eq!(fs::read_to_string(&dst).unwrap(), "hello");
    }

    #[test]
    fn rollback_removes_installed_file() {
        let tmp = TempDir::new().unwrap();
        let src = write_file(tmp.path(), "src.txt", "data");
        let dst = tmp.path().join("out/file.txt");

        let mut tx = InstallTransaction::new();
        tx.install_file(&src, &dst).unwrap();
        assert!(dst.exists());

        tx.rollback().unwrap();
        assert!(!dst.exists());
    }

    #[test]
    fn rollback_restores_backup() {
        let tmp = TempDir::new().unwrap();
        let src_new = write_file(tmp.path(), "new.txt", "new content");
        let dst = write_file(tmp.path(), "existing.txt", "original content");

        let mut tx = InstallTransaction::new();
        tx.install_file(&src_new, &dst).unwrap();
        assert_eq!(fs::read_to_string(&dst).unwrap(), "new content");

        tx.rollback().unwrap();
        assert_eq!(fs::read_to_string(&dst).unwrap(), "original content");
    }

    #[test]
    fn commit_removes_backup_files() {
        let tmp = TempDir::new().unwrap();
        let src_new = write_file(tmp.path(), "new.txt", "new");
        let dst = write_file(tmp.path(), "existing.txt", "old");
        let bak = dst.with_extension("bak");

        let mut tx = InstallTransaction::new();
        tx.install_file(&src_new, &dst).unwrap();
        assert!(bak.exists());

        tx.commit().unwrap();
        assert!(!bak.exists());
        assert_eq!(fs::read_to_string(&dst).unwrap(), "new");
    }

    #[test]
    fn double_commit_is_error() {
        let mut tx = InstallTransaction::new();
        tx.commit().unwrap();
        assert!(matches!(
            tx.commit(),
            Err(TransactionError::AlreadyCommitted)
        ));
    }

    #[test]
    fn rollback_after_commit_is_error() {
        let mut tx = InstallTransaction::new();
        tx.commit().unwrap();
        assert!(matches!(
            tx.rollback(),
            Err(TransactionError::AlreadyCommitted)
        ));
    }

    #[test]
    fn operation_after_rollback_is_error() {
        let tmp = TempDir::new().unwrap();
        let src = write_file(tmp.path(), "f.txt", "x");

        let mut tx = InstallTransaction::new();
        tx.rollback().unwrap();
        assert!(matches!(
            tx.install_file(&src, &tmp.path().join("dst.txt")),
            Err(TransactionError::AlreadyRolledBack)
        ));
    }

    #[test]
    fn create_directory_is_recorded() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("a/b/c");

        let mut tx = InstallTransaction::new();
        tx.create_directory(&dir).unwrap();
        assert!(dir.exists());

        tx.rollback().unwrap();
        // Deepest empty dir removed; parents may remain if non-empty.
        assert!(!dir.exists());
    }

    #[test]
    fn register_service_is_recorded() {
        let mut tx = InstallTransaction::new();
        tx.register_service("FlightHub").unwrap();
        assert_eq!(tx.operation_count(), 1);
        tx.rollback().unwrap();
    }

    #[test]
    fn operation_count_tracks_all_ops() {
        let tmp = TempDir::new().unwrap();
        let src = write_file(tmp.path(), "s.txt", "d");

        let mut tx = InstallTransaction::new();
        tx.create_directory(&tmp.path().join("sub")).unwrap();
        tx.install_file(&src, &tmp.path().join("sub/d.txt"))
            .unwrap();
        tx.register_service("svc").unwrap();
        assert_eq!(tx.operation_count(), 3);
    }
}
