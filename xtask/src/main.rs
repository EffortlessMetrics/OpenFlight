// SPDX-License-Identifier: MIT OR Apache-2.0

//! xtask - Project automation commands for Flight Hub
//!
//! This crate provides a unified entry point for all project automation tasks,
//! including validation, testing, and infrastructure management.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::env;

mod ac_status;
mod bench_compare;
mod changelog;
mod check;
mod clean_worktrees;
mod compat;
mod config;
mod coverage;
mod cross_ref;
mod device_report;
mod front_matter;
mod fuzz_smoke;
mod gherkin;
mod hotas;
mod normalize_docs;
mod quality_gates;
mod release;
mod schema;
mod validate;
mod validate_infra;

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "Project automation commands for Flight Hub", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Fast local smoke test (fmt, clippy, core tests)
    Check,

    /// Full quality gate (check + benches, API, cross-ref)
    Validate,

    /// Generate feature status report
    AcStatus,

    /// Normalize documentation front matter
    NormalizeDocs,

    /// Validate infrastructure configurations
    ValidateInfra,

    /// Validate a YAML file against a JSON schema (for testing)
    ValidateSchema {
        /// Path to the YAML file to validate
        yaml_path: String,
        /// Path to the JSON schema file
        schema_path: String,
    },

    /// HOTAS device verification tools
    Hotas {
        #[command(subcommand)]
        command: hotas::HotasCommand,
    },

    /// Generate COMPATIBILITY.md from compat/ manifests
    GenCompat,

    /// Generate COMPATIBILITY.md and compatibility.json from compat/ manifests
    GenerateCompat,

    /// Run code coverage report on core crates using cargo-llvm-cov
    Coverage {
        /// Generate an HTML report in target/coverage/
        #[arg(long)]
        html: bool,

        /// Minimum coverage percentage (default: 60%)
        #[arg(long)]
        threshold: Option<f64>,
    },

    /// Prepare a new release (bump versions, update CHANGELOG, tag)
    Release {
        /// Version to release (e.g., 1.2.3)
        version: String,
    },

    /// Generate device coverage report from compat/devices/ manifests
    DeviceReport {
        /// Output as JSON instead of a table
        #[arg(long)]
        json: bool,
    },

    /// Clean up stale/merged git worktrees
    CleanWorktrees {
        /// Force removal of stale worktrees without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Run each fuzz target for a short duration as a smoke test
    FuzzSmoke {
        /// Duration in seconds per fuzz target (default: 5)
        #[arg(long)]
        duration: Option<u64>,
    },

    /// Compare benchmark results against stored baselines
    BenchCompare {
        /// Regression threshold percentage (default: 10)
        #[arg(long, default_value_t = 10.0)]
        threshold: f64,

        /// Save current results as the new baseline
        #[arg(long)]
        save_baseline: bool,
    },

    /// Generate changelog from conventional commits since the last tag
    Changelog {
        /// Git ref to start from (tag, commit, branch). Defaults to latest tag.
        #[arg(long)]
        since: Option<String>,

        /// Write output into CHANGELOG.md instead of stdout
        #[arg(long)]
        write: bool,
    },

    /// Prepare a release: generate changelog, bump versions, create tag
    PrepareRelease {
        /// Explicit version to release (e.g., 1.2.3). Mutually exclusive with --bump.
        version: Option<String>,

        /// Automatically bump: major, minor, patch, or pre:<label> (e.g., pre:rc.1)
        #[arg(long)]
        bump: Option<String>,
    },
}

fn main() -> Result<()> {
    // Ensure we're running from the workspace root
    ensure_workspace_root()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Check => check::run_check(),
        Commands::Validate => validate::run_validate(),
        Commands::AcStatus => ac_status::run_ac_status(),
        Commands::NormalizeDocs => normalize_docs::run_normalize_docs(),
        Commands::ValidateInfra => validate_infra::run_validate_infra(),
        Commands::ValidateSchema {
            yaml_path,
            schema_path,
        } => {
            use std::path::Path;
            let yaml_path = Path::new(&yaml_path);
            let schema_path = Path::new(&schema_path);

            match schema::validate_yaml_against_schema(yaml_path, schema_path) {
                Ok(()) => {
                    println!(
                        "✓ Validation successful: {} conforms to schema",
                        yaml_path.display()
                    );
                    Ok(())
                }
                Err(errors) => {
                    eprintln!("✗ Validation failed with {} error(s):", errors.len());
                    for error in errors {
                        eprintln!("{}", error.format());
                    }
                    anyhow::bail!("Schema validation failed");
                }
            }
        }
        Commands::Hotas { command } => hotas::run(command),
        Commands::GenCompat => compat::run_gen_compat(),
        Commands::GenerateCompat => compat::run_gen_compat(),
        Commands::Coverage { html, threshold } => coverage::run_coverage(html, threshold),
        Commands::Release { version } => release::run_release(&version),
        Commands::DeviceReport { json } => device_report::run_device_report(json),
        Commands::CleanWorktrees { force } => clean_worktrees::run_clean_worktrees(force),
        Commands::FuzzSmoke { duration } => fuzz_smoke::run_fuzz_smoke(duration),
        Commands::BenchCompare {
            threshold,
            save_baseline,
        } => bench_compare::run_bench_compare(threshold, save_baseline),
        Commands::Changelog { since, write } => changelog::run_changelog(since.as_deref(), write),
        Commands::PrepareRelease { version, bump } => {
            let version = release::resolve_version(version, bump)?;
            release::run_prepare_release(&version)
        }
    }
}

/// Ensure xtask is running from the workspace root.
///
/// All xtask commands MUST run from workspace root to ensure consistent
/// path resolution. This function changes the current directory to the
/// workspace root if needed.
fn ensure_workspace_root() -> Result<()> {
    // Get the current executable's directory (xtask binary location)
    let current_dir = env::current_dir()?;

    // Look for Cargo.toml in current directory or parent directories
    let mut search_dir = current_dir.clone();
    loop {
        let cargo_toml = search_dir.join("Cargo.toml");
        if cargo_toml.exists() {
            // Check if this is a workspace Cargo.toml
            let content = std::fs::read_to_string(&cargo_toml)?;
            if content.contains("[workspace]") {
                // Found workspace root, change to it if not already there
                if search_dir != current_dir {
                    env::set_current_dir(&search_dir)?;
                    println!(
                        "Changed directory to workspace root: {}",
                        search_dir.display()
                    );
                }
                return Ok(());
            }
        }

        // Move up one directory
        if let Some(parent) = search_dir.parent() {
            search_dir = parent.to_path_buf();
        } else {
            anyhow::bail!("Could not find workspace root (Cargo.toml with [workspace])");
        }
    }
}
