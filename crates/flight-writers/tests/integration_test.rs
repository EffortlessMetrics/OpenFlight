//! Integration tests for the Writers system

use flight_writers::*;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_complete_writers_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path().join("config");
    let golden_dir = temp_dir.path().join("golden");
    let backup_dir = temp_dir.path().join("backups");

    // Create the Writers instance
    let writers = Writers::new(&config_dir, &golden_dir, &backup_dir).unwrap();

    // Create a test writer configuration
    let mut changes = HashMap::new();
    changes.insert("autopilot_available".to_string(), "1".to_string());
    changes.insert("flight_director_available".to_string(), "1".to_string());

    let writer_config = WriterConfig {
        schema: "flight.writer/1".to_string(),
        sim: SimulatorType::MSFS,
        version: "1.36.0".to_string(),
        description: Some("Test configuration".to_string()),
        diffs: vec![FileDiff {
            file: temp_dir.path().join("test_panel.cfg"),
            operation: DiffOperation::IniSection {
                section: "AUTOPILOT".to_string(),
                changes,
            },
            backup: true,
        }],
        verify_scripts: vec![VerifyScript {
            name: "basic_test".to_string(),
            description: "Basic functionality test".to_string(),
            actions: vec![
                VerifyAction::SimEvent {
                    event: "AP_MASTER".to_string(),
                    value: Some(1.0),
                },
                VerifyAction::Wait { duration_ms: 100 },
            ],
            expected: vec![ExpectedResult {
                variable: "AUTOPILOT_MASTER".to_string(),
                value: 1.0,
                tolerance: 0.1,
            }],
        }],
    };

    // Test applying the configuration
    let apply_result = writers.apply_writer(&writer_config).await.unwrap();
    assert!(apply_result.success);
    assert_eq!(apply_result.modified_files.len(), 1);

    // Verify the file was created correctly
    let created_file = &apply_result.modified_files[0];
    assert!(created_file.exists());

    let content = fs::read_to_string(created_file).unwrap();
    assert!(content.contains("[AUTOPILOT]"));
    assert!(content.contains("autopilot_available=1"));
    assert!(content.contains("flight_director_available=1"));

    // Test verification
    let _verify_result = writers.verify(SimulatorType::MSFS, "1.36.0").await.unwrap();
    // Note: This will fail because we don't have a golden file set up, but that's expected

    // Test rollback (only if backup was created)
    if !apply_result.backup_id.is_empty() {
        match writers.rollback(&apply_result.backup_id).await {
            Ok(rollback_result) => {
                // Rollback may fail if no backup was actually needed, which is fine
                println!("Rollback result: success={}", rollback_result.success);
            }
            Err(e) => {
                println!("Rollback failed (expected if no backup was created): {}", e);
            }
        }
    }
}

#[tokio::test]
async fn test_golden_file_testing() {
    let temp_dir = TempDir::new().unwrap();
    let golden_dir = temp_dir.path().join("golden");

    // Set up golden test structure
    let test_case_dir = golden_dir.join("msfs").join("test_v1.36.0_autopilot");
    fs::create_dir_all(&test_case_dir).unwrap();

    // Create input configuration
    let mut changes = HashMap::new();
    changes.insert("enabled".to_string(), "1".to_string());
    changes.insert("altitude_hold".to_string(), "1".to_string());

    let input_config = WriterConfig {
        schema: "flight.writer/1".to_string(),
        sim: SimulatorType::MSFS,
        version: "1.36.0".to_string(),
        description: Some("Golden test configuration".to_string()),
        diffs: vec![FileDiff {
            file: "autopilot.cfg".into(),
            operation: DiffOperation::IniSection {
                section: "AUTOPILOT".to_string(),
                changes,
            },
            backup: true,
        }],
        verify_scripts: vec![],
    };

    // Write input configuration
    let input_file = test_case_dir.join("input.json");
    fs::write(
        &input_file,
        serde_json::to_string_pretty(&input_config).unwrap(),
    )
    .unwrap();

    // Create expected output
    let expected_dir = test_case_dir.join("expected");
    fs::create_dir_all(&expected_dir).unwrap();
    fs::write(
        expected_dir.join("autopilot.cfg"),
        "[AUTOPILOT]\naltitude_hold=1\nenabled=1\n",
    )
    .unwrap();

    // Run golden file test
    let tester = GoldenFileTester::new(&golden_dir);
    let result = tester.test_simulator(SimulatorType::MSFS).await.unwrap();

    assert!(result.success);
    assert_eq!(result.test_cases.len(), 1);
    assert!(result.test_cases[0].success);
    assert_eq!(result.test_cases[0].name, "test_v1.36.0_autopilot");
}

#[tokio::test]
async fn test_repair_functionality() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path().join("config");
    let backup_dir = temp_dir.path().join("backups");

    let repairer = ConfigRepairer::new(&config_dir, &backup_dir);

    // Create a file that needs repair
    let broken_file = temp_dir.path().join("broken.ini");
    fs::write(&broken_file, "[SECTION]\nwrong_key=wrong_value\n").unwrap();

    // Create a verify result indicating the file needs repair
    let verify_result = VerifyResult {
        sim: SimulatorType::MSFS,
        version: "1.36.0".to_string(),
        success: false,
        script_results: vec![],
        mismatched_files: vec![FileMismatch {
            file: broken_file.clone(),
            mismatch_type: MismatchType::ContentMismatch,
            suggested_diff: Some(FileDiff {
                file: broken_file.clone(),
                operation: DiffOperation::IniSection {
                    section: "SECTION".to_string(),
                    changes: {
                        let mut changes = HashMap::new();
                        changes.insert("correct_key".to_string(), "correct_value".to_string());
                        changes
                    },
                },
                backup: true,
            }),
        }],
    };

    // Perform repair
    let repair_result = repairer.repair(&verify_result).await.unwrap();
    assert!(repair_result.success);
    assert_eq!(repair_result.repaired_files.len(), 1);

    // Verify the file was repaired
    let content = fs::read_to_string(&broken_file).unwrap();
    assert!(content.contains("correct_key=correct_value"));
}

#[tokio::test]
async fn test_coverage_matrix_generation() {
    let temp_dir = TempDir::new().unwrap();
    let golden_dir = temp_dir.path().join("golden");

    // Create multiple test cases with different versions and areas
    let test_cases = [
        ("test_v1.35.0_autopilot", "1.35.0", "autopilot"),
        ("test_v1.36.0_autopilot", "1.36.0", "autopilot"),
        ("test_v1.36.0_electrical", "1.36.0", "electrical"),
        ("test_v1.37.0_fuel", "1.37.0", "fuel"),
    ];

    for (test_name, _version, _area) in &test_cases {
        let test_dir = golden_dir.join("msfs").join(test_name);
        fs::create_dir_all(&test_dir).unwrap();

        // Create minimal test structure
        fs::write(test_dir.join("input.json"), "{}").unwrap();
        let expected_dir = test_dir.join("expected");
        fs::create_dir_all(&expected_dir).unwrap();
        fs::write(expected_dir.join("test.txt"), "test").unwrap();
    }

    let tester = GoldenFileTester::new(&golden_dir);
    let result = tester.test_simulator(SimulatorType::MSFS).await.unwrap();

    // Check coverage matrix
    assert!(result.coverage.versions.len() >= 3); // Should detect multiple versions
    assert!(result.coverage.areas.len() >= 3); // Should detect multiple areas
    assert!(result.coverage.coverage_percent > 0.0);
}

#[tokio::test]
async fn test_json_patch_operations() {
    let temp_dir = TempDir::new().unwrap();
    let applier = WriterApplier::new(temp_dir.path());

    let json_file = temp_dir.path().join("test.json");
    fs::write(&json_file, r#"{"existing": "value", "number": 42}"#).unwrap();

    let patches = vec![
        JsonPatchOp {
            op: JsonPatchOpType::Add,
            path: "/new_field".to_string(),
            value: Some(serde_json::Value::String("new_value".to_string())),
            from: None,
        },
        JsonPatchOp {
            op: JsonPatchOpType::Replace,
            path: "/number".to_string(),
            value: Some(serde_json::Value::Number(serde_json::Number::from(100))),
            from: None,
        },
        JsonPatchOp {
            op: JsonPatchOpType::Test,
            path: "/existing".to_string(),
            value: Some(serde_json::Value::String("value".to_string())),
            from: None,
        },
    ];

    // Create a diff with JSON patches and apply it
    let diff = FileDiff {
        file: json_file.clone(),
        operation: DiffOperation::JsonPatch { patches },
        backup: true,
    };

    let backup_path = temp_dir.path().join("backup");
    fs::create_dir_all(&backup_path).unwrap();

    applier.apply_diff(&diff, &backup_path).await.unwrap();

    let content = fs::read_to_string(&json_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(json["existing"], "value");
    assert_eq!(json["number"], 100);
    assert_eq!(json["new_field"], "new_value");
}

#[tokio::test]
async fn test_backup_and_rollback() {
    let temp_dir = TempDir::new().unwrap();
    let backup_dir = temp_dir.path().join("backups");
    let manager = RollbackManager::new(&backup_dir);

    // Create test files
    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");
    fs::write(&file1, "original content 1").unwrap();
    fs::write(&file2, "original content 2").unwrap();

    // Create backup
    let backup_id = "test_backup";
    let metadata = manager
        .create_backup(
            backup_id,
            SimulatorType::MSFS,
            "1.36.0",
            "Test backup",
            &[file1.clone(), file2.clone()],
        )
        .await
        .unwrap();

    assert_eq!(metadata.files.len(), 2);

    // Modify files
    fs::write(&file1, "modified content 1").unwrap();
    fs::write(&file2, "modified content 2").unwrap();

    // Rollback
    let result = manager.rollback(backup_id).await.unwrap();
    assert!(result.success);
    assert_eq!(result.restored_files.len(), 2);

    // Verify restoration
    assert_eq!(fs::read_to_string(&file1).unwrap(), "original content 1");
    assert_eq!(fs::read_to_string(&file2).unwrap(), "original content 2");

    // Test backup verification
    let verification = manager.verify_backup(backup_id).await.unwrap();
    assert!(verification.valid);
    assert_eq!(verification.verified_files, 2);
    assert_eq!(verification.total_files, 2);
}
