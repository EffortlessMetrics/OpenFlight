// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Additional snapshot tests for `flight-writers` types and diff table formats.
//!
//! Captures the serialisation shape of every `DiffOperation` variant and
//! default `WritersConfig` values so regressions are caught early.
//! Run `cargo insta review` to accept new or changed snapshots.

use flight_writers::{
    CcWriterConfig, CcConfigDiff, CcDiffOperation, CcVerificationTest, VerificationTestType,
    WritersConfig,
};
use flight_writers::{
    DiffOperation, FileDiff, JsonPatchOp, JsonPatchOpType, SimulatorType, VerifyScript,
    WriterConfig,
};
use std::collections::HashMap;
use std::path::PathBuf;

// ── WritersConfig defaults ───────────────────────────────────────────────────

#[test]
fn snapshot_writers_config_default() {
    let cfg = WritersConfig::default();
    insta::assert_debug_snapshot!("writers_config_default", cfg);
}

// ── DiffOperation variant serialisation ──────────────────────────────────────

#[test]
fn snapshot_diff_operation_replace_json() {
    let diff = FileDiff {
        file: PathBuf::from("MSFS/UserCfg.opt"),
        operation: DiffOperation::Replace {
            content: "[CONTROLS]\nUseLinearCurves=1\n".into(),
        },
        backup: true,
    };
    insta::assert_json_snapshot!("diff_op_replace", diff);
}

#[test]
fn snapshot_diff_operation_ini_section_json() {
    let mut changes = HashMap::new();
    changes.insert("UseLinearCurves".into(), "1".into());
    changes.insert("DisableNonLinearControls".into(), "1".into());

    let diff = FileDiff {
        file: PathBuf::from("MSFS/UserCfg.opt"),
        operation: DiffOperation::IniSection {
            section: "CONTROLS".into(),
            changes,
        },
        backup: true,
    };
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("diff_op_ini_section", diff);
    });
}

#[test]
fn snapshot_diff_operation_json_patch_json() {
    let diff = FileDiff {
        file: PathBuf::from("XPlane/preferences.json"),
        operation: DiffOperation::JsonPatch {
            patches: vec![
                JsonPatchOp {
                    op: JsonPatchOpType::Add,
                    path: "/controls/linear".into(),
                    value: Some(serde_json::json!(true)),
                    from: None,
                },
                JsonPatchOp {
                    op: JsonPatchOpType::Replace,
                    path: "/controls/sensitivity".into(),
                    value: Some(serde_json::json!(1.0)),
                    from: None,
                },
            ],
        },
        backup: false,
    };
    insta::assert_json_snapshot!("diff_op_json_patch", diff);
}

#[test]
fn snapshot_diff_operation_line_replace_json() {
    let diff = FileDiff {
        file: PathBuf::from("DCS/options.lua"),
        operation: DiffOperation::LineReplace {
            pattern: r#"["curves"] = true"#.into(),
            replacement: r#"["curves"] = false"#.into(),
            regex: false,
        },
        backup: true,
    };
    insta::assert_json_snapshot!("diff_op_line_replace", diff);
}

// ── Full WriterConfig for each simulator ─────────────────────────────────────

#[test]
fn snapshot_writer_config_xplane() {
    let config = WriterConfig {
        schema: "flight.writer/1".into(),
        sim: SimulatorType::XPlane,
        version: "12.0".into(),
        description: Some("Disable X-Plane built-in curves".into()),
        diffs: vec![FileDiff {
            file: PathBuf::from("Output/preferences/X-Plane Window Positions.prf"),
            operation: DiffOperation::LineReplace {
                pattern: "joy_linear 0".into(),
                replacement: "joy_linear 1".into(),
                regex: false,
            },
            backup: true,
        }],
        verify_scripts: vec![],
    };
    insta::assert_json_snapshot!("writer_config_xplane_12_0", config);
}

#[test]
fn snapshot_writer_config_dcs() {
    let mut changes = HashMap::new();
    changes.insert("use_linear_controls".into(), "true".into());

    let config = WriterConfig {
        schema: "flight.writer/1".into(),
        sim: SimulatorType::DCS,
        version: "2.9".into(),
        description: Some("Apply linear control curves for DCS".into()),
        diffs: vec![FileDiff {
            file: PathBuf::from("Config/options.lua"),
            operation: DiffOperation::IniSection {
                section: "controls".into(),
                changes,
            },
            backup: true,
        }],
        verify_scripts: vec![],
    };
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("writer_config_dcs_2_9", config);
    });
}

// ── CurveConflict writer config ──────────────────────────────────────────────

#[test]
fn snapshot_cc_writer_config_json() {
    let mut changes = HashMap::new();
    changes.insert("UseLinearCurves".into(), "1".into());

    let config = CcWriterConfig {
        sim: "msfs".into(),
        version: "1.36.0".into(),
        description: "Disable MSFS built-in control curves".into(),
        diffs: vec![CcConfigDiff {
            file: "UserCfg.opt".into(),
            section: Some("CONTROLS".into()),
            changes,
            operation: CcDiffOperation::Set,
        }],
        verification_tests: vec![CcVerificationTest {
            name: "check_linear_curves".into(),
            description: "Verify UseLinearCurves is set".into(),
            test_type: VerificationTestType::FileContains,
            expected_result: "UseLinearCurves=1".into(),
        }],
    };
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("cc_writer_config_msfs", config);
    });
}

// ── SimulatorType display ────────────────────────────────────────────────────

#[test]
fn snapshot_simulator_type_display() {
    let types = [SimulatorType::MSFS, SimulatorType::XPlane, SimulatorType::DCS];
    let output: Vec<String> = types.iter().map(|t| format!("{t}")).collect();
    insta::assert_debug_snapshot!("simulator_type_display", output);
}
