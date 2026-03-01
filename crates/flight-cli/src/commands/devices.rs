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
        DeviceAction::Dump { device_id } => {
            device_dump(device_id, output_format, verbose, client_manager).await
        }
        DeviceAction::Calibrate {
            device_id,
            non_interactive,
        } => {
            calibrate_device(
                device_id,
                *non_interactive,
                output_format,
                verbose,
                client_manager,
            )
            .await
        }
        DeviceAction::Test {
            device_id,
            interval_ms,
            count,
        } => {
            test_device(
                device_id,
                *interval_ms,
                *count,
                output_format,
                verbose,
                client_manager,
            )
            .await
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

    let output = match output_format {
        OutputFormat::Human => format_device_table(&devices),
        OutputFormat::Json => output_format.list(devices, Some(response.total_count)),
    };
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

async fn device_dump(
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

    if let Some(discovery) = metadata_json(device, "descriptor_discovery")? {
        device_json["descriptor_discovery"] = discovery;
    }

    if let Some(control_map) = metadata_json(device, "control_map")? {
        device_json["control_map"] = control_map;
    }

    if verbose && !device.metadata.is_empty() {
        device_json["metadata"] = json!(device.metadata);
    }

    let output = output_format.success(device_json);
    Ok(Some(output))
}

fn metadata_json(device: &flight_ipc::Device, key: &str) -> anyhow::Result<Option<Value>> {
    let raw = match device.metadata.get(key) {
        Some(value) => value,
        None => return Ok(None),
    };

    let parsed: Value = serde_json::from_str(raw).unwrap_or_else(|_| json!(raw));
    Ok(Some(parsed))
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

async fn calibrate_device(
    device_id: &str,
    non_interactive: bool,
    output_format: OutputFormat,
    _verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;

    // Verify device exists and is connected
    let request = ListDevicesRequest {
        include_disconnected: false,
        filter_types: vec![],
    };

    let response = client.list_devices(request).await?;

    let device = response
        .devices
        .iter()
        .find(|d| d.id == device_id)
        .ok_or_else(|| anyhow::anyhow!("Device '{}' not found or not connected", device_id))?;

    // Calibration requires a CalibrateDevice RPC; return the contract for now
    let result = json!({
        "device_id": device.id,
        "device_name": device.name,
        "device_type": device_type_to_string(device.r#type()),
        "calibration_started": true,
        "non_interactive": non_interactive,
        "message": "Calibration wizard started. Move all axes to their full range of motion.",
        "steps": [
            "Center all axes and press Enter",
            "Move each axis to its minimum position",
            "Move each axis to its maximum position",
            "Release all axes to center and press Enter to finish"
        ],
        "note": "Full calibration requires CalibrateDevice RPC to be implemented in the service"
    });

    let output = output_format.success(result);
    Ok(Some(output))
}

async fn test_device(
    device_id: &str,
    interval_ms: u64,
    count: Option<u64>,
    output_format: OutputFormat,
    _verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;

    // Verify device exists and is connected
    let request = ListDevicesRequest {
        include_disconnected: false,
        filter_types: vec![],
    };

    let response = client.list_devices(request).await?;

    let device = response
        .devices
        .iter()
        .find(|d| d.id == device_id)
        .ok_or_else(|| anyhow::anyhow!("Device '{}' not found or not connected", device_id))?;

    // Live test input display requires a SubscribeDeviceInput RPC
    let result = json!({
        "device_id": device.id,
        "device_name": device.name,
        "device_type": device_type_to_string(device.r#type()),
        "test_mode": true,
        "interval_ms": interval_ms,
        "sample_count": count,
        "axes": {},
        "buttons": {},
        "message": "Live input test started. Press Ctrl+C to stop.",
        "note": "Full live input display requires SubscribeDeviceInput RPC to be implemented in the service"
    });

    let output = output_format.success(result);
    Ok(Some(output))
}

/// Format a list of devices as an aligned table for human-readable output
pub fn format_device_table(devices: &[Value]) -> String {
    if devices.is_empty() {
        return "No devices found".to_string();
    }

    let header = format!("{:<30} {:<15} {:<12}", "NAME", "TYPE", "STATUS");
    let separator = "-".repeat(header.len());
    let mut lines = vec![header, separator];

    for device in devices {
        let name = device["name"].as_str().unwrap_or("unknown");
        let device_type = device["type"].as_str().unwrap_or("unknown");
        let status = device["status"].as_str().unwrap_or("unknown");
        lines.push(format!("{:<30} {:<15} {:<12}", name, device_type, status));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_type_to_string_covers_all_variants() {
        assert_eq!(
            device_type_to_string(DeviceType::Unspecified),
            "unspecified"
        );
        assert_eq!(device_type_to_string(DeviceType::Joystick), "joystick");
        assert_eq!(device_type_to_string(DeviceType::Throttle), "throttle");
        assert_eq!(device_type_to_string(DeviceType::Rudder), "rudder");
        assert_eq!(device_type_to_string(DeviceType::Panel), "panel");
        assert_eq!(
            device_type_to_string(DeviceType::ForceFeedback),
            "force-feedback"
        );
        assert_eq!(device_type_to_string(DeviceType::Streamdeck), "streamdeck");
    }

    #[test]
    fn device_status_to_string_covers_all_variants() {
        assert_eq!(
            device_status_to_string(flight_ipc::DeviceStatus::Unspecified),
            "unspecified"
        );
        assert_eq!(
            device_status_to_string(flight_ipc::DeviceStatus::Connected),
            "connected"
        );
        assert_eq!(
            device_status_to_string(flight_ipc::DeviceStatus::Disconnected),
            "disconnected"
        );
        assert_eq!(
            device_status_to_string(flight_ipc::DeviceStatus::Error),
            "error"
        );
        assert_eq!(
            device_status_to_string(flight_ipc::DeviceStatus::Faulted),
            "faulted"
        );
    }

    #[test]
    fn metadata_json_returns_none_for_missing_key() {
        let device = flight_ipc::Device::default();
        let result = metadata_json(&device, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn metadata_json_parses_valid_json_string() {
        let mut device = flight_ipc::Device::default();
        device
            .metadata
            .insert("test_key".to_string(), r#"{"foo":"bar"}"#.to_string());
        let result = metadata_json(&device, "test_key").unwrap().unwrap();
        assert_eq!(result["foo"], "bar");
    }

    #[test]
    fn metadata_json_wraps_non_json_as_string() {
        let mut device = flight_ipc::Device::default();
        device
            .metadata
            .insert("test_key".to_string(), "plain text".to_string());
        let result = metadata_json(&device, "test_key").unwrap().unwrap();
        assert_eq!(result, "plain text");
    }

    #[test]
    fn format_device_table_with_devices() {
        let devices = vec![
            json!({
                "name": "VKB Gladiator NXT",
                "type": "joystick",
                "status": "connected",
            }),
            json!({
                "name": "Virpil CM2 Throttle",
                "type": "throttle",
                "status": "connected",
            }),
        ];
        let table = format_device_table(&devices);
        assert!(table.contains("NAME"));
        assert!(table.contains("TYPE"));
        assert!(table.contains("STATUS"));
        assert!(table.contains("VKB Gladiator NXT"));
        assert!(table.contains("joystick"));
        assert!(table.contains("Virpil CM2 Throttle"));
        assert!(table.contains("throttle"));
    }

    #[test]
    fn format_device_table_empty() {
        let table = format_device_table(&[]);
        assert_eq!(table, "No devices found");
    }

    #[test]
    fn format_device_table_single_device() {
        let devices = vec![json!({
            "name": "Test Device",
            "type": "panel",
            "status": "disconnected",
        })];
        let table = format_device_table(&devices);
        assert!(table.contains("Test Device"));
        assert!(table.contains("panel"));
        assert!(table.contains("disconnected"));
        // Header, separator, one device row
        assert_eq!(table.lines().count(), 3);
    }
}
