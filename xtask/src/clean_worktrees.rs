// SPDX-License-Identifier: MIT OR Apache-2.0

//! Git worktree cleanup utility.
//!
//! Run with: `cargo xtask clean-worktrees [--force]`

use anyhow::{Context, Result};
use std::process::Command;

/// A parsed git worktree entry.
#[derive(Debug)]
struct Worktree {
    path: String,
    branch: String,
    is_bare: bool,
}

/// Run worktree cleanup.
pub fn run_clean_worktrees(force: bool) -> Result<()> {
    println!("🌳 Git Worktree Cleanup\n");

    let worktrees = list_worktrees()?;

    if worktrees.is_empty() {
        println!("  No worktrees found (only the main working tree).");
        return Ok(());
    }

    println!("  Found {} worktree(s):\n", worktrees.len());
    println!("  {:<60} {:<30} Status", "Path", "Branch");
    println!("  {}", "-".repeat(100));

    let merged_branches = get_merged_branches()?;
    let mut stale_worktrees = Vec::new();

    for wt in &worktrees {
        if wt.is_bare {
            continue;
        }

        let is_merged = merged_branches.iter().any(|b| wt.branch.contains(b));
        let is_stale = !std::path::Path::new(&wt.path).exists();

        let status = if is_stale {
            "STALE (path missing)"
        } else if is_merged {
            "MERGED"
        } else {
            "active"
        };

        println!("  {:<60} {:<30} {}", wt.path, wt.branch, status);

        if is_stale || is_merged {
            stale_worktrees.push(wt);
        }
    }

    if stale_worktrees.is_empty() {
        println!("\n  ✅ No stale or merged worktrees to clean up.");
        return Ok(());
    }

    println!(
        "\n  Found {} worktree(s) eligible for cleanup.",
        stale_worktrees.len()
    );

    if !force {
        println!("  Run with --force to remove them.");
        println!("\n  First, pruning stale worktree references...");

        // Always safe to prune stale references
        let prune_status = Command::new("git")
            .args(["worktree", "prune"])
            .status()
            .context("Failed to run git worktree prune")?;

        if prune_status.success() {
            println!("  ✅ Pruned stale worktree references.");
        }

        return Ok(());
    }

    // Force mode: remove stale worktrees
    println!("\n  Removing stale/merged worktrees...");

    // Prune stale references first
    let _ = Command::new("git").args(["worktree", "prune"]).status();

    for wt in &stale_worktrees {
        println!("  Removing: {}", wt.path);
        let remove_status = Command::new("git")
            .args(["worktree", "remove", "--force", &wt.path])
            .status()
            .with_context(|| format!("Failed to remove worktree {}", wt.path))?;

        if remove_status.success() {
            println!("    ✅ Removed");
        } else {
            eprintln!("    ⚠ Failed to remove (may need manual cleanup)");
        }
    }

    println!("\n  ✅ Worktree cleanup complete.");
    Ok(())
}

/// List all git worktrees (excluding the main one).
fn list_worktrees() -> Result<Vec<Worktree>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .output()
        .context("Failed to run git worktree list")?;

    if !output.status.success() {
        anyhow::bail!("git worktree list failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_path = String::new();
    let mut current_branch = String::new();
    let mut is_bare = false;
    let mut is_first = true;

    for line in stdout.lines() {
        if line.starts_with("worktree ") {
            // Save previous worktree (skip the first/main one)
            if !current_path.is_empty() && !is_first {
                worktrees.push(Worktree {
                    path: current_path.clone(),
                    branch: current_branch.clone(),
                    is_bare,
                });
            }
            is_first = false;
            current_path = line.strip_prefix("worktree ").unwrap_or("").to_string();
            current_branch = String::new();
            is_bare = false;
        } else if line.starts_with("branch ") {
            current_branch = line
                .strip_prefix("branch refs/heads/")
                .unwrap_or(line.strip_prefix("branch ").unwrap_or(""))
                .to_string();
        } else if line == "bare" {
            is_bare = true;
        }
    }

    // Don't forget the last entry
    if !current_path.is_empty() && !is_first {
        worktrees.push(Worktree {
            path: current_path,
            branch: current_branch,
            is_bare,
        });
    }

    Ok(worktrees)
}

/// Get list of branches already merged into main/HEAD.
fn get_merged_branches() -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["branch", "--merged", "HEAD"])
        .output()
        .context("Failed to run git branch --merged")?;

    if !output.status.success() {
        // Not fatal — just return empty
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let branches: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().trim_start_matches("* ").to_string())
        .filter(|b| !b.is_empty() && b != "main" && b != "master")
        .collect();

    Ok(branches)
}
