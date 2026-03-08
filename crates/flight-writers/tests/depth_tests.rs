// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Combined depth tests for the flight-writers crate.
//!
//! Covers: variable tables, write batching, golden file regression,
//! diff tables, serialization, integration flows, and property-based (proptest) fuzzing.

use flight_writers::*;
use proptest::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

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

fn make_applier(tmp: &TempDir) -> WriterApplier {
    WriterApplier::new(tmp.path())
}

fn make_backup_dir(tmp: &TempDir) -> PathBuf {
    let p = tmp.path().join("backup");
    fs::create_dir_all(&p).unwrap();
    p
}

fn make_ini_diff(target: PathBuf, section: &str, kv: Vec<(&str, &str)>) -> FileDiff {
    let mut changes = HashMap::new();
    for (k, v) in kv {
        changes.insert(k.to_string(), v.to_string());
    }
    FileDiff {
        file: target,
        operation: DiffOperation::IniSection {
            section: section.to_string(),
            changes,
        },
        backup: false,
    }
}

fn make_ini_diff_slice(target: PathBuf, section: &str, kvs: &[(&str, &str)]) -> FileDiff {
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

fn make_json_patch_diff(target: PathBuf, patches: Vec<JsonPatchOp>) -> FileDiff {
    FileDiff {
        file: target,
        operation: DiffOperation::JsonPatch { patches },
        backup: false,
    }
}

fn make_replace_diff(target: PathBuf, content: &str) -> FileDiff {
    FileDiff {
        file: target,
        operation: DiffOperation::Replace {
            content: content.to_string(),
        },
        backup: false,
    }
}

fn make_line_replace_diff(target: PathBuf, pattern: &str, replacement: &str, regex: bool) -> FileDiff {
    FileDiff {
        file: target,
        operation: DiffOperation::LineReplace {
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
            regex,
        },
        backup: false,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Variable tables / Loading & Validation
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

#[test]
fn all_writer_json_files_deserialize_to_cc_writer_config() {
    let dir = writers_dir();
    let mut count = 0u32;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let content = fs::read_to_string(&path).unwrap();
            serde_json::from_str::<CcWriterConfig>(&content)
                .unwrap_or_else(|e| panic!("{path:?} failed CcWriterConfig deser: {e}"));
            count += 1;
        }
    }
    assert!(count >= 3, "expected at least 3 writer JSONs, got {count}");
}

#[test]
fn msfs_diff_table_has_required_top_level_keys() {
    let v = read_writer_json("msfs_1.36.0.json");
    assert!(v.get("sim").is_some());
    assert!(v.get("version").is_some());
    assert!(v.get("diffs").is_some());
    assert!(v.get("verification_tests").is_some());
}

#[test]
fn verification_tests_have_name_and_type() {
    for filename in ["msfs_1.36.0.json", "xplane_12.0.json", "dcs_2.9.json"] {
        let v = read_writer_json(filename);
        if let Some(tests) = v.get("verification_tests").and_then(|t| t.as_array()) {
            for (i, test) in tests.iter().enumerate() {
                assert!(test.get("name").is_some());
                assert!(test.get("test_type").is_some());
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Write batching
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
            make_ini_diff_slice(f1.clone(), "S", &[("k1", "v1")]),
            make_ini_diff_slice(f2.clone(), "S", &[("k2", "v2")]),
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

    let targets: Vec<PathBuf> = (0..5).map(|i| tmp.path().join(format!("f{i}.ini"))).collect();
    let diffs: Vec<FileDiff> = targets
        .iter()
        .map(|t| make_ini_diff_slice(t.clone(), "SEC", &[("key", "val")]))
        .collect();

    let config = make_writer_config(SimulatorType::XPlane, "12.0", diffs);
    let result = applier.apply(&config).await.unwrap();
    assert_eq!(result.modified_files.len(), 5);

    for t in &targets {
        assert!(t.exists());
    }
}

#[tokio::test]
async fn wb_priority_ordering_last_write_wins() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
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
    assert_eq!(content, "second");
}

#[tokio::test]
async fn wb_coalescing_duplicate_ini_keys() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("coalesce.ini");
    let diff1 = make_ini_diff_slice(target.clone(), "S", &[("key", "old")]);
    applier.apply_diff(&diff1, &backup).await.unwrap();

    let diff2 = make_ini_diff_slice(target.clone(), "S", &[("key", "new")]);
    applier.apply_diff(&diff2, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("key=new"));
    let count = content.matches("key=").count();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn wb_batch_size_limit_respected() {
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

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Golden file tests
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
        vec![make_ini_diff_slice(
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
        vec![make_ini_diff_slice(
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

    let expected = test_dir.join("expected");
    fs::create_dir_all(&expected).unwrap();
    fs::write(expected.join("cfg.ini"), "[SEC]\nkey=WRONG_VALUE\n").unwrap();

    let tester = GoldenFileTester::new(&golden_dir);
    let result = tester.test_simulator(SimulatorType::MSFS).await.unwrap();
    assert!(!result.success);
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
    assert!(result.coverage.areas.len() >= 3);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Diff tables / Application
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn dt_version_to_version_diff() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("panel.ini");

    let v1 = make_ini_diff_slice(target.clone(), "PANEL", &[("rev", "1"), ("feature_a", "on")]);
    applier.apply_diff(&v1, &backup).await.unwrap();

    let v2 = make_ini_diff_slice(
        target.clone(),
        "PANEL",
        &[("rev", "2"), ("feature_b", "on")],
    );
    applier.apply_diff(&v2, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("rev=2"));
    assert!(content.contains("feature_a=on"));
    assert!(content.contains("feature_b=on"));
}

#[tokio::test]
async fn dt_additive_diff() {
    let tmp = TempDir::new().unwrap();
    let applier = WriterApplier::new(tmp.path());
    let backup = tmp.path().join("bk");
    fs::create_dir_all(&backup).unwrap();

    let target = tmp.path().join("add.ini");
    fs::write(&target, "[SEC]\nexisting=1\n").unwrap();

    let diff = make_ini_diff_slice(target.clone(), "SEC", &[("new_key", "new_val")]);
    applier.apply_diff(&diff, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("existing=1"));
    assert!(content.contains("new_key=new_val"));
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
            patches: vec![JsonPatchOp {
                op: JsonPatchOpType::Remove,
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
    assert!(json.get("remove_me").is_none());
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
            patches: vec![JsonPatchOp {
                op: JsonPatchOpType::Replace,
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
async fn ini_diff_adds_new_section_and_keys() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);
    let target = tmp.path().join("new.ini");

    let diff = make_ini_diff(target.clone(), "NEW_SEC", vec![("key1", "val1"), ("key2", "val2")]);
    applier.apply_diff(&diff, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("[NEW_SEC]"));
    assert!(content.contains("key1=val1"));
    assert!(content.contains("key2=val2"));
}

#[tokio::test]
async fn line_replace_literal_works() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);
    let target = tmp.path().join("file.txt");

    fs::write(&target, "foo bar baz").unwrap();
    let diff = make_line_replace_diff(target.clone(), "bar", "qux", false);
    applier.apply_diff(&diff, &backup).await.unwrap();

    assert_eq!(fs::read_to_string(&target).unwrap(), "foo qux baz");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Serialization / Type Checking
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
fn simulator_type_display() {
    assert_eq!(SimulatorType::MSFS.to_string(), "msfs");
    assert_eq!(SimulatorType::XPlane.to_string(), "xplane");
    assert_eq!(SimulatorType::DCS.to_string(), "dcs");
}

#[test]
fn diff_operation_serde_ini_section() {
    let mut changes = HashMap::new();
    changes.insert("k".to_string(), "v".to_string());
    let op = DiffOperation::IniSection {
        section: "S".to_string(),
        changes,
    };
    let json_str = serde_json::to_string(&op).unwrap();
    let decoded: DiffOperation = serde_json::from_str(&json_str).unwrap();
    match decoded {
        DiffOperation::IniSection { section, .. } => assert_eq!(section, "S"),
        _ => panic!("expected IniSection"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Integration
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn int_full_pipeline_ini() {
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
        diffs: vec![make_ini_diff_slice(
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
}

#[tokio::test]
async fn int_curve_conflict_writer_default_configs() {
    let tmp = TempDir::new().unwrap();
    let config = WritersConfig {
        config_dir: tmp.path().join("cfg"),
        backup_dir: tmp.path().join("bk"),
        max_backups: 5,
        enable_verification: false,
    };

    let writer = CurveConflictWriter::with_config(config.clone()).unwrap();
    assert!(config.config_dir.join("msfs_1.36.0.json").exists());
    assert!(config.config_dir.join("xplane_12.0.json").exists());
    assert!(config.config_dir.join("dcs_2.9.json").exists());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. Error Handling
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn invalid_json_fails_to_parse_as_writer_config() {
    let bad_json = "{ not valid json }";
    let result = serde_json::from_str::<WriterConfig>(bad_json);
    assert!(result.is_err());
}

#[tokio::test]
async fn json_patch_test_op_fails_on_value_mismatch() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);

    let target = tmp.path().join("test.json");
    fs::write(&target, r#"{"key": "actual"}"#).unwrap();

    let diff = make_json_patch_diff(
        target,
        vec![JsonPatchOp {
            op: JsonPatchOpType::Test,
            path: "/key".to_string(),
            value: Some(json!("expected_different")),
            from: None,
        }],
    );

    let result = applier.apply_diff(&diff, &backup).await;
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. Property-Based Tests (proptest)
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn parse_arbitrary_json_as_writer_config_never_panics(s in "\\PC{0,256}") {
        let _ = serde_json::from_str::<WriterConfig>(&s);
    }

    #[test]
    fn simulator_type_display_never_panics(idx in 0u8..3) {
        let sim = match idx {
            0 => SimulatorType::MSFS,
            1 => SimulatorType::XPlane,
            _ => SimulatorType::DCS,
        };
        let s = sim.to_string();
        prop_assert!(!s.is_empty());
    }
}
