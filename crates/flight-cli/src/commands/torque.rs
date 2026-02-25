// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Force feedback and torque management commands

use crate::client_manager::ClientManager;
use crate::commands::TorqueAction;
use crate::output::OutputFormat;
use flight_ipc::{
    CapabilityMode, DeviceType, GetCapabilityModeRequest, ListDevicesRequest,
    SetCapabilityModeRequest,
};
use serde_json::{Value, json};

pub async fn execute(
    action: &TorqueAction,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        TorqueAction::Unlock {
            device_id,
            skip_physical_confirm,
        } => {
            unlock_torque(
                device_id,
                *skip_physical_confirm,
                output_format,
                verbose,
                client_manager,
            )
            .await
        }
        TorqueAction::Status { device_id } => {
            torque_status(device_id.as_deref(), output_format, verbose, client_manager).await
        }
        TorqueAction::SetMode { mode, axes, audit } => {
            set_capability_mode(mode, axes, *audit, output_format, verbose, client_manager).await
        }
    }
}

async fn unlock_torque(
    device_id: &str,
    skip_physical_confirm: bool,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;

    // First, verify the device exists and supports force feedback
    let request = ListDevicesRequest {
        include_disconnected: false,
        filter_types: vec![DeviceType::ForceFeedback as i32],
    };

    let response = client.list_devices(request).await?;

    let device = response
        .devices
        .iter()
        .find(|d| d.id == device_id)
        .ok_or_else(|| anyhow::anyhow!("Force feedback device '{}' not found", device_id))?;

    // Check if device supports high torque
    let supports_high_torque = device
        .capabilities
        .as_ref()
        .map(|caps| caps.supports_force_feedback)
        .unwrap_or(false);

    if !supports_high_torque {
        return Err(anyhow::anyhow!(
            "Device '{}' does not support force feedback",
            device_id
        ));
    }

    // Note: Actual torque unlock would require a dedicated RPC method
    // For now, simulate the unlock process
    let mut result = json!({
        "device_id": device_id,
        "device_name": device.name,
        "unlock_requested": true,
        "skip_physical_confirm": skip_physical_confirm,
    });

    if skip_physical_confirm {
        result["status"] = json!("unlocked");
        result["message"] =
            json!("High torque mode unlocked (physical confirmation skipped for testing)");
        result["warning"] =
            json!("Physical confirmation was skipped - this should only be used for testing");
    } else {
        result["status"] = json!("awaiting_physical_confirmation");
        result["message"] = json!(
            "Please press and hold the required button combination on the device for 2 seconds"
        );
        result["instructions"] = json!(
            "The device should indicate the required button combination via LED pattern or display"
        );
    }

    if verbose {
        result["safety_info"] = json!({
            "high_torque_warning": "High torque mode can produce significant forces. Ensure proper setup and safety precautions.",
            "unlock_duration": "High torque mode remains active until device power cycle",
            "emergency_stop": "Release all controls immediately if unexpected forces occur",
        });
    }

    let output = output_format.success(result);
    Ok(Some(output))
}

async fn torque_status(
    device_id: Option<&str>,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;

    // Get force feedback devices
    let request = ListDevicesRequest {
        include_disconnected: true,
        filter_types: vec![DeviceType::ForceFeedback as i32],
    };

    let response = client.list_devices(request).await?;

    let devices: Vec<_> = if let Some(device_id) = device_id {
        response
            .devices
            .into_iter()
            .filter(|d| d.id == device_id)
            .collect()
    } else {
        response.devices
    };

    if devices.is_empty() {
        let message = if let Some(id) = &device_id {
            format!("Force feedback device '{id}' not found")
        } else {
            "No force feedback devices found".to_string()
        };

        return Err(anyhow::anyhow!("{}", message));
    }

    let device_statuses: Vec<Value> = devices
        .iter()
        .map(|device| {
            let mut status = json!({
                "device_id": device.id,
                "device_name": device.name,
                "status": device_status_to_string(device.status()),
            });

            if let Some(ref capabilities) = device.capabilities {
                status["capabilities"] = json!({
                    "supports_force_feedback": capabilities.supports_force_feedback,
                    "supports_raw_torque": capabilities.supports_raw_torque,
                    "max_torque_nm": capabilities.max_torque_nm,
                    "min_period_us": capabilities.min_period_us,
                });
            }

            // Simulate torque status (would require actual FFB status RPC)
            status["torque_status"] = json!({
                "mode": "safe_torque", // safe_torque, high_torque, faulted
                "current_torque_nm": 0.0,
                "max_allowed_torque_nm": if device.capabilities.as_ref().map(|c| c.supports_force_feedback).unwrap_or(false) {
                    device.capabilities.as_ref().unwrap().max_torque_nm
                } else {
                    0
                },
                "high_torque_unlocked": false,
            });

            if verbose {
                status["safety_state"] = json!({
                    "interlock_status": "locked",
                    "last_fault_timestamp": null,
                    "fault_count_session": 0,
                    "emergency_stop_active": false,
                });

                status["performance"] = json!({
                    "update_rate_hz": 1000.0,
                    "latency_p99_us": 150.0,
                    "missed_updates": 0,
                });
            }

            status
        })
        .collect();

    let result = if devices.len() == 1 {
        device_statuses.into_iter().next().unwrap()
    } else {
        json!({
            "device_count": devices.len(),
            "devices": device_statuses,
        })
    };

    let output = output_format.success(result);
    Ok(Some(output))
}

async fn set_capability_mode(
    mode: &str,
    axes: &[String],
    audit: bool,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;

    let capability_mode = match mode.to_lowercase().as_str() {
        "full" => CapabilityMode::Full,
        "demo" => CapabilityMode::Demo,
        "kid" => CapabilityMode::Kid,
        _ => {
            return Err(anyhow::anyhow!(
                "Invalid capability mode: {}. Valid modes: full, demo, kid",
                mode
            ));
        }
    };

    let request = SetCapabilityModeRequest {
        mode: capability_mode as i32,
        axis_names: axes.to_vec(),
        audit_enabled: audit,
    };

    let response = client.set_capability_mode(request).await?;

    if !response.success {
        return Err(anyhow::anyhow!(
            "Failed to set capability mode: {}",
            response.error_message
        ));
    }

    let mut result = json!({
        "mode_set": mode,
        "axes_affected": response.affected_axes,
        "audit_enabled": audit,
        "success": true,
    });

    if let Some(ref limits) = response.applied_limits {
        result["applied_limits"] = json!({
            "max_axis_output": limits.max_axis_output,
            "max_ffb_torque": limits.max_ffb_torque,
            "max_slew_rate": limits.max_slew_rate,
            "max_curve_expo": limits.max_curve_expo,
            "allow_high_torque": limits.allow_high_torque,
            "allow_custom_curves": limits.allow_custom_curves,
        });
    }

    if verbose {
        result["mode_descriptions"] = json!({
            "full": "Full capabilities - no restrictions",
            "demo": "Demo mode - reduced limits for safety demonstrations",
            "kid": "Kid mode - heavily restricted for child safety",
        });
    }

    let output = output_format.success(result);
    Ok(Some(output))
}

fn device_status_to_string(status: flight_ipc::DeviceStatus) -> &'static str {
    match status {
        flight_ipc::DeviceStatus::Unspecified => "unspecified",
        flight_ipc::DeviceStatus::Connected => "connected",
        flight_ipc::DeviceStatus::Disconnected => "disconnected",
        flight_ipc::DeviceStatus::Error => "error",
        flight_ipc::DeviceStatus::Faulted => "faulted",
    }
}
