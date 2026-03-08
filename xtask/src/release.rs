// SPDX-License-Identifier: MIT OR Apache-2.0

//! Release preparation automation.
//!
//! Run with: `cargo xtask release <version>`
//!
//! Steps:
//! 1. Bump version in all crate Cargo.toml files
//! 2. Update CHANGELOG.md with a new entry
//! 3. Verify all tests pass
//! 4. Create git tag
//! 5. Print checklist of remaining manual steps

use anyhow::{Context, Result};
use chrono::Local;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::changelog;

/// Semver bump type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BumpType {
    Major,
    Minor,
    Patch,
    Pre(String),
}

/// Bump a semver version string according to `bump_type`.
///
/// Returns the new version string (without a leading `v`).
pub fn bump_version(current: &str, bump_type: BumpType) -> Result<String> {
    let current = current.strip_prefix('v').unwrap_or(current);

    // Strip any existing pre-release suffix for the base parse
    let base = current.split('-').next().unwrap_or(current);
    let parts: Vec<&str> = base.split('.').collect();
    if parts.len() != 3 {
        anyhow::bail!("Invalid semver base: '{current}'. Expected MAJOR.MINOR.PATCH[-pre]");
    }

    let major: u64 = parts[0]
        .parse()
        .with_context(|| format!("Invalid major version: '{}'", parts[0]))?;
    let minor: u64 = parts[1]
        .parse()
        .with_context(|| format!("Invalid minor version: '{}'", parts[1]))?;
    let patch: u64 = parts[2]
        .parse()
        .with_context(|| format!("Invalid patch version: '{}'", parts[2]))?;

    let new_version = match bump_type {
        BumpType::Major => format!("{}.0.0", major + 1),
        BumpType::Minor => format!("{major}.{}.0", minor + 1),
        BumpType::Patch => format!("{major}.{minor}.{}", patch + 1),
        BumpType::Pre(pre) => format!("{major}.{minor}.{patch}-{pre}"),
    };

    Ok(new_version)
}

/// Resolve the target version from either an explicit string or a `--bump` flag.
///
/// When `--bump` is given, reads the current version from the root `Cargo.toml`
/// and applies the requested bump.
pub fn resolve_version(explicit: Option<String>, bump: Option<String>) -> Result<String> {
    match (explicit, bump) {
        (Some(v), None) => Ok(v),
        (None, Some(b)) => {
            let bump_type = parse_bump_type(&b)?;
            let current = read_current_version()?;
            bump_version(&current, bump_type)
        }
        (Some(_), Some(_)) => {
            anyhow::bail!("Specify either a version or --bump, not both");
        }
        (None, None) => {
            anyhow::bail!("Provide a version argument or --bump <major|minor|patch|pre:LABEL>");
        }
    }
}

/// Parse a bump type string (e.g. "major", "minor", "patch", "pre:rc.1").
fn parse_bump_type(s: &str) -> Result<BumpType> {
    match s {
        "major" => Ok(BumpType::Major),
        "minor" => Ok(BumpType::Minor),
        "patch" => Ok(BumpType::Patch),
        other if other.starts_with("pre:") => {
            let label = &other[4..];
            if label.is_empty() {
                anyhow::bail!("Pre-release label cannot be empty (use pre:<label>)");
            }
            Ok(BumpType::Pre(label.to_string()))
        }
        other => {
            anyhow::bail!("Unknown bump type: '{other}'. Use major, minor, patch, or pre:<label>")
        }
    }
}

/// Read the workspace package version from the root Cargo.toml.
fn read_current_version() -> Result<String> {
    let content = fs::read_to_string("Cargo.toml").context("Failed to read root Cargo.toml")?;

    let version_re =
        Regex::new(r#"^version\s*=\s*"([^"]*)""#).context("Failed to compile version regex")?;

    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" || trimmed == "[workspace.package]" {
            in_package = true;
        } else if trimmed.starts_with('[') {
            in_package = false;
        }
        if in_package && let Some(caps) = version_re.captures(trimmed) {
            return Ok(caps[1].to_string());
        }
    }

    anyhow::bail!("Could not find version in root Cargo.toml [package] or [workspace.package]")
}

/// Run `cargo xtask prepare-release <version>`.
///
/// This is a streamlined release flow that generates the changelog from
/// conventional commits, bumps versions, and creates a git tag.
pub fn run_prepare_release(version: &str) -> Result<()> {
    validate_version(version)?;

    println!("🚀 Preparing release v{version}\n");

    // Step 1: Generate changelog from conventional commits
    println!("📋 Step 1/4: Generating changelog from conventional commits...");
    let since = get_latest_tag();
    let entries = if let Some(ref tag) = since {
        changelog::read_git_log(tag)?
    } else {
        changelog::read_git_log_all()?
    };

    let ref_name = since.as_deref().unwrap_or("initial commit");
    println!(
        "  Found {} conventional commit(s) since {ref_name}",
        entries.len()
    );

    let today = Local::now().format("%Y-%m-%d");
    let title = format!("## [{version}] - {today}");
    let changelog_text = changelog::generate_changelog(&entries, &title);
    insert_versioned_changelog(&changelog_text)?;
    println!("  ✅ CHANGELOG.md updated\n");

    // Step 2: Bump versions
    println!("📝 Step 2/4: Bumping versions...");
    bump_versions(version)?;
    println!("  ✅ Versions updated\n");

    // Step 3: Create git tag
    println!("🏷️  Step 3/4: Creating git tag...");
    let tag = format!("v{version}");
    let tag_status = Command::new("git")
        .args(["tag", "-a", &tag, "-m", &format!("Release {tag}")])
        .status()
        .context("Failed to create git tag")?;

    if !tag_status.success() {
        eprintln!("  ⚠ Git tag creation failed (tag may already exist)");
    } else {
        println!("  ✅ Created tag {tag}\n");
    }

    // Step 4: Summary
    println!("📋 Step 4/4: Release summary\n");
    println!("  ✅ Changelog generated from {ref_name}");
    println!("  ✅ Version bumped to {version}");
    println!("  ✅ Git tag {tag} created\n");
    println!("  Next steps:");
    println!("    □ Review CHANGELOG.md entry");
    println!("    □ Commit: git commit -am \"chore: release {tag}\"");
    println!("    □ Push: git push && git push origin {tag}");

    Ok(())
}

/// Insert a versioned changelog section into CHANGELOG.md.
fn insert_versioned_changelog(section: &str) -> Result<()> {
    let path = Path::new("CHANGELOG.md");
    let content = fs::read_to_string(path).context("Failed to read CHANGELOG.md")?;

    let new_content = if let Some(start) = content.find("## [Unreleased]") {
        let after_header = start + "## [Unreleased]".len();
        let next_section = content[after_header..]
            .find("\n## [")
            .map_or(content.len(), |i| after_header + i + 1);

        format!(
            "{}## [Unreleased]\n\n{}\n{}",
            &content[..start],
            section.trim_end(),
            &content[next_section..]
        )
    } else {
        let insert_pos = content.find("\n## ").map_or(content.len(), |i| i + 1);
        format!(
            "{}{}\n\n{}",
            &content[..insert_pos],
            section.trim_end(),
            &content[insert_pos..]
        )
    };

    fs::write(path, new_content).context("Failed to write CHANGELOG.md")?;
    Ok(())
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

/// Run release preparation.
pub fn run_release(version: &str) -> Result<()> {
    // Validate version format
    validate_version(version)?;

    println!("🚀 Preparing release v{version}\n");

    // Step 1: Bump versions
    println!("📝 Step 1/5: Bumping versions...");
    bump_versions(version)?;
    println!("  ✅ Versions updated\n");

    // Step 2: Update CHANGELOG
    println!("📋 Step 2/5: Updating CHANGELOG.md...");
    update_changelog(version)?;
    println!("  ✅ CHANGELOG.md updated\n");

    // Step 3: Verify tests
    println!("🧪 Step 3/5: Running tests...");
    let test_status = Command::new("cargo")
        .args(["test", "--workspace"])
        .status()
        .context("Failed to run cargo test")?;

    if !test_status.success() {
        anyhow::bail!("❌ Tests failed. Fix test failures before releasing.");
    }
    println!("  ✅ All tests passed\n");

    // Step 4: Create git tag
    println!("🏷️  Step 4/5: Creating git tag...");
    let tag = format!("v{version}");
    let tag_status = Command::new("git")
        .args(["tag", "-a", &tag, "-m", &format!("Release {tag}")])
        .status()
        .context("Failed to create git tag")?;

    if !tag_status.success() {
        eprintln!("  ⚠ Git tag creation failed (tag may already exist)");
    } else {
        println!("  ✅ Created tag {tag}\n");
    }

    // Step 5: Print checklist
    println!("📋 Step 5/5: Release checklist\n");
    println!("  Automated steps completed:");
    println!("    ✅ Version bumped to {version}");
    println!("    ✅ CHANGELOG.md updated");
    println!("    ✅ Tests passed");
    println!("    ✅ Git tag {tag} created\n");
    println!("  Remaining manual steps:");
    println!("    □ Review CHANGELOG.md entry for completeness");
    println!("    □ Commit version bumps: git commit -am \"chore: release {tag}\"");
    println!("    □ Push commits and tag: git push && git push origin {tag}");
    println!("    □ Create GitHub release from tag {tag}");
    println!("    □ Verify CI pipeline passes on the tag");
    println!("    □ Publish crates if applicable: cargo publish");
    println!("    □ Announce release in project channels");

    Ok(())
}

/// Validate that the version string looks like semver.
fn validate_version(version: &str) -> Result<()> {
    let re = Regex::new(r"^\d+\.\d+\.\d+(-[a-zA-Z0-9.]+)?$")
        .context("Failed to compile version regex")?;
    if !re.is_match(version) {
        anyhow::bail!(
            "Invalid version format: '{}'. Expected semver (e.g., 1.2.3 or 1.2.3-rc.1)",
            version
        );
    }
    Ok(())
}

/// Bump version in all workspace crate Cargo.toml files.
fn bump_versions(version: &str) -> Result<()> {
    let crates_dir = Path::new("crates");
    if !crates_dir.exists() {
        anyhow::bail!("crates/ directory not found. Run from workspace root.");
    }

    let version_re =
        Regex::new(r#"^version\s*=\s*"[^"]*""#).context("Failed to compile version regex")?;
    let replacement = format!("version = \"{version}\"");

    let mut updated = 0;

    // Update root Cargo.toml workspace package version if present
    update_cargo_toml_version(
        Path::new("Cargo.toml"),
        &version_re,
        &replacement,
        &mut updated,
    )?;

    // Update xtask Cargo.toml
    update_cargo_toml_version(
        Path::new("xtask/Cargo.toml"),
        &version_re,
        &replacement,
        &mut updated,
    )?;

    // Update each crate's Cargo.toml
    if let Ok(entries) = fs::read_dir(crates_dir) {
        for entry in entries.flatten() {
            let cargo_toml = entry.path().join("Cargo.toml");
            if cargo_toml.exists() {
                update_cargo_toml_version(&cargo_toml, &version_re, &replacement, &mut updated)?;
            }
        }
    }

    println!("  Updated {updated} Cargo.toml files");
    Ok(())
}

/// Update the version field in a single Cargo.toml.
fn update_cargo_toml_version(
    path: &Path,
    version_re: &Regex,
    replacement: &str,
    count: &mut usize,
) -> Result<()> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let mut in_package = false;
    let mut modified = false;
    let mut new_lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
        } else if trimmed.starts_with('[') && trimmed != "[package]" {
            in_package = false;
        }

        if in_package && version_re.is_match(line) {
            new_lines.push(replacement.to_string());
            modified = true;
        } else {
            new_lines.push(line.to_string());
        }
    }

    if modified {
        let new_content = new_lines.join("\n");
        // Preserve trailing newline if original had one
        let new_content = if content.ends_with('\n') {
            format!("{new_content}\n")
        } else {
            new_content
        };
        fs::write(path, new_content)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        *count += 1;
    }

    Ok(())
}

/// Update CHANGELOG.md with a new release entry.
fn update_changelog(version: &str) -> Result<()> {
    let changelog_path = Path::new("CHANGELOG.md");
    let content = fs::read_to_string(changelog_path).context("Failed to read CHANGELOG.md")?;

    let today = Local::now().format("%Y-%m-%d");
    let new_entry = format!(
        "## [{}] - {}\n\n### Added\n\n### Changed\n\n### Fixed\n\n",
        version, today
    );

    // Insert the new entry after the [Unreleased] section header
    let new_content = if let Some(pos) = content.find("## [Unreleased]") {
        // Find the end of the [Unreleased] line
        let line_end = content[pos..]
            .find('\n')
            .map_or(content.len(), |i| pos + i + 1);
        // Find next section or insert after unreleased header
        let next_section = content[line_end..]
            .find("\n## [")
            .map_or(content.len(), |i| line_end + i + 1);

        format!(
            "{}{}{}",
            &content[..next_section],
            new_entry,
            &content[next_section..]
        )
    } else {
        // No [Unreleased] section, prepend after the header
        let insert_pos = content.find("\n## ").map_or(content.len(), |i| i + 1);
        format!(
            "{}{}{}",
            &content[..insert_pos],
            new_entry,
            &content[insert_pos..]
        )
    };

    fs::write(changelog_path, new_content).context("Failed to write CHANGELOG.md")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── bump_version ────────────────────────────────────────────

    #[test]
    fn bump_major() {
        assert_eq!(bump_version("1.2.3", BumpType::Major).unwrap(), "2.0.0");
    }

    #[test]
    fn bump_minor() {
        assert_eq!(bump_version("1.2.3", BumpType::Minor).unwrap(), "1.3.0");
    }

    #[test]
    fn bump_patch() {
        assert_eq!(bump_version("1.2.3", BumpType::Patch).unwrap(), "1.2.4");
    }

    #[test]
    fn bump_pre_release() {
        assert_eq!(
            bump_version("1.2.3", BumpType::Pre("rc.1".into())).unwrap(),
            "1.2.3-rc.1"
        );
    }

    #[test]
    fn bump_strips_v_prefix() {
        assert_eq!(bump_version("v1.2.3", BumpType::Patch).unwrap(), "1.2.4");
    }

    #[test]
    fn bump_from_pre_release() {
        assert_eq!(
            bump_version("1.2.3-beta.1", BumpType::Patch).unwrap(),
            "1.2.4"
        );
    }

    #[test]
    fn bump_major_from_zero() {
        assert_eq!(bump_version("0.1.0", BumpType::Major).unwrap(), "1.0.0");
    }

    #[test]
    fn bump_invalid_version() {
        assert!(bump_version("not-a-version", BumpType::Patch).is_err());
    }

    #[test]
    fn bump_incomplete_version() {
        assert!(bump_version("1.2", BumpType::Patch).is_err());
    }

    // ── validate_version ────────────────────────────────────────

    #[test]
    fn validate_good_version() {
        assert!(validate_version("1.2.3").is_ok());
        assert!(validate_version("0.0.1").is_ok());
        assert!(validate_version("1.2.3-rc.1").is_ok());
        assert!(validate_version("1.2.3-beta").is_ok());
    }

    #[test]
    fn validate_bad_version() {
        assert!(validate_version("v1.2.3").is_err());
        assert!(validate_version("1.2").is_err());
        assert!(validate_version("abc").is_err());
    }
}
