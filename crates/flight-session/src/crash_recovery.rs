// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Crash recovery with lock files and session journals.
//!
//! [`CrashRecovery`] detects incomplete sessions left behind by crashes
//! (via a lock-file pattern) and provides a journal of atomic writes so
//! the most recent consistent state can be recovered.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const LOCK_FILE: &str = "session.lock";
const JOURNAL_FILE: &str = "session.journal";

/// A single journal entry written atomically before a state change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JournalEntry {
    /// Monotonic sequence number.
    pub seq: u64,
    /// Unix epoch seconds when the entry was written.
    pub timestamp: u64,
    /// What operation was about to happen.
    pub operation: String,
    /// Arbitrary key-value payload.
    pub payload: std::collections::HashMap<String, String>,
}

/// Summary produced after inspecting an incomplete session.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryReport {
    /// Whether a lock file was found (i.e. the previous session did not shut
    /// down cleanly).
    pub was_incomplete: bool,
    /// PID recorded in the lock file, if parseable.
    pub last_pid: Option<u32>,
    /// Epoch seconds from the lock file, if parseable.
    pub lock_timestamp: Option<u64>,
    /// Journal entries that can be replayed/inspected.
    pub journal_entries: Vec<JournalEntry>,
}

/// Errors produced by [`CrashRecovery`].
#[derive(Debug, thiserror::Error)]
pub enum CrashRecoveryError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("journal parse error: {0}")]
    JournalParse(#[from] serde_json::Error),
}

/// Lock-file + journal based crash recovery.
pub struct CrashRecovery {
    dir: PathBuf,
    next_seq: u64,
}

impl CrashRecovery {
    /// Create a recovery manager that stores files in `dir`.
    ///
    /// Scans any existing journal to resume sequence numbering.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let next_seq = Self::init_next_seq(&dir);
        Self { dir, next_seq }
    }

    /// The state directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    // ── Lock file ────────────────────────────────────────────────────────

    /// Scan an existing journal for the highest sequence number.
    fn init_next_seq(dir: &Path) -> u64 {
        let journal_path = dir.join(JOURNAL_FILE);
        let data = match std::fs::read_to_string(&journal_path) {
            Ok(d) => d,
            Err(_) => return 1,
        };
        let mut max_seq = 0u64;
        for line in data.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<JournalEntry>(line) {
                max_seq = max_seq.max(entry.seq);
            }
        }
        max_seq + 1
    }

    fn lock_path(&self) -> PathBuf {
        self.dir.join(LOCK_FILE)
    }

    fn journal_path(&self) -> PathBuf {
        self.dir.join(JOURNAL_FILE)
    }

    /// Acquire the session lock. Writes the current PID and timestamp.
    ///
    /// Fails with [`io::ErrorKind::AlreadyExists`] if a lock file is already
    /// present (i.e. another session is running or a previous one crashed).
    pub fn acquire_lock(&self) -> Result<(), CrashRecoveryError> {
        std::fs::create_dir_all(&self.dir)?;
        let content = format!("{}:{}", std::process::id(), now_secs());
        let mut file = std::fs::File::create_new(self.lock_path())?;
        {
            use std::io::Write;
            file.write_all(content.as_bytes())?;
        }
        Ok(())
    }

    /// Release the session lock (clean shutdown).
    pub fn release_lock(&self) -> Result<(), CrashRecoveryError> {
        match std::fs::remove_file(self.lock_path()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Returns `true` if a lock file exists (session did not shut down
    /// cleanly).
    pub fn has_lock(&self) -> bool {
        self.lock_path().exists()
    }

    /// Parse the lock file into (pid, timestamp).
    fn parse_lock(&self) -> Result<(Option<u32>, Option<u64>), CrashRecoveryError> {
        let content = match std::fs::read_to_string(self.lock_path()) {
            Ok(c) => c,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((None, None)),
            Err(e) => return Err(e.into()),
        };
        let mut parts = content.trim().split(':');
        let pid = parts.next().and_then(|s| s.parse::<u32>().ok());
        let ts = parts.next().and_then(|s| s.parse::<u64>().ok());
        Ok((pid, ts))
    }

    // ── Journal ──────────────────────────────────────────────────────────

    /// Append a journal entry. Each entry is a single JSON line.
    pub fn write_journal(
        &mut self,
        operation: impl Into<String>,
        payload: std::collections::HashMap<String, String>,
    ) -> Result<JournalEntry, CrashRecoveryError> {
        std::fs::create_dir_all(&self.dir)?;

        let entry = JournalEntry {
            seq: self.next_seq,
            timestamp: now_secs(),
            operation: operation.into(),
            payload,
        };
        self.next_seq += 1;

        let mut line = serde_json::to_string(&entry)?;
        line.push('\n');

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.journal_path())?;
        let n = file.write(line.as_bytes())?;
        if n < line.len() {
            return Err(CrashRecoveryError::Io(io::Error::new(
                io::ErrorKind::WriteZero,
                "short write to journal",
            )));
        }
        file.sync_data()?;

        Ok(entry)
    }

    /// Read all journal entries.
    pub fn read_journal(&self) -> Result<Vec<JournalEntry>, CrashRecoveryError> {
        let data = match std::fs::read_to_string(self.journal_path()) {
            Ok(d) => d,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };

        let mut entries = Vec::new();
        for line in data.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let entry: JournalEntry = serde_json::from_str(line)?;
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Remove the journal file (e.g. after successful recovery).
    pub fn clear_journal(&self) -> Result<(), CrashRecoveryError> {
        match std::fs::remove_file(self.journal_path()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    // ── Recovery report ──────────────────────────────────────────────────

    /// Inspect the state directory and produce a [`RecoveryReport`].
    pub fn inspect(&self) -> Result<RecoveryReport, CrashRecoveryError> {
        let was_incomplete = self.has_lock();
        let (last_pid, lock_timestamp) = self.parse_lock()?;
        let journal_entries = self.read_journal()?;

        Ok(RecoveryReport {
            was_incomplete,
            last_pid,
            lock_timestamp,
            journal_entries,
        })
    }

    /// Clean up after recovery: remove both lock and journal.
    pub fn cleanup(&self) -> Result<(), CrashRecoveryError> {
        self.release_lock()?;
        self.clear_journal()?;
        Ok(())
    }
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
    fn no_lock_means_clean() {
        let dir = TempDir::new().unwrap();
        let cr = CrashRecovery::new(dir.path().join("state"));
        assert!(!cr.has_lock());
    }

    #[test]
    fn acquire_and_release_lock() {
        let dir = TempDir::new().unwrap();
        let cr = CrashRecovery::new(dir.path().join("state"));
        cr.acquire_lock().unwrap();
        assert!(cr.has_lock());

        cr.release_lock().unwrap();
        assert!(!cr.has_lock());
    }

    #[test]
    fn detect_incomplete_session() {
        let dir = TempDir::new().unwrap();
        let cr = CrashRecovery::new(dir.path().join("state"));
        cr.acquire_lock().unwrap();
        // Simulate crash — no release_lock.

        let report = cr.inspect().unwrap();
        assert!(report.was_incomplete);
        assert!(report.last_pid.is_some());
        assert!(report.lock_timestamp.is_some());
    }

    #[test]
    fn clean_session_report() {
        let dir = TempDir::new().unwrap();
        let cr = CrashRecovery::new(dir.path().join("state"));
        cr.acquire_lock().unwrap();
        cr.release_lock().unwrap();

        let report = cr.inspect().unwrap();
        assert!(!report.was_incomplete);
    }

    #[test]
    fn journal_write_and_read() {
        let dir = TempDir::new().unwrap();
        let mut cr = CrashRecovery::new(dir.path().join("state"));

        let mut payload = std::collections::HashMap::new();
        payload.insert("profile".into(), "combat".into());
        cr.write_journal("profile_switch", payload).unwrap();

        let mut payload2 = std::collections::HashMap::new();
        payload2.insert("device".into(), "stick-1".into());
        cr.write_journal("device_connect", payload2).unwrap();

        let entries = cr.read_journal().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].seq, 1);
        assert_eq!(entries[0].operation, "profile_switch");
        assert_eq!(entries[1].seq, 2);
        assert_eq!(entries[1].operation, "device_connect");
    }

    #[test]
    fn empty_journal_returns_empty_vec() {
        let dir = TempDir::new().unwrap();
        let cr = CrashRecovery::new(dir.path().join("state"));
        let entries = cr.read_journal().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn clear_journal_removes_file() {
        let dir = TempDir::new().unwrap();
        let mut cr = CrashRecovery::new(dir.path().join("state"));
        cr.write_journal("test", std::collections::HashMap::new())
            .unwrap();
        assert!(cr.journal_path().exists());

        cr.clear_journal().unwrap();
        assert!(!cr.journal_path().exists());
    }

    #[test]
    fn cleanup_removes_lock_and_journal() {
        let dir = TempDir::new().unwrap();
        let mut cr = CrashRecovery::new(dir.path().join("state"));
        cr.acquire_lock().unwrap();
        cr.write_journal("op", std::collections::HashMap::new())
            .unwrap();

        cr.cleanup().unwrap();
        assert!(!cr.has_lock());
        assert!(cr.read_journal().unwrap().is_empty());
    }

    #[test]
    fn inspect_includes_journal_entries() {
        let dir = TempDir::new().unwrap();
        let mut cr = CrashRecovery::new(dir.path().join("state"));
        cr.acquire_lock().unwrap();
        cr.write_journal("state_save", std::collections::HashMap::new())
            .unwrap();

        let report = cr.inspect().unwrap();
        assert!(report.was_incomplete);
        assert_eq!(report.journal_entries.len(), 1);
        assert_eq!(report.journal_entries[0].operation, "state_save");
    }

    #[test]
    fn release_missing_lock_is_ok() {
        let dir = TempDir::new().unwrap();
        let cr = CrashRecovery::new(dir.path().join("state"));
        cr.release_lock().unwrap();
    }

    #[test]
    fn clear_missing_journal_is_ok() {
        let dir = TempDir::new().unwrap();
        let cr = CrashRecovery::new(dir.path().join("state"));
        cr.clear_journal().unwrap();
    }

    #[test]
    fn journal_entries_have_timestamps() {
        let dir = TempDir::new().unwrap();
        let mut cr = CrashRecovery::new(dir.path().join("state"));
        let entry = cr
            .write_journal("op", std::collections::HashMap::new())
            .unwrap();
        assert!(entry.timestamp > 0);
    }

    #[test]
    fn journal_sequence_numbers_increment() {
        let dir = TempDir::new().unwrap();
        let mut cr = CrashRecovery::new(dir.path().join("state"));
        let e1 = cr
            .write_journal("a", std::collections::HashMap::new())
            .unwrap();
        let e2 = cr
            .write_journal("b", std::collections::HashMap::new())
            .unwrap();
        assert_eq!(e1.seq, 1);
        assert_eq!(e2.seq, 2);
    }
}
