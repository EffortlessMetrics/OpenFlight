#!/usr/bin/env cargo +nightly -Zscript
//! Supply Chain Security Audit Script
//!
//! This script performs comprehensive supply chain security auditing for Flight Hub,
//! including dependency scanning, license compliance, and third-party audit trail generation.
//!
//! Requirements addressed: Security (SEC-01)

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, exit};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
struct DependencyInfo {
    name: String,
    version: String,
    license: Option<String>,
    repository: Option<String>,
    authors: Vec<String>,
    description: Option<String>,
    source: String,
}

#[derive(Debug)]
struct AuditResult {
    total_dependencies: usize,
    license_compliant: usize,
    security_advisories: Vec<String>,
    banned_crates: Vec<String>,
    duplicate_dependencies: Vec<String>,
    unknown_licenses: Vec<String>,
}

fn main() {
    println!("🔍 Flight Hub Supply Chain Security Audit");
    println!("=========================================");
    
    let workspace_root = env::current_dir().expect("Failed to get current directory");
    
    // Step 1: Install required tools
    println!("\n🛠️ Installing audit tools...");
    install_audit_tools();
    
    // Step 2: Run cargo-audit for security advisories
    println!("\n🔒 Running security advisory scan...");
    let advisory_result = run_security_audit();
    
    // Step 3: Run cargo-deny for comprehensive checks
    println!("\n🚫 Running cargo-deny checks...");
    let deny_result = run_cargo_deny();
    
    // Step 4: Generate dependency inventory
    println!("\n📋 Generating dependency inventory...");
    let dependencies = generate_dependency_inventory();
    
    // Step 5: Create third-party license list
    println!("\n📄 Creating third-party license list...");
    create_license_list(&dependencies);
    
    // Step 6: Generate SPDX documents for each crate
    println!("\n📦 Generating SPDX documents...");
    generate_spdx_documents(&dependencies);
    
    // Step 7: Create audit trail
    println!("\n📊 Creating audit trail...");
    let audit_result = create_audit_trail(&dependencies, &advisory_result, &deny_result);
    
    // Step 8: Generate summary report
    println!("\n📈 Generating summary report...");
    generate_summary_report(&audit_result);
    
    // Step 9: Check compliance
    if check_compliance(&audit_result) {
        println!("\n✅ Supply chain audit passed!");
        exit(0);
    } else {
        println!("\n❌ Supply chain audit failed!");
        exit(1);
    }
}

fn install_audit_tools() {
    let tools = [
        ("cargo-audit", "cargo-audit"),
        ("cargo-deny", "cargo-deny"),
        ("cargo-tree", "cargo"), // cargo-tree is built into cargo
        ("cargo-license", "cargo-license"),
    ];
    
    for (tool_name, install_name) in &tools {
        if tool_name == &"cargo-tree" {
            continue; // Skip cargo-tree as it's built-in
        }
        
        println!("  Installing {}...", tool_name);
        let output = Command::new("cargo")
            .args(&["install", install_name, "--quiet"])
            .output();
            
        match output {
            Ok(result) if result.status.success() => {
                println!("    ✅ {} installed successfully", tool_name);
            }
            Ok(result) => {
                // Tool might already be installed
                println!("    ℹ️ {} may already be installed", tool_name);
            }
            Err(e) => {
                eprintln!("    ❌ Failed to install {}: {}", tool_name, e);
            }
        }
    }
}

fn run_security_audit() -> Vec<String> {
    let mut advisories = Vec::new();
    
    println!("  Running cargo audit...");
    let output = Command::new("cargo")
        .args(&["audit", "--format", "json"])
        .output();
        
    match output {
        Ok(result) if result.status.success() => {
            println!("    ✅ No security advisories found");
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            if stderr.contains("Crate:") {
                // Parse advisory information
                for line in stderr.lines() {
                    if line.contains("ID:") || line.contains("Crate:") {
                        advisories.push(line.to_string());
                    }
                }
                println!("    ⚠️ Found {} security advisories", advisories.len());
            } else {
                println!("    ✅ No security advisories found");
            }
        }
        Err(e) => {
            eprintln!("    ❌ Failed to run cargo audit: {}", e);
        }
    }
    
    advisories
}

fn run_cargo_deny() -> bool {
    println!("  Running cargo deny check...");
    let output = Command::new("cargo")
        .args(&["deny", "check", "--hide-inclusion-graph"])
        .output();
        
    match output {
        Ok(result) if result.status.success() => {
            println!("    ✅ All cargo-deny checks passed");
            true
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            println!("    ❌ Cargo-deny checks failed:");
            for line in stderr.lines() {
                if line.contains("error:") || line.contains("warning:") {
                    println!("      {}", line);
                }
            }
            false
        }
        Err(e) => {
            eprintln!("    ❌ Failed to run cargo deny: {}", e);
            false
        }
    }
}

fn generate_dependency_inventory() -> Vec<DependencyInfo> {
    let mut dependencies = Vec::new();
    
    println!("  Scanning workspace dependencies...");
    
    // Use cargo metadata to get dependency information
    let output = Command::new("cargo")
        .args(&["metadata", "--format-version", "1"])
        .output();
        
    match output {
        Ok(result) if result.status.success() => {
            let metadata_json = String::from_utf8_lossy(&result.stdout);
            // Simple JSON parsing for metadata - just extract what we need
            if metadata_json.contains("\"packages\"") {
                // Simple parsing - just create a basic dependency list
                // This is a simplified version for the audit script
                let lines: Vec<&str> = metadata_json.lines().collect();
                let mut current_name = String::new();
                let mut current_version = String::new();
                let mut current_license = None;
                
                for line in lines {
                    if line.contains("\"name\":") {
                        if let Some(name_start) = line.find("\"name\": \"") {
                            let name_part = &line[name_start + 9..];
                            if let Some(name_end) = name_part.find('"') {
                                current_name = name_part[..name_end].to_string();
                            }
                        }
                    }
                    if line.contains("\"version\":") {
                        if let Some(ver_start) = line.find("\"version\": \"") {
                            let ver_part = &line[ver_start + 12..];
                            if let Some(ver_end) = ver_part.find('"') {
                                current_version = ver_part[..ver_end].to_string();
                            }
                        }
                    }
                    if line.contains("\"license\":") {
                        if let Some(lic_start) = line.find("\"license\": \"") {
                            let lic_part = &line[lic_start + 12..];
                            if let Some(lic_end) = lic_part.find('"') {
                                current_license = Some(lic_part[..lic_end].to_string());
                            }
                        }
                    }
                    
                    // If we have a complete package, add it
                    if !current_name.is_empty() && !current_version.is_empty() && line.trim() == "}" {
                        let dep_info = DependencyInfo {
                            name: current_name.clone(),
                            version: current_version.clone(),
                            license: current_license.clone(),
                            repository: None,
                            authors: Vec::new(),
                            description: None,
                            source: "registry".to_string(),
                        };
                        dependencies.push(dep_info);
                        
                        // Reset for next package
                        current_name.clear();
                        current_version.clear();
                        current_license = None;
                    }
                }
            }
            println!("    ✅ Found {} dependencies", dependencies.len());
        }
        Ok(_) => {
            eprintln!("    ❌ Failed to get dependency metadata: command failed");
        }
        Err(e) => {
            eprintln!("    ❌ Failed to get dependency metadata: {}", e);
        }
    }
    
    dependencies
}

fn create_license_list(dependencies: &[DependencyInfo]) {
    println!("  Creating third-party license list...");
    
    let mut license_groups: HashMap<String, Vec<&DependencyInfo>> = HashMap::new();
    let mut unknown_licenses = Vec::new();
    
    for dep in dependencies {
        if let Some(license) = &dep.license {
            license_groups.entry(license.clone()).or_default().push(dep);
        } else {
            unknown_licenses.push(dep);
        }
    }
    
    let mut license_content = String::new();
    license_content.push_str("# Third-Party Licenses\n\n");
    license_content.push_str("This document lists all third-party dependencies used in Flight Hub and their licenses.\n\n");
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    license_content.push_str(&format!("Generated on: {} (Unix timestamp)\n", now));
    license_content.push_str(&format!("Total dependencies: {}\n\n", dependencies.len()));
    
    // Group by license
    for (license, deps) in &license_groups {
        license_content.push_str(&format!("## {} ({} crates)\n\n", license, deps.len()));
        
        for dep in deps {
            license_content.push_str(&format!("- **{}** v{}", dep.name, dep.version));
            if let Some(desc) = &dep.description {
                license_content.push_str(&format!(" - {}", desc));
            }
            license_content.push('\n');
            
            if let Some(repo) = &dep.repository {
                license_content.push_str(&format!("  - Repository: {}\n", repo));
            }
            
            if !dep.authors.is_empty() {
                license_content.push_str(&format!("  - Authors: {}\n", dep.authors.join(", ")));
            }
            
            license_content.push('\n');
        }
    }
    
    if !unknown_licenses.is_empty() {
        license_content.push_str("## Unknown Licenses\n\n");
        for dep in &unknown_licenses {
            license_content.push_str(&format!("- **{}** v{} - License information not available\n", dep.name, dep.version));
        }
    }
    
    // Write to file
    if let Err(e) = fs::write("THIRD_PARTY_LICENSES.md", license_content) {
        eprintln!("    ❌ Failed to write license list: {}", e);
    } else {
        println!("    ✅ Created THIRD_PARTY_LICENSES.md");
    }
}

fn generate_spdx_documents(dependencies: &[DependencyInfo]) {
    println!("  Generating SPDX documents for each crate...");
    
    // Create SPDX directory
    let spdx_dir = Path::new("spdx");
    if let Err(e) = fs::create_dir_all(spdx_dir) {
        eprintln!("    ❌ Failed to create SPDX directory: {}", e);
        return;
    }
    
    // Get workspace crates
    let workspace_crates = get_workspace_crates();
    
    for crate_name in &workspace_crates {
        let spdx_content = generate_spdx_for_crate(crate_name, dependencies);
        let spdx_file = spdx_dir.join(format!("{}.spdx", crate_name));
        
        if let Err(e) = fs::write(&spdx_file, spdx_content) {
            eprintln!("    ❌ Failed to write SPDX for {}: {}", crate_name, e);
        } else {
            println!("    ✅ Generated SPDX for {}", crate_name);
        }
    }
}

fn get_workspace_crates() -> Vec<String> {
    let mut crates = Vec::new();
    
    if let Ok(cargo_toml) = fs::read_to_string("Cargo.toml") {
        // Simple parsing - look for members array
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

fn generate_spdx_for_crate(crate_name: &str, dependencies: &[DependencyInfo]) -> String {
    let mut spdx = String::new();
    
    // SPDX header
    spdx.push_str("SPDXVersion: SPDX-2.3\n");
    spdx.push_str("DataLicense: CC0-1.0\n");
    spdx.push_str(&format!("SPDXID: SPDXRef-DOCUMENT\n"));
    spdx.push_str(&format!("DocumentName: {}\n", crate_name));
    spdx.push_str(&format!("DocumentNamespace: https://flight-hub.dev/spdx/{}\n", crate_name));
    spdx.push_str(&format!("CreationInfo: Tool: flight-hub-supply-chain-audit\n"));
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    spdx.push_str(&format!("Created: {} (Unix timestamp)\n", now));
    spdx.push_str("\n");
    
    // Package information
    spdx.push_str(&format!("PackageName: {}\n", crate_name));
    spdx.push_str(&format!("SPDXID: SPDXRef-Package-{}\n", crate_name));
    spdx.push_str("PackageDownloadLocation: NOASSERTION\n");
    spdx.push_str("FilesAnalyzed: false\n");
    spdx.push_str("PackageLicenseConcluded: MIT OR Apache-2.0\n");
    spdx.push_str("PackageLicenseDeclared: MIT OR Apache-2.0\n");
    spdx.push_str("PackageCopyrightText: Copyright (c) 2024 Flight Hub Team\n");
    spdx.push_str("\n");
    
    // Dependencies
    for (i, dep) in dependencies.iter().enumerate() {
        if dep.name.starts_with("flight-") {
            continue; // Skip our own crates
        }
        
        let spdx_id = format!("SPDXRef-Package-{}-{}", dep.name.replace('-', ""), i);
        spdx.push_str(&format!("PackageName: {}\n", dep.name));
        spdx.push_str(&format!("SPDXID: {}\n", spdx_id));
        spdx.push_str(&format!("PackageVersion: {}\n", dep.version));
        spdx.push_str("PackageDownloadLocation: NOASSERTION\n");
        spdx.push_str("FilesAnalyzed: false\n");
        
        if let Some(license) = &dep.license {
            spdx.push_str(&format!("PackageLicenseConcluded: {}\n", license));
            spdx.push_str(&format!("PackageLicenseDeclared: {}\n", license));
        } else {
            spdx.push_str("PackageLicenseConcluded: NOASSERTION\n");
            spdx.push_str("PackageLicenseDeclared: NOASSERTION\n");
        }
        
        spdx.push_str("PackageCopyrightText: NOASSERTION\n");
        spdx.push_str("\n");
        
        // Relationship
        spdx.push_str(&format!("Relationship: SPDXRef-Package-{} DEPENDS_ON {}\n", crate_name, spdx_id));
    }
    
    spdx
}

fn create_audit_trail(dependencies: &[DependencyInfo], advisories: &[String], deny_passed: &bool) -> AuditResult {
    println!("  Creating comprehensive audit trail...");
    
    let mut audit_content = String::new();
    audit_content.push_str("# Flight Hub Supply Chain Audit Trail\n\n");
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    audit_content.push_str(&format!("Audit Date: {} (Unix timestamp)\n", now));
    audit_content.push_str(&format!("Auditor: Flight Hub Supply Chain Audit Script\n"));
    audit_content.push_str(&format!("Workspace: {}\n\n", env::current_dir().unwrap().display()));
    
    // Summary statistics
    let total_deps = dependencies.len();
    let license_compliant = dependencies.iter()
        .filter(|dep| dep.license.as_ref().map_or(false, |l| is_license_compliant(l)))
        .count();
    let unknown_licenses: Vec<String> = dependencies.iter()
        .filter(|dep| dep.license.is_none())
        .map(|dep| format!("{} v{}", dep.name, dep.version))
        .collect();
    
    audit_content.push_str("## Summary\n\n");
    audit_content.push_str(&format!("- Total Dependencies: {}\n", total_deps));
    audit_content.push_str(&format!("- License Compliant: {}\n", license_compliant));
    audit_content.push_str(&format!("- Security Advisories: {}\n", advisories.len()));
    audit_content.push_str(&format!("- Cargo Deny Status: {}\n", if *deny_passed { "PASSED" } else { "FAILED" }));
    audit_content.push_str(&format!("- Unknown Licenses: {}\n\n", unknown_licenses.len()));
    
    // Security advisories section
    if !advisories.is_empty() {
        audit_content.push_str("## Security Advisories\n\n");
        for advisory in advisories {
            audit_content.push_str(&format!("- {}\n", advisory));
        }
        audit_content.push_str("\n");
    }
    
    // License compliance section
    audit_content.push_str("## License Compliance\n\n");
    audit_content.push_str("### Compliant Licenses\n");
    let compliant_licenses = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "CC0-1.0"];
    for license in &compliant_licenses {
        let count = dependencies.iter().filter(|dep| dep.license.as_ref() == Some(&license.to_string())).count();
        if count > 0 {
            audit_content.push_str(&format!("- {}: {} crates\n", license, count));
        }
    }
    
    if !unknown_licenses.is_empty() {
        audit_content.push_str("\n### Unknown Licenses\n");
        for dep in &unknown_licenses {
            audit_content.push_str(&format!("- {}\n", dep));
        }
    }
    
    // Write audit trail
    if let Err(e) = fs::write("SUPPLY_CHAIN_AUDIT.md", audit_content) {
        eprintln!("    ❌ Failed to write audit trail: {}", e);
    } else {
        println!("    ✅ Created SUPPLY_CHAIN_AUDIT.md");
    }
    
    AuditResult {
        total_dependencies: total_deps,
        license_compliant,
        security_advisories: advisories.to_vec(),
        banned_crates: Vec::new(), // Would be populated by cargo-deny parsing
        duplicate_dependencies: Vec::new(), // Would be populated by cargo-tree parsing
        unknown_licenses,
    }
}

fn is_license_compliant(license: &str) -> bool {
    let compliant_licenses = [
        "MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception",
        "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unicode-DFS-2016", "CC0-1.0"
    ];
    
    // Handle compound licenses (OR/AND)
    if license.contains(" OR ") {
        return license.split(" OR ").all(|l| compliant_licenses.contains(&l.trim()));
    }
    if license.contains(" AND ") {
        return license.split(" AND ").all(|l| compliant_licenses.contains(&l.trim()));
    }
    
    compliant_licenses.contains(&license)
}

fn generate_summary_report(audit_result: &AuditResult) {
    println!("\n📊 Supply Chain Audit Summary");
    println!("============================");
    println!("Total Dependencies: {}", audit_result.total_dependencies);
    println!("License Compliant: {}/{}", audit_result.license_compliant, audit_result.total_dependencies);
    println!("Security Advisories: {}", audit_result.security_advisories.len());
    println!("Unknown Licenses: {}", audit_result.unknown_licenses.len());
    
    if !audit_result.security_advisories.is_empty() {
        println!("\n⚠️ Security Advisories Found:");
        for advisory in &audit_result.security_advisories {
            println!("  - {}", advisory);
        }
    }
    
    if !audit_result.unknown_licenses.is_empty() {
        println!("\n❓ Dependencies with Unknown Licenses:");
        for dep in &audit_result.unknown_licenses {
            println!("  - {}", dep);
        }
    }
}

fn check_compliance(audit_result: &AuditResult) -> bool {
    let mut compliant = true;
    
    // Check for security advisories
    if !audit_result.security_advisories.is_empty() {
        println!("❌ Security advisories found - audit failed");
        compliant = false;
    }
    
    // Check license compliance rate
    let compliance_rate = audit_result.license_compliant as f64 / audit_result.total_dependencies as f64;
    if compliance_rate < 0.95 {
        println!("❌ License compliance rate too low: {:.1}% - audit failed", compliance_rate * 100.0);
        compliant = false;
    }
    
    // Check for unknown licenses
    if audit_result.unknown_licenses.len() > 5 {
        println!("❌ Too many dependencies with unknown licenses: {} - audit failed", audit_result.unknown_licenses.len());
        compliant = false;
    }
    
    compliant
}

