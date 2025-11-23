// SPDX-License-Identifier: MIT OR Apache-2.0

//! Fast local smoke test implementation.
//!
//! This module implements the `cargo xtask check` command, which runs:
//! 1. Formatting checks (`cargo fmt --all -- --check`)
//! 2. Clippy linting for core crates
//! 3. Unit tests for core crates
//!
//! The check command is designed to complete quickly (< 1 minute) to provide
//! rapid feedback during local development.

use anyhow::{Context, Result};
use std::process::Command;

use crate::config::CORE_CRATES;

/// Run fast local smoke tests.
///
/// This function executes formatting checks, clippy linting, and unit tests
/// for core crates. It returns an error if any check fails.
///
/// # Errors
///
/// Returns an error if:
/// - Formatting check fails
/// - Clippy reports warnings or errors
/// - Any unit test fails
pub fn run_check() -> Result<()> {
    println!("🔍 Running fast local checks...\n");

    let mut all_passed = true;

    // Step 1: Formatting check
    println!("📝 Checking formatting...");
    if !check_formatting()? {
        all_passed = false;
        eprintln!("❌ Formatting check failed");
    } else {
        println!("✅ Formatting check passed\n");
    }

    // Step 2: Clippy for core crates
    println!("🔧 Running clippy on core crates...");
    if !check_clippy()? {
        all_passed = false;
        eprintln!("❌ Clippy check failed");
    } else {
        println!("✅ Clippy check passed\n");
    }

    // Step 3: Unit tests for core crates
    println!("🧪 Running unit tests on core crates...");
    if !check_tests()? {
        all_passed = false;
        eprintln!("❌ Unit tests failed");
    } else {
        println!("✅ Unit tests passed\n");
    }

    // Final result
    if all_passed {
        println!("✅ All checks passed!");
        Ok(())
    } else {
        anyhow::bail!("Some checks failed. See output above for details.");
    }
}

/// Check code formatting using `cargo fmt`.
///
/// Returns `true` if formatting is correct, `false` otherwise.
fn check_formatting() -> Result<bool> {
    let status = Command::new("cargo")
        .args(["fmt", "--all", "--", "--check"])
        .status()
        .context("Failed to execute cargo fmt")?;

    Ok(status.success())
}

/// Run clippy on core crates.
///
/// Returns `true` if clippy passes with no warnings, `false` otherwise.
fn check_clippy() -> Result<bool> {
    let mut all_passed = true;

    for crate_name in CORE_CRATES {
        println!("  Checking {}...", crate_name);

        let status = Command::new("cargo")
            .args(["clippy", "-p", crate_name, "--", "-D", "warnings"])
            .status()
            .context(format!("Failed to execute clippy for {}", crate_name))?;

        if !status.success() {
            eprintln!("  ❌ Clippy failed for {}", crate_name);
            all_passed = false;
        }
    }

    Ok(all_passed)
}

/// Run unit tests on core crates.
///
/// Returns `true` if all tests pass, `false` otherwise.
fn check_tests() -> Result<bool> {
    let mut all_passed = true;

    for crate_name in CORE_CRATES {
        println!("  Testing {}...", crate_name);

        let status = Command::new("cargo")
            .args(["test", "-p", crate_name])
            .status()
            .context(format!("Failed to execute tests for {}", crate_name))?;

        if !status.success() {
            eprintln!("  ❌ Tests failed for {}", crate_name);
            all_passed = false;
        }
    }

    Ok(all_passed)
}
