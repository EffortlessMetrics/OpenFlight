// SPDX-License-Identifier: MIT OR Apache-2.0

//! Coverage report generation using `cargo llvm-cov`.
//!
//! Run with: `cargo xtask coverage [--html] [--threshold <percent>]`

use anyhow::{Context, Result};
use std::process::Command;

/// Core crates measured for coverage.
const COVERAGE_CRATES: &[&str] = &[
    "flight-core",
    "flight-axis",
    "flight-bus",
    "flight-scheduler",
];

/// Default minimum coverage percentage (line coverage).
const DEFAULT_THRESHOLD: f64 = 60.0;

/// Run coverage report.
pub fn run_coverage(html: bool, threshold: Option<f64>) -> Result<()> {
    let threshold = threshold.unwrap_or(DEFAULT_THRESHOLD);

    println!("📊 Running coverage report on core crates...\n");

    // Check that cargo-llvm-cov is available
    let version_check = Command::new("cargo")
        .args(["llvm-cov", "--version"])
        .output()
        .context(
            "Failed to execute cargo llvm-cov. Is it installed? Run: cargo install cargo-llvm-cov",
        )?;

    if !version_check.status.success() {
        anyhow::bail!("cargo-llvm-cov not found. Install with: cargo install cargo-llvm-cov");
    }

    // Build package args: -p crate1 -p crate2 ...
    let mut pkg_args: Vec<&str> = Vec::new();
    for crate_name in COVERAGE_CRATES {
        pkg_args.push("-p");
        pkg_args.push(crate_name);
    }

    // Run coverage with JSON output for threshold checking
    println!("  Running tests with coverage instrumentation...");
    let mut json_args = vec!["llvm-cov", "--json"];
    json_args.extend_from_slice(&pkg_args);

    let json_output = Command::new("cargo")
        .args(&json_args)
        .output()
        .context("Failed to execute cargo llvm-cov --json")?;

    if !json_output.status.success() {
        let stderr = String::from_utf8_lossy(&json_output.stderr);
        anyhow::bail!("Coverage run failed:\n{}", stderr);
    }

    // Parse JSON coverage data
    let json_str = String::from_utf8_lossy(&json_output.stdout);
    let cov_data: serde_json::Value =
        serde_json::from_str(&json_str).context("Failed to parse coverage JSON output")?;

    // Extract and display per-crate summary
    println!(
        "\n  {:<25} {:>10} {:>10} {:>10}",
        "Crate", "Lines", "Covered", "Coverage"
    );
    println!("  {}", "-".repeat(58));

    let mut total_lines: u64 = 0;
    let mut total_covered: u64 = 0;

    // llvm-cov JSON has data[].files[] with summary info
    if let Some(data) = cov_data.get("data").and_then(|d| d.as_array()) {
        for entry in data {
            if let Some(files) = entry.get("files").and_then(|f| f.as_array()) {
                for file in files {
                    let filename = file.get("filename").and_then(|f| f.as_str()).unwrap_or("");

                    // Match files to crates
                    for &crate_name in COVERAGE_CRATES {
                        let crate_dir = crate_name.replace('-', "_");
                        if filename.contains(&crate_dir) || filename.contains(crate_name) {
                            if let Some(summary) = file.get("summary") {
                                let lines = summary
                                    .get("lines")
                                    .and_then(|l| l.get("count"))
                                    .and_then(|c| c.as_u64())
                                    .unwrap_or(0);
                                let covered = summary
                                    .get("lines")
                                    .and_then(|l| l.get("covered"))
                                    .and_then(|c| c.as_u64())
                                    .unwrap_or(0);
                                total_lines += lines;
                                total_covered += covered;
                            }
                            break;
                        }
                    }
                }
            }

            // Use totals from the summary if available
            if let Some(totals) = entry.get("totals") {
                let lines = totals
                    .get("lines")
                    .and_then(|l| l.get("count"))
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0);
                let covered = totals
                    .get("lines")
                    .and_then(|l| l.get("covered"))
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0);

                if lines > 0 {
                    total_lines = lines;
                    total_covered = covered;
                }
            }
        }
    }

    let overall_pct = if total_lines > 0 {
        (total_covered as f64 / total_lines as f64) * 100.0
    } else {
        0.0
    };

    println!(
        "  {:<25} {:>10} {:>10} {:>9.1}%",
        "TOTAL", total_lines, total_covered, overall_pct
    );

    // Also run per-crate to show individual summaries
    for crate_name in COVERAGE_CRATES {
        print_crate_summary(crate_name);
    }

    // Generate HTML report if requested
    if html {
        println!("\n  Generating HTML coverage report...");
        let mut html_args = vec!["llvm-cov", "--html", "--output-dir", "target/coverage"];
        html_args.extend_from_slice(&pkg_args);

        let html_status = Command::new("cargo")
            .args(&html_args)
            .status()
            .context("Failed to generate HTML coverage report")?;

        if html_status.success() {
            println!("  ✅ HTML report written to target/coverage/");
        } else {
            eprintln!("  ⚠ HTML report generation failed");
        }
    }

    // Check threshold
    println!();
    if overall_pct < threshold {
        anyhow::bail!(
            "❌ Coverage {:.1}% is below threshold {:.1}%",
            overall_pct,
            threshold
        );
    }

    println!(
        "✅ Coverage {:.1}% meets threshold {:.1}%",
        overall_pct, threshold
    );
    Ok(())
}

/// Print a brief summary line for a single crate (best-effort).
fn print_crate_summary(crate_name: &str) {
    let output = Command::new("cargo")
        .args(["llvm-cov", "--json", "-p", crate_name])
        .output();

    if let Ok(out) = output
        && out.status.success()
    {
        let json_str = String::from_utf8_lossy(&out.stdout);
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&json_str)
            && let Some(totals) = data
                .get("data")
                .and_then(|d| d.get(0))
                .and_then(|e| e.get("totals"))
        {
            let lines = totals
                .get("lines")
                .and_then(|l| l.get("count"))
                .and_then(|c| c.as_u64())
                .unwrap_or(0);
            let covered = totals
                .get("lines")
                .and_then(|l| l.get("covered"))
                .and_then(|c| c.as_u64())
                .unwrap_or(0);
            let pct = if lines > 0 {
                (covered as f64 / lines as f64) * 100.0
            } else {
                0.0
            };
            println!(
                "  {:<25} {:>10} {:>10} {:>9.1}%",
                crate_name, lines, covered, pct
            );
        }
    }
}
