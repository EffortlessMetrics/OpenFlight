// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! `flightctl metrics` — system-wide metrics snapshot.

use crate::client_manager::ClientManager;
use crate::commands::MetricsAction;
use crate::output::OutputFormat;
use serde_json::json;

pub async fn execute(
    action: &MetricsAction,
    output_format: OutputFormat,
    _verbose: bool,
    _client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        MetricsAction::Snapshot { reset } => snapshot(*reset, output_format).await,
    }
}

async fn snapshot(reset: bool, output_format: OutputFormat) -> anyhow::Result<Option<String>> {
    // In production this would call a GetMetrics RPC on the daemon.
    // For now, return the documented schema so the contract is established.
    let result = json!({
        "captured_at": chrono::Utc::now().to_rfc3339(),
        "reset_after_capture": reset,
        "sim": {
            "frames_total": 0,
            "errors_total": 0,
            "connected": false,
            "data_rate_hz": 0.0,
            "last_packet_age_ms": 0.0,
            "profile_switches_total": 0,
            "frame_latency_ms": null
        },
        "ffb": {
            "effects_applied_total": 0,
            "fault_count_total": 0,
            "envelope_clamp_total": 0,
            "emergency_stop_total": 0,
            "max_torque_nm": 0.0,
            "current_torque_nm": 0.0,
            "effect_latency_ms": null
        },
        "rt": {
            "ticks_total": 0,
            "missed_deadlines_total": 0,
            "jitter_us": null
        }
    });

    Ok(Some(output_format.success(result)))
}
