// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Poll-based configuration file watcher (REQ-711).
//!
//! Periodically checks a config file for modifications (by mtime and size)
//! and sends a [`ConfigChange`] event through a channel when changes are
//! detected.

use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::debug;

/// Describes a detected configuration file change.
#[derive(Debug, Clone)]
pub struct ConfigChange {
    /// Path of the changed file.
    pub path: PathBuf,
    /// Timestamp of the change detection (monotonic instant).
    pub detected_at: std::time::Instant,
}

/// Metadata snapshot used for change detection.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSnapshot {
    modified: std::time::SystemTime,
    size: u64,
}

impl FileSnapshot {
    fn capture(path: &Path) -> std::io::Result<Self> {
        let meta = std::fs::metadata(path)?;
        Ok(Self {
            modified: meta.modified()?,
            size: meta.len(),
        })
    }
}

/// Watches a configuration file for changes using a polling strategy.
pub struct ConfigWatcher {
    path: PathBuf,
    interval: Duration,
}

impl ConfigWatcher {
    /// Create a new watcher for the given path with the specified poll interval.
    pub fn new(path: impl Into<PathBuf>, interval: Duration) -> Self {
        Self {
            path: path.into(),
            interval,
        }
    }

    /// Start watching. Returns a receiver that yields [`ConfigChange`] events.
    ///
    /// The watcher runs as a background tokio task and stops when the returned
    /// receiver is dropped.
    pub fn watch(&self) -> mpsc::Receiver<ConfigChange> {
        let (tx, rx) = mpsc::channel(16);
        let path = self.path.clone();
        let interval = self.interval;

        tokio::spawn(async move {
            let mut last_snapshot = FileSnapshot::capture(&path).ok();

            loop {
                tokio::time::sleep(interval).await;

                let current = match FileSnapshot::capture(&path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                let changed = match &last_snapshot {
                    Some(prev) => *prev != current,
                    None => true,
                };

                if changed {
                    debug!(path = %path.display(), "config file change detected");
                    let event = ConfigChange {
                        path: path.clone(),
                        detected_at: std::time::Instant::now(),
                    };
                    if tx.send(event).await.is_err() {
                        // Receiver dropped — stop watching.
                        break;
                    }
                    last_snapshot = Some(current);
                }
            }
        });

        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tokio::time::timeout;

    #[tokio::test]
    async fn detect_file_modification() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("config.json");
        std::fs::write(&file_path, r#"{"v":1}"#).unwrap();

        let watcher = ConfigWatcher::new(&file_path, Duration::from_millis(20));
        let mut rx = watcher.watch();

        // Allow initial snapshot to be captured.
        tokio::time::sleep(Duration::from_millis(30)).await;

        // Modify the file.
        {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&file_path)
                .unwrap();
            write!(f, r#"{{"v":2}}"#).unwrap();
        }

        let change = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for change")
            .expect("channel closed");
        assert_eq!(change.path, file_path);
    }

    #[tokio::test]
    async fn no_false_trigger_on_unchanged_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("stable.json");
        std::fs::write(&file_path, r#"{"stable":true}"#).unwrap();

        let watcher = ConfigWatcher::new(&file_path, Duration::from_millis(20));
        let mut rx = watcher.watch();

        // Wait for several poll cycles without modifying the file.
        let result = timeout(Duration::from_millis(150), rx.recv()).await;
        assert!(result.is_err(), "should not receive an event for unchanged file");
    }
}
