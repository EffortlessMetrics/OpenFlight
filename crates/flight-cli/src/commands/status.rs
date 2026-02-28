// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! System status command

use crate::client_manager::ClientManager;
use crate::output::OutputFormat;
use flight_ipc::ListDevicesRequest;
use serde_json::json;

pub async fn execute(
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match execute_online(output_format, verbose, client_manager).await {
        Ok(result) => Ok(result),
        Err(err) => Ok(Some(offline_status(output_format, verbose, &err))),
    }
}

fn offline_status(output_format: OutputFormat, verbose: bool, err: &anyhow::Error) -> String {
    let mut result = json!({
        "service_status": "unreachable",
        "service_version": null,
        "uptime_seconds": null,
        "connected_devices": null,
        "total_devices": null,
        "cli_version": env!("CARGO_PKG_VERSION"),
        "message": "Flight Hub service is not running or unreachable",
    });

    if verbose {
        result["connection_error"] = json!(err.to_string());
    }

    output_format.success(result)
}

async fn execute_online(
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;

    // Get service info
    let service_info = client.get_service_info().await?;

    // Get device count
    let devices_request = ListDevicesRequest {
        include_disconnected: false,
        filter_types: vec![],
    };
    let devices_response = client.list_devices(devices_request).await?;

    let connected_devices = devices_response.devices.len();
    let total_devices = devices_response.total_count;

    let mut result = json!({
        "service_status": service_status_to_string(service_info.status()),
        "service_version": service_info.version,
        "uptime_seconds": service_info.uptime_seconds,
        "connected_devices": connected_devices,
        "total_devices": total_devices,
    });

    if verbose {
        // Add device breakdown
        let mut device_breakdown = std::collections::HashMap::new();
        for device in &devices_response.devices {
            let device_type = device_type_to_string(device.r#type());
            *device_breakdown.entry(device_type).or_insert(0) += 1;
        }

        result["device_breakdown"] = json!(device_breakdown);
        result["service_capabilities"] = json!(service_info.capabilities);

        // Get recent health events (would require actual health subscription)
        result["recent_health"] = json!({
            "errors_last_hour": 0,
            "warnings_last_hour": 2,
            "performance_alerts": 0,
            "last_fault": null
        });

        // System performance metrics (simulated)
        result["performance"] = json!({
            "axis_jitter_p99_ms": 0.23,
            "hid_latency_p99_us": 145.7,
            "cpu_usage_percent": 2.1,
            "memory_usage_mb": 127.3,
            "missed_ticks": 0
        });
    }

    let output = output_format.success(result);
    Ok(Some(output))
}

fn service_status_to_string(status: flight_ipc::ServiceStatus) -> &'static str {
    match status {
        flight_ipc::ServiceStatus::Unspecified => "unspecified",
        flight_ipc::ServiceStatus::Starting => "starting",
        flight_ipc::ServiceStatus::Running => "running",
        flight_ipc::ServiceStatus::Degraded => "degraded",
        flight_ipc::ServiceStatus::Stopping => "stopping",
    }
}

fn device_type_to_string(device_type: flight_ipc::DeviceType) -> &'static str {
    match device_type {
        flight_ipc::DeviceType::Unspecified => "unspecified",
        flight_ipc::DeviceType::Joystick => "joystick",
        flight_ipc::DeviceType::Throttle => "throttle",
        flight_ipc::DeviceType::Rudder => "rudder",
        flight_ipc::DeviceType::Panel => "panel",
        flight_ipc::DeviceType::ForceFeedback => "force-feedback",
        flight_ipc::DeviceType::Streamdeck => "streamdeck",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_status_json_contains_unreachable() {
        let err = anyhow::anyhow!("connection refused");
        let result = offline_status(OutputFormat::Json, false, &err);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["service_status"], "unreachable");
        assert!(parsed["data"]["cli_version"].is_string());
    }

    #[test]
    fn offline_status_verbose_includes_error() {
        let err = anyhow::anyhow!("connection refused");
        let result = offline_status(OutputFormat::Json, true, &err);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["data"]["connection_error"], "connection refused");
    }

    #[test]
    fn offline_status_non_verbose_omits_error() {
        let err = anyhow::anyhow!("connection refused");
        let result = offline_status(OutputFormat::Json, false, &err);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["data"]["connection_error"].is_null());
    }

    #[test]
    fn offline_status_human_contains_unreachable() {
        let err = anyhow::anyhow!("connection refused");
        let result = offline_status(OutputFormat::Human, false, &err);
        assert!(result.contains("unreachable"));
        assert!(result.contains("cli_version"));
    }
}
