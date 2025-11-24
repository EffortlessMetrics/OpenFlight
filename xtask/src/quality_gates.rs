// SPDX-License-Identifier: MIT OR Apache-2.0

//! Quality gate checks for Flight Hub CI pipeline.
//!
//! This module implements the quality gates defined in the sim-integration-implementation spec:
//! - QG-SIM-MAPPING: Verify simulator mapping documentation exists
//! - QG-UNIT-CONV: Verify unit conversion test coverage
//! - QG-SANITY-GATE: Verify sanity gate tests (future)
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
}
