// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile management commands

use crate::client_manager::ClientManager;
use crate::commands::ProfileAction;
use crate::output::OutputFormat;
use flight_ipc::ApplyProfileRequest;
use serde_json::{Value, json};
use std::fs;

pub async fn execute(
    action: &ProfileAction,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        ProfileAction::List { include_builtin } => {
            list_profiles(*include_builtin, output_format, verbose, client_manager).await
        }
        ProfileAction::Apply {
            profile_path,
            validate_only,
            force,
        } => {
            apply_profile(
                profile_path,
                *validate_only,
                *force,
                output_format,
                verbose,
                client_manager,
            )
            .await
        }
        ProfileAction::Show { name, raw } => {
            show_profile(
                name.as_deref(),
                *raw,
                output_format,
                verbose,
                client_manager,
            )
            .await
        }
        ProfileAction::Activate { name } => {
            activate_profile(name, output_format, verbose, client_manager).await
        }
        ProfileAction::Validate { path } => {
            validate_profile(path, output_format, verbose, client_manager).await
        }
        ProfileAction::Export { name, path } => {
            export_profile(name, path, output_format, verbose, client_manager).await
        }
    }
}

async fn apply_profile(
    profile_path: &std::path::Path,
    validate_only: bool,
    force: bool,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    // Read profile file
    let profile_content = fs::read_to_string(profile_path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to read profile file '{}': {}",
            profile_path.display(),
            e
        )
    })?;

    // Validate JSON format
    let _profile_json: Value = serde_json::from_str(&profile_content)
        .map_err(|e| anyhow::anyhow!("Invalid JSON in profile file: {}", e))?;

    let mut client = client_manager.get_client().await?;

    let request = ApplyProfileRequest {
        profile_json: profile_content,
        validate_only,
        force_apply: force,
    };

    let response = client.apply_profile(request).await?;

    if !response.success {
        let error_msg = if !response.validation_errors.is_empty() {
            let mut errors = vec!["Validation errors:".to_string()];
            for error in &response.validation_errors {
                errors.push(format!(
                    "  Line {}, Column {}: {} ({})",
                    error.line_number,
                    error.column_number,
                    error.error_message,
                    validation_error_type_to_string(error.error_type())
                ));
            }
            errors.join("\n")
        } else {
            response.error_message
        };

        return Err(anyhow::anyhow!("{}", error_msg));
    }

    let mut result = json!({
        "success": true,
        "validate_only": validate_only,
        "effective_profile_hash": response.effective_profile_hash,
        "compile_time_ms": response.compile_time_ms,
    });

    if validate_only {
        result["message"] = json!("Profile validation successful");
    } else {
        result["message"] = json!("Profile applied successfully");
    }

    if verbose && !response.validation_errors.is_empty() {
        let validation_errors: Vec<Value> = response
            .validation_errors
            .iter()
            .map(|error| {
                json!({
                    "field_path": error.field_path,
                    "line_number": error.line_number,
                    "column_number": error.column_number,
                    "error_message": error.error_message,
                    "error_type": validation_error_type_to_string(error.error_type()),
                })
            })
            .collect();

        result["validation_warnings"] = json!(validation_errors);
    }

    let output = output_format.success(result);
    Ok(Some(output))
}

async fn list_profiles(
    include_builtin: bool,
    output_format: OutputFormat,
    _verbose: bool,
    _client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    // Scan the profile directories for available profiles
    let profile_dir = dirs::config_dir()
        .map(|d| d.join("OpenFlight").join("profiles"))
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;

    let mut profiles: Vec<Value> = Vec::new();

    if profile_dir.exists() {
        for entry in fs::read_dir(&profile_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let mut profile = json!({
                    "name": name,
                    "path": path.display().to_string(),
                    "source": "user",
                });

                // Try to read and extract metadata
                if let Ok(content) = fs::read_to_string(&path)
                    && let Ok(parsed) = serde_json::from_str::<Value>(&content)
                {
                    if let Some(desc) = parsed.get("description") {
                        profile["description"] = desc.clone();
                    }
                    if let Some(version) = parsed.get("version") {
                        profile["version"] = version.clone();
                    }
                }

                profiles.push(profile);
            }
        }
    }

    if include_builtin {
        profiles.push(json!({
            "name": "default",
            "source": "builtin",
            "description": "Default profile with linear response curves",
        }));
    }

    let total = profiles.len() as i32;
    let output = output_format.list(profiles, Some(total));
    Ok(Some(output))
}

async fn show_profile(
    name: Option<&str>,
    raw: bool,
    output_format: OutputFormat,
    _verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    if let Some(profile_name) = name {
        // Look up named profile from disk
        let profile_dir = dirs::config_dir()
            .map(|d| d.join("OpenFlight").join("profiles"))
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;

        let profile_path = profile_dir.join(format!("{}.json", profile_name));
        if !profile_path.exists() {
            return Err(anyhow::anyhow!("Profile '{}' not found", profile_name));
        }

        let content = fs::read_to_string(&profile_path)?;

        if raw {
            return Ok(Some(content));
        }

        let parsed: Value = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Invalid JSON in profile '{}': {}", profile_name, e))?;

        let result = json!({
            "name": profile_name,
            "path": profile_path.display().to_string(),
            "configuration": parsed,
        });

        let output = output_format.success(result);
        return Ok(Some(output));
    }

    // Show current effective profile from daemon
    let result = json!({
        "message": "Show current effective profile requires GetCurrentProfile RPC method to be implemented in the service",
        "raw_requested": raw,
    });

    let output = output_format.success(result);
    Ok(Some(output))
}

fn validation_error_type_to_string(error_type: flight_ipc::ValidationErrorType) -> &'static str {
    match error_type {
        flight_ipc::ValidationErrorType::Unspecified => "unspecified",
        flight_ipc::ValidationErrorType::Schema => "schema",
        flight_ipc::ValidationErrorType::Monotonic => "monotonic",
        flight_ipc::ValidationErrorType::Range => "range",
        flight_ipc::ValidationErrorType::Conflict => "conflict",
    }
}

async fn activate_profile(
    name: &str,
    output_format: OutputFormat,
    _verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    // Look up profile on disk
    let profile_dir = dirs::config_dir()
        .map(|d| d.join("OpenFlight").join("profiles"))
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;

    let profile_path = profile_dir.join(format!("{}.json", name));
    if !profile_path.exists() {
        return Err(anyhow::anyhow!("Profile '{}' not found", name));
    }

    let profile_content = fs::read_to_string(&profile_path)?;

    // Validate JSON format
    let _profile_json: Value = serde_json::from_str(&profile_content)
        .map_err(|e| anyhow::anyhow!("Invalid JSON in profile '{}': {}", name, e))?;

    let mut client = client_manager.get_client().await?;

    let request = ApplyProfileRequest {
        profile_json: profile_content,
        validate_only: false,
        force_apply: false,
    };

    let response = client.apply_profile(request).await?;

    if !response.success {
        return Err(anyhow::anyhow!(
            "Failed to activate profile '{}': {}",
            name,
            response.error_message
        ));
    }

    let result = json!({
        "profile_name": name,
        "activated": true,
        "effective_profile_hash": response.effective_profile_hash,
        "compile_time_ms": response.compile_time_ms,
        "message": format!("Profile '{}' activated successfully", name),
    });

    let output = output_format.success(result);
    Ok(Some(output))
}

async fn validate_profile(
    path: &std::path::Path,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let profile_content = fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read profile file '{}': {}", path.display(), e))?;

    // Validate JSON format locally first
    let _profile_json: Value = serde_json::from_str(&profile_content)
        .map_err(|e| anyhow::anyhow!("Invalid JSON in profile file: {}", e))?;

    let mut client = client_manager.get_client().await?;

    let request = ApplyProfileRequest {
        profile_json: profile_content,
        validate_only: true,
        force_apply: false,
    };

    let response = client.apply_profile(request).await?;

    if !response.success {
        let error_msg = if !response.validation_errors.is_empty() {
            let mut errors = vec!["Validation errors:".to_string()];
            for error in &response.validation_errors {
                errors.push(format!(
                    "  Line {}, Column {}: {} ({})",
                    error.line_number,
                    error.column_number,
                    error.error_message,
                    validation_error_type_to_string(error.error_type())
                ));
            }
            errors.join("\n")
        } else {
            response.error_message
        };

        return Err(anyhow::anyhow!("{}", error_msg));
    }

    let mut result = json!({
        "valid": true,
        "path": path.display().to_string(),
        "message": "Profile validation passed",
    });

    if verbose && !response.validation_errors.is_empty() {
        let warnings: Vec<Value> = response
            .validation_errors
            .iter()
            .map(|error| {
                json!({
                    "field_path": error.field_path,
                    "error_message": error.error_message,
                    "error_type": validation_error_type_to_string(error.error_type()),
                })
            })
            .collect();
        result["warnings"] = json!(warnings);
    }

    let output = output_format.success(result);
    Ok(Some(output))
}

async fn export_profile(
    name: &str,
    path: &std::path::Path,
    output_format: OutputFormat,
    _verbose: bool,
    _client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let profile_dir = dirs::config_dir()
        .map(|d| d.join("OpenFlight").join("profiles"))
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;

    let profile_path = profile_dir.join(format!("{}.json", name));
    if !profile_path.exists() {
        return Err(anyhow::anyhow!("Profile '{}' not found", name));
    }

    let content = fs::read_to_string(&profile_path)?;

    // Validate the content before exporting
    let _: Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Invalid JSON in profile '{}': {}", name, e))?;

    // Write to the export destination
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        return Err(anyhow::anyhow!(
            "Output directory '{}' does not exist",
            parent.display()
        ));
    }

    fs::write(path, &content)
        .map_err(|e| anyhow::anyhow!("Failed to write export file '{}': {}", path.display(), e))?;

    let result = json!({
        "profile_name": name,
        "exported_to": path.display().to_string(),
        "size_bytes": content.len(),
        "message": format!("Profile '{}' exported to '{}'", name, path.display()),
    });

    let output = output_format.success(result);
    Ok(Some(output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_error_type_covers_all_variants() {
        assert_eq!(
            validation_error_type_to_string(flight_ipc::ValidationErrorType::Unspecified),
            "unspecified"
        );
        assert_eq!(
            validation_error_type_to_string(flight_ipc::ValidationErrorType::Schema),
            "schema"
        );
        assert_eq!(
            validation_error_type_to_string(flight_ipc::ValidationErrorType::Monotonic),
            "monotonic"
        );
        assert_eq!(
            validation_error_type_to_string(flight_ipc::ValidationErrorType::Range),
            "range"
        );
        assert_eq!(
            validation_error_type_to_string(flight_ipc::ValidationErrorType::Conflict),
            "conflict"
        );
    }

    #[test]
    fn list_profiles_json_format() {
        let profiles = vec![
            json!({
                "name": "combat",
                "source": "user",
                "path": "/profiles/combat.json",
            }),
            json!({
                "name": "civilian",
                "source": "user",
                "path": "/profiles/civilian.json",
            }),
        ];
        let output = OutputFormat::Json.list(profiles, Some(2));
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["total_count"], 2);
        assert_eq!(parsed["data"][0]["name"], "combat");
        assert_eq!(parsed["data"][1]["name"], "civilian");
    }

    #[test]
    fn list_profiles_human_format() {
        let profiles = vec![json!({
            "name": "combat",
            "source": "user",
        })];
        let output = OutputFormat::Human.list(profiles, Some(1));
        assert!(output.contains("combat"));
        assert!(output.contains("user"));
    }

    #[test]
    fn export_result_json_format() {
        let result = json!({
            "profile_name": "test",
            "exported_to": "/tmp/test.json",
            "size_bytes": 1024,
            "message": "Profile 'test' exported to '/tmp/test.json'",
        });
        let output = OutputFormat::Json.success(result);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["profile_name"], "test");
        assert_eq!(parsed["data"]["size_bytes"], 1024);
    }

    #[test]
    fn activate_result_json_format() {
        let result = json!({
            "profile_name": "combat",
            "activated": true,
            "message": "Profile 'combat' activated successfully",
        });
        let output = OutputFormat::Json.success(result);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["activated"], true);
    }

    #[test]
    fn validate_result_json_format() {
        let result = json!({
            "valid": true,
            "path": "/tmp/profile.json",
            "message": "Profile validation passed",
        });
        let output = OutputFormat::Json.success(result);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["valid"], true);
    }
}
