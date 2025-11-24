// SPDX-License-Identifier: MIT OR Apache-2.0

//! Quality gate checks for Flight Hub CI pipeline.
//!
//! This module implements the quality gates defined in the sim-integration-implementation spec:
//! - QG-SIM-MAPPING: Verify simulator mapping documentation exists
//! - QG-UNIT-CONV: Verify unit conversion test coverage (future)
//! - QG-SANITY-GATE: Verify sanity gate tests (future)
//! - QG-FFB-SAFETY: Verify FFB safety tests (future)
//! - QG-RT-JITTER: Verify real-time jitter tests (future)
//! - QG-HID-LATENCY: Verify HID latency tests (future)
//! - QG-LEGAL-DOC: Verify legal documentation (future)

use anyhow::Result;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sim_mapping_docs_exist() {
        // This test verifies that the mapping documentation files exist
        // It will fail if any required files are missing
        let result = check_sim_mapping_docs().expect("QG-SIM-MAPPING check failed");
        
        if !result.passed {
            panic!(
                "QG-SIM-MAPPING failed: {}",
                result.details.unwrap_or_else(|| "Unknown error".to_string())
            );
        }
    }
}
