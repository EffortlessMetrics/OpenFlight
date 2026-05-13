// SPDX-License-Identifier: MIT OR Apache-2.0

//! PR-scoped RIPR evidence helpers.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RIPR_PR_DIR: &str = "target/ripr/pr";
const RIPR_REVIEW_DIR: &str = "target/ripr/review";

pub fn run_ripr_pr(check: bool) -> Result<()> {
    let workspace_root = workspace_root_path()?;
    let evidence_dir = workspace_root.join(RIPR_PR_DIR);

    if !check {
        fs::create_dir_all(&evidence_dir)
            .with_context(|| format!("failed to create {}", evidence_dir.display()))?;
        run_ripr_pilot(&workspace_root, &evidence_dir)?;
        ensure_repo_exposure_contract_files(&evidence_dir)?;
        println!("ripr-pr: wrote PR-scoped evidence under {RIPR_PR_DIR}/");
    }

    validate_ripr_pr_contract(&evidence_dir)
}

pub fn run_ripr_review_comments(check: bool) -> Result<()> {
    let workspace_root = workspace_root_path()?;
    let review_dir = workspace_root.join(RIPR_REVIEW_DIR);

    if !check {
        fs::create_dir_all(&review_dir)
            .with_context(|| format!("failed to create {}", review_dir.display()))?;
        run_ripr_review_comments_command(&workspace_root, &review_dir.join("comments.json"))?;
    }

    validate_review_contract(&review_dir)
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

fn ripr_bin() -> String {
    std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string())
}

fn run_ripr_pilot(workspace_root: &Path, out_dir: &Path) -> Result<()> {
    let ripr_bin = ripr_bin();
    let output = Command::new(&ripr_bin)
        .arg("pilot")
        .arg("--root")
        .arg(workspace_root)
        .arg("--out")
        .arg(out_dir)
        .arg("--mode")
        .arg("instant")
        .arg("--timeout-ms")
        .arg("120000")
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("failed to run {ripr_bin}"))?;

    if !output.status.success() {
        bail!(
            "{ripr_bin} pilot failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn ensure_repo_exposure_contract_files(evidence_dir: &Path) -> Result<()> {
    let repo_json = evidence_dir.join("repo-exposure.json");
    let repo_md = evidence_dir.join("repo-exposure.md");
    if repo_json.exists() && repo_md.exists() {
        return Ok(());
    }

    let summary_json = evidence_dir.join("pilot-summary.json");
    let summary_md = evidence_dir.join("pilot-summary.md");
    if !summary_json.exists() || !summary_md.exists() {
        return Ok(());
    }

    let mut value = read_json(&summary_json)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "repo_exposure_contract".to_string(),
            serde_json::json!({
                "status": "partial",
                "reason": "ripr pilot did not finish within the configured budget",
                "source": "pilot-summary"
            }),
        );
    }
    let json = serde_json::to_string_pretty(&value)
        .context("failed to serialize partial RIPR evidence")?;
    fs::write(&repo_json, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", repo_json.display()))?;

    let markdown = fs::read_to_string(&summary_md)
        .with_context(|| format!("failed to read {}", summary_md.display()))?;
    fs::write(
        &repo_md,
        format!(
            "# RIPR PR Evidence\n\nRIPR produced a partial pilot summary before the configured timeout. The machine-readable partial evidence is in `repo-exposure.json`; rerun the retry command from the summary for complete repo exposure.\n\n{markdown}"
        ),
    )
    .with_context(|| format!("failed to write {}", repo_md.display()))
}

fn run_ripr_review_comments_command(workspace_root: &Path, out: &Path) -> Result<()> {
    let ripr_bin = ripr_bin();
    let output = Command::new(&ripr_bin)
        .arg("review-comments")
        .arg("--root")
        .arg(workspace_root)
        .arg("--base")
        .arg("origin/main")
        .arg("--head")
        .arg("HEAD")
        .arg("--out")
        .arg(out)
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("failed to run {ripr_bin}"))?;

    if !output.status.success() {
        bail!(
            "{ripr_bin} review-comments failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn validate_ripr_pr_contract(evidence_dir: &Path) -> Result<()> {
    let json_path = evidence_dir.join("repo-exposure.json");
    let markdown_path = evidence_dir.join("repo-exposure.md");
    read_json(&json_path)?;
    read_nonempty_text(&markdown_path)?;
    println!("ripr-pr: evidence contract is valid");
    Ok(())
}

fn validate_review_contract(review_dir: &Path) -> Result<()> {
    let json_path = review_dir.join("comments.json");
    let markdown_path = review_dir.join("comments.md");
    read_json(&json_path)?;
    read_nonempty_text(&markdown_path)?;
    println!("ripr-review-comments: evidence contract is valid");
    Ok(())
}

fn read_json(path: &Path) -> Result<Value> {
    let bytes =
        fs::read(path).with_context(|| format!("missing required file {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("invalid JSON in {}", path.display()))
}

fn read_nonempty_text(path: &Path) -> Result<String> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("missing required file {}", path.display()))?;
    if text.trim().is_empty() {
        bail!("required file {} is empty", path.display());
    }
    Ok(text)
}
