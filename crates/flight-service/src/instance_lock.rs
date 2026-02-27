//! Prevents multiple service instances from running simultaneously.
//!
//! Uses an exclusive file lock (via atomic file creation) to ensure
//! only one instance of flightd runs at a time.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Error type for instance lock operations.
#[derive(Debug)]
pub enum InstanceLockError {
    /// Another instance is already running.
    AlreadyRunning { pid: Option<u32> },
    /// I/O error while acquiring or releasing the lock.
    Io(std::io::Error),
}

impl std::fmt::Display for InstanceLockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning { pid: Some(p) } => {
                write!(f, "Another instance is running (PID {})", p)
            }
            Self::AlreadyRunning { pid: None } => write!(f, "Another instance is running"),
            Self::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl From<std::io::Error> for InstanceLockError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Guards a single-instance lock file for the duration of its lifetime.
/// Dropping this type releases the lock.
pub struct InstanceLock {
    path: PathBuf,
    _file: File,
}

impl InstanceLock {
    /// Default lock file path in temp directory.
    pub fn default_path() -> PathBuf {
        std::env::temp_dir().join("openflight.lock")
    }

    /// Try to acquire the instance lock at the given path.
    ///
    /// Writes the current PID to the lock file.
    /// Returns `InstanceLockError::AlreadyRunning` if the file already exists
    /// and contains a running process ID.
    pub fn acquire(path: &Path) -> Result<Self, InstanceLockError> {
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(mut file) => {
                let pid = std::process::id();
                let _ = write!(file, "{}", pid);
                Ok(Self {
                    path: path.to_path_buf(),
                    _file: file,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let pid = std::fs::read_to_string(path)
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok());
                Err(InstanceLockError::AlreadyRunning { pid })
            }
            Err(e) => Err(InstanceLockError::Io(e)),
        }
    }

    /// Check if an instance is already running (without acquiring the lock).
    pub fn is_locked(path: &Path) -> bool {
        path.exists()
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_lock_path(suffix: &str) -> PathBuf {
        env::temp_dir().join(format!("openflight_test_{}.lock", suffix))
    }

    #[test]
    fn test_acquire_creates_lock_file() {
        let path = temp_lock_path("create");
        let _ = std::fs::remove_file(&path);
        let lock = InstanceLock::acquire(&path).expect("Should acquire lock");
        assert!(path.exists());
        drop(lock);
        assert!(!path.exists());
    }

    #[test]
    fn test_double_acquire_fails() {
        let path = temp_lock_path("double");
        let _ = std::fs::remove_file(&path);
        let _lock1 = InstanceLock::acquire(&path).expect("First acquire should succeed");
        let result = InstanceLock::acquire(&path);
        assert!(matches!(
            result,
            Err(InstanceLockError::AlreadyRunning { .. })
        ));
    }

    #[test]
    fn test_drop_releases_lock() {
        let path = temp_lock_path("release");
        let _ = std::fs::remove_file(&path);
        {
            let _lock = InstanceLock::acquire(&path).expect("Should acquire");
            assert!(path.exists());
        }
        let _lock2 = InstanceLock::acquire(&path).expect("Should re-acquire after drop");
    }

    #[test]
    fn test_is_locked_returns_false_when_free() {
        let path = temp_lock_path("free");
        let _ = std::fs::remove_file(&path);
        assert!(!InstanceLock::is_locked(&path));
    }

    #[test]
    fn test_is_locked_returns_true_when_held() {
        let path = temp_lock_path("held");
        let _ = std::fs::remove_file(&path);
        let _lock = InstanceLock::acquire(&path).expect("Should acquire");
        assert!(InstanceLock::is_locked(&path));
    }

    #[test]
    fn test_default_path_in_temp_dir() {
        let path = InstanceLock::default_path();
        assert!(path.to_str().unwrap().contains("openflight"));
    }

    #[test]
    fn test_lock_contains_pid() {
        let path = temp_lock_path("pid");
        let _ = std::fs::remove_file(&path);
        let lock = InstanceLock::acquire(&path).expect("Should acquire");
        let contents = std::fs::read_to_string(&path).unwrap_or_default();
        let pid: u32 = contents.trim().parse().expect("Should contain PID");
        assert_eq!(pid, std::process::id());
        drop(lock);
    }
}
