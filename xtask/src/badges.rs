// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BADGE_ENDPOINT_DIR: &str = "badges";
const BADGE_ENDPOINT_TARGET_DIR: &str = "target/xtask/badges";
const RIPR_TEST_EFFICIENCY_REPORT: &str = "target/ripr/reports/test-efficiency.json";
const EMPTY_TEST_EFFICIENCY_REPORT: &str = r#"{
  "schema_version": "0.1",
  "tests": [],
  "metrics": {
    "tests_scanned": 0
  }
}
"#;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct ShieldsEndpointBadge {
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

    if check {
        compare_files(
            &workspace_root
                .join(BADGE_ENDPOINT_DIR)
                .join("ripr-plus.json"),
            &target_dir.join("ripr-plus.json"),
        )?;

        println!("badges: committed endpoints are current");
        return Ok(());
    }

    let committed_dir = workspace_root.join(BADGE_ENDPOINT_DIR);
    fs::create_dir_all(&committed_dir)?;
    fs::copy(
        target_dir.join("ripr-plus.json"),
        committed_dir.join("ripr-plus.json"),
    )?;

    println!("badges: refreshed public endpoint JSON under badges/");
    Ok(())
}

fn workspace_root_path() -> Result<PathBuf> {
    std::env::current_dir().context("resolve workspace root")
}

fn ripr_plus_badge(workspace_root: &Path) -> Result<ShieldsEndpointBadge> {
    ensure_test_efficiency_report(workspace_root)?;

    let ripr_bin = std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());

    // Public README badge: repo-scoped, not PR/diff scoped.
    let output = Command::new(&ripr_bin)
        .arg("check")
        .arg("--root")
        .arg(workspace_root)
        .arg("--format")
        .arg("repo-badge-plus-shields")
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("run {ripr_bin} for repo-scoped ripr+ badge"))?;

    if !output.status.success() {
        anyhow::bail!(
            "{ripr_bin} repo-badge-plus-shields failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("{ripr_bin} emitted invalid Shields endpoint JSON"))
}

fn ensure_test_efficiency_report(workspace_root: &Path) -> Result<()> {
    let report_path = workspace_root.join(RIPR_TEST_EFFICIENCY_REPORT);
    if report_path.exists() {
        return Ok(());
    }

    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report_path, EMPTY_TEST_EFFICIENCY_REPORT)?;
    Ok(())
}

pub(crate) fn validate_shields_badge(
    badge: &ShieldsEndpointBadge,
    expected_label: Option<&str>,
) -> Result<()> {
    if badge.schema_version != 1 {
        anyhow::bail!("badge `{}` has unsupported schemaVersion", badge.label);
    }

    if let Some(expected_label) = expected_label
        && badge.label != expected_label
    {
        anyhow::bail!(
            "badge label drifted: got `{}`, expected `{expected_label}`",
            badge.label
        );
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
    let mut json = serde_json::to_string_pretty(badge)?;
    json.push('\n');
    fs::write(path, json)?;
    Ok(())
}

fn compare_files(committed: &Path, generated: &Path) -> Result<()> {
    let committed_bytes = fs::read(committed)
        .with_context(|| format!("missing committed badge endpoint `{}`", committed.display()))?;
    let generated_bytes = fs::read(generated)
        .with_context(|| format!("missing generated badge endpoint `{}`", generated.display()))?;

    if committed_bytes != generated_bytes {
        anyhow::bail!(
            "badge endpoint drift: `{}` differs from generated `{}`; run `cargo xtask badges`",
            committed.display(),
            generated.display()
        );
    }

    let badge: ShieldsEndpointBadge = serde_json::from_slice(&committed_bytes)
        .with_context(|| format!("parse committed badge endpoint `{}`", committed.display()))?;
    validate_shields_badge(&badge, None)?;
    Ok(())
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
}
