#!/usr/bin/env cargo
//! SPDX-License-Identifier: MIT OR Apache-2.0
//! SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tokio Feature Validation Script
//!
//! This script validates that tokio dependencies do not use the "full" feature
//! at the workspace root level. This prevents accidental re-enabling of all
//! tokio features which increases dependency count and compilation time.
//!
//! Usage: cargo run --bin validate_tokio_features

use std::fs;
use std::process;

fn main() {
    println!("🔍 Validating tokio feature configuration...");
    
    // Check workspace Cargo.toml
    let workspace_toml = match fs::read_to_string("Cargo.toml") {
        Ok(content) => content,
        Err(e) => {
            eprintln!("❌ Failed to read workspace Cargo.toml: {}", e);
            process::exit(1);
        }
    };
    
    // Check for tokio "full" features in workspace dependencies
    if workspace_toml.contains(r#"tokio = { version = "1.35", features = ["full"]"#) {
        eprintln!("❌ VALIDATION FAILED: tokio uses 'full' features in workspace dependencies");
        eprintln!("   Expected: tokio = {{ version = \"1.35\", features = [\"macros\", \"rt-multi-thread\", \"time\", \"fs\", \"signal\", \"sync\"] }}");
        eprintln!("   Found: tokio with 'full' features");
        eprintln!("   This violates requirement SC-05.4 for minimal feature sets");
        process::exit(1);
    }
    
    // Check examples Cargo.toml
    let examples_toml = match fs::read_to_string("examples/Cargo.toml") {
        Ok(content) => content,
        Err(e) => {
            eprintln!("❌ Failed to read examples/Cargo.toml: {}", e);
            process::exit(1);
        }
    };
    
    // Check for tokio "full" features in examples
    if examples_toml.contains(r#"tokio = { version = "1.35", features = ["full"]"#) {
        eprintln!("❌ VALIDATION FAILED: tokio uses 'full' features in examples crate");
        eprintln!("   Expected: tokio = {{ version = \"1.35\", features = [\"macros\", \"rt-multi-thread\", \"time\", \"fs\", \"signal\", \"sync\"], optional = true }}");
        eprintln!("   Found: tokio with 'full' features");
        eprintln!("   This violates requirement SC-05.4 for minimal feature sets");
        process::exit(1);
    }
    
    // Verify the correct minimal feature set is present
    let expected_features = r#"features = ["macros", "rt-multi-thread", "time", "fs", "signal", "sync"]"#;
    
    if !workspace_toml.contains(expected_features) {
        eprintln!("❌ VALIDATION FAILED: workspace tokio does not use expected minimal feature set");
        eprintln!("   Expected features: [\"macros\", \"rt-multi-thread\", \"time\", \"fs\", \"signal\", \"sync\"]");
        process::exit(1);
    }
    
    println!("✅ Tokio feature validation passed");
    println!("   - Workspace tokio uses minimal feature set");
    println!("   - Examples tokio uses minimal feature set");
    println!("   - No 'full' features detected");
}