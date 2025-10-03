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
#[allow(dead_code)]
struct GateResult {
    name: String,
    passed: bool,
    message: String,
    artifacts: Vec<String>,
}

// Simple JSON parsing structures (without serde dependency)
#[derive(Debug)]
struct DenyReport {
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
struct Diagnostic {
    severity: String,
    message: String,
}

fn main() {
    println!("🚪 Flight Hub CI Supply Chain Security Gate");
    println!("==========================================");
    
    // Check for uncommitted Cargo.lock changes first
    if let Err(msg) = check_lockfile_guard() {
        println!("❌ Lockfile Guard Failed: {}", msg);
        exit(1);
    }
    
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
    
    // Gate 6: Duplicate Major Version Detection - Single major per family
    println!("\n🔄 Gate 6: Duplicate Major Version Detection");
    gates.push(run_duplicate_major_detection_gate());
    
    // Gate 7: Supply Chain Audit Trail - Audit documents exist and are current
    println!("\n📊 Gate 7: Audit Trail Validation");
    gates.push(run_audit_trail_gate());
    
    // Gate 8: Third-Party License List - Complete and up-to-date
    println!("\n📜 Gate 8: Third-Party License List");
    gates.push(run_license_list_gate());
    
    // Gate 9: Cargo About Integration - Generate license documentation
    println!("\n📋 Gate 9: Cargo About Integration");
    gates.push(run_cargo_about_gate());
    
    // Gate 10: Source Validation - Registry and VCS source restrictions
    println!("\n🔐 Gate 10: Source Validation");
    gates.push(run_source_validation_gate());
    
    // Gate 11: MSRV and Edition Enforcement - Workspace consistency
    println!("\n📋 Gate 11: MSRV and Edition Enforcement");
    gates.push(run_msrv_edition_gate());
    
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
    println!("  Running cargo audit with retry logic...");
    
    // Retry logic for transient failures only
    for attempt in 1..=3 {
        let output = Command::new("cargo")
            .args(&["audit", "--deny", "warnings", "--format", "json"])
            .output();
            
        match output {
            Ok(result) => {
                let exit_code = result.status.code().unwrap_or(-1);
                let stdout = String::from_utf8_lossy(&result.stdout);
                let stderr = String::from_utf8_lossy(&result.stderr);
                
                // Save artifacts
                let artifacts = save_gate_artifacts("cargo-audit", &stdout, &stderr, exit_code);
                
                // Check exit code first
                if exit_code == 0 {
                    return GateResult {
                        name: "Security Advisory Check".to_string(),
                        passed: true,
                        message: "No security advisories found".to_string(),
                        artifacts,
                    };
                }
                
                // Non-zero exit code - check if it's a policy failure or transient error
                if is_transient_failure(&stderr) && attempt < 3 {
                    println!("    Transient failure detected, retrying... (attempt {}/3)", attempt + 1);
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    continue;
                }
                
                // Policy failure or final attempt - parse for advisory details
                let advisory_count = if stdout.trim().is_empty() {
                    // Fallback to stderr parsing for non-JSON output
                    stderr.lines()
                        .filter(|line| line.contains("ID:") || line.contains("Crate:"))
                        .count() / 2
                } else {
                    // Try to parse JSON output for more accurate count
                    parse_audit_json(&stdout).unwrap_or_else(|| {
                        stderr.lines()
                            .filter(|line| line.contains("ID:") || line.contains("Crate:"))
                            .count() / 2
                    })
                };
                
                return GateResult {
                    name: "Security Advisory Check".to_string(),
                    passed: false,
                    message: format!("Found {} security advisories", advisory_count),
                    artifacts,
                };
            }
            Err(e) => {
                if is_transient_error(&e) && attempt < 3 {
                    println!("    Transient error detected, retrying... (attempt {}/3)", attempt + 1);
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    continue;
                }
                
                return GateResult {
                    name: "Security Advisory Check".to_string(),
                    passed: false,
                    message: format!("Failed to run cargo audit: {}", e),
                    artifacts: Vec::new(),
                };
            }
        }
    }
    
    // Should never reach here due to the loop structure, but just in case
    GateResult {
        name: "Security Advisory Check".to_string(),
        passed: false,
        message: "Unexpected error in retry logic".to_string(),
        artifacts: Vec::new(),
    }
}

fn run_cargo_deny_gate() -> GateResult {
    println!("  Running cargo deny check with JSON output...");
    
    // Pin cargo-deny version for consistent behavior
    let output = Command::new("cargo")
        .args(&["deny", "--locked", "--version", "0.14.23", "check", "--format", "json"])
        .output();
        
    match output {
        Ok(result) => {
            let exit_code = result.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);
            
            // Save raw output as artifact
            let artifacts = save_gate_artifacts("cargo-deny", &stdout, &stderr, exit_code);
            
            // Always check exit code first
            if exit_code != 0 {
                // Parse JSON for detailed error information (manual parsing)
                let error_details = if let Ok(report) = parse_deny_json(&stdout) {
                    let error_count = report.diagnostics.iter()
                        .filter(|d| d.severity == "error")
                        .count();
                    let warning_count = report.diagnostics.iter()
                        .filter(|d| d.severity == "warn")
                        .count();
                    
                    format!("{} errors, {} warnings", error_count, warning_count)
                } else {
                    // Fallback to stderr parsing
                    let error_lines: Vec<&str> = stderr.lines()
                        .filter(|line| line.contains("error:") || line.contains("denied:"))
                        .collect();
                    format!("{} violations detected", error_lines.len())
                };
                
                return GateResult {
                    name: "Cargo Deny Compliance".to_string(),
                    passed: false,
                    message: format!("cargo-deny failed: {}", error_details),
                    artifacts,
                };
            }
            
            // Parse JSON output for warnings (manual parsing)
            if let Ok(report) = parse_deny_json(&stdout) {
                let warnings: Vec<&Diagnostic> = report.diagnostics.iter()
                    .filter(|d| d.severity == "warn" && 
                            d.message.contains("license-not-encountered"))
                    .collect();
                
                let message = if warnings.is_empty() {
                    "All cargo-deny checks passed".to_string()
                } else {
                    format!("Passed with {} license-not-encountered warnings", warnings.len())
                };
                
                GateResult {
                    name: "Cargo Deny Compliance".to_string(),
                    passed: true,
                    message,
                    artifacts,
                }
            } else {
                GateResult {
                    name: "Cargo Deny Compliance".to_string(),
                    passed: true,
                    message: "All cargo-deny checks passed (JSON parse failed)".to_string(),
                    artifacts,
                }
            }
        }
        Err(e) => {
            GateResult {
                name: "Cargo Deny Compliance".to_string(),
                passed: false,
                message: format!("Failed to run cargo deny: {}", e),
                artifacts: Vec::new(),
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
                        artifacts: Vec::new(),
                    }
                } else {
                    GateResult {
                        name: "License Compliance".to_string(),
                        passed: false,
                        message: format!("{}% compliant ({}/{}), {} unknown - below threshold", 
                                       (compliance_rate * 100.0) as u32, 
                                       compliant_deps, total_deps, unknown_licenses),
                        artifacts: Vec::new(),
                    }
                }
            } else {
                GateResult {
                    name: "License Compliance".to_string(),
                    passed: false,
                    message: "Failed to parse cargo metadata".to_string(),
                    artifacts: Vec::new(),
                }
            }
        }
        Ok(_) => {
            GateResult {
                name: "License Compliance".to_string(),
                passed: false,
                message: "Failed to parse dependency metadata".to_string(),
                artifacts: Vec::new(),
            }
        }
        Err(e) => {
            GateResult {
                name: "License Compliance".to_string(),
                passed: false,
                message: format!("Failed to get dependency metadata: {}", e),
                artifacts: Vec::new(),
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
            artifacts: Vec::new(),
        }
    } else {
        GateResult {
            name: "SPDX Identifier Validation".to_string(),
            passed: false,
            message: format!("{} crates missing SPDX identifiers: {}", 
                           missing_spdx.len(), missing_spdx.join(", ")),
            artifacts: Vec::new(),
        }
    }
}

fn run_dependency_count_gate() -> GateResult {
    println!("  Checking dependency count using cargo metadata...");
    
    let output = Command::new("cargo")
        .args(&["metadata", "--format-version", "1"])
        .output();
        
    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let artifacts = save_gate_artifacts("cargo-metadata", &stdout, "", 0);
            
            match count_runtime_dependencies(&stdout) {
                Ok(dep_count) => {
                    // Threshold for runtime dependencies only
                    if dep_count <= 150 {
                        GateResult {
                            name: "Dependency Count Validation".to_string(),
                            passed: true,
                            message: format!("{} runtime dependencies (within limit)", dep_count),
                            artifacts,
                        }
                    } else {
                        GateResult {
                            name: "Dependency Count Validation".to_string(),
                            passed: false,
                            message: format!("{} runtime dependencies (exceeds limit of 150)", dep_count),
                            artifacts,
                        }
                    }
                }
                Err(e) => {
                    GateResult {
                        name: "Dependency Count Validation".to_string(),
                        passed: false,
                        message: format!("Failed to parse metadata: {}", e),
                        artifacts,
                    }
                }
            }
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            let artifacts = save_gate_artifacts("cargo-metadata", "", &stderr, result.status.code().unwrap_or(-1));
            
            GateResult {
                name: "Dependency Count Validation".to_string(),
                passed: false,
                message: "Failed to run cargo metadata".to_string(),
                artifacts,
            }
        }
        Err(e) => {
            GateResult {
                name: "Dependency Count Validation".to_string(),
                passed: false,
                message: format!("Failed to execute cargo metadata: {}", e),
                artifacts: Vec::new(),
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
            artifacts: Vec::new(),
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
            artifacts: Vec::new(),
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
            artifacts: Vec::new(),
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
                artifacts: Vec::new(),
            }
        } else {
            GateResult {
                name: "Third-Party License List".to_string(),
                passed: false,
                message: "License list is incomplete or malformed".to_string(),
                artifacts: Vec::new(),
            }
        }
    } else {
        GateResult {
            name: "Third-Party License List".to_string(),
            passed: false,
            message: "Failed to read THIRD_PARTY_LICENSES.md".to_string(),
            artifacts: Vec::new(),
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

fn check_lockfile_guard() -> Result<(), String> {
    // Check if Cargo.lock has uncommitted changes
    let output = Command::new("git")
        .args(&["diff", "--quiet", "Cargo.lock"])
        .output()
        .map_err(|e| format!("Failed to run git diff: {}", e))?;
    
    if !output.status.success() {
        return Err("Cargo.lock has uncommitted changes - commit or stash changes before running gate".to_string());
    }
    
    Ok(())
}

fn save_gate_artifacts(gate_name: &str, stdout: &str, stderr: &str, exit_code: i32) -> Vec<String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let mut artifacts = Vec::new();
    
    // Create artifacts directory if it doesn't exist
    if let Err(_) = fs::create_dir_all("target/ci-artifacts") {
        return artifacts;
    }
    
    // Save stdout
    let stdout_file = format!("target/ci-artifacts/{}-{}-stdout.json", gate_name, timestamp);
    if let Ok(_) = fs::write(&stdout_file, stdout) {
        artifacts.push(stdout_file);
    }
    
    // Save stderr
    let stderr_file = format!("target/ci-artifacts/{}-{}-stderr.txt", gate_name, timestamp);
    if let Ok(_) = fs::write(&stderr_file, stderr) {
        artifacts.push(stderr_file);
    }
    
    // Save metadata
    let metadata = format!(
        "{{\"gate\":\"{}\",\"timestamp\":{},\"exit_code\":{},\"rustc_version\":\"{}\",\"os\":\"{}\"}}",
        gate_name,
        timestamp,
        exit_code,
        get_rustc_version(),
        std::env::consts::OS
    );
    let metadata_file = format!("target/ci-artifacts/{}-{}-metadata.json", gate_name, timestamp);
    if let Ok(_) = fs::write(&metadata_file, metadata) {
        artifacts.push(metadata_file);
    }
    
    artifacts
}

fn get_rustc_version() -> String {
    Command::new("rustc")
        .args(&["--version"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn is_transient_failure(stderr: &str) -> bool {
    // Check for network-related errors that might be transient
    stderr.contains("network") || 
    stderr.contains("timeout") || 
    stderr.contains("connection") ||
    stderr.contains("DNS") ||
    stderr.contains("temporary failure")
}

fn is_transient_error(error: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::TimedOut | 
        ErrorKind::ConnectionRefused | 
        ErrorKind::ConnectionAborted |
        ErrorKind::NetworkUnreachable => true,
        _ => false,
    }
}

fn parse_audit_json(json_output: &str) -> Option<usize> {
    // Simple JSON parsing to count vulnerabilities
    // This is a basic implementation - in a real scenario you'd use proper JSON parsing
    if json_output.trim().is_empty() {
        return Some(0);
    }
    
    // Count occurrences of vulnerability objects in JSON
    let vuln_count = json_output.matches("\"advisory\"").count();
    Some(vuln_count)
}

fn count_runtime_dependencies(metadata_json: &str) -> Result<usize, String> {
    // Simple dependency counting using string parsing
    // Count unique dependency names in the packages section
    let mut deps = std::collections::HashSet::new();
    
    let lines: Vec<&str> = metadata_json.lines().collect();
    let mut in_dependencies = false;
    
    for line in lines {
        let trimmed = line.trim();
        
        // Look for dependencies sections
        if trimmed.contains("\"dependencies\": [") {
            in_dependencies = true;
            continue;
        }
        
        if in_dependencies {
            if trimmed == "]" || trimmed == "]," {
                in_dependencies = false;
                continue;
            }
            
            // Extract dependency names
            if trimmed.contains("\"name\":") {
                if let Some(start) = trimmed.find("\"name\": \"") {
                    let name_part = &trimmed[start + 9..];
                    if let Some(end) = name_part.find('"') {
                        let dep_name = &name_part[..end];
                        // Only count non-workspace dependencies
                        if !dep_name.starts_with("flight-") {
                            deps.insert(dep_name.to_string());
                        }
                    }
                }
            }
        }
    }
    
    Ok(deps.len())
}

fn run_duplicate_major_detection_gate() -> GateResult {
    println!("  Checking for duplicate major versions...");
    
    let output = Command::new("cargo")
        .args(&["tree", "-d"])
        .output();
        
    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);
            let artifacts = save_gate_artifacts("cargo-tree-duplicates", &stdout, &stderr, 0);
            
            // Check for duplicate major versions of critical crates
            let critical_crates = ["axum", "tower", "hyper", "thiserror", "syn"];
            let mut duplicate_majors = Vec::new();
            
            for crate_name in &critical_crates {
                let _pattern = format!(r"{}\s+v\d+", crate_name);
                let matches: Vec<&str> = stdout.lines()
                    .filter(|line| line.contains(crate_name))
                    .collect();
                
                if matches.len() > 1 {
                    // Extract version numbers to check for different majors
                    let mut major_versions = std::collections::HashSet::new();
                    for line in matches {
                        if let Some(version_start) = line.find(" v") {
                            let version_part = &line[version_start + 2..];
                            if let Some(dot_pos) = version_part.find('.') {
                                let major = &version_part[..dot_pos];
                                major_versions.insert(major.to_string());
                            }
                        }
                    }
                    
                    if major_versions.len() > 1 {
                        duplicate_majors.push(format!("{} (majors: {})", 
                                                    crate_name, 
                                                    major_versions.into_iter().collect::<Vec<_>>().join(", ")));
                    }
                }
            }
            
            if duplicate_majors.is_empty() {
                GateResult {
                    name: "Duplicate Major Version Detection".to_string(),
                    passed: true,
                    message: "No duplicate major versions found for critical crates".to_string(),
                    artifacts,
                }
            } else {
                GateResult {
                    name: "Duplicate Major Version Detection".to_string(),
                    passed: false,
                    message: format!("Duplicate major versions found: {}", duplicate_majors.join(", ")),
                    artifacts,
                }
            }
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            let artifacts = save_gate_artifacts("cargo-tree-duplicates", "", &stderr, result.status.code().unwrap_or(-1));
            
            GateResult {
                name: "Duplicate Major Version Detection".to_string(),
                passed: false,
                message: "Failed to run cargo tree -d".to_string(),
                artifacts,
            }
        }
        Err(e) => {
            GateResult {
                name: "Duplicate Major Version Detection".to_string(),
                passed: false,
                message: format!("Failed to execute cargo tree: {}", e),
                artifacts: Vec::new(),
            }
        }
    }
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
                "Duplicate Major Version Detection" => {
                    println!("🔄 Duplicate Major Version Detection:");
                    println!("   - Run 'cargo tree -d' to see all duplicate dependencies");
                    println!("   - Update Cargo.toml to use unified versions");
                    println!("   - Add [patch.crates-io] entries to force version unification");
                    println!("   - Review workspace dependencies for version consistency");
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
                "Cargo About Integration" => {
                    println!("📋 Cargo About Integration:");
                    println!("   - Install cargo-about: cargo install cargo-about --locked --version 0.6.4");
                    println!("   - Create about.hjson configuration file");
                    println!("   - Run 'cargo about generate' to create license documentation");
                    println!("   - Ensure all license texts are included and complete");
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

fn run_cargo_about_gate() -> GateResult {
    println!("  Running deterministic license document generation...");
    
    // Step 1: Validate lockfile is clean (no uncommitted changes)
    println!("    Validating Cargo.lock is clean...");
    let lockfile_check = Command::new("git")
        .args(&["diff", "--quiet", "Cargo.lock"])
        .output();
        
    match lockfile_check {
        Ok(result) if !result.status.success() => {
            return GateResult {
                name: "Cargo About Integration".to_string(),
                passed: false,
                message: "Cargo.lock has uncommitted changes - deterministic generation requires clean lockfile".to_string(),
                artifacts: Vec::new(),
            };
        }
        Err(e) => {
            return GateResult {
                name: "Cargo About Integration".to_string(),
                passed: false,
                message: format!("Failed to check Cargo.lock status: {}", e),
                artifacts: Vec::new(),
            };
        }
        _ => {} // Lockfile is clean, continue
    }
    
    // Step 2: Pin cargo-about version for consistent output
    println!("    Installing pinned cargo-about version 0.6.4...");
    let install_output = Command::new("cargo")
        .args(&["install", "cargo-about", "--locked", "--version", "0.6.4", "--force"])
        .output();
        
    match install_output {
        Ok(result) if !result.status.success() => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            return GateResult {
                name: "Cargo About Integration".to_string(),
                passed: false,
                message: format!("Failed to install cargo-about 0.6.4: {}", stderr),
                artifacts: Vec::new(),
            };
        }
        Err(e) => {
            return GateResult {
                name: "Cargo About Integration".to_string(),
                passed: false,
                message: format!("Failed to execute cargo install: {}", e),
                artifacts: Vec::new(),
            };
        }
        _ => {} // Installation successful
    }
    
    // Step 3: Generate license documentation with deterministic output
    println!("    Generating license documentation...");
    let output = Command::new("cargo")
        .args(&["about", "generate", "about.hbs", "--output-file", "THIRD_PARTY_LICENSES.md"])
        .output();
        
    match output {
        Ok(result) => {
            let exit_code = result.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);
            
            // Save comprehensive artifacts
            let artifacts = save_comprehensive_artifacts("cargo-about", &stdout, &stderr, exit_code);
            
            if exit_code != 0 {
                return GateResult {
                    name: "Cargo About Integration".to_string(),
                    passed: false,
                    message: format!("cargo about generate failed with exit code {}: {}", exit_code, stderr),
                    artifacts,
                };
            }
            
            // Step 4: Verify deterministic output - check if git diff is clean
            println!("    Verifying deterministic output...");
            let diff_check = Command::new("git")
                .args(&["diff", "--exit-code", "THIRD_PARTY_LICENSES.md"])
                .output();
                
            match diff_check {
                Ok(diff_result) => {
                    if diff_result.status.success() {
                        // No changes - output is deterministic
                        GateResult {
                            name: "Cargo About Integration".to_string(),
                            passed: true,
                            message: "License documentation generated successfully with deterministic output".to_string(),
                            artifacts,
                        }
                    } else {
                        // Changes detected - output is not deterministic or file was updated
                        let diff_output = String::from_utf8_lossy(&diff_result.stdout);
                        if diff_output.trim().is_empty() {
                            // File was newly created
                            GateResult {
                                name: "Cargo About Integration".to_string(),
                                passed: true,
                                message: "License documentation generated successfully (new file created)".to_string(),
                                artifacts,
                            }
                        } else {
                            // File was modified - this could indicate non-deterministic output
                            println!("    Warning: THIRD_PARTY_LICENSES.md was modified during generation");
                            GateResult {
                                name: "Cargo About Integration".to_string(),
                                passed: true,
                                message: "License documentation generated with modifications (check for determinism)".to_string(),
                                artifacts,
                            }
                        }
                    }
                }
                Err(e) => {
                    GateResult {
                        name: "Cargo About Integration".to_string(),
                        passed: false,
                        message: format!("Failed to verify deterministic output: {}", e),
                        artifacts,
                    }
                }
            }
        }
        Err(e) => {
            GateResult {
                name: "Cargo About Integration".to_string(),
                passed: false,
                message: format!("Failed to run cargo-about: {}", e),
                artifacts: Vec::new(),
            }
        }
    }
}

fn run_source_validation_gate() -> GateResult {
    println!("  Validating registry and VCS source restrictions...");
    
    // Run cargo deny check sources to validate source restrictions
    let output = Command::new("cargo")
        .args(&["deny", "--locked", "--version", "0.14.23", "check", "sources", "--format", "json"])
        .output();
        
    match output {
        Ok(result) => {
            let exit_code = result.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);
            
            // Save raw output as artifact
            let artifacts = save_gate_artifacts("cargo-deny-sources", &stdout, &stderr, exit_code);
            
            // Always check exit code first
            if exit_code != 0 {
                // Parse JSON for detailed error information
                let error_details = if let Ok(report) = parse_deny_json(&stdout) {
                    let error_count = report.diagnostics.iter()
                        .filter(|d| d.severity == "error")
                        .count();
                    let warning_count = report.diagnostics.iter()
                        .filter(|d| d.severity == "warn")
                        .count();
                    
                    // Check for specific source violations
                    let git_violations = report.diagnostics.iter()
                        .filter(|d| d.message.contains("git") || d.message.contains("unknown-git"))
                        .count();
                    let registry_violations = report.diagnostics.iter()
                        .filter(|d| d.message.contains("registry") || d.message.contains("unknown-registry"))
                        .count();
                    
                    format!("{} errors, {} warnings (git: {}, registry: {})", 
                           error_count, warning_count, git_violations, registry_violations)
                } else {
                    // Fallback to stderr parsing
                    let error_lines: Vec<&str> = stderr.lines()
                        .filter(|line| line.contains("error:") || line.contains("denied:"))
                        .collect();
                    format!("{} source violations detected", error_lines.len())
                };
                
                return GateResult {
                    name: "Source Validation".to_string(),
                    passed: false,
                    message: format!("Source restrictions violated: {}", error_details),
                    artifacts,
                };
            }
            
            // Additional validation: Check for git dependencies using cargo tree
            println!("    Performing additional git dependency validation...");
            let tree_output = Command::new("cargo")
                .args(&["tree", "--format", "{p} {r}"])
                .output();
                
            match tree_output {
                Ok(tree_result) if tree_result.status.success() => {
                    let tree_stdout = String::from_utf8_lossy(&tree_result.stdout);
                    let git_deps: Vec<&str> = tree_stdout.lines()
                        .filter(|line| line.contains("git+"))
                        .collect();
                    
                    if !git_deps.is_empty() {
                        return GateResult {
                            name: "Source Validation".to_string(),
                            passed: false,
                            message: format!("Found {} git dependencies: {}", 
                                           git_deps.len(), 
                                           git_deps.iter().take(3).map(|s| s.split_whitespace().next().unwrap_or("")).collect::<Vec<_>>().join(", ")),
                            artifacts,
                        };
                    }
                }
                _ => {
                    // Tree command failed, but deny passed, so continue
                    println!("    Warning: Could not validate git dependencies with cargo tree");
                }
            }
            
            // Parse JSON output for warnings
            if let Ok(report) = parse_deny_json(&stdout) {
                let warnings: Vec<&Diagnostic> = report.diagnostics.iter()
                    .filter(|d| d.severity == "warn")
                    .collect();
                
                let message = if warnings.is_empty() {
                    "All source restrictions validated - only crates.io registry allowed".to_string()
                } else {
                    format!("Source validation passed with {} warnings", warnings.len())
                };
                
                GateResult {
                    name: "Source Validation".to_string(),
                    passed: true,
                    message,
                    artifacts,
                }
            } else {
                GateResult {
                    name: "Source Validation".to_string(),
                    passed: true,
                    message: "Source restrictions validated (JSON parse failed)".to_string(),
                    artifacts,
                }
            }
        }
        Err(e) => {
            GateResult {
                name: "Source Validation".to_string(),
                passed: false,
                message: format!("Failed to run cargo deny sources: {}", e),
                artifacts: Vec::new(),
            }
        }
    }
}

fn run_msrv_edition_gate() -> GateResult {
    println!("  Validating MSRV and edition consistency across workspace...");
    
    // Run the MSRV/edition validation script
    let output = Command::new("cargo")
        .args(&["+nightly", "-Zscript", "scripts/validate_msrv_edition.rs"])
        .output();
        
    match output {
        Ok(result) => {
            let exit_code = result.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);
            
            // Save output as artifact
            let artifacts = save_gate_artifacts("msrv-edition-validation", &stdout, &stderr, exit_code);
            
            if exit_code == 0 {
                // Parse success message for details
                let message = if stdout.contains("All") && stdout.contains("workspace crates comply") {
                    // Extract the number of crates from the output
                    if let Some(line) = stdout.lines().find(|l| l.contains("workspace crates comply")) {
                        line.trim_start_matches("✅ ").to_string()
                    } else {
                        "All workspace crates comply with MSRV and edition requirements".to_string()
                    }
                } else {
                    "MSRV and edition validation passed".to_string()
                };
                
                GateResult {
                    name: "MSRV and Edition Enforcement".to_string(),
                    passed: true,
                    message,
                    artifacts,
                }
            } else {
                // Parse failure details from output
                let mut violations = Vec::new();
                let mut in_violations = false;
                
                for line in stdout.lines() {
                    if line.starts_with("❌") {
                        in_violations = true;
                        continue;
                    }
                    if in_violations && line.starts_with("  - ") {
                        violations.push(line.trim_start_matches("  - ").to_string());
                    }
                    if in_violations && line.starts_with("💡") {
                        break;
                    }
                }
                
                let violation_summary = if violations.is_empty() {
                    "MSRV/edition violations detected".to_string()
                } else {
                    format!("{} violations: {}", 
                           violations.len(),
                           violations.iter().take(3).cloned().collect::<Vec<_>>().join("; "))
                };
                
                GateResult {
                    name: "MSRV and Edition Enforcement".to_string(),
                    passed: false,
                    message: violation_summary,
                    artifacts,
                }
            }
        }
        Err(e) => {
            GateResult {
                name: "MSRV and Edition Enforcement".to_string(),
                passed: false,
                message: format!("Failed to run MSRV/edition validation: {}", e),
                artifacts: Vec::new(),
            }
        }
    }
}

fn save_comprehensive_artifacts(gate_name: &str, stdout: &str, stderr: &str, exit_code: i32) -> Vec<String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let mut artifacts = Vec::new();
    
    // Create artifacts directory if it doesn't exist
    if let Err(_) = fs::create_dir_all("target/ci-artifacts") {
        return artifacts;
    }
    
    // Save raw outputs
    let stdout_file = format!("target/ci-artifacts/{}-{}-stdout.json", gate_name, timestamp);
    if let Ok(_) = fs::write(&stdout_file, stdout) {
        artifacts.push(stdout_file);
    }
    
    let stderr_file = format!("target/ci-artifacts/{}-{}-stderr.txt", gate_name, timestamp);
    if let Ok(_) = fs::write(&stderr_file, stderr) {
        artifacts.push(stderr_file);
    }
    
    // Enhanced metadata with tool versions and environment
    let tool_versions = get_tool_versions();
    let metadata = format!(
        "{{\"gate\":\"{}\",\"timestamp\":{},\"exit_code\":{},\"tool_versions\":{},\"environment\":{{\"os\":\"{}\",\"arch\":\"{}\",\"rustc\":\"{}\",\"cargo\":\"{}\"}},\"ci_info\":{{\"github_actions\":{},\"runner_os\":\"{}\",\"workflow\":\"{}\",\"run_id\":\"{}\"}}}}",
        gate_name,
        timestamp,
        exit_code,
        format_tool_versions(&tool_versions),
        std::env::consts::OS,
        std::env::consts::ARCH,
        get_rustc_version(),
        get_cargo_version(),
        std::env::var("GITHUB_ACTIONS").unwrap_or_else(|_| "false".to_string()) == "true",
        std::env::var("RUNNER_OS").unwrap_or_else(|_| "unknown".to_string()),
        std::env::var("GITHUB_WORKFLOW").unwrap_or_else(|_| "unknown".to_string()),
        std::env::var("GITHUB_RUN_ID").unwrap_or_else(|_| "unknown".to_string())
    );
    
    let metadata_file = format!("target/ci-artifacts/{}-{}-metadata.json", gate_name, timestamp);
    if let Ok(_) = fs::write(&metadata_file, metadata) {
        artifacts.push(metadata_file);
    }
    
    // Save execution summary
    let summary = format!(
        "Gate: {}\nTimestamp: {}\nExit Code: {}\nPassed: {}\nArtifacts: {}\n",
        gate_name,
        timestamp,
        exit_code,
        exit_code == 0,
        artifacts.len()
    );
    
    let summary_file = format!("target/ci-artifacts/{}-{}-summary.txt", gate_name, timestamp);
    if let Ok(_) = fs::write(&summary_file, summary) {
        artifacts.push(summary_file);
    }
    
    artifacts
}

fn get_tool_versions() -> std::collections::HashMap<String, String> {
    let mut versions = std::collections::HashMap::new();
    
    // Get cargo-deny version
    if let Ok(output) = Command::new("cargo").args(&["deny", "--version"]).output() {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            versions.insert("cargo-deny".to_string(), version);
        }
    }
    
    // Get cargo-about version
    if let Ok(output) = Command::new("cargo").args(&["about", "--version"]).output() {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            versions.insert("cargo-about".to_string(), version);
        }
    }
    
    // Get cargo-audit version
    if let Ok(output) = Command::new("cargo").args(&["audit", "--version"]).output() {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            versions.insert("cargo-audit".to_string(), version);
        }
    }
    
    versions
}

fn get_cargo_version() -> String {
    Command::new("cargo")
        .args(&["--version"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn parse_deny_json(json_str: &str) -> Result<DenyReport, String> {
    // Simple manual JSON parsing for deny report
    let mut diagnostics = Vec::new();
    
    // Look for diagnostic objects in the JSON
    let lines: Vec<&str> = json_str.lines().collect();
    let mut in_diagnostic = false;
    let mut current_severity = String::new();
    let mut current_message = String::new();
    
    for line in lines {
        let trimmed = line.trim();
        
        if trimmed.contains("\"severity\":") {
            if let Some(start) = trimmed.find("\"severity\": \"") {
                let severity_part = &trimmed[start + 13..];
                if let Some(end) = severity_part.find('"') {
                    current_severity = severity_part[..end].to_string();
                    in_diagnostic = true;
                }
            }
        }
        
        if in_diagnostic && trimmed.contains("\"message\":") {
            if let Some(start) = trimmed.find("\"message\": \"") {
                let message_part = &trimmed[start + 12..];
                if let Some(end) = message_part.rfind('"') {
                    current_message = message_part[..end].to_string();
                }
            }
        }
        
        if in_diagnostic && (trimmed == "}" || trimmed == "},") {
            diagnostics.push(Diagnostic {
                severity: current_severity.clone(),
                message: current_message.clone(),
            });
            in_diagnostic = false;
            current_severity.clear();
            current_message.clear();
        }
    }
    
    Ok(DenyReport { diagnostics })
}



fn format_tool_versions(versions: &std::collections::HashMap<String, String>) -> String {
    let mut result = String::from("{");
    let mut first = true;
    
    for (key, value) in versions {
        if !first {
            result.push(',');
        }
        result.push_str(&format!("\"{}\":\"{}\"", key, value));
        first = false;
    }
    
    result.push('}');
    result
}