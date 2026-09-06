// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! `flightctl metrics` — system-wide metrics snapshot.

use crate::client_manager::ClientManager;
use crate::commands::MetricsAction;
use crate::output::OutputFormat;

pub async fn execute(
    action: &MetricsAction,
    _output_format: OutputFormat,
    _verbose: bool,
    _client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        MetricsAction::Snapshot { reset } => snapshot(*reset).await,
    }
}

async fn snapshot(reset: bool) -> anyhow::Result<Option<String>> {
    let reset_note = if reset {
        " The requested reset was not performed."
    } else {
        ""
    };

    anyhow::bail!(
        "System-wide metrics are unavailable from the current daemon IPC contract: GetMetrics RPC is not implemented.{reset_note}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn snapshot_fails_explicitly_until_metrics_rpc_exists() {
        let error = snapshot(false)
            .await
            .expect_err("metrics snapshot must not report synthetic success");
        let message = error.to_string();

        assert!(message.contains("unavailable"));
        assert!(message.contains("GetMetrics RPC"));
    }

    #[tokio::test]
    async fn reset_request_is_not_reported_as_completed() {
        let error = snapshot(true)
            .await
            .expect_err("unsupported reset must not report success");

        assert!(error.to_string().contains("reset was not performed"));
    }
}
