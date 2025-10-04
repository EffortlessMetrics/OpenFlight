#!/usr/bin/env cargo +nightly -Zscript
//! Regression prevention script for OpenFlight workspace
//! 
//! This script implements comprehensive regression prevention measures including:
//! - Feature powerset testing
//! - Clippy enforcement for core crates
//! - Dead code cleanup
//! - Critical pattern verification
//!
//! Usage: cargo +nightly -Zscript scripts/regression_prevention.rs [command]
//! Commands:
//!   - feature-powerset: Run feature powerset testing
//!   - clippy-strict: Run strict clippy checks on core crates
//!   - dead-code-cleanup: Clean up dead code and imports
//!   - verify-patterns: Verify critical patterns are fixed
//!   - all: Run all checks (default)

use std::process::{Command, exit};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("all");
    
    match command {
        "feature-powerset" => run_feature_powerset(),
        "clippy-strict" => run_clippy_strict(),
        "dead-code-cleanup" => run_dead_code_cleanup(),
        "verify-patterns" => verify_critical_patterns(),
        "all" => {
            run_feature_powerset();
            run_clippy_strict();
            run_dead_code_cleanup();
            verify_critical_patterns();
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            eprintln!("Available commands: feature-powerset, clippy-strict, dead-code-cleanup, verify-patterns, all");
            exit(1);
        }
    }
}

fn run_feature_powerset() {
    println!("🔍 Running feature powerset testing...");
    
    // Check if cargo-hack is installed
    let hack_check = Command::new("cargo")
        .args(&["hack", "--version"])
        .output();
        
    if hack_check.is_err() {
        println!("📦 Installing cargo-hack...");
        let install_result = Command::new("cargo")
            .args(&["install", "cargo-hack"])
            .status()
            .expect("Failed to install cargo-hack");
            
        if !install_result.success() {
            eprintln!("❌ Failed to install cargo-hack");
            exit(1);
        }
    }
    
    // Run feature powerset testing with depth 2 to balance coverage and CI time
    let result = Command::new("cargo")
        .args(&["hack", "check", "--workspace", "--feature-powerset", "--depth", "2"])
        .status()
        .expect("Failed to run cargo hack");
        
    if !result.success() {
        eprintln!("❌ Feature powerset testing failed");
        exit(1);
    }
    
    println!("✅ Feature powerset testing passed");
}

fn run_clippy_strict() {
    println!("🔍 Running strict clippy checks on core crates...");
    
    // Core crates that must pass strict clippy checks
    let core_crates = [
        "flight-core",
        "flight-axis", 
        "flight-bus",
        "flight-ipc",
        "flight-simconnect",
        "flight-hid",
        "flight-panels",
        "flight-service",
    ];
    
    for crate_name in &core_crates {
        println!("  Checking {}...", crate_name);
        let result = Command::new("cargo")
            .args(&["clippy", "-p", crate_name, "--", "-D", "warnings"])
            .status()
            .expect(&format!("Failed to run clippy on {}", crate_name));
            
        if !result.success() {
            eprintln!("❌ Clippy failed for {}", crate_name);
            exit(1);
        }
    }
    
    println!("✅ All core crates pass strict clippy checks");
}

fn run_dead_code_cleanup() {
    println!("🔍 Running dead code cleanup...");
    
    let result = Command::new("cargo")
        .args(&["fix", "--workspace", "--allow-dirty"])
        .status()
        .expect("Failed to run cargo fix");
        
    if !result.success() {
        eprintln!("❌ Dead code cleanup failed");
        exit(1);
    }
    
    println!("✅ Dead code cleanup completed");
}

fn verify_critical_patterns() {
    println!("🔍 Verifying critical patterns are fixed...");
    
    // Pattern 1: Profile::merge should be replaced with Profile::merge_with
    let profile_merge_check = Command::new("git")
        .args(&["grep", "-n", "Profile::merge("])
        .output()
        .expect("Failed to run git grep");
        
    if profile_merge_check.status.success() && !profile_merge_check.stdout.is_empty() {
        eprintln!("❌ Found Profile::merge( calls - should be Profile::merge_with:");
        eprintln!("{}", String::from_utf8_lossy(&profile_merge_check.stdout));
        exit(1);
    }
    
    // Pattern 2: BlackboxWriter::new? should not have ? operator if it returns T not Result<T, E>
    let blackbox_writer_check = Command::new("git")
        .args(&["grep", "-n", "BlackboxWriter::new.*?"])
        .output()
        .expect("Failed to run git grep");
        
    if blackbox_writer_check.status.success() && !blackbox_writer_check.stdout.is_empty() {
        eprintln!("❌ Found BlackboxWriter::new with ? operator:");
        eprintln!("{}", String::from_utf8_lossy(&blackbox_writer_check.stdout));
        exit(1);
    }
    
    // Pattern 3: Engine::new should have 2 arguments
    let engine_new_check = Command::new("git")
        .args(&["grep", "-n", "Engine::new("])
        .output()
        .expect("Failed to run git grep");
        
    if engine_new_check.status.success() {
        let output = String::from_utf8_lossy(&engine_new_check.stdout);
        // Check that all Engine::new calls have 2 arguments (name and config)
        for line in output.lines() {
            if !line.contains(",") {
                eprintln!("❌ Found Engine::new with incorrect signature:");
                eprintln!("{}", line);
                exit(1);
            }
        }
    }
    
    // Pattern 4: Check for unaligned references in packed structs
    let packed_ref_check = Command::new("cargo")
        .args(&["clippy", "--workspace", "--", "-W", "clippy::unaligned_references"])
        .output()
        .expect("Failed to run clippy for unaligned references");
        
    if !packed_ref_check.status.success() {
        eprintln!("❌ Found unaligned reference warnings:");
        eprintln!("{}", String::from_utf8_lossy(&packed_ref_check.stderr));
        exit(1);
    }
    
    // Pattern 5: Check for criterion::black_box usage (should be std::hint::black_box)
    let black_box_check = Command::new("git")
        .args(&["grep", "-n", "criterion::black_box"])
        .output()
        .expect("Failed to run git grep");
        
    if black_box_check.status.success() && !black_box_check.stdout.is_empty() {
        eprintln!("❌ Found criterion::black_box usage - should be std::hint::black_box:");
        eprintln!("{}", String::from_utf8_lossy(&black_box_check.stdout));
        exit(1);
    }
    
    println!("✅ All critical patterns verified");
}