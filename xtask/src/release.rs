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
