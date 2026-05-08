// SPDX-License-Identifier: MIT OR Apache-2.0

//! Third-party license inventory generation.
//!
//! Replaces the former PowerShell helper with an xtask subcommand that works on
//! every supported developer platform while still delegating crate license
//! detection to `cargo license`.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct CargoLicensePackage {
    name: String,
    version: String,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    repository: Option<String>,
}

struct Component {
    name: String,
    version: String,
    license: String,
    repository: Option<String>,
}

pub fn run_license_inventory() -> Result<()> {
    println!("Generating third-party license inventory...");
    ensure_cargo_license()?;

    println!("Fetching license information from crates.io...");
    let output = Command::new("cargo")
        .args(["license", "--json"])
        .output()
        .context("failed to run `cargo license --json`")?;

    if !output.status.success() {
        bail!(
            "`cargo license --json` failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let packages: Vec<CargoLicensePackage> = serde_json::from_slice(&output.stdout)
        .context("failed to parse `cargo license --json` output")?;

    let mut components: Vec<Component> = packages
        .into_iter()
        .filter(|package| !package.name.starts_with("flight-"))
        .map(|package| Component {
            name: package.name,
            version: package.version,
            license: package.license.unwrap_or_else(|| "Unknown".to_string()),
            repository: package.repository,
        })
        .collect();

    components.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.version.cmp(&b.version)));

    let generated_at = chrono::Utc::now().to_rfc3339();
    write_toml(&components, &generated_at)?;
    write_markdown(&components, &generated_at)?;

    println!(
        "\nGenerated third-party-components.toml with {} dependencies",
        components.len()
    );
    println!("Generated docs/reference/third-party-licenses.md");
    Ok(())
}

fn ensure_cargo_license() -> Result<()> {
    let status = Command::new("cargo")
        .args(["license", "--version"])
        .status()
        .context("failed to check for cargo-license")?;

    if status.success() {
        return Ok(());
    }

    println!("Installing cargo-license...");
    let install_status = Command::new("cargo")
        .args(["install", "cargo-license"])
        .status()
        .context("failed to install cargo-license")?;

    if install_status.success() {
        Ok(())
    } else {
        bail!("`cargo install cargo-license` failed with status {install_status}")
    }
}

fn write_toml(components: &[Component], generated_at: &str) -> Result<()> {
    let mut output = String::new();
    output.push_str("# Third-Party Components Inventory\n");
    output.push_str(&format!("# Generated: {generated_at}\n"));
    output.push_str(&format!("# Total dependencies: {}\n\n", components.len()));

    for component in components {
        output.push_str("\n[[components]]\n");
        output.push_str(&format!("name = \"{}\"\n", toml_escape(&component.name)));
        output.push_str(&format!(
            "version = \"{}\"\n",
            toml_escape(&component.version)
        ));
        output.push_str(&format!(
            "license = \"{}\"\n",
            toml_escape(&component.license)
        ));
        if let Some(repository) = &component.repository {
            output.push_str(&format!("repository = \"{}\"\n", toml_escape(repository)));
        }
    }

    fs::write("third-party-components.toml", output)
        .context("failed to write third-party-components.toml")
}

fn write_markdown(components: &[Component], generated_at: &str) -> Result<()> {
    let mut by_license: HashMap<&str, usize> = HashMap::new();
    for component in components {
        *by_license.entry(&component.license).or_default() += 1;
    }

    let mut licenses: Vec<_> = by_license.into_iter().collect();
    licenses.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    let mut output = String::new();
    output.push_str("# Third-Party Components\n\n");
    output.push_str(&format!("Generated: {generated_at}\n\n"));
    output.push_str(&format!("Total dependencies: {}\n\n", components.len()));
    output.push_str("## License Summary\n\n");
    output.push_str("| License | Count |\n");
    output.push_str("|---------|-------|\n");
    for (license, count) in licenses {
        output.push_str(&format!("| {license} | {count} |\n"));
    }

    output.push_str("\n## All Components\n\n");
    output.push_str("| Name | Version | License |\n");
    output.push_str("|------|---------|---------|\n");
    for component in components {
        output.push_str(&format!(
            "| {} | {} | {} |\n",
            component.name, component.version, component.license
        ));
    }

    fs::write("docs/reference/third-party-licenses.md", output)
        .context("failed to write docs/reference/third-party-licenses.md")
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
