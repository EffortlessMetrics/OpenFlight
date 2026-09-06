// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Simulator adapter management commands.
//!
//! The current daemon IPC contract does not expose adapter management RPCs.
//! These commands therefore fail explicitly instead of reporting synthetic
//! state changes that never reached a backing subsystem.

use crate::client_manager::ClientManager;
use crate::commands::AdaptersAction;
use crate::output::OutputFormat;

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
    _output_format: OutputFormat,
    _verbose: bool,
    _client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    anyhow::bail!(
        "Adapter status is unavailable from the current daemon IPC contract: adapter status RPCs are not implemented"
    )
}

async fn toggle_adapter(
    sim: &str,
    enable: bool,
    _output_format: OutputFormat,
    _verbose: bool,
    _client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    validate_sim_id(sim)?;

    let rpc = if enable {
        "EnableAdapter"
    } else {
        "DisableAdapter"
    };
    let action = if enable { "enabled" } else { "disabled" };

    anyhow::bail!(
        "Adapter '{sim}' was not {action}: {rpc} RPC is not implemented in the current daemon IPC contract"
    )
}

async fn reconnect_adapter(
    sim: &str,
    _output_format: OutputFormat,
    _verbose: bool,
    _client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    validate_sim_id(sim)?;

    anyhow::bail!(
        "Adapter '{sim}' was not reconnected: ReconnectAdapter RPC is not implemented in the current daemon IPC contract"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_ipc::ClientConfig;

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
        let error = result.expect_err("unknown simulator must fail").to_string();
        assert!(error.contains("Unknown simulator"));
        assert!(error.contains("msfs"));
    }

    #[tokio::test]
    async fn status_does_not_report_synthetic_adapter_state() {
        let client = ClientManager::new(ClientConfig::default());
        let error = adapter_status(OutputFormat::Json, false, &client)
            .await
            .expect_err("adapter status must be unavailable until backed by RPC");

        assert!(error.to_string().contains("not implemented"));
    }

    #[tokio::test]
    async fn enable_does_not_report_an_unperformed_state_change() {
        let client = ClientManager::new(ClientConfig::default());
        let error = toggle_adapter("msfs", true, OutputFormat::Json, false, &client)
            .await
            .expect_err("adapter enable must not be simulated");
        let message = error.to_string();

        assert!(message.contains("was not enabled"));
        assert!(message.contains("EnableAdapter RPC"));
    }

    #[tokio::test]
    async fn reconnect_does_not_report_an_unperformed_state_change() {
        let client = ClientManager::new(ClientConfig::default());
        let error = reconnect_adapter("dcs", OutputFormat::Json, false, &client)
            .await
            .expect_err("adapter reconnect must not be simulated");
        let message = error.to_string();

        assert!(message.contains("was not reconnected"));
        assert!(message.contains("ReconnectAdapter RPC"));
    }
}
