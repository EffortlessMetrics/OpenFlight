// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Journal file reader for Elite: Dangerous JSONL event logs.
//!
//! Elite Dangerous writes a new `Journal.<yyyyMMddHHmmss>.<session>.log` file
//! each time the game starts. Each line in the file is a JSON object.
//!
//! [`JournalReader`] discovers the most-recent journal file and tails new
//! events from it on each [`read_new_events`](JournalReader::read_new_events)
//! call.

use crate::protocol::{JournalEvent, parse_journal_line};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Reads new journal events from the latest Elite Dangerous journal file.
///
/// Call [`read_new_events`](Self::read_new_events) periodically; it returns
/// only events written since the last call (byte-offset based tailing).
pub struct JournalReader {
    journal_dir: PathBuf,
    /// Currently tracked journal file path.
    current_file: Option<PathBuf>,
    /// Byte offset in `current_file` up to which events have been consumed.
    byte_offset: u64,
}

impl JournalReader {
    /// Create a new reader for the given journal directory.
    ///
    /// Does not open any files until [`read_new_events`](Self::read_new_events)
    /// is first called.
    pub fn new(journal_dir: impl Into<PathBuf>) -> Self {
        Self {
            journal_dir: journal_dir.into(),
            current_file: None,
            byte_offset: 0,
        }
    }

    /// Find the most-recently modified `Journal.*.log` file in `dir`.
    ///
    /// Returns `None` if no journal files exist.
    pub fn find_latest_journal(dir: &Path) -> Option<PathBuf> {
        let rd = std::fs::read_dir(dir).ok()?;
        rd.filter_map(|e| {
            let e = e.ok()?;
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("Journal.") && name.ends_with(".log") {
                let modified = e.metadata().ok()?.modified().ok()?;
                Some((modified, e.path()))
            } else {
                None
            }
        })
        .max_by_key(|(m, _)| *m)
        .map(|(_, p)| p)
    }

    /// Read all new [`JournalEvent`]s written since the last call.
    ///
    /// If the latest journal file has changed (a new game session started),
    /// the reader automatically switches to the new file and starts from the
    /// beginning.
    ///
    /// Returns an empty `Vec` when there are no new events or when no journal
    /// file exists.
    pub fn read_new_events(&mut self) -> std::io::Result<Vec<JournalEvent>> {
        let latest = match Self::find_latest_journal(&self.journal_dir) {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        // Switch to the new file if the session has changed.
        if self.current_file.as_deref() != Some(&*latest) {
            debug!(
                "JournalReader: switching to {}",
                latest.display()
            );
            self.current_file = Some(latest.clone());
            self.byte_offset = 0;
        }

        let mut file = File::open(&latest)?;
        let file_len = file.metadata()?.len();

        // Nothing new.
        if file_len <= self.byte_offset {
            return Ok(Vec::new());
        }

        file.seek(SeekFrom::Start(self.byte_offset))?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        let mut bytes_read: u64 = 0;
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    warn!("JournalReader: I/O error reading line: {e}");
                    break;
                }
            };
            // +1 for the newline character
            bytes_read += line.len() as u64 + 1;

            if let Some(event) = parse_journal_line(&line) {
                events.push(event);
            }
        }

        self.byte_offset += bytes_read;
        Ok(events)
    }

    /// Return the path of the journal file currently being tailed, if any.
    pub fn current_file(&self) -> Option<&Path> {
        self.current_file.as_deref()
    }

    /// Reset the reader to the start of the current file (re-reads all events).
    pub fn reset(&mut self) {
        self.byte_offset = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::JournalEvent;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_journal(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn find_latest_journal_returns_none_for_empty_dir() {
        let dir = TempDir::new().unwrap();
        assert!(JournalReader::find_latest_journal(dir.path()).is_none());
    }

    #[test]
    fn find_latest_journal_returns_most_recent() {
        let dir = TempDir::new().unwrap();
        write_journal(&dir, "Journal.20250101120000.01.log", "");
        // Sleep briefly so file modification times differ on all platforms.
        std::thread::sleep(std::time::Duration::from_millis(10));
        write_journal(&dir, "Journal.20250102120000.01.log", "");

        let latest = JournalReader::find_latest_journal(dir.path()).unwrap();
        assert!(latest.to_string_lossy().contains("20250102"));
    }

    #[test]
    fn ignores_non_journal_files() {
        let dir = TempDir::new().unwrap();
        write_journal(&dir, "Status.json", r#"{"Flags":0}"#);
        assert!(JournalReader::find_latest_journal(dir.path()).is_none());
    }

    #[test]
    fn reads_new_events_from_journal() {
        let dir = TempDir::new().unwrap();
        write_journal(
            &dir,
            "Journal.20250101120000.01.log",
            "{\"timestamp\":\"2025-01-01T00:00:00Z\",\"event\":\"LoadGame\",\"Ship\":\"SideWinder\"}\n\
             {\"timestamp\":\"2025-01-01T00:00:01Z\",\"event\":\"Music\",\"MusicTrack\":\"MainMenu\"}\n",
        );

        let mut reader = JournalReader::new(dir.path());
        let events = reader.read_new_events().unwrap();
        // LoadGame should parse; Music should be skipped.
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], JournalEvent::LoadGame { .. }));
    }

    #[test]
    fn second_call_returns_only_new_events() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("Journal.20250101120000.01.log");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(
                f,
                r#"{{"timestamp":"2025-01-01T00:00:00Z","event":"LoadGame","Ship":"SideWinder"}}"#
            )
            .unwrap();
        }

        let mut reader = JournalReader::new(dir.path());
        let first = reader.read_new_events().unwrap();
        assert_eq!(first.len(), 1);

        // Append a new event.
        {
            let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
            writeln!(
                f,
                r#"{{"timestamp":"2025-01-01T00:01:00Z","event":"FsdJump","StarSystem":"Sol","StarPos":[0.0,0.0,0.0]}}"#
            )
            .unwrap();
        }

        let second = reader.read_new_events().unwrap();
        assert_eq!(second.len(), 1);
        assert!(matches!(second[0], JournalEvent::FsdJump { .. }));
    }

    #[test]
    fn empty_dir_returns_empty_vec() {
        let dir = TempDir::new().unwrap();
        let mut reader = JournalReader::new(dir.path());
        let events = reader.read_new_events().unwrap();
        assert!(events.is_empty());
    }
}
