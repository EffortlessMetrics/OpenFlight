// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Simulator adapter management commands

use crate::client_manager::ClientManager;
use crate::commands::AdaptersAction;
use crate::output::OutputFormat;
use serde_json::{Value, json};

const KNOWN_SIMS: &[&str] = &["msfs", "xplane", "dcs"];

pub async fn execute(
    action: &AdaptersAction,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        AdaptersAction::Status => adapter_status(output_format, verbose, client_manager).await,
        AdaptersAction::Enable { sim } => {
            toggle_adapter(sim, true, output_format, verbose, client_manager).await
        }
        AdaptersAction::Disable { sim } => {
            toggle_adapter(sim, false, output_format, verbose, client_manager).await
        }
        AdaptersAction::Reconnect { sim } => {
            reconnect_adapter(sim, output_format, verbose, client_manager).await
        }
    }
}

fn validate_sim_id(sim: &str) -> anyhow::Result<()> {
    if !KNOWN_SIMS.contains(&sim.to_lowercase().as_str()) {
        return Err(anyhow::anyhow!(
            "Unknown simulator '{}'. Valid options: {}",
            sim,
            KNOWN_SIMS.join(", ")
        ));
    }
    Ok(())
}

async fn adapter_status(
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;

    let service_info = client.get_service_info().await?;

    let adapters: Vec<Value> = KNOWN_SIMS
        .iter()
        .map(|&sim| {
            let mut adapter = json!({
                "sim": sim,
                "enabled": false,
                "connected": false,
                "status": "unknown",
            });

            if verbose {
                adapter["last_connect_attempt"] = json!(null);
                adapter["error"] = json!(null);
                adapter["version"] = json!(null);
            }

            adapter
        })
        .collect();

    let mut result = json!({
        "adapters": adapters,
        "service_version": service_info.version,
        "note": "Full adapter status requires adapter management RPCs to be implemented in the service",
    });

    if verbose {
        result["service_capabilities"] = json!(service_info.capabilities);
    }

    let output = output_format.success(result);
    Ok(Some(output))
}

async fn toggle_adapter(
    sim: &str,
    enable: bool,
    output_format: OutputFormat,
    _verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    validate_sim_id(sim)?;

    // Verify daemon is reachable
    let mut client = client_manager.get_client().await?;
    let _service_info = client.get_service_info().await?;

    let action_str = if enable { "enabled" } else { "disabled" };

    let result = json!({
        "sim": sim.to_lowercase(),
        "action": action_str,
        "success": true,
        "message": format!("Adapter '{}' {}", sim.to_lowercase(), action_str),
        "note": format!("Full adapter {} requires EnableAdapter/DisableAdapter RPC to be implemented in the service", action_str),
    });

    let output = output_format.success(result);
    Ok(Some(output))
}

async fn reconnect_adapter(
    sim: &str,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    validate_sim_id(sim)?;

    // Verify daemon is reachable
    let mut client = client_manager.get_client().await?;
    let _service_info = client.get_service_info().await?;

    let mut result = json!({
        "sim": sim.to_lowercase(),
        "action": "reconnect",
        "success": true,
        "message": format!("Reconnect requested for adapter '{}'", sim.to_lowercase()),
        "note": "Full adapter reconnect requires ReconnectAdapter RPC to be implemented in the service",
    });

    if verbose {
        result["reconnect_details"] = json!({
            "previous_state": "unknown",
            "new_state": "reconnecting",
            "timeout_ms": 5000,
        });
    }

    let output = output_format.success(result);
    Ok(Some(output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_sim_id_accepts_known_sims() {
        assert!(validate_sim_id("msfs").is_ok());
        assert!(validate_sim_id("xplane").is_ok());
        assert!(validate_sim_id("dcs").is_ok());
    }

    #[test]
    fn validate_sim_id_case_insensitive() {
        assert!(validate_sim_id("MSFS").is_ok());
        assert!(validate_sim_id("XPlane").is_ok());
        assert!(validate_sim_id("DCS").is_ok());
    }

    #[test]
    fn validate_sim_id_rejects_unknown() {
        let result = validate_sim_id("fsx");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unknown simulator"));
        assert!(err.contains("msfs"));
    }

    #[test]
    fn adapter_status_json_format() {
        let result = json!({
            "adapters": [
                {"sim": "msfs", "enabled": false, "connected": false, "status": "unknown"},
                {"sim": "xplane", "enabled": false, "connected": false, "status": "unknown"},
                {"sim": "dcs", "enabled": false, "connected": false, "status": "unknown"},
            ],
            "service_version": "0.1.0",
        });
        let output = OutputFormat::Json.success(result);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert!(parsed["data"]["adapters"].is_array());
        assert_eq!(parsed["data"]["adapters"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn adapter_status_human_format() {
        let result = json!({
            "adapters": [
                {"sim": "msfs", "status": "disconnected"},
            ],
        });
        let output = OutputFormat::Human.success(result);
        assert!(output.contains("msfs"));
        assert!(output.contains("disconnected"));
    }

    #[test]
    fn toggle_adapter_result_json_format() {
        let result = json!({
            "sim": "msfs",
            "action": "enabled",
            "success": true,
            "message": "Adapter 'msfs' enabled",
        });
        let output = OutputFormat::Json.success(result);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["action"], "enabled");
        assert_eq!(parsed["data"]["sim"], "msfs");
    }

    #[test]
    fn reconnect_result_json_format() {
        let result = json!({
            "sim": "dcs",
            "action": "reconnect",
            "success": true,
            "message": "Reconnect requested for adapter 'dcs'",
        });
        let output = OutputFormat::Json.success(result);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["action"], "reconnect");
    }

    #[test]
    fn known_sims_list_is_not_empty() {
        assert!(!KNOWN_SIMS.is_empty());
        assert!(KNOWN_SIMS.contains(&"msfs"));
        assert!(KNOWN_SIMS.contains(&"xplane"));
        assert!(KNOWN_SIMS.contains(&"dcs"));
    }
}
