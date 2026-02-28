// SPDX-License-Identifier: MIT OR Apache-2.0

//! Quick fuzz smoke test runner.
//!
//! Run with: `cargo xtask fuzz-smoke [--duration <seconds>]`
//!
//! Discovers fuzz targets in `crates/*/fuzz/` and runs each for a short
//! duration, reporting pass/fail per target.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Default duration per fuzz target in seconds.
const DEFAULT_DURATION_SECS: u64 = 5;

/// A discovered fuzz target.
#[derive(Debug)]
struct FuzzTarget {
    crate_name: String,
    target_name: String,
    fuzz_dir: PathBuf,
}

/// Run fuzz smoke tests.
pub fn run_fuzz_smoke(duration_secs: Option<u64>) -> Result<()> {
    let duration = duration_secs.unwrap_or(DEFAULT_DURATION_SECS);

    println!("🔥 Fuzz Smoke Test ({}s per target)\n", duration);

    // Check cargo-fuzz availability
    let fuzz_check = Command::new("cargo")
        .args(["fuzz", "--version"])
        .output()
        .context("Failed to execute cargo fuzz. Is it installed? Run: cargo install cargo-fuzz")?;

    if !fuzz_check.status.success() {
        anyhow::bail!("cargo-fuzz not found. Install with: cargo install cargo-fuzz");
    }

    let targets = discover_fuzz_targets()?;

    if targets.is_empty() {
        println!("  No fuzz targets found in crates/*/fuzz/");
        return Ok(());
    }

    println!("  Discovered {} fuzz target(s)\n", targets.len());

    let mut passed = 0;
    let mut failed = 0;
    let mut failures: Vec<String> = Vec::new();

    for target in &targets {
        print!("  {}/{}: ", target.crate_name, target.target_name);

        match run_single_target(target, duration) {
            Ok(true) => {
                println!("✅ PASS");
                passed += 1;
            }
            Ok(false) => {
                println!("❌ FAIL");
                failed += 1;
                failures.push(format!("{}/{}", target.crate_name, target.target_name));
            }
            Err(e) => {
                println!("⚠ ERROR: {}", e);
                failed += 1;
                failures.push(format!(
                    "{}/{} (error)",
                    target.crate_name, target.target_name
                ));
            }
        }
    }

    println!("\n  Results: {} passed, {} failed", passed, failed);

    if !failures.is_empty() {
        println!("\n  Failed targets:");
        for f in &failures {
            println!("    ❌ {}", f);
        }
        anyhow::bail!("{} fuzz target(s) failed", failed);
    }

    println!("\n  ✅ All fuzz targets passed smoke test.");
    Ok(())
}

/// Discover fuzz targets by scanning `crates/*/fuzz/Cargo.toml` for `[[bin]]` entries.
fn discover_fuzz_targets() -> Result<Vec<FuzzTarget>> {
    let crates_dir = Path::new("crates");
    if !crates_dir.exists() {
        anyhow::bail!("crates/ directory not found. Run from workspace root.");
    }

    let mut targets = Vec::new();

    let Ok(entries) = fs::read_dir(crates_dir) else {
        return Ok(targets);
    };

    for entry in entries.flatten() {
        let fuzz_dir = entry.path().join("fuzz");
        let fuzz_cargo = fuzz_dir.join("Cargo.toml");

        if !fuzz_cargo.exists() {
            continue;
        }

        let crate_name = entry.file_name().to_string_lossy().to_string();

        // Parse fuzz Cargo.toml for [[bin]] target names
        let content = fs::read_to_string(&fuzz_cargo)
            .with_context(|| format!("Failed to read {}", fuzz_cargo.display()))?;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("name")
                && trimmed.contains('=')
                && let Some(name) = extract_toml_string_value(trimmed)
            {
                targets.push(FuzzTarget {
                    crate_name: crate_name.clone(),
                    target_name: name,
                    fuzz_dir: fuzz_dir.clone(),
                });
            }
        }
    }

    targets.sort_by(|a, b| {
        a.crate_name
            .cmp(&b.crate_name)
            .then(a.target_name.cmp(&b.target_name))
    });

    Ok(targets)
}

/// Extract a string value from a TOML line like `name = "fuzz_target"`.
fn extract_toml_string_value(line: &str) -> Option<String> {
    let parts: Vec<&str> = line.splitn(2, '=').collect();
    if parts.len() == 2 {
        let value = parts[1].trim().trim_matches('"');
        // Skip package names (which are usually *-fuzz)
        if value.ends_with("-fuzz") || value == "true" || value == "false" {
            return None;
        }
        Some(value.to_string())
    } else {
        None
    }
}

/// Run a single fuzz target for the given duration. Returns `Ok(true)` on success.
fn run_single_target(target: &FuzzTarget, duration_secs: u64) -> Result<bool> {
    let max_total_time = format!("{}", duration_secs);

    let status = Command::new("cargo")
        .args([
            "fuzz",
            "run",
            &target.target_name,
            "--",
            "-max_total_time",
            &max_total_time,
        ])
        .current_dir(&target.fuzz_dir)
        .status()
        .with_context(|| {
            format!(
                "Failed to run fuzz target {}/{}",
                target.crate_name, target.target_name
            )
        })?;

    Ok(status.success())
}
