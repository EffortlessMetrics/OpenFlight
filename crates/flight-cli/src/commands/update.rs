// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! `flightctl update` — local update preferences and update capability status.
//!
//! Automatic update checking is not currently a released OpenFlight capability.
//! Channel preferences remain local configuration, but `update check` fails
//! explicitly until the signed manifest/key/endpoint path is production-ready.

use crate::{client_manager::ClientManager, output::OutputFormat};
use clap::Subcommand;
use flight_updater::channels::Channel;
use serde_json::json;
use std::path::PathBuf;
use std::str::FromStr;

/// Persisted update configuration stored in the user's data directory.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct UpdatePrefs {
    channel: String,
}

impl Default for UpdatePrefs {
    fn default() -> Self {
        Self {
            channel: "stable".to_string(),
        }
    }
}

fn prefs_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("flight-hub")
        .join("update-prefs.json")
}

fn load_prefs() -> UpdatePrefs {
    let path = prefs_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        UpdatePrefs::default()
    }
}

fn save_prefs(prefs: &UpdatePrefs) -> anyhow::Result<()> {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(prefs)?)?;
    Ok(())
}

/// Update channel management subcommands.
#[derive(Subcommand)]
pub enum ChannelAction {
    /// Show the locally saved update-channel preference.
    Show,
    /// Set the locally saved update-channel preference.
    Set {
        /// Channel preference to save: stable, beta, canary.
        channel: String,
    },
}

/// `flightctl update` subcommands.
#[derive(Subcommand)]
pub enum UpdateAction {
    /// Check for available updates on the current channel.
    Check,
    /// Manage the local update-channel preference.
    Channel {
        #[command(subcommand)]
        action: ChannelAction,
    },
    /// Show known channel preference values and updater availability.
    Channels,
}

pub async fn execute(
    action: &UpdateAction,
    output: OutputFormat,
    _verbose: bool,
    _client: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        UpdateAction::Check => check_for_updates().await,
        UpdateAction::Channel { action } => channel_command(action, output),
        UpdateAction::Channels => show_channels(output),
    }
}

async fn check_for_updates() -> anyhow::Result<Option<String>> {
    let prefs = load_prefs();
    let channel = Channel::from_str(&prefs.channel).unwrap_or(Channel::Stable);

    anyhow::bail!(
        "Update checks are unavailable: the '{}' channel preference is saved locally, but OpenFlight does not yet ship a production signed update manifest, trusted channel key, and live update endpoint",
        channel
    )
}

fn channel_command(action: &ChannelAction, output: OutputFormat) -> anyhow::Result<Option<String>> {
    match action {
        ChannelAction::Show => show_current_channel(output),
        ChannelAction::Set { channel } => set_channel(channel, output),
    }
}

fn show_current_channel(output: OutputFormat) -> anyhow::Result<Option<String>> {
    let prefs = load_prefs();
    let channel = Channel::from_str(&prefs.channel).unwrap_or(Channel::Stable);
    match output {
        OutputFormat::Json => Ok(Some(
            json!({
                "success": true,
                "channel_preference": channel.to_string(),
                "update_check_available": false
            })
            .to_string(),
        )),
        OutputFormat::Human => Ok(Some(format!(
            "Saved update channel preference: {channel}\nUpdate checks available: no"
        ))),
    }
}

fn set_channel(channel_str: &str, output: OutputFormat) -> anyhow::Result<Option<String>> {
    let channel = Channel::from_str(channel_str).map_err(|_| {
        anyhow::anyhow!(
            "Unknown channel '{}'. Valid preferences: stable, beta, canary",
            channel_str
        )
    })?;

    let mut prefs = load_prefs();
    let old = prefs.channel.clone();
    prefs.channel = channel.to_string();
    save_prefs(&prefs)?;

    match output {
        OutputFormat::Json => Ok(Some(
            json!({
                "success": true,
                "previous_channel_preference": old,
                "channel_preference": channel.to_string(),
                "update_check_available": false
            })
            .to_string(),
        )),
        OutputFormat::Human => Ok(Some(format!(
            "Saved update channel preference: {old} -> {channel}\nUpdate checks available: no"
        ))),
    }
}

fn show_channels(output: OutputFormat) -> anyhow::Result<Option<String>> {
    let prefs = load_prefs();
    let current = Channel::from_str(&prefs.channel).unwrap_or(Channel::Stable);

    let channels = [Channel::Stable, Channel::Beta, Channel::Canary];

    match output {
        OutputFormat::Json => {
            let list: Vec<_> = channels
                .iter()
                .map(|channel| {
                    json!({
                        "channel": channel.to_string(),
                        "active_preference": *channel == current,
                        "update_check_available": false
                    })
                })
                .collect();
            Ok(Some(
                json!({
                    "success": true,
                    "channels": list,
                    "note": "These are local preference values only; production update checking is not available."
                })
                .to_string(),
            ))
        }
        OutputFormat::Human => {
            let mut lines = vec![
                "Known update channel preferences:".to_string(),
                String::new(),
            ];
            for channel in channels {
                let marker = if channel == current { "*" } else { " " };
                lines.push(format!("{marker} {channel}"));
            }
            lines.push(String::new());
            lines.push("Update checks available: no".to_string());
            lines.push(
                "The saved preference will become operational only after OpenFlight ships a trusted signed update service."
                    .to_string(),
            );
            Ok(Some(lines.join("\n")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_roundtrip() {
        for ch in ["stable", "beta", "canary"] {
            let parsed = Channel::from_str(ch).unwrap();
            assert_eq!(parsed.to_string(), ch);
        }
    }

    #[test]
    fn test_invalid_channel() {
        assert!(Channel::from_str("nightly").is_err());
    }

    #[test]
    fn test_prefs_default() {
        let p = UpdatePrefs::default();
        assert_eq!(p.channel, "stable");
    }

    #[tokio::test]
    async fn update_check_is_explicitly_unavailable() {
        let error = check_for_updates()
            .await
            .expect_err("update check must not report a synthetic endpoint success");
        let message = error.to_string();

        assert!(message.contains("unavailable"));
        assert!(message.contains("signed update manifest"));
        assert!(message.contains("trusted channel key"));
    }

    #[test]
    fn channel_list_does_not_claim_release_quality_or_live_endpoints() {
        let output = show_channels(OutputFormat::Json).expect("channel list should render");
        let rendered = output.expect("channel list should have output");
        let value: serde_json::Value = serde_json::from_str(&rendered).expect("valid json");

        assert_eq!(value["success"], true);
        assert_eq!(value["channels"][0]["update_check_available"], false);
        assert!(!rendered.contains("Thoroughly tested"));
        assert!(!rendered.contains("updates.flight-hub.dev"));
    }
}
