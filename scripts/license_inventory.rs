#!/usr/bin/env -S cargo +nightly -Zscript
//! Third-party license inventory generator
//!
//! Parses Cargo.lock and generates a third-party-components.toml file
//! with license information for all dependencies.
//!
//! Requirements: 12.1, 12.2
//!
//! Usage:
//!   cargo +nightly -Zscript scripts/license_inventory.rs
//!
//! Or with cargo-script:
//!   cargo script scripts/license_inventory.rs

//! ```cargo
//! [dependencies]
//! cargo-lock = "10.0"
//! serde = { version = "1.0", features = ["derive"] }
//! toml = "0.8"
//! anyhow = "1.0"
//! reqwest = { version = "0.12", features = ["blocking", "json"] }
//! ```

use anyhow::{Context, Result};
use cargo_lock::Lockfile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Third-party component information
#[derive(Debug, Serialize, Deserialize)]
struct ThirdPartyComponent {
    name: String,
    version: String,
    license: String,
    license_url: Option<String>,
    repository: Option<String>,
    description: Option<String>,
}

/// Complete inventory
#[derive(Debug, Serialize, Deserialize)]
struct ThirdPartyInventory {
    generated_at: String,
    total_dependencies: usize,
    components: Vec<ThirdPartyComponent>,
}

/// Crates.io API response for crate info
#[derive(Debug, Deserialize)]
struct CrateInfo {
    #[serde(rename = "crate")]
    krate: CrateData,
}

#[derive(Debug, Deserialize)]
struct CrateData {
    description: Option<String>,
    license: Option<String>,
    repository: Option<String>,
}

fn main() -> Result<()> {
    println!("Generating third-party license inventory...");

    // Parse Cargo.lock
    let lockfile = Lockfile::load("Cargo.lock").context("Failed to load Cargo.lock")?;

    let mut components = Vec::new();
    let client = reqwest::blocking::Client::builder()
        .user_agent("flight-hub-license-inventory/1.0")
        .build()?;

    for package in &lockfile.packages {
        // Skip our own crates
        if package.name.as_str().starts_with("flight-") {
            continue;
        }

        // Skip workspace members
        if package.source.is_none() {
            continue;
        }

        println!("  Processing: {} {}", package.name, package.version);

        // Try to get license info from crates.io
        let (license, description, repository) = match get_crate_info(
            &client,
            &package.name.as_str(),
            &package.version.to_string(),
        ) {
            Ok(info) => (
                info.krate.license.unwrap_or_else(|| "Unknown".to_string()),
                info.krate.description,
                info.krate.repository,
            ),
            Err(e) => {
                eprintln!(
                    "    Warning: Could not fetch info for {}: {}",
                    package.name, e
                );
                ("Unknown".to_string(), None, None)
            }
        };

        let license_url = get_license_url(&license);

        components.push(ThirdPartyComponent {
            name: package.name.as_str().to_string(),
            version: package.version.to_string(),
            license,
            license_url,
            repository,
            description,
        });
    }

    // Sort by name
    components.sort_by(|a, b| a.name.cmp(&b.name));

    let inventory = ThirdPartyInventory {
        generated_at: chrono::Utc::now().to_rfc3339(),
        total_dependencies: components.len(),
        components,
    };

    // Write TOML output
    let toml_content =
        toml::to_string_pretty(&inventory).context("Failed to serialize inventory")?;

    fs::write("third-party-components.toml", &toml_content)
        .context("Failed to write third-party-components.toml")?;

    println!(
        "\nGenerated third-party-components.toml with {} dependencies",
        inventory.total_dependencies
    );

    // Also generate a summary for docs
    generate_summary(&inventory)?;

    Ok(())
}

fn get_crate_info(
    client: &reqwest::blocking::Client,
    name: &str,
    _version: &str,
) -> Result<CrateInfo> {
    let url = format!("https://crates.io/api/v1/crates/{}", name);

    let response = client
        .get(&url)
        .send()
        .context("Failed to fetch crate info")?;

    if !response.status().is_success() {
        anyhow::bail!("API returned status {}", response.status());
    }

    let info: CrateInfo = response.json().context("Failed to parse crate info")?;

    Ok(info)
}

fn get_license_url(license: &str) -> Option<String> {
    // Map common licenses to their URLs
    let license_urls: HashMap<&str, &str> = [
        ("MIT", "https://opensource.org/licenses/MIT"),
        ("Apache-2.0", "https://www.apache.org/licenses/LICENSE-2.0"),
        (
            "BSD-2-Clause",
            "https://opensource.org/licenses/BSD-2-Clause",
        ),
        (
            "BSD-3-Clause",
            "https://opensource.org/licenses/BSD-3-Clause",
        ),
        ("ISC", "https://opensource.org/licenses/ISC"),
        ("MPL-2.0", "https://www.mozilla.org/en-US/MPL/2.0/"),
        ("Zlib", "https://opensource.org/licenses/Zlib"),
        ("Unlicense", "https://unlicense.org/"),
        (
            "CC0-1.0",
            "https://creativecommons.org/publicdomain/zero/1.0/",
        ),
    ]
    .into_iter()
    .collect();

    // Handle dual licenses like "MIT OR Apache-2.0"
    for (key, url) in &license_urls {
        if license.contains(key) {
            return Some(url.to_string());
        }
    }

    None
}

fn generate_summary(inventory: &ThirdPartyInventory) -> Result<()> {
    let mut summary = String::new();
    summary.push_str("# Third-Party Components\n\n");
    summary.push_str(&format!("Generated: {}\n\n", inventory.generated_at));
    summary.push_str(&format!(
        "Total dependencies: {}\n\n",
        inventory.total_dependencies
    ));

    // Group by license
    let mut by_license: HashMap<String, Vec<&ThirdPartyComponent>> = HashMap::new();
    for component in &inventory.components {
        by_license
            .entry(component.license.clone())
            .or_default()
            .push(component);
    }

    summary.push_str("## License Summary\n\n");
    summary.push_str("| License | Count |\n");
    summary.push_str("|---------|-------|\n");

    let mut licenses: Vec<_> = by_license.iter().collect();
    licenses.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    for (license, components) in &licenses {
        summary.push_str(&format!("| {} | {} |\n", license, components.len()));
    }

    summary.push_str("\n## All Components\n\n");
    summary.push_str("| Name | Version | License |\n");
    summary.push_str("|------|---------|--------|\n");

    for component in &inventory.components {
        summary.push_str(&format!(
            "| {} | {} | {} |\n",
            component.name, component.version, component.license
        ));
    }

    fs::write("docs/reference/third-party-licenses.md", &summary)
        .context("Failed to write third-party-licenses.md")?;

    println!("Generated docs/reference/third-party-licenses.md");

    Ok(())
}
