// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Session state persistence for resumption after restart.
//!
//! Provides [`SessionState`] snapshots and JSON serialization so that
//! the active profile, aircraft, sim selection, device configs and user
//! preferences survive a service restart.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Serializable session state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionState {
    pub session_id: String,
    pub active_profile: Option<String>,
    pub active_aircraft: Option<String>,
    pub active_sim: Option<String>,
    pub device_configs: HashMap<String, String>,
    pub preferences: HashMap<String, String>,
    pub last_save_timestamp: u64,
}

/// Manages session state snapshots.
pub struct StatePersistence {
    current_state: SessionState,
    snapshots: Vec<SessionState>,
    max_snapshots: usize,
    dirty: bool,
}

impl StatePersistence {
    /// Create a new persistence manager for the given session.
    pub fn new(session_id: &str, max_snapshots: usize) -> Self {
        Self {
            current_state: SessionState {
                session_id: session_id.to_owned(),
                active_profile: None,
                active_aircraft: None,
                active_sim: None,
                device_configs: HashMap::new(),
                preferences: HashMap::new(),
                last_save_timestamp: 0,
            },
            snapshots: Vec::new(),
            max_snapshots,
            dirty: false,
        }
    }

    pub fn set_profile(&mut self, profile: &str) {
        self.current_state.active_profile = Some(profile.to_owned());
        self.dirty = true;
    }

    pub fn set_aircraft(&mut self, aircraft: &str) {
        self.current_state.active_aircraft = Some(aircraft.to_owned());
        self.dirty = true;
    }

    pub fn set_sim(&mut self, sim: &str) {
        self.current_state.active_sim = Some(sim.to_owned());
        self.dirty = true;
    }

    pub fn set_device_config(&mut self, device_id: &str, config: &str) {
        self.current_state
            .device_configs
            .insert(device_id.to_owned(), config.to_owned());
        self.dirty = true;
    }

    pub fn set_preference(&mut self, key: &str, value: &str) {
        self.current_state
            .preferences
            .insert(key.to_owned(), value.to_owned());
        self.dirty = true;
    }

    pub fn get_preference(&self, key: &str) -> Option<&str> {
        self.current_state.preferences.get(key).map(|s| s.as_str())
    }

    /// Save the current state as a snapshot.
    ///
    /// The timestamp is set to the current Unix epoch seconds.
    /// If the number of snapshots exceeds `max_snapshots`, the oldest is dropped.
    pub fn snapshot(&mut self) {
        let mut snap = self.current_state.clone();
        snap.last_save_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.snapshots.push(snap);
        if self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
        }
    }

    /// Restore the most recent snapshot, removing it from the list.
    pub fn restore_latest(&mut self) -> Option<SessionState> {
        let snap = self.snapshots.pop()?;
        self.current_state = snap.clone();
        self.dirty = false;
        Some(snap)
    }

    /// Return a reference to a snapshot by index (0 = oldest).
    pub fn restore_by_index(&self, index: usize) -> Option<&SessionState> {
        self.snapshots.get(index)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Serialize the **current** state to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.current_state)
            .expect("SessionState serialization is infallible")
    }

    /// Deserialize a `SessionState` from JSON.
    pub fn from_json(json: &str) -> Result<SessionState, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_has_empty_state() {
        let sp = StatePersistence::new("s1", 5);
        assert_eq!(sp.current_state.session_id, "s1");
        assert!(sp.current_state.active_profile.is_none());
        assert!(sp.current_state.active_aircraft.is_none());
        assert!(sp.current_state.active_sim.is_none());
        assert!(sp.current_state.device_configs.is_empty());
        assert!(sp.current_state.preferences.is_empty());
        assert_eq!(sp.snapshot_count(), 0);
        assert!(!sp.is_dirty());
    }

    #[test]
    fn set_profile_aircraft_sim() {
        let mut sp = StatePersistence::new("s1", 5);
        sp.set_profile("default");
        sp.set_aircraft("C172");
        sp.set_sim("MSFS");
        assert_eq!(sp.current_state.active_profile.as_deref(), Some("default"));
        assert_eq!(sp.current_state.active_aircraft.as_deref(), Some("C172"));
        assert_eq!(sp.current_state.active_sim.as_deref(), Some("MSFS"));
    }

    #[test]
    fn device_configs_stored() {
        let mut sp = StatePersistence::new("s1", 5);
        sp.set_device_config("stick-1", r#"{"axes":2}"#);
        sp.set_device_config("throttle-1", r#"{"axes":4}"#);
        assert_eq!(sp.current_state.device_configs.len(), 2);
        assert_eq!(
            sp.current_state.device_configs.get("stick-1").unwrap(),
            r#"{"axes":2}"#
        );
    }

    #[test]
    fn preferences_stored_and_retrieved() {
        let mut sp = StatePersistence::new("s1", 5);
        sp.set_preference("theme", "dark");
        assert_eq!(sp.get_preference("theme"), Some("dark"));
        assert_eq!(sp.get_preference("missing"), None);
    }

    #[test]
    fn snapshot_saves_current_state() {
        let mut sp = StatePersistence::new("s1", 5);
        sp.set_profile("combat");
        sp.snapshot();
        assert_eq!(sp.snapshot_count(), 1);
        let snap = sp.restore_by_index(0).unwrap();
        assert_eq!(snap.active_profile.as_deref(), Some("combat"));
        assert!(snap.last_save_timestamp > 0);
    }

    #[test]
    fn restore_latest_returns_most_recent() {
        let mut sp = StatePersistence::new("s1", 5);
        sp.set_profile("a");
        sp.snapshot();
        sp.set_profile("b");
        sp.snapshot();
        let restored = sp.restore_latest().unwrap();
        assert_eq!(restored.active_profile.as_deref(), Some("b"));
        assert_eq!(sp.snapshot_count(), 1);
    }

    #[test]
    fn multiple_snapshots_tracked() {
        let mut sp = StatePersistence::new("s1", 5);
        for i in 0..3 {
            sp.set_preference("iter", &i.to_string());
            sp.snapshot();
        }
        assert_eq!(sp.snapshot_count(), 3);
        assert_eq!(
            sp.restore_by_index(0)
                .unwrap()
                .preferences
                .get("iter")
                .unwrap(),
            "0"
        );
        assert_eq!(
            sp.restore_by_index(2)
                .unwrap()
                .preferences
                .get("iter")
                .unwrap(),
            "2"
        );
    }

    #[test]
    fn max_snapshots_enforced() {
        let mut sp = StatePersistence::new("s1", 3);
        for i in 0..5 {
            sp.set_preference("v", &i.to_string());
            sp.snapshot();
        }
        assert_eq!(sp.snapshot_count(), 3);
        // Oldest two (0,1) were dropped; remaining are 2,3,4
        assert_eq!(
            sp.restore_by_index(0)
                .unwrap()
                .preferences
                .get("v")
                .unwrap(),
            "2"
        );
    }

    #[test]
    fn dirty_flag_tracks_modifications() {
        let mut sp = StatePersistence::new("s1", 5);
        assert!(!sp.is_dirty());
        sp.set_profile("x");
        assert!(sp.is_dirty());
    }

    #[test]
    fn mark_clean_resets_dirty() {
        let mut sp = StatePersistence::new("s1", 5);
        sp.set_profile("x");
        assert!(sp.is_dirty());
        sp.mark_clean();
        assert!(!sp.is_dirty());
    }

    #[test]
    fn json_round_trip() {
        let mut sp = StatePersistence::new("s1", 5);
        sp.set_profile("ga");
        sp.set_aircraft("A320");
        sp.set_sim("XPlane");
        sp.set_device_config("dev1", "{}");
        sp.set_preference("units", "metric");

        let json = sp.to_json();
        let restored = StatePersistence::from_json(&json).unwrap();
        assert_eq!(restored.session_id, "s1");
        assert_eq!(restored.active_profile.as_deref(), Some("ga"));
        assert_eq!(restored.active_aircraft.as_deref(), Some("A320"));
        assert_eq!(restored.active_sim.as_deref(), Some("XPlane"));
        assert_eq!(restored.device_configs.get("dev1").unwrap(), "{}");
        assert_eq!(restored.preferences.get("units").unwrap(), "metric");
    }

    #[test]
    fn invalid_json_returns_error() {
        let result = StatePersistence::from_json("not valid json");
        assert!(result.is_err());
    }
}
