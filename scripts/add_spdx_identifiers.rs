#!/usr/bin/env cargo +nightly -Zscript
//! Add SPDX License Identifiers to All Crates
//!
//! This script adds SPDX license identifiers to all crate Cargo.toml files
//! in the Flight Hub workspace, ensuring compliance with supply chain requirements.

use std::fs;
use std::path::Path;

fn main() {
    println!("📋 Adding SPDX License Identifiers to Flight Hub Crates");
    println!("======================================================");
    
    let workspace_crates = get_workspace_crates();
    let mut updated_count = 0;
    
    for crate_name in &workspace_crates {
        let cargo_toml_path = format!("crates/{}/Cargo.toml", crate_name);
        
        if Path::new(&cargo_toml_path).exists() {
            if add_spdx_to_crate(&cargo_toml_path, crate_name) {
                println!("  ✅ Updated {}", crate_name);
                updated_count += 1;
            } else {
                println!("  ℹ️ {} already has SPDX identifiers", crate_name);
            }
        } else {
            println!("  ❌ Cargo.toml not found for {}", crate_name);
        }
    }
    
    println!("\n📊 Summary: Updated {} out of {} crates", updated_count, workspace_crates.len());
    
    // Also add SPDX to source files
    println!("\n📝 Adding SPDX headers to source files...");
    add_spdx_to_source_files(&workspace_crates);
    
    println!("\n✅ SPDX identifier addition completed!");
}

fn get_workspace_crates() -> Vec<String> {
    let mut crates = Vec::new();
    
    if let Ok(cargo_toml) = fs::read_to_string("Cargo.toml") {
        let lines: Vec<&str> = cargo_toml.lines().collect();
        let mut in_members = false;
        
        for line in lines {
            let trimmed = line.trim();
            if trimmed.starts_with("members = [") {
                in_members = true;
                continue;
            }
            if in_members {
                if trimmed == "]" {
                    break;
                }
                if let Some(crate_path) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix("\",")) {
                    if let Some(crate_name) = crate_path.strip_prefix("crates/") {
                        crates.push(crate_name.to_string());
                    }
                }
            }
        }
    }
    
    crates
}

fn add_spdx_to_crate(cargo_toml_path: &str, crate_name: &str) -> bool {
    let content = match fs::read_to_string(cargo_toml_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("    ❌ Failed to read {}: {}", cargo_toml_path, e);
            return false;
        }
    };
    
    // Check if SPDX identifiers already exist
    if content.contains("SPDX-License-Identifier") {
        return false; // Already has SPDX identifiers
    }
    
    let lines: Vec<&str> = content.lines().collect();
    let mut new_content = String::new();
    let mut added_spdx = false;
    
    for line in lines {
        new_content.push_str(line);
        new_content.push('\n');
        
        // Add SPDX identifiers after the repository line
        if line.starts_with("repository.workspace = true") && !added_spdx {
            new_content.push('\n');
            new_content.push_str("# SPDX License Identifier\n");
            new_content.push_str("# SPDX-License-Identifier: MIT OR Apache-2.0\n");
            new_content.push_str("# SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team\n");
            added_spdx = true;
        }
    }
    
    // Write updated content
    match fs::write(cargo_toml_path, new_content) {
        Ok(_) => true,
        Err(e) => {
            eprintln!("    ❌ Failed to write {}: {}", cargo_toml_path, e);
            false
        }
    }
}

fn add_spdx_to_source_files(workspace_crates: &[String]) {
    let spdx_header = "// SPDX-License-Identifier: MIT OR Apache-2.0\n// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team\n\n";
    
    for crate_name in workspace_crates {
        let src_dir = format!("crates/{}/src", crate_name);
        if Path::new(&src_dir).exists() {
            add_spdx_to_rust_files(&src_dir, spdx_header);
        }
    }
    
    // Also add to examples
    if Path::new("examples").exists() {
        add_spdx_to_rust_files("examples", spdx_header);
    }
    
    // Add to scripts
    if Path::new("scripts").exists() {
        add_spdx_to_rust_files("scripts", spdx_header);
    }
}

fn add_spdx_to_rust_files(dir: &str, spdx_header: &str) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
                add_spdx_to_rust_file(&path, spdx_header);
            } else if path.is_dir() {
                // Recursively process subdirectories
                if let Some(path_str) = path.to_str() {
                    add_spdx_to_rust_files(path_str, spdx_header);
                }
            }
        }
    }
}

fn add_spdx_to_rust_file(file_path: &Path, spdx_header: &str) {
    let content = match fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(_) => return, // Skip files we can't read
    };
    
    // Skip if already has SPDX identifier
    if content.contains("SPDX-License-Identifier") {
        return;
    }
    
    // Skip if it's a script with shebang
    if content.starts_with("#!/usr/bin/env cargo") {
        return;
    }
    
    let new_content = format!("{}{}", spdx_header, content);
    
    if let Err(e) = fs::write(file_path, new_content) {
        eprintln!("    ❌ Failed to update {}: {}", file_path.display(), e);
    }
}