//! Profile management commands

use crate::commands::ProfileAction;
use crate::output::OutputFormat;
use crate::client_manager::ClientManager;
use flight_ipc::ApplyProfileRequest;
use serde_json::{json, Value};
use std::fs;

pub async fn execute(
    action: &ProfileAction,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        ProfileAction::Apply { profile_path, validate_only, force } => {
            apply_profile(profile_path, *validate_only, *force, output_format, verbose, client_manager).await
        }
        ProfileAction::Show { raw } => {
            show_profile(*raw, output_format, verbose, client_manager).await
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
    let profile_content = fs::read_to_string(profile_path)
        .map_err(|e| anyhow::anyhow!("Failed to read profile file '{}': {}", profile_path.display(), e))?;
    
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
        let validation_errors: Vec<Value> = response.validation_errors
            .iter()
            .map(|error| json!({
                "field_path": error.field_path,
                "line_number": error.line_number,
                "column_number": error.column_number,
                "error_message": error.error_message,
                "error_type": validation_error_type_to_string(error.error_type()),
            }))
            .collect();
        
        result["validation_warnings"] = json!(validation_errors);
    }
    
    let output = output_format.success(result);
    Ok(Some(output))
}

async fn show_profile(
    raw: bool,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    // Note: This would require a GetCurrentProfile RPC method in the service
    // For now, return a placeholder indicating this functionality needs to be implemented
    let result = json!({
        "message": "Show profile functionality requires GetCurrentProfile RPC method to be implemented in the service",
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