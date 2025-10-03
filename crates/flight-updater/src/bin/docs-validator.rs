// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Documentation validation tool for integration docs

use clap::{Arg, Command};
use flight_updater::integration_docs::IntegrationDocsManager;
use std::path::PathBuf;
use std::process;

#[tokio::main]
async fn main() {
    let matches = Command::new("docs-validator")
        .about("Validate Flight Hub integration documentation")
        .version("1.0.0")
        .arg(
            Arg::new("docs-dir")
                .short('d')
                .long("docs-dir")
                .value_name("DIR")
                .help("Directory containing documentation files")
                .default_value("docs/integration")
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output file for generated documentation")
        )
        .arg(
            Arg::new("simulator")
                .short('s')
                .long("simulator")
                .value_name("SIM")
                .help("Generate docs for specific simulator (msfs, xplane, dcs)")
        )
        .arg(
            Arg::new("validate-only")
                .long("validate-only")
                .help("Only validate, don't generate documentation")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    let docs_dir = PathBuf::from(matches.get_one::<String>("docs-dir").unwrap());
    let validate_only = matches.get_flag("validate-only");

    // Initialize documentation manager
    let mut manager = IntegrationDocsManager::new(&docs_dir);
    
    // Load all documentation
    if let Err(e) = manager.load_all_docs().await {
        eprintln!("Error loading documentation: {}", e);
        process::exit(1);
    }

    println!("Loaded integration documentation for all simulators");

    // Validate documentation
    match manager.validate_docs().await {
        Ok(report) => {
            println!("\n=== Validation Report ===");
            
            if !report.info.is_empty() {
                println!("\nInfo:");
                for info in &report.info {
                    println!("  ℹ {}", info);
                }
            }
            
            if !report.warnings.is_empty() {
                println!("\nWarnings:");
                for warning in &report.warnings {
                    println!("  ⚠ {}", warning);
                }
            }
            
            if !report.errors.is_empty() {
                println!("\nErrors:");
                for error in &report.errors {
                    println!("  ❌ {}", error);
                }
                
                eprintln!("\nValidation failed with {} errors", report.errors.len());
                process::exit(1);
            }
            
            println!("\n✅ Validation passed!");
        }
        Err(e) => {
            eprintln!("Validation error: {}", e);
            process::exit(1);
        }
    }

    // Generate documentation if requested
    if !validate_only {
        if let Some(simulator) = matches.get_one::<String>("simulator") {
            // Generate docs for specific simulator
            match manager.generate_user_docs(simulator) {
                Some(docs) => {
                    if let Some(output_file) = matches.get_one::<String>("output") {
                        if let Err(e) = tokio::fs::write(output_file, &docs).await {
                            eprintln!("Error writing output file: {}", e);
                            process::exit(1);
                        }
                        println!("Generated documentation for {} -> {}", simulator, output_file);
                    } else {
                        println!("\n=== {} Integration Documentation ===\n", simulator.to_uppercase());
                        println!("{}", docs);
                    }
                }
                None => {
                    eprintln!("Unknown simulator: {}", simulator);
                    process::exit(1);
                }
            }
        } else {
            // Generate docs for all simulators
            let simulators = ["msfs", "xplane", "dcs"];
            
            for sim in &simulators {
                if let Some(docs) = manager.generate_user_docs(sim) {
                    let output_file = if let Some(base) = matches.get_one::<String>("output") {
                        format!("{}_{}.md", base.trim_end_matches(".md"), sim)
                    } else {
                        format!("{}_integration.md", sim)
                    };
                    
                    if let Err(e) = tokio::fs::write(&output_file, &docs).await {
                        eprintln!("Error writing {}: {}", output_file, e);
                        continue;
                    }
                    
                    println!("Generated documentation: {}", output_file);
                }
            }
        }
    }

    println!("\nDocumentation validation and generation completed successfully!");
}