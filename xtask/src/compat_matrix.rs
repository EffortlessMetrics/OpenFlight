// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Generate COMPATIBILITY.md and compat/matrix.json from `compat/` YAML manifests.
//!
//! Unlike `gen-compat`, this command produces:
//! - A vendor-grouped summary table with per-vendor device counts
//! - Capability coverage statistics (axes, buttons, FFB)
//! - Per-vendor device lists in COMPATIBILITY.md
//! - A machine-readable `compat/matrix.json` export
//!
//! Run with: `cargo xtask compat-matrix`

use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fmt::Write as FmtWrite,
    fs,
    path::{Path, PathBuf},
};

// ---------- JSON output types ----------

#[derive(Serialize)]
struct MatrixJson {
    generated_by: &'static str,
    summary: MatrixSummary,
    vendors: Vec<VendorSummary>,
    devices: Vec<MatrixDevice>,
    games: Vec<MatrixGame>,
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
    tier_1: usize,
    tier_2: usize,
    tier_3: usize,
    ffb_devices: usize,
}

#[derive(Serialize, Clone)]
struct MatrixDevice {
    name: String,
    vendor: String,
    vendor_id: String,
    product_id: String,
    axes: u64,
    buttons: u64,
    force_feedback: bool,
    tier: u64,
    test_coverage: DeviceTestCoverage,
}

#[derive(Serialize, Clone)]
struct DeviceTestCoverage {
    simulated: bool,
    hil: bool,
}

#[derive(Serialize)]
struct MatrixGame {
    name: String,
    id: String,
    mechanism: String,
    crate_name: String,
    features: GameFeatures,
    test_coverage: GameTestCoverage,
    tier: u64,
}

#[derive(Serialize)]
struct GameFeatures {
    telemetry_read: bool,
    control_injection: bool,
    force_feedback_translation: bool,
    aircraft_detection: bool,
}

#[derive(Serialize)]
struct GameTestCoverage {
    trace_replay: bool,
    hil: bool,
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
    let mut devices: Vec<MatrixDevice> = Vec::new();
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

    let mut games: Vec<MatrixGame> = Vec::new();
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
    let mut vendor_map: BTreeMap<String, Vec<MatrixDevice>> = BTreeMap::new();
    for d in &devices {
        vendor_map
            .entry(d.vendor.clone())
            .or_default()
            .push(d.clone());
    }

    let vendors: Vec<VendorSummary> = vendor_map
        .iter()
        .map(|(name, devs)| VendorSummary {
            name: name.clone(),
            device_count: devs.len(),
            tier_1: devs.iter().filter(|d| d.tier == 1).count(),
            tier_2: devs.iter().filter(|d| d.tier == 2).count(),
            tier_3: devs.iter().filter(|d| d.tier == 3).count(),
            ffb_devices: devs.iter().filter(|d| d.force_feedback).count(),
        })
        .collect();

    // Tier distributions
    let mut devices_by_tier: BTreeMap<String, usize> = BTreeMap::new();
    for d in &devices {
        *devices_by_tier
            .entry(format!("tier_{}", d.tier))
            .or_insert(0) += 1;
    }
    let mut games_by_tier: BTreeMap<String, usize> = BTreeMap::new();
    for g in &games {
        *games_by_tier
            .entry(format!("tier_{}", g.tier))
            .or_insert(0) += 1;
    }

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

    // ---- Generate COMPATIBILITY.md ----
    let md = generate_markdown(&devices, &games, &vendors, &devices_by_tier, &games_by_tier, &vendor_map)?;
    let md_path = "COMPATIBILITY.md";
    fs::write(md_path, &md).with_context(|| format!("Failed to write {md_path}"))?;

    // ---- Generate compat/matrix.json ----
    let matrix = MatrixJson {
        generated_by: "cargo xtask compat-matrix",
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
    println!("✓ Written {json_path} ({} bytes)", json_out.len());
    println!(
        "  Devices: {}  Games: {}  Vendors: {}",
        matrix.summary.total_devices,
        matrix.summary.total_games,
        matrix.summary.total_vendors
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
    devices: &[MatrixDevice],
    games: &[MatrixGame],
    vendors: &[VendorSummary],
    devices_by_tier: &BTreeMap<String, usize>,
    games_by_tier: &BTreeMap<String, usize>,
    vendor_map: &BTreeMap<String, Vec<MatrixDevice>>,
) -> Result<String> {
    let mut out = String::new();
    writeln!(out, "# OpenFlight Compatibility Matrix")?;
    writeln!(out)?;
    writeln!(
        out,
        "> Auto-generated by `cargo xtask compat-matrix`. Do not edit manually."
    )?;
    writeln!(out)?;

    // Summary
    writeln!(out, "## Summary")?;
    writeln!(out)?;
    writeln!(out, "- **Total devices:** {}", devices.len())?;
    writeln!(out, "- **Total vendors:** {}", vendors.len())?;
    writeln!(out, "- **Total games:** {}", games.len())?;
    writeln!(out, "- **Devices with axes:** {}", devices.iter().filter(|d| d.axes > 0).count())?;
    writeln!(out, "- **Devices with buttons:** {}", devices.iter().filter(|d| d.buttons > 0).count())?;
    writeln!(
        out,
        "- **Devices with force feedback:** {}",
        devices.iter().filter(|d| d.force_feedback).count()
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

    // Vendor summary table
    writeln!(out, "## Vendors")?;
    writeln!(out)?;
    writeln!(
        out,
        "| Vendor | Devices | Tier 1 | Tier 2 | Tier 3 | FFB |"
    )?;
    writeln!(
        out,
        "|--------|---------|--------|--------|--------|-----|"
    )?;
    for v in vendors {
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} |",
            v.name, v.device_count, v.tier_1, v.tier_2, v.tier_3, v.ffb_devices
        )?;
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

    // Support tier legend
    writeln!(out, "## Support Tier Legend")?;
    writeln!(out)?;
    writeln!(out, "| Tier | Meaning |")?;
    writeln!(out, "|------|---------|")?;
    writeln!(
        out,
        "| 1 | Automated trace tests + recent HIL validation |"
    )?;
    writeln!(
        out,
        "| 2 | Automated tests (no HIL) + community confirmation |"
    )?;
    writeln!(out, "| 3 | Compiles / best-effort — no guarantees |")?;

    Ok(out)
}

// ---------- manifest parsing ----------

fn parse_device(path: &Path) -> Result<MatrixDevice> {
    let text = fs::read_to_string(path)?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&text)?;

    Ok(MatrixDevice {
        name: doc["device"]["name"].as_str().unwrap_or("?").to_string(),
        vendor: doc["device"]["vendor"]
            .as_str()
            .unwrap_or("?")
            .to_string(),
        vendor_id: doc["device"]["usb"]["vendor_id"]
            .as_u64()
            .map_or_else(|| "?".to_string(), |v| format!("0x{v:04X}")),
        product_id: doc["device"]["usb"]["product_id"]
            .as_u64()
            .map_or_else(|| "?".to_string(), |v| format!("0x{v:04X}")),
        axes: doc["capabilities"]["axes"]["count"].as_u64().unwrap_or(0),
        buttons: doc["capabilities"]["buttons"].as_u64().unwrap_or(0),
        force_feedback: doc["capabilities"]["force_feedback"]
            .as_bool()
            .unwrap_or(false),
        tier: doc["support"]["tier"].as_u64().unwrap_or(0),
        test_coverage: DeviceTestCoverage {
            simulated: doc["support"]["test_coverage"]["simulated"]
                .as_bool()
                .unwrap_or(false),
            hil: doc["support"]["test_coverage"]["hil"]
                .as_bool()
                .unwrap_or(false),
        },
    })
}

fn parse_game(path: &Path) -> Result<MatrixGame> {
    let text = fs::read_to_string(path)?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&text)?;

    let control_injection = {
        let ci = &doc["features"]["control_injection"];
        let std_events = ci["standard_events"].as_bool().unwrap_or(false);
        let direct = ci["direct"].as_bool().unwrap_or(false);
        let dataref = ci["dataref_write"].as_bool().unwrap_or(false);
        let commands = ci["commands"].as_bool().unwrap_or(false);
        std_events || direct || dataref || commands
    };

    Ok(MatrixGame {
        name: doc["game"]["name"].as_str().unwrap_or("?").to_string(),
        id: doc["game"]["id"].as_str().unwrap_or("?").to_string(),
        mechanism: doc["integration"]["mechanism"]
            .as_str()
            .unwrap_or("?")
            .to_string(),
        crate_name: doc["integration"]["crate"]
            .as_str()
            .unwrap_or("?")
            .to_string(),
        features: GameFeatures {
            telemetry_read: doc["features"]["telemetry_read"]
                .as_bool()
                .unwrap_or(false),
            control_injection,
            force_feedback_translation: doc["features"]["force_feedback_translation"]
                .as_bool()
                .unwrap_or(false),
            aircraft_detection: doc["features"]["aircraft_detection"]
                .as_bool()
                .unwrap_or(false),
        },
        test_coverage: GameTestCoverage {
            trace_replay: doc["test_coverage"]["trace_replay"]
                .as_bool()
                .unwrap_or(false),
            hil: doc["test_coverage"]["hil"].as_bool().unwrap_or(false),
        },
        tier: doc["support_tier"].as_u64().unwrap_or(0),
    })
}

// ---------- helpers ----------

fn collect_manifests(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    collect_yaml(dir, &mut paths);
    paths.sort();
    Ok(paths)
}

fn collect_yaml(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_yaml(&p, out);
        } else if p.extension().is_some_and(|e| e == "yaml") {
            out.push(p);
        }
    }
}

fn bool_to_check(v: bool) -> &'static str {
    if v { "✓" } else { "✗" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
        fs::write(&path, yaml).unwrap();
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
        fs::write(&path, yaml).unwrap();
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
            MatrixDevice {
                name: "Dev A".into(),
                vendor: "Vendor1".into(),
                vendor_id: "0x1234".into(),
                product_id: "0x0001".into(),
                axes: 3,
                buttons: 12,
                force_feedback: true,
                tier: 1,
                test_coverage: DeviceTestCoverage {
                    simulated: true,
                    hil: true,
                },
            },
            MatrixDevice {
                name: "Dev B".into(),
                vendor: "Vendor1".into(),
                vendor_id: "0x1234".into(),
                product_id: "0x0002".into(),
                axes: 2,
                buttons: 8,
                force_feedback: false,
                tier: 2,
                test_coverage: DeviceTestCoverage {
                    simulated: true,
                    hil: false,
                },
            },
            MatrixDevice {
                name: "Dev C".into(),
                vendor: "Vendor2".into(),
                vendor_id: "0x5678".into(),
                product_id: "0x0001".into(),
                axes: 0,
                buttons: 16,
                force_feedback: false,
                tier: 3,
                test_coverage: DeviceTestCoverage {
                    simulated: false,
                    hil: false,
                },
            },
        ];

        let mut vendor_map: BTreeMap<String, Vec<MatrixDevice>> = BTreeMap::new();
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
        let result = collect_manifests(Path::new("nonexistent_dir_abc123"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_bool_to_check() {
        assert_eq!(bool_to_check(true), "✓");
        assert_eq!(bool_to_check(false), "✗");
    }
}
