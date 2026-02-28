// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Version information command with build metadata

use crate::client_manager::ClientManager;
use crate::output::OutputFormat;
use serde_json::json;

pub async fn execute(
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut result = json!({
        "cli_version": env!("CARGO_PKG_VERSION"),
        "build_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "build_target": std::env::consts::ARCH,
        "build_os": std::env::consts::OS,
        "rust_version": env!("CARGO_PKG_RUST_VERSION"),
    });

    if verbose {
        result["package_name"] = json!(env!("CARGO_PKG_NAME"));
        result["package_description"] = json!(env!("CARGO_PKG_DESCRIPTION"));
        result["package_homepage"] = json!(env!("CARGO_PKG_HOMEPAGE"));
        result["package_repository"] = json!(env!("CARGO_PKG_REPOSITORY"));
    }

    // Try to get service version
    match client_manager.get_client().await {
        Ok(mut client) => {
            if let Ok(info) = client.get_service_info().await {
                result["service_version"] = json!(info.version);
                result["service_status"] = json!(service_status_to_string(info.status()));

                if verbose {
                    result["protocol_version"] = json!(flight_ipc::PROTOCOL_VERSION);
                    result["ipc_features"] = json!(flight_ipc::SUPPORTED_FEATURES);
                }
            }
        }
        Err(_) => {
            result["service_version"] = json!(null);
            result["service_status"] = json!("unreachable");
        }
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
    fn version_json_format() {
        let result = json!({
            "cli_version": "0.1.0",
            "build_profile": "debug",
            "build_target": "x86_64",
            "build_os": "windows",
            "rust_version": "1.92.0",
            "service_version": null,
            "service_status": "unreachable",
        });
        let output = OutputFormat::Json.success(result);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert!(parsed["data"]["cli_version"].is_string());
        assert!(parsed["data"]["build_profile"].is_string());
    }

    #[test]
    fn version_human_format() {
        let result = json!({
            "cli_version": "0.1.0",
            "build_profile": "debug",
            "service_status": "unreachable",
        });
        let output = OutputFormat::Human.success(result);
        assert!(output.contains("0.1.0"));
    }
}
