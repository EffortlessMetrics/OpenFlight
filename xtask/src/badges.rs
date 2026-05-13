// SPDX-License-Identifier: MIT OR Apache-2.0

//! Generated public README badge endpoints.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BADGE_ENDPOINT_DIR: &str = "badges";
const BADGE_ENDPOINT_TARGET_DIR: &str = "target/xtask/badges";
const TEST_EFFICIENCY_REPORT: &str = "target/ripr/reports/test-efficiency.json";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct ShieldsEndpointBadge {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    label: String,
    message: String,
    color: String,
}

pub fn run_badges(check: bool) -> Result<()> {
    let workspace_root = workspace_root_path()?;
    let target_dir = workspace_root.join(BADGE_ENDPOINT_TARGET_DIR);
    fs::create_dir_all(&target_dir)
        .with_context(|| format!("failed to create {}", target_dir.display()))?;

    let ripr_plus = ripr_plus_badge(&workspace_root)?;
    validate_shields_badge(&ripr_plus, Some("ripr+"))?;
    write_json_pretty(&target_dir.join("ripr-plus.json"), &ripr_plus)?;

    if check {
        let committed_dir = workspace_root.join(BADGE_ENDPOINT_DIR);
        compare_files(
            &committed_dir.join("ripr-plus.json"),
            &target_dir.join("ripr-plus.json"),
        )?;
        println!("badges: committed endpoints are current");
        return Ok(());
    }

    let committed_dir = workspace_root.join(BADGE_ENDPOINT_DIR);
    fs::create_dir_all(&committed_dir)
        .with_context(|| format!("failed to create {}", committed_dir.display()))?;
    fs::copy(
        target_dir.join("ripr-plus.json"),
        committed_dir.join("ripr-plus.json"),
    )
    .context("failed to refresh badges/ripr-plus.json")?;

    println!("badges: refreshed public endpoint JSON under badges/");
    Ok(())
}

fn workspace_root_path() -> Result<PathBuf> {
    let current_dir = std::env::current_dir().context("failed to read current directory")?;
    let mut search_dir = current_dir.as_path();

    loop {
        let cargo_toml = search_dir.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = fs::read_to_string(&cargo_toml)
                .with_context(|| format!("failed to read {}", cargo_toml.display()))?;
            if content.contains("[workspace]") {
                return Ok(search_dir.to_path_buf());
            }
        }

        search_dir = search_dir
            .parent()
            .ok_or_else(|| anyhow::anyhow!("could not find workspace root"))?;
    }
}

fn ripr_plus_badge(workspace_root: &Path) -> Result<ShieldsEndpointBadge> {
    let ripr_bin = std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());

    ensure_test_efficiency_report(workspace_root)?;

    // Public README badge: repo-scoped, not PR/diff scoped.
    let output = Command::new(&ripr_bin)
        .arg("check")
        .arg("--root")
        .arg(workspace_root)
        .arg("--format")
        .arg("repo-badge-plus-shields")
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("failed to run {ripr_bin}"))?;

    if !output.status.success() {
        bail!(
            "{ripr_bin} repo-badge-plus-shields failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("{ripr_bin} emitted invalid Shields endpoint JSON"))
}

fn ensure_test_efficiency_report(workspace_root: &Path) -> Result<()> {
    let report_path = workspace_root.join(TEST_EFFICIENCY_REPORT);
    if report_path.exists() {
        return Ok(());
    }

    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let report = serde_json::json!({
        "schema_version": "0.1",
        "tests": [],
        "metrics": {
            "tests_scanned": 0,
            "reason_counts": {}
        }
    });
    let json = serde_json::to_string_pretty(&report)
        .context("failed to serialize default test-efficiency report")?;
    fs::write(&report_path, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", report_path.display()))
}

fn validate_shields_badge(
    badge: &ShieldsEndpointBadge,
    expected_label: Option<&str>,
) -> Result<()> {
    if badge.schema_version != 1 {
        bail!("badge `{}` has unsupported schemaVersion", badge.label);
    }

    if let Some(expected_label) = expected_label
        && badge.label != expected_label
    {
        bail!(
            "badge label drifted: got `{}`, expected `{expected_label}`",
            badge.label
        );
    }

    if badge.message.trim().is_empty() {
        bail!("badge `{}` has empty message", badge.label);
    }

    if badge.color.trim().is_empty() {
        bail!("badge `{}` has empty color", badge.label);
    }

    Ok(())
}

fn write_json_pretty(path: &Path, badge: &ShieldsEndpointBadge) -> Result<()> {
    validate_shields_badge(badge, None)?;
    let json = serde_json::to_string_pretty(badge).context("failed to serialize badge JSON")?;
    fs::write(path, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

fn compare_files(committed: &Path, generated: &Path) -> Result<()> {
    let committed_bytes = fs::read(committed)
        .with_context(|| format!("missing committed badge endpoint {}", committed.display()))?;
    let generated_bytes = fs::read(generated)
        .with_context(|| format!("missing generated badge endpoint {}", generated.display()))?;

    if committed_bytes != generated_bytes {
        bail!(
            "badge endpoint drift detected: {} differs from generated {} (run `cargo xtask badges`)",
            committed.display(),
            generated.display()
        );
    }

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
    fn empty_badge_message_is_rejected() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "ripr+".to_string(),
            message: " ".to_string(),
            color: "brightgreen".to_string(),
        };

        assert!(validate_shields_badge(&badge, Some("ripr+")).is_err());
    }
}
