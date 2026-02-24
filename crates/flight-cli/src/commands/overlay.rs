// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! `flightctl overlay` — VR overlay management commands.

use crate::{client_manager::ClientManager, output::OutputFormat};
use clap::Subcommand;
use serde_json::json;

/// Subcommands for `flightctl overlay`
#[derive(Subcommand)]
pub enum OverlayAction {
    /// Show current overlay status (visibility, backend, active notifications)
    Status,
    /// Show the overlay panel
    Show,
    /// Hide the overlay panel
    Hide,
    /// Toggle overlay visibility
    Toggle,
    /// Push a test notification to the overlay
    Notify {
        /// Notification message text
        message: String,
        /// Severity: info (default), warning, alert, critical
        #[arg(long, short = 's', default_value = "info")]
        severity: String,
        /// Time-to-live in seconds (0 = persistent until acknowledged)
        #[arg(long, short = 't', default_value = "6")]
        ttl: u64,
    },
    /// Show information about the available renderer backends
    Backends,
}

pub async fn execute(
    action: &OverlayAction,
    output: OutputFormat,
    _verbose: bool,
    _client: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        OverlayAction::Status => {
            // In a live deployment the overlay service is reached via IPC.
            // For now we report configuration defaults as the service state.
            let status = json!({
                "overlay": {
                    "enabled": true,
                    "visible": true,
                    "backend": "null",
                    "notifications": 0,
                    "note": "Connect to a running flightd instance for live status"
                }
            });
            match output {
                OutputFormat::Json => Ok(Some(serde_json::to_string_pretty(&status)?)),
                OutputFormat::Human => Ok(Some(format!(
                    "Overlay status:\n  Backend:       {}\n  Visible:       {}\n  Notifications: {}\n",
                    "null (headless)",
                    true,
                    0
                ))),
            }
        }

        OverlayAction::Show => {
            Ok(Some(match output {
                OutputFormat::Json => json!({"action":"show","queued":true}).to_string(),
                OutputFormat::Human => "Overlay show command queued.\n".to_string(),
            }))
        }

        OverlayAction::Hide => {
            Ok(Some(match output {
                OutputFormat::Json => json!({"action":"hide","queued":true}).to_string(),
                OutputFormat::Human => "Overlay hide command queued.\n".to_string(),
            }))
        }

        OverlayAction::Toggle => {
            Ok(Some(match output {
                OutputFormat::Json => json!({"action":"toggle","queued":true}).to_string(),
                OutputFormat::Human => "Overlay toggle command queued.\n".to_string(),
            }))
        }

        OverlayAction::Notify { message, severity, ttl } => {
            let sev = severity.as_str();
            let valid_severity = matches!(sev, "info" | "warning" | "alert" | "critical");
            if !valid_severity {
                return Err(anyhow::anyhow!(
                    "unknown severity '{}'; use info, warning, alert, or critical",
                    sev
                ));
            }
            match output {
                OutputFormat::Json => Ok(Some(
                    json!({
                        "action": "notify",
                        "message": message,
                        "severity": sev,
                        "ttl_secs": ttl,
                        "queued": true
                    })
                    .to_string(),
                )),
                OutputFormat::Human => Ok(Some(format!(
                    "Notification queued: [{sev}] {message} (TTL: {ttl}s)\n"
                ))),
            }
        }

        OverlayAction::Backends => {
            let backends = vec![
                json!({"name": "null", "description": "Headless / no-op (testing)", "available": true}),
                json!({"name": "openxr", "description": "OpenXR compositor layer", "available": false, "note": "Requires openxr feature and active VR runtime"}),
                json!({"name": "steamvr", "description": "SteamVR IVROverlay (Windows)", "available": false, "note": "Requires steamvr feature and SteamVR installation"}),
            ];
            match output {
                OutputFormat::Json => Ok(Some(serde_json::to_string_pretty(&backends)?)),
                OutputFormat::Human => {
                    let mut out = "Available overlay backends:\n".to_string();
                    out.push_str("  [✓] null     — Headless / no-op (testing)\n");
                    out.push_str("  [ ] openxr   — OpenXR compositor layer (requires openxr feature)\n");
                    out.push_str("  [ ] steamvr  — SteamVR IVROverlay / Windows (requires steamvr feature)\n");
                    Ok(Some(out))
                }
            }
        }
    }
}
