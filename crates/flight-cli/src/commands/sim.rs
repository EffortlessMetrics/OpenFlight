//! Simulator configuration commands

use crate::commands::{SimAction, SimConfigAction};
use crate::output::OutputFormat;
use crate::client_manager::ClientManager;
use flight_ipc::{
    DetectCurveConflictsRequest, ResolveCurveConflictRequest, OneClickResolveRequest,
    ResolutionAction, ResolutionType,
};
use serde_json::{json, Value};

pub async fn execute(
    action: &SimAction,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        SimAction::Configure { sim_type, action } => {
            configure_sim(sim_type, action, output_format, verbose, client_manager).await
        }
        SimAction::DetectConflicts { axes, sim_id, aircraft_id } => {
            detect_conflicts(axes, sim_id.as_deref(), aircraft_id.as_deref(), output_format, verbose, client_manager).await
        }
        SimAction::ResolveConflict { axis_name, resolution_type, apply_immediately, create_backup } => {
            resolve_conflict(axis_name, resolution_type, *apply_immediately, *create_backup, output_format, verbose, client_manager).await
        }
        SimAction::OneClickResolve { axis_name, create_backup, verify_resolution } => {
            one_click_resolve(axis_name, *create_backup, *verify_resolution, output_format, verbose, client_manager).await
        }
    }
}

async fn configure_sim(
    sim_type: &str,
    action: &SimConfigAction,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        SimConfigAction::Verify => {
            let result = json!({
                "sim_type": sim_type,
                "action": "verify",
                "message": "Sim configuration verification requires writers system implementation",
            });
            
            let output = output_format.success(result);
            Ok(Some(output))
        }
        SimConfigAction::Repair { auto_apply } => {
            let result = json!({
                "sim_type": sim_type,
                "action": "repair",
                "auto_apply": auto_apply,
                "message": "Sim configuration repair requires writers system implementation",
            });
            
            let output = output_format.success(result);
            Ok(Some(output))
        }
        SimConfigAction::Rollback { backup_id } => {
            let result = json!({
                "sim_type": sim_type,
                "action": "rollback",
                "backup_id": backup_id,
                "message": "Sim configuration rollback requires writers system implementation",
            });
            
            let output = output_format.success(result);
            Ok(Some(output))
        }
    }
}

async fn detect_conflicts(
    axes: &[String],
    sim_id: Option<&str>,
    aircraft_id: Option<&str>,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;
    
    let request = DetectCurveConflictsRequest {
        axis_names: axes.to_vec(),
        sim_id: sim_id.unwrap_or("").to_string(),
        aircraft_id: aircraft_id.unwrap_or("").to_string(),
    };
    
    let response = client.detect_curve_conflicts(request).await?;
    
    if !response.success {
        return Err(anyhow::anyhow!("Conflict detection failed: {}", response.error_message));
    }
    
    if response.conflicts.is_empty() {
        let result = json!({
            "conflicts_found": false,
            "message": "No curve conflicts detected",
            "axes_checked": axes,
        });
        
        let output = output_format.success(result);
        return Ok(Some(output));
    }
    
    let conflicts: Vec<Value> = response.conflicts
        .iter()
        .map(|conflict| {
            let mut conflict_json = json!({
                "axis_name": conflict.axis_name,
                "conflict_type": conflict_type_to_string(conflict.conflict_type()),
                "severity": conflict_severity_to_string(conflict.severity()),
                "description": conflict.description,
            });
            
            if verbose {
                if let Some(ref metadata) = conflict.metadata {
                    conflict_json["metadata"] = json!({
                        "sim_curve_strength": metadata.sim_curve_strength,
                        "profile_curve_strength": metadata.profile_curve_strength,
                        "combined_nonlinearity": metadata.combined_nonlinearity,
                        "detection_timestamp": metadata.detection_timestamp,
                    });
                }
                
                let resolutions: Vec<Value> = conflict.suggested_resolutions
                    .iter()
                    .map(|resolution| json!({
                        "resolution_type": resolution_type_to_string(resolution.resolution_type()),
                        "description": resolution.description,
                        "estimated_improvement": resolution.estimated_improvement,
                        "requires_sim_restart": resolution.requires_sim_restart,
                    }))
                    .collect();
                
                conflict_json["suggested_resolutions"] = json!(resolutions);
            }
            
            conflict_json
        })
        .collect();
    
    let result = json!({
        "conflicts_found": true,
        "conflict_count": conflicts.len(),
        "conflicts": conflicts,
    });
    
    let output = output_format.success(result);
    Ok(Some(output))
}

async fn resolve_conflict(
    axis_name: &str,
    resolution_type: &str,
    apply_immediately: bool,
    create_backup: bool,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;
    
    let resolution_type_enum = match resolution_type.to_lowercase().as_str() {
        "disable-sim-curve" => ResolutionType::DisableSimCurve,
        "disable-profile-curve" => ResolutionType::DisableProfileCurve,
        "gain-compensation" => ResolutionType::ApplyGainCompensation,
        "reduce-curve-strength" => ResolutionType::ReduceCurveStrength,
        _ => return Err(anyhow::anyhow!("Invalid resolution type: {}", resolution_type)),
    };
    
    let resolution_action = ResolutionAction {
        r#type: resolution_type_enum as i32,
        parameters: std::collections::HashMap::new(),
        affected_files: vec![],
        backup_info: "".to_string(),
    };
    
    let request = ResolveCurveConflictRequest {
        axis_name: axis_name.to_string(),
        resolution: Some(resolution_action),
        apply_immediately,
        create_backup,
    };
    
    let response = client.resolve_curve_conflict(request).await?;
    
    if !response.success {
        return Err(anyhow::anyhow!("Conflict resolution failed: {}", response.error_message));
    }
    
    let mut result = json!({
        "axis_name": axis_name,
        "resolution_applied": resolution_type,
        "apply_immediately": apply_immediately,
        "create_backup": create_backup,
        "success": true,
    });
    
    if let Some(ref resolution_result) = response.result {
        result["applied_resolution"] = json!(resolution_type_to_string(resolution_result.applied_resolution()));
        result["modified_files"] = json!(resolution_result.modified_files);
        result["backup_path"] = json!(resolution_result.backup_path);
        result["verification_passed"] = json!(resolution_result.verification_passed);
        
        if verbose {
            result["verification_details"] = json!(resolution_result.verification_details);
        }
    }
    
    let output = output_format.success(result);
    Ok(Some(output))
}

async fn one_click_resolve(
    axis_name: &str,
    create_backup: bool,
    verify_resolution: bool,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;
    
    let request = OneClickResolveRequest {
        axis_name: axis_name.to_string(),
        create_backup,
        verify_resolution,
    };
    
    let response = client.one_click_resolve(request).await?;
    
    if !response.success {
        return Err(anyhow::anyhow!("One-click resolution failed: {}", response.error_message));
    }
    
    let mut result = json!({
        "axis_name": axis_name,
        "create_backup": create_backup,
        "verify_resolution": verify_resolution,
        "success": true,
    });
    
    if let Some(ref one_click_result) = response.result {
        result["resolution_type"] = json!(resolution_type_to_string(one_click_result.resolution_type()));
        result["modified_files"] = json!(one_click_result.modified_files);
        
        if let Some(ref backup_info) = one_click_result.backup_info {
            result["backup_info"] = json!({
                "timestamp": backup_info.timestamp,
                "description": backup_info.description,
                "backup_dir": backup_info.backup_dir,
            });
        }
        
        if let Some(ref verification) = one_click_result.verification {
            result["verification"] = json!({
                "passed": verification.passed,
                "duration_ms": verification.duration_ms,
                "conflict_resolved": verification.conflict_resolved,
            });
            
            if verbose {
                result["verification"]["details"] = json!(verification.details);
            }
        }
        
        if verbose && let Some(ref metrics) = one_click_result.metrics {
            result["metrics"] = json!({
                "improvement": metrics.improvement,
            });
            
            let steps: Vec<Value> = one_click_result.steps_performed
                .iter()
                .map(|step| json!({
                    "name": step.name,
                    "description": step.description,
                    "success": step.success,
                    "duration_ms": step.duration_ms,
                    "error": step.error,
                }))
                .collect();
            
            result["steps_performed"] = json!(steps);
        }
    }
    
    let output = output_format.success(result);
    Ok(Some(output))
}

fn conflict_type_to_string(conflict_type: flight_ipc::ConflictType) -> &'static str {
    match conflict_type {
        flight_ipc::ConflictType::Unspecified => "unspecified",
        flight_ipc::ConflictType::DoubleCurve => "double-curve",
        flight_ipc::ConflictType::ExcessiveNonlinearity => "excessive-nonlinearity",
        flight_ipc::ConflictType::OpposingCurves => "opposing-curves",
    }
}

fn conflict_severity_to_string(severity: flight_ipc::ConflictSeverity) -> &'static str {
    match severity {
        flight_ipc::ConflictSeverity::Unspecified => "unspecified",
        flight_ipc::ConflictSeverity::Low => "low",
        flight_ipc::ConflictSeverity::Medium => "medium",
        flight_ipc::ConflictSeverity::High => "high",
        flight_ipc::ConflictSeverity::Critical => "critical",
    }
}

fn resolution_type_to_string(resolution_type: flight_ipc::ResolutionType) -> &'static str {
    match resolution_type {
        flight_ipc::ResolutionType::Unspecified => "unspecified",
        flight_ipc::ResolutionType::DisableSimCurve => "disable-sim-curve",
        flight_ipc::ResolutionType::DisableProfileCurve => "disable-profile-curve",
        flight_ipc::ResolutionType::ApplyGainCompensation => "gain-compensation",
        flight_ipc::ResolutionType::ReduceCurveStrength => "reduce-curve-strength",
    }
}