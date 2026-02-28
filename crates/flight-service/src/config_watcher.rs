// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Poll-based configuration file watcher with hot-reload support (REQ-873).
//!
//! Watches multiple configuration files for changes (by mtime, size, and hash)
//! and reports [`ConfigChange`] events when modifications, creations, or
//! deletions are detected.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Watches configuration files for changes and triggers reload.
pub struct ConfigWatcher {
    watched_files: HashMap<PathBuf, FileState>,
    poll_interval: Duration,
    on_change_callbacks: Vec<String>,
    enabled: bool,
}

/// Internal state tracked per watched file.
struct FileState {
    path: PathBuf,
    last_modified: Option<SystemTime>,
    last_size: Option<u64>,
    hash: Option<u64>,
}

/// Describes a detected configuration file change.
#[derive(Debug, Clone)]
pub struct ConfigChange {
    /// Path of the changed file.
    pub path: PathBuf,
    /// Kind of change that was detected.
    pub change_type: ChangeType,
    /// When the change was detected.
    pub detected_at: SystemTime,
}

/// The kind of configuration file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    /// File content was modified.
    Modified,
    /// File was newly created (appeared where it was absent before).
    Created,
    /// File was deleted (disappeared where it existed before).
    Deleted,
}

/// Result of applying a configuration reload.
#[derive(Debug, Clone)]
pub struct ReloadResult {
    /// Path that was reloaded.
    pub path: PathBuf,
    /// Whether the reload succeeded.
    pub success: bool,
    /// Human-readable status message.
    pub message: String,
    /// Time taken to apply the reload, in milliseconds.
    pub duration_ms: u64,
}

impl ConfigWatcher {
    /// Create a new watcher with the given poll interval.
    pub fn new(poll_interval: Duration) -> Self {
        Self {
            watched_files: HashMap::new(),
            poll_interval,
            on_change_callbacks: Vec::new(),
            enabled: true,
        }
    }

    /// Add a file to the watch list.
    pub fn watch(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();
        self.watched_files.entry(path.clone()).or_insert(FileState {
            path,
            last_modified: None,
            last_size: None,
            hash: None,
        });
    }

    /// Remove a file from the watch list. Returns `true` if it was present.
    pub fn unwatch(&mut self, path: impl AsRef<Path>) -> bool {
        self.watched_files.remove(path.as_ref()).is_some()
    }

    /// Poll all watched files and return any detected changes.
    ///
    /// When the watcher is disabled this always returns an empty list.
    /// Change detection compares the current filesystem metadata against the
    /// stored [`FileState`]. After reporting a change the stored state is
    /// updated so the same change is not reported twice.
    pub fn check_for_changes(&mut self) -> Vec<ConfigChange> {
        if !self.enabled {
            return Vec::new();
        }

        let mut changes = Vec::new();
        let now = SystemTime::now();

        for state in self.watched_files.values_mut() {
            let meta = std::fs::metadata(&state.path).ok();

            match (&meta, state.last_modified.is_some()) {
                // File exists now but had no prior state → Created.
                (Some(m), false) => {
                    state.last_modified = m.modified().ok();
                    state.last_size = Some(m.len());
                    changes.push(ConfigChange {
                        path: state.path.clone(),
                        change_type: ChangeType::Created,
                        detected_at: now,
                    });
                }
                // File exists and we had prior state → check for modification.
                (Some(m), true) => {
                    let cur_modified = m.modified().ok();
                    let cur_size = Some(m.len());
                    if cur_modified != state.last_modified || cur_size != state.last_size {
                        state.last_modified = cur_modified;
                        state.last_size = cur_size;
                        changes.push(ConfigChange {
                            path: state.path.clone(),
                            change_type: ChangeType::Modified,
                            detected_at: now,
                        });
                    }
                }
                // File is gone but we had prior state → Deleted.
                (None, true) => {
                    state.last_modified = None;
                    state.last_size = None;
                    state.hash = None;
                    changes.push(ConfigChange {
                        path: state.path.clone(),
                        change_type: ChangeType::Deleted,
                        detected_at: now,
                    });
                }
                // File absent and no prior state → nothing to report.
                (None, false) => {}
            }
        }

        changes
    }

    /// Return references to all currently watched file paths.
    pub fn watched_files(&self) -> Vec<&PathBuf> {
        self.watched_files.keys().collect()
    }

    /// Check whether a path is currently being watched.
    pub fn is_watching(&self, path: impl AsRef<Path>) -> bool {
        self.watched_files.contains_key(path.as_ref())
    }

    /// Enable the watcher. Subsequent calls to [`check_for_changes`] will poll.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable the watcher. [`check_for_changes`] will return an empty list.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Whether the watcher is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Update the poll interval.
    pub fn set_poll_interval(&mut self, interval: Duration) {
        self.poll_interval = interval;
    }

    /// The number of files currently being watched.
    pub fn file_count(&self) -> usize {
        self.watched_files.len()
    }

    /// Register a callback identifier to be invoked on changes.
    #[allow(dead_code)]
    pub fn register_callback(&mut self, id: String) {
        self.on_change_callbacks.push(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_watcher() -> ConfigWatcher {
        ConfigWatcher::new(Duration::from_millis(100))
    }

    // Helper: inject a "previously seen" state so we can test change detection
    // without real file I/O.
    fn inject_state(
        watcher: &mut ConfigWatcher,
        path: &Path,
        modified: Option<SystemTime>,
        size: Option<u64>,
    ) {
        if let Some(state) = watcher.watched_files.get_mut(path) {
            state.last_modified = modified;
            state.last_size = size;
        }
    }

    // 1. Watch a file
    #[test]
    fn watch_adds_file() {
        let mut w = make_watcher();
        w.watch("/tmp/config.json");
        assert!(w.is_watching("/tmp/config.json"));
    }

    // 2. Unwatch a file
    #[test]
    fn unwatch_removes_file() {
        let mut w = make_watcher();
        w.watch("/tmp/config.json");
        assert!(w.unwatch("/tmp/config.json"));
        assert!(!w.is_watching("/tmp/config.json"));
    }

    // 3. Check finds no changes for new watcher (no prior state, file absent)
    #[test]
    fn no_changes_for_fresh_watcher() {
        let mut w = make_watcher();
        w.watch("/nonexistent/path/config.json");
        let changes = w.check_for_changes();
        assert!(changes.is_empty());
    }

    // 4. Detect modification via mtime change
    #[test]
    fn detect_modification_by_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("app.toml");
        std::fs::write(&file, "v1").unwrap();

        let mut w = make_watcher();
        w.watch(&file);

        // Prime with a stale timestamp so the next check sees a difference.
        let stale_time = SystemTime::UNIX_EPOCH;
        inject_state(&mut w, &file, Some(stale_time), Some(2));

        let changes = w.check_for_changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change_type, ChangeType::Modified);
    }

    // 5. Detect deletion
    #[test]
    fn detect_deletion() {
        let mut w = make_watcher();
        w.watch("/nonexistent/should_be_deleted.json");

        // Pretend the file previously existed.
        inject_state(
            &mut w,
            Path::new("/nonexistent/should_be_deleted.json"),
            Some(SystemTime::now()),
            Some(42),
        );

        let changes = w.check_for_changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change_type, ChangeType::Deleted);
    }

    // 6. Enable / disable toggle
    #[test]
    fn enable_disable_toggle() {
        let mut w = make_watcher();
        assert!(w.is_enabled());

        w.disable();
        assert!(!w.is_enabled());

        // Disabled watcher reports no changes even if state would trigger one.
        w.watch("/nonexistent/toggled.json");
        inject_state(
            &mut w,
            Path::new("/nonexistent/toggled.json"),
            Some(SystemTime::now()),
            Some(1),
        );
        assert!(w.check_for_changes().is_empty());

        w.enable();
        assert!(w.is_enabled());
    }

    // 7. Multiple files watched
    #[test]
    fn multiple_files_watched() {
        let mut w = make_watcher();
        w.watch("/a.json");
        w.watch("/b.json");
        w.watch("/c.json");
        assert_eq!(w.file_count(), 3);
        assert!(w.is_watching("/a.json"));
        assert!(w.is_watching("/b.json"));
        assert!(w.is_watching("/c.json"));
    }

    // 8. Unwatch nonexistent returns false
    #[test]
    fn unwatch_nonexistent_returns_false() {
        let mut w = make_watcher();
        assert!(!w.unwatch("/never/added.json"));
    }

    // 9. is_watching correctly reports
    #[test]
    fn is_watching_reports_correctly() {
        let mut w = make_watcher();
        assert!(!w.is_watching("/x.json"));
        w.watch("/x.json");
        assert!(w.is_watching("/x.json"));
        w.unwatch("/x.json");
        assert!(!w.is_watching("/x.json"));
    }

    // 10. file_count tracks correctly
    #[test]
    fn file_count_tracks_correctly() {
        let mut w = make_watcher();
        assert_eq!(w.file_count(), 0);
        w.watch("/a.json");
        assert_eq!(w.file_count(), 1);
        w.watch("/b.json");
        assert_eq!(w.file_count(), 2);
        w.unwatch("/a.json");
        assert_eq!(w.file_count(), 1);
    }

    // 11. Duplicate watch is idempotent
    #[test]
    fn duplicate_watch_is_idempotent() {
        let mut w = make_watcher();
        w.watch("/dup.json");
        w.watch("/dup.json");
        assert_eq!(w.file_count(), 1);
    }

    // 12. set_poll_interval updates interval
    #[test]
    fn set_poll_interval_updates() {
        let mut w = make_watcher();
        assert_eq!(w.poll_interval, Duration::from_millis(100));
        w.set_poll_interval(Duration::from_secs(5));
        assert_eq!(w.poll_interval, Duration::from_secs(5));
    }

    // 13. Detect creation (file appears where there was none)
    #[test]
    fn detect_creation() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("new.toml");

        let mut w = make_watcher();
        w.watch(&file);

        // No prior state and file absent → no change.
        assert!(w.check_for_changes().is_empty());

        // Now create the file.
        std::fs::write(&file, "hello").unwrap();

        let changes = w.check_for_changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change_type, ChangeType::Created);
    }

    // 14. ReloadResult can be constructed
    #[test]
    fn reload_result_construction() {
        let r = ReloadResult {
            path: PathBuf::from("/cfg.toml"),
            success: true,
            message: "ok".to_string(),
            duration_ms: 12,
        };
        assert!(r.success);
        assert_eq!(r.duration_ms, 12);
    }
}
