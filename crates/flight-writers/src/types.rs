// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Core types for the Writers system

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Supported simulator types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SimulatorType {
    #[serde(rename = "msfs")]
    MSFS,
    #[serde(rename = "xplane")]
    XPlane,
    #[serde(rename = "dcs")]
    DCS,
}

impl std::fmt::Display for SimulatorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimulatorType::MSFS => write!(f, "msfs"),
            SimulatorType::XPlane => write!(f, "xplane"),
            SimulatorType::DCS => write!(f, "dcs"),
        }
    }
}

/// A complete writer configuration for a specific simulator version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriterConfig {
    /// Schema version for compatibility checking
    pub schema: String,
    /// Target simulator
    pub sim: SimulatorType,
    /// Simulator version this config applies to
    pub version: String,
    /// Human-readable description
    pub description: Option<String>,
    /// List of file modifications to apply
    pub diffs: Vec<FileDiff>,
    /// Verification scripts to run after applying
    pub verify_scripts: Vec<VerifyScript>,
}

/// A single file modification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    /// Target file path (relative to simulator installation)
    pub file: PathBuf,
    /// Type of modification
    #[serde(flatten)]
    pub operation: DiffOperation,
    /// Backup this file before modification
    #[serde(default = "default_backup")]
    pub backup: bool,
}

fn default_backup() -> bool {
    true
}

/// Types of file modifications supported
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DiffOperation {
    /// Replace entire file content
    #[serde(rename = "replace")]
    Replace { content: String },
    /// Modify specific section in INI-style file
    #[serde(rename = "ini_section")]
    IniSection {
        section: String,
        changes: HashMap<String, String>,
    },
    /// JSON patch operation
    #[serde(rename = "json_patch")]
    JsonPatch { patches: Vec<JsonPatchOp> },
    /// Line-based replacement
    #[serde(rename = "line_replace")]
    LineReplace {
        pattern: String,
        replacement: String,
        /// If true, use regex matching
        #[serde(default)]
        regex: bool,
    },
}

/// JSON Patch operation (RFC 6902)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonPatchOp {
    pub op: JsonPatchOpType,
    pub path: String,
    pub value: Option<serde_json::Value>,
    pub from: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JsonPatchOpType {
    Add,
    Remove,
    Replace,
    Move,
    Copy,
    Test,
}

/// Script to verify configuration is working correctly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyScript {
    /// Script name/identifier
    pub name: String,
    /// Description of what this script tests
    pub description: String,
    /// List of actions to perform
    pub actions: Vec<VerifyAction>,
    /// Expected results after actions
    pub expected: Vec<ExpectedResult>,
}

/// A single verification action
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VerifyAction {
    /// Send a simulator event
    #[serde(rename = "sim_event")]
    SimEvent { event: String, value: Option<f64> },
    /// Wait for a specified duration
    #[serde(rename = "wait")]
    Wait { duration_ms: u64 },
    /// Check a simulator variable
    #[serde(rename = "check_var")]
    CheckVar {
        variable: String,
        expected: f64,
        tolerance: Option<f64>,
    },
}

/// Expected result from verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedResult {
    /// Variable to check
    pub variable: String,
    /// Expected value
    pub value: f64,
    /// Tolerance for floating point comparison
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,
}

fn default_tolerance() -> f64 {
    0.001
}

/// Result of applying a writer configuration
#[derive(Debug, Clone)]
pub struct ApplyResult {
    /// Whether the application was successful
    pub success: bool,
    /// Files that were modified
    pub modified_files: Vec<PathBuf>,
    /// Backup identifier for rollback
    pub backup_id: String,
    /// Any errors that occurred
    pub errors: Vec<String>,
}

/// Result of verifying simulator configuration
#[derive(Debug, Clone)]
pub struct VerifyResult {
    /// Simulator that was verified
    pub sim: SimulatorType,
    /// Version that was checked
    pub version: String,
    /// Overall verification status
    pub success: bool,
    /// Results of individual script runs
    pub script_results: Vec<ScriptResult>,
    /// Files that don't match expected state
    pub mismatched_files: Vec<FileMismatch>,
}

/// Result of running a single verification script
#[derive(Debug, Clone)]
pub struct ScriptResult {
    /// Script name
    pub name: String,
    /// Whether the script passed
    pub success: bool,
    /// Detailed results of each action
    pub action_results: Vec<ActionResult>,
    /// Any errors that occurred
    pub errors: Vec<String>,
}

/// Result of a single verification action
#[derive(Debug, Clone)]
pub struct ActionResult {
    /// Action that was performed
    pub action: String,
    /// Whether it succeeded
    pub success: bool,
    /// Actual value (for variable checks)
    pub actual_value: Option<f64>,
    /// Expected value (for variable checks)
    pub expected_value: Option<f64>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Information about a file that doesn't match expected state
#[derive(Debug, Clone)]
pub struct FileMismatch {
    /// Path to the mismatched file
    pub file: PathBuf,
    /// Type of mismatch
    pub mismatch_type: MismatchType,
    /// Suggested fix
    pub suggested_diff: Option<FileDiff>,
}

/// Types of file mismatches
#[derive(Debug, Clone)]
pub enum MismatchType {
    /// File is missing
    Missing,
    /// File content doesn't match
    ContentMismatch,
    /// File permissions are wrong
    PermissionMismatch,
}

/// Result of repairing simulator configuration
#[derive(Debug, Clone)]
pub struct RepairResult {
    /// Whether the repair was successful
    pub success: bool,
    /// Files that were repaired
    pub repaired_files: Vec<PathBuf>,
    /// Backup identifier for rollback
    pub backup_id: String,
    /// Any errors that occurred
    pub errors: Vec<String>,
}

/// Result of rolling back configuration
#[derive(Debug, Clone)]
pub struct RollbackResult {
    /// Whether the rollback was successful
    pub success: bool,
    /// Files that were restored
    pub restored_files: Vec<PathBuf>,
    /// Any errors that occurred
    pub errors: Vec<String>,
}

/// Result of golden file testing
#[derive(Debug, Clone)]
pub struct GoldenTestResult {
    /// Simulator that was tested
    pub sim: SimulatorType,
    /// Overall test status
    pub success: bool,
    /// Results of individual test cases
    pub test_cases: Vec<GoldenTestCase>,
    /// Coverage matrix information
    pub coverage: CoverageMatrix,
}

/// Result of a single golden test case
#[derive(Debug, Clone)]
pub struct GoldenTestCase {
    /// Test case name
    pub name: String,
    /// Whether the test passed
    pub success: bool,
    /// Expected output file
    pub expected_file: PathBuf,
    /// Actual output file
    pub actual_file: PathBuf,
    /// Diff if test failed
    pub diff: Option<String>,
}

/// Coverage matrix showing what's tested
#[derive(Debug, Clone)]
pub struct CoverageMatrix {
    /// Simulator versions covered
    pub versions: Vec<String>,
    /// Configuration areas covered
    pub areas: Vec<String>,
    /// Percentage of coverage
    pub coverage_percent: f64,
    /// Missing coverage areas
    pub missing_coverage: Vec<String>,
}
