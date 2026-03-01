// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the flight-writers crate.
//!
//! Covers: writer definition loading/validation, JSON diff table parsing,
//! version-specific overrides, variable name resolution, type checking,
//! error handling, golden/snapshot tests, and property-based (proptest) fuzzing.

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
// 1. Writer Definition Loading & Validation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parse_msfs_writer_json_as_cc_writer_config() {
    let content = fs::read_to_string(writers_dir().join("msfs_1.36.0.json")).unwrap();
    let config: CcWriterConfig = serde_json::from_str(&content).unwrap();
    assert_eq!(config.sim, "msfs");
    assert_eq!(config.version, "1.36.0");
    assert!(!config.diffs.is_empty());
}

#[test]
fn parse_xplane_writer_json_as_cc_writer_config() {
    let content = fs::read_to_string(writers_dir().join("xplane_12.0.json")).unwrap();
    let config: CcWriterConfig = serde_json::from_str(&content).unwrap();
    assert_eq!(config.sim, "xplane");
    assert_eq!(config.version, "12.0");
}

#[test]
fn parse_dcs_writer_json_as_cc_writer_config() {
    let content = fs::read_to_string(writers_dir().join("dcs_2.9.json")).unwrap();
    let config: CcWriterConfig = serde_json::from_str(&content).unwrap();
    assert_eq!(config.sim, "dcs");
    assert_eq!(config.version, "2.9");
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
fn writer_config_round_trip_serde() {
    let config = WriterConfig {
        schema: "flight.writer/1".to_string(),
        sim: SimulatorType::MSFS,
        version: "1.36.0".to_string(),
        description: Some("round trip test".to_string()),
        diffs: vec![],
        verify_scripts: vec![],
    };
    let json_str = serde_json::to_string(&config).unwrap();
    let decoded: WriterConfig = serde_json::from_str(&json_str).unwrap();
    assert_eq!(decoded.schema, config.schema);
    assert_eq!(decoded.sim, config.sim);
    assert_eq!(decoded.version, config.version);
    assert_eq!(decoded.description, config.description);
}

#[test]
fn simulator_type_serde_round_trip() {
    for sim in [SimulatorType::MSFS, SimulatorType::XPlane, SimulatorType::DCS] {
        let s = serde_json::to_string(&sim).unwrap();
        let d: SimulatorType = serde_json::from_str(&s).unwrap();
        assert_eq!(d, sim);
    }
}

#[test]
fn simulator_type_display() {
    assert_eq!(SimulatorType::MSFS.to_string(), "msfs");
    assert_eq!(SimulatorType::XPlane.to_string(), "xplane");
    assert_eq!(SimulatorType::DCS.to_string(), "dcs");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. JSON Diff Table Parsing & Structure
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn msfs_diff_table_has_required_top_level_keys() {
    let v = read_writer_json("msfs_1.36.0.json");
    assert!(v.get("sim").is_some(), "missing 'sim' key");
    assert!(v.get("version").is_some(), "missing 'version' key");
    assert!(v.get("diffs").is_some(), "missing 'diffs' key");
    assert!(
        v.get("verification_tests").is_some(),
        "missing 'verification_tests' key"
    );
}

#[test]
fn xplane_diff_table_has_required_top_level_keys() {
    let v = read_writer_json("xplane_12.0.json");
    assert!(v.get("sim").is_some());
    assert!(v.get("diffs").is_some());
}

#[test]
fn dcs_diff_table_has_required_top_level_keys() {
    let v = read_writer_json("dcs_2.9.json");
    assert!(v.get("sim").is_some());
    assert!(v.get("diffs").is_some());
}

#[test]
fn diff_entries_have_file_and_operation_fields() {
    for filename in ["msfs_1.36.0.json", "xplane_12.0.json", "dcs_2.9.json"] {
        let v = read_writer_json(filename);
        let diffs = v["diffs"].as_array().unwrap();
        for (i, diff) in diffs.iter().enumerate() {
            assert!(
                diff.get("file").is_some(),
                "{filename} diff[{i}] missing 'file'"
            );
            assert!(
                diff.get("operation").is_some(),
                "{filename} diff[{i}] missing 'operation'"
            );
        }
    }
}

#[test]
fn verification_tests_have_name_and_type() {
    for filename in ["msfs_1.36.0.json", "xplane_12.0.json", "dcs_2.9.json"] {
        let v = read_writer_json(filename);
        if let Some(tests) = v.get("verification_tests").and_then(|t| t.as_array()) {
            for (i, test) in tests.iter().enumerate() {
                assert!(
                    test.get("name").is_some(),
                    "{filename} verification_tests[{i}] missing 'name'"
                );
                assert!(
                    test.get("test_type").is_some(),
                    "{filename} verification_tests[{i}] missing 'test_type'"
                );
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Version-Specific Override Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn msfs_version_is_1_36_0() {
    let v = read_writer_json("msfs_1.36.0.json");
    assert_eq!(v["version"].as_str().unwrap(), "1.36.0");
}

#[test]
fn xplane_version_is_12_0() {
    let v = read_writer_json("xplane_12.0.json");
    assert_eq!(v["version"].as_str().unwrap(), "12.0");
}

#[test]
fn dcs_version_is_2_9() {
    let v = read_writer_json("dcs_2.9.json");
    assert_eq!(v["version"].as_str().unwrap(), "2.9");
}

#[test]
fn curve_conflict_writer_loads_configs_by_sim_version() {
    let tmp = TempDir::new().unwrap();
    let config = WritersConfig {
        config_dir: tmp.path().join("config"),
        backup_dir: tmp.path().join("backup"),
        max_backups: 5,
        enable_verification: false,
    };
    let writer = CurveConflictWriter::with_config(config).unwrap();
    // Default configs are created for msfs_1.36.0, xplane_12.0, dcs_2.9
    let params = HashMap::new();
    // Requesting a non-existent version should fail
    let result = writer.resolve_curve_conflict("msfs", "99.0.0", "disable", &params);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Variable Name Resolution
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn msfs_writer_contains_use_linear_curves_key() {
    let v = read_writer_json("msfs_1.36.0.json");
    let all_keys: Vec<String> = v["diffs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d.get("changes"))
        .filter_map(|c| c.as_object())
        .flat_map(|obj| obj.keys().cloned())
        .collect();
    assert!(all_keys.contains(&"UseLinearCurves".to_string()));
}

#[test]
fn xplane_writer_contains_joy_linear_curves_key() {
    let v = read_writer_json("xplane_12.0.json");
    let all_keys: Vec<String> = v["diffs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d.get("changes"))
        .filter_map(|c| c.as_object())
        .flat_map(|obj| obj.keys().cloned())
        .collect();
    assert!(all_keys.contains(&"_joy_use_linear_curves".to_string()));
}

#[test]
fn dcs_writer_contains_use_linear_curves_key() {
    let v = read_writer_json("dcs_2.9.json");
    let all_keys: Vec<String> = v["diffs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d.get("changes"))
        .filter_map(|c| c.as_object())
        .flat_map(|obj| obj.keys().cloned())
        .collect();
    assert!(all_keys.contains(&"useLinearCurves".to_string()));
}

#[test]
fn resolve_curve_conflict_returns_error_for_missing_config() {
    let tmp = TempDir::new().unwrap();
    let config = WritersConfig {
        config_dir: tmp.path().join("config"),
        backup_dir: tmp.path().join("backup"),
        max_backups: 5,
        enable_verification: false,
    };
    let writer = CurveConflictWriter::with_config(config).unwrap();
    let params = HashMap::new();
    // Non-existent sim/version combo should error
    let result = writer.resolve_curve_conflict("msfs", "0.0.0", "disable", &params);
    assert!(result.is_err());
}

#[test]
fn default_configs_are_created_on_init() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("config");
    let config = WritersConfig {
        config_dir: config_dir.clone(),
        backup_dir: tmp.path().join("backup"),
        max_backups: 5,
        enable_verification: false,
    };
    let _writer = CurveConflictWriter::with_config(config).unwrap();
    // At least 3 default JSON config files should exist
    let json_count = fs::read_dir(&config_dir)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .ok()
                .and_then(|e| e.path().extension().map(|ext| ext == "json"))
                .unwrap_or(false)
        })
        .count();
    assert!(json_count >= 3, "expected >=3 config files, got {json_count}");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Type Checking & Conversion
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn diff_operation_serde_replace() {
    let op = DiffOperation::Replace {
        content: "hello".to_string(),
    };
    let json_str = serde_json::to_string(&op).unwrap();
    let decoded: DiffOperation = serde_json::from_str(&json_str).unwrap();
    match decoded {
        DiffOperation::Replace { content } => assert_eq!(content, "hello"),
        _ => panic!("expected Replace variant"),
    }
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
        DiffOperation::IniSection { section, changes } => {
            assert_eq!(section, "S");
            assert_eq!(changes.get("k").unwrap(), "v");
        }
        _ => panic!("expected IniSection variant"),
    }
}

#[test]
fn diff_operation_serde_json_patch() {
    let op = DiffOperation::JsonPatch {
        patches: vec![JsonPatchOp {
            op: JsonPatchOpType::Add,
            path: "/foo".to_string(),
            value: Some(json!(42)),
            from: None,
        }],
    };
    let json_str = serde_json::to_string(&op).unwrap();
    let decoded: DiffOperation = serde_json::from_str(&json_str).unwrap();
    match decoded {
        DiffOperation::JsonPatch { patches } => {
            assert_eq!(patches.len(), 1);
            assert_eq!(patches[0].path, "/foo");
        }
        _ => panic!("expected JsonPatch variant"),
    }
}

#[test]
fn diff_operation_serde_line_replace() {
    let op = DiffOperation::LineReplace {
        pattern: "old".to_string(),
        replacement: "new".to_string(),
        regex: true,
    };
    let json_str = serde_json::to_string(&op).unwrap();
    let decoded: DiffOperation = serde_json::from_str(&json_str).unwrap();
    match decoded {
        DiffOperation::LineReplace {
            pattern,
            replacement,
            regex,
        } => {
            assert_eq!(pattern, "old");
            assert_eq!(replacement, "new");
            assert!(regex);
        }
        _ => panic!("expected LineReplace variant"),
    }
}

#[test]
fn json_patch_op_type_serde_all_variants() {
    for op in [
        JsonPatchOpType::Add,
        JsonPatchOpType::Remove,
        JsonPatchOpType::Replace,
        JsonPatchOpType::Move,
        JsonPatchOpType::Copy,
        JsonPatchOpType::Test,
    ] {
        let s = serde_json::to_string(&op).unwrap();
        let d: JsonPatchOpType = serde_json::from_str(&s).unwrap();
        // Just ensure the round-trip doesn't panic
        assert!(!s.is_empty());
        let _ = d;
    }
}

#[test]
fn file_diff_default_backup_is_true() {
    let json_str = r#"{"file": "test.cfg", "type": "replace", "content": "x"}"#;
    let diff: FileDiff = serde_json::from_str(json_str).unwrap();
    assert!(diff.backup, "backup should default to true");
}

#[test]
fn expected_result_default_tolerance() {
    let json_str = r#"{"variable": "ALT", "value": 1000.0}"#;
    let er: ExpectedResult = serde_json::from_str(json_str).unwrap();
    assert!((er.tolerance - 0.001).abs() < f64::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Error Handling
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn invalid_json_fails_to_parse_as_writer_config() {
    let bad_json = "{ not valid json }";
    let result = serde_json::from_str::<WriterConfig>(bad_json);
    assert!(result.is_err());
}

#[test]
fn empty_json_object_fails_to_parse_as_writer_config() {
    let result = serde_json::from_str::<WriterConfig>("{}");
    assert!(result.is_err(), "empty object should not parse as WriterConfig");
}

#[test]
fn missing_required_field_fails_writer_config_parse() {
    // Missing 'sim' and 'version'
    let partial = r#"{"schema": "flight.writer/1", "diffs": [], "verify_scripts": []}"#;
    let result = serde_json::from_str::<WriterConfig>(partial);
    assert!(result.is_err());
}

#[test]
fn invalid_simulator_type_fails_parse() {
    let json_str = r#"{"schema":"x","sim":"invalid_sim","version":"1","diffs":[],"verify_scripts":[]}"#;
    let result = serde_json::from_str::<WriterConfig>(json_str);
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
    assert!(result.is_err(), "test op should fail on value mismatch");
}

#[tokio::test]
async fn json_patch_remove_nonexistent_path_errors() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);

    let target = tmp.path().join("test.json");
    fs::write(&target, r#"{"a": 1}"#).unwrap();

    let diff = make_json_patch_diff(
        target,
        vec![JsonPatchOp {
            op: JsonPatchOpType::Remove,
            path: "/nonexistent/deep/path".to_string(),
            value: None,
            from: None,
        }],
    );

    let result = applier.apply_diff(&diff, &backup).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn json_patch_on_invalid_json_file_errors() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);

    let target = tmp.path().join("bad.json");
    fs::write(&target, "NOT JSON AT ALL").unwrap();

    let diff = make_json_patch_diff(
        target,
        vec![JsonPatchOp {
            op: JsonPatchOpType::Add,
            path: "/key".to_string(),
            value: Some(json!("val")),
            from: None,
        }],
    );

    let result = applier.apply_diff(&diff, &backup).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn replace_creates_parent_directories() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);

    let target = tmp.path().join("deep").join("nested").join("dir").join("file.txt");
    let diff = make_replace_diff(target.clone(), "content");

    applier.apply_diff(&diff, &backup).await.unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "content");
}

#[test]
fn writers_new_creates_directories() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("a");
    let golden_dir = tmp.path().join("b");
    let backup_dir = tmp.path().join("c");

    let _w = Writers::new(&config_dir, &golden_dir, &backup_dir).unwrap();
    assert!(config_dir.exists());
    assert!(golden_dir.exists());
    assert!(backup_dir.exists());
}

#[test]
fn curve_conflict_error_display() {
    let io_err = CurveConflictError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "oops"));
    assert!(io_err.to_string().contains("oops"));

    let config_err = CurveConflictError::Configuration("bad config".to_string());
    assert!(config_err.to_string().contains("bad config"));

    let writer_err = CurveConflictError::Writer("write fail".to_string());
    assert!(writer_err.to_string().contains("write fail"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. Diff Application Depth Tests
// ═══════════════════════════════════════════════════════════════════════════════

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
async fn ini_diff_updates_existing_key_in_section() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);
    let target = tmp.path().join("existing.ini");

    fs::write(&target, "[SEC]\nold_key=old_val\n").unwrap();

    let diff = make_ini_diff(target.clone(), "SEC", vec![("old_key", "new_val")]);
    applier.apply_diff(&diff, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("old_key=new_val"));
    assert!(!content.contains("old_key=old_val"));
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

#[tokio::test]
async fn line_replace_regex_works() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);
    let target = tmp.path().join("file.txt");

    fs::write(&target, "value=123\nvalue=456").unwrap();
    let diff = make_line_replace_diff(target.clone(), r"value=\d+", "value=0", true);
    applier.apply_diff(&diff, &backup).await.unwrap();

    let content = fs::read_to_string(&target).unwrap();
    assert_eq!(content, "value=0\nvalue=0");
}

#[tokio::test]
async fn json_patch_add_nested_path() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);
    let target = tmp.path().join("nested.json");

    fs::write(&target, "{}").unwrap();
    let diff = make_json_patch_diff(
        target.clone(),
        vec![JsonPatchOp {
            op: JsonPatchOpType::Add,
            path: "/a/b/c".to_string(),
            value: Some(json!(true)),
            from: None,
        }],
    );
    applier.apply_diff(&diff, &backup).await.unwrap();

    let v: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    assert_eq!(v["a"]["b"]["c"], true);
}

#[tokio::test]
async fn json_patch_replace_existing_value() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);
    let target = tmp.path().join("replace.json");

    fs::write(&target, r#"{"x": 1}"#).unwrap();
    let diff = make_json_patch_diff(
        target.clone(),
        vec![JsonPatchOp {
            op: JsonPatchOpType::Replace,
            path: "/x".to_string(),
            value: Some(json!(99)),
            from: None,
        }],
    );
    applier.apply_diff(&diff, &backup).await.unwrap();

    let v: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    assert_eq!(v["x"], 99);
}

#[tokio::test]
async fn json_patch_test_op_succeeds_on_match() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);
    let target = tmp.path().join("match.json");

    fs::write(&target, r#"{"key": "value"}"#).unwrap();
    let diff = make_json_patch_diff(
        target,
        vec![JsonPatchOp {
            op: JsonPatchOpType::Test,
            path: "/key".to_string(),
            value: Some(json!("value")),
            from: None,
        }],
    );
    applier.apply_diff(&diff, &backup).await.unwrap();
}

#[tokio::test]
async fn json_patch_copy_preserves_source() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);
    let target = tmp.path().join("copy.json");

    fs::write(&target, r#"{"src": "hello", "dst": {}}"#).unwrap();
    let diff = make_json_patch_diff(
        target.clone(),
        vec![JsonPatchOp {
            op: JsonPatchOpType::Copy,
            path: "/dst/copied".to_string(),
            value: None,
            from: Some("/src".to_string()),
        }],
    );
    applier.apply_diff(&diff, &backup).await.unwrap();

    let v: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    assert_eq!(v["src"], "hello", "source must remain");
    assert_eq!(v["dst"]["copied"], "hello");
}

#[tokio::test]
async fn json_patch_move_removes_source() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);
    let target = tmp.path().join("move.json");

    fs::write(&target, r#"{"a": 42, "b": {}}"#).unwrap();
    let diff = make_json_patch_diff(
        target.clone(),
        vec![JsonPatchOp {
            op: JsonPatchOpType::Move,
            path: "/b/moved".to_string(),
            value: None,
            from: Some("/a".to_string()),
        }],
    );
    applier.apply_diff(&diff, &backup).await.unwrap();

    let v: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    assert!(v.get("a").is_none(), "source 'a' must be removed after move");
    assert_eq!(v["b"]["moved"], 42);
}

#[tokio::test]
async fn json_patch_move_to_child_path_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let applier = make_applier(&tmp);
    let backup = make_backup_dir(&tmp);
    let target = tmp.path().join("bad_move.json");

    fs::write(&target, r#"{"a": {"b": 1}}"#).unwrap();
    let diff = make_json_patch_diff(
        target,
        vec![JsonPatchOp {
            op: JsonPatchOpType::Move,
            path: "/a/b/c".to_string(),
            value: None,
            from: Some("/a".to_string()),
        }],
    );
    assert!(applier.apply_diff(&diff, &backup).await.is_err());
}

#[tokio::test]
async fn apply_writer_config_multiple_diffs() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join("cfg");
    let golden_dir = tmp.path().join("golden");
    let backup_dir = tmp.path().join("bak");

    let writers = Writers::new(&config_dir, &golden_dir, &backup_dir).unwrap();

    let file1 = tmp.path().join("f1.txt");
    let file2 = tmp.path().join("f2.txt");

    let config = WriterConfig {
        schema: "flight.writer/1".to_string(),
        sim: SimulatorType::XPlane,
        version: "12.0".to_string(),
        description: None,
        diffs: vec![
            make_replace_diff(file1.clone(), "content1"),
            make_replace_diff(file2.clone(), "content2"),
        ],
        verify_scripts: vec![],
    };

    let result = writers.apply_writer(&config).await.unwrap();
    assert!(result.success);
    assert_eq!(result.modified_files.len(), 2);
    assert_eq!(fs::read_to_string(&file1).unwrap(), "content1");
    assert_eq!(fs::read_to_string(&file2).unwrap(), "content2");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. Verify & Repair Depth Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn verify_returns_false_when_config_missing() {
    let tmp = TempDir::new().unwrap();
    let verifier = ConfigVerifier::new(tmp.path());
    let result = verifier.verify(SimulatorType::DCS, "99.0").await.unwrap();
    assert!(!result.success);
    assert!(result.script_results.is_empty());
}

#[tokio::test]
async fn repair_no_mismatches_is_noop() {
    let tmp = TempDir::new().unwrap();
    let repairer = ConfigRepairer::new(tmp.path(), tmp.path());

    let vr = VerifyResult {
        sim: SimulatorType::MSFS,
        version: "1.0".to_string(),
        success: true,
        script_results: vec![],
        mismatched_files: vec![],
    };
    let result = repairer.repair(&vr).await.unwrap();
    assert!(result.success);
    assert!(result.repaired_files.is_empty());
}

#[tokio::test]
async fn rollback_nonexistent_backup_fails_gracefully() {
    let tmp = TempDir::new().unwrap();
    let mgr = RollbackManager::new(tmp.path());
    let result = mgr.rollback("does_not_exist").await.unwrap();
    assert!(!result.success);
    assert!(!result.errors.is_empty());
}

#[tokio::test]
async fn rollback_list_backups_empty_dir() {
    let tmp = TempDir::new().unwrap();
    let mgr = RollbackManager::new(tmp.path().join("no_such_dir"));
    let backups = mgr.list_backups().await.unwrap();
    assert!(backups.is_empty());
}

#[tokio::test]
async fn backup_verify_nonexistent_returns_invalid() {
    let tmp = TempDir::new().unwrap();
    let mgr = RollbackManager::new(tmp.path());
    let result = mgr.verify_backup("nonexistent").await.unwrap();
    assert!(!result.valid);
    assert_eq!(result.verified_files, 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 9. CurveConflictWriter Depth Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn writers_config_default_values() {
    let cfg = WritersConfig::default();
    assert_eq!(cfg.max_backups, 10);
    assert!(cfg.enable_verification);
    assert_eq!(cfg.config_dir, PathBuf::from("writers"));
    assert_eq!(cfg.backup_dir, PathBuf::from("backups"));
}

#[test]
fn cc_diff_operation_variants_serialize() {
    let ops = [
        CcDiffOperation::Set,
        CcDiffOperation::Remove,
        CcDiffOperation::Append,
        CcDiffOperation::Replace,
    ];
    for op in &ops {
        let s = serde_json::to_string(op).unwrap();
        assert!(!s.is_empty());
    }
}

#[test]
fn verification_test_type_variants_serialize() {
    let types = [
        VerificationTestType::FileExists,
        VerificationTestType::FileContains,
        VerificationTestType::RegistryValue,
        VerificationTestType::Command,
    ];
    for t in &types {
        let s = serde_json::to_string(t).unwrap();
        let _: VerificationTestType = serde_json::from_str(&s).unwrap();
    }
}

#[test]
fn backup_info_serde_round_trip() {
    let info = BackupInfo {
        timestamp: 1234567890,
        description: "test backup".to_string(),
        affected_files: vec![PathBuf::from("a.txt"), PathBuf::from("b.cfg")],
        backup_dir: PathBuf::from("/tmp/backup"),
        writer_config: "msfs_1.36.0".to_string(),
    };
    let json_str = serde_json::to_string(&info).unwrap();
    let decoded: BackupInfo = serde_json::from_str(&json_str).unwrap();
    assert_eq!(decoded.timestamp, info.timestamp);
    assert_eq!(decoded.affected_files.len(), 2);
}

#[test]
fn backup_metadata_serde_round_trip() {
    let meta = BackupMetadata {
        id: "bk_1".to_string(),
        timestamp: 999,
        sim: SimulatorType::XPlane,
        version: "12.0".to_string(),
        description: "test".to_string(),
        files: vec![],
    };
    let json_str = serde_json::to_string(&meta).unwrap();
    let decoded: BackupMetadata = serde_json::from_str(&json_str).unwrap();
    assert_eq!(decoded.id, "bk_1");
    assert_eq!(decoded.sim, SimulatorType::XPlane);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 10. Property-Based Tests (proptest)
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn parse_arbitrary_json_as_writer_config_never_panics(s in "\\PC{0,256}") {
        let _ = serde_json::from_str::<WriterConfig>(&s);
    }

    #[test]
    fn parse_arbitrary_json_as_cc_writer_config_never_panics(s in "\\PC{0,256}") {
        let _ = serde_json::from_str::<CcWriterConfig>(&s);
    }

    #[test]
    fn parse_arbitrary_json_as_file_diff_never_panics(s in "\\PC{0,256}") {
        let _ = serde_json::from_str::<FileDiff>(&s);
    }

    #[test]
    fn parse_arbitrary_json_as_diff_operation_never_panics(s in "\\PC{0,256}") {
        let _ = serde_json::from_str::<DiffOperation>(&s);
    }

    #[test]
    fn parse_arbitrary_json_as_simulator_type_never_panics(s in "\\PC{0,64}") {
        let _ = serde_json::from_str::<SimulatorType>(&s);
    }

    #[test]
    fn curve_conflict_writer_creation_never_panics(
        max_backups in 1u8..20,
    ) {
        let tmp = TempDir::new().unwrap();
        let config = WritersConfig {
            config_dir: tmp.path().join("c"),
            backup_dir: tmp.path().join("b"),
            max_backups: max_backups as usize,
            enable_verification: false,
        };
        let _ = CurveConflictWriter::with_config(config);
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
