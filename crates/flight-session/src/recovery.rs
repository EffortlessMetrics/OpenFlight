// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Crash recovery for the flight-session service.
//!
//! [`RecoveryManager`] uses a heartbeat file to detect unclean shutdowns. If
//! the service crashes, the next startup sees a heartbeat without a clean
//! shutdown marker and enters recovery mode, restoring the last known good
//! state from disk.

use crate::store::{SessionState, SessionStore, ShutdownInfo, ShutdownReason, StoreError};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

const HEARTBEAT_FILE: &str = "heartbeat";
const CLEAN_SHUTDOWN_FILE: &str = "clean_shutdown";
const STATE_FILE: &str = "session_state.json";
const DEFAULT_STALENESS_SECS: u64 = 30;

/// Errors produced by [`RecoveryManager`] operations.
#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("store error: {0}")]
    Store(#[from] StoreError),
}

/// Manages heartbeat liveness checks and crash recovery.
pub struct RecoveryManager {
    state_dir: PathBuf,
    store: SessionStore,
    staleness_threshold: Duration,
}

impl RecoveryManager {
    /// Create a manager that keeps all state files under `state_dir`.
    pub fn new(state_dir: impl Into<PathBuf>) -> Self {
        let state_dir = state_dir.into();
        let store_path = state_dir.join(STATE_FILE);
        Self {
            store: SessionStore::new(store_path),
            state_dir,
            staleness_threshold: Duration::from_secs(DEFAULT_STALENESS_SECS),
        }
    }

    /// Override the heartbeat staleness threshold (default 30 s).
    pub fn with_staleness_threshold(mut self, threshold: Duration) -> Self {
        self.staleness_threshold = threshold;
        self
    }

    /// Access the inner [`SessionStore`].
    pub fn store(&self) -> &SessionStore {
        &self.store
    }

    /// The directory containing all state/heartbeat files.
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    // ── Path helpers ─────────────────────────────────────────────────────

    fn heartbeat_path(&self) -> PathBuf {
        self.state_dir.join(HEARTBEAT_FILE)
    }

    fn clean_shutdown_path(&self) -> PathBuf {
        self.state_dir.join(CLEAN_SHUTDOWN_FILE)
    }

    // ── Public API ───────────────────────────────────────────────────────

    /// Record a heartbeat timestamp to prove liveness. Call periodically.
    pub fn set_heartbeat(&self) -> Result<(), RecoveryError> {
        std::fs::create_dir_all(&self.state_dir)?;
        let now = now_secs();
        std::fs::write(self.heartbeat_path(), now.to_string())?;
        Ok(())
    }

    /// Returns `true` if a clean-shutdown marker is present.
    pub fn check_clean_shutdown(&self) -> Result<bool, RecoveryError> {
        Ok(self.clean_shutdown_path().exists())
    }

    /// Restore the last known good state after a crash.
    ///
    /// Clears the heartbeat and clean-shutdown marker, then loads the
    /// persisted [`SessionState`].
    pub fn recover(&self) -> Result<Option<SessionState>, RecoveryError> {
        let _ = std::fs::remove_file(self.heartbeat_path());
        let _ = std::fs::remove_file(self.clean_shutdown_path());
        let state = self.store.load()?;
        Ok(state)
    }

    /// Mark the current shutdown as clean. Call during graceful shutdown.
    pub fn mark_clean_shutdown(&self) -> Result<(), RecoveryError> {
        std::fs::create_dir_all(&self.state_dir)?;
        let now = now_secs();
        std::fs::write(self.clean_shutdown_path(), now.to_string())?;
        // Heartbeat is no longer needed once we're shutting down cleanly.
        let _ = std::fs::remove_file(self.heartbeat_path());
        Ok(())
    }

    /// Returns `true` if the heartbeat file exists and is older than the
    /// staleness threshold.
    pub fn is_heartbeat_stale(&self) -> Result<bool, RecoveryError> {
        let content = match std::fs::read_to_string(self.heartbeat_path()) {
            Ok(c) => c,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e.into()),
        };

        let hb_timestamp: u64 = content.trim().parse().unwrap_or(0);
        let now = now_secs();

        Ok(now.saturating_sub(hb_timestamp) > self.staleness_threshold.as_secs())
    }

    /// Determine whether the service needs recovery.
    ///
    /// Returns `true` when a heartbeat file exists **without** a
    /// corresponding clean-shutdown marker — i.e. the service likely
    /// crashed.
    pub fn needs_recovery(&self) -> Result<bool, RecoveryError> {
        let has_heartbeat = self.heartbeat_path().exists();
        let has_clean = self.clean_shutdown_path().exists();
        Ok(has_heartbeat && !has_clean)
    }

    /// Convenience: save state through the inner store and record a
    /// shutdown marker in one call.
    pub fn save_and_mark_shutdown(
        &self,
        state: &SessionState,
        reason: ShutdownReason,
    ) -> Result<(), RecoveryError> {
        let mut state = state.clone();
        state.last_shutdown = Some(ShutdownInfo {
            timestamp: now_secs(),
            reason,
        });
        self.store.save(&state)?;
        self.mark_clean_shutdown()?;
        Ok(())
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SessionState;
    use tempfile::TempDir;

    #[test]
    fn fresh_dir_no_recovery_needed() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));
        assert!(!mgr.needs_recovery().unwrap());
    }

    #[test]
    fn heartbeat_without_shutdown_needs_recovery() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));
        mgr.set_heartbeat().unwrap();
        assert!(mgr.needs_recovery().unwrap());
    }

    #[test]
    fn clean_shutdown_clears_heartbeat() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));
        mgr.set_heartbeat().unwrap();
        mgr.mark_clean_shutdown().unwrap();

        assert!(!mgr.needs_recovery().unwrap());
        assert!(mgr.check_clean_shutdown().unwrap());
        assert!(!mgr.heartbeat_path().exists());
    }

    #[test]
    fn recover_loads_persisted_state() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));

        let mut state = SessionState::default();
        state.active_profile = Some("combat".into());
        mgr.store().save(&state).unwrap();
        mgr.set_heartbeat().unwrap();

        // Simulate crash: no clean shutdown marker.
        assert!(mgr.needs_recovery().unwrap());

        let recovered = mgr.recover().unwrap().expect("state should be present");
        assert_eq!(recovered.active_profile.as_deref(), Some("combat"));
        // After recovery, heartbeat is removed.
        assert!(!mgr.heartbeat_path().exists());
    }

    #[test]
    fn recover_with_no_state_returns_none() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));
        mgr.set_heartbeat().unwrap();

        let recovered = mgr.recover().unwrap();
        assert!(recovered.is_none());
    }

    #[test]
    fn heartbeat_staleness_fresh_is_not_stale() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"))
            .with_staleness_threshold(Duration::from_secs(60));
        mgr.set_heartbeat().unwrap();
        assert!(!mgr.is_heartbeat_stale().unwrap());
    }

    #[test]
    fn heartbeat_staleness_old_is_stale() {
        let dir = TempDir::new().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        // Write a heartbeat from the distant past.
        std::fs::write(session_dir.join(HEARTBEAT_FILE), "1000000000").unwrap();

        let mgr =
            RecoveryManager::new(&session_dir).with_staleness_threshold(Duration::from_secs(10));
        assert!(mgr.is_heartbeat_stale().unwrap());
    }

    #[test]
    fn no_heartbeat_is_not_stale() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));
        assert!(!mgr.is_heartbeat_stale().unwrap());
    }

    #[test]
    fn save_and_mark_shutdown_persists_state() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));

        let state = SessionState::default();
        mgr.save_and_mark_shutdown(&state, ShutdownReason::Clean)
            .unwrap();

        assert!(mgr.check_clean_shutdown().unwrap());
        let loaded = mgr.store().load().unwrap().unwrap();
        assert!(loaded.last_shutdown.is_some());
        assert_eq!(loaded.last_shutdown.unwrap().reason, ShutdownReason::Clean);
    }

    #[test]
    fn multiple_heartbeats_overwrite() {
        let dir = TempDir::new().unwrap();
        let mgr = RecoveryManager::new(dir.path().join("session"));
        mgr.set_heartbeat().unwrap();
        let first = std::fs::read_to_string(mgr.heartbeat_path()).unwrap();
        // Second heartbeat overwrites the first.
        mgr.set_heartbeat().unwrap();
        let second = std::fs::read_to_string(mgr.heartbeat_path()).unwrap();
        // Both should be valid timestamps (the second ≥ first).
        let t1: u64 = first.trim().parse().unwrap();
        let t2: u64 = second.trim().parse().unwrap();
        assert!(t2 >= t1);
    }
}
