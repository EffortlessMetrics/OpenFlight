// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Golden-file / snapshot tests for the versioned SimConnect variable diff tables.
//!
//! These tests catch accidental regressions when variable names or diff structure
//! are changed in the versioned JSON writer files under `writers/`.
//!
//! Run `cargo insta review` (or `INSTA_UPDATE=new cargo test -p flight-writers`)
//! to accept new or changed snapshots.

use flight_writers::{DiffOperation, FileDiff, SimulatorType, WriterApplier, WriterConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

// ── helpers ───────────────────────────────────────────────────────────────────

fn writers_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/writers"))
}

fn read_writer_json(filename: &str) -> serde_json::Value {
    let path = writers_dir().join(filename);
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {filename}: {e}"));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{filename} contains invalid JSON: {e}"))
}

// ── 1. Snapshot all diff table JSON files ─────────────────────────────────────

#[test]
fn snapshot_msfs_1_36_0_diff_table() {
    let value = read_writer_json("msfs_1.36.0.json");
    insta::assert_json_snapshot!("msfs_1_36_0", value);
}

#[test]
fn snapshot_xplane_12_0_diff_table() {
    let value = read_writer_json("xplane_12.0.json");
    insta::assert_json_snapshot!("xplane_12_0", value);
}

#[test]
fn snapshot_dcs_2_9_diff_table() {
    let value = read_writer_json("dcs_2.9.json");
    insta::assert_json_snapshot!("dcs_2_9", value);
}

// ── 2. Apply diff produces valid result ───────────────────────────────────────

#[tokio::test]
async fn apply_ini_section_diff_produces_expected_output() {
    let temp = TempDir::new().unwrap();
    let applier = WriterApplier::new(temp.path());
    let target = temp.path().join("UserCfg.opt");

    let mut changes = HashMap::new();
    changes.insert("UseLinearCurves".to_string(), "1".to_string());
    changes.insert("DisableNonLinearControls".to_string(), "1".to_string());

    let diff = FileDiff {
        file: target.clone(),
        operation: DiffOperation::IniSection {
            section: "CONTROLS".to_string(),
            changes,
        },
        backup: false,
    };

    let backup_path = temp.path().join("backup");
    std::fs::create_dir_all(&backup_path).unwrap();
    applier.apply_diff(&diff, &backup_path).await.unwrap();

    let output = std::fs::read_to_string(&target).unwrap();
    assert!(
        output.contains("[CONTROLS]"),
        "section header must be present"
    );
    assert!(
        output.contains("UseLinearCurves=1"),
        "UseLinearCurves setting must be written"
    );
    assert!(
        output.contains("DisableNonLinearControls=1"),
        "DisableNonLinearControls setting must be written"
    );
}

// ── 3. Diff is idempotent ─────────────────────────────────────────────────────

#[tokio::test]
async fn ini_section_diff_is_idempotent() {
    let temp = TempDir::new().unwrap();
    let applier = WriterApplier::new(temp.path());
    let target = temp.path().join("target.ini");

    let mut changes = HashMap::new();
    changes.insert("UseLinearCurves".to_string(), "1".to_string());

    let diff = FileDiff {
        file: target.clone(),
        operation: DiffOperation::IniSection {
            section: "CONTROLS".to_string(),
            changes,
        },
        backup: false,
    };

    let backup_path = temp.path().join("backup");
    std::fs::create_dir_all(&backup_path).unwrap();

    applier.apply_diff(&diff, &backup_path).await.unwrap();
    let after_first = std::fs::read_to_string(&target).unwrap();

    applier.apply_diff(&diff, &backup_path).await.unwrap();
    let after_second = std::fs::read_to_string(&target).unwrap();

    assert_eq!(
        after_first, after_second,
        "applying the same INI diff twice must produce identical output"
    );
}

// ── 4. Critical setting names are present in MSFS writer ─────────────────────

/// Regression guard: the control-curve disablement keys must remain present
/// in the MSFS 1.36.0 writer diff table.
#[test]
fn msfs_writer_contains_critical_control_curve_settings() {
    let value = read_writer_json("msfs_1.36.0.json");
    let diffs = value["diffs"].as_array().expect("diffs must be an array");
    assert!(!diffs.is_empty(), "diffs array must not be empty");

    let all_change_keys: Vec<String> = diffs
        .iter()
        .filter_map(|d| d.get("changes"))
        .filter_map(|c| c.as_object())
        .flat_map(|obj| obj.keys().cloned())
        .collect();

    assert!(
        all_change_keys.iter().any(|k| k == "UseLinearCurves"),
        "UseLinearCurves must be present in MSFS diff changes; got: {all_change_keys:?}"
    );
    assert!(
        all_change_keys
            .iter()
            .any(|k| k == "DisableNonLinearControls"),
        "DisableNonLinearControls must be present in MSFS diff changes; got: {all_change_keys:?}"
    );
}

// ── 5. All JSON files in writers/ are valid JSON ──────────────────────────────

#[test]
fn all_writer_json_files_parse_without_error() {
    let dir = writers_dir();
    let entries = std::fs::read_dir(&dir).expect("writers/ directory must exist");

    let mut checked = 0u32;
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
            serde_json::from_str::<serde_json::Value>(&content)
                .unwrap_or_else(|e| panic!("{path:?} contains invalid JSON: {e}"));
            checked += 1;
        }
    }

    assert!(checked > 0, "must find at least one .json file in writers/");
}

// ── 6. Serialization golden test (WriterConfig round-trip) ────────────────────

#[test]
fn snapshot_writer_config_serialization() {
    let mut changes = HashMap::new();
    changes.insert("UseLinearCurves".to_string(), "1".to_string());
    changes.insert("DisableNonLinearControls".to_string(), "1".to_string());

    let config = WriterConfig {
        schema: "flight.writer/1".to_string(),
        sim: SimulatorType::MSFS,
        version: "1.36.0".to_string(),
        description: Some("Disable MSFS built-in control curves".to_string()),
        diffs: vec![FileDiff {
            file: PathBuf::from("MSFS/UserCfg.opt"),
            operation: DiffOperation::IniSection {
                section: "CONTROLS".to_string(),
                changes,
            },
            backup: true,
        }],
        verify_scripts: vec![],
    };

    // sort_maps ensures stable output despite HashMap non-determinism
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("writer_config_msfs_1_36_0", config);
    });
}
