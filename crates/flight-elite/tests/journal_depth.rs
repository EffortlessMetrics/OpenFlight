// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for Elite Dangerous journal file reader:
//! file discovery, tailing, session switching, reset, edge cases.

use flight_elite::journal::JournalReader;
use flight_elite::protocol::JournalEvent;
use std::io::Write;
use tempfile::TempDir;

fn write_journal(dir: &TempDir, name: &str, content: &str) {
    let path = dir.path().join(name);
    std::fs::write(path, content).unwrap();
}

// ── find_latest_journal ─────────────────────────────────────────────────────

#[test]
fn find_latest_ignores_partial_name_matches() {
    let dir = TempDir::new().unwrap();
    // Files that start with "Journal." but don't end with ".log"
    write_journal(&dir, "Journal.20250101.txt", "");
    write_journal(&dir, "Journal.20250101.json", "");
    assert!(JournalReader::find_latest_journal(dir.path()).is_none());
}

#[test]
fn find_latest_ignores_non_journal_prefixes() {
    let dir = TempDir::new().unwrap();
    write_journal(&dir, "Status.json", "{}");
    write_journal(&dir, "Cargo.json", "{}");
    write_journal(&dir, "Market.json", "{}");
    write_journal(&dir, "Outfitting.json", "{}");
    assert!(JournalReader::find_latest_journal(dir.path()).is_none());
}

#[test]
fn find_latest_single_file() {
    let dir = TempDir::new().unwrap();
    write_journal(&dir, "Journal.20250601120000.01.log", "");
    let latest = JournalReader::find_latest_journal(dir.path()).unwrap();
    assert!(latest.to_string_lossy().contains("20250601"));
}

#[test]
fn find_latest_among_many_sessions() {
    let dir = TempDir::new().unwrap();
    for i in 1..=5 {
        write_journal(&dir, &format!("Journal.2025060{i}120000.01.log"), "");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let latest = JournalReader::find_latest_journal(dir.path()).unwrap();
    assert!(latest.to_string_lossy().contains("20250605"));
}

#[test]
fn find_latest_nonexistent_dir_returns_none() {
    let result = JournalReader::find_latest_journal(std::path::Path::new(
        "C:\\nonexistent\\path\\that\\should\\not\\exist",
    ));
    assert!(result.is_none());
}

// ── read_new_events tailing ─────────────────────────────────────────────────

#[test]
fn read_new_events_empty_journal_file() {
    let dir = TempDir::new().unwrap();
    write_journal(&dir, "Journal.20250101120000.01.log", "");
    let mut reader = JournalReader::new(dir.path());
    let events = reader.read_new_events().unwrap();
    assert!(events.is_empty());
}

#[test]
fn read_new_events_single_event() {
    let dir = TempDir::new().unwrap();
    write_journal(
        &dir,
        "Journal.20250101120000.01.log",
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"LoadGame","Ship":"Cobra_Mk3"}"#,
    );
    // File needs a trailing newline for line-based reading
    let path = dir.path().join("Journal.20250101120000.01.log");
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    writeln!(f).unwrap();
    drop(f);

    let mut reader = JournalReader::new(dir.path());
    let events = reader.read_new_events().unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        JournalEvent::LoadGame { ship, .. } => assert_eq!(ship, "Cobra_Mk3"),
        other => panic!("expected LoadGame, got {other:?}"),
    }
}

#[test]
fn read_events_mixed_known_and_unknown() {
    let dir = TempDir::new().unwrap();
    let content = "\
{\"timestamp\":\"2025-01-01T00:00:00Z\",\"event\":\"LoadGame\",\"Ship\":\"Eagle\"}\n\
{\"timestamp\":\"2025-01-01T00:00:01Z\",\"event\":\"Music\",\"MusicTrack\":\"Exploration\"}\n\
{\"timestamp\":\"2025-01-01T00:00:02Z\",\"event\":\"FsdJump\",\"StarSystem\":\"Sol\",\"StarPos\":[0.0,0.0,0.0]}\n\
{\"timestamp\":\"2025-01-01T00:00:03Z\",\"event\":\"Scan\",\"BodyName\":\"Earth\"}\n\
{\"timestamp\":\"2025-01-01T00:00:04Z\",\"event\":\"Docked\",\"StationName\":\"Abraham Lincoln\",\"StarSystem\":\"Sol\"}\n";

    write_journal(&dir, "Journal.20250101120000.01.log", content);

    let mut reader = JournalReader::new(dir.path());
    let events = reader.read_new_events().unwrap();
    // Known: LoadGame, FsdJump, Docked = 3; Unknown: Music, Scan = skipped
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0], JournalEvent::LoadGame { .. }));
    assert!(matches!(&events[1], JournalEvent::FsdJump { .. }));
    assert!(matches!(&events[2], JournalEvent::Docked { .. }));
}

#[test]
fn read_events_incremental_append() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Journal.20250101120000.01.log");

    // Write initial event.
    {
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"timestamp":"2025-01-01T00:00:00Z","event":"LoadGame","Ship":"Viper_Mk3"}}"#
        )
        .unwrap();
    }

    let mut reader = JournalReader::new(dir.path());
    let first = reader.read_new_events().unwrap();
    assert_eq!(first.len(), 1);

    // No new data → empty.
    let noop = reader.read_new_events().unwrap();
    assert!(noop.is_empty());

    // Append two more events.
    {
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(
            f,
            r#"{{"timestamp":"2025-01-01T00:01:00Z","event":"Location","StarSystem":"Sol"}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"timestamp":"2025-01-01T00:02:00Z","event":"FsdJump","StarSystem":"Barnard's Star","StarPos":[0.0,0.0,0.0]}}"#
        )
        .unwrap();
    }

    let second = reader.read_new_events().unwrap();
    assert_eq!(second.len(), 2);
    assert!(matches!(&second[0], JournalEvent::Location { .. }));
    assert!(matches!(&second[1], JournalEvent::FsdJump { .. }));
}

// ── Session switching ───────────────────────────────────────────────────────

#[test]
fn switches_to_newer_journal_file() {
    let dir = TempDir::new().unwrap();

    // First session.
    let path1 = dir.path().join("Journal.20250101120000.01.log");
    {
        let mut f = std::fs::File::create(&path1).unwrap();
        writeln!(
            f,
            r#"{{"timestamp":"2025-01-01T00:00:00Z","event":"LoadGame","Ship":"Eagle"}}"#
        )
        .unwrap();
    }

    let mut reader = JournalReader::new(dir.path());
    let first = reader.read_new_events().unwrap();
    assert_eq!(first.len(), 1);
    assert!(
        reader
            .current_file()
            .unwrap()
            .to_string_lossy()
            .contains("20250101")
    );

    // Small delay to ensure different modification time.
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second (newer) session.
    let path2 = dir.path().join("Journal.20250102120000.01.log");
    {
        let mut f = std::fs::File::create(&path2).unwrap();
        writeln!(
            f,
            r#"{{"timestamp":"2025-01-02T00:00:00Z","event":"LoadGame","Ship":"Python"}}"#
        )
        .unwrap();
    }

    let second = reader.read_new_events().unwrap();
    assert_eq!(second.len(), 1);
    match &second[0] {
        JournalEvent::LoadGame { ship, .. } => assert_eq!(ship, "Python"),
        other => panic!("expected LoadGame with Python, got {other:?}"),
    }
    assert!(
        reader
            .current_file()
            .unwrap()
            .to_string_lossy()
            .contains("20250102")
    );
}

// ── Reset ───────────────────────────────────────────────────────────────────

#[test]
fn reset_rereads_all_events() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Journal.20250101120000.01.log");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"timestamp":"2025-01-01T00:00:00Z","event":"LoadGame","Ship":"Sidewinder"}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"timestamp":"2025-01-01T00:01:00Z","event":"Location","StarSystem":"LHS 3447"}}"#
        )
        .unwrap();
    }

    let mut reader = JournalReader::new(dir.path());

    // Read all events.
    let first = reader.read_new_events().unwrap();
    assert_eq!(first.len(), 2);

    // No new events.
    assert!(reader.read_new_events().unwrap().is_empty());

    // Reset and re-read everything.
    reader.reset();
    let replayed = reader.read_new_events().unwrap();
    assert_eq!(replayed.len(), 2);
    assert!(matches!(&replayed[0], JournalEvent::LoadGame { .. }));
    assert!(matches!(&replayed[1], JournalEvent::Location { .. }));
}

// ── Edge cases ──────────────────────────────────────────────────────────────

#[test]
fn empty_lines_in_journal_produce_no_events() {
    let dir = TempDir::new().unwrap();
    let content = "\n\n\n{\"timestamp\":\"2025-01-01T00:00:00Z\",\"event\":\"LoadGame\",\"Ship\":\"Eagle\"}\n\n\n";
    write_journal(&dir, "Journal.20250101120000.01.log", content);

    let mut reader = JournalReader::new(dir.path());
    let events = reader.read_new_events().unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], JournalEvent::LoadGame { .. }));
}

#[test]
fn malformed_json_lines_skipped_gracefully() {
    let dir = TempDir::new().unwrap();
    let content = "\
{not valid json}\n\
{\"timestamp\":\"2025-01-01T00:00:00Z\",\"event\":\"LoadGame\",\"Ship\":\"Cobra_Mk3\"}\n\
{truncated\n\
{\"timestamp\":\"2025-01-01T00:00:02Z\",\"event\":\"Location\",\"StarSystem\":\"Sol\"}\n";

    write_journal(&dir, "Journal.20250101120000.01.log", content);

    let mut reader = JournalReader::new(dir.path());
    let events = reader.read_new_events().unwrap();
    assert_eq!(events.len(), 2, "should skip malformed lines");
    assert!(matches!(&events[0], JournalEvent::LoadGame { .. }));
    assert!(matches!(&events[1], JournalEvent::Location { .. }));
}

#[test]
fn current_file_none_before_first_read() {
    let dir = TempDir::new().unwrap();
    let reader = JournalReader::new(dir.path());
    assert!(reader.current_file().is_none());
}

#[test]
fn current_file_set_after_first_read() {
    let dir = TempDir::new().unwrap();
    write_journal(&dir, "Journal.20250101120000.01.log", "");
    let mut reader = JournalReader::new(dir.path());
    let _ = reader.read_new_events().unwrap();
    assert!(reader.current_file().is_some());
}

#[test]
fn all_journal_event_types_from_file() {
    let dir = TempDir::new().unwrap();
    let content = "\
{\"timestamp\":\"T\",\"event\":\"LoadGame\",\"Ship\":\"Eagle\",\"Commander\":\"CMDR\"}\n\
{\"timestamp\":\"T\",\"event\":\"Location\",\"StarSystem\":\"Sol\"}\n\
{\"timestamp\":\"T\",\"event\":\"FsdJump\",\"StarSystem\":\"Alpha Centauri\",\"StarPos\":[0.0,0.0,0.0]}\n\
{\"timestamp\":\"T\",\"event\":\"Docked\",\"StationName\":\"Jameson\",\"StarSystem\":\"Shinrarta\"}\n\
{\"timestamp\":\"T\",\"event\":\"Undocked\",\"StationName\":\"Jameson\"}\n\
{\"timestamp\":\"T\",\"event\":\"Touchdown\",\"Latitude\":1.0,\"Longitude\":2.0}\n\
{\"timestamp\":\"T\",\"event\":\"Liftoff\",\"Latitude\":1.0,\"Longitude\":2.0}\n\
{\"timestamp\":\"T\",\"event\":\"RefuelAll\",\"Amount\":10.0}\n";

    write_journal(&dir, "Journal.20250101120000.01.log", content);

    let mut reader = JournalReader::new(dir.path());
    let events = reader.read_new_events().unwrap();
    assert_eq!(events.len(), 8, "all 8 tracked event types should parse");
    assert!(matches!(&events[0], JournalEvent::LoadGame { .. }));
    assert!(matches!(&events[1], JournalEvent::Location { .. }));
    assert!(matches!(&events[2], JournalEvent::FsdJump { .. }));
    assert!(matches!(&events[3], JournalEvent::Docked { .. }));
    assert!(matches!(&events[4], JournalEvent::Undocked { .. }));
    assert!(matches!(&events[5], JournalEvent::Touchdown { .. }));
    assert!(matches!(&events[6], JournalEvent::Liftoff { .. }));
    assert!(matches!(&events[7], JournalEvent::RefuelAll { .. }));
}

#[test]
fn multiple_reads_after_incremental_writes() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Journal.20250101120000.01.log");

    let mut reader = JournalReader::new(dir.path());

    // No journal file yet.
    assert!(reader.read_new_events().unwrap().is_empty());

    // Create file with one event.
    {
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"timestamp":"T","event":"LoadGame","Ship":"Hauler"}}"#
        )
        .unwrap();
    }
    let batch1 = reader.read_new_events().unwrap();
    assert_eq!(batch1.len(), 1);

    // Append.
    {
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(
            f,
            r#"{{"timestamp":"T","event":"Undocked","StationName":"Port"}}"#
        )
        .unwrap();
    }
    let batch2 = reader.read_new_events().unwrap();
    assert_eq!(batch2.len(), 1);
    assert!(matches!(&batch2[0], JournalEvent::Undocked { .. }));

    // No new data.
    assert!(reader.read_new_events().unwrap().is_empty());
}
