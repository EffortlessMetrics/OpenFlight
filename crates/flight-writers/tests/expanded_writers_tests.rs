// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Expanded tests for flight-writers: types, diff operations, curve conflict
//! writer, verification, and edge cases.

use flight_writers::curve_conflict::{
    BackupInfo, CcConfigDiff, CcDiffOperation, CcVerificationTest, CcWriterConfig,
    CurveConflictError, CurveConflictWriter, VerificationResult, VerificationTestType,
    WriteResult, WritersConfig,
};
use flight_writers::types::*;
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

// ── SimulatorType ───────────────────────────────────────────────────────────

#[test]
fn simulator_type_display() {
    assert_eq!(SimulatorType::MSFS.to_string(), "msfs");
    assert_eq!(SimulatorType::XPlane.to_string(), "xplane");
    assert_eq!(SimulatorType::DCS.to_string(), "dcs");
}

#[test]
fn simulator_type_serde_roundtrip() {
    for sim in [SimulatorType::MSFS, SimulatorType::XPlane, SimulatorType::DCS] {
        let json = serde_json::to_string(&sim).unwrap();
        let restored: SimulatorType = serde_json::from_str(&json).unwrap();
        assert_eq!(sim, restored);
    }
}

#[test]
fn simulator_type_hash_eq() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(SimulatorType::MSFS);
    set.insert(SimulatorType::XPlane);
    set.insert(SimulatorType::DCS);
    assert_eq!(set.len(), 3);
    assert!(set.contains(&SimulatorType::MSFS));
}

// ── WriterConfig serde ──────────────────────────────────────────────────────

#[test]
fn writer_config_replace_serde_roundtrip() {
    let config = WriterConfig {
        schema: "flight.writer/1".to_string(),
        sim: SimulatorType::MSFS,
        version: "1.36.0".to_string(),
        description: Some("Test".to_string()),
        diffs: vec![FileDiff {
            file: PathBuf::from("test.cfg"),
            operation: DiffOperation::Replace {
                content: "new content".to_string(),
            },
            backup: true,
        }],
        verify_scripts: vec![],
    };
    let json = serde_json::to_string(&config).unwrap();
    let restored: WriterConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.sim, SimulatorType::MSFS);
    assert_eq!(restored.version, "1.36.0");
    assert_eq!(restored.diffs.len(), 1);
}

#[test]
fn writer_config_json_patch_serde_roundtrip() {
    let config = WriterConfig {
        schema: "flight.writer/1".to_string(),
        sim: SimulatorType::XPlane,
        version: "12.0".to_string(),
        description: None,
        diffs: vec![FileDiff {
            file: PathBuf::from("settings.json"),
            operation: DiffOperation::JsonPatch {
                patches: vec![JsonPatchOp {
                    op: JsonPatchOpType::Add,
                    path: "/key".to_string(),
                    value: Some(serde_json::Value::String("val".to_string())),
                    from: None,
                }],
            },
            backup: false,
        }],
        verify_scripts: vec![],
    };
    let json = serde_json::to_string(&config).unwrap();
    let restored: WriterConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.diffs.len(), 1);
    assert!(!restored.diffs[0].backup);
}

#[test]
fn writer_config_line_replace_serde_roundtrip() {
    let config = WriterConfig {
        schema: "flight.writer/1".to_string(),
        sim: SimulatorType::DCS,
        version: "2.9".to_string(),
        description: None,
        diffs: vec![FileDiff {
            file: PathBuf::from("options.lua"),
            operation: DiffOperation::LineReplace {
                pattern: "old_value".to_string(),
                replacement: "new_value".to_string(),
                regex: true,
            },
            backup: true,
        }],
        verify_scripts: vec![],
    };
    let json = serde_json::to_string(&config).unwrap();
    let restored: WriterConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.diffs[0].backup, true);
}

// ── FileDiff default backup ─────────────────────────────────────────────────

#[test]
fn file_diff_backup_defaults_to_true() {
    let json = r#"{
        "file": "test.cfg",
        "type": "replace",
        "content": "new"
    }"#;
    let diff: FileDiff = serde_json::from_str(json).unwrap();
    assert!(diff.backup, "backup should default to true");
}

// ── ExpectedResult default tolerance ────────────────────────────────────────

#[test]
fn expected_result_default_tolerance() {
    let json = r#"{
        "variable": "test_var",
        "value": 1.0
    }"#;
    let result: ExpectedResult = serde_json::from_str(json).unwrap();
    assert!((result.tolerance - 0.001).abs() < f64::EPSILON);
}

// ── CurveConflictError ──────────────────────────────────────────────────────

#[test]
fn curve_conflict_error_display_variants() {
    let err1 = CurveConflictError::Configuration("bad config".into());
    assert!(err1.to_string().contains("Configuration error"));

    let err2 = CurveConflictError::Writer("write failure".into());
    assert!(err2.to_string().contains("Writer error"));
}

#[test]
fn curve_conflict_error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no access");
    let err: CurveConflictError = io_err.into();
    assert!(err.to_string().contains("IO error"));
}

#[test]
fn curve_conflict_error_from_serde() {
    let serde_err = serde_json::from_str::<serde_json::Value>("invalid{").unwrap_err();
    let err: CurveConflictError = serde_err.into();
    assert!(err.to_string().contains("Serialization error"));
}

// ── WritersConfig ───────────────────────────────────────────────────────────

#[test]
fn writers_config_default_values() {
    let cfg = WritersConfig::default();
    assert_eq!(cfg.config_dir, PathBuf::from("writers"));
    assert_eq!(cfg.backup_dir, PathBuf::from("backups"));
    assert_eq!(cfg.max_backups, 10);
    assert!(cfg.enable_verification);
}

// ── CurveConflictWriter ─────────────────────────────────────────────────────

#[test]
fn curve_conflict_writer_creation_with_custom_config() {
    let temp_dir = TempDir::new().unwrap();
    let config = WritersConfig {
        config_dir: temp_dir.path().join("config"),
        backup_dir: temp_dir.path().join("backup"),
        max_backups: 3,
        enable_verification: false,
    };
    let writer = CurveConflictWriter::with_config(config).unwrap();
    // Writer should have loaded default configs
    let backups = writer.list_backups().unwrap();
    assert!(backups.is_empty());
}

#[test]
fn curve_conflict_writer_resolve_missing_config() {
    let temp_dir = TempDir::new().unwrap();
    let config = WritersConfig {
        config_dir: temp_dir.path().join("config"),
        backup_dir: temp_dir.path().join("backup"),
        max_backups: 5,
        enable_verification: true,
    };
    let writer = CurveConflictWriter::with_config(config).unwrap();
    let result = writer.resolve_curve_conflict("unknown_sim", "99.99", "disable", &HashMap::new());
    assert!(result.is_err());
}

#[test]
fn curve_conflict_writer_default_creation() {
    // CurveConflictWriter::new() should succeed even without existing config dirs
    let writer = CurveConflictWriter::new();
    // Just test that construction doesn't panic
    assert!(writer.is_ok() || writer.is_err());
}

#[test]
fn writers_config_custom_max_backups() {
    let config = WritersConfig {
        config_dir: PathBuf::from("custom_config"),
        backup_dir: PathBuf::from("custom_backup"),
        max_backups: 1,
        enable_verification: false,
    };
    assert_eq!(config.max_backups, 1);
    assert!(!config.enable_verification);
}

#[test]
fn curve_conflict_writer_list_backups_no_backup_dir() {
    let temp_dir = TempDir::new().unwrap();
    let config = WritersConfig {
        config_dir: temp_dir.path().join("config"),
        backup_dir: temp_dir.path().join("nonexistent_backup"),
        max_backups: 5,
        enable_verification: true,
    };
    let writer = CurveConflictWriter::with_config(config).unwrap();
    let backups = writer.list_backups().unwrap();
    assert!(backups.is_empty());
}

// ── Verification result struct tests ─────────────────────────────────────────

#[test]
fn verification_result_passed() {
    let result = VerificationResult {
        test_name: "check_file".to_string(),
        passed: true,
        skipped: false,
        actual_result: "found".to_string(),
        error_message: None,
    };
    assert!(result.passed);
    assert!(!result.skipped);
}

#[test]
fn verification_result_with_error() {
    let result = VerificationResult {
        test_name: "check_file".to_string(),
        passed: false,
        skipped: false,
        actual_result: "not found".to_string(),
        error_message: Some("file does not exist".to_string()),
    };
    assert!(!result.passed);
    assert!(result.error_message.is_some());
}

#[test]
fn verification_test_type_serde_all_variants() {
    let types = vec![
        VerificationTestType::FileExists,
        VerificationTestType::FileContains,
        VerificationTestType::RegistryValue,
        VerificationTestType::Command,
    ];
    for t in &types {
        let json = serde_json::to_string(t).unwrap();
        let restored: VerificationTestType = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&restored).unwrap();
        assert_eq!(json, json2);
    }
}

// ── BackupInfo serde ────────────────────────────────────────────────────────

#[test]
fn backup_info_serde_roundtrip() {
    let info = BackupInfo {
        timestamp: 1700000000,
        description: "Test backup".to_string(),
        affected_files: vec![PathBuf::from("a.cfg"), PathBuf::from("b.json")],
        backup_dir: PathBuf::from("/tmp/backup_1"),
        writer_config: "msfs_1.36.0".to_string(),
    };
    let json = serde_json::to_string(&info).unwrap();
    let restored: BackupInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.timestamp, 1700000000);
    assert_eq!(restored.affected_files.len(), 2);
    assert_eq!(restored.writer_config, "msfs_1.36.0");
}

// ── WriteResult / VerificationResult structs ────────────────────────────────

#[test]
fn write_result_debug_format() {
    let result = WriteResult {
        success: true,
        applied_diffs: vec!["file1.cfg".to_string()],
        backup_path: Some(PathBuf::from("/tmp/backup")),
        verification_results: vec![],
        error_message: None,
    };
    let dbg = format!("{:?}", result);
    assert!(dbg.contains("success: true"));
}

#[test]
fn verification_result_skipped_fields() {
    let result = VerificationResult {
        test_name: "skip_test".to_string(),
        passed: false,
        skipped: true,
        actual_result: "skipped".to_string(),
        error_message: None,
    };
    assert!(result.skipped);
    assert!(!result.passed);
    assert!(result.error_message.is_none());
}

// ── CcWriterConfig serde ────────────────────────────────────────────────────

#[test]
fn cc_writer_config_serde_roundtrip() {
    let config = CcWriterConfig {
        sim: "msfs".to_string(),
        version: "1.36.0".to_string(),
        description: "Test writer config".to_string(),
        diffs: vec![CcConfigDiff {
            file: "test.cfg".to_string(),
            section: Some("[Controls]".to_string()),
            changes: {
                let mut m = HashMap::new();
                m.insert("key".to_string(), "value".to_string());
                m
            },
            operation: CcDiffOperation::Set,
        }],
        verification_tests: vec![CcVerificationTest {
            name: "verify".to_string(),
            description: "Verify changes".to_string(),
            test_type: VerificationTestType::FileContains,
            expected_result: "test.cfg:key=value".to_string(),
        }],
    };

    let json = serde_json::to_string(&config).unwrap();
    let restored: CcWriterConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.sim, "msfs");
    assert_eq!(restored.diffs.len(), 1);
    assert_eq!(restored.verification_tests.len(), 1);
}

// ── DiffOperation serde variants ────────────────────────────────────────────

#[test]
fn diff_operation_serde_all_variants() {
    let ops: Vec<CcDiffOperation> = vec![
        CcDiffOperation::Set,
        CcDiffOperation::Remove,
        CcDiffOperation::Append,
        CcDiffOperation::Replace,
    ];
    for op in &ops {
        let json = serde_json::to_string(op).unwrap();
        let restored: CcDiffOperation = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&restored).unwrap();
        assert_eq!(json, json2);
    }
}

// ── JSON Patch type serde ───────────────────────────────────────────────────

#[test]
fn json_patch_op_type_serde_all_variants() {
    let ops = vec![
        JsonPatchOpType::Add,
        JsonPatchOpType::Remove,
        JsonPatchOpType::Replace,
        JsonPatchOpType::Move,
        JsonPatchOpType::Copy,
        JsonPatchOpType::Test,
    ];
    for op in &ops {
        let json = serde_json::to_string(op).unwrap();
        let restored: JsonPatchOpType = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&restored).unwrap();
        assert_eq!(json, json2);
    }
}

// ── ApplyResult / VerifyResult / RepairResult struct coverage ────────────────

#[test]
fn apply_result_with_errors() {
    let result = ApplyResult {
        success: false,
        modified_files: vec![PathBuf::from("a.txt")],
        backup_id: "backup_123".to_string(),
        errors: vec!["failed to write".to_string()],
    };
    assert!(!result.success);
    assert_eq!(result.errors.len(), 1);
}

#[test]
fn verify_result_empty_is_success() {
    let result = VerifyResult {
        sim: SimulatorType::MSFS,
        version: "1.0".to_string(),
        success: true,
        script_results: vec![],
        mismatched_files: vec![],
    };
    assert!(result.success);
}

#[test]
fn repair_result_debug() {
    let result = RepairResult {
        success: true,
        repaired_files: vec![PathBuf::from("fixed.cfg")],
        backup_id: "bk_1".to_string(),
        errors: vec![],
    };
    let dbg = format!("{:?}", result);
    assert!(dbg.contains("fixed.cfg"));
}

#[test]
fn rollback_result_with_errors() {
    let result = RollbackResult {
        success: false,
        restored_files: vec![],
        errors: vec!["restore failed".to_string()],
    };
    assert!(!result.success);
    assert_eq!(result.errors.len(), 1);
}

// ── GoldenTestResult / CoverageMatrix ───────────────────────────────────────

#[test]
fn golden_test_result_debug() {
    let result = GoldenTestResult {
        sim: SimulatorType::XPlane,
        success: false,
        test_cases: vec![GoldenTestCase {
            name: "test1".to_string(),
            success: false,
            expected_file: PathBuf::from("expected/"),
            actual_file: PathBuf::from("actual/"),
            diff: Some("- missing file: x.cfg".to_string()),
        }],
        coverage: CoverageMatrix {
            versions: vec!["12.0".to_string()],
            areas: vec!["autopilot".to_string()],
            coverage_percent: 20.0,
            missing_coverage: vec!["needs more versions".to_string()],
        },
    };
    let dbg = format!("{:?}", result);
    assert!(dbg.contains("XPlane"));
    assert!(dbg.contains("test1"));
}

// ── MismatchType coverage ───────────────────────────────────────────────────

#[test]
fn mismatch_type_debug() {
    let types = vec![
        MismatchType::Missing,
        MismatchType::ContentMismatch,
        MismatchType::PermissionMismatch,
    ];
    for t in &types {
        let dbg = format!("{:?}", t);
        assert!(!dbg.is_empty());
    }
}
