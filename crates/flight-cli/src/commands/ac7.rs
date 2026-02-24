// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Ace Combat 7 integration CLI commands.

use crate::client_manager::ClientManager;
use crate::output::OutputFormat;
use anyhow::{Context, Result};
use clap::Subcommand;
use flight_ac7_input::{
    Ac7InputProfile, ac7_input_ini_path, ac7_save_games_dir, install_profile, render_managed_block,
    steam_input_hint,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum Ac7Action {
    /// Show AC7 integration status and paths.
    Status,
    /// Render a managed Input.ini block to a file.
    RenderInput {
        /// Output path for rendered managed block text.
        #[arg(long, short)]
        output: PathBuf,
        /// Optional JSON profile file path.
        #[arg(long)]
        profile: Option<PathBuf>,
    },
    /// Install/update managed mappings in AC7 Input.ini.
    InstallInput {
        /// Explicit Input.ini path (auto-detected if omitted).
        #[arg(long)]
        input_ini: Option<PathBuf>,
        /// Optional JSON profile file path.
        #[arg(long)]
        profile: Option<PathBuf>,
        /// Create backup before write.
        #[arg(long, default_value_t = true)]
        backup: bool,
    },
}

pub async fn execute(
    action: &Ac7Action,
    output_format: OutputFormat,
    _verbose: bool,
    _client_manager: &ClientManager,
) -> Result<Option<String>> {
    match action {
        Ac7Action::Status => {
            let input_ini = ac7_input_ini_path();
            let save_dir = ac7_save_games_dir();
            let input_exists = input_ini.as_ref().is_some_and(|p| p.exists());
            let save_exists = save_dir.as_ref().is_some_and(|p| p.exists());

            let output = match output_format {
                OutputFormat::Json => json!({
                    "input_ini": input_ini.map(|p| p.display().to_string()),
                    "input_ini_exists": input_exists,
                    "save_games_dir": save_dir.map(|p| p.display().to_string()),
                    "save_games_dir_exists": save_exists,
                    "steam_input_hint": steam_input_hint(),
                })
                .to_string(),
                OutputFormat::Human => {
                    format!(
                        "AC7 Input.ini: {}\nInput.ini exists: {}\nAC7 SaveGames: {}\nSaveGames exists: {}\nHint: {}",
                        path_or_unknown(input_ini.as_ref()),
                        input_exists,
                        path_or_unknown(save_dir.as_ref()),
                        save_exists,
                        steam_input_hint()
                    )
                }
            };

            Ok(Some(output))
        }
        Ac7Action::RenderInput { output, profile } => {
            let profile = load_profile(profile)?;
            let block = render_managed_block(&profile)?;
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(output, block.as_bytes())?;

            let output = match output_format {
                OutputFormat::Json => json!({
                    "success": true,
                    "output": output.display().to_string(),
                    "profile_name": profile.name
                })
                .to_string(),
                OutputFormat::Human => {
                    format!(
                        "Rendered AC7 managed block to {} (profile: {})",
                        output.display(),
                        profile.name
                    )
                }
            };

            Ok(Some(output))
        }
        Ac7Action::InstallInput {
            input_ini,
            profile,
            backup,
        } => {
            let profile = load_profile(profile)?;
            let target = input_ini
                .clone()
                .or_else(ac7_input_ini_path)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Could not resolve default AC7 Input.ini path; pass --input-ini explicitly"
                    )
                })?;

            let result = install_profile(&target, &profile, *backup)?;

            let output = match output_format {
                OutputFormat::Json => json!({
                    "success": true,
                    "input_ini": result.input_ini_path.display().to_string(),
                    "backup_path": result.backup_path.map(|p| p.display().to_string()),
                    "bytes_written": result.bytes_written,
                    "profile_name": profile.name,
                })
                .to_string(),
                OutputFormat::Human => {
                    let mut msg = format!(
                        "Installed AC7 managed mappings at {} ({} bytes)",
                        result.input_ini_path.display(),
                        result.bytes_written
                    );
                    if let Some(backup_path) = result.backup_path {
                        msg.push_str(&format!("\nBackup: {}", backup_path.display()));
                    }
                    msg
                }
            };

            Ok(Some(output))
        }
    }
}

fn load_profile(path: &Option<PathBuf>) -> Result<Ac7InputProfile> {
    if let Some(path) = path {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read profile file {}", path.display()))?;
        let profile = serde_json::from_str::<Ac7InputProfile>(&content)
            .with_context(|| format!("failed to parse profile JSON {}", path.display()))?;
        Ok(profile)
    } else {
        Ok(Ac7InputProfile::default())
    }
}

fn path_or_unknown(path: Option<&PathBuf>) -> String {
    path.map(|p| p.display().to_string())
        .unwrap_or_else(|| "<unresolved>".to_string())
}
