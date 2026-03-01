// SPDX-License-Identifier: MIT OR Apache-2.0

//! Changelog generation from conventional commits.
//!
//! Run with: `cargo xtask changelog [--since <tag>] [--write]`
//!
//! Reads git log since the last tag (or a specified ref) and generates
//! Keep-a-Changelog formatted output grouped by conventional commit type.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::process::Command;

/// A parsed conventional commit entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogEntry {
    /// Commit type (feat, fix, docs, etc.)
    pub commit_type: String,
    /// Optional scope (e.g., `axis` in `feat(axis): ...`)
    pub scope: Option<String>,
    /// Commit message (the description after `type(scope): `)
    pub message: String,
    /// Abbreviated commit hash
    pub hash: String,
    /// Whether this is a breaking change (`!` suffix or `BREAKING CHANGE` footer)
    pub breaking: bool,
}

/// Map from commit type to human-readable Keep-a-Changelog section header.
fn type_to_section(commit_type: &str) -> &str {
    match commit_type {
        "feat" => "Added",
        "fix" => "Fixed",
        "docs" => "Documentation",
        "test" => "Testing",
        "ci" => "CI",
        "refactor" => "Refactored",
        "perf" => "Performance",
        "chore" => "Chore",
        _ => "Other",
    }
}

/// Section display order (lower = earlier in output).
fn section_order(section: &str) -> u8 {
    match section {
        "Breaking Changes" => 0,
        "Added" => 1,
        "Fixed" => 2,
        "Performance" => 3,
        "Refactored" => 4,
        "Documentation" => 5,
        "Testing" => 6,
        "CI" => 7,
        "Chore" => 8,
        _ => 9,
    }
}

/// Parse a single conventional commit line into a `ChangelogEntry`.
///
/// Expected format: `<hash> <type>[(<scope>)][!]: <message>`
pub fn parse_commit(line: &str) -> Option<ChangelogEntry> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Split into hash and rest
    let (hash, rest) = line.split_once(' ')?;
    let hash = hash.to_string();

    // Match conventional commit pattern: type[(scope)][!]: message
    let rest = rest.trim();

    // Find the colon-space separator
    let colon_pos = rest.find(": ")?;
    let prefix = &rest[..colon_pos];
    let message = rest[colon_pos + 2..].trim().to_string();

    if message.is_empty() {
        return None;
    }

    // Parse prefix: type[(scope)][!]
    let breaking = prefix.ends_with('!');
    let prefix = if breaking {
        &prefix[..prefix.len() - 1]
    } else {
        prefix
    };

    let (commit_type, scope) = if let Some(paren_start) = prefix.find('(') {
        let paren_end = prefix.find(')')?;
        let commit_type = prefix[..paren_start].to_string();
        let scope = prefix[paren_start + 1..paren_end].to_string();
        (commit_type, Some(scope))
    } else {
        (prefix.to_string(), None)
    };

    // Validate commit type
    let valid_types = [
        "feat", "fix", "docs", "test", "ci", "refactor", "perf", "chore", "build", "style",
        "revert",
    ];
    if !valid_types.contains(&commit_type.as_str()) {
        return None;
    }

    Some(ChangelogEntry {
        commit_type,
        scope,
        message,
        hash,
        breaking,
    })
}

/// Group changelog entries by their Keep-a-Changelog section.
pub fn group_by_type(
    entries: &[ChangelogEntry],
) -> BTreeMap<&str, Vec<&ChangelogEntry>> {
    let mut groups: BTreeMap<&str, Vec<&ChangelogEntry>> = BTreeMap::new();

    for entry in entries {
        if entry.breaking {
            groups.entry("Breaking Changes").or_default().push(entry);
        }
        let section = type_to_section(&entry.commit_type);
        groups.entry(section).or_default().push(entry);
    }

    groups
}

/// Format a single changelog entry as a markdown bullet point.
fn format_entry(entry: &ChangelogEntry) -> String {
    let scope_prefix = entry
        .scope
        .as_ref()
        .map_or(String::new(), |s| format!("**{s}**: "));
    let breaking_prefix = if entry.breaking { "**BREAKING** " } else { "" };
    format!(
        "- {breaking_prefix}{scope_prefix}{} ({})",
        entry.message, entry.hash
    )
}

/// Generate a complete changelog string from a list of entries.
pub fn generate_changelog(entries: &[ChangelogEntry], title: &str) -> String {
    if entries.is_empty() {
        return format!("{title}\n\nNo conventional commits found.\n");
    }

    let groups = group_by_type(entries);

    // Sort sections by display order
    let mut sorted_sections: Vec<_> = groups.into_iter().collect();
    sorted_sections.sort_by_key(|(section, _)| section_order(section));

    let mut output = format!("{title}\n");

    for (section, entries) in &sorted_sections {
        output.push_str(&format!("\n### {section}\n\n"));
        for entry in entries {
            output.push_str(&format_entry(entry));
            output.push('\n');
        }
    }

    output
}

/// Get the most recent git tag, or `None` if no tags exist.
fn get_latest_tag() -> Option<String> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
        .ok()?;

    if output.status.success() {
        let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if tag.is_empty() { None } else { Some(tag) }
    } else {
        None
    }
}

/// Read git log since a given ref (tag, commit, etc.) and parse entries.
pub fn read_git_log(since: &str) -> Result<Vec<ChangelogEntry>> {
    let range = format!("{since}..HEAD");
    let output = Command::new("git")
        .args([
            "log",
            "--oneline",
            "--no-decorate",
            "--format=%h %s",
            &range,
        ])
        .output()
        .context("Failed to run git log")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git log failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().filter_map(parse_commit).collect())
}

/// Read all git log entries (when no tag exists).
fn read_git_log_all() -> Result<Vec<ChangelogEntry>> {
    let output = Command::new("git")
        .args(["log", "--oneline", "--no-decorate", "--format=%h %s"])
        .output()
        .context("Failed to run git log")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git log failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().filter_map(parse_commit).collect())
}

/// Main entry point for `cargo xtask changelog`.
pub fn run_changelog(since: Option<&str>, write: bool) -> Result<()> {
    let (entries, ref_name) = if let Some(since) = since {
        (read_git_log(since)?, since.to_string())
    } else if let Some(tag) = get_latest_tag() {
        let entries = read_git_log(&tag)?;
        (entries, tag)
    } else {
        (read_git_log_all()?, "initial commit".to_string())
    };

    println!(
        "Found {} conventional commit(s) since {ref_name}",
        entries.len()
    );

    let title = "## [Unreleased]";
    let changelog = generate_changelog(&entries, title);

    if write {
        write_changelog(&changelog)?;
        println!("✅ CHANGELOG.md updated");
    } else {
        println!("\n{changelog}");
    }

    Ok(())
}

/// Splice the generated changelog into CHANGELOG.md, replacing the
/// `[Unreleased]` section while preserving everything else.
fn write_changelog(new_unreleased: &str) -> Result<()> {
    let path = std::path::Path::new("CHANGELOG.md");
    let content = std::fs::read_to_string(path).context("Failed to read CHANGELOG.md")?;

    let new_content = if let Some(start) = content.find("## [Unreleased]") {
        // Find where the next versioned section begins
        let after_header = start + "## [Unreleased]".len();
        let next_section = content[after_header..]
            .find("\n## [")
            .map_or(content.len(), |i| after_header + i + 1);

        format!(
            "{}{}\n{}",
            &content[..start],
            new_unreleased.trim_end(),
            &content[next_section..]
        )
    } else {
        // No [Unreleased] section — prepend after the file header
        let insert_pos = content.find("\n## ").map_or(content.len(), |i| i + 1);
        format!(
            "{}{}\n\n{}",
            &content[..insert_pos],
            new_unreleased.trim_end(),
            &content[insert_pos..]
        )
    };

    std::fs::write(path, new_content).context("Failed to write CHANGELOG.md")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_commit ────────────────────────────────────────────

    #[test]
    fn parse_basic_feat() {
        let entry = parse_commit("abc1234 feat: add new feature").unwrap();
        assert_eq!(entry.commit_type, "feat");
        assert_eq!(entry.scope, None);
        assert_eq!(entry.message, "add new feature");
        assert_eq!(entry.hash, "abc1234");
        assert!(!entry.breaking);
    }

    #[test]
    fn parse_scoped_fix() {
        let entry = parse_commit("def5678 fix(axis): correct deadzone").unwrap();
        assert_eq!(entry.commit_type, "fix");
        assert_eq!(entry.scope.as_deref(), Some("axis"));
        assert_eq!(entry.message, "correct deadzone");
        assert!(!entry.breaking);
    }

    #[test]
    fn parse_breaking_change() {
        let entry = parse_commit("aaa1111 feat!: redesign API").unwrap();
        assert_eq!(entry.commit_type, "feat");
        assert!(entry.breaking);
        assert_eq!(entry.message, "redesign API");
    }

    #[test]
    fn parse_breaking_with_scope() {
        let entry = parse_commit("bbb2222 fix(ipc)!: change protocol").unwrap();
        assert_eq!(entry.commit_type, "fix");
        assert_eq!(entry.scope.as_deref(), Some("ipc"));
        assert!(entry.breaking);
    }

    #[test]
    fn parse_docs_commit() {
        let entry = parse_commit("ccc3333 docs: update README").unwrap();
        assert_eq!(entry.commit_type, "docs");
        assert_eq!(entry.message, "update README");
    }

    #[test]
    fn parse_all_valid_types() {
        let types = [
            "feat", "fix", "docs", "test", "ci", "refactor", "perf", "chore", "build", "style",
            "revert",
        ];
        for t in &types {
            let line = format!("abc1234 {t}: some message");
            let entry = parse_commit(&line);
            assert!(entry.is_some(), "Failed to parse type: {t}");
            assert_eq!(entry.unwrap().commit_type, *t);
        }
    }

    #[test]
    fn reject_invalid_type() {
        assert!(parse_commit("abc1234 yolo: something").is_none());
    }

    #[test]
    fn reject_empty_line() {
        assert!(parse_commit("").is_none());
        assert!(parse_commit("   ").is_none());
    }

    #[test]
    fn reject_missing_message() {
        assert!(parse_commit("abc1234 feat: ").is_none());
    }

    #[test]
    fn reject_no_colon_space() {
        assert!(parse_commit("abc1234 feat add something").is_none());
    }

    #[test]
    fn reject_non_conventional() {
        assert!(parse_commit("abc1234 Merge branch 'main' into feat/x").is_none());
    }

    #[test]
    fn parse_with_extra_whitespace() {
        let entry = parse_commit("  abc1234 feat(core): trimmed  ").unwrap();
        assert_eq!(entry.commit_type, "feat");
        assert_eq!(entry.scope.as_deref(), Some("core"));
        assert_eq!(entry.message, "trimmed");
    }

    // ── group_by_type ───────────────────────────────────────────

    #[test]
    fn group_by_type_basic() {
        let entries = vec![
            ChangelogEntry {
                commit_type: "feat".into(),
                scope: None,
                message: "feature A".into(),
                hash: "aaa".into(),
                breaking: false,
            },
            ChangelogEntry {
                commit_type: "fix".into(),
                scope: None,
                message: "bugfix B".into(),
                hash: "bbb".into(),
                breaking: false,
            },
            ChangelogEntry {
                commit_type: "feat".into(),
                scope: None,
                message: "feature C".into(),
                hash: "ccc".into(),
                breaking: false,
            },
        ];

        let groups = group_by_type(&entries);
        assert_eq!(groups.get("Added").map(|v| v.len()), Some(2));
        assert_eq!(groups.get("Fixed").map(|v| v.len()), Some(1));
    }

    #[test]
    fn group_breaking_changes_separately() {
        let entries = vec![ChangelogEntry {
            commit_type: "feat".into(),
            scope: None,
            message: "breaking thing".into(),
            hash: "xxx".into(),
            breaking: true,
        }];

        let groups = group_by_type(&entries);
        assert!(groups.contains_key("Breaking Changes"));
        // Also listed under its normal section
        assert!(groups.contains_key("Added"));
    }

    #[test]
    fn group_empty_entries() {
        let groups = group_by_type(&[]);
        assert!(groups.is_empty());
    }

    // ── generate_changelog ──────────────────────────────────────

    #[test]
    fn generate_empty_changelog() {
        let output = generate_changelog(&[], "## [Unreleased]");
        assert!(output.contains("No conventional commits found"));
    }

    #[test]
    fn generate_changelog_with_entries() {
        let entries = vec![
            ChangelogEntry {
                commit_type: "feat".into(),
                scope: Some("hid".into()),
                message: "add device enumeration".into(),
                hash: "aaa1234".into(),
                breaking: false,
            },
            ChangelogEntry {
                commit_type: "fix".into(),
                scope: None,
                message: "resolve panic on startup".into(),
                hash: "bbb5678".into(),
                breaking: false,
            },
        ];

        let output = generate_changelog(&entries, "## [Unreleased]");
        assert!(output.contains("### Added"));
        assert!(output.contains("**hid**: add device enumeration"));
        assert!(output.contains("### Fixed"));
        assert!(output.contains("resolve panic on startup"));
    }

    #[test]
    fn generate_changelog_breaking_first() {
        let entries = vec![
            ChangelogEntry {
                commit_type: "fix".into(),
                scope: None,
                message: "normal fix".into(),
                hash: "nnn".into(),
                breaking: false,
            },
            ChangelogEntry {
                commit_type: "feat".into(),
                scope: None,
                message: "breaking api".into(),
                hash: "bbb".into(),
                breaking: true,
            },
        ];

        let output = generate_changelog(&entries, "## [Unreleased]");
        let breaking_pos = output.find("### Breaking Changes").unwrap();
        let added_pos = output.find("### Added").unwrap();
        let fixed_pos = output.find("### Fixed").unwrap();
        assert!(breaking_pos < added_pos);
        assert!(added_pos < fixed_pos);
    }

    #[test]
    fn format_entry_with_scope_and_breaking() {
        let entry = ChangelogEntry {
            commit_type: "feat".into(),
            scope: Some("api".into()),
            message: "new endpoints".into(),
            hash: "abc".into(),
            breaking: true,
        };
        let formatted = format_entry(&entry);
        assert!(formatted.contains("**BREAKING**"));
        assert!(formatted.contains("**api**:"));
        assert!(formatted.contains("new endpoints"));
        assert!(formatted.contains("(abc)"));
    }

    // ── type_to_section mapping ─────────────────────────────────

    #[test]
    fn type_to_section_mappings() {
        assert_eq!(type_to_section("feat"), "Added");
        assert_eq!(type_to_section("fix"), "Fixed");
        assert_eq!(type_to_section("docs"), "Documentation");
        assert_eq!(type_to_section("test"), "Testing");
        assert_eq!(type_to_section("ci"), "CI");
        assert_eq!(type_to_section("refactor"), "Refactored");
        assert_eq!(type_to_section("perf"), "Performance");
        assert_eq!(type_to_section("chore"), "Chore");
        assert_eq!(type_to_section("unknown"), "Other");
    }
}
