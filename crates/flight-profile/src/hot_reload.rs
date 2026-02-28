// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Hot-reload tracker for profile files.
//!
//! Pure logic module — detects changes by comparing [`FileState`] snapshots.
//! No actual filesystem watching is performed here.

use std::collections::HashMap;

/// Snapshot of a tracked file's metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileState {
    pub path: String,
    pub hash: u64,
    pub last_modified: u64,
    pub size: u64,
}

/// Action to take after a change-detection check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadAction {
    /// Nothing changed.
    None,
    /// File content changed — reload it.
    Reload(String),
    /// File was removed — untrack it.
    Remove(String),
    /// Something went wrong with this path.
    Error(String),
}

/// Tracks file states and emits [`ReloadAction`]s on detected changes.
pub struct HotReloadTracker {
    tracked: HashMap<String, FileState>,
    debounce_ms: u64,
    last_check_ms: u64,
}

impl HotReloadTracker {
    /// Create a new tracker with the given debounce interval.
    pub fn new(debounce_ms: u64) -> Self {
        Self {
            tracked: HashMap::new(),
            debounce_ms,
            last_check_ms: 0,
        }
    }

    /// Start tracking a file.
    pub fn track(&mut self, path: String, state: FileState) {
        self.tracked.insert(path, state);
    }

    /// Stop tracking a file.
    pub fn untrack(&mut self, path: &str) {
        self.tracked.remove(path);
    }

    /// Number of currently tracked files.
    pub fn tracked_count(&self) -> usize {
        self.tracked.len()
    }

    /// Returns `true` if `now_ms` falls within the debounce window since the
    /// last check.
    pub fn is_debouncing(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_check_ms) < self.debounce_ms
    }

    /// Compare `current_states` against stored states and return actions.
    ///
    /// If the debounce window has not elapsed, returns an empty list.
    /// Files present in the tracker but absent from `current_states` produce
    /// [`ReloadAction::Remove`]. Changed hashes produce [`ReloadAction::Reload`].
    pub fn check_changes(
        &mut self,
        current_states: &[FileState],
        now_ms: u64,
    ) -> Vec<ReloadAction> {
        if self.is_debouncing(now_ms) {
            return Vec::new();
        }
        self.last_check_ms = now_ms;

        let current_map: HashMap<&str, &FileState> = current_states
            .iter()
            .map(|s| (s.path.as_str(), s))
            .collect();

        let mut actions = Vec::new();

        // Check existing tracked files for changes or removal.
        for (path, old_state) in &self.tracked {
            match current_map.get(path.as_str()) {
                Some(new_state) => {
                    if new_state.hash != old_state.hash {
                        actions.push(ReloadAction::Reload(path.clone()));
                    }
                }
                None => {
                    actions.push(ReloadAction::Remove(path.clone()));
                }
            }
        }

        // Apply removals to the tracker.
        for action in &actions {
            if let ReloadAction::Remove(path) = action {
                self.tracked.remove(path);
            }
        }

        // Update stored states for reloaded files.
        for state in current_states {
            if self.tracked.contains_key(&state.path) {
                self.tracked.insert(state.path.clone(), state.clone());
            }
        }

        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(path: &str, hash: u64) -> FileState {
        FileState {
            path: path.to_string(),
            hash,
            last_modified: 1000,
            size: 100,
        }
    }

    #[test]
    fn no_change_returns_none() {
        let mut tracker = HotReloadTracker::new(100);
        tracker.track("a.json".into(), state("a.json", 1));
        let actions = tracker.check_changes(&[state("a.json", 1)], 200);
        assert!(actions.is_empty());
    }

    #[test]
    fn hash_change_triggers_reload() {
        let mut tracker = HotReloadTracker::new(100);
        tracker.track("a.json".into(), state("a.json", 1));
        let actions = tracker.check_changes(&[state("a.json", 2)], 200);
        assert_eq!(actions, vec![ReloadAction::Reload("a.json".into())]);
    }

    #[test]
    fn missing_file_triggers_remove() {
        let mut tracker = HotReloadTracker::new(100);
        tracker.track("a.json".into(), state("a.json", 1));
        let actions = tracker.check_changes(&[], 200);
        assert_eq!(actions, vec![ReloadAction::Remove("a.json".into())]);
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn debounce_suppresses_check() {
        let mut tracker = HotReloadTracker::new(100);
        tracker.track("a.json".into(), state("a.json", 1));

        // First check at t=200
        let _ = tracker.check_changes(&[state("a.json", 2)], 200);

        // Second check within debounce window at t=250 should be suppressed
        tracker.track("a.json".into(), state("a.json", 2)); // reset state
        let actions = tracker.check_changes(&[state("a.json", 3)], 250);
        assert!(actions.is_empty());
    }

    #[test]
    fn after_debounce_window_check_proceeds() {
        let mut tracker = HotReloadTracker::new(100);
        tracker.track("a.json".into(), state("a.json", 1));
        let _ = tracker.check_changes(&[state("a.json", 1)], 200);

        // After debounce window (200 + 100 = 300)
        let actions = tracker.check_changes(&[state("a.json", 2)], 300);
        assert_eq!(actions, vec![ReloadAction::Reload("a.json".into())]);
    }

    #[test]
    fn track_and_untrack() {
        let mut tracker = HotReloadTracker::new(100);
        tracker.track("a.json".into(), state("a.json", 1));
        assert_eq!(tracker.tracked_count(), 1);
        tracker.untrack("a.json");
        assert_eq!(tracker.tracked_count(), 0);
    }

    #[test]
    fn multiple_files_changed() {
        let mut tracker = HotReloadTracker::new(0);
        tracker.track("a.json".into(), state("a.json", 1));
        tracker.track("b.json".into(), state("b.json", 10));
        tracker.track("c.json".into(), state("c.json", 100));

        let current = vec![
            state("a.json", 2),  // changed
            state("b.json", 10), // unchanged
            state("c.json", 99), // changed
        ];
        let mut actions = tracker.check_changes(&current, 100);
        actions.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));

        assert_eq!(actions.len(), 2);
        assert!(actions.contains(&ReloadAction::Reload("a.json".into())));
        assert!(actions.contains(&ReloadAction::Reload("c.json".into())));
    }

    #[test]
    fn is_debouncing_logic() {
        let mut tracker = HotReloadTracker::new(50);
        // Fresh tracker with last_check_ms=0 debounces at t=0 (0 < 50).
        assert!(tracker.is_debouncing(0));
        // But not once debounce window passes.
        assert!(!tracker.is_debouncing(50));
        tracker.last_check_ms = 100;
        assert!(tracker.is_debouncing(130));
        assert!(!tracker.is_debouncing(150));
        assert!(!tracker.is_debouncing(200));
    }

    #[test]
    fn new_file_not_in_tracker_is_ignored() {
        let mut tracker = HotReloadTracker::new(0);
        tracker.track("a.json".into(), state("a.json", 1));
        let current = vec![
            state("a.json", 1),
            state("new.json", 42), // not tracked
        ];
        let actions = tracker.check_changes(&current, 100);
        assert!(actions.is_empty());
        // The new file should NOT be auto-added
        assert_eq!(tracker.tracked_count(), 1);
    }

    #[test]
    fn reload_updates_stored_state() {
        let mut tracker = HotReloadTracker::new(0);
        tracker.track("a.json".into(), state("a.json", 1));

        // First check: detect change
        let actions = tracker.check_changes(&[state("a.json", 2)], 100);
        assert_eq!(actions, vec![ReloadAction::Reload("a.json".into())]);

        // Second check with same state: no change
        let actions = tracker.check_changes(&[state("a.json", 2)], 200);
        assert!(actions.is_empty());
    }

    #[test]
    fn zero_debounce_always_checks() {
        let mut tracker = HotReloadTracker::new(0);
        tracker.track("a.json".into(), state("a.json", 1));

        let actions = tracker.check_changes(&[state("a.json", 2)], 100);
        assert_eq!(actions.len(), 1);

        let actions = tracker.check_changes(&[state("a.json", 3)], 100);
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn remove_then_re_track() {
        let mut tracker = HotReloadTracker::new(0);
        tracker.track("a.json".into(), state("a.json", 1));

        // Remove via absence
        let actions = tracker.check_changes(&[], 100);
        assert_eq!(actions, vec![ReloadAction::Remove("a.json".into())]);
        assert_eq!(tracker.tracked_count(), 0);

        // Re-track
        tracker.track("a.json".into(), state("a.json", 5));
        assert_eq!(tracker.tracked_count(), 1);

        let actions = tracker.check_changes(&[state("a.json", 5)], 200);
        assert!(actions.is_empty());
    }
}
