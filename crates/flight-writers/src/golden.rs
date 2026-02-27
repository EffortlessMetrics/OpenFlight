// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Golden file testing for writer configurations

use crate::types::*;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Manages golden file tests for writer configurations
pub struct GoldenFileTester {
    golden_dir: PathBuf,
}

impl GoldenFileTester {
    pub fn new<P: AsRef<Path>>(golden_dir: P) -> Self {
        Self {
            golden_dir: golden_dir.as_ref().to_path_buf(),
        }
    }

    /// Run golden file tests for a specific simulator
    pub async fn test_simulator(&self, sim: SimulatorType) -> Result<GoldenTestResult> {
        info!("Running golden file tests for {}", sim);

        let sim_dir = self.golden_dir.join(sim.to_string());
        if !sim_dir.exists() {
            return Ok(GoldenTestResult {
                sim,
                success: false,
                test_cases: vec![],
                coverage: CoverageMatrix {
                    versions: vec![],
                    areas: vec![],
                    coverage_percent: 0.0,
                    missing_coverage: vec!["No golden files found".to_string()],
                },
            });
        }

        let mut test_cases = Vec::new();
        let mut all_versions = Vec::new();
        let mut all_areas = Vec::new();

        // Find all test case directories
        for entry in fs::read_dir(&sim_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let test_name = entry.file_name().to_string_lossy().to_string();
                let test_case = self.run_test_case(sim, &test_name, &entry.path()).await?;

                // Extract version and area information
                if let Some(version) = self.extract_version_from_test_name(&test_name)
                    && !all_versions.contains(&version)
                {
                    all_versions.push(version);
                }

                if let Some(area) = self.extract_area_from_test_name(&test_name)
                    && !all_areas.contains(&area)
                {
                    all_areas.push(area);
                }

                test_cases.push(test_case);
            }
        }

        let success = test_cases.iter().all(|tc| tc.success);
        let coverage = self.calculate_coverage(&all_versions, &all_areas);

        info!(
            "Golden file tests for {} completed: {}/{} passed",
            sim,
            test_cases.iter().filter(|tc| tc.success).count(),
            test_cases.len()
        );

        Ok(GoldenTestResult {
            sim,
            success,
            test_cases,
            coverage,
        })
    }

    /// Run a single test case
    async fn run_test_case(
        &self,
        _sim: SimulatorType,
        test_name: &str,
        test_dir: &Path,
    ) -> Result<GoldenTestCase> {
        debug!("Running test case: {}", test_name);

        let input_file = test_dir.join("input.json");
        let expected_file = test_dir.join("expected");
        let actual_file = test_dir.join("actual");

        // Clean up any previous actual output
        if actual_file.exists() {
            fs::remove_dir_all(&actual_file)?;
        }
        fs::create_dir_all(&actual_file)?;

        let success = match self
            .execute_test_case(&input_file, &expected_file, &actual_file)
            .await
        {
            Ok(()) => {
                // Compare expected vs actual
                self.compare_directories(&expected_file, &actual_file)
                    .await?
            }
            Err(e) => {
                warn!("Test case {} failed: {}", test_name, e);
                false
            }
        };

        let diff = if !success {
            Some(self.generate_diff(&expected_file, &actual_file).await?)
        } else {
            None
        };

        Ok(GoldenTestCase {
            name: test_name.to_string(),
            success,
            expected_file,
            actual_file,
            diff,
        })
    }

    /// Execute a single test case
    async fn execute_test_case(
        &self,
        input_file: &Path,
        _expected_file: &Path,
        actual_file: &Path,
    ) -> Result<()> {
        // Read the writer configuration
        let config_content =
            fs::read_to_string(input_file).context("Failed to read input configuration")?;

        let config: WriterConfig = serde_json::from_str(&config_content)
            .context("Failed to parse writer configuration")?;

        // Create a temporary applier that writes to the actual output directory
        let applier = TestApplier::new(actual_file);

        // Apply the configuration
        applier
            .apply_test_config(&config)
            .await
            .context("Failed to apply test configuration")?;

        Ok(())
    }

    /// Compare two directories recursively
    async fn compare_directories(&self, expected: &Path, actual: &Path) -> Result<bool> {
        if !expected.exists() || !actual.exists() {
            return Ok(false);
        }

        let expected_files = self.collect_files_recursive(expected)?;
        let actual_files = self.collect_files_recursive(actual)?;

        // Check if file sets match
        if expected_files.len() != actual_files.len() {
            return Ok(false);
        }

        // Compare each file
        for (rel_path, expected_path) in &expected_files {
            if let Some(actual_path) = actual_files.get(rel_path) {
                if !self.compare_files(expected_path, actual_path).await? {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Collect all files in a directory recursively
    fn collect_files_recursive(&self, dir: &Path) -> Result<HashMap<PathBuf, PathBuf>> {
        let mut files = HashMap::new();
        Self::collect_files_recursive_impl(dir, dir, &mut files)?;
        Ok(files)
    }

    fn collect_files_recursive_impl(
        base_dir: &Path,
        current_dir: &Path,
        files: &mut HashMap<PathBuf, PathBuf>,
    ) -> Result<()> {
        for entry in fs::read_dir(current_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let rel_path = path.strip_prefix(base_dir)?;
                files.insert(rel_path.to_path_buf(), path);
            } else if path.is_dir() {
                Self::collect_files_recursive_impl(base_dir, &path, files)?;
            }
        }
        Ok(())
    }

    /// Compare two files for equality
    async fn compare_files(&self, expected: &Path, actual: &Path) -> Result<bool> {
        let expected_content =
            fs::read_to_string(expected).context("Failed to read expected file")?;
        let actual_content = fs::read_to_string(actual).context("Failed to read actual file")?;

        // Normalize line endings for comparison
        let expected_normalized = expected_content.replace("\r\n", "\n");
        let actual_normalized = actual_content.replace("\r\n", "\n");

        Ok(expected_normalized == actual_normalized)
    }

    /// Generate a diff between expected and actual directories
    async fn generate_diff(&self, expected: &Path, actual: &Path) -> Result<String> {
        let mut diff_lines = Vec::new();

        let expected_files = self.collect_files_recursive(expected)?;
        let actual_files = self.collect_files_recursive(actual)?;

        // Files only in expected
        for rel_path in expected_files.keys() {
            if !actual_files.contains_key(rel_path) {
                diff_lines.push(format!("- Missing file: {}", rel_path.display()));
            }
        }

        // Files only in actual
        for rel_path in actual_files.keys() {
            if !expected_files.contains_key(rel_path) {
                diff_lines.push(format!("+ Extra file: {}", rel_path.display()));
            }
        }

        // Files with different content
        for (rel_path, expected_path) in &expected_files {
            if let Some(actual_path) = actual_files.get(rel_path)
                && !self.compare_files(expected_path, actual_path).await?
            {
                diff_lines.push(format!("~ Modified file: {}", rel_path.display()));

                // Add a simple content diff
                let expected_content = fs::read_to_string(expected_path)?;
                let actual_content = fs::read_to_string(actual_path)?;

                diff_lines.push("  Expected:".to_string());
                for line in expected_content.lines().take(5) {
                    diff_lines.push(format!("    {}", line));
                }

                diff_lines.push("  Actual:".to_string());
                for line in actual_content.lines().take(5) {
                    diff_lines.push(format!("    {}", line));
                }
            }
        }

        Ok(diff_lines.join("\n"))
    }

    /// Extract version information from test name
    fn extract_version_from_test_name(&self, test_name: &str) -> Option<String> {
        // Look for version patterns like "v1.36.0" or "1.36.0"
        if let Some(start) = test_name.find("v") {
            let version_part = &test_name[start + 1..];
            if let Some(end) = version_part.find('_') {
                Some(version_part[..end].to_string())
            } else {
                Some(version_part.to_string())
            }
        } else {
            // Look for numeric version patterns
            let parts: Vec<&str> = test_name.split('_').collect();
            for part in parts {
                if part.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    return Some(part.to_string());
                }
            }
            None
        }
    }

    /// Extract area information from test name
    fn extract_area_from_test_name(&self, test_name: &str) -> Option<String> {
        // Common configuration areas
        let areas = [
            "autopilot",
            "electrical",
            "fuel",
            "hydraulics",
            "engine",
            "avionics",
        ];

        for area in &areas {
            if test_name.to_lowercase().contains(area) {
                return Some(area.to_string());
            }
        }

        None
    }

    /// Calculate coverage matrix
    fn calculate_coverage(&self, versions: &[String], areas: &[String]) -> CoverageMatrix {
        // This is a simplified coverage calculation
        // In a real implementation, you'd want to define what constitutes full coverage
        let total_expected_versions = 5; // Example: support last 5 versions
        let total_expected_areas = 10; // Example: 10 configuration areas

        let version_coverage = versions.len() as f64 / total_expected_versions as f64;
        let area_coverage = areas.len() as f64 / total_expected_areas as f64;
        let overall_coverage = (version_coverage + area_coverage) / 2.0 * 100.0;

        let mut missing_coverage = Vec::new();
        if versions.len() < total_expected_versions {
            missing_coverage.push(format!(
                "Version coverage: {}/{} versions",
                versions.len(),
                total_expected_versions
            ));
        }
        if areas.len() < total_expected_areas {
            missing_coverage.push(format!(
                "Area coverage: {}/{} areas",
                areas.len(),
                total_expected_areas
            ));
        }

        CoverageMatrix {
            versions: versions.to_vec(),
            areas: areas.to_vec(),
            coverage_percent: overall_coverage.min(100.0),
            missing_coverage,
        }
    }
}

/// Test applier that writes to a test output directory instead of real simulator files
struct TestApplier {
    output_dir: PathBuf,
}

impl TestApplier {
    fn new<P: AsRef<Path>>(output_dir: P) -> Self {
        Self {
            output_dir: output_dir.as_ref().to_path_buf(),
        }
    }

    async fn apply_test_config(&self, config: &WriterConfig) -> Result<()> {
        // Create a mock file structure and apply diffs to it
        for diff in &config.diffs {
            let output_file = self.output_dir.join(&diff.file);

            // Ensure parent directory exists
            if let Some(parent) = output_file.parent() {
                fs::create_dir_all(parent)?;
            }

            // Apply the diff operation to create the expected output
            match &diff.operation {
                DiffOperation::Replace { content } => {
                    fs::write(&output_file, content)?;
                }
                DiffOperation::IniSection { section, changes } => {
                    let mut content = String::new();
                    content.push_str(&format!("[{}]\n", section));
                    // Sort keys for deterministic output (HashMap order is random).
                    let mut sorted: Vec<(&String, &String)> = changes.iter().collect();
                    sorted.sort_by_key(|(k, _)| k.as_str());
                    for (key, value) in sorted {
                        content.push_str(&format!("{}={}\n", key, value));
                    }
                    fs::write(&output_file, content)?;
                }
                DiffOperation::JsonPatch { patches: _ } => {
                    // For testing, create a simple JSON file
                    fs::write(&output_file, "{\"test\": \"value\"}")?;
                }
                DiffOperation::LineReplace { replacement, .. } => {
                    fs::write(&output_file, replacement)?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_golden_file_testing() {
        let temp_dir = TempDir::new().unwrap();
        let golden_dir = temp_dir.path().join("golden");
        let test_dir = golden_dir.join("msfs").join("test_case_v1.36.0_autopilot");

        fs::create_dir_all(&test_dir).unwrap();

        // Create input configuration
        let input_config = WriterConfig {
            schema: "flight.writer/1".to_string(),
            sim: SimulatorType::MSFS,
            version: "1.36.0".to_string(),
            description: Some("Test configuration".to_string()),
            diffs: vec![FileDiff {
                file: PathBuf::from("test.ini"),
                operation: DiffOperation::IniSection {
                    section: "AUTOPILOT".to_string(),
                    changes: {
                        let mut changes = HashMap::new();
                        changes.insert("enabled".to_string(), "1".to_string());
                        changes
                    },
                },
                backup: true,
            }],
            verify_scripts: vec![],
        };

        let input_file = test_dir.join("input.json");
        fs::write(
            &input_file,
            serde_json::to_string_pretty(&input_config).unwrap(),
        )
        .unwrap();

        // Create expected output
        let expected_dir = test_dir.join("expected");
        fs::create_dir_all(&expected_dir).unwrap();
        fs::write(expected_dir.join("test.ini"), "[AUTOPILOT]\nenabled=1\n").unwrap();

        let tester = GoldenFileTester::new(&golden_dir);
        let result = tester.test_simulator(SimulatorType::MSFS).await.unwrap();

        assert!(result.success);
        assert_eq!(result.test_cases.len(), 1);
        assert!(result.test_cases[0].success);
    }
}
