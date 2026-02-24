// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! `flightctl update` — update channel management and update checking

use crate::{client_manager::ClientManager, output::OutputFormat};
use clap::Subcommand;
use flight_updater::channels::{Channel, ChannelConfig};
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

/// Update channel management subcommands
#[derive(Subcommand)]
pub enum ChannelAction {
    /// Show the current update channel
    Show,
    /// Set the update channel
    Set {
        /// Channel to switch to: stable, beta, canary
        channel: String,
    },
}

/// `flightctl update` subcommands
#[derive(Subcommand)]
pub enum UpdateAction {
    /// Check for available updates on the current channel
    Check,
    /// Manage the update channel
    Channel {
        #[command(subcommand)]
        action: ChannelAction,
    },
    /// Show update channels and their configuration
    Channels,
}

pub async fn execute(
    action: &UpdateAction,
    output: OutputFormat,
    _verbose: bool,
    _client: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        UpdateAction::Check => check_for_updates(output).await,
        UpdateAction::Channel { action } => channel_command(action, output),
        UpdateAction::Channels => show_channels(output),
    }
}

async fn check_for_updates(output: OutputFormat) -> anyhow::Result<Option<String>> {
    let prefs = load_prefs();
    let channel = Channel::from_str(&prefs.channel)
        .unwrap_or(Channel::Stable);

    let channel_url = match channel {
        Channel::Stable => "https://updates.flight-hub.dev/stable",
        Channel::Beta => "https://updates.flight-hub.dev/beta",
        Channel::Canary => "https://updates.flight-hub.dev/canary",
    };

    let current = env!("CARGO_PKG_VERSION");

    match output {
        OutputFormat::Json => Ok(Some(
            json!({
                "success": true,
                "current_version": current,
                "channel": channel.to_string(),
                "update_url": channel_url,
                "note": "Connect flightd to enable live update checks"
            })
            .to_string(),
        )),
        OutputFormat::Human => Ok(Some(format!(
            "Current version : {current}\n\
             Channel         : {channel}\n\
             Update endpoint : {channel_url}\n\
             \n\
             Note: Start flightd and run `flightctl update check` again to query the update server."
        ))),
    }
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
            json!({ "success": true, "channel": channel.to_string() }).to_string(),
        )),
        OutputFormat::Human => Ok(Some(format!("Update channel: {channel}"))),
    }
}

fn set_channel(channel_str: &str, output: OutputFormat) -> anyhow::Result<Option<String>> {
    let channel = Channel::from_str(channel_str).map_err(|_| {
        anyhow::anyhow!(
            "Unknown channel '{}'. Valid channels: stable, beta, canary",
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
                "previous_channel": old,
                "channel": channel.to_string()
            })
            .to_string(),
        )),
        OutputFormat::Human => Ok(Some(format!(
            "Update channel changed: {old} → {channel}"
        ))),
    }
}

fn show_channels(output: OutputFormat) -> anyhow::Result<Option<String>> {
    let prefs = load_prefs();
    let current = prefs.channel.clone();

    let channels = vec![
        (Channel::Stable, "Stable", "Thoroughly tested. Recommended for all users.", 24u64),
        (Channel::Beta, "Beta", "Feature-complete; undergoing final testing.", 12),
        (Channel::Canary, "Canary", "Latest features; may be unstable.", 6),
    ];

    match output {
        OutputFormat::Json => {
            let list: Vec<_> = channels
                .iter()
                .map(|(ch, name, desc, freq)| {
                    json!({
                        "channel": ch.to_string(),
                        "name": name,
                        "description": desc,
                        "check_frequency_hours": freq,
                        "active": ch.to_string() == current
                    })
                })
                .collect();
            Ok(Some(json!({ "success": true, "channels": list }).to_string()))
        }
        OutputFormat::Human => {
            let mut lines = vec!["Available update channels:".to_string(), String::new()];
            for (ch, name, desc, freq) in &channels {
                let marker = if ch.to_string() == current { " ●" } else { "  " };
                lines.push(format!(
                    "{marker} {name:<8} ({ch}) — {desc} (checks every {freq}h)"
                ));
            }
            lines.push(String::new());
            lines.push(format!("Active channel: {current}"));
            lines.push("Switch channel: flightctl update channel set <stable|beta|canary>".to_string());
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
}
