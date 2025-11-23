// SPDX-License-Identifier: MIT OR Apache-2.0

//! xtask - Project automation commands for Flight Hub
//!
//! This crate provides a unified entry point for all project automation tasks,
//! including validation, testing, and infrastructure management.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::env;

mod ac_status;
mod check;
mod config;
mod cross_ref;
mod front_matter;
mod gherkin;
mod normalize_docs;
mod schema;
mod validate;

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
        Commands::ValidateInfra => {
            println!("Validating infrastructure...");
            println!("✓ ValidateInfra command placeholder - implementation in next task");
            Ok(())
        }
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
