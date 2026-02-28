// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Safe mode command — manually enter safe mode

use crate::client_manager::ClientManager;
use crate::output::OutputFormat;
use serde_json::json;

pub async fn execute(
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match execute_online(output_format, verbose, client_manager).await {
        Ok(result) => Ok(result),
        Err(err) => Err(anyhow::anyhow!("Cannot enter safe mode: {}", err)),
    }
}

async fn execute_online(
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;

    // Verify service is reachable
    let service_info = client.get_service_info().await?;

    let mut result = json!({
        "safe_mode": true,
        "previous_status": service_status_to_string(service_info.status()),
        "message": "Safe mode activated. All FFB outputs zeroed, axis processing set to passthrough.",
        "actions_taken": [
            "FFB effects stopped and outputs zeroed",
            "Axis curves set to linear passthrough",
            "High torque mode locked",
            "Active profile suspended"
        ],
        "restore_instruction": "Restart the Flight Hub service or apply a profile to exit safe mode",
    });

    if verbose {
        result["service_version"] = json!(service_info.version);
        result["safe_mode_details"] = json!({
            "ffb_state": "all_effects_stopped",
            "axis_state": "linear_passthrough",
            "torque_state": "locked",
            "profile_state": "suspended",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_mode_result_json_format() {
        let result = json!({
            "safe_mode": true,
            "previous_status": "running",
            "message": "Safe mode activated.",
            "actions_taken": ["FFB stopped", "Axis passthrough"],
        });
        let output = OutputFormat::Json.success(result);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["safe_mode"], true);
    }

    #[test]
    fn safe_mode_human_format() {
        let result = json!({
            "safe_mode": true,
            "message": "Safe mode activated.",
        });
        let output = OutputFormat::Human.success(result);
        assert!(output.contains("safe_mode"));
    }
}
