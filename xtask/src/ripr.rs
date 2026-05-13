// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{Context, Result};
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RIPR_PR_DIR: &str = "target/ripr/pr";
const RIPR_REVIEW_DIR: &str = "target/ripr/review";

pub(crate) fn run_ripr_pr(check: bool) -> Result<()> {
    let workspace_root = workspace_root_path()?;
    let out_dir = workspace_root.join(RIPR_PR_DIR);

    if check {
        check_pr_contract(&out_dir)?;
        println!("ripr-pr: output contract is intact");
        return Ok(());
    }

    fs::create_dir_all(&out_dir)?;
    let json_path = out_dir.join("repo-exposure.json");
    let md_path = out_dir.join("repo-exposure.md");
    let ripr_bin = ripr_bin();
    let base = default_base(&workspace_root);

    let json = run_ripr_capture(
        &ripr_bin,
        &workspace_root,
        vec![
            "check".into(),
            "--root".into(),
            workspace_root.as_os_str().to_os_string(),
            "--mode".into(),
            "instant".into(),
            "--base".into(),
            base.clone(),
            "--format".into(),
            "json".into(),
        ],
        "repo exposure JSON",
    )?;
    fs::write(&json_path, json)?;

    let markdown = run_ripr_capture(
        &ripr_bin,
        &workspace_root,
        vec![
            "check".into(),
            "--root".into(),
            workspace_root.as_os_str().to_os_string(),
            "--mode".into(),
            "instant".into(),
            "--base".into(),
            base.clone(),
            "--format".into(),
            "human".into(),
        ],
        "repo exposure Markdown",
    )?;
    fs::write(&md_path, markdown)?;

    check_pr_contract(&out_dir)?;
    println!("ripr-pr: wrote PR evidence under {}", out_dir.display());
    Ok(())
}

pub(crate) fn run_ripr_review_comments(check: bool) -> Result<()> {
    let workspace_root = workspace_root_path()?;
    let out_dir = workspace_root.join(RIPR_REVIEW_DIR);

    if check {
        check_review_contract(&out_dir)?;
        println!("ripr-review-comments: output contract is intact");
        return Ok(());
    }

    fs::create_dir_all(&out_dir)?;
    let json_path = out_dir.join("comments.json");
    let ripr_bin = ripr_bin();
    let base = default_base(&workspace_root);

    run_ripr(
        &ripr_bin,
        &workspace_root,
        vec![
            "review-comments".into(),
            "--root".into(),
            workspace_root.as_os_str().to_os_string(),
            "--base".into(),
            base,
            "--head".into(),
            "HEAD".into(),
            "--out".into(),
            json_path.as_os_str().to_os_string(),
        ],
        "review comments",
    )?;

    check_review_contract(&out_dir)?;
    println!(
        "ripr-review-comments: wrote review guidance under {}",
        out_dir.display()
    );
    Ok(())
}

fn default_base(workspace_root: &Path) -> OsString {
    let has_origin_main = Command::new("git")
        .args(["rev-parse", "--verify", "origin/main"])
        .current_dir(workspace_root)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    if has_origin_main {
        "origin/main".into()
    } else {
        "HEAD".into()
    }
}

fn run_ripr(
    ripr_bin: &str,
    workspace_root: &Path,
    args: Vec<OsString>,
    description: &str,
) -> Result<()> {
    let output = Command::new(ripr_bin)
        .args(args)
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("failed to run {ripr_bin} for {description}"))?;

    if !output.status.success() {
        anyhow::bail!(
            "{ripr_bin} {description} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn run_ripr_capture(
    ripr_bin: &str,
    workspace_root: &Path,
    args: Vec<OsString>,
    description: &str,
) -> Result<Vec<u8>> {
    let output = Command::new(ripr_bin)
        .args(args)
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("failed to run {ripr_bin} for {description}"))?;

    if !output.status.success() {
        anyhow::bail!(
            "{ripr_bin} {description} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(output.stdout)
}

fn check_pr_contract(out_dir: &Path) -> Result<()> {
    check_json_file(&out_dir.join("repo-exposure.json"))?;
    check_nonempty_file(&out_dir.join("repo-exposure.md"))?;
    Ok(())
}

fn check_review_contract(out_dir: &Path) -> Result<()> {
    check_json_file(&out_dir.join("comments.json"))?;
    check_nonempty_file(&out_dir.join("comments.md"))?;
    Ok(())
}

fn check_json_file(path: &Path) -> Result<Value> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("missing required RIPR JSON file: {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("invalid RIPR JSON file: {}", path.display()))
}

fn check_nonempty_file(path: &Path) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("missing required RIPR file: {}", path.display()))?;
    if content.trim().is_empty() {
        anyhow::bail!("RIPR output file is empty: {}", path.display());
    }
    Ok(())
}

fn ripr_bin() -> String {
    std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string())
}

fn workspace_root_path() -> Result<PathBuf> {
    std::env::current_dir().context("failed to resolve workspace root")
}
