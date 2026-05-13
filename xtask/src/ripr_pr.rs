// SPDX-License-Identifier: MIT OR Apache-2.0

//! PR-scoped RIPR evidence automation.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

const RIPR_PR_DIR: &str = "target/ripr/pr";
const RIPR_REVIEW_DIR: &str = "target/ripr/review";
const REPO_EXPOSURE_JSON: &str = "repo-exposure.json";
const REPO_EXPOSURE_MD: &str = "repo-exposure.md";
const COMMENTS_JSON: &str = "comments.json";
const COMMENTS_MD: &str = "comments.md";

pub(crate) fn run_ripr_pr(check: bool, base: Option<&str>) -> Result<()> {
    let workspace_root = workspace_root_path()?;
    let out_dir = workspace_root.join(RIPR_PR_DIR);

    if check {
        check_pr_contract(&out_dir)?;
        println!("ripr-pr: output contract is intact");
        return Ok(());
    }

    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    let base = base.unwrap_or("origin/main");
    run_ripr_check(
        &workspace_root,
        base,
        "repo-exposure-json",
        &out_dir.join(REPO_EXPOSURE_JSON),
    )?;
    run_ripr_check(
        &workspace_root,
        base,
        "repo-exposure-md",
        &out_dir.join(REPO_EXPOSURE_MD),
    )?;
    check_pr_contract(&out_dir)?;
    println!("ripr-pr: wrote PR-scoped evidence under {RIPR_PR_DIR}/");
    Ok(())
}

pub(crate) fn run_ripr_review_comments(
    check: bool,
    base: Option<&str>,
    head: Option<&str>,
) -> Result<()> {
    let workspace_root = workspace_root_path()?;
    let out_dir = workspace_root.join(RIPR_REVIEW_DIR);
    let json_path = out_dir.join(COMMENTS_JSON);
    let md_path = out_dir.join(COMMENTS_MD);

    if check {
        read_json_file(&json_path)?;
        ensure_non_empty_file(&md_path)?;
        println!("ripr-review-comments: output contract is intact");
        return Ok(());
    }

    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;

    let ripr_bin = ripr_bin();
    let output = Command::new(&ripr_bin)
        .arg("review-comments")
        .arg("--root")
        .arg(&workspace_root)
        .arg("--base")
        .arg(base.unwrap_or("origin/main"))
        .arg("--head")
        .arg(head.unwrap_or("HEAD"))
        .arg("--out")
        .arg(&json_path)
        .current_dir(&workspace_root)
        .output()
        .with_context(|| format!("failed to run {ripr_bin}"))?;

    if !output.status.success() {
        anyhow::bail!(
            "{ripr_bin} review-comments failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    read_json_file(&json_path)?;
    ensure_non_empty_file(&md_path)?;
    println!("ripr-review-comments: wrote review guidance under {RIPR_REVIEW_DIR}/");
    Ok(())
}

fn run_ripr_check(workspace_root: &Path, base: &str, format: &str, out: &Path) -> Result<()> {
    let ripr_bin = ripr_bin();
    let output = Command::new(&ripr_bin)
        .arg("check")
        .arg("--root")
        .arg(workspace_root)
        .arg("--base")
        .arg(base)
        .arg("--format")
        .arg(format)
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("failed to run {ripr_bin}"))?;

    if !output.status.success() {
        anyhow::bail!(
            "{ripr_bin} check --format {format} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    std::fs::write(out, output.stdout)
        .with_context(|| format!("failed to write {}", out.display()))?;
    Ok(())
}

fn check_pr_contract(out_dir: &Path) -> Result<()> {
    read_json_file(&out_dir.join(REPO_EXPOSURE_JSON))?;
    ensure_non_empty_file(&out_dir.join(REPO_EXPOSURE_MD))?;
    Ok(())
}

fn read_json_file(path: &Path) -> Result<serde_json::Value> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("missing required JSON artifact {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("invalid JSON artifact {}", path.display()))
}

fn ensure_non_empty_file(path: &Path) -> Result<()> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("missing required artifact {}", path.display()))?;
    if metadata.len() == 0 {
        anyhow::bail!("required artifact {} is empty", path.display());
    }
    Ok(())
}

fn workspace_root_path() -> Result<PathBuf> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .context("failed to run cargo metadata")?;

    if !output.status.success() {
        anyhow::bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("cargo metadata emitted invalid JSON")?;
    let root = metadata
        .get("workspace_root")
        .and_then(|value| value.as_str())
        .context("cargo metadata did not include workspace_root")?;
    Ok(PathBuf::from(root))
}

fn ripr_bin() -> String {
    std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string())
}
