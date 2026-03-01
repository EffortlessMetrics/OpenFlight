// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Generate COMPATIBILITY-MATRIX.md and compat/matrix.json from `compat/` YAML manifests.
//!
//! Produces:
//! - A vendor-grouped summary table with per-vendor device counts
//! - Capability coverage statistics (axes, buttons, FFB)
//! - Per-vendor device lists in COMPATIBILITY-MATRIX.md
//! - A machine-readable `compat/matrix.json` export
//!
//! Run with: `cargo xtask gen-compat` or `cargo xtask generate-compat`

use crate::compat::{
    DeviceEntry, GameEntry, bool_to_check, collect_manifests, compute_devices_by_tier,
    compute_games_by_tier, parse_device, parse_game,
};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as FmtWrite,
    fs,
    path::Path,
};

// ---------- JSON output types ----------

#[derive(Serialize)]
struct MatrixJson {
    generated_by: &'static str,
    summary: MatrixSummary,
    vendors: Vec<VendorSummary>,
    devices: Vec<DeviceEntry>,
    games: Vec<GameEntry>,
}

#[derive(Serialize)]
struct MatrixSummary {
    total_devices: usize,
    total_games: usize,
    total_vendors: usize,
    devices_by_tier: BTreeMap<String, usize>,
    games_by_tier: BTreeMap<String, usize>,
    capability_coverage: CapabilityCoverage,
}

#[derive(Serialize)]
struct CapabilityCoverage {
    devices_with_axes: usize,
    devices_with_buttons: usize,
    devices_with_ffb: usize,
    total_axes: u64,
    total_buttons: u64,
}

#[derive(Serialize)]
struct VendorSummary {
    name: String,
    device_count: usize,
    tiers: BTreeMap<u64, usize>,
    ffb_devices: usize,
}

// ---------- entry point ----------

pub fn run_compat_matrix() -> Result<()> {
    let compat_dir = Path::new("compat");
    if !compat_dir.exists() {
        anyhow::bail!("compat/ directory not found. Run from workspace root.");
    }

    let device_paths = collect_manifests(&compat_dir.join("devices"))?;
    let game_paths = collect_manifests(&compat_dir.join("games"))?;

    // Parse manifests
    let mut devices: Vec<DeviceEntry> = Vec::new();
    let mut parse_errors = 0usize;
    for path in &device_paths {
        match parse_device(path) {
            Ok(d) => devices.push(d),
            Err(e) => {
                eprintln!("⚠ Skipping {}: {e}", path.display());
                parse_errors += 1;
            }
        }
    }

    let mut games: Vec<GameEntry> = Vec::new();
    for path in &game_paths {
        match parse_game(path) {
            Ok(g) => games.push(g),
            Err(e) => {
                eprintln!("⚠ Skipping {}: {e}", path.display());
                parse_errors += 1;
            }
        }
    }

    if parse_errors > 0 {
        eprintln!("⚠ {parse_errors} manifest(s) skipped due to parse errors\n");
    }

    // Aggregate by vendor
    let mut vendor_map: BTreeMap<String, Vec<DeviceEntry>> = BTreeMap::new();
    for d in &devices {
        vendor_map
            .entry(d.vendor.clone())
            .or_default()
            .push(d.clone());
    }

    let vendors: Vec<VendorSummary> = vendor_map
        .iter()
        .map(|(name, devs)| {
            let mut tiers = BTreeMap::new();
            for d in devs {
                *tiers.entry(d.tier).or_insert(0) += 1;
            }
            VendorSummary {
                name: name.clone(),
                device_count: devs.len(),
                tiers,
                ffb_devices: devs.iter().filter(|d| d.force_feedback).count(),
            }
        })
        .collect();

    // Tier distributions
    let devices_by_tier = compute_devices_by_tier(&devices);
    let games_by_tier = compute_games_by_tier(&games);

    // Capability coverage
    let capability_coverage = CapabilityCoverage {
        devices_with_axes: devices.iter().filter(|d| d.axes > 0).count(),
        devices_with_buttons: devices.iter().filter(|d| d.buttons > 0).count(),
        devices_with_ffb: devices.iter().filter(|d| d.force_feedback).count(),
        total_axes: devices.iter().map(|d| d.axes).sum(),
        total_buttons: devices.iter().map(|d| d.buttons).sum(),
    };

    let summary = MatrixSummary {
        total_devices: devices.len(),
        total_games: games.len(),
        total_vendors: vendors.len(),
        devices_by_tier: devices_by_tier.clone(),
        games_by_tier: games_by_tier.clone(),
        capability_coverage,
    };

    // ---- Generate COMPATIBILITY-MATRIX.md ----
    let md = generate_markdown(&games, &vendor_map)?;
    let md_path = "COMPATIBILITY-MATRIX.md";
    fs::write(md_path, &md).with_context(|| format!("Failed to write {md_path}"))?;

    // ---- Generate compat/matrix.json ----
    let matrix = MatrixJson {
        generated_by: "cargo xtask gen-compat",
        summary,
        vendors,
        devices,
        games,
    };
    let json_path = "compat/matrix.json";
    let json_out =
        serde_json::to_string_pretty(&matrix).context("Failed to serialize matrix.json")?;
    fs::write(json_path, format!("{json_out}\n"))
        .with_context(|| format!("Failed to write {json_path}"))?;

    println!("✓ Written {md_path} ({} bytes)", md.len());
    println!("✓ Written {json_path} ({} bytes)", json_out.len() + 1);
    println!(
        "  Devices: {}  Games: {}  Vendors: {}",
        matrix.summary.total_devices, matrix.summary.total_games, matrix.summary.total_vendors
    );
    for (tier, count) in &devices_by_tier {
        println!("  Device {tier}: {count}");
    }
    for (tier, count) in &games_by_tier {
        println!("  Game {tier}: {count}");
    }
    Ok(())
}

// ---------- markdown generation ----------

fn generate_markdown(
    games: &[GameEntry],
    vendor_map: &BTreeMap<String, Vec<DeviceEntry>>,
) -> Result<String> {
    let mut out = String::new();
    writeln!(out, "# OpenFlight Compatibility Matrix")?;
    writeln!(out)?;
    writeln!(
        out,
        "> Auto-generated by `cargo xtask gen-compat`. Do not edit manually."
    )?;
    writeln!(out)?;

    // Compute summary counts from vendor_map
    let all_devices: Vec<&DeviceEntry> = vendor_map.values().flatten().collect();

    // Compute tier distributions
    let mut devices_by_tier: BTreeMap<String, usize> = BTreeMap::new();
    for d in &all_devices {
        *devices_by_tier
            .entry(format!("tier_{}", d.tier))
            .or_insert(0) += 1;
    }
    let mut games_by_tier: BTreeMap<String, usize> = BTreeMap::new();
    for g in games {
        *games_by_tier.entry(format!("tier_{}", g.tier)).or_insert(0) += 1;
    }

    // Compute vendor summaries
    let vendors: Vec<VendorSummary> = vendor_map
        .iter()
        .map(|(name, devs)| {
            let mut tiers = BTreeMap::new();
            for d in devs {
                *tiers.entry(d.tier).or_insert(0) += 1;
            }
            VendorSummary {
                name: name.clone(),
                device_count: devs.len(),
                tiers,
                ffb_devices: devs.iter().filter(|d| d.force_feedback).count(),
            }
        })
        .collect();

    // Summary
    writeln!(out, "## Summary")?;
    writeln!(out)?;
    writeln!(out, "- **Total devices:** {}", all_devices.len())?;
    writeln!(out, "- **Total vendors:** {}", vendors.len())?;
    writeln!(out, "- **Total games:** {}", games.len())?;
    writeln!(
        out,
        "- **Devices with axes:** {}",
        all_devices.iter().filter(|d| d.axes > 0).count()
    )?;
    writeln!(
        out,
        "- **Devices with buttons:** {}",
        all_devices.iter().filter(|d| d.buttons > 0).count()
    )?;
    writeln!(
        out,
        "- **Devices with force feedback:** {}",
        all_devices.iter().filter(|d| d.force_feedback).count()
    )?;
    writeln!(out, "- **Tier distribution (devices):**")?;
    for (tier, count) in devices_by_tier {
        writeln!(out, "  - {tier}: {count}")?;
    }
    writeln!(out, "- **Tier distribution (games):**")?;
    for (tier, count) in games_by_tier {
        writeln!(out, "  - {tier}: {count}")?;
    }
    writeln!(out)?;

    // Collect all tier numbers for dynamic columns
    let all_tiers: BTreeSet<u64> = vendors
        .iter()
        .flat_map(|v| v.tiers.keys().copied())
        .collect();

    // Vendor summary table
    writeln!(out, "## Vendors")?;
    writeln!(out)?;
    // Dynamic header
    let mut header = "| Vendor | Devices |".to_string();
    let mut separator = "|--------|---------|".to_string();
    for tier in &all_tiers {
        write!(header, " Tier {} |", tier)?;
        separator.push_str("--------|");
    }
    header.push_str(" FFB |");
    separator.push_str("-----|");
    writeln!(out, "{header}")?;
    writeln!(out, "{separator}")?;
    for v in vendors {
        write!(out, "| {} | {} |", v.name, v.device_count)?;
        for tier in &all_tiers {
            write!(out, " {} |", v.tiers.get(tier).unwrap_or(&0))?;
        }
        writeln!(out, " {} |", v.ffb_devices)?;
    }
    writeln!(out)?;

    // Per-vendor device tables
    writeln!(out, "## Hardware Devices")?;
    writeln!(out)?;
    for (vendor, devs) in vendor_map {
        writeln!(out, "### {vendor}")?;
        writeln!(out)?;
        writeln!(
            out,
            "| Device | Vendor ID | Product ID | Axes | Buttons | FFB | Tier | Test Coverage |"
        )?;
        writeln!(
            out,
            "|--------|-----------|------------|------|---------|-----|------|---------------|"
        )?;
        for d in devs {
            let ffb = if d.force_feedback { "✓" } else { "✗" };
            let coverage = match (d.test_coverage.simulated, d.test_coverage.hil) {
                (true, true) => "sim + HIL",
                (true, false) => "sim",
                (false, true) => "HIL",
                (false, false) => "none",
            };
            writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                d.name, d.vendor_id, d.product_id, d.axes, d.buttons, ffb, d.tier, coverage
            )?;
        }
        writeln!(out)?;
    }

    // Game integration table
    writeln!(out, "## Game Integrations")?;
    writeln!(out)?;
    writeln!(
        out,
        "| Game | Adapter | Integration | Telemetry | Control Injection | FFB | Aircraft Detection | Tier | HIL Tested |"
    )?;
    writeln!(
        out,
        "|------|---------|-------------|-----------|-------------------|-----|--------------------|------|------------|"
    )?;
    for g in games {
        let telemetry = bool_to_check(g.features.telemetry_read);
        let control = bool_to_check(g.features.control_injection);
        let ffb = bool_to_check(g.features.force_feedback_translation);
        let ac_detect = bool_to_check(g.features.aircraft_detection);
        let hil = bool_to_check(g.test_coverage.hil);
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            g.name, g.crate_name, g.mechanism, telemetry, control, ffb, ac_detect, g.tier, hil
        )?;
    }
    writeln!(out)?;

    // Dynamic support tier legend
    let all_data_tiers: BTreeSet<u64> = vendor_map
        .values()
        .flatten()
        .map(|d| d.tier)
        .chain(games.iter().map(|g| g.tier))
        .collect();
    writeln!(out, "## Support Tier Legend")?;
    writeln!(out)?;
    writeln!(out, "| Tier | Meaning |")?;
    writeln!(out, "|------|---------|")?;
    for tier in &all_data_tiers {
        writeln!(out, "| {} | {} |", tier, tier_meaning(*tier))?;
    }

    Ok(out)
}

fn tier_meaning(tier: u64) -> &'static str {
    match tier {
        1 => "Automated trace tests + recent HIL validation",
        2 => "Automated tests (no HIL) + community confirmation",
        3 => "Compiles / best-effort — no guarantees",
        4 => "Known compatible — limited testing",
        5 => "Experimental / community-reported",
        _ => "Uncategorized",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::TestCoverage;

    #[test]
    fn test_parse_device_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
device:
  name: Test Stick
  vendor: TestVendor
  usb:
    vendor_id: 0x1234
    product_id: 0x5678
capabilities:
  axes:
    count: 3
  buttons: 12
  force_feedback: true
support:
  tier: 2
  test_coverage:
    simulated: true
    hil: false
"#;
        let path = dir.path().join("test.yaml");
        std::fs::write(&path, yaml).unwrap();
        let d = parse_device(&path).unwrap();
        assert_eq!(d.name, "Test Stick");
        assert_eq!(d.vendor, "TestVendor");
        assert_eq!(d.axes, 3);
        assert_eq!(d.buttons, 12);
        assert!(d.force_feedback);
        assert_eq!(d.tier, 2);
        assert!(d.test_coverage.simulated);
        assert!(!d.test_coverage.hil);
    }

    #[test]
    fn test_parse_game_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
game:
  name: Test Sim
  id: test-sim
integration:
  mechanism: SimConnect
  crate: flight-simconnect
features:
  telemetry_read: true
  control_injection:
    standard_events: true
  force_feedback_translation: true
  aircraft_detection: true
test_coverage:
  trace_replay: true
  hil: false
support_tier: 1
"#;
        let path = dir.path().join("test.yaml");
        std::fs::write(&path, yaml).unwrap();
        let g = parse_game(&path).unwrap();
        assert_eq!(g.name, "Test Sim");
        assert_eq!(g.id, "test-sim");
        assert_eq!(g.mechanism, "SimConnect");
        assert!(g.features.telemetry_read);
        assert!(g.features.control_injection);
        assert!(g.features.force_feedback_translation);
        assert_eq!(g.tier, 1);
    }

    #[test]
    fn test_vendor_aggregation() {
        let devices = vec![
            DeviceEntry {
                name: "Dev A".into(),
                vendor: "Vendor1".into(),
                vendor_id: "0x1234".into(),
                product_id: "0x0001".into(),
                axes: 3,
                buttons: 12,
                force_feedback: true,
                tier: 1,
                quirks: vec![],
                test_coverage: TestCoverage {
                    simulated: true,
                    hil: true,
                },
            },
            DeviceEntry {
                name: "Dev B".into(),
                vendor: "Vendor1".into(),
                vendor_id: "0x1234".into(),
                product_id: "0x0002".into(),
                axes: 2,
                buttons: 8,
                force_feedback: false,
                tier: 2,
                quirks: vec![],
                test_coverage: TestCoverage {
                    simulated: true,
                    hil: false,
                },
            },
            DeviceEntry {
                name: "Dev C".into(),
                vendor: "Vendor2".into(),
                vendor_id: "0x5678".into(),
                product_id: "0x0001".into(),
                axes: 0,
                buttons: 16,
                force_feedback: false,
                tier: 3,
                quirks: vec![],
                test_coverage: TestCoverage {
                    simulated: false,
                    hil: false,
                },
            },
        ];

        let mut vendor_map: BTreeMap<String, Vec<DeviceEntry>> = BTreeMap::new();
        for d in &devices {
            vendor_map
                .entry(d.vendor.clone())
                .or_default()
                .push(d.clone());
        }

        assert_eq!(vendor_map.len(), 2);
        assert_eq!(vendor_map["Vendor1"].len(), 2);
        assert_eq!(vendor_map["Vendor2"].len(), 1);
    }

    #[test]
    fn test_collect_manifests_missing_dir() {
        let result = collect_manifests(std::path::Path::new("nonexistent_dir_abc123"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_bool_to_check() {
        assert_eq!(bool_to_check(true), "✓");
        assert_eq!(bool_to_check(false), "✗");
    }
}
