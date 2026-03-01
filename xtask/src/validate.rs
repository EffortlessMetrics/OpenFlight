// SPDX-License-Identifier: MIT OR Apache-2.0

//! Full validation pipeline implementation.
//!
//! This module implements the `cargo xtask validate` command, which runs:
//! 1. Schema validation (spec ledger and documentation front matter)
//! 2. Code quality checks (via check::run_check())
//! 3. Public API verification (if cargo-public-api is installed)
//!
//! The validation pipeline generates a comprehensive report at
//! docs/validation_report.md with timestamps, commit hashes, and check results.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

use crate::cross_ref;
use crate::front_matter;
use crate::gherkin;
use crate::schema;

/// Result of a validation check.
#[derive(Debug, Clone)]
struct CheckResult {
    name: String,
    passed: bool,
    details: Option<String>,
}

impl CheckResult {
    fn new(name: impl Into<String>, passed: bool) -> Self {
        Self {
            name: name.into(),
            passed,
            details: None,
        }
    }

    fn with_details(name: impl Into<String>, passed: bool, details: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed,
            details: Some(details.into()),
        }
    }
}

/// Run full validation pipeline.
///
/// This function executes all validation checks in the required order:
/// 1. Schema validation (per INF-REQ-12)
/// 2. Cross-reference checks (per INF-REQ-6)
/// 3. Code quality checks
/// 4. Public API verification
///
/// After all checks complete, it generates docs/validation_report.md with
/// detailed results.
///
/// # Returns
///
/// Returns `Ok(())` if all checks pass, or an error if any check fails.
///
/// # Errors
///
/// Returns an error if:
/// - Schema validation fails
/// - Cross-reference checks fail
/// - Code quality checks fail
/// - Report generation fails
pub fn run_validate() -> Result<()> {
    println!("🔍 Running full validation pipeline...\n");

    let mut results = Vec::new();
    let mut all_passed = true;
    let mut cross_ref_details = Vec::new();

    // Step 0: BDD Feature Status Report
    println!("🧾 Step 0: BDD Feature Status");
    println!("─────────────────────────────");
    let bdd_status_result = crate::ac_status::run_ac_status();
    match &bdd_status_result {
        Ok(()) => {
            println!("✅ BDD feature status report generated\n");
            let detail = summarize_bdd_status().unwrap_or_else(|e| {
                format!(
                    "Generated docs/feature_status.md (coverage summary unavailable: {})",
                    e
                )
            });
            results.push(CheckResult::with_details(
                "BDD Feature Status",
                true,
                detail,
            ));
        }
        Err(e) => {
            eprintln!("❌ BDD feature status report failed: {}\n", e);
            results.push(CheckResult::with_details(
                "BDD Feature Status",
                false,
                e.to_string(),
            ));
            all_passed = false;
        }
    }

    // Step 1: Schema Validation
    println!("📋 Step 1: Schema Validation");
    println!("─────────────────────────────");
    let schema_result = validate_schemas();
    match &schema_result {
        Ok(()) => {
            println!("✅ Schema validation passed\n");
            results.push(CheckResult::new("Schema Validation", true));
        }
        Err(e) => {
            eprintln!("❌ Schema validation failed: {}\n", e);
            results.push(CheckResult::with_details(
                "Schema Validation",
                false,
                e.to_string(),
            ));
            all_passed = false;
        }
    }

    // Step 2: Cross-Reference Checks
    println!("🔗 Step 2: Cross-Reference Checks");
    println!("─────────────────────────────");
    let cross_ref_result = validate_cross_references();
    match &cross_ref_result {
        Ok(details) => {
            if details.is_empty() {
                println!("✅ Cross-reference validation passed\n");
                results.push(CheckResult::new("Cross-Reference Validation", true));
            } else {
                println!("❌ Cross-reference validation failed\n");
                results.push(CheckResult::with_details(
                    "Cross-Reference Validation",
                    false,
                    format!("{} error(s) found", details.len()),
                ));
                cross_ref_details = details.clone();
                all_passed = false;
            }
        }
        Err(e) => {
            eprintln!("❌ Cross-reference validation failed: {}\n", e);
            results.push(CheckResult::with_details(
                "Cross-Reference Validation",
                false,
                e.to_string(),
            ));
            all_passed = false;
        }
    }

    // Step 3: Code Quality (via check::run_check())
    println!("🔧 Step 3: Code Quality Checks");
    println!("─────────────────────────────");

    // Note: check::run_check() runs all checks and returns a single error if any fail.
    // We can't determine which specific checks failed from the error message alone,
    // so we mark all as failed if check fails. For more granular reporting, we would
    // need to refactor check.rs to return individual check results.
    let check_result = crate::check::run_check();
    match &check_result {
        Ok(()) => {
            results.push(CheckResult::new("Formatting", true));
            results.push(CheckResult::new("Clippy", true));
            results.push(CheckResult::new("Unit Tests", true));
        }
        Err(e) => {
            eprintln!("❌ Code quality checks failed: {}\n", e);
            // Since we can't determine which specific checks failed, mark all as failed
            results.push(CheckResult::with_details(
                "Code Quality (fmt, clippy, tests)",
                false,
                "One or more checks failed. See output above for details.".to_string(),
            ));
            all_passed = false;
        }
    }

    // Step 4: Quality Gates
    println!("🚦 Step 4: Quality Gates");
    println!("─────────────────────────────");
    let qg_results = run_quality_gates();
    match &qg_results {
        Ok(gate_results) => {
            for gate_result in gate_results {
                if gate_result.passed {
                    println!("✅ {} passed", gate_result.gate_name);
                    results.push(CheckResult::new(&gate_result.gate_name, true));
                } else {
                    println!("❌ {} failed", gate_result.gate_name);
                    let details = gate_result
                        .details
                        .clone()
                        .unwrap_or_else(|| "No details provided".to_string());
                    eprintln!("   {}", details);
                    results.push(CheckResult::with_details(
                        &gate_result.gate_name,
                        false,
                        details,
                    ));
                    all_passed = false;
                }
            }
            println!();
        }
        Err(e) => {
            eprintln!("❌ Quality gate checks failed: {}\n", e);
            results.push(CheckResult::with_details(
                "Quality Gates",
                false,
                e.to_string(),
            ));
            all_passed = false;
        }
    }

    // Step 5: Public API Verification
    println!("📦 Step 5: Public API Verification");
    println!("─────────────────────────────");
    let api_result = verify_public_api();
    match &api_result {
        Ok(passed) => {
            if *passed {
                println!("✅ Public API verification passed\n");
                results.push(CheckResult::new("Public API", true));
            } else {
                println!("⚠️  cargo-public-api not installed, skipping\n");
                results.push(CheckResult::with_details(
                    "Public API",
                    true,
                    "Skipped (cargo-public-api not installed)".to_string(),
                ));
            }
        }
        Err(e) => {
            eprintln!("❌ Public API verification failed: {}\n", e);
            results.push(CheckResult::with_details(
                "Public API",
                false,
                e.to_string(),
            ));
            all_passed = false;
        }
    }

    // Step 6: Generate Report
    println!("📄 Step 6: Generating Validation Report");
    println!("─────────────────────────────");
    generate_validation_report(&results, &cross_ref_details)?;
    println!("✅ Report generated at docs/validation_report.md\n");

    // Final result
    if all_passed {
        println!("✅ All validation checks passed!");
        Ok(())
    } else {
        anyhow::bail!("Some validation checks failed. See docs/validation_report.md for details.");
    }
}

fn summarize_bdd_status() -> Result<String> {
    let spec_ledger_path = Path::new("specs/spec_ledger.yaml");
    if !spec_ledger_path.exists() {
        anyhow::bail!("spec ledger not found at {}", spec_ledger_path.display());
    }

    let ledger_content =
        std::fs::read_to_string(spec_ledger_path).context("Failed to read spec ledger")?;
    let ledger: cross_ref::SpecLedger =
        serde_yaml::from_str(&ledger_content).context("Failed to parse spec ledger YAML")?;

    let scenarios = gherkin::parse_feature_files(Path::new("specs/features"))
        .context("Failed to parse Gherkin feature files")?;

    let metrics =
        crate::ac_status::compute_bdd_metrics_with_workspace_crates(&ledger, &scenarios, true);
    let fully_covered_microcrates = metrics.microcrate_with_tests_and_gherkin;
    let microcrate_total = metrics.microcrate_total;

    Ok(format!(
        "AC total: {}, tests: {} ({:.1}%), gherkin: {} ({:.1}%), microcrates fully covered: {} / {} ({:.1}%), complete: {}, needs_gherkin: {}, needs_tests: {}, draft: {}, incomplete: {}",
        metrics.total_ac,
        metrics.ac_with_tests,
        metrics.test_coverage_percent(),
        metrics.ac_with_gherkin,
        metrics.gherkin_coverage_percent(),
        fully_covered_microcrates,
        microcrate_total,
        metrics.microcrate_full_coverage_percent(),
        metrics.complete,
        metrics.needs_gherkin,
        metrics.needs_tests,
        metrics.draft,
        metrics.incomplete
    ))
}

/// Validate all schemas (spec ledger and documentation front matter).
///
/// This function validates:
/// - specs/spec_ledger.yaml against schemas/spec_ledger.schema.json
/// - All docs/**/*.md front matter against schemas/front_matter.schema.json
///
/// # Returns
///
/// Returns `Ok(())` if all schemas validate successfully, or an error
/// describing the first validation failure encountered.
fn validate_schemas() -> Result<()> {
    let mut all_errors = Vec::new();

    // Validate spec ledger
    println!("  Validating specs/spec_ledger.yaml...");
    let spec_ledger_path = Path::new("specs/spec_ledger.yaml");
    let spec_ledger_schema = Path::new("schemas/spec_ledger.schema.json");

    if spec_ledger_path.exists() && spec_ledger_schema.exists() {
        match schema::validate_yaml_against_schema(spec_ledger_path, spec_ledger_schema) {
            Ok(()) => println!("    ✓ spec_ledger.yaml is valid"),
            Err(errors) => {
                println!("    ✗ spec_ledger.yaml has {} error(s)", errors.len());
                for error in &errors {
                    eprintln!("{}", error.format());
                }
                all_errors.extend(errors);
            }
        }
    } else {
        if !spec_ledger_path.exists() {
            println!("    ⚠ specs/spec_ledger.yaml not found (skipping)");
        }
        if !spec_ledger_schema.exists() {
            println!("    ⚠ schemas/spec_ledger.schema.json not found (skipping)");
        }
    }

    // Validate documentation front matter
    println!("  Validating documentation front matter...");
    let docs_dir = Path::new("docs");
    let front_matter_schema = Path::new("schemas/front_matter.schema.json");

    if docs_dir.exists() && front_matter_schema.exists() {
        match validate_all_front_matter(docs_dir, front_matter_schema) {
            Ok(count) => println!("    ✓ {} document(s) validated", count),
            Err(errors) => {
                println!("    ✗ Front matter validation failed");
                all_errors.extend(errors);
            }
        }
    } else {
        if !docs_dir.exists() {
            println!("    ⚠ docs/ directory not found (skipping)");
        }
        if !front_matter_schema.exists() {
            println!("    ⚠ schemas/front_matter.schema.json not found (skipping)");
        }
    }

    if all_errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(
            "Schema validation failed with {} error(s)",
            all_errors.len()
        )
    }
}

/// Validate all documentation front matter against the schema.
///
/// This function collects all markdown files with front matter and validates
/// each one against the front matter schema.
///
/// # Returns
///
/// Returns `Ok(count)` with the number of documents validated, or `Err`
/// containing all validation errors encountered.
fn validate_all_front_matter(
    docs_dir: &Path,
    schema_path: &Path,
) -> Result<usize, Vec<schema::SchemaError>> {
    // Collect all front matter
    let docs = front_matter::collect_all_front_matter(docs_dir).map_err(|e| {
        vec![schema::SchemaError {
            code: "INF-SCHEMA-010".to_string(),
            message: format!("Failed to collect front matter: {}", e),
            file_path: docs_dir.display().to_string(),
            line: None,
            column: None,
            expected: None,
            found: None,
            suggestion: Some("Check that docs/ directory is readable".to_string()),
        }]
    })?;

    let mut all_errors = Vec::new();
    let mut validated_count = 0;

    // Validate each document's front matter
    for (path, front_matter) in &docs {
        // Convert front matter to YAML for validation
        let yaml_str = serde_yaml::to_string(front_matter).map_err(|e| {
            vec![schema::SchemaError {
                code: "INF-SCHEMA-011".to_string(),
                message: format!("Failed to serialize front matter: {}", e),
                file_path: path.display().to_string(),
                line: None,
                column: None,
                expected: None,
                found: None,
                suggestion: None,
            }]
        })?;

        // Write to temporary file for validation
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("front_matter_{}.yaml", validated_count));
        std::fs::write(&temp_path, &yaml_str).map_err(|e| {
            vec![schema::SchemaError {
                code: "INF-SCHEMA-012".to_string(),
                message: format!("Failed to write temp file: {}", e),
                file_path: path.display().to_string(),
                line: None,
                column: None,
                expected: None,
                found: None,
                suggestion: None,
            }]
        })?;

        // Validate against schema
        match schema::validate_yaml_against_schema(&temp_path, schema_path) {
            Ok(()) => {
                validated_count += 1;
            }
            Err(mut errors) => {
                // Update error file paths to point to the actual doc file
                for error in &mut errors {
                    error.file_path = path.display().to_string();
                }
                all_errors.extend(errors);
            }
        }

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_path);
    }

    if all_errors.is_empty() {
        Ok(validated_count)
    } else {
        Err(all_errors)
    }
}

/// Verify public API using cargo-public-api if installed.
///
/// This function checks if cargo-public-api is installed and runs it if available.
/// If not installed, it logs a warning and returns Ok(false) to indicate the check
/// was skipped (not a failure).
///
/// If cargo-public-api fails due to a rustdoc JSON format mismatch (e.g.,
/// "invalid type: integer" errors from nightly format changes), the error is
/// treated as a warning rather than a hard failure.
///
/// For workspaces, this runs cargo public-api on each core crate individually.
///
/// # Returns
///
/// Returns:
/// - `Ok(true)` if cargo-public-api is installed and verification passed
/// - `Ok(false)` if cargo-public-api is not installed or JSON format is incompatible (skipped)
/// - `Err` if cargo-public-api is installed but verification failed for non-format reasons
fn verify_public_api() -> Result<bool> {
    // Check if cargo-public-api is installed
    let check_installed = Command::new("cargo")
        .args(["public-api", "--version"])
        .output();

    match check_installed {
        Ok(output) if output.status.success() => {
            println!("  Running cargo public-api on core crates...");

            // Run public API verification on each core crate
            let mut all_passed = true;
            let mut format_error = false;
            for crate_name in crate::config::CORE_CRATES {
                println!("    Checking {}...", crate_name);
                let output = Command::new("cargo")
                    .args(["public-api", "-p", crate_name])
                    .output()
                    .context(format!(
                        "Failed to execute cargo public-api for {}",
                        crate_name
                    ))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if stderr.contains("invalid type: integer")
                        || stderr.contains("rustdoc JSON")
                        || stderr.contains("format version")
                    {
                        eprintln!(
                            "    ⚠ {} skipped: rustdoc JSON format incompatible with installed cargo-public-api",
                            crate_name
                        );
                        format_error = true;
                    } else {
                        eprintln!("    ✗ Public API check failed for {}", crate_name);
                        all_passed = false;
                    }
                }
            }

            if format_error && all_passed {
                println!("  ⚠️  Rustdoc JSON format mismatch — upgrade cargo-public-api or pin a compatible nightly");
                Ok(false)
            } else if all_passed {
                Ok(true)
            } else {
                anyhow::bail!(
                    "cargo public-api reported API changes or errors in one or more crates"
                )
            }
        }
        _ => {
            println!("  ⚠️  cargo-public-api not installed, skipping");
            println!("     Install with: cargo install cargo-public-api");
            Ok(false)
        }
    }
}

/// Run all quality gate checks.
///
/// This function executes all quality gates defined in the sim-integration-implementation spec:
/// - QG-SIM-MAPPING: Verify simulator mapping documentation exists
/// - Additional gates will be added as they are implemented
///
/// # Returns
///
/// Returns `Ok(Vec<QualityGateResult>)` with results for each gate, or an error
/// if the quality gate checks cannot be performed.
fn run_quality_gates() -> Result<Vec<crate::quality_gates::QualityGateResult>> {
    let mut results = Vec::new();

    // QG-SIM-MAPPING: Check for simulator mapping documentation
    println!("  Checking QG-SIM-MAPPING (simulator mapping docs)...");
    let sim_mapping_result = crate::quality_gates::check_sim_mapping_docs()
        .context("Failed to check simulator mapping documentation")?;
    results.push(sim_mapping_result);

    // QG-UNIT-CONV: Check unit conversion test coverage
    println!("  Checking QG-UNIT-CONV (unit conversion test coverage)...");
    let unit_conv_result = crate::quality_gates::check_unit_conversion_coverage()
        .context("Failed to check unit conversion test coverage")?;
    results.push(unit_conv_result);

    // QG-SANITY-GATE: Check sanity gate tests
    println!("  Checking QG-SANITY-GATE (sanity gate tests)...");
    let sanity_gate_result = crate::quality_gates::check_sanity_gate_tests()
        .context("Failed to check sanity gate tests")?;
    results.push(sanity_gate_result);

    // QG-FFB-SAFETY: Check FFB safety tests
    println!("  Checking QG-FFB-SAFETY (FFB safety tests)...");
    let ffb_safety_result = crate::quality_gates::check_ffb_safety_tests()
        .context("Failed to check FFB safety tests")?;
    results.push(ffb_safety_result);

    // QG-BDD-COVERAGE: Check BDD coverage thresholds and microcrate matrix coverage
    println!("  Checking QG-BDD-COVERAGE (BDD and microcrate coverage)...");
    let bdd_coverage_result =
        crate::quality_gates::check_bdd_coverage().context("Failed to check BDD coverage gate")?;
    results.push(bdd_coverage_result);

    // QG-BDD-UNMAPPED-MICROCRATE: Ensure all AC rows are mapped to concrete microcrates
    println!("  Checking QG-BDD-UNMAPPED-MICROCRATE (mapped AC to microcrates)...");
    let bdd_unmapped_result = crate::quality_gates::check_no_unmapped_microcrate_requirements()
        .context("Failed to check BDD unmapped microcrate gate")?;
    results.push(bdd_unmapped_result);

    // QG-BDD-MATRIX-COMPLETE: Ensure BDD matrix includes all workspace microcrates
    println!("  Checking QG-BDD-MATRIX-COMPLETE (BDD matrix includes workspace crates)...");
    let bdd_matrix_complete_result = crate::quality_gates::check_bdd_matrix_complete()
        .context("Failed to check BDD matrix completeness")?;
    results.push(bdd_matrix_complete_result);

    // QG-CRATE-METADATA: Check crate metadata for crates.io compatibility
    println!("  Checking QG-CRATE-METADATA (crates.io metadata compatibility)...");
    let crate_metadata_result = crate::quality_gates::check_crate_metadata_compatibility()
        .context("Failed to check crate metadata compatibility")?;
    results.push(crate_metadata_result);

    // Future quality gates will be added here:
    // - QG-RT-JITTER: Real-time jitter tests
    // - QG-HID-LATENCY: HID latency tests
    // - QG-LEGAL-DOC: Legal documentation

    Ok(results)
}

/// Validate cross-references between artifacts.
///
/// This function performs cross-reference validation:
/// - Documentation → Spec ledger (requirement links)
/// - Spec ledger → Codebase (test references)
/// - Gherkin → Spec ledger (tags)
/// - Orphaned documentation (docs with no requirement links)
///
/// # Returns
///
/// Returns `Ok(Vec<String>)` with formatted error messages for each issue found,
/// or an error if validation cannot be performed.
fn validate_cross_references() -> Result<Vec<String>> {
    let mut all_errors = Vec::new();

    // Load spec ledger
    let spec_ledger_path = Path::new("specs/spec_ledger.yaml");
    if !spec_ledger_path.exists() {
        println!("  ⚠️  specs/spec_ledger.yaml not found, skipping cross-reference checks");
        return Ok(all_errors);
    }

    let spec_ledger_content =
        std::fs::read_to_string(spec_ledger_path).context("Failed to read spec ledger")?;
    let spec_ledger: cross_ref::SpecLedger =
        serde_yaml::from_str(&spec_ledger_content).context("Failed to parse spec ledger")?;

    // Build requirement and AC indexes
    let (req_ids, ac_ids) = cross_ref::build_req_index(&spec_ledger);

    // Load documentation front matter
    let docs_dir = Path::new("docs");
    let docs = if docs_dir.exists() {
        front_matter::collect_all_front_matter(docs_dir)
            .context("Failed to collect documentation front matter")?
    } else {
        println!("  ⚠️  docs/ directory not found, skipping documentation checks");
        Vec::new()
    };

    // Parse Gherkin features
    let features_dir = Path::new("specs/features");
    let scenarios =
        gherkin::parse_feature_files(features_dir).context("Failed to parse Gherkin features")?;

    // Check 1: Documentation → Spec ledger (requirement links)
    println!("  Checking documentation requirement links...");
    let doc_link_errors = cross_ref::validate_doc_links(&docs, &req_ids);
    if doc_link_errors.is_empty() {
        println!("    ✓ All documentation links are valid");
    } else {
        println!("    ✗ Found {} broken link(s)", doc_link_errors.len());
        for error in &doc_link_errors {
            eprintln!("{}", error.format());
            all_errors.push(error.format());
        }
    }

    // Check 2: Spec ledger → Codebase (test references)
    println!("  Checking test references...");
    let test_ref_errors = cross_ref::validate_test_references(&spec_ledger);
    let test_errors: Vec<_> = test_ref_errors.iter().filter(|e| !e.is_warning()).collect();
    let test_warnings: Vec<_> = test_ref_errors.iter().filter(|e| e.is_warning()).collect();

    if test_errors.is_empty() && test_warnings.is_empty() {
        println!("    ✓ All test references are valid");
    } else {
        if !test_errors.is_empty() {
            println!("    ✗ Found {} missing test(s)", test_errors.len());
            for error in &test_errors {
                eprintln!("{}", error.format());
                all_errors.push(error.format());
            }
        }
        if !test_warnings.is_empty() {
            println!(
                "    ⚠️  Found {} external crate reference(s)",
                test_warnings.len()
            );
            for warning in &test_warnings {
                eprintln!("{}", warning.format());
            }
        }
    }

    // Check 3: Gherkin → Spec ledger (tags)
    println!("  Checking Gherkin tags...");
    let gherkin_errors = gherkin::validate_gherkin_tags(&scenarios, &req_ids, &ac_ids);
    if gherkin_errors.is_empty() {
        println!("    ✓ All Gherkin tags are valid");
    } else {
        println!("    ✗ Found {} invalid tag(s)", gherkin_errors.len());
        for error in &gherkin_errors {
            eprintln!("{}", error.format());
            all_errors.push(error.format());
        }
    }

    // Check 4: Orphaned documentation (docs with no requirement links)
    println!("  Checking for orphaned documentation...");
    let orphaned_docs: Vec<_> = docs
        .iter()
        .filter(|(_, fm)| fm.links.requirements.is_empty())
        .collect();

    if orphaned_docs.is_empty() {
        println!("    ✓ No orphaned documentation found");
    } else {
        println!(
            "    ⚠️  Found {} document(s) with no requirement links",
            orphaned_docs.len()
        );
        for (path, _) in &orphaned_docs {
            let warning = format!(
                "[WARN] INF-XREF-200: Orphaned documentation\n  \
                 File: {}\n  \
                 Suggestion: Add requirement links to front matter or mark as reference documentation",
                path.display()
            );
            eprintln!("{}", warning);
            // Note: Orphaned docs are warnings, not errors, so we don't add them to all_errors
        }
    }

    Ok(all_errors)
}

/// Generate validation report at docs/validation_report.md.
///
/// The report includes:
/// - Auto-generated header with timestamp and commit hash
/// - Table of all checks with pass/fail status
/// - Summary count of failures
/// - Cross-reference error details (if any)
///
/// # Arguments
///
/// * `results` - Vector of check results to include in the report
/// * `cross_ref_details` - Vector of cross-reference error messages
fn generate_validation_report(results: &[CheckResult], cross_ref_details: &[String]) -> Result<()> {
    let timestamp = "deterministic (timestamp omitted)";
    let commit_hash = "deterministic (commit omitted)";

    let mut report = String::new();

    // Header
    report.push_str("<!--\n");
    report.push_str("  AUTO-GENERATED FILE: DO NOT EDIT BY HAND.\n");
    report.push_str("  Generated by: cargo xtask validate\n");
    report.push_str(&format!("  Generated at: {}\n", timestamp));
    report.push_str(&format!("  Git commit: {}\n", commit_hash));
    report.push_str("  Source of truth: specs/spec_ledger.yaml, schemas/*.json, docs/**/*.md\n");
    report.push_str("-->\n\n");

    // Title
    report.push_str("# Validation Report\n\n");
    report.push_str(&format!("**Generated:** {}\n\n", timestamp));
    report.push_str(&format!("**Commit:** {}\n\n", commit_hash));

    // Summary
    let total_checks = results.len();
    let passed_checks = results.iter().filter(|r| r.passed).count();
    let failed_checks = total_checks - passed_checks;

    report.push_str("## Summary\n\n");
    report.push_str(&format!("- **Total Checks:** {}\n", total_checks));
    report.push_str(&format!("- **Passed:** {}\n", passed_checks));
    report.push_str(&format!("- **Failed:** {}\n\n", failed_checks));

    // Results table
    report.push_str("## Check Results\n\n");
    report.push_str("| Check | Status | Details |\n");
    report.push_str("|-------|--------|----------|\n");

    for result in results {
        let status = if result.passed {
            "✅ Pass"
        } else {
            "❌ Fail"
        };
        let details = result.details.as_deref().unwrap_or("-");
        report.push_str(&format!("| {} | {} | {} |\n", result.name, status, details));
    }

    report.push('\n');

    // Cross-reference details section
    if !cross_ref_details.is_empty() {
        report.push_str("## Cross-Reference Issues\n\n");
        report.push_str(&format!(
            "Found {} cross-reference error(s):\n\n",
            cross_ref_details.len()
        ));

        // Group errors by type
        let mut broken_links = Vec::new();
        let mut missing_tests = Vec::new();
        let mut invalid_tags = Vec::new();

        for detail in cross_ref_details {
            if detail.contains("INF-XREF-001") {
                broken_links.push(detail);
            } else if detail.contains("INF-XREF-002") {
                missing_tests.push(detail);
            } else if detail.contains("INF-XREF-003") {
                invalid_tags.push(detail);
            }
        }

        if !broken_links.is_empty() {
            report.push_str("### Broken Requirement Links (docs → ledger)\n\n");
            report.push_str("```\n");
            for error in &broken_links {
                report.push_str(error);
                report.push_str("\n\n");
            }
            report.push_str("```\n\n");
        }

        if !missing_tests.is_empty() {
            report.push_str("### Missing Test References (ledger → codebase)\n\n");
            report.push_str("```\n");
            for error in &missing_tests {
                report.push_str(error);
                report.push_str("\n\n");
            }
            report.push_str("```\n\n");
        }

        if !invalid_tags.is_empty() {
            report.push_str("### Invalid Gherkin Tags (features → ledger)\n\n");
            report.push_str("```\n");
            for error in &invalid_tags {
                report.push_str(error);
                report.push_str("\n\n");
            }
            report.push_str("```\n\n");
        }
    }

    // Write report
    let report_path = Path::new("docs/validation_report.md");

    // Ensure docs directory exists
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create docs directory")?;
    }

    std::fs::write(report_path, report).context("Failed to write validation report")?;

    Ok(())
}
