// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile hot-reload via file system polling.
//!
//! Monitors a profile directory for changes and provides a mechanism
//! to notify the service when profiles need to be reloaded.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// Represents a file change event.
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub path: PathBuf,
    pub kind: FileChangeKind,
}

/// Type of file change detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChangeKind {
    Modified,
    Created,
    Deleted,
}

/// Polls a directory for file changes.
///
/// Compares file modification timestamps on each poll to detect changes.
/// This is suitable for configuration directories where files change infrequently.
pub struct ProfileWatcher {
    watch_dir: PathBuf,
    known_files: HashMap<PathBuf, SystemTime>,
    poll_interval: Duration,
}

impl ProfileWatcher {
    /// Create a new watcher for the given directory.
    pub fn new(watch_dir: PathBuf, poll_interval: Duration) -> Self {
        Self {
            watch_dir,
            known_files: HashMap::new(),
            poll_interval,
        }
    }

    /// Create with default 1-second poll interval.
    pub fn with_default_interval(watch_dir: PathBuf) -> Self {
        Self::new(watch_dir, Duration::from_secs(1))
    }

    /// Poll for changes since last call. Returns list of changed files.
    pub fn poll(&mut self) -> Vec<FileChangeEvent> {
        let mut events = Vec::new();

        // Scan directory for current files
        let mut current = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(&self.watch_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "yaml" || e == "toml")
                    && let Ok(meta) = entry.metadata()
                    && let Ok(modified) = meta.modified()
                {
                    current.insert(path, modified);
                }
            }
        }

        // Detect new and modified files
        for (path, mtime) in &current {
            match self.known_files.get(path) {
                None => events.push(FileChangeEvent {
                    path: path.clone(),
                    kind: FileChangeKind::Created,
                }),
                Some(old_mtime) if old_mtime != mtime => events.push(FileChangeEvent {
                    path: path.clone(),
                    kind: FileChangeKind::Modified,
                }),
                _ => {}
            }
        }

        // Detect deleted files
        for path in self.known_files.keys() {
            if !current.contains_key(path) {
                events.push(FileChangeEvent {
                    path: path.clone(),
                    kind: FileChangeKind::Deleted,
                });
            }
        }

        self.known_files = current;
        events
    }

    pub fn watch_dir(&self) -> &Path {
        &self.watch_dir
    }

    pub fn poll_interval(&self) -> Duration {
        self.poll_interval
    }
}

/// Thread-safe wrapper for profile reload notifications.
///
/// Can be shared between the watcher thread and the service thread.
#[derive(Clone)]
pub struct ReloadNotifier {
    pending: Arc<Mutex<Vec<PathBuf>>>,
}

impl ReloadNotifier {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Signal that a path needs reload.
    pub fn notify(&self, path: PathBuf) {
        if let Ok(mut p) = self.pending.lock()
            && !p.contains(&path)
        {
            p.push(path);
        }
    }

    /// Drain pending reload notifications.
    pub fn drain(&self) -> Vec<PathBuf> {
        if let Ok(mut p) = self.pending.lock() {
            std::mem::take(&mut *p)
        } else {
            Vec::new()
        }
    }

    pub fn has_pending(&self) -> bool {
        self.pending.lock().map(|p| !p.is_empty()).unwrap_or(false)
    }
}

impl Default for ReloadNotifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    fn temp_dir_for_test(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("openflight_watcher_test_{name}"));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn test_empty_dir_no_events() {
        let dir = temp_dir_for_test("empty");
        let mut watcher = ProfileWatcher::with_default_interval(dir.clone());
        let events = watcher.poll();
        assert!(events.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_new_file_detected_as_created() {
        let dir = temp_dir_for_test("created");
        let mut watcher = ProfileWatcher::with_default_interval(dir.clone());
        watcher.poll(); // initial scan

        // Create a file
        let file = dir.join("test.yaml");
        fs::write(&file, "test: true").unwrap();

        let events = watcher.poll();
        assert!(
            events
                .iter()
                .any(|e| e.kind == FileChangeKind::Created && e.path == file)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_deleted_file_detected() {
        let dir = temp_dir_for_test("deleted");
        let file = dir.join("test.yaml");
        fs::write(&file, "test: true").unwrap();

        let mut watcher = ProfileWatcher::with_default_interval(dir.clone());
        watcher.poll(); // initial scan registers the file

        fs::remove_file(&file).unwrap();
        let events = watcher.poll();
        assert!(
            events
                .iter()
                .any(|e| e.kind == FileChangeKind::Deleted && e.path == file)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_nonexistent_dir_no_panic() {
        let dir = PathBuf::from("/nonexistent/openflight_watcher");
        let mut watcher = ProfileWatcher::with_default_interval(dir);
        let events = watcher.poll();
        assert!(events.is_empty());
    }

    #[test]
    fn test_watcher_watch_dir() {
        let dir = temp_dir_for_test("dir_getter");
        let watcher = ProfileWatcher::with_default_interval(dir.clone());
        assert_eq!(watcher.watch_dir(), dir.as_path());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_notifier_notify_and_drain() {
        let notifier = ReloadNotifier::new();
        assert!(!notifier.has_pending());
        notifier.notify(PathBuf::from("test.yaml"));
        assert!(notifier.has_pending());
        let drained = notifier.drain();
        assert_eq!(drained.len(), 1);
        assert!(!notifier.has_pending());
    }

    #[test]
    fn test_notifier_deduplicates() {
        let notifier = ReloadNotifier::new();
        notifier.notify(PathBuf::from("same.yaml"));
        notifier.notify(PathBuf::from("same.yaml"));
        let drained = notifier.drain();
        assert_eq!(drained.len(), 1);
    }

    #[test]
    fn test_notifier_clone_shares_state() {
        let n1 = ReloadNotifier::new();
        let n2 = n1.clone();
        n1.notify(PathBuf::from("foo.yaml"));
        assert!(n2.has_pending());
    }
}
