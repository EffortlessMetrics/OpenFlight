// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Device management commands

use crate::client_manager::ClientManager;
use crate::commands::DeviceAction;
use crate::output::{OutputFormat, proto_to_json};
use flight_ipc::{DeviceType, ListDevicesRequest};
use serde_json::{Value, json};

pub async fn execute(
    action: &DeviceAction,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        DeviceAction::List {
            include_disconnected,
            filter_types,
        } => {
            list_devices(
                include_disconnected,
                filter_types,
                output_format,
                verbose,
                client_manager,
            )
            .await
        }
        DeviceAction::Info { device_id } => {
            device_info(device_id, output_format, verbose, client_manager).await
        }
    }
}

async fn list_devices(
    include_disconnected: &bool,
    filter_types: &[String],
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;

    // Convert string filter types to enum values
    let device_types: Vec<DeviceType> = filter_types
        .iter()
        .filter_map(|s| match s.to_lowercase().as_str() {
            "joystick" => Some(DeviceType::Joystick),
            "throttle" => Some(DeviceType::Throttle),
            "rudder" => Some(DeviceType::Rudder),
            "panel" => Some(DeviceType::Panel),
            "force-feedback" | "ffb" => Some(DeviceType::ForceFeedback),
            "streamdeck" => Some(DeviceType::Streamdeck),
            _ => None,
        })
        .collect();

    let request = ListDevicesRequest {
        include_disconnected: *include_disconnected,
        filter_types: device_types.into_iter().map(|t| t as i32).collect(),
    };

    let response = client.list_devices(request).await?;

    if response.devices.is_empty() {
        match output_format {
            OutputFormat::Json => {
                return Ok(Some(
                    json!({
                        "success": true,
                        "data": [],
                        "total_count": 0,
                        "message": "No devices found"
                    })
                    .to_string(),
                ));
            }
            OutputFormat::Human => {
                return Ok(Some("No devices found".to_string()));
            }
        }
    }

    let devices: Vec<Value> = response
        .devices
        .iter()
        .map(|device| {
            let mut device_json = json!({
                "id": device.id,
                "name": device.name,
                "type": device_type_to_string(device.r#type()),
                "status": device_status_to_string(device.status()),
            });

            let warnings: Vec<String> = device
                .metadata
                .iter()
                .filter_map(|(key, value)| {
                    if key.starts_with("warning.") {
                        Some(value.clone())
                    } else {
                        None
                    }
                })
                .collect();

            if !warnings.is_empty() {
                device_json["warnings"] = json!(warnings);
            }

            if verbose {
                if let Some(ref capabilities) = device.capabilities {
                    device_json["capabilities"] = json!({
                        "supports_force_feedback": capabilities.supports_force_feedback,
                        "supports_raw_torque": capabilities.supports_raw_torque,
                        "max_torque_nm": capabilities.max_torque_nm,
                        "min_period_us": capabilities.min_period_us,
                        "has_health_stream": capabilities.has_health_stream,
                        "supported_protocols": capabilities.supported_protocols,
                    });
                }

                if let Some(ref health) = device.health {
                    device_json["health"] = json!({
                        "temperature_celsius": health.temperature_celsius,
                        "current_amperes": health.current_amperes,
                        "packet_loss_count": health.packet_loss_count,
                        "last_seen_timestamp": health.last_seen_timestamp,
                        "active_faults": health.active_faults,
                    });
                }

                if !device.metadata.is_empty() {
                    device_json["metadata"] = json!(device.metadata);
                }
            }

            device_json
        })
        .collect();

    let output = output_format.list(devices, Some(response.total_count));
    Ok(Some(output))
}

async fn device_info(
    device_id: &str,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;

    let request = ListDevicesRequest {
        include_disconnected: true,
        filter_types: vec![],
    };

    let response = client.list_devices(request).await?;

    let device = response
        .devices
        .iter()
        .find(|d| d.id == device_id)
        .ok_or_else(|| anyhow::anyhow!("Device '{}' not found", device_id))?;

    let mut device_json = json!({
        "id": device.id,
        "name": device.name,
        "type": device_type_to_string(device.r#type()),
        "status": device_status_to_string(device.status()),
    });

    let warnings: Vec<String> = device
        .metadata
        .iter()
        .filter_map(|(key, value)| {
            if key.starts_with("warning.") {
                Some(value.clone())
            } else {
                None
            }
        })
        .collect();

    if !warnings.is_empty() {
        device_json["warnings"] = json!(warnings);
    }

    // Always include detailed info for device info command
    if let Some(ref capabilities) = device.capabilities {
        device_json["capabilities"] = json!({
            "supports_force_feedback": capabilities.supports_force_feedback,
            "supports_raw_torque": capabilities.supports_raw_torque,
            "max_torque_nm": capabilities.max_torque_nm,
            "min_period_us": capabilities.min_period_us,
            "has_health_stream": capabilities.has_health_stream,
            "supported_protocols": capabilities.supported_protocols,
        });
    }

    if let Some(ref health) = device.health {
        device_json["health"] = json!({
            "temperature_celsius": health.temperature_celsius,
            "current_amperes": health.current_amperes,
            "packet_loss_count": health.packet_loss_count,
            "last_seen_timestamp": health.last_seen_timestamp,
            "active_faults": health.active_faults,
        });
    }

    if !device.metadata.is_empty() {
        device_json["metadata"] = json!(device.metadata);
    }

    let output = output_format.success(device_json);
    Ok(Some(output))
}

fn device_type_to_string(device_type: DeviceType) -> &'static str {
    match device_type {
        DeviceType::Unspecified => "unspecified",
        DeviceType::Joystick => "joystick",
        DeviceType::Throttle => "throttle",
        DeviceType::Rudder => "rudder",
        DeviceType::Panel => "panel",
        DeviceType::ForceFeedback => "force-feedback",
        DeviceType::Streamdeck => "streamdeck",
    }
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
