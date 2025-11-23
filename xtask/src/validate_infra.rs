// SPDX-License-Identifier: MIT OR Apache-2.0

//! Infrastructure validation module.
//!
//! This module implements the `cargo xtask validate-infra` command, which validates:
//! 1. All infra/**/invariants.yaml files against schemas/invariants.schema.json
//! 2. Docker Compose configurations (if docker/docker compose is available)
//! 3. Kubernetes configurations (if kubectl is available and infra/k8s/ exists)
//! 4. Cross-checks between invariants.yaml ports and compose service ports

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

use crate::schema;

/// Error information for infrastructure validation failures.
#[derive(Debug, Clone)]
pub struct InfraError {
    /// Error code in format INF-INFRA-NNN
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// File path where the error occurred (if applicable)
    pub file_path: Option<String>,
    /// Suggestion for fixing the error
    pub suggestion: Option<String>,
}

impl InfraError {
    /// Format the error for display according to INF-INFRA-NNN format.
    pub fn format(&self) -> String {
        let mut output = format!("[ERROR] {}: {}", self.code, self.message);

        if let Some(file_path) = &self.file_path {
            output.push_str(&format!("\n  File: {}", file_path));
        }

        if let Some(suggestion) = &self.suggestion {
            output.push_str(&format!("\n  Suggestion: {}", suggestion));
        }

        output
    }
}

/// Invariants structure matching the schema.
#[derive(Debug, Deserialize)]
struct Invariants {
    environment: String,
    rust_version: String,
    #[serde(default)]
    rust_edition: Option<String>,
    #[serde(default)]
    ports: HashMap<String, u16>,
    #[serde(default)]
    env_vars: HashMap<String, EnvVarSpec>,
    #[serde(default)]
    resources: Option<Resources>,
}

#[derive(Debug, Deserialize)]
struct EnvVarSpec {
    required: bool,
    #[serde(default)]
    default: Option<String>,
    description: String,
}

#[derive(Debug, Deserialize)]
struct Resources {
    #[serde(default)]
    cpu_limit: Option<String>,
    #[serde(default)]
    memory_limit: Option<String>,
}

/// Run infrastructure validation.
///
/// This function validates:
/// 1. All invariants.yaml files against the schema
/// 2. Docker Compose configurations (if available)
/// 3. Kubernetes configurations (if available)
/// 4. Port consistency between invariants and compose files
///
/// # Returns
///
/// Returns `Ok(())` if all checks pass, or an error if any check fails.
///
/// # Errors
///
/// Returns an error if:
/// - Schema validation fails for any invariants.yaml
/// - Docker Compose validation fails (when docker is available)
/// - Kubernetes validation fails (when kubectl is available)
/// - Port mismatches are detected
pub fn run_validate_infra() -> Result<()> {
    println!("🏗️  Validating infrastructure configurations...\n");

    let mut all_errors = Vec::new();
    let mut warnings = Vec::new();

    // Step 1: Validate all invariants.yaml files against schema
    println!("📋 Step 1: Validating invariants.yaml files");
    println!("─────────────────────────────────────────────");
    match validate_all_invariants() {
        Ok(count) => {
            println!("✅ Validated {} invariants.yaml file(s)\n", count);
        }
        Err(errors) => {
            println!("❌ Invariants validation failed\n");
            all_errors.extend(errors);
        }
    }

    // Step 2: Validate Docker Compose configurations
    println!("🐳 Step 2: Validating Docker Compose configurations");
    println!("─────────────────────────────────────────────");
    match validate_docker_compose() {
        Ok(Some(msg)) => {
            println!("✅ {}\n", msg);
        }
        Ok(None) => {
            let warning = "[WARN] INF-INFRA-100: Docker not available, skipping Docker Compose validation";
            println!("⚠️  {}\n", warning);
            warnings.push(warning.to_string());
        }
        Err(e) => {
            println!("❌ Docker Compose validation failed\n");
            all_errors.push(InfraError {
                code: "INF-INFRA-001".to_string(),
                message: format!("Docker Compose validation failed: {}", e),
                file_path: Some("infra/local/docker-compose.yml".to_string()),
                suggestion: Some("Run 'docker compose -f infra/local/docker-compose.yml config' manually to see detailed errors".to_string()),
            });
        }
    }

    // Step 3: Validate Kubernetes configurations (if infra/k8s/ exists)
    println!("☸️  Step 3: Validating Kubernetes configurations");
    println!("─────────────────────────────────────────────");
    match validate_kubernetes() {
        Ok(Some(msg)) => {
            println!("✅ {}\n", msg);
        }
        Ok(None) => {
            let warning = "[WARN] INF-INFRA-101: kubectl not available or infra/k8s/ not found, skipping Kubernetes validation";
            println!("⚠️  {}\n", warning);
            warnings.push(warning.to_string());
        }
        Err(e) => {
            println!("❌ Kubernetes validation failed\n");
            all_errors.push(InfraError {
                code: "INF-INFRA-002".to_string(),
                message: format!("Kubernetes validation failed: {}", e),
                file_path: Some("infra/k8s/".to_string()),
                suggestion: Some("Run 'kubectl apply --dry-run=client -f infra/k8s/' manually to see detailed errors".to_string()),
            });
        }
    }

    // Step 4: Cross-check invariants.yaml ports against compose service ports
    println!("🔍 Step 4: Cross-checking ports");
    println!("─────────────────────────────────────────────");
    match cross_check_ports() {
        Ok(()) => {
            println!("✅ Port consistency check passed\n");
        }
        Err(errors) => {
            println!("❌ Port consistency check failed\n");
            all_errors.extend(errors);
        }
    }

    // Print summary
    println!("📊 Summary");
    println!("─────────────────────────────────────────────");
    println!("Errors: {}", all_errors.len());
    println!("Warnings: {}", warnings.len());

    if !all_errors.is_empty() {
        println!("\n❌ Infrastructure validation failed with {} error(s):", all_errors.len());
        for error in &all_errors {
            eprintln!("\n{}", error.format());
        }
        anyhow::bail!("Infrastructure validation failed");
    }

    if !warnings.is_empty() {
        println!("\n⚠️  {} warning(s) - some checks were skipped", warnings.len());
    }

    println!("\n✅ Infrastructure validation passed!");
    Ok(())
}

/// Validate all invariants.yaml files against the schema.
///
/// This function walks the infra/ directory tree and validates each
/// invariants.yaml file found against schemas/invariants.schema.json.
///
/// # Returns
///
/// Returns `Ok(count)` with the number of files validated, or `Err`
/// containing all validation errors encountered.
fn validate_all_invariants() -> Result<usize, Vec<InfraError>> {
    let infra_dir = Path::new("infra");
    let schema_path = Path::new("schemas/invariants.schema.json");

    if !infra_dir.exists() {
        println!("  ⚠️  infra/ directory not found, skipping");
        return Ok(0);
    }

    if !schema_path.exists() {
        return Err(vec![InfraError {
            code: "INF-INFRA-003".to_string(),
            message: "Schema file not found".to_string(),
            file_path: Some(schema_path.display().to_string()),
            suggestion: Some("Ensure schemas/invariants.schema.json exists".to_string()),
        }]);
    }

    // Find all invariants.yaml files
    let invariants_files: Vec<PathBuf> = WalkDir::new(infra_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.file_name() == "invariants.yaml")
        .map(|e| e.path().to_path_buf())
        .collect();

    if invariants_files.is_empty() {
        println!("  ⚠️  No invariants.yaml files found in infra/");
        return Ok(0);
    }

    let mut all_errors = Vec::new();
    let mut validated_count = 0;

    for invariants_path in &invariants_files {
        println!("  Validating {}...", invariants_path.display());

        match schema::validate_yaml_against_schema(invariants_path, schema_path) {
            Ok(()) => {
                println!("    ✓ Valid");
                validated_count += 1;
            }
            Err(schema_errors) => {
                println!("    ✗ {} error(s) found", schema_errors.len());
                
                // Convert schema errors to infra errors
                for schema_error in schema_errors {
                    eprintln!("{}", schema_error.format());
                    all_errors.push(InfraError {
                        code: schema_error.code,
                        message: schema_error.message,
                        file_path: Some(schema_error.file_path),
                        suggestion: schema_error.suggestion,
                    });
                }
            }
        }
    }

    if all_errors.is_empty() {
        Ok(validated_count)
    } else {
        Err(all_errors)
    }
}

/// Validate Docker Compose configurations.
///
/// This function runs `docker compose config` to validate the compose file.
/// If docker or docker compose is not installed, it returns Ok(None) to
/// indicate the check was skipped (not a failure).
///
/// # Returns
///
/// Returns:
/// - `Ok(Some(msg))` if docker compose is available and validation passed
/// - `Ok(None)` if docker compose is not available (skipped)
/// - `Err` if docker compose is available but validation failed
fn validate_docker_compose() -> Result<Option<String>> {
    let compose_file = Path::new("infra/local/docker-compose.yml");

    if !compose_file.exists() {
        println!("  ⚠️  infra/local/docker-compose.yml not found, skipping");
        return Ok(None);
    }

    // Check if docker compose is available
    let check_docker = Command::new("docker")
        .args(["compose", "version"])
        .output();

    match check_docker {
        Ok(output) if output.status.success() => {
            println!("  Running docker compose config...");

            // Run docker compose config to validate
            let status = Command::new("docker")
                .args([
                    "compose",
                    "-f",
                    "infra/local/docker-compose.yml",
                    "config",
                    "--quiet",
                ])
                .status()
                .context("Failed to execute docker compose config")?;

            if status.success() {
                Ok(Some("Docker Compose configuration is valid".to_string()))
            } else {
                anyhow::bail!("docker compose config reported errors")
            }
        }
        _ => {
            // Docker or docker compose not available
            Ok(None)
        }
    }
}

/// Validate Kubernetes configurations.
///
/// This function runs `kubectl apply --dry-run=client` to validate k8s manifests.
/// If kubectl is not installed or infra/k8s/ doesn't exist, it returns Ok(None)
/// to indicate the check was skipped (not a failure).
///
/// # Returns
///
/// Returns:
/// - `Ok(Some(msg))` if kubectl is available and validation passed
/// - `Ok(None)` if kubectl is not available or infra/k8s/ doesn't exist (skipped)
/// - `Err` if kubectl is available but validation failed
fn validate_kubernetes() -> Result<Option<String>> {
    let k8s_dir = Path::new("infra/k8s");

    if !k8s_dir.exists() {
        println!("  ⚠️  infra/k8s/ directory not found, skipping");
        return Ok(None);
    }

    // Check if kubectl is available
    let check_kubectl = Command::new("kubectl")
        .args(["version", "--client", "--short"])
        .output();

    match check_kubectl {
        Ok(output) if output.status.success() => {
            println!("  Running kubectl apply --dry-run=client...");

            // Run kubectl apply with dry-run
            let status = Command::new("kubectl")
                .args([
                    "apply",
                    "--dry-run=client",
                    "-f",
                    "infra/k8s/",
                    "--recursive",
                ])
                .status()
                .context("Failed to execute kubectl apply")?;

            if status.success() {
                Ok(Some("Kubernetes manifests are valid".to_string()))
            } else {
                anyhow::bail!("kubectl apply --dry-run reported errors")
            }
        }
        _ => {
            // kubectl not available
            Ok(None)
        }
    }
}

/// Cross-check ports between invariants.yaml and docker-compose.yml.
///
/// This function verifies that ports defined in invariants.yaml match
/// the ports exposed in docker-compose.yml for the same service names.
///
/// # Returns
///
/// Returns `Ok(())` if all ports match, or `Err` containing mismatches.
fn cross_check_ports() -> Result<(), Vec<InfraError>> {
    let invariants_path = Path::new("infra/local/invariants.yaml");
    let compose_path = Path::new("infra/local/docker-compose.yml");

    // If either file doesn't exist, skip the check
    if !invariants_path.exists() || !compose_path.exists() {
        println!("  ⚠️  Skipping port cross-check (files not found)");
        return Ok(());
    }

    // Load invariants
    let invariants_content = std::fs::read_to_string(invariants_path)
        .context("Failed to read invariants.yaml")
        .map_err(|e| {
            vec![InfraError {
                code: "INF-INFRA-004".to_string(),
                message: format!("Failed to read invariants.yaml: {}", e),
                file_path: Some(invariants_path.display().to_string()),
                suggestion: None,
            }]
        })?;

    let invariants: Invariants = serde_yaml::from_str(&invariants_content)
        .context("Failed to parse invariants.yaml")
        .map_err(|e| {
            vec![InfraError {
                code: "INF-INFRA-005".to_string(),
                message: format!("Failed to parse invariants.yaml: {}", e),
                file_path: Some(invariants_path.display().to_string()),
                suggestion: Some("Ensure invariants.yaml is valid YAML".to_string()),
            }]
        })?;

    // Load compose file
    let compose_content = std::fs::read_to_string(compose_path)
        .context("Failed to read docker-compose.yml")
        .map_err(|e| {
            vec![InfraError {
                code: "INF-INFRA-006".to_string(),
                message: format!("Failed to read docker-compose.yml: {}", e),
                file_path: Some(compose_path.display().to_string()),
                suggestion: None,
            }]
        })?;

    // Parse compose file to extract port mappings
    // Note: We're doing a simple text-based extraction here rather than
    // parsing the full compose schema, which would require additional dependencies
    let compose_ports = extract_compose_ports(&compose_content);

    // Cross-check ports
    let mut errors = Vec::new();

    for (service_name, expected_port) in &invariants.ports {
        if let Some(actual_port) = compose_ports.get(service_name) {
            if actual_port != expected_port {
                errors.push(InfraError {
                    code: "INF-INFRA-007".to_string(),
                    message: format!(
                        "Port mismatch for service '{}': invariants.yaml specifies {}, but docker-compose.yml uses {}",
                        service_name, expected_port, actual_port
                    ),
                    file_path: Some(compose_path.display().to_string()),
                    suggestion: Some(format!(
                        "Update docker-compose.yml to use port {} for service '{}'",
                        expected_port, service_name
                    )),
                });
            }
        } else {
            println!(
                "  ⚠️  Service '{}' defined in invariants.yaml but not found in docker-compose.yml",
                service_name
            );
        }
    }

    if errors.is_empty() {
        println!("  ✓ All ports match between invariants.yaml and docker-compose.yml");
        Ok(())
    } else {
        Err(errors)
    }
}

/// Extract port mappings from docker-compose.yml content.
///
/// This is a simple text-based extraction that looks for service names
/// and their port mappings. It's not a full YAML parser but sufficient
/// for basic port validation.
///
/// # Arguments
///
/// * `compose_content` - The content of docker-compose.yml as a string
///
/// # Returns
///
/// A HashMap mapping service names to their exposed ports.
fn extract_compose_ports(compose_content: &str) -> HashMap<String, u16> {
    let mut ports = HashMap::new();
    let mut current_service: Option<String> = None;

    for line in compose_content.lines() {
        let trimmed = line.trim();

        // Look for service definitions (e.g., "flight-service:")
        if !trimmed.starts_with('-') && !trimmed.starts_with('#') && trimmed.ends_with(':') {
            let service_name = trimmed.trim_end_matches(':').to_string();
            // Only consider it a service if it's not a top-level key like "services:"
            if service_name != "services"
                && service_name != "ports"
                && service_name != "environment"
                && service_name != "volumes"
                && service_name != "deploy"
                && service_name != "resources"
                && service_name != "limits"
                && service_name != "healthcheck"
            {
                current_service = Some(service_name);
            }
        }

        // Look for port mappings (e.g., "- \"8080:8080\"" or "- 8080:8080")
        if let Some(ref service) = current_service {
            if trimmed.starts_with("- \"") || trimmed.starts_with("- ") {
                // Extract port mapping
                let port_str = trimmed
                    .trim_start_matches("- \"")
                    .trim_start_matches("- ")
                    .trim_end_matches('"');

                // Parse "host:container" format
                if let Some(colon_pos) = port_str.find(':') {
                    let host_port = &port_str[..colon_pos];
                    if let Ok(port) = host_port.parse::<u16>() {
                        ports.insert(service.clone(), port);
                    }
                }
            }
        }
    }

    ports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_compose_ports() {
        let compose_content = r#"
services:
  flight-service:
    image: rust:1.89.0
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=info
  
  metrics:
    image: prom/prometheus
    ports:
      - "9090:9090"
"#;

        let ports = extract_compose_ports(compose_content);
        assert_eq!(ports.get("flight-service"), Some(&8080));
        assert_eq!(ports.get("metrics"), Some(&9090));
    }

    #[test]
    fn test_extract_compose_ports_no_quotes() {
        let compose_content = r#"
services:
  flight-service:
    ports:
      - 8080:8080
"#;

        let ports = extract_compose_ports(compose_content);
        assert_eq!(ports.get("flight-service"), Some(&8080));
    }

    #[test]
    fn test_infra_error_formatting() {
        let error = InfraError {
            code: "INF-INFRA-007".to_string(),
            message: "Port mismatch detected".to_string(),
            file_path: Some("infra/local/docker-compose.yml".to_string()),
            suggestion: Some("Update the port mapping".to_string()),
        };

        let formatted = error.format();
        assert!(formatted.contains("[ERROR] INF-INFRA-007"));
        assert!(formatted.contains("Port mismatch detected"));
        assert!(formatted.contains("File: infra/local/docker-compose.yml"));
        assert!(formatted.contains("Suggestion: Update the port mapping"));
    }
}
