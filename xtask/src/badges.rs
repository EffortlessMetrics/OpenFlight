// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BADGE_ENDPOINT_DIR: &str = "badges";
const BADGE_ENDPOINT_TARGET_DIR: &str = "target/xtask/badges";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct ShieldsEndpointBadge {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    label: String,
    message: String,
    color: String,
}

pub(crate) fn run_badges(check: bool) -> Result<()> {
    let workspace_root = workspace_root_path()?;
    let target_dir = workspace_root.join(BADGE_ENDPOINT_TARGET_DIR);
    fs::create_dir_all(&target_dir)?;

    let ripr_plus = ripr_plus_badge(&workspace_root)?;
    validate_shields_badge(&ripr_plus, Some("ripr+"))?;
    write_json_pretty(&target_dir.join("ripr-plus.json"), &ripr_plus)?;

    let committed_dir = workspace_root.join(BADGE_ENDPOINT_DIR);
    if check {
        compare_files(
            &committed_dir.join("ripr-plus.json"),
            &target_dir.join("ripr-plus.json"),
        )?;
        println!("badges: committed endpoints are current");
        return Ok(());
    }

    fs::create_dir_all(&committed_dir)?;
    fs::copy(
        target_dir.join("ripr-plus.json"),
        committed_dir.join("ripr-plus.json"),
    )?;

    println!("badges: refreshed public endpoint JSON under badges/");
    Ok(())
}

fn ripr_plus_badge(workspace_root: &Path) -> Result<ShieldsEndpointBadge> {
    let ripr_bin = std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());

    let output = run_ripr_badge_format(&ripr_bin, workspace_root, "repo-badge-plus-shields")?;
    let output = if output.status.success() {
        output
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("test-efficiency.json") {
            run_ripr_badge_format(&ripr_bin, workspace_root, "repo-badge-shields")?
        } else {
            anyhow::bail!("{ripr_bin} repo-badge-plus-shields failed: {stderr}");
        }
    };

    if !output.status.success() {
        anyhow::bail!(
            "{ripr_bin} repo-badge-shields failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let mut badge: ShieldsEndpointBadge = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("{ripr_bin} emitted invalid Shields endpoint JSON"))?;
    badge.label = "ripr+".to_string();
    Ok(badge)
}

fn run_ripr_badge_format(
    ripr_bin: &str,
    workspace_root: &Path,
    format: &str,
) -> Result<std::process::Output> {
    Command::new(ripr_bin)
        .arg("check")
        .arg("--root")
        .arg(workspace_root)
        .arg("--mode")
        .arg("instant")
        .arg("--format")
        .arg(format)
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("failed to run {ripr_bin}"))
}

fn validate_shields_badge(
    badge: &ShieldsEndpointBadge,
    expected_label: Option<&str>,
) -> Result<()> {
    if badge.schema_version != 1 {
        anyhow::bail!("badge `{}` has unsupported schemaVersion", badge.label);
    }

    if let Some(expected_label) = expected_label {
        if badge.label != expected_label {
            anyhow::bail!(
                "badge label drifted: got `{}`, expected `{expected_label}`",
                badge.label
            );
        }
    }

    if badge.message.trim().is_empty() {
        anyhow::bail!("badge `{}` has empty message", badge.label);
    }

    if badge.color.trim().is_empty() {
        anyhow::bail!("badge `{}` has empty color", badge.label);
    }

    Ok(())
}

fn write_json_pretty(path: &Path, badge: &ShieldsEndpointBadge) -> Result<()> {
    let json = serde_json::to_string_pretty(badge)?;
    fs::write(path, format!("{json}\n"))?;
    Ok(())
}

fn compare_files(committed: &Path, generated: &Path) -> Result<()> {
    let committed_content = fs::read(committed)
        .with_context(|| format!("missing committed badge endpoint: {}", committed.display()))?;
    let generated_content = fs::read(generated)
        .with_context(|| format!("missing generated badge endpoint: {}", generated.display()))?;

    if committed_content != generated_content {
        anyhow::bail!(
            "badge endpoint drifted: {} differs from {}",
            committed.display(),
            generated.display()
        );
    }

    Ok(())
}

fn workspace_root_path() -> Result<PathBuf> {
    std::env::current_dir().context("failed to resolve workspace root")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ripr_plus_badge_shape_is_stable() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "ripr+".to_string(),
            message: "0".to_string(),
            color: "brightgreen".to_string(),
        };

        validate_shields_badge(&badge, Some("ripr+")).unwrap();
    }

    #[test]
    fn scanner_safe_badge_shape_is_stable() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "fixtures".to_string(),
            message: "scanner-safe".to_string(),
            color: "brightgreen".to_string(),
        };

        validate_shields_badge(&badge, Some("fixtures")).unwrap();
    }

    #[test]
    fn rejects_empty_badge_message() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "ripr+".to_string(),
            message: " ".to_string(),
            color: "brightgreen".to_string(),
        };

        assert!(validate_shields_badge(&badge, Some("ripr+")).is_err());
    }
}
