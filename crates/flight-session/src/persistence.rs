// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Session persistence with JSON-backed save/load and metadata tracking.
//!
//! [`SessionPersistence`] manages a single session file on disk, including
//! UUID generation, metadata (simulator, aircraft, profile, devices), and
//! last-active timestamps for crash-recovery heuristics.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Metadata describing a persisted session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMetadata {
    /// Unique session identifier (UUID v4-style hex string).
    pub session_id: String,
    /// Unix epoch seconds when the session started.
    pub start_time: u64,
    /// Last time (epoch secs) the session was marked active.
    pub last_active: u64,
    /// Connected simulator, if any.
    pub simulator: Option<String>,
    /// Currently loaded aircraft, if any.
    pub aircraft: Option<String>,
    /// Active profile name, if any.
    pub profile: Option<String>,
    /// List of connected device identifiers.
    pub devices: Vec<String>,
}

/// Errors produced by [`SessionPersistence`].
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Manages saving and loading session metadata to a JSON file.
pub struct SessionPersistence {
    path: PathBuf,
}

impl SessionPersistence {
    /// Create a persistence handle targeting the given file path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The file path managed by this instance.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Generate a new [`SessionMetadata`] with a fresh ID and the current time.
    pub fn create_session() -> SessionMetadata {
        let now = now_secs();
        SessionMetadata {
            session_id: generate_session_id(),
            start_time: now,
            last_active: now,
            simulator: None,
            aircraft: None,
            profile: None,
            devices: Vec::new(),
        }
    }

    /// Save `metadata` to disk as pretty-printed JSON.
    ///
    /// Uses write-to-temp-then-rename for atomicity.
    pub fn save(&self, metadata: &SessionMetadata) -> Result<(), PersistenceError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(metadata)?;
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, json.as_bytes())?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    /// Load [`SessionMetadata`] from disk. Returns `Ok(None)` if the file is
    /// absent.
    pub fn load(&self) -> Result<Option<SessionMetadata>, PersistenceError> {
        let data = match std::fs::read_to_string(&self.path) {
            Ok(d) => d,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let meta: SessionMetadata = serde_json::from_str(&data)?;
        Ok(Some(meta))
    }

    /// Update the `last_active` timestamp in an existing on-disk session to
    /// the current time.
    pub fn touch(&self) -> Result<(), PersistenceError> {
        let Some(mut meta) = self.load()? else {
            return Ok(());
        };
        meta.last_active = now_secs();
        self.save(&meta)
    }

    /// Delete the session file. Silently succeeds if already absent.
    pub fn delete(&self) -> Result<(), PersistenceError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// Generate a 128-bit hex session identifier.
///
/// Uses a simple combination of timestamp + random bits; not a true UUID
/// library to avoid extra dependencies.
fn generate_session_id() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let ts = now_secs();
    let hasher_state = RandomState::new();
    let mut hasher = hasher_state.build_hasher();
    hasher.write_u64(ts);
    let rand_bits = hasher.finish();
    format!("{ts:016x}{rand_bits:016x}")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn create_session_generates_unique_ids() {
        let a = SessionPersistence::create_session();
        let b = SessionPersistence::create_session();
        assert_ne!(a.session_id, b.session_id);
    }

    #[test]
    fn create_session_sets_timestamps() {
        let meta = SessionPersistence::create_session();
        assert!(meta.start_time > 0);
        assert_eq!(meta.start_time, meta.last_active);
    }

    #[test]
    fn save_load_round_trip() {
        let dir = TempDir::new().unwrap();
        let p = SessionPersistence::new(dir.path().join("session.json"));

        let mut meta = SessionPersistence::create_session();
        meta.simulator = Some("MSFS".into());
        meta.aircraft = Some("C172".into());
        meta.profile = Some("default".into());
        meta.devices = vec!["stick-1".into(), "throttle-1".into()];

        p.save(&meta).unwrap();
        let loaded = p.load().unwrap().expect("should exist");
        assert_eq!(meta, loaded);
    }

    #[test]
    fn load_missing_returns_none() {
        let dir = TempDir::new().unwrap();
        let p = SessionPersistence::new(dir.path().join("nope.json"));
        assert!(p.load().unwrap().is_none());
    }

    #[test]
    fn metadata_fields_persist() {
        let dir = TempDir::new().unwrap();
        let p = SessionPersistence::new(dir.path().join("session.json"));

        let mut meta = SessionPersistence::create_session();
        meta.simulator = Some("XPlane".into());
        meta.aircraft = Some("A320".into());
        meta.profile = Some("airliner".into());
        meta.devices = vec!["rudder-1".into()];

        p.save(&meta).unwrap();
        let loaded = p.load().unwrap().unwrap();
        assert_eq!(loaded.simulator.as_deref(), Some("XPlane"));
        assert_eq!(loaded.aircraft.as_deref(), Some("A320"));
        assert_eq!(loaded.profile.as_deref(), Some("airliner"));
        assert_eq!(loaded.devices, vec!["rudder-1".to_string()]);
    }

    #[test]
    fn touch_updates_last_active() {
        let dir = TempDir::new().unwrap();
        let p = SessionPersistence::new(dir.path().join("session.json"));

        let mut meta = SessionPersistence::create_session();
        meta.last_active = 1_000_000;
        p.save(&meta).unwrap();

        // Touch should update last_active to current time.
        p.touch().unwrap();
        let loaded = p.load().unwrap().unwrap();
        assert!(loaded.last_active > 1_000_000);
    }

    #[test]
    fn touch_missing_file_is_noop() {
        let dir = TempDir::new().unwrap();
        let p = SessionPersistence::new(dir.path().join("nope.json"));
        // Should not error.
        p.touch().unwrap();
    }

    #[test]
    fn delete_removes_file() {
        let dir = TempDir::new().unwrap();
        let p = SessionPersistence::new(dir.path().join("session.json"));
        let meta = SessionPersistence::create_session();
        p.save(&meta).unwrap();
        assert!(p.path().exists());

        p.delete().unwrap();
        assert!(!p.path().exists());
    }

    #[test]
    fn delete_missing_is_ok() {
        let dir = TempDir::new().unwrap();
        let p = SessionPersistence::new(dir.path().join("nope.json"));
        p.delete().unwrap();
    }

    #[test]
    fn session_id_is_32_hex_chars() {
        let id = generate_session_id();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("a").join("b").join("session.json");
        let p = SessionPersistence::new(nested);
        let meta = SessionPersistence::create_session();
        p.save(&meta).unwrap();
        assert!(p.path().exists());
    }
}
