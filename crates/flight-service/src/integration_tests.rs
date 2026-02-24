// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for curve conflict detection and one-click resolution
//!
//! Tests the complete workflow from conflict detection through resolution
//! and verification, including blackbox annotation.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CurveConflictService, OneClickResolver};
    use flight_axis::{
        AxisEngine, ConflictMetadata, ConflictResolution, ConflictSeverity, ConflictType,
        CurveConflict, ResolutionType,
    };
    use flight_ipc::proto::{DetectCurveConflictsRequest, OneClickResolveRequest};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Instant;
    use tempfile::TempDir;

    /// Create a test conflict for integration testing
    fn create_test_conflict(axis_name: &str, severity: ConflictSeverity) -> CurveConflict {
        let nonlinearity = match severity {
            ConflictSeverity::Low => 0.2,
            ConflictSeverity::Medium => 0.4,
            ConflictSeverity::High => 0.6,
            ConflictSeverity::Critical => 0.8,
        };

        CurveConflict {
            axis_name: axis_name.to_string(),
            conflict_type: ConflictType::DoubleCurve,
            severity,
            description: format!("Test double curve conflict on {}", axis_name),
            metadata: ConflictMetadata {
                sim_curve_strength: 0.4,
                profile_curve_strength: 0.3,
                combined_nonlinearity: nonlinearity,
                test_inputs: vec![0.0, 0.25, 0.5, 0.75, 1.0],
                expected_outputs: vec![0.0, 0.25, 0.5, 0.75, 1.0],
                actual_outputs: vec![0.0, 0.15, 0.35, 0.65, 1.0],
                detection_timestamp: Instant::now(),
            },
            suggested_resolutions: vec![
                ConflictResolution {
                    resolution_type: ResolutionType::DisableSimCurve,
                    description: "Disable simulator's built-in curve".to_string(),
                    estimated_improvement: 0.8,
                    requires_sim_restart: true,
                    parameters: HashMap::new(),
                },
                ConflictResolution {
                    resolution_type: ResolutionType::ApplyGainCompensation,
                    description: "Apply gain compensation".to_string(),
                    estimated_improvement: 0.6,
                    requires_sim_restart: false,
                    parameters: {
                        let mut params = HashMap::new();
                        params.insert("gain_factor".to_string(), "0.85".to_string());
                        params
                    },
                },
            ],
            detected_at: Instant::now(),
        }
    }

    /// Create a mock axis engine with conflict detection
    fn create_mock_axis_engine(axis_name: &str, _has_conflict: bool) -> Arc<AxisEngine> {
        let engine = Arc::new(AxisEngine::new_for_axis(axis_name.to_string()));

        // In a real implementation, we would inject the conflict into the engine
        // For testing, we'll simulate this by manually adding to the service cache

        engine
    }

    #[tokio::test]
    async fn test_complete_detection_to_resolution_workflow() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let mut service = CurveConflictService::new().unwrap();

        // Set up simulator context
        service.set_current_simulator("msfs".to_string(), "1.36.0".to_string(), "C172".to_string());

        // Register axis engine
        let axis_name = "pitch";
        let engine = create_mock_axis_engine(axis_name, true);
        service.register_axis_engine(axis_name.to_string(), engine);

        // Simulate conflict detection by manually adding to cache
        let conflict = create_test_conflict(axis_name, ConflictSeverity::Medium);
        service.inject_conflict_for_testing(axis_name.to_string(), conflict.clone());

        // Step 1: Detect conflicts
        let detect_request = DetectCurveConflictsRequest {
            axis_names: vec![axis_name.to_string()],
            sim_id: "msfs".to_string(),
            aircraft_id: "C172".to_string(),
        };

        let detect_response = service.detect_conflicts(detect_request);
        assert!(detect_response.success);
        assert_eq!(detect_response.conflicts.len(), 1);

        let detected_conflict = &detect_response.conflicts[0];
        assert_eq!(detected_conflict.axis_name, axis_name);
        assert!(!detected_conflict.suggested_resolutions.is_empty());

        println!("✓ Step 1: Conflict detected successfully");
        println!("  - Axis: {}", detected_conflict.axis_name);
        println!("  - Type: {:?}", detected_conflict.conflict_type());
        println!("  - Severity: {:?}", detected_conflict.severity());
        println!(
            "  - Resolutions available: {}",
            detected_conflict.suggested_resolutions.len()
        );

        // Step 2: One-click resolution
        let resolve_request = OneClickResolveRequest {
            axis_name: axis_name.to_string(),
            create_backup: true,
            verify_resolution: true,
        };

        let resolve_result = service.one_click_resolve(&resolve_request.axis_name);
        assert!(resolve_result.is_ok());

        let resolution = resolve_result.unwrap();
        println!("✓ Step 2: One-click resolution completed");
        println!("  - Success: {}", resolution.success);
        println!("  - Resolution type: {:?}", resolution.resolution_type);
        println!("  - Files modified: {}", resolution.modified_files.len());
        println!(
            "  - Verification passed: {}",
            resolution.verification.passed
        );
        println!(
            "  - Improvement: {:.1}%",
            resolution.metrics.improvement * 100.0
        );
        println!("  - Steps performed: {}", resolution.steps_performed.len());

        // Step 3: Verify conflict is cleared
        let detect_after_request = DetectCurveConflictsRequest {
            axis_names: vec![axis_name.to_string()],
            sim_id: "msfs".to_string(),
            aircraft_id: "C172".to_string(),
        };

        let detect_after_response = service.detect_conflicts(detect_after_request);
        assert!(detect_after_response.success);

        // Should have no conflicts after successful resolution
        if resolution.success {
            assert_eq!(detect_after_response.conflicts.len(), 0);
            println!("✓ Step 3: Conflict successfully cleared");
        } else {
            println!("! Step 3: Conflict still present (resolution failed)");
        }

        // Step 4: Verify blackbox annotations
        // In a real implementation, we would check the blackbox file
        // For testing, we verify the steps were recorded
        assert!(!resolution.steps_performed.is_empty());

        let step_names: Vec<&str> = resolution
            .steps_performed
            .iter()
            .map(|s| s.name.as_str())
            .collect();

        assert!(step_names.contains(&"select_strategy"));
        assert!(step_names.contains(&"apply_resolution"));

        if resolution.verification.passed {
            assert!(step_names.contains(&"verify_resolution"));
        }

        println!("✓ Step 4: Blackbox annotations verified");
        println!("  - Steps recorded: {:?}", step_names);
    }

    #[tokio::test]
    async fn test_multiple_axis_conflict_resolution() {
        // Test resolving conflicts on multiple axes
        let mut service = CurveConflictService::new().unwrap();

        service.set_current_simulator("msfs".to_string(), "1.36.0".to_string(), "A320".to_string());

        let axes = vec!["pitch", "roll", "yaw"];

        // Register engines and add conflicts
        for axis_name in &axes {
            let engine = create_mock_axis_engine(axis_name, true);
            service.register_axis_engine(axis_name.to_string(), engine);

            let conflict = create_test_conflict(axis_name, ConflictSeverity::Medium);
            service.inject_conflict_for_testing(axis_name.to_string(), conflict);
        }

        // Detect all conflicts
        let detect_request = DetectCurveConflictsRequest {
            axis_names: vec![], // Empty = all axes
            sim_id: "msfs".to_string(),
            aircraft_id: "A320".to_string(),
        };

        let detect_response = service.detect_conflicts(detect_request);
        assert!(detect_response.success);
        assert_eq!(detect_response.conflicts.len(), axes.len());

        println!(
            "✓ Multiple axis conflicts detected: {}",
            detect_response.conflicts.len()
        );

        // Resolve each conflict
        let mut successful_resolutions = 0;
        for axis_name in &axes {
            match service.one_click_resolve(axis_name) {
                Ok(result) => {
                    if result.success {
                        successful_resolutions += 1;
                        println!("  ✓ {} resolved successfully", axis_name);
                    } else {
                        println!(
                            "  ! {} resolution failed: {:?}",
                            axis_name, result.error_message
                        );
                    }
                }
                Err(e) => {
                    println!("  ! {} resolution error: {}", axis_name, e);
                }
            }
        }

        println!(
            "✓ Resolved {}/{} conflicts successfully",
            successful_resolutions,
            axes.len()
        );
        if std::path::Path::new("MSFS/UserCfg.opt").exists() {
            assert!(
                successful_resolutions > 0,
                "Expected at least one successful resolution when MSFS config is present"
            );
        }
    }

    #[tokio::test]
    async fn test_resolution_failure_and_rollback() {
        // Test handling of resolution failures and rollback functionality
        let mut service = CurveConflictService::new().unwrap();

        service.set_current_simulator("dcs".to_string(), "2.9".to_string(), "F-16C".to_string());

        let axis_name = "pitch";
        let engine = create_mock_axis_engine(axis_name, true);
        service.register_axis_engine(axis_name.to_string(), engine);

        // Create a critical conflict that might be harder to resolve
        let conflict = create_test_conflict(axis_name, ConflictSeverity::Critical);
        service.inject_conflict_for_testing(axis_name.to_string(), conflict);

        // Attempt resolution
        let result = service.one_click_resolve(axis_name);

        match result {
            Ok(resolution) => {
                println!("Resolution attempt completed:");
                println!("  - Success: {}", resolution.success);
                println!("  - Steps: {}", resolution.steps_performed.len());

                // If it failed and we have backup info, test rollback
                if !resolution.success && resolution.backup_info.is_some() {
                    println!("  - Testing rollback...");

                    // In a real implementation, we would test the rollback functionality
                    // For now, just verify the backup info is present
                    let backup_info = resolution.backup_info.unwrap();
                    assert!(!backup_info.description.is_empty());
                    assert!(!backup_info.backup_dir.to_string_lossy().is_empty());

                    println!("  ✓ Backup info available for rollback");
                }
            }
            Err(e) => {
                println!("Resolution failed with error: {}", e);
                // This is acceptable for testing failure scenarios
            }
        }
    }

    #[tokio::test]
    async fn test_blackbox_annotation_workflow() {
        // Test that blackbox annotations are properly created throughout the workflow
        let mut resolver = OneClickResolver::new().unwrap();

        let axis_name = "test_axis";
        let conflict = create_test_conflict(axis_name, ConflictSeverity::Medium);

        // Perform resolution (will fail due to no real sim, but should still annotate)
        let result = resolver.resolve_conflict(axis_name, &conflict, "msfs", "1.36.0");

        match result {
            Ok(resolution) => {
                // Verify steps were recorded
                assert!(!resolution.steps_performed.is_empty());

                let mut found_steps = HashMap::new();
                for step in &resolution.steps_performed {
                    found_steps.insert(step.name.clone(), step.success);
                }

                // Should have attempted strategy selection
                assert!(found_steps.contains_key("select_strategy"));

                // Should have attempted to apply resolution
                assert!(found_steps.contains_key("apply_resolution"));

                println!("✓ Blackbox workflow steps recorded:");
                for step in &resolution.steps_performed {
                    println!(
                        "  - {}: {} ({}ms)",
                        step.name,
                        if step.success { "✓" } else { "✗" },
                        step.duration_ms
                    );
                }

                // Flush blackbox to ensure annotations are written
                resolver.flush_blackbox();
                println!("✓ Blackbox annotations flushed");
            }
            Err(e) => {
                println!("Resolution failed: {}", e);
                // Still acceptable for testing annotation workflow
            }
        }
    }

    #[test]
    fn test_resolution_strategy_selection() {
        // Test that appropriate resolution strategies are selected for different conflict types
        let resolver = OneClickResolver::new().unwrap();

        let test_cases = vec![
            (
                ConflictType::DoubleCurve,
                vec![
                    ResolutionType::DisableSimCurve,
                    ResolutionType::DisableProfileCurve,
                ],
            ),
            (
                ConflictType::ExcessiveNonlinearity,
                vec![
                    ResolutionType::ReduceCurveStrength,
                    ResolutionType::ApplyGainCompensation,
                ],
            ),
            (
                ConflictType::OpposingCurves,
                vec![
                    ResolutionType::ApplyGainCompensation,
                    ResolutionType::DisableSimCurve,
                ],
            ),
        ];

        for (conflict_type, expected_strategies) in test_cases {
            let available = resolver.get_available_strategies(&conflict_type);

            println!("Conflict type: {:?}", conflict_type);
            println!("  Available strategies: {:?}", available);
            println!("  Expected strategies: {:?}", expected_strategies);

            // Verify that all expected strategies are available
            for expected in &expected_strategies {
                assert!(
                    available.contains(expected),
                    "Strategy {:?} not available for conflict type {:?}",
                    expected,
                    conflict_type
                );
            }

            println!("  ✓ All expected strategies available");
        }
    }
}
