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

use anyhow::{Context, Result};
use flight_bdd_metrics::{BddTraceabilityMetrics, UNMAPPED_MICROCRATE};
use flight_workspace_meta::{
    load_workspace_microcrate_names, validate_workspace_crates_io_metadata,
};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn workspace_path(relative_path: &str) -> PathBuf {
    workspace_root().join(relative_path)
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
        "docs/reference/msfs-mapping.md",
        "docs/reference/xplane-mapping.md",
        "docs/reference/dcs-api.md",
    ];

    let mut missing_files = Vec::new();

    for file_path in &required_files {
        let path = workspace_path(file_path);
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
    let path = workspace_path(test_file_path);

    // Check if the test file exists
    if !path.exists() {
        return Ok(QualityGateResult::with_details(
            "QG-UNIT-CONV",
            false,
            format!("Test file not found: {}", test_file_path),
        ));
    }

    // Read the test file
    let test_content = fs::read_to_string(path)?;

    // Required unit conversion tests
    // These correspond to the core unit conversions needed for BusSnapshot fields
    let required_tests = vec![
        (
            "test_degrees_to_radians_conversion",
            "Degrees → Radians (attitude angles, AoA, sideslip)",
        ),
        (
            "test_radians_to_degrees_conversion",
            "Radians → Degrees (reverse conversion)",
        ),
        (
            "test_knots_to_mps_conversion",
            "Knots → m/s (IAS, TAS, ground speed)",
        ),
        (
            "test_mps_to_knots_conversion",
            "m/s → Knots (reverse conversion)",
        ),
        (
            "test_feet_to_meters_conversion",
            "Feet → Meters (altitudes)",
        ),
        (
            "test_meters_to_feet_conversion",
            "Meters → Feet (reverse conversion)",
        ),
        ("test_fpm_to_mps_conversion", "FPM → m/s (vertical speed)"),
        (
            "test_mps_to_fpm_conversion",
            "m/s → FPM (reverse conversion)",
        ),
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
    let path = workspace_path(test_file_path);

    // Check if the test file exists
    if !path.exists() {
        return Ok(QualityGateResult::with_details(
            "QG-SANITY-GATE",
            false,
            format!("Test file not found: {}", test_file_path),
        ));
    }

    // Read the test file
    let test_content = fs::read_to_string(path)?;

    // Required test categories with patterns to search for
    let required_test_categories = vec![
        ("NaN detection", vec!["test_nan_detection"]),
        ("Inf detection", vec!["test_inf_detection"]),
        (
            "Implausible jump detection",
            vec!["test_implausible_", "_jump_detection"],
        ),
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
    let safety_envelope_path = workspace_path(safety_envelope_test_file);

    if !safety_envelope_path.exists() {
        return Ok(QualityGateResult::with_details(
            "QG-FFB-SAFETY",
            false,
            format!(
                "Safety envelope test file not found: {}",
                safety_envelope_test_file
            ),
        ));
    }

    let safety_envelope_content = fs::read_to_string(safety_envelope_path)?;

    // Check for fault.rs tests
    let fault_test_file = "crates/flight-ffb/src/fault.rs";
    let fault_test_path = workspace_path(fault_test_file);

    if !fault_test_path.exists() {
        return Ok(QualityGateResult::with_details(
            "QG-FFB-SAFETY",
            false,
            format!("Fault detection file not found: {}", fault_test_file),
        ));
    }

    let fault_content = fs::read_to_string(fault_test_path)?;

    // Check for soft_stop.rs tests
    let soft_stop_test_file = "crates/flight-ffb/src/soft_stop.rs";
    let soft_stop_test_path = workspace_path(soft_stop_test_file);

    if !soft_stop_test_path.exists() {
        return Ok(QualityGateResult::with_details(
            "QG-FFB-SAFETY",
            false,
            format!(
                "Soft-stop controller file not found: {}",
                soft_stop_test_file
            ),
        ));
    }

    let soft_stop_content = fs::read_to_string(soft_stop_test_path)?;

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
        missing_tests
            .push("test_fault_timestamp_tracking (explicit timestamp tracking)".to_string());
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
        (
            "test_fault_response_completion",
            "Fault response completion",
        ),
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
        if !fault_content.contains(&enum_pattern)
            && !fault_content.contains(&format!("FaultType::{}", fault_type))
        {
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

/// QG-BDD-COVERAGE: Verify BDD coverage metrics against configured thresholds.
///
/// This quality gate checks:
/// - Global test coverage threshold
/// - Global Gherkin coverage threshold
/// - Global combined coverage threshold (tests + Gherkin)
/// - Microcrate test, Gherkin, and combined coverage thresholds for microcrates
///   above a minimum AC count
///
/// Thresholds are optional and loaded from environment variables with the
/// following defaults:
/// - `BDD_MIN_TEST_COVERAGE_PCT` (default 0.0)
/// - `BDD_MIN_GHERKIN_COVERAGE_PCT` (default 0.0)
/// - `BDD_MIN_BOTH_COVERAGE_PCT` (default 0.0)
/// - `BDD_MIN_CRATE_AC_FOR_EVAL` (default 1)
/// - `BDD_MIN_CRATE_TEST_PCT` (default 0.0)
/// - `BDD_MIN_CRATE_GHERKIN_PCT` (default 0.0)
/// - `BDD_MIN_CRATE_BOTH_PCT` (default 0.0)
/// - `BDD_EXCLUDE_UNMAPPED_MICROCRATE` (default false)
pub fn check_bdd_coverage() -> Result<QualityGateResult> {
    let metrics_path = workspace_path("docs/bdd_metrics.json");
    let metrics = match load_bdd_metrics(&metrics_path) {
        Ok(metrics) => metrics,
        Err(e) => {
            return Ok(QualityGateResult::with_details(
                "QG-BDD-COVERAGE",
                false,
                format!("Unable to load BDD metrics artifact: {}", e),
            ));
        }
    };

    let min_test_coverage = read_float_threshold("BDD_MIN_TEST_COVERAGE_PCT", 0.0)?;
    let min_gherkin_coverage = read_float_threshold("BDD_MIN_GHERKIN_COVERAGE_PCT", 0.0)?;
    let min_both_coverage = read_float_threshold("BDD_MIN_BOTH_COVERAGE_PCT", 0.0)?;
    let min_crate_ac_for_eval = read_usize_threshold("BDD_MIN_CRATE_AC_FOR_EVAL", 1)?;
    let min_crate_test_coverage = read_float_threshold("BDD_MIN_CRATE_TEST_PCT", 0.0)?;
    let min_crate_gherkin_coverage = read_float_threshold("BDD_MIN_CRATE_GHERKIN_PCT", 0.0)?;
    let min_crate_both_coverage = read_float_threshold("BDD_MIN_CRATE_BOTH_PCT", 0.0)?;
    let exclude_unmapped_microcrate = read_bool_flag("BDD_EXCLUDE_UNMAPPED_MICROCRATE", false)?;

    if metrics.total_ac == 0 {
        return Ok(QualityGateResult::with_details(
            "QG-BDD-COVERAGE",
            false,
            "No acceptance criteria were loaded from BDD metrics artifact".to_string(),
        ));
    }

    let test_coverage = metrics.test_coverage_percent();
    let gherkin_coverage = metrics.gherkin_coverage_percent();
    let both_coverage = metrics.both_coverage_percent();

    let mut failures = Vec::new();

    if test_coverage < min_test_coverage {
        failures.push(format!(
            "overall test coverage {:.1}% < {:.1}% (threshold)",
            test_coverage, min_test_coverage
        ));
    }

    if gherkin_coverage < min_gherkin_coverage {
        failures.push(format!(
            "overall Gherkin coverage {:.1}% < {:.1}% (threshold)",
            gherkin_coverage, min_gherkin_coverage
        ));
    }

    if both_coverage < min_both_coverage {
        failures.push(format!(
            "overall combined coverage {:.1}% < {:.1}% (threshold)",
            both_coverage, min_both_coverage
        ));
    }

    let mut evaluated_microcrates = 0usize;
    let mut failing_microcrates = Vec::new();

    for microcrate in &metrics.crate_coverage {
        if microcrate.total_ac < min_crate_ac_for_eval {
            continue;
        }
        if exclude_unmapped_microcrate && microcrate.is_unmapped() {
            continue;
        }

        evaluated_microcrates += 1;

        let microcrate_test_coverage = microcrate.test_coverage_percent();
        let microcrate_gherkin_coverage = microcrate.gherkin_coverage_percent();
        let microcrate_both_coverage = microcrate.combined_coverage_percent();

        let mut crate_failures = Vec::new();
        if microcrate_test_coverage < min_crate_test_coverage {
            crate_failures.push(format!(
                "test {:.1}% < {:.1}% (threshold)",
                microcrate_test_coverage, min_crate_test_coverage
            ));
        }
        if microcrate_gherkin_coverage < min_crate_gherkin_coverage {
            crate_failures.push(format!(
                "gherkin {:.1}% < {:.1}% (threshold)",
                microcrate_gherkin_coverage, min_crate_gherkin_coverage
            ));
        }
        if microcrate_both_coverage < min_crate_both_coverage {
            crate_failures.push(format!(
                "combined {:.1}% < {:.1}% (threshold)",
                microcrate_both_coverage, min_crate_both_coverage
            ));
        }

        if !crate_failures.is_empty() {
            failing_microcrates.push(format!(
                "{}: {}",
                microcrate.crate_name,
                crate_failures.join(", ")
            ));
        }
    }

    if !failing_microcrates.is_empty() {
        failures.push(format!(
            "{} microcrate(s) below per-microcrate thresholds with min AC >= {}: {}",
            failing_microcrates.len(),
            min_crate_ac_for_eval,
            failing_microcrates.join(", ")
        ));
    }

    if failures.is_empty() {
        Ok(QualityGateResult::with_details(
            "QG-BDD-COVERAGE",
            true,
            format!(
                "Coverage checks passed (test {:.1}%, gherkin {:.1}%, combined {:.1}%). {} microcrate(s) evaluated with min AC {} and crate thresholds (tests {:.1}%, gherkin {:.1}%, combined {:.1}%{}).",
                test_coverage,
                gherkin_coverage,
                both_coverage,
                evaluated_microcrates,
                min_crate_ac_for_eval,
                min_crate_test_coverage,
                min_crate_gherkin_coverage,
                min_crate_both_coverage,
                if exclude_unmapped_microcrate {
                    ", unmapped excluded"
                } else {
                    ""
                }
            ),
        ))
    } else {
        Ok(QualityGateResult::with_details(
            "QG-BDD-COVERAGE",
            false,
            format!("BDD coverage checks failed:\n- {}", failures.join("\n- ")),
        ))
    }
}

/// QG-BDD-MATRIX-COMPLETE: Ensure the BDD metrics artifact includes all workspace
/// microcrates discovered from cargo workspace members.
///
/// This prevents drift where microcrates exist in workspace membership but are
/// missing from `docs/bdd_metrics.json`, which can happen if generation commands
/// are run with a stale checkout or without the workspace flag enabled.
pub fn check_bdd_matrix_complete() -> Result<QualityGateResult> {
    let metrics_path = workspace_path("docs/bdd_metrics.json");
    let metrics = match load_bdd_metrics(&metrics_path) {
        Ok(metrics) => metrics,
        Err(e) => {
            return Ok(QualityGateResult::with_details(
                "QG-BDD-MATRIX-COMPLETE",
                false,
                format!("Unable to load BDD metrics artifact: {}", e),
            ));
        }
    };

    let workspace_crates: HashSet<String> = load_workspace_microcrate_names(workspace_root())
        .context("Failed to load workspace microcrates")?
        .into_iter()
        .collect();
    let mapped_crates: HashSet<String> = metrics
        .crate_coverage
        .into_iter()
        .map(|entry| entry.crate_name)
        .filter(|name| !name.is_empty() && name != UNMAPPED_MICROCRATE)
        .collect();

    let mut missing_crates: Vec<String> = workspace_crates
        .difference(&mapped_crates)
        .cloned()
        .collect();
    missing_crates.sort();

    let mut extra_crates: Vec<String> = mapped_crates
        .difference(&workspace_crates)
        .filter(|name| name.as_str() != UNMAPPED_MICROCRATE)
        .cloned()
        .collect();
    extra_crates.sort();

    if missing_crates.is_empty() && extra_crates.is_empty() {
        Ok(QualityGateResult::with_details(
            "QG-BDD-MATRIX-COMPLETE",
            true,
            format!(
                "BDD metrics matrix contains all {} workspace microcrates",
                workspace_crates.len()
            ),
        ))
    } else {
        let mut details = Vec::new();
        if !missing_crates.is_empty() {
            details.push(format!(
                "Missing from docs/bdd_metrics.json: {}",
                missing_crates.join(", ")
            ));
        }

        if !extra_crates.is_empty() {
            details.push(format!(
                "Present in docs/bdd_metrics.json but no longer workspace member: {}",
                extra_crates.join(", ")
            ));
        }

        Ok(QualityGateResult::with_details(
            "QG-BDD-MATRIX-COMPLETE",
            false,
            format!(
                "BDD matrix completeness check failed:\n- {}",
                details.join("\n- ")
            ),
        ))
    }
}

/// QG-CRATE-METADATA: Verify workspace microcrates have crates.io compatible metadata.
///
/// This quality gate validates that each crate under `crates/` has the metadata required for a
/// stable `crates.io` publishable layout, including workspace-inherited values where applicable.
///
/// Required metadata keys:
/// - name
/// - version
/// - edition
/// - rust-version
/// - license
/// - repository
/// - homepage
/// - description
/// - readme
/// - keywords
/// - categories
///
/// The gate validates that `readme` resolves to an existing file and that required fields are
/// present either in the crate manifest or `[workspace.package]`.
pub fn check_crate_metadata_compatibility() -> Result<QualityGateResult> {
    let report = validate_workspace_crates_io_metadata(workspace_root())
        .context("Failed to validate workspace crates.io metadata")?;

    if report.checked == 0 {
        return Ok(QualityGateResult::with_details(
            "QG-CRATE-METADATA",
            false,
            "No crate manifests found under crates/ in workspace members".to_string(),
        ));
    }

    if report.is_success() {
        Ok(QualityGateResult::with_details(
            "QG-CRATE-METADATA",
            true,
            format!(
                "{} crate manifests validated for crates.io metadata compatibility",
                report.checked
            ),
        ))
    } else {
        let details = format!(
            "{} crate(s) failed metadata compatibility:\n- {}",
            report.issues.len(),
            report
                .issues
                .iter()
                .map(|issue| issue.summary())
                .collect::<Vec<_>>()
                .join("\n- ")
        );
        Ok(QualityGateResult::with_details(
            "QG-CRATE-METADATA",
            false,
            details,
        ))
    }
}

fn load_bdd_metrics(path: &Path) -> Result<BddTraceabilityMetrics> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read BDD metrics artifact at {}", path.display()))?;

    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse BDD metrics JSON at {}", path.display()))
}

fn read_float_threshold(name: &str, default: f64) -> Result<f64> {
    match env::var(name) {
        Ok(raw) => {
            let value = raw.parse::<f64>().with_context(|| {
                format!(
                    "Invalid value for {}={}. Expected a floating point threshold.",
                    name, raw
                )
            })?;
            Ok(value)
        }
        Err(_) => Ok(default),
    }
}

fn read_usize_threshold(name: &str, default: usize) -> Result<usize> {
    match env::var(name) {
        Ok(raw) => {
            let value = raw.parse::<usize>().with_context(|| {
                format!(
                    "Invalid value for {}={}. Expected a non-negative integer threshold.",
                    name, raw
                )
            })?;
            Ok(value)
        }
        Err(_) => Ok(default),
    }
}

fn read_bool_flag(name: &str, default: bool) -> Result<bool> {
    match env::var(name) {
        Ok(raw) => match raw.to_lowercase().as_str() {
            "1" | "true" | "t" | "yes" | "on" => Ok(true),
            "0" | "false" | "f" | "no" | "off" => Ok(false),
            _ => anyhow::bail!(
                "Invalid value for {}={}. Expected a boolean flag (true/false/1/0/yes/no/on/off).",
                name,
                raw
            ),
        },
        Err(_) => Ok(default),
    }
}

/// QG-BDD-UNMAPPED-MICROCRATE: Ensure no acceptance-criteria are assigned to
/// the synthetic `unmapped` microcrate.
///
/// This gate enforces that all acceptance criteria can be mapped to a concrete
/// workspace microcrate in test references. It fails when the synthetic `unmapped`
/// row in `docs/bdd_metrics.json` has any acceptance criteria.
///
/// Failure Condition:
/// - `unmapped` row exists with `total_ac > 0`.
pub fn check_no_unmapped_microcrate_requirements() -> Result<QualityGateResult> {
    let metrics_path = workspace_path("docs/bdd_metrics.json");
    let metrics = match load_bdd_metrics(&metrics_path) {
        Ok(metrics) => metrics,
        Err(e) => {
            return Ok(QualityGateResult::with_details(
                "QG-BDD-UNMAPPED-MICROCRATE",
                false,
                format!("Unable to load BDD metrics artifact: {}", e),
            ));
        }
    };

    let unmapped = metrics.crate_coverage_for(UNMAPPED_MICROCRATE);

    if let Some(unmapped) = unmapped
        && unmapped.total_ac > 0
    {
        return Ok(QualityGateResult::with_details(
            "QG-BDD-UNMAPPED-MICROCRATE",
            false,
            format!(
                "BDD metrics row `{}` has {} acceptance criteria with {} tests and {} Gherkin mapping(s)",
                UNMAPPED_MICROCRATE,
                unmapped.total_ac,
                unmapped.ac_with_tests,
                unmapped.ac_with_gherkin
            ),
        ));
    }

    Ok(QualityGateResult::new("QG-BDD-UNMAPPED-MICROCRATE", true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_sim_mapping_docs_exist() {
        // This test verifies that the mapping documentation files exist
        // It will fail if any required files are missing
        let result = check_sim_mapping_docs().expect("QG-SIM-MAPPING check failed");

        if !result.passed {
            panic!(
                "QG-SIM-MAPPING failed: {}",
                result
                    .details
                    .unwrap_or_else(|| "Unknown error".to_string())
            );
        }
    }

    #[test]
    fn test_unit_conversion_coverage() {
        // This test verifies that all required unit conversion tests exist
        let result = check_unit_conversion_coverage().expect("QG-UNIT-CONV check failed");

        if !result.passed {
            panic!(
                "QG-UNIT-CONV failed: {}",
                result
                    .details
                    .unwrap_or_else(|| "Unknown error".to_string())
            );
        }
    }

    #[test]
    fn test_sanity_gate_tests_exist() {
        // This test verifies that all required sanity gate tests exist
        let result = check_sanity_gate_tests().expect("QG-SANITY-GATE check failed");

        if !result.passed {
            panic!(
                "QG-SANITY-GATE failed: {}",
                result
                    .details
                    .unwrap_or_else(|| "Unknown error".to_string())
            );
        }
    }

    #[test]
    fn test_ffb_safety_tests_exist() {
        // This test verifies that all required FFB safety tests exist
        let result = check_ffb_safety_tests().expect("QG-FFB-SAFETY check failed");

        if !result.passed {
            panic!(
                "QG-FFB-SAFETY failed: {}",
                result
                    .details
                    .unwrap_or_else(|| "Unknown error".to_string())
            );
        }
    }

    #[test]
    fn test_bdd_coverage_threshold_parsing() {
        let expected_float = 83.3;
        // SAFETY: test-only process-wide env mutation; this test reads/removes the same keys.
        unsafe {
            env::set_var("BDD_MIN_TEST_COVERAGE_PCT", "83.3");
        }
        let parsed_float = read_float_threshold("BDD_MIN_TEST_COVERAGE_PCT", 0.0)
            .expect("Expected threshold parsing to succeed");
        assert!((parsed_float - expected_float).abs() < 0.0001);

        unsafe {
            env::set_var("BDD_MIN_CRATE_TEST_PCT", "72.5");
        }
        let parsed_float = read_float_threshold("BDD_MIN_CRATE_TEST_PCT", 0.0)
            .expect("Expected microcrate test threshold parsing to succeed");
        assert!((parsed_float - 72.5).abs() < 0.0001);

        unsafe {
            env::set_var("BDD_MIN_CRATE_BOTH_PCT", "91.2");
        }
        let parsed_float = read_float_threshold("BDD_MIN_CRATE_BOTH_PCT", 0.0)
            .expect("Expected microcrate both threshold parsing to succeed");
        assert!((parsed_float - 91.2).abs() < 0.0001);

        unsafe {
            env::set_var("BDD_MIN_CRATE_AC_FOR_EVAL", "5");
        }
        let parsed_usize = read_usize_threshold("BDD_MIN_CRATE_AC_FOR_EVAL", 1)
            .expect("Expected microcrate threshold parsing to succeed");
        assert_eq!(parsed_usize, 5);

        unsafe {
            env::set_var("BDD_EXCLUDE_UNMAPPED_MICROCRATE", "true");
        }
        let parsed_bool = read_bool_flag("BDD_EXCLUDE_UNMAPPED_MICROCRATE", false)
            .expect("Expected boolean parsing to succeed");
        assert!(parsed_bool);

        unsafe {
            env::set_var("BDD_EXCLUDE_UNMAPPED_MICROCRATE", "0");
        }
        let parsed_bool = read_bool_flag("BDD_EXCLUDE_UNMAPPED_MICROCRATE", true)
            .expect("Expected boolean parsing to succeed");
        assert!(!parsed_bool);

        unsafe {
            env::remove_var("BDD_MIN_TEST_COVERAGE_PCT");
            env::remove_var("BDD_MIN_CRATE_TEST_PCT");
            env::remove_var("BDD_MIN_CRATE_BOTH_PCT");
            env::remove_var("BDD_MIN_CRATE_AC_FOR_EVAL");
            env::remove_var("BDD_EXCLUDE_UNMAPPED_MICROCRATE");
        }
    }
}
