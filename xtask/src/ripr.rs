// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{Context, Result};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

const RIPR_PR_DIR: &str = "target/ripr/pr";
const RIPR_REVIEW_DIR: &str = "target/ripr/review";

pub(crate) fn run_ripr_pr(check: bool) -> Result<()> {
    let workspace_root = workspace_root_path()?;
    let out_dir = workspace_root.join(RIPR_PR_DIR);

    if check {
        validate_pr_outputs(&out_dir)?;
        println!("ripr-pr: output contract is intact");
        return Ok(());
    }

    fs::create_dir_all(&out_dir)?;
    run_ripr_check(
        &workspace_root,
        "repo-exposure-json",
        &out_dir.join("repo-exposure.json"),
    )?;
    run_ripr_check(
        &workspace_root,
        "repo-exposure-md",
        &out_dir.join("repo-exposure.md"),
    )?;
    validate_pr_outputs(&out_dir)?;
    println!("ripr-pr: wrote PR evidence under {RIPR_PR_DIR}/");
    Ok(())
}

pub(crate) fn run_ripr_review_comments(check: bool) -> Result<()> {
    let workspace_root = workspace_root_path()?;
    let out_dir = workspace_root.join(RIPR_REVIEW_DIR);

    if check {
        validate_review_outputs(&out_dir)?;
        println!("ripr-review-comments: output contract is intact");
        return Ok(());
    }

    fs::create_dir_all(&out_dir)?;
    let output_path = out_dir.join("comments.json");
    let ripr_bin = ripr_bin();
    let output = Command::new(&ripr_bin)
        .arg("review-comments")
        .arg("--root")
        .arg(&workspace_root)
        .arg("--base")
        .arg(ripr_base())
        .arg("--head")
        .arg(ripr_head())
        .arg("--out")
        .arg(&output_path)
        .current_dir(&workspace_root)
        .output()
        .with_context(|| format!("run {ripr_bin} review-comments"))?;

    if !output.status.success() {
        anyhow::bail!(
            "{ripr_bin} review-comments failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    validate_review_outputs(&out_dir)?;
    println!("ripr-review-comments: wrote review guidance under {RIPR_REVIEW_DIR}/");
    Ok(())
}

fn workspace_root_path() -> Result<PathBuf> {
    std::env::current_dir().context("resolve workspace root")
}

fn ripr_bin() -> String {
    std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string())
}

fn ripr_base() -> String {
    std::env::var("RIPR_BASE").unwrap_or_else(|_| "origin/main".to_string())
}

fn ripr_head() -> String {
    std::env::var("RIPR_HEAD").unwrap_or_else(|_| "HEAD".to_string())
}

fn run_ripr_check(workspace_root: &Path, format: &str, output_path: &Path) -> Result<()> {
    let ripr_bin = ripr_bin();
    let output = Command::new(&ripr_bin)
        .arg("check")
        .arg("--root")
        .arg(workspace_root)
        .arg("--format")
        .arg(format)
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("run {ripr_bin} check --format {format}"))?;

    if !output.status.success() {
        anyhow::bail!(
            "{ripr_bin} check --format {format} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fs::write(output_path, output.stdout)?;
    Ok(())
}

fn validate_pr_outputs(out_dir: &Path) -> Result<()> {
    let json_path = out_dir.join("repo-exposure.json");
    let md_path = out_dir.join("repo-exposure.md");
    validate_json_file(&json_path)?;
    validate_nonempty_file(&md_path)?;
    Ok(())
}

fn validate_review_outputs(out_dir: &Path) -> Result<()> {
    let json_path = out_dir.join("comments.json");
    let md_path = out_dir.join("comments.md");
    validate_json_file(&json_path)?;
    validate_nonempty_file(&md_path)?;
    Ok(())
}

fn validate_json_file(path: &Path) -> Result<()> {
    const FULL_JSON_PARSE_LIMIT_BYTES: u64 = 10 * 1024 * 1024;

    let mut file = fs::File::open(path)
        .with_context(|| format!("missing required file `{}`", path.display()))?;
    let len = file.metadata()?.len();
    if len == 0 {
        anyhow::bail!("required file `{}` is empty", path.display());
    }

    if len <= FULL_JSON_PARSE_LIMIT_BYTES {
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        serde_json::from_slice::<serde_json::Value>(&bytes)
            .with_context(|| format!("invalid JSON in `{}`", path.display()))?;
        return Ok(());
    }

    let mut prefix = [0_u8; 4096];
    let count = file.read(&mut prefix)?;
    let first_non_ws = prefix[..count]
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace());
    if !matches!(first_non_ws, Some(b'{') | Some(b'[')) {
        anyhow::bail!("invalid JSON in `{}`", path.display());
    }

    Ok(())
}

fn validate_nonempty_file(path: &Path) -> Result<()> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("missing required file `{}`", path.display()))?;
    if text.trim().is_empty() {
        anyhow::bail!("required file `{}` is empty", path.display());
    }
    Ok(())
}
