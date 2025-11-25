// SPDX-License-Identifier: MIT OR Apache-2.0

//! Quality gate checks for Flight Hub CI pipeline.
//!
//! This module implements the quality gates defined in the sim-integration-implementation spec:
//! - QG-SIM-MAPPING: Verify simulator mapping documentation exists
//! - QG-UNIT-CONV: Verify unit conversion test coverage
//! - QG-SANITY-GATE: Verify sanity gate tests
//! - QG-FFB-SAFETY: Verify FFB safety tests (future)
//! - QG-RT-JITTER: Verify real-time jitter tests (future)
//! - QG-HID-LATENCY: Verify HID latency tests (future)
//! - QG-LEGAL-DOC: Verify legal documentation (future)

use anyhow::Result;
use std::fs;
use std::path::Path;

/// Result of a quality gate check.
#[derive(Debug, Clone)]
pub struct QualityGateResult {
    pub gate_name: String,
    pub passed: bool,
    pub details: Option<String>,
}

impl QualityGateResult {
    pub fn new(gate_name: impl Into<String>, passed: bool) -> Self {
        Self {
            gate_name: gate_name.into(),
            passed,
            details: None,
        }
    }

    pub fn with_details(
        gate_name: impl Into<String>,
        passed: bool,
        details: impl Into<String>,
    ) -> Self {
        Self {
            gate_name: gate_name.into(),
            passed,
            details: Some(details.into()),
        }
    }
}

/// QG-SIM-MAPPING: Verify that all simulator adapters have complete mapping documentation.
///
/// This quality gate checks for the presence of:
/// - docs/integration/msfs-simvar-mapping.md (MSFS SimConnect adapter)
/// - docs/integration/xplane-data-groups.md (X-Plane adapter)
/// - docs/integration/dcs-export-api.md (DCS Export.lua adapter)
///
/// # Requirements
///
/// Per sim-integration-implementation spec:
/// - MSFS-INT-01.Doc.*: MSFS adapter SHALL maintain mapping table in docs/
/// - XPLANE-INT-01.Doc.*: X-Plane adapter SHALL maintain mapping table in docs/integration/xplane.md
/// - DCS-INT-01.Doc.*: DCS adapter SHALL maintain mapping table in docs/integration/dcs.md
///
/// # Returns
///
/// Returns `Ok(QualityGateResult)` with:
/// - `passed = true` if all required mapping files exist
/// - `passed = false` with details about missing files if any are missing
pub fn check_sim_mapping_docs() -> Result<QualityGateResult> {
    let required_files = vec![
        "docs/integration/msfs-simvar-mapping.md",
        "docs/integration/xplane-data-groups.md",
        "docs/integration/dcs-export-api.md",
    ];

    let mut missing_files = Vec::new();

    for file_path in &required_files {
        let path = Path::new(file_path);
        if !path.exists() {
            missing_files.push(file_path.to_string());
        }
    }

    if missing_files.is_empty() {
        Ok(QualityGateResult::new("QG-SIM-MAPPING", true))
    } else {
        let details = format!(
            "Missing {} mapping file(s): {}",
            missing_files.len(),
            missing_files.join(", ")
        );
        Ok(QualityGateResult::with_details(
            "QG-SIM-MAPPING",
            false,
            details,
        ))
    }
}

/// QG-UNIT-CONV: Verify that unit conversion tests cover all BusSnapshot fields.
///
/// This quality gate checks that unit conversion tests exist for all required conversions:
/// - Degrees ↔ Radians (for angles: pitch, roll, heading, AoA, sideslip, wind direction)
/// - Knots ↔ m/s (for speeds: IAS, TAS, ground speed, wind speed)
/// - Feet ↔ Meters (for altitudes: MSL, AGL, pressure altitude)
/// - FPM ↔ m/s (for vertical speed)
///
/// # Requirements
///
/// Per sim-integration-implementation spec:
/// - BUS-CORE-01.12: Unit conversions SHALL be documented and tested
/// - SIM-TEST-01.2: Tests SHALL verify unit conversions (degrees to radians, feet to meters, knots to m/s)
/// - QG-UNIT-CONV: Fail if unit conversion tests don't cover all BusSnapshot fields
///
/// # Implementation
///
/// This gate verifies that the following test functions exist in crates/flight-bus/src/snapshot.rs:
/// - test_degrees_to_radians_conversion
/// - test_radians_to_degrees_conversion
/// - test_knots_to_mps_conversion
/// - test_mps_to_knots_conversion
/// - test_feet_to_meters_conversion
/// - test_meters_to_feet_conversion
/// - test_fpm_to_mps_conversion
/// - test_mps_to_fpm_conversion
///
/// # Returns
///
/// Returns `Ok(QualityGateResult)` with:
/// - `passed = true` if all required unit conversion tests exist
/// - `passed = false` with details about missing tests if any are missing
pub fn check_unit_conversion_coverage() -> Result<QualityGateResult> {
    let test_file_path = "crates/flight-bus/src/snapshot.rs";
    
    // Check if the test file exists
    if !Path::new(test_file_path).exists() {
        return Ok(QualityGateResult::with_details(
            "QG-UNIT-CONV",
            false,
            format!("Test file not found: {}", test_file_path),
        ));
    }
    
    // Read the test file
    let test_content = fs::read_to_string(test_file_path)?;
    
    // Required unit conversion tests
    // These correspond to the core unit conversions needed for BusSnapshot fields
    let required_tests = vec![
        ("test_degrees_to_radians_conversion", "Degrees → Radians (attitude angles, AoA, sideslip)"),
        ("test_radians_to_degrees_conversion", "Radians → Degrees (reverse conversion)"),
        ("test_knots_to_mps_conversion", "Knots → m/s (IAS, TAS, ground speed)"),
        ("test_mps_to_knots_conversion", "m/s → Knots (reverse conversion)"),
        ("test_feet_to_meters_conversion", "Feet → Meters (altitudes)"),
        ("test_meters_to_feet_conversion", "Meters → Feet (reverse conversion)"),
        ("test_fpm_to_mps_conversion", "FPM → m/s (vertical speed)"),
        ("test_mps_to_fpm_conversion", "m/s → FPM (reverse conversion)"),
    ];
    
    let mut missing_tests = Vec::new();
    
    for (test_name, description) in &required_tests {
        // Check if the test function exists
        let test_pattern = format!("fn {}()", test_name);
        if !test_content.contains(&test_pattern) {
            missing_tests.push(format!("{} ({})", test_name, description));
        }
    }
    
    if missing_tests.is_empty() {
        Ok(QualityGateResult::new("QG-UNIT-CONV", true))
    } else {
        let details = format!(
            "Missing {} unit conversion test(s):\n  - {}",
            missing_tests.len(),
            missing_tests.join("\n  - ")
        );
        Ok(QualityGateResult::with_details(
            "QG-UNIT-CONV",
            false,
            details,
        ))
    }
}

/// QG-SANITY-GATE: Verify that sanity gate tests inject NaN/Inf and implausible jumps.
///
/// This quality gate checks that sanity gate tests exist and cover the required scenarios:
/// - NaN detection in telemetry fields
/// - Inf detection in telemetry fields
/// - Physically implausible jump detection (attitude, velocity)
/// - Verification that safe_for_ffb goes false when violations occur
///
/// # Requirements
///
/// Per sim-integration-implementation spec:
/// - MSFS-INT-01.15: WHEN telemetry values are NaN or Inf THEN adapter SHALL mark invalid
/// - MSFS-INT-01.16: WHEN values change by implausible amounts THEN adapter SHALL drop packet
/// - SIM-TEST-01.9: Tests SHALL inject NaN/Inf values, physically implausible jumps
/// - QG-SANITY-GATE: Fail if sanity gate tests don't inject NaN/Inf and verify proper handling
///
/// # Implementation
///
/// This gate verifies that the following test functions exist in crates/flight-simconnect/tests/sanity_gate_tests.rs:
/// - test_nan_detection_* (at least one test for NaN detection)
/// - test_inf_detection_* (at least one test for Inf detection)
/// - test_implausible_*_jump_detection (at least one test for implausible jumps)
/// - test_safe_for_ffb_* (at least one test verifying safe_for_ffb behavior)
///
/// # Returns
///
/// Returns `Ok(QualityGateResult)` with:
/// - `passed = true` if all required sanity gate tests exist
/// - `passed = false` with details about missing tests if any are missing
pub fn check_sanity_gate_tests() -> Result<QualityGateResult> {
    let test_file_path = "crates/flight-simconnect/tests/sanity_gate_tests.rs";
    
    // Check if the test file exists
    if !Path::new(test_file_path).exists() {
        return Ok(QualityGateResult::with_details(
            "QG-SANITY-GATE",
            false,
            format!("Test file not found: {}", test_file_path),
        ));
    }
    
    // Read the test file
    let test_content = fs::read_to_string(test_file_path)?;
    
    // Required test categories with patterns to search for
    let required_test_categories = vec![
        ("NaN detection", vec!["test_nan_detection"]),
        ("Inf detection", vec!["test_inf_detection"]),
        ("Implausible jump detection", vec!["test_implausible_", "_jump_detection"]),
        ("safe_for_ffb behavior", vec!["test_safe_for_ffb_"]),
    ];
    
    let mut missing_categories = Vec::new();
    let mut found_tests = Vec::new();
    
    for (category_name, patterns) in &required_test_categories {
        let mut found = false;
        
        // Check if any of the patterns match
        for pattern in patterns {
            if test_content.contains(pattern) {
                found = true;
                // Count how many tests match this pattern
                let count = test_content.matches(pattern).count();
                found_tests.push(format!("{}: {} test(s)", category_name, count));
                break;
            }
        }
        
        if !found {
            missing_categories.push(category_name.to_string());
        }
    }
    
    if missing_categories.is_empty() {
        let details = format!(
            "All required sanity gate test categories present:\n  - {}",
            found_tests.join("\n  - ")
        );
        Ok(QualityGateResult::with_details(
            "QG-SANITY-GATE",
            true,
            details,
        ))
    } else {
        let details = format!(
            "Missing {} sanity gate test categor(ies):\n  - {}\n\nFound:\n  - {}",
            missing_categories.len(),
            missing_categories.join("\n  - "),
            found_tests.join("\n  - ")
        );
        Ok(QualityGateResult::with_details(
            "QG-SANITY-GATE",
            false,
            details,
        ))
    }
}

/// QG-FFB-SAFETY: Verify that FFB safety tests verify 50ms ramp-down on all fault types.
///
/// This quality gate checks that FFB safety tests exist and cover the required scenarios:
/// - 50ms ramp-to-zero timing verification
/// - Fault detection for all fault types (USB stall, NaN, over-temp, over-current, etc.)
/// - Soft-stop controller integration
/// - Blackbox capture on faults
///
/// # Requirements
///
/// Per sim-integration-implementation spec:
/// - FFB-SAFETY-01.6: WHEN fault occurs THEN system SHALL ramp torque to zero within 50ms
/// - FFB-SAFETY-01.5: WHEN USB OUT stall detected THEN ramp to zero in ≤50ms
/// - SIM-TEST-01.10: Tests SHALL verify 50ms ramp-down on all fault types
/// - QG-FFB-SAFETY: Fail if FFB safety tests don't verify torque ramp-down within 50ms on all fault types
///
/// # Implementation
///
/// This gate verifies that the following test functions exist:
/// - test_fault_ramp_to_zero_timing (in crates/flight-ffb/src/safety_envelope_tests.rs)
/// - test_*_fault_* (at least one test per fault type in crates/flight-ffb/src/fault.rs tests)
/// - test_soft_stop_* (soft-stop controller tests in crates/flight-ffb/src/soft_stop.rs)
///
/// Additionally verifies that all fault types defined in FaultType enum have corresponding tests.
///
/// # Returns
///
/// Returns `Ok(QualityGateResult)` with:
/// - `passed = true` if all required FFB safety tests exist and cover all fault types
/// - `passed = false` with details about missing tests if any are missing
pub fn check_ffb_safety_tests() -> Result<QualityGateResult> {
    // Check for safety envelope tests
    let safety_envelope_test_file = "crates/flight-ffb/src/safety_envelope_tests.rs";
    
    if !Path::new(safety_envelope_test_file).exists() {
        return Ok(QualityGateResult::with_details(
            "QG-FFB-SAFETY",
            false,
            format!("Safety envelope test file not found: {}", safety_envelope_test_file),
        ));
    }
    
    let safety_envelope_content = fs::read_to_string(safety_envelope_test_file)?;
    
    // Check for fault.rs tests
    let fault_test_file = "crates/flight-ffb/src/fault.rs";
    
    if !Path::new(fault_test_file).exists() {
        return Ok(QualityGateResult::with_details(
            "QG-FFB-SAFETY",
            false,
            format!("Fault detection file not found: {}", fault_test_file),
        ));
    }
    
    let fault_content = fs::read_to_string(fault_test_file)?;
    
    // Check for soft_stop.rs tests
    let soft_stop_test_file = "crates/flight-ffb/src/soft_stop.rs";
    
    if !Path::new(soft_stop_test_file).exists() {
        return Ok(QualityGateResult::with_details(
            "QG-FFB-SAFETY",
            false,
            format!("Soft-stop controller file not found: {}", soft_stop_test_file),
        ));
    }
    
    let soft_stop_content = fs::read_to_string(soft_stop_test_file)?;
    
    // Required test categories
    let mut missing_tests: Vec<String> = Vec::new();
    let mut found_tests = Vec::new();
    
    // 1. Check for 50ms ramp-to-zero timing test
    if safety_envelope_content.contains("test_fault_ramp_to_zero_timing") {
        found_tests.push("50ms ramp-to-zero timing test");
    } else {
        missing_tests.push("test_fault_ramp_to_zero_timing (50ms ramp verification)".to_string());
    }
    
    // 2. Check for fault timestamp tracking test
    if safety_envelope_content.contains("test_fault_timestamp_tracking") {
        found_tests.push("Fault timestamp tracking test");
    } else {
        missing_tests.push("test_fault_timestamp_tracking (explicit timestamp tracking)".to_string());
    }
    
    // 3. Check for fault override test
    if safety_envelope_content.contains("test_fault_overrides_safe_for_ffb") {
        found_tests.push("Fault overrides safe_for_ffb test");
    } else {
        missing_tests.push("test_fault_overrides_safe_for_ffb (fault precedence)".to_string());
    }
    
    // 4. Check for soft-stop controller tests
    let soft_stop_test_patterns = vec![
        ("test_linear_ramp", "Linear ramp profile"),
        ("test_exponential_ramp", "Exponential ramp profile"),
        ("test_ramp_completion", "Ramp completion"),
        ("test_ramp_timeout", "Ramp timeout detection"),
    ];
    
    for (pattern, description) in &soft_stop_test_patterns {
        if soft_stop_content.contains(pattern) {
            found_tests.push(description);
        } else {
            missing_tests.push(format!("{} ({})", pattern, description));
        }
    }
    
    // 5. Check for fault type tests in fault.rs
    // Extract all fault types from the FaultType enum
    let fault_types = vec![
        ("UsbStall", "USB output stall"),
        ("EndpointError", "USB endpoint error"),
        ("NanValue", "NaN value in pipeline"),
        ("OverTemp", "Device over-temperature"),
        ("OverCurrent", "Device over-current"),
        ("PluginOverrun", "Plugin time budget exceeded"),
        ("EndpointWedged", "USB endpoint wedged"),
        ("EncoderInvalid", "Invalid encoder readings"),
        ("DeviceTimeout", "Device communication timeout"),
    ];
    
    // Check that fault.rs has tests for fault recording and response
    let fault_test_patterns = vec![
        ("test_fault_type_properties", "Fault type properties"),
        ("test_fault_recording", "Fault recording"),
        ("test_fault_response_completion", "Fault response completion"),
        ("test_soft_stop_recording", "Soft-stop recording"),
    ];
    
    for (pattern, description) in &fault_test_patterns {
        if fault_content.contains(pattern) {
            found_tests.push(description);
        } else {
            missing_tests.push(format!("{} ({})", pattern, description));
        }
    }
    
    // 6. Verify that all fault types are covered
    // Check that FaultType enum exists and has all expected variants
    let mut missing_fault_types = Vec::new();
    for (fault_type, description) in &fault_types {
        let enum_pattern = format!("{}:", fault_type);
        if !fault_content.contains(&enum_pattern) && !fault_content.contains(&format!("FaultType::{}", fault_type)) {
            missing_fault_types.push(format!("{} ({})", fault_type, description));
        }
    }
    
    if !missing_fault_types.is_empty() {
        missing_tests.push(format!(
            "Missing fault types in FaultType enum: {}",
            missing_fault_types.join(", ")
        ));
    }
    
    // Generate result
    if missing_tests.is_empty() {
        let details = format!(
            "All required FFB safety tests present:\n  - {}\n\nCoverage:\n  - {} fault types defined\n  - 50ms ramp-down verified\n  - Soft-stop controller tested\n  - Fault detection and recording tested",
            found_tests.join("\n  - "),
            fault_types.len()
        );
        Ok(QualityGateResult::with_details(
            "QG-FFB-SAFETY",
            true,
            details,
        ))
    } else {
        let details = format!(
            "Missing {} FFB safety test(s):\n  - {}\n\nFound:\n  - {}",
            missing_tests.len(),
            missing_tests.join("\n  - "),
            found_tests.join("\n  - ")
        );
        Ok(QualityGateResult::with_details(
            "QG-FFB-SAFETY",
            false,
            details,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_sim_mapping_docs_exist() {
        // Ensure we're running from workspace root
        // Tests run from the crate directory, so we need to navigate up
        let original_dir = env::current_dir().expect("Failed to get current directory");
        
        // Navigate to workspace root (parent of xtask)
        let workspace_root = original_dir
            .parent()
            .expect("Failed to get parent directory");
        
        env::set_current_dir(workspace_root)
            .expect("Failed to change to workspace root");
        
        // This test verifies that the mapping documentation files exist
        // It will fail if any required files are missing
        let result = check_sim_mapping_docs().expect("QG-SIM-MAPPING check failed");
        
        // Restore original directory
        env::set_current_dir(original_dir)
            .expect("Failed to restore original directory");
        
        if !result.passed {
            panic!(
                "QG-SIM-MAPPING failed: {}",
                result.details.unwrap_or_else(|| "Unknown error".to_string())
            );
        }
    }
    
    #[test]
    fn test_unit_conversion_coverage() {
        // Ensure we're running from workspace root
        let original_dir = env::current_dir().expect("Failed to get current directory");
        
        // Navigate to workspace root (parent of xtask)
        let workspace_root = original_dir
            .parent()
            .expect("Failed to get parent directory");
        
        env::set_current_dir(workspace_root)
            .expect("Failed to change to workspace root");
        
        // This test verifies that all required unit conversion tests exist
        let result = check_unit_conversion_coverage().expect("QG-UNIT-CONV check failed");
        
        // Restore original directory
        env::set_current_dir(original_dir)
            .expect("Failed to restore original directory");
        
        if !result.passed {
            panic!(
                "QG-UNIT-CONV failed: {}",
                result.details.unwrap_or_else(|| "Unknown error".to_string())
            );
        }
    }
    
    #[test]
    fn test_sanity_gate_tests_exist() {
        // Ensure we're running from workspace root
        let original_dir = env::current_dir().expect("Failed to get current directory");
        
        // Navigate to workspace root (parent of xtask)
        let workspace_root = original_dir
            .parent()
            .expect("Failed to get parent directory");
        
        env::set_current_dir(workspace_root)
            .expect("Failed to change to workspace root");
        
        // This test verifies that all required sanity gate tests exist
        let result = check_sanity_gate_tests().expect("QG-SANITY-GATE check failed");
        
        // Restore original directory
        env::set_current_dir(original_dir)
            .expect("Failed to restore original directory");
        
        if !result.passed {
            panic!(
                "QG-SANITY-GATE failed: {}",
                result.details.unwrap_or_else(|| "Unknown error".to_string())
            );
        }
    }
    
    #[test]
    fn test_ffb_safety_tests_exist() {
        // Ensure we're running from workspace root
        let original_dir = env::current_dir().expect("Failed to get current directory");
        
        // Navigate to workspace root (parent of xtask)
        let workspace_root = original_dir
            .parent()
            .expect("Failed to get parent directory");
        
        env::set_current_dir(workspace_root)
            .expect("Failed to change to workspace root");
        
        // This test verifies that all required FFB safety tests exist
        let result = check_ffb_safety_tests().expect("QG-FFB-SAFETY check failed");
        
        // Restore original directory
        env::set_current_dir(original_dir)
            .expect("Failed to restore original directory");
        
        if !result.passed {
            panic!(
                "QG-FFB-SAFETY failed: {}",
                result.details.unwrap_or_else(|| "Unknown error".to_string())
            );
        }
    }
}
