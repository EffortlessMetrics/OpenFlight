// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the Writers data-tables system (ADR-002).
//!
//! Covers variable tables, write batching, golden file regression,
//! diff tables, serialization, and integration flows.

use flight_writers::{
    CcWriterConfig, CurveConflictWriter, DiffOperation, FileDiff, GoldenFileTester, SimulatorType,
    WriterApplier, WriterConfig, WritersConfig,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ── helpers ──────────────────────────────────────────────────────────────────

fn writers_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/writers"))
}

fn read_writer_json(filename: &str) -> Value {
    let path = writers_dir().join(filename);
    let content =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {filename}: {e}"));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{filename} contains invalid JSON: {e}"))
}

fn make_ini_diff(target: PathBuf, section: &str, kvs: &[(&str, &str)]) -> FileDiff {
    let changes: HashMap<String, String> = kvs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    FileDiff {
        file: target,
        operation: DiffOperation::IniSection {
            section: section.to_string(),
            changes,
        },
        backup: false,
    }
}

fn make_writer_config(sim: SimulatorType, version: &str, diffs: Vec<FileDiff>) -> WriterConfig {
    WriterConfig {
        schema: "flight.writer/1".to_string(),
        sim,
        version: version.to_string(),
        description: Some("depth test config".to_string()),
        diffs,
        verify_scripts: vec![],
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Variable tables (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn vt_load_msfs_table() {
    let v = read_writer_json("msfs_1.36.0.json");
    assert_eq!(v["sim"], "msfs");
    assert_eq!(v["version"], "1.36.0");
}

#[test]
fn vt_load_xplane_table() {
    let v = read_writer_json("xplane_12.0.json");
    assert_eq!(v["sim"], "xplane");
    assert_eq!(v["version"], "12.0");
}

#[test]
fn vt_load_dcs_table() {
    let v = read_writer_json("dcs_2.9.json");
    assert_eq!(v["sim"], "dcs");
    assert_eq!(v["version"], "2.9");
}

#[test]
fn vt_lookup_variable_by_name() {
    let v = read_writer_json("msfs_1.36.0.json");
    let diffs = v["diffs"].as_array().unwrap();
    let keys: Vec<String> = diffs
        .iter()
        .filter_map(|d| d.get("changes"))
        .filter_map(|c| c.as_object())
        .flat_map(|obj| obj.keys().cloned())
        .collect();
    assert!(
        keys.contains(&"UseLinearCurves".to_string()),
        "UseLinearCurves must be findable by name"
    );
}

#[test]
fn vt_category_filtering_by_section() {
    let v = read_writer_json("msfs_1.36.0.json");
    let diffs = v["diffs"].as_array().unwrap();
    let sections: Vec<&str> = diffs
        .iter()
        .filter_map(|d| d["section"].as_str())
        .collect();
    assert!(
        sections.iter().any(|s| s.contains("CONTROLS")),
        "MSFS table must have a CONTROLS section"
    );
}

#[test]
fn vt_type_information_preserved() {
    let v = read_writer_json("msfs_1.36.0.json");
    let changes = &v["diffs"][0]["changes"];
    assert!(
        changes.is_object(),
        "changes must be a JSON object with string values"
    );
    for (_k, val) in changes.as_object().unwrap() {
        assert!(val.is_string(), "all change values must be strings");
    }
}

#[test]
fn vt_unit_values_are_strings() {
    // ADR-002: change values are always string-typed in the JSON tables
    for file in &["msfs_1.36.0.json", "xplane_12.0.json", "dcs_2.9.json"] {
        let v = read_writer_json(file);
        for diff in v["diffs"].as_array().unwrap() {
            if let Some(obj) = diff["changes"].as_object() {
                for (key, val) in obj {
                    assert!(val.is_string(), "{file}: value for '{key}' must be string");
                }
            }
        }
    }
}

#[test]
fn vt_version_string_present_in_all_tables() {
    for file in &["msfs_1.36.0.json", "xplane_12.0.json", "dcs_2.9.json"] {
        let v = read_writer_json(file);
        assert!(
            v["version"].is_string(),
            "{file} must have a version field"
        );
        let ver = v["version"].as_str().unwrap();
        assert!(!ver.is_empty(), "{file}: version must not be empty");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Write batching (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn wb_batch_accumulation_multiple_diffs() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let f1 = tmp.path().join("a.ini");
    let f2 = tmp.path().join("b.ini");

    let config = make_writer_config(
        SimulatorType::MSFS,
        "1.36.0",
        vec![
            make_ini_diff(f1.clone(), "S", &[("k1", "v1")]),
            make_ini_diff(f2.clone(), "S", &[("k2", "v2")]),
        ],
    );

    let result = applier.apply(&config).await.unwrap();
    assert!(result.success);
    assert_eq!(result.modified_files.len(), 2);
}

#[tokio::test]
async fn wb_flush_produces_all_files() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let targets: Vec<PathBuf> = (0..5).map(|i| tmp.path().join(format!("f{i}.ini"))).collect();
    let diffs: Vec<FileDiff> = targets
        .iter()
        .map(|t| make_ini_diff(t.clone(), "SEC", &[("key", "val")]))
        .collect();

    let config = make_writer_config(SimulatorType::XPlane, "12.0", diffs);
    let result = applier.apply(&config).await.unwrap();
    assert_eq!(result.modified_files.len(), 5);

    for t in &targets {
        assert!(t.exists(), "{t:?} must be created");
    }
}

#[tokio::test]
async fn wb_priority_ordering_last_write_wins() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("priority.txt");

    let config = make_writer_config(
        SimulatorType::DCS,
        "2.9",
        vec![
            FileDiff {
                file: target.clone(),
                operation: DiffOperation::Replace {
                    content: "first".to_string(),
                },
                backup: false,
            },
            FileDiff {
                file: target.clone(),
                operation: DiffOperation::Replace {
                    content: "second".to_string(),
                },
                backup: false,
            },
        ],
    );

    let result = applier.apply(&config).await.unwrap();
    assert!(result.success);
    let content = fs::read_to_string(&target).unwrap();
    assert_eq!(content, "second", "later diff must overwrite earlier");
}

#[tokio::test]
async fn wb_coalescing_duplicate_ini_keys() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("coalesce.ini");
    // First write sets key=old
    let diff1 = make_ini_diff(target.clone(), "S", &[("key", "old")]);
    applier.apply_diff(&diff1, &backup).await.unwrap();

    // Second write overwrites key=new
    let diff2 = make_ini_diff(target.clone(), "S", &[("key", "new")]);
    applier.apply_diff(&diff2, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("key=new"), "coalesced value must be 'new'");
    // Key should appear only once in the section
    let count = content.matches("key=").count();
    assert_eq!(count, 1, "key should appear exactly once after coalescing");
}

#[tokio::test]
async fn wb_batch_size_limit_respected() {
    // Verify we can apply a large batch without error
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());

    let diffs: Vec<FileDiff> = (0..50)
        .map(|i| FileDiff {
            file: tmp.path().join(format!("large_{i}.txt")),
            operation: DiffOperation::Replace {
                content: format!("content_{i}"),
            },
            backup: false,
        })
        .collect();

    let config = make_writer_config(SimulatorType::MSFS, "1.36.0", diffs);
    let result = applier.apply(&config).await.unwrap();
    assert!(result.success);
    assert_eq!(result.modified_files.len(), 50);
}

#[tokio::test]
async fn wb_empty_batch_succeeds() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());

    let config = make_writer_config(SimulatorType::MSFS, "1.36.0", vec![]);
    let result = applier.apply(&config).await.unwrap();
    assert!(result.success);
    assert!(result.modified_files.is_empty());
}

#[tokio::test]
async fn wb_line_replace_diff_applied() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("line.txt");
    fs::write(&target, "hello world\nfoo bar\n").unwrap();

    let diff = FileDiff {
        file: target.clone(),
        operation: DiffOperation::LineReplace {
            pattern: "foo".to_string(),
            replacement: "baz".to_string(),
            regex: false,
        },
        backup: false,
    };
    applier.apply_diff(&diff, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("baz"), "pattern must be replaced");
    assert!(!content.contains("foo"), "original pattern must be gone");
}

#[tokio::test]
async fn wb_regex_line_replace() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("regex.txt");
    fs::write(&target, "version=1.2.3\n").unwrap();

    let diff = FileDiff {
        file: target.clone(),
        operation: DiffOperation::LineReplace {
            pattern: r"version=\d+\.\d+\.\d+".to_string(),
            replacement: "version=2.0.0".to_string(),
            regex: true,
        },
        backup: false,
    };
    applier.apply_diff(&diff, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("version=2.0.0"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Golden file tests (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn gf_snapshot_current_table_output() {
    let tmp = TempDir::new().unwrap();
    let golden_dir = tmp.path().join("golden");
    let test_dir = golden_dir.join("msfs").join("test_v1.36.0_autopilot");
    fs::create_dir_all(&test_dir).unwrap();

    let config = make_writer_config(
        SimulatorType::MSFS,
        "1.36.0",
        vec![make_ini_diff(
            "ap.cfg".into(),
            "AUTOPILOT",
            &[("enabled", "1")],
        )],
    );
    fs::write(
        test_dir.join("input.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();

    let expected = test_dir.join("expected");
    fs::create_dir_all(&expected).unwrap();
    fs::write(expected.join("ap.cfg"), "[AUTOPILOT]\nenabled=1\n").unwrap();

    let tester = GoldenFileTester::new(&golden_dir);
    let result = tester.test_simulator(SimulatorType::MSFS).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn gf_regression_detection_mismatch() {
    let tmp = TempDir::new().unwrap();
    let golden_dir = tmp.path().join("golden");
    let test_dir = golden_dir.join("msfs").join("test_v1.36.0_regression");
    fs::create_dir_all(&test_dir).unwrap();

    let config = make_writer_config(
        SimulatorType::MSFS,
        "1.36.0",
        vec![make_ini_diff(
            "cfg.ini".into(),
            "SEC",
            &[("key", "actual_value")],
        )],
    );
    fs::write(
        test_dir.join("input.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();

    // Expected output differs from what config produces
    let expected = test_dir.join("expected");
    fs::create_dir_all(&expected).unwrap();
    fs::write(expected.join("cfg.ini"), "[SEC]\nkey=WRONG_VALUE\n").unwrap();

    let tester = GoldenFileTester::new(&golden_dir);
    let result = tester.test_simulator(SimulatorType::MSFS).await.unwrap();
    assert!(!result.success, "mismatch must be detected as failure");
    assert!(result.test_cases[0].diff.is_some());
}

#[tokio::test]
async fn gf_version_diff_format_detected() {
    let tmp = TempDir::new().unwrap();
    let golden_dir = tmp.path().join("golden");

    // Two different version test cases
    for ver in &["v1.35.0", "v1.36.0"] {
        let test_dir = golden_dir
            .join("msfs")
            .join(format!("test_{ver}_autopilot"));
        fs::create_dir_all(&test_dir).unwrap();
        fs::write(test_dir.join("input.json"), "{}").unwrap();
        let expected = test_dir.join("expected");
        fs::create_dir_all(&expected).unwrap();
        fs::write(expected.join("test.txt"), "test").unwrap();
    }

    let tester = GoldenFileTester::new(&golden_dir);
    let result = tester.test_simulator(SimulatorType::MSFS).await.unwrap();
    assert!(
        result.coverage.versions.len() >= 2,
        "both versions must be detected"
    );
}

#[tokio::test]
async fn gf_no_golden_dir_returns_failure() {
    let tmp = TempDir::new().unwrap();
    let golden_dir = tmp.path().join("nonexistent_golden");

    let tester = GoldenFileTester::new(&golden_dir);
    let result = tester.test_simulator(SimulatorType::MSFS).await.unwrap();
    assert!(!result.success);
    assert!(result.test_cases.is_empty());
}

#[tokio::test]
async fn gf_coverage_matrix_tracks_areas() {
    let tmp = TempDir::new().unwrap();
    let golden_dir = tmp.path().join("golden");

    for area in &["autopilot", "electrical", "fuel"] {
        let test_dir = golden_dir
            .join("xplane")
            .join(format!("test_v12.0_{area}"));
        fs::create_dir_all(&test_dir).unwrap();
        fs::write(test_dir.join("input.json"), "{}").unwrap();
        let expected = test_dir.join("expected");
        fs::create_dir_all(&expected).unwrap();
        fs::write(expected.join("test.txt"), "x").unwrap();
    }

    let tester = GoldenFileTester::new(&golden_dir);
    let result = tester.test_simulator(SimulatorType::XPlane).await.unwrap();
    assert!(
        result.coverage.areas.len() >= 3,
        "all areas must appear in coverage"
    );
}

#[tokio::test]
async fn gf_dcs_golden_test_passes() {
    let tmp = TempDir::new().unwrap();
    let golden_dir = tmp.path().join("golden");
    let test_dir = golden_dir.join("dcs").join("test_v2.9_engine");
    fs::create_dir_all(&test_dir).unwrap();

    let config = make_writer_config(
        SimulatorType::DCS,
        "2.9",
        vec![FileDiff {
            file: "engine.cfg".into(),
            operation: DiffOperation::Replace {
                content: "throttle=linear".to_string(),
            },
            backup: false,
        }],
    );
    fs::write(
        test_dir.join("input.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();

    let expected = test_dir.join("expected");
    fs::create_dir_all(&expected).unwrap();
    fs::write(expected.join("engine.cfg"), "throttle=linear").unwrap();

    let tester = GoldenFileTester::new(&golden_dir);
    let result = tester.test_simulator(SimulatorType::DCS).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn gf_multiple_files_in_single_test_case() {
    let tmp = TempDir::new().unwrap();
    let golden_dir = tmp.path().join("golden");
    let test_dir = golden_dir.join("msfs").join("test_v1.36.0_avionics");
    fs::create_dir_all(&test_dir).unwrap();

    let config = make_writer_config(
        SimulatorType::MSFS,
        "1.36.0",
        vec![
            FileDiff {
                file: "nav.cfg".into(),
                operation: DiffOperation::Replace {
                    content: "nav=on".to_string(),
                },
                backup: false,
            },
            FileDiff {
                file: "radio.cfg".into(),
                operation: DiffOperation::Replace {
                    content: "radio=active".to_string(),
                },
                backup: false,
            },
        ],
    );
    fs::write(
        test_dir.join("input.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();

    let expected = test_dir.join("expected");
    fs::create_dir_all(&expected).unwrap();
    fs::write(expected.join("nav.cfg"), "nav=on").unwrap();
    fs::write(expected.join("radio.cfg"), "radio=active").unwrap();

    let tester = GoldenFileTester::new(&golden_dir);
    let result = tester.test_simulator(SimulatorType::MSFS).await.unwrap();
    assert!(result.success);
}

#[test]
fn gf_all_writer_files_deserialize_as_cc_config() {
    // Every JSON in writers/ must parse as CcWriterConfig
    let dir = writers_dir();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let content = fs::read_to_string(&path).unwrap();
            let result = serde_json::from_str::<CcWriterConfig>(&content);
            assert!(
                result.is_ok(),
                "{}: failed to deserialize as CcWriterConfig: {}",
                path.display(),
                result.unwrap_err()
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Diff tables (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn dt_version_to_version_diff() {
    // Applying two versions in sequence should produce cumulative changes
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("panel.ini");

    let v1 = make_ini_diff(target.clone(), "PANEL", &[("rev", "1"), ("feature_a", "on")]);
    applier.apply_diff(&v1, &backup).await.unwrap();

    let v2 = make_ini_diff(
        target.clone(),
        "PANEL",
        &[("rev", "2"), ("feature_b", "on")],
    );
    applier.apply_diff(&v2, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("rev=2"), "rev must be updated to 2");
    assert!(content.contains("feature_a=on"), "feature_a must persist");
    assert!(content.contains("feature_b=on"), "feature_b must be added");
}

#[tokio::test]
async fn dt_additive_diff() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("add.ini");
    fs::write(&target, "[SEC]\nexisting=1\n").unwrap();

    let diff = make_ini_diff(target.clone(), "SEC", &[("new_key", "new_val")]);
    applier.apply_diff(&diff, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("existing=1"), "existing keys preserved");
    assert!(content.contains("new_key=new_val"), "new key added");
}

#[tokio::test]
async fn dt_removal_via_json_patch() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("remove.json");
    fs::write(&target, r#"{"keep": 1, "remove_me": 2}"#).unwrap();

    let diff = FileDiff {
        file: target.clone(),
        operation: DiffOperation::JsonPatch {
            patches: vec![flight_writers::JsonPatchOp {
                op: flight_writers::JsonPatchOpType::Remove,
                path: "/remove_me".to_string(),
                value: None,
                from: None,
            }],
        },
        backup: false,
    };
    applier.apply_diff(&diff, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    let json: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(json["keep"], 1);
    assert!(json.get("remove_me").is_none(), "removed key must be gone");
}

#[tokio::test]
async fn dt_modified_fields_via_json_replace() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("modify.json");
    fs::write(&target, r#"{"field": "old"}"#).unwrap();

    let diff = FileDiff {
        file: target.clone(),
        operation: DiffOperation::JsonPatch {
            patches: vec![flight_writers::JsonPatchOp {
                op: flight_writers::JsonPatchOpType::Replace,
                path: "/field".to_string(),
                value: Some(json!("new")),
                from: None,
            }],
        },
        backup: false,
    };
    applier.apply_diff(&diff, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    let json: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(json["field"], "new");
}

#[tokio::test]
async fn dt_diff_application_to_empty_json() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    // File does not exist yet — apply_json_patches creates it with {}
    let target = tmp.path().join("new.json");

    let diff = FileDiff {
        file: target.clone(),
        operation: DiffOperation::JsonPatch {
            patches: vec![flight_writers::JsonPatchOp {
                op: flight_writers::JsonPatchOpType::Add,
                path: "/created".to_string(),
                value: Some(json!(true)),
                from: None,
            }],
        },
        backup: false,
    };
    applier.apply_diff(&diff, &backup).await.unwrap();

    let json: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    assert_eq!(json["created"], true);
}

#[tokio::test]
async fn dt_multiple_patches_in_single_diff() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("multi.json");
    fs::write(&target, r#"{"a": 1, "b": 2, "c": 3}"#).unwrap();

    let diff = FileDiff {
        file: target.clone(),
        operation: DiffOperation::JsonPatch {
            patches: vec![
                flight_writers::JsonPatchOp {
                    op: flight_writers::JsonPatchOpType::Replace,
                    path: "/a".to_string(),
                    value: Some(json!(10)),
                    from: None,
                },
                flight_writers::JsonPatchOp {
                    op: flight_writers::JsonPatchOpType::Remove,
                    path: "/b".to_string(),
                    value: None,
                    from: None,
                },
                flight_writers::JsonPatchOp {
                    op: flight_writers::JsonPatchOpType::Add,
                    path: "/d".to_string(),
                    value: Some(json!(4)),
                    from: None,
                },
            ],
        },
        backup: false,
    };
    applier.apply_diff(&diff, &backup).await.unwrap();

    let json: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    assert_eq!(json["a"], 10);
    assert!(json.get("b").is_none());
    assert_eq!(json["c"], 3);
    assert_eq!(json["d"], 4);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Serialization (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ser_json_round_trip_writer_config() {
    let config = make_writer_config(
        SimulatorType::MSFS,
        "1.36.0",
        vec![FileDiff {
            file: "test.cfg".into(),
            operation: DiffOperation::Replace {
                content: "data".to_string(),
            },
            backup: true,
        }],
    );

    let json_str = serde_json::to_string(&config).unwrap();
    let deserialized: WriterConfig = serde_json::from_str(&json_str).unwrap();

    assert_eq!(deserialized.sim, SimulatorType::MSFS);
    assert_eq!(deserialized.version, "1.36.0");
    assert_eq!(deserialized.diffs.len(), 1);
}

#[test]
fn ser_schema_validation_present() {
    let config = make_writer_config(SimulatorType::XPlane, "12.0", vec![]);
    let json: Value = serde_json::to_value(&config).unwrap();
    assert_eq!(json["schema"], "flight.writer/1");
}

#[test]
fn ser_unknown_fields_preserved_in_raw_json() {
    // Raw JSON with extra fields should parse without error
    let raw = r#"{
        "sim": "msfs",
        "version": "1.36.0",
        "description": "test",
        "diffs": [],
        "verification_tests": [],
        "future_field": "should_be_preserved"
    }"#;

    let value: Value = serde_json::from_str(raw).unwrap();
    assert_eq!(value["future_field"], "should_be_preserved");
    // CcWriterConfig should still parse the known fields
    let parsed: CcWriterConfig = serde_json::from_str(raw).unwrap();
    assert_eq!(parsed.sim, "msfs");
}

#[test]
fn ser_empty_diffs_array() {
    let config = make_writer_config(SimulatorType::DCS, "2.9", vec![]);
    let json_str = serde_json::to_string(&config).unwrap();
    let deserialized: WriterConfig = serde_json::from_str(&json_str).unwrap();
    assert!(deserialized.diffs.is_empty());
}

#[test]
fn ser_large_config_round_trip() {
    let diffs: Vec<FileDiff> = (0..100)
        .map(|i| FileDiff {
            file: format!("file_{i}.cfg").into(),
            operation: DiffOperation::Replace {
                content: format!("content_{i}"),
            },
            backup: i % 2 == 0,
        })
        .collect();

    let config = make_writer_config(SimulatorType::MSFS, "1.36.0", diffs);
    let json_str = serde_json::to_string(&config).unwrap();
    let deserialized: WriterConfig = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.diffs.len(), 100);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Integration (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn int_full_pipeline_ini() {
    // sim config → variable table → write batch → output
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("cfg");
    let golden_dir = tmp.path().join("golden");
    let backup_dir = tmp.path().join("bk");

    let writers = flight_writers::Writers::new(&config_dir, &golden_dir, &backup_dir).unwrap();

    let target = tmp.path().join("panel.cfg");
    let config = WriterConfig {
        schema: "flight.writer/1".to_string(),
        sim: SimulatorType::MSFS,
        version: "1.36.0".to_string(),
        description: Some("integration test".to_string()),
        diffs: vec![make_ini_diff(
            target.clone(),
            "CONTROLS",
            &[("UseLinearCurves", "1"), ("sensitivity", "50")],
        )],
        verify_scripts: vec![],
    };

    let result = writers.apply_writer(&config).await.unwrap();
    assert!(result.success);

    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("[CONTROLS]"));
    assert!(content.contains("UseLinearCurves=1"));
    assert!(content.contains("sensitivity=50"));
}

#[tokio::test]
async fn int_full_pipeline_json_patch() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("cfg");
    let golden_dir = tmp.path().join("golden");
    let backup_dir = tmp.path().join("bk");

    let writers = flight_writers::Writers::new(&config_dir, &golden_dir, &backup_dir).unwrap();

    let target = tmp.path().join("settings.json");
    fs::write(&target, r#"{"graphics": {"quality": "low"}}"#).unwrap();

    let config = WriterConfig {
        schema: "flight.writer/1".to_string(),
        sim: SimulatorType::XPlane,
        version: "12.0".to_string(),
        description: Some("json integration".to_string()),
        diffs: vec![FileDiff {
            file: target.clone(),
            operation: DiffOperation::JsonPatch {
                patches: vec![flight_writers::JsonPatchOp {
                    op: flight_writers::JsonPatchOpType::Replace,
                    path: "/graphics/quality".to_string(),
                    value: Some(json!("high")),
                    from: None,
                }],
            },
            backup: true,
        }],
        verify_scripts: vec![],
    };

    let result = writers.apply_writer(&config).await.unwrap();
    assert!(result.success);

    let json: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    assert_eq!(json["graphics"]["quality"], "high");
}

#[tokio::test]
async fn int_variable_not_found_handling() {
    // JSON patch test op should fail when the path doesn't exist
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("sparse.json");
    fs::write(&target, r#"{"a": 1}"#).unwrap();

    let diff = FileDiff {
        file: target.clone(),
        operation: DiffOperation::JsonPatch {
            patches: vec![flight_writers::JsonPatchOp {
                op: flight_writers::JsonPatchOpType::Test,
                path: "/nonexistent".to_string(),
                value: Some(json!(42)),
                from: None,
            }],
        },
        backup: false,
    };

    let result = applier.apply_diff(&diff, &backup).await;
    assert!(result.is_err(), "test against missing path must error");
}

#[tokio::test]
async fn int_type_mismatch_test_op_fails() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("types.json");
    fs::write(&target, r#"{"num": 42}"#).unwrap();

    let diff = FileDiff {
        file: target.clone(),
        operation: DiffOperation::JsonPatch {
            patches: vec![flight_writers::JsonPatchOp {
                op: flight_writers::JsonPatchOpType::Test,
                path: "/num".to_string(),
                value: Some(json!("forty-two")), // string vs number
                from: None,
            }],
        },
        backup: false,
    };

    let result = applier.apply_diff(&diff, &backup).await;
    assert!(result.is_err(), "type mismatch in test op must fail");
}

#[test]
fn int_curve_conflict_writer_default_configs() {
    // CurveConflictWriter loads default configs for all three sims
    let tmp = TempDir::new().unwrap();
    let config = WritersConfig {
        config_dir: tmp.path().join("cfg"),
        backup_dir: tmp.path().join("bk"),
        max_backups: 5,
        enable_verification: false,
    };

    let writer = CurveConflictWriter::with_config(config.clone()).unwrap();
    let backups = writer.list_backups().unwrap();
    // No backups yet, but writer was created successfully with default configs
    assert!(backups.is_empty());

    // Verify that default config files were actually created
    assert!(config.config_dir.join("msfs_1.36.0.json").exists(), "MSFS default config missing");
    assert!(config.config_dir.join("xplane_12.0.json").exists(), "X-Plane default config missing");
    assert!(config.config_dir.join("dcs_2.9.json").exists(), "DCS default config missing");
}
