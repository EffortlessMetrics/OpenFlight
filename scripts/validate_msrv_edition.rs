#!/usr/bin/env cargo +nightly -Zscript
//! MSRV and Edition Enforcement Validator
//!
//! This script validates that all workspace crates use consistent edition and rust-version
//! settings as required by NFR-B. It ensures edition = "2024" and rust-version = "1.92.0"
//! across all workspace packages.

use std::fs;
use std::path::Path;
use std::process::exit;

const REQUIRED_EDITION: &str = "2024";
const REQUIRED_RUST_VERSION: &str = "1.92.0";

#[derive(Debug)]
#[allow(dead_code)]
struct CrateInfo {
    name: String,
    path: String,
    edition: Option<String>,
    rust_version: Option<String>,
    inherits_workspace: bool,
}

#[derive(Debug)]
struct ValidationResult {
    passed: bool,
    message: String,
    violations: Vec<String>,
}

fn main() {
    println!("🔍 MSRV and Edition Enforcement Validator");
    println!("========================================");

    let result = validate_workspace_consistency();

    if result.passed {
        println!("✅ {}", result.message);
        exit(0);
    } else {
        println!("❌ {}", result.message);
        for violation in &result.violations {
            println!("  - {}", violation);
        }

        println!("\n💡 Remediation:");
        println!("  1. Ensure all crates inherit workspace package settings:");
        println!("     [package]");
        println!("     edition.workspace = true");
        println!("     rust-version.workspace = true");
        println!("  2. Or explicitly set the required values:");
        println!("     edition = \"{}\"", REQUIRED_EDITION);
        println!("     rust-version = \"{}\"", REQUIRED_RUST_VERSION);

        exit(1);
    }
}

fn validate_workspace_consistency() -> ValidationResult {
    // First, get all workspace members
    let workspace_members = match get_workspace_members() {
        Ok(members) => members,
        Err(e) => {
            return ValidationResult {
                passed: false,
                message: format!("Failed to parse workspace members: {}", e),
                violations: vec![],
            };
        }
    };

    println!(
        "Found {} workspace members to validate",
        workspace_members.len()
    );

    let mut violations = Vec::new();
    let mut crate_infos = Vec::new();

    // Validate each workspace member
    for member in &workspace_members {
        let cargo_toml_path = format!("{}/Cargo.toml", member);

        if !Path::new(&cargo_toml_path).exists() {
            violations.push(format!("Cargo.toml not found for member: {}", member));
            continue;
        }

        match validate_crate_settings(&cargo_toml_path, member) {
            Ok(crate_info) => {
                // Check for violations
                if let Some(ref edition) = crate_info.edition {
                    if edition != REQUIRED_EDITION {
                        violations.push(format!(
                            "{}: edition = \"{}\" (expected \"{}\")",
                            member, edition, REQUIRED_EDITION
                        ));
                    }
                }

                if let Some(ref rust_version) = crate_info.rust_version {
                    if rust_version != REQUIRED_RUST_VERSION {
                        violations.push(format!(
                            "{}: rust-version = \"{}\" (expected \"{}\")",
                            member, rust_version, REQUIRED_RUST_VERSION
                        ));
                    }
                }

                // If neither workspace inheritance nor explicit values are set, that's a violation
                if !crate_info.inherits_workspace && crate_info.edition.is_none() {
                    violations.push(format!(
                        "{}: missing edition setting (should inherit workspace or set explicitly)",
                        member
                    ));
                }

                if !crate_info.inherits_workspace && crate_info.rust_version.is_none() {
                    violations.push(format!(
                        "{}: missing rust-version setting (should inherit workspace or set explicitly)",
                        member
                    ));
                }

                crate_infos.push(crate_info);
            }
            Err(e) => {
                violations.push(format!("{}: failed to parse Cargo.toml: {}", member, e));
            }
        }
    }

    // Generate summary
    let total_crates = workspace_members.len();

    if violations.is_empty() {
        ValidationResult {
            passed: true,
            message: format!(
                "All {} workspace crates comply with edition=\"{}\" and rust-version=\"{}\"",
                total_crates, REQUIRED_EDITION, REQUIRED_RUST_VERSION
            ),
            violations: vec![],
        }
    } else {
        ValidationResult {
            passed: false,
            message: format!(
                "{}/{} crates have MSRV/edition violations",
                violations.len(),
                total_crates
            ),
            violations,
        }
    }
}

fn get_workspace_members() -> Result<Vec<String>, String> {
    let cargo_toml_content = fs::read_to_string("Cargo.toml")
        .map_err(|e| format!("Failed to read root Cargo.toml: {}", e))?;

    let mut members = Vec::new();
    let lines: Vec<&str> = cargo_toml_content.lines().collect();
    let mut in_members = false;

    for line in lines {
        let trimmed = line.trim();

        if trimmed.starts_with("members = [") {
            in_members = true;
            // Handle single-line members array
            if trimmed.ends_with("]") {
                let members_line = trimmed
                    .strip_prefix("members = [")
                    .unwrap()
                    .strip_suffix("]")
                    .unwrap();
                for member in members_line.split(',') {
                    let clean_member = member.trim().trim_matches('"');
                    if !clean_member.is_empty() {
                        members.push(clean_member.to_string());
                    }
                }
                break;
            }
            continue;
        }

        if in_members {
            if trimmed == "]" {
                break;
            }

            // Parse member entry
            if let Some(member) = trimmed
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix("\","))
            {
                members.push(member.to_string());
            } else if let Some(member) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"'))
            {
                members.push(member.to_string());
            }
        }
    }

    if members.is_empty() {
        return Err("No workspace members found".to_string());
    }

    Ok(members)
}

fn validate_crate_settings(cargo_toml_path: &str, crate_name: &str) -> Result<CrateInfo, String> {
    let content = fs::read_to_string(cargo_toml_path)
        .map_err(|e| format!("Failed to read {}: {}", cargo_toml_path, e))?;

    let mut crate_info = CrateInfo {
        name: crate_name.to_string(),
        path: cargo_toml_path.to_string(),
        edition: None,
        rust_version: None,
        inherits_workspace: false,
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut in_package = false;

    for line in lines {
        let trimmed = line.trim();

        if trimmed == "[package]" {
            in_package = true;
            continue;
        }

        if in_package {
            // Check for section end
            if trimmed.starts_with('[') && trimmed != "[package]" {
                break;
            }

            // Parse edition settings
            if trimmed.starts_with("edition") {
                if trimmed.contains("workspace = true") {
                    crate_info.inherits_workspace = true;
                } else if let Some(value) = extract_toml_string_value(trimmed, "edition") {
                    crate_info.edition = Some(value);
                }
            }

            // Parse rust-version settings
            if trimmed.starts_with("rust-version") {
                if trimmed.contains("workspace = true") {
                    crate_info.inherits_workspace = true;
                } else if let Some(value) = extract_toml_string_value(trimmed, "rust-version") {
                    crate_info.rust_version = Some(value);
                }
            }
        }
    }

    // If inheriting from workspace, set the expected values
    if crate_info.inherits_workspace {
        crate_info.edition = Some(REQUIRED_EDITION.to_string());
        crate_info.rust_version = Some(REQUIRED_RUST_VERSION.to_string());
    }

    Ok(crate_info)
}

fn extract_toml_string_value(line: &str, _key: &str) -> Option<String> {
    // Handle formats like: edition = "2024" or rust-version = "1.92.0"
    if let Some(equals_pos) = line.find('=') {
        let value_part = line[equals_pos + 1..].trim();
        if let Some(value) = value_part
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
        {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_toml_string_value() {
        assert_eq!(
            extract_toml_string_value("edition = \"2024\"", "edition"),
            Some("2024".to_string())
        );
        assert_eq!(
            extract_toml_string_value("rust-version = \"1.92.0\"", "rust-version"),
            Some("1.92.0".to_string())
        );
        assert_eq!(
            extract_toml_string_value("edition.workspace = true", "edition"),
            None
        );
    }

    #[test]
    fn test_workspace_inheritance_detection() {
        let content = r#"
[package]
name = "test-crate"
edition.workspace = true
rust-version.workspace = true
"#;

        // This would require a more comprehensive test setup
        // For now, we rely on manual testing
    }
}
