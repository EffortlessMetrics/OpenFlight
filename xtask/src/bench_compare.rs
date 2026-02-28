// SPDX-License-Identifier: MIT OR Apache-2.0

//! `bench-compare` — run benchmarks and compare against stored baselines.
//!
//! Loads baseline data from `benches/baselines/*.json`, runs the corresponding
//! criterion benchmarks, and reports any regressions exceeding the configured
//! threshold.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Baseline schema
// ---------------------------------------------------------------------------

/// Top-level baseline file (matches `benches/baselines/<crate>.json`).
#[derive(Debug, Serialize, Deserialize)]
pub struct BaselineFile {
    pub version: u32,
    pub created_at: String,
    pub baselines: BTreeMap<String, BaselineEntry>,
}

/// A single benchmark baseline.
#[derive(Debug, Serialize, Deserialize)]
pub struct BaselineEntry {
    pub mean_ns: f64,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Criterion estimates (subset we care about)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CriterionEstimates {
    mean: CriterionStat,
}

#[derive(Debug, Deserialize)]
struct CriterionStat {
    point_estimate: f64,
    // unit is always nanoseconds in criterion
}

// ---------------------------------------------------------------------------
// Comparison result
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct BenchResult {
    name: String,
    baseline_ns: f64,
    measured_ns: f64,
    change_pct: f64,
    regression: bool,
}

// ---------------------------------------------------------------------------
// Tracked benchmark suites
// ---------------------------------------------------------------------------

struct BenchSuite {
    crate_name: &'static str,
    bench_name: &'static str,
    baseline_file: &'static str,
    extra_features: &'static [&'static str],
}

const SUITES: &[BenchSuite] = &[
    BenchSuite {
        crate_name: "flight-axis",
        bench_name: "axis_performance",
        baseline_file: "flight-axis.json",
        extra_features: &["benches-optin"],
    },
    BenchSuite {
        crate_name: "flight-bus",
        bench_name: "bus_routing",
        baseline_file: "flight-bus.json",
        extra_features: &[],
    },
    BenchSuite {
        crate_name: "flight-profile",
        bench_name: "profile_merge",
        baseline_file: "flight-profile.json",
        extra_features: &[],
    },
];

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run_bench_compare(threshold: f64, save_baseline: bool) -> Result<()> {
    let baselines_dir = PathBuf::from("benches/baselines");

    if save_baseline {
        return save_baselines(&baselines_dir);
    }

    println!("🏋️  Performance regression check (threshold: {threshold}%)");
    println!();

    let mut all_results: Vec<BenchResult> = Vec::new();
    let mut suite_errors: Vec<String> = Vec::new();

    for suite in SUITES {
        let baseline_path = baselines_dir.join(suite.baseline_file);
        if !baseline_path.exists() {
            println!(
                "⚠  Baseline not found: {} — skipping {}",
                baseline_path.display(),
                suite.crate_name
            );
            continue;
        }

        let baseline: BaselineFile = serde_json::from_str(
            &std::fs::read_to_string(&baseline_path)
                .with_context(|| format!("reading {}", baseline_path.display()))?,
        )
        .with_context(|| format!("parsing {}", baseline_path.display()))?;

        // Run the criterion benchmark
        println!(
            "▶  Running benchmarks: {} ({})",
            suite.bench_name, suite.crate_name
        );
        if let Err(e) = run_criterion_bench(suite) {
            let msg = format!("{}: {e}", suite.crate_name);
            println!("⚠  Benchmark run failed: {msg}");
            suite_errors.push(msg);
            continue;
        }

        // Collect results from criterion output
        let results = collect_results(suite, &baseline, threshold)?;
        all_results.extend(results);
    }

    // Print summary
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  Performance Regression Report");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let regressions: Vec<&BenchResult> = all_results.iter().filter(|r| r.regression).collect();
    let improvements: Vec<&BenchResult> = all_results
        .iter()
        .filter(|r| r.change_pct < -threshold)
        .collect();

    for r in &all_results {
        let icon = if r.regression {
            "❌"
        } else if r.change_pct < -threshold {
            "🚀"
        } else {
            "✅"
        };
        println!(
            "  {icon} {:<45} {:>+7.1}%  ({:.0}ns → {:.0}ns)",
            r.name, r.change_pct, r.baseline_ns, r.measured_ns
        );
    }

    println!();
    println!(
        "  Checked: {}  Regressions: {}  Improvements: {}",
        all_results.len(),
        regressions.len(),
        improvements.len()
    );

    if !suite_errors.is_empty() {
        println!(
            "  ⚠  {} suite(s) failed to run: {}",
            suite_errors.len(),
            suite_errors.join(", ")
        );
    }

    if !regressions.is_empty() {
        println!();
        println!(
            "❌ {} regression(s) detected above {threshold}% threshold:",
            regressions.len()
        );
        for r in &regressions {
            println!("   • {} ({:+.1}%)", r.name, r.change_pct);
        }
        anyhow::bail!(
            "Performance regression detected: {} benchmark(s) exceeded {threshold}% threshold",
            regressions.len()
        );
    }

    println!();
    println!("✅ No performance regressions detected.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Run a criterion benchmark suite
// ---------------------------------------------------------------------------

fn run_criterion_bench(suite: &BenchSuite) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("bench")
        .arg("-p")
        .arg(suite.crate_name)
        .arg("--bench")
        .arg(suite.bench_name);

    if !suite.extra_features.is_empty() {
        cmd.arg("--features").arg(suite.extra_features.join(","));
    }

    // Pass criterion args: no HTML report, minimal output
    cmd.arg("--").arg("--noplot");

    let status = cmd
        .status()
        .with_context(|| format!("running bench for {}", suite.crate_name))?;

    if !status.success() {
        anyhow::bail!("cargo bench failed for {}", suite.crate_name);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Collect criterion results and compare against baselines
// ---------------------------------------------------------------------------

fn collect_results(
    _suite: &BenchSuite,
    baseline: &BaselineFile,
    threshold: f64,
) -> Result<Vec<BenchResult>> {
    let criterion_dir = PathBuf::from("target/criterion");
    let mut results = Vec::new();

    for (bench_name, entry) in &baseline.baselines {
        let estimates_path = find_estimates_file(&criterion_dir, bench_name);

        let measured_ns = match estimates_path {
            Some(path) => read_criterion_mean(&path)?,
            None => {
                println!("  ⚠  No criterion output for '{}' — skipping", bench_name);
                continue;
            }
        };

        let change_pct = ((measured_ns - entry.mean_ns) / entry.mean_ns) * 100.0;
        let regression = change_pct > threshold;

        results.push(BenchResult {
            name: bench_name.clone(),
            baseline_ns: entry.mean_ns,
            measured_ns,
            change_pct,
            regression,
        });
    }

    Ok(results)
}

fn find_estimates_file(criterion_dir: &Path, bench_name: &str) -> Option<PathBuf> {
    // Criterion stores results in target/criterion/<group>/<bench_name>/new/estimates.json
    // or target/criterion/<bench_name>/new/estimates.json for ungrouped benchmarks.
    let direct = criterion_dir
        .join(bench_name)
        .join("new")
        .join("estimates.json");
    if direct.exists() {
        return Some(direct);
    }

    // Search for the bench name in subdirectories (grouped benchmarks)
    if let Ok(entries) = std::fs::read_dir(criterion_dir) {
        for entry in entries.flatten() {
            let nested = entry
                .path()
                .join(bench_name)
                .join("new")
                .join("estimates.json");
            if nested.exists() {
                return Some(nested);
            }
        }
    }

    None
}

fn read_criterion_mean(path: &Path) -> Result<f64> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let estimates: CriterionEstimates =
        serde_json::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
    Ok(estimates.mean.point_estimate)
}

// ---------------------------------------------------------------------------
// Save current benchmark results as new baselines
// ---------------------------------------------------------------------------

fn save_baselines(baselines_dir: &Path) -> Result<()> {
    println!("📊 Saving benchmark baselines...");

    for suite in SUITES {
        println!(
            "▶  Running benchmarks: {} ({})",
            suite.bench_name, suite.crate_name
        );
        run_criterion_bench(suite)?;

        let criterion_dir = PathBuf::from("target/criterion");
        let baseline_path = baselines_dir.join(suite.baseline_file);

        // Load existing baseline to preserve descriptions
        let existing: Option<BaselineFile> = std::fs::read_to_string(&baseline_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());

        let mut baselines = BTreeMap::new();

        // If existing baseline exists, update it; otherwise create new entries
        if let Some(existing) = &existing {
            for (name, entry) in &existing.baselines {
                if let Some(path) = find_estimates_file(&criterion_dir, name) {
                    let mean_ns = read_criterion_mean(&path)?;
                    baselines.insert(
                        name.clone(),
                        BaselineEntry {
                            mean_ns,
                            description: entry.description.clone(),
                        },
                    );
                    println!("  ✓ {name}: {mean_ns:.1}ns");
                } else {
                    println!("  ⚠ {name}: no criterion output, keeping existing value");
                    baselines.insert(
                        name.clone(),
                        BaselineEntry {
                            mean_ns: entry.mean_ns,
                            description: entry.description.clone(),
                        },
                    );
                }
            }
        }

        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let file = BaselineFile {
            version: 1,
            created_at: now,
            baselines,
        };

        let json = serde_json::to_string_pretty(&file)?;
        std::fs::write(&baseline_path, format!("{json}\n"))?;
        println!("  💾 Saved {}", baseline_path.display());
    }

    println!();
    println!("✅ Baselines saved.");
    Ok(())
}
