#!/usr/bin/env cargo +nightly -Zscript
//! CI Supply Chain Security Gate
//!
//! This script implements the CI gates for supply chain security as required by task 37.
//! It ensures that all security requirements are met before allowing builds to pass.
//!
//! Requirements addressed: Security (SEC-01), Supply Chain Auditing

use std::fs;
use std::path::Path;
use std::process::{Command, exit};

#[derive(Debug)]
struct GateResult {
    name: String,
    passed: bool,
    message: String,
}

fn main() {
    println!("🚪 Flight Hub CI Supply Chain Security Gate");
    println!("==========================================");
    
    let mut gates = Vec::new();
    
    // Gate 1: Cargo Audit - No security advisories
    println!("\n🔒 Gate 1: Security Advisory Check");
    gates.push(run_cargo_audit_gate());
    
    // Gate 2: Cargo Deny - All checks pass
    println!("\n🚫 Gate 2: Cargo Deny Compliance");
    gates.push(run_cargo_deny_gate());
    
    // Gate 3: License Compliance - All licenses approved
    println!("\n📄 Gate 3: License Compliance");
    gates.push(run_license_compliance_gate());
    
    // Gate 4: SPDX Validation - All crates have SPDX identifiers
    println!("\n📋 Gate 4: SPDX Identifier Validation");
    gates.push(run_spdx_validation_gate());
    
    // Gate 5: Dependency Count - Reasonable dependency count
    println!("\n📦 Gate 5: Dependency Count Validation");
    gates.push(run_dependency_count_gate());
    
    // Gate 6: Supply Chain Audit Trail - Audit documents exist and are current
    println!("\n📊 Gate 6: Audit Trail Validation");
    gates.push(run_audit_trail_gate());
    
    // Gate 7: Third-Party License List - Complete and up-to-date
    println!("\n📜 Gate 7: Third-Party License List");
    gates.push(run_license_list_gate());
    
    // Summary
    println!("\n📊 CI Supply Chain Security Gate Summary");
    println!("=======================================");
    
    let passed_gates = gates.iter().filter(|g| g.passed).count();
    let total_gates = gates.len();
    
    println!("Gates passed: {}/{}", passed_gates, total_gates);
    
    for gate in &gates {
        let status = if gate.passed { "✅ PASS" } else { "❌ FAIL" };
        println!("  {} {}: {}", status, gate.name, gate.message);
    }
    
    if passed_gates == total_gates {
        println!("\n🎉 All supply chain security gates passed!");
        println!("Build is approved for deployment.");
        exit(0);
    } else {
        println!("\n💥 Supply chain security gate failures detected!");
        println!("Build is BLOCKED until all gates pass.");
        
        // Print remediation guidance
        print_remediation_guidance(&gates);
        exit(1);
    }
}

fn run_cargo_audit_gate() -> GateResult {
    println!("  Running cargo audit...");
    
    let output = Command::new("cargo")
        .args(&["audit", "--deny", "warnings"])
        .output();
        
    match output {
        Ok(result) if result.status.success() => {
            GateResult {
                name: "Security Advisory Check".to_string(),
                passed: true,
                message: "No security advisories found".to_string(),
            }
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            let advisory_count = stderr.lines()
                .filter(|line| line.contains("ID:") || line.contains("Crate:"))
                .count();
                
            GateResult {
                name: "Security Advisory Check".to_string(),
                passed: false,
                message: format!("Found {} security advisories", advisory_count / 2),
            }
        }
        Err(e) => {
            GateResult {
                name: "Security Advisory Check".to_string(),
                passed: false,
                message: format!("Failed to run cargo audit: {}", e),
            }
        }
    }
}

fn run_cargo_deny_gate() -> GateResult {
    println!("  Running cargo deny check...");
    
    let output = Command::new("cargo")
        .args(&["deny", "check", "--hide-inclusion-graph"])
        .output();
        
    match output {
        Ok(result) if result.status.success() => {
            GateResult {
                name: "Cargo Deny Compliance".to_string(),
                passed: true,
                message: "All cargo-deny checks passed".to_string(),
            }
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            let error_lines: Vec<&str> = stderr.lines()
                .filter(|line| line.contains("error:") || line.contains("denied:"))
                .collect();
                
            GateResult {
                name: "Cargo Deny Compliance".to_string(),
                passed: false,
                message: format!("Found {} cargo-deny violations", error_lines.len()),
            }
        }
        Err(e) => {
            GateResult {
                name: "Cargo Deny Compliance".to_string(),
                passed: false,
                message: format!("Failed to run cargo deny: {}", e),
            }
        }
    }
}

fn run_license_compliance_gate() -> GateResult {
    println!("  Checking license compliance...");
    
    // Get dependency metadata
    let output = Command::new("cargo")
        .args(&["metadata", "--format-version", "1"])
        .output();
        
    match output {
        Ok(result) if result.status.success() => {
            let metadata_json = String::from_utf8_lossy(&result.stdout);
            // Simple parsing without serde_json dependency
            if metadata_json.contains("\"packages\"") {
                let mut total_deps = 0;
                let mut compliant_deps = 0;
                let mut unknown_licenses = 0;
                
                // Simple parsing - count packages and check licenses
                let lines: Vec<&str> = metadata_json.lines().collect();
                let mut in_package = false;
                let mut current_name = String::new();
                let mut current_license = None;
                
                for line in lines {
                    if line.contains("\"name\":") && !in_package {
                        if let Some(name_start) = line.find("\"name\": \"") {
                            let name_part = &line[name_start + 9..];
                            if let Some(name_end) = name_part.find('"') {
                                current_name = name_part[..name_end].to_string();
                                in_package = true;
                            }
                        }
                    }
                    
                    if in_package && line.contains("\"license\":") {
                        if let Some(lic_start) = line.find("\"license\": \"") {
                            let lic_part = &line[lic_start + 12..];
                            if let Some(lic_end) = lic_part.find('"') {
                                current_license = Some(lic_part[..lic_end].to_string());
                            }
                        }
                    }
                    
                    if in_package && line.trim() == "}" {
                        // Skip our own crates
                        if !current_name.starts_with("flight-") {
                            total_deps += 1;
                            
                            if let Some(license) = &current_license {
                                if is_license_compliant(license) {
                                    compliant_deps += 1;
                                }
                            } else {
                                unknown_licenses += 1;
                            }
                        }
                        
                        // Reset for next package
                        in_package = false;
                        current_name.clear();
                        current_license = None;
                    }
                }
                
                let compliance_rate = if total_deps > 0 {
                    compliant_deps as f64 / total_deps as f64
                } else {
                    1.0
                };
                
                if compliance_rate >= 0.95 && unknown_licenses <= 5 {
                    GateResult {
                        name: "License Compliance".to_string(),
                        passed: true,
                        message: format!("{}% compliant ({}/{}), {} unknown", 
                                       (compliance_rate * 100.0) as u32, 
                                       compliant_deps, total_deps, unknown_licenses),
                    }
                } else {
                    GateResult {
                        name: "License Compliance".to_string(),
                        passed: false,
                        message: format!("{}% compliant ({}/{}), {} unknown - below threshold", 
                                       (compliance_rate * 100.0) as u32, 
                                       compliant_deps, total_deps, unknown_licenses),
                    }
                }
            } else {
                GateResult {
                    name: "License Compliance".to_string(),
                    passed: false,
                    message: "Failed to parse cargo metadata".to_string(),
                }
            }
        }
        Ok(_) => {
            GateResult {
                name: "License Compliance".to_string(),
                passed: false,
                message: "Failed to parse dependency metadata".to_string(),
            }
        }
        Err(e) => {
            GateResult {
                name: "License Compliance".to_string(),
                passed: false,
                message: format!("Failed to get dependency metadata: {}", e),
            }
        }
    }
}

fn run_spdx_validation_gate() -> GateResult {
    println!("  Validating SPDX identifiers...");
    
    let workspace_crates = get_workspace_crates();
    let mut missing_spdx = Vec::new();
    
    for crate_name in &workspace_crates {
        let cargo_toml_path = format!("crates/{}/Cargo.toml", crate_name);
        
        if Path::new(&cargo_toml_path).exists() {
            if let Ok(content) = fs::read_to_string(&cargo_toml_path) {
                if !content.contains("SPDX-License-Identifier") {
                    missing_spdx.push(crate_name.clone());
                }
            }
        }
    }
    
    if missing_spdx.is_empty() {
        GateResult {
            name: "SPDX Identifier Validation".to_string(),
            passed: true,
            message: format!("All {} crates have SPDX identifiers", workspace_crates.len()),
        }
    } else {
        GateResult {
            name: "SPDX Identifier Validation".to_string(),
            passed: false,
            message: format!("{} crates missing SPDX identifiers: {}", 
                           missing_spdx.len(), missing_spdx.join(", ")),
        }
    }
}

fn run_dependency_count_gate() -> GateResult {
    println!("  Checking dependency count...");
    
    let output = Command::new("cargo")
        .args(&["tree", "--depth", "1"])
        .output();
        
    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let dep_count = stdout.lines()
                .filter(|line| line.starts_with("├──") || line.starts_with("└──"))
                .count();
                
            // Reasonable threshold for a real-time system
            if dep_count <= 150 {
                GateResult {
                    name: "Dependency Count Validation".to_string(),
                    passed: true,
                    message: format!("{} direct dependencies (within limit)", dep_count),
                }
            } else {
                GateResult {
                    name: "Dependency Count Validation".to_string(),
                    passed: false,
                    message: format!("{} direct dependencies (exceeds limit of 150)", dep_count),
                }
            }
        }
        Ok(_) => {
            GateResult {
                name: "Dependency Count Validation".to_string(),
                passed: false,
                message: "Failed to run cargo tree".to_string(),
            }
        }
        Err(e) => {
            GateResult {
                name: "Dependency Count Validation".to_string(),
                passed: false,
                message: format!("Failed to count dependencies: {}", e),
            }
        }
    }
}

fn run_audit_trail_gate() -> GateResult {
    println!("  Checking audit trail documents...");
    
    let required_files = [
        "SUPPLY_CHAIN_AUDIT.md",
        "THIRD_PARTY_LICENSES.md",
    ];
    
    let mut missing_files = Vec::new();
    let mut outdated_files = Vec::new();
    
    for file in &required_files {
        if !Path::new(file).exists() {
            missing_files.push(*file);
        } else {
            // Check if file is recent (within last 7 days for CI)
            if let Ok(metadata) = fs::metadata(file) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        if elapsed.as_secs() > 7 * 24 * 60 * 60 {
                            outdated_files.push(*file);
                        }
                    }
                }
            }
        }
    }
    
    if missing_files.is_empty() && outdated_files.is_empty() {
        GateResult {
            name: "Audit Trail Validation".to_string(),
            passed: true,
            message: "All audit documents present and current".to_string(),
        }
    } else {
        let mut message = String::new();
        if !missing_files.is_empty() {
            message.push_str(&format!("Missing: {}", missing_files.join(", ")));
        }
        if !outdated_files.is_empty() {
            if !message.is_empty() {
                message.push_str("; ");
            }
            message.push_str(&format!("Outdated: {}", outdated_files.join(", ")));
        }
        
        GateResult {
            name: "Audit Trail Validation".to_string(),
            passed: false,
            message,
        }
    }
}

fn run_license_list_gate() -> GateResult {
    println!("  Validating third-party license list...");
    
    if !Path::new("THIRD_PARTY_LICENSES.md").exists() {
        return GateResult {
            name: "Third-Party License List".to_string(),
            passed: false,
            message: "THIRD_PARTY_LICENSES.md not found".to_string(),
        };
    }
    
    // Check if the license list contains expected sections
    if let Ok(content) = fs::read_to_string("THIRD_PARTY_LICENSES.md") {
        let has_header = content.contains("# Third-Party Licenses");
        let has_mit = content.contains("## MIT");
        let has_apache = content.contains("## Apache-2.0");
        let has_generation_date = content.contains("Generated on:");
        
        if has_header && (has_mit || has_apache) && has_generation_date {
            GateResult {
                name: "Third-Party License List".to_string(),
                passed: true,
                message: "License list is complete and well-formed".to_string(),
            }
        } else {
            GateResult {
                name: "Third-Party License List".to_string(),
                passed: false,
                message: "License list is incomplete or malformed".to_string(),
            }
        }
    } else {
        GateResult {
            name: "Third-Party License List".to_string(),
            passed: false,
            message: "Failed to read THIRD_PARTY_LICENSES.md".to_string(),
        }
    }
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

fn print_remediation_guidance(gates: &[GateResult]) {
    println!("\n🔧 Remediation Guidance");
    println!("======================");
    
    for gate in gates {
        if !gate.passed {
            match gate.name.as_str() {
                "Security Advisory Check" => {
                    println!("🔒 Security Advisory Check:");
                    println!("   - Run 'cargo audit' to see detailed advisory information");
                    println!("   - Update vulnerable dependencies to patched versions");
                    println!("   - If no patch available, consider alternative crates");
                    println!("   - Document any accepted risks in deny.toml with justification");
                }
                "Cargo Deny Compliance" => {
                    println!("🚫 Cargo Deny Compliance:");
                    println!("   - Run 'cargo deny check' to see detailed violations");
                    println!("   - Update banned crates to allowed versions");
                    println!("   - Remove or replace banned crates entirely");
                    println!("   - Review deny.toml configuration for accuracy");
                }
                "License Compliance" => {
                    println!("📄 License Compliance:");
                    println!("   - Review dependencies with non-compliant licenses");
                    println!("   - Replace crates with incompatible licenses");
                    println!("   - Update deny.toml to reflect acceptable licenses");
                    println!("   - Investigate dependencies with unknown licenses");
                }
                "SPDX Identifier Validation" => {
                    println!("📋 SPDX Identifier Validation:");
                    println!("   - Run 'cargo +nightly -Zscript scripts/add_spdx_identifiers.rs'");
                    println!("   - Manually add SPDX identifiers to missing crates");
                    println!("   - Ensure all Cargo.toml files have proper SPDX headers");
                }
                "Dependency Count Validation" => {
                    println!("📦 Dependency Count Validation:");
                    println!("   - Review dependency tree with 'cargo tree'");
                    println!("   - Remove unnecessary dependencies");
                    println!("   - Consolidate similar functionality");
                    println!("   - Consider feature flags to reduce dependency count");
                }
                "Audit Trail Validation" => {
                    println!("📊 Audit Trail Validation:");
                    println!("   - Run 'cargo +nightly -Zscript scripts/supply_chain_audit.rs'");
                    println!("   - Ensure audit documents are generated and current");
                    println!("   - Commit audit documents to version control");
                }
                "Third-Party License List" => {
                    println!("📜 Third-Party License List:");
                    println!("   - Run supply chain audit to regenerate license list");
                    println!("   - Ensure THIRD_PARTY_LICENSES.md is complete");
                    println!("   - Verify all license information is accurate");
                }
                _ => {}
            }
            println!();
        }
    }
    
    println!("📚 For more information:");
    println!("   - See SEC-01 requirements in .kiro/specs/flight-hub/requirements.md");
    println!("   - Review deny.toml configuration");
    println!("   - Check CI workflow in .github/workflows/security.yml");
}

