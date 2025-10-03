#!/usr/bin/env cargo +nightly -Zscript
//! Validates that all gRPC crates use identical 0.14.x versions
//! 
//! This script ensures version alignment across the gRPC stack:
//! - prost, prost-build, prost-types
//! - tonic, tonic-build
//! 
//! Requirements: SC-03.1, SC-03.2, SC-03.5

use std::collections::HashMap;
use std::process::{Command, exit};

fn main() {
    println!("🔍 Validating gRPC version alignment...");
    
    let grpc_crates = vec![
        "prost",
        "prost-build", 
        "prost-types",
        "tonic",
        "tonic-build"
    ];
    
    let mut versions = HashMap::new();
    
    // Get cargo tree output to check actual resolved versions
    let output = Command::new("cargo")
        .args(&["tree", "--format", "{p}"])
        .output()
        .expect("Failed to execute cargo tree");
    
    if !output.status.success() {
        eprintln!("❌ Failed to run cargo tree");
        exit(1);
    }
    
    let tree_output = String::from_utf8_lossy(&output.stdout);
    
    // Parse versions from cargo tree output
    for line in tree_output.lines() {
        for crate_name in &grpc_crates {
            if line.starts_with(&format!("{} v", crate_name)) {
                let version = line.split(' ').nth(1)
                    .and_then(|v| v.strip_prefix('v'))
                    .unwrap_or("unknown");
                versions.insert(crate_name.to_string(), version.to_string());
            }
        }
    }
    
    println!("📦 Found gRPC crate versions:");
    for (crate_name, version) in &versions {
        println!("  {} = {}", crate_name, version);
    }
    
    // Validate all versions are 0.14.x
    let mut all_valid = true;
    for (crate_name, version) in &versions {
        if !version.starts_with("0.14.") {
            eprintln!("❌ {} version {} is not 0.14.x", crate_name, version);
            all_valid = false;
        }
    }
    
    // Check that prost family uses same minor version
    let prost_versions: Vec<_> = versions.iter()
        .filter(|(name, _)| name.starts_with("prost"))
        .map(|(_, version)| version)
        .collect();
    
    if prost_versions.len() > 1 {
        let first_version = prost_versions[0];
        for version in &prost_versions[1..] {
            if version != &first_version {
                eprintln!("❌ prost family versions not aligned: {} vs {}", first_version, version);
                all_valid = false;
            }
        }
    }
    
    // Check that tonic family uses same minor version  
    let tonic_versions: Vec<_> = versions.iter()
        .filter(|(name, _)| name.starts_with("tonic"))
        .map(|(_, version)| version)
        .collect();
    
    if tonic_versions.len() > 1 {
        let first_version = tonic_versions[0];
        for version in &tonic_versions[1..] {
            if version != &first_version {
                eprintln!("❌ tonic family versions not aligned: {} vs {}", first_version, version);
                all_valid = false;
            }
        }
    }
    
    if all_valid {
        println!("✅ All gRPC crates use aligned 0.14.x versions");
    } else {
        eprintln!("❌ gRPC version alignment validation failed");
        exit(1);
    }
}