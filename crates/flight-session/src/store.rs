// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Session state persistence with atomic disk writes.
//!
//! [`SessionStore`] serialises a [`SessionState`] snapshot to JSON and writes
//! it atomically (write-temp then rename) so that a crash mid-write never
//! leaves a corrupt state file.

use crate::migration::{self, CURRENT_VERSION};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// Errors produced by [`SessionStore`] operations.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("migration error: {0}")]
    Migration(#[from] migration::MigrationError),
}

// ── Data types ───────────────────────────────────────────────────────────

/// Persisted session state that survives service restarts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SessionState {
    /// Currently loaded profile name.
    pub active_profile: Option<String>,
    /// Device-id → role mapping.
    pub device_assignments: HashMap<String, String>,
    /// Last connected simulator identifier.
    pub last_sim: Option<String>,
    /// UI window positions keyed by window name.
    pub window_positions: HashMap<String, WindowPosition>,
    /// Per-device calibration results keyed by device id.
    pub calibration_data: HashMap<String, CalibrationData>,
    /// Information about the last shutdown.
    pub last_shutdown: Option<ShutdownInfo>,
}

/// A savedwindow position and size.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Per-device calibration snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationData {
    pub min: f64,
    pub max: f64,
    pub center: f64,
    pub deadzone: f64,
    /// Unix epoch seconds when calibration was performed.
    pub timestamp: u64,
}

/// Reason the service shut down.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ShutdownReason {
    Clean,
    Crash,
    Unknown,
}

/// Shutdown metadata persisted with the session state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShutdownInfo {
    /// Unix epoch seconds of shutdown.
    pub timestamp: u64,
    pub reason: ShutdownReason,
}

// ── Versioned envelope ───────────────────────────────────────────────────

/// On-disk wrapper that carries the schema version alongside the state JSON.
#[derive(Debug, Serialize, Deserialize)]
struct VersionedState {
    version: u32,
    state: serde_json::Value,
}

// ── SessionStore ─────────────────────────────────────────────────────────

/// Manages reading/writing [`SessionState`] to a JSON file on disk.
pub struct SessionStore {
    path: PathBuf,
}

impl SessionStore {
    /// Create a store that persists to `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The file path used by this store.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Serialize `state` and write it atomically (write-temp, then rename).
    pub fn save(&self, state: &SessionState) -> Result<(), StoreError> {
        let state_value = serde_json::to_value(state)?;
        let envelope = VersionedState {
            version: CURRENT_VERSION,
            state: state_value,
        };
        let json = serde_json::to_string_pretty(&envelope)?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Atomic: write to a temp file in the same directory, then rename.
        let tmp_path = self.path.with_extension("tmp");
        std::fs::write(&tmp_path, json.as_bytes())?;
        std::fs::rename(&tmp_path, &self.path)?;

        Ok(())
    }

    /// Load persisted state. Returns `Ok(None)` when the file does not exist.
    pub fn load(&self) -> Result<Option<SessionState>, StoreError> {
        let data = match std::fs::read_to_string(&self.path) {
            Ok(data) => data,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let envelope: VersionedState = serde_json::from_str(&data)?;
        let state = migration::migrate(envelope.version, envelope.state)?;
        Ok(Some(state))
    }

    /// Delete the persisted state file. Silently succeeds if already absent.
    pub fn clear(&self) -> Result<(), StoreError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_state() -> SessionState {
        let mut state = SessionState::default();
        state.active_profile = Some("combat".into());
        state.last_sim = Some("MSFS".into());
        state
            .device_assignments
            .insert("stick-1".into(), "pitch_roll".into());
        state.window_positions.insert(
            "main".into(),
            WindowPosition {
                x: 100,
                y: 200,
                width: 1024,
                height: 768,
            },
        );
        state.calibration_data.insert(
            "stick-1".into(),
            CalibrationData {
                min: -1.0,
                max: 1.0,
                center: 0.0,
                deadzone: 0.05,
                timestamp: 1_700_000_000,
            },
        );
        state.last_shutdown = Some(ShutdownInfo {
            timestamp: 1_700_000_100,
            reason: ShutdownReason::Clean,
        });
        state
    }

    #[test]
    fn save_load_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));
        let state = sample_state();

        store.save(&state).unwrap();
        let loaded = store.load().unwrap().expect("state should exist");
        assert_eq!(state, loaded);
    }

    #[test]
    fn load_missing_file_returns_none() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("does_not_exist.json"));
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn clear_removes_file() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));
        store.save(&SessionState::default()).unwrap();
        assert!(store.path().exists());

        store.clear().unwrap();
        assert!(!store.path().exists());
    }

    #[test]
    fn clear_missing_file_is_ok() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("nope.json"));
        store.clear().unwrap(); // should not error
    }

    #[test]
    fn corrupt_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, "not valid json {{{").unwrap();

        let store = SessionStore::new(path);
        assert!(store.load().is_err());
    }

    #[test]
    fn atomic_write_no_tmp_leftover() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));
        store.save(&SessionState::default()).unwrap();

        let tmp = dir.path().join("state.tmp");
        assert!(!tmp.exists(), "temp file should be removed after rename");
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("a").join("b").join("state.json");
        let store = SessionStore::new(nested);
        store.save(&SessionState::default()).unwrap();
        assert!(store.path().exists());
    }

    #[test]
    fn default_session_state_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));
        let state = SessionState::default();

        store.save(&state).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(state, loaded);
    }

    #[test]
    fn overwrite_preserves_latest() {
        let dir = TempDir::new().unwrap();
        let store = SessionStore::new(dir.path().join("state.json"));

        let mut first = SessionState::default();
        first.active_profile = Some("alpha".into());
        store.save(&first).unwrap();

        let mut second = SessionState::default();
        second.active_profile = Some("beta".into());
        store.save(&second).unwrap();

        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.active_profile.as_deref(), Some("beta"));
    }
}
