// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Generate `COMPATIBILITY.md` and `compat/compatibility.json` from `compat/` YAML manifests.
//!
//! This module scans device and game YAML manifests under `compat/devices/` and
//! `compat/games/`, validates required fields, and produces:
//!
//! - `COMPATIBILITY.md`  — human-readable compatibility matrix
//! - `compat/compatibility.json` — machine-readable export
//!
//! Run with: `cargo xtask generate-compat`

use crate::compat::{
    DeviceEntry, GameEntry, bool_to_check, collect_manifests, parse_device, parse_game,
};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fmt::Write as FmtWrite,
    fs,
    path::{Path, PathBuf},
};

// ---------- validation ----------

struct ValidationError {
    path: PathBuf,
    message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

// ---------- JSON output ----------

#[derive(Serialize)]
struct CompatJson {
    generated_by: &'static str,
    summary: Summary,
    devices: Vec<DeviceEntry>,
    games: Vec<GameEntry>,
}

#[derive(Serialize)]
struct Summary {
    total_devices: usize,
    total_games: usize,
    total_vendors: usize,
    devices_by_tier: BTreeMap<String, usize>,
    games_by_tier: BTreeMap<String, usize>,
}

// ---------- entry point ----------

/// Entry point for `cargo xtask generate-compat`.
pub fn run_generate_compat() -> Result<()> {
    let compat_dir = Path::new("compat");
    if !compat_dir.exists() {
        anyhow::bail!("compat/ directory not found. Run from workspace root.");
    }

    let device_paths = collect_manifests(&compat_dir.join("devices"))?;
    let game_paths = collect_manifests(&compat_dir.join("games"))?;

    // Validate manifests
    let mut errors = Vec::new();
    for p in &device_paths {
        validate_device_manifest(p, &mut errors);
    }
    for p in &game_paths {
        validate_game_manifest(p, &mut errors);
    }
    if !errors.is_empty() {
        eprintln!(
            "⚠ Manifest validation: {} warning(s) across {} device + {} game manifests:",
            errors.len(),
            device_paths.len(),
            game_paths.len()
        );
        for e in &errors {
            eprintln!("  {e}");
        }
        eprintln!();
    }

    // Parse manifests
    let mut devices = Vec::new();
    for path in &device_paths {
        if let Ok(entry) = parse_device(path) {
            devices.push(entry);
        }
    }
    let mut games = Vec::new();
    for path in &game_paths {
        if let Ok(entry) = parse_game(path) {
            games.push(entry);
        }
    }

    // Sort devices by vendor → name
    devices.sort_by(|a, b| a.vendor.cmp(&b.vendor).then_with(|| a.name.cmp(&b.name)));

    // Compute summaries
    let mut devices_by_tier: BTreeMap<String, usize> = BTreeMap::new();
    for d in &devices {
        *devices_by_tier
            .entry(format!("tier_{}", d.tier))
            .or_insert(0) += 1;
    }
    let mut games_by_tier: BTreeMap<String, usize> = BTreeMap::new();
    for g in &games {
        *games_by_tier.entry(format!("tier_{}", g.tier)).or_insert(0) += 1;
    }

    let vendors: BTreeMap<String, Vec<&DeviceEntry>> = {
        let mut m: BTreeMap<String, Vec<&DeviceEntry>> = BTreeMap::new();
        for d in &devices {
            m.entry(d.vendor.clone()).or_default().push(d);
        }
        m
    };

    let summary = Summary {
        total_devices: devices.len(),
        total_games: games.len(),
        total_vendors: vendors.len(),
        devices_by_tier: devices_by_tier.clone(),
        games_by_tier: games_by_tier.clone(),
    };

    // Generate COMPATIBILITY.md
    let md = generate_markdown(&devices, &games, &vendors, &devices_by_tier, &games_by_tier)?;
    let md_path = "COMPATIBILITY.md";
    fs::write(md_path, &md).with_context(|| format!("Failed to write {md_path}"))?;

    // Generate compat/compatibility.json
    let compat_json = CompatJson {
        generated_by: "cargo xtask generate-compat",
        summary,
        devices,
        games,
    };
    let json_path = "compat/compatibility.json";
    let json_out = serde_json::to_string_pretty(&compat_json)
        .context("Failed to serialize compatibility.json")?;
    fs::write(json_path, format!("{json_out}\n"))
        .with_context(|| format!("Failed to write {json_path}"))?;

    println!("✓ Written {md_path} ({} bytes)", md.len());
    println!("✓ Written {json_path} ({} bytes)", json_out.len() + 1);
    println!(
        "  Devices: {}  Games: {}  Vendors: {}",
        compat_json.summary.total_devices,
        compat_json.summary.total_games,
        compat_json.summary.total_vendors,
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
    devices: &[DeviceEntry],
    games: &[GameEntry],
    vendors: &BTreeMap<String, Vec<&DeviceEntry>>,
    devices_by_tier: &BTreeMap<String, usize>,
    games_by_tier: &BTreeMap<String, usize>,
) -> Result<String> {
    let mut out = String::new();
    writeln!(out, "# OpenFlight Compatibility Matrix")?;
    writeln!(out)?;
    writeln!(
        out,
        "> Auto-generated by `cargo xtask generate-compat`. Do not edit manually."
    )?;
    writeln!(out)?;

    // Summary
    writeln!(out, "## Summary")?;
    writeln!(out)?;
    writeln!(out, "- **Total devices:** {}", devices.len())?;
    writeln!(out, "- **Total vendors:** {}", vendors.len())?;
    writeln!(out, "- **Total games:** {}", games.len())?;
    writeln!(out, "- **Tier distribution (devices):**")?;
    for (tier, count) in devices_by_tier {
        writeln!(out, "  - {tier}: {count}")?;
    }
    writeln!(out, "- **Tier distribution (games):**")?;
    for (tier, count) in games_by_tier {
        writeln!(out, "  - {tier}: {count}")?;
    }
    writeln!(out)?;

    // Device table sorted by vendor → name
    writeln!(out, "## Hardware Devices")?;
    writeln!(out)?;
    writeln!(
        out,
        "| Device | Vendor | VID | PID | Tier | Axes | Buttons | FFB | Quirks |"
    )?;
    writeln!(
        out,
        "|--------|--------|-----|-----|------|------|---------|-----|--------|"
    )?;
    for d in devices {
        let ffb = bool_to_check(d.force_feedback);
        let quirks = if d.quirks.is_empty() {
            "—".to_string()
        } else {
            d.quirks.join(", ")
        };
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            d.name, d.vendor, d.vendor_id, d.product_id, d.tier, d.axes, d.buttons, ffb, quirks
        )?;
    }
    writeln!(out)?;

    // Game table
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
        "| 1 | **Tier 1** — Automated trace tests + recent HIL validation |"
    )?;
    writeln!(
        out,
        "| 2 | **Tier 2** — Automated tests (no HIL) + community confirmation |"
    )?;
    writeln!(
        out,
        "| 3 | **Tier 3** — Compiles / best-effort — no guarantees |"
    )?;

    Ok(out)
}

// ---------- manifest validation ----------

fn validate_device_manifest(path: &Path, errors: &mut Vec<ValidationError>) {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            errors.push(ValidationError {
                path: path.to_path_buf(),
                message: format!("cannot read file: {e}"),
            });
            return;
        }
    };
    let doc: serde_yaml::Value = match serde_yaml::from_str(&text) {
        Ok(d) => d,
        Err(e) => {
            errors.push(ValidationError {
                path: path.to_path_buf(),
                message: format!("invalid YAML: {e}"),
            });
            return;
        }
    };

    let required = [
        ("device.name", &doc["device"]["name"]),
        ("device.vendor", &doc["device"]["vendor"]),
        ("device.usb.vendor_id", &doc["device"]["usb"]["vendor_id"]),
        ("device.usb.product_id", &doc["device"]["usb"]["product_id"]),
        ("capabilities.axes", &doc["capabilities"]["axes"]),
        ("capabilities.buttons", &doc["capabilities"]["buttons"]),
        (
            "capabilities.force_feedback",
            &doc["capabilities"]["force_feedback"],
        ),
        ("support.tier", &doc["support"]["tier"]),
    ];
    for (field, val) in &required {
        if val.is_null() {
            errors.push(ValidationError {
                path: path.to_path_buf(),
                message: format!("missing required field: {field}"),
            });
        }
    }

    if let Some(tier) = doc["support"]["tier"].as_u64()
        && !(1..=3).contains(&tier)
    {
        errors.push(ValidationError {
            path: path.to_path_buf(),
            message: format!("support.tier must be 1, 2, or 3 (got {tier})"),
        });
    }
}

fn validate_game_manifest(path: &Path, errors: &mut Vec<ValidationError>) {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            errors.push(ValidationError {
                path: path.to_path_buf(),
                message: format!("cannot read file: {e}"),
            });
            return;
        }
    };
    let doc: serde_yaml::Value = match serde_yaml::from_str(&text) {
        Ok(d) => d,
        Err(e) => {
            errors.push(ValidationError {
                path: path.to_path_buf(),
                message: format!("invalid YAML: {e}"),
            });
            return;
        }
    };

    let required = [
        ("game.name", &doc["game"]["name"]),
        ("game.id", &doc["game"]["id"]),
        ("integration.mechanism", &doc["integration"]["mechanism"]),
        ("integration.crate", &doc["integration"]["crate"]),
        (
            "features.telemetry_read",
            &doc["features"]["telemetry_read"],
        ),
        (
            "features.control_injection",
            &doc["features"]["control_injection"],
        ),
        ("test_coverage.hil", &doc["test_coverage"]["hil"]),
    ];
    for (field, val) in &required {
        if val.is_null() {
            errors.push(ValidationError {
                path: path.to_path_buf(),
                message: format!("missing required field: {field}"),
            });
        }
    }

    if let Some(tier) = doc["support_tier"].as_u64()
        && !(1..=3).contains(&tier)
    {
        errors.push(ValidationError {
            path: path.to_path_buf(),
            message: format!("support_tier must be 1, 2, or 3 (got {tier})"),
        });
    }
}

// ---------- unit tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{GameTestCoverage, TestCoverage};

    fn write_yaml(dir: &std::path::Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    // ---- device parser tests ----

    #[test]
    fn test_parse_valid_device() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
device:
  name: Test Joystick
  vendor: TestCorp
  usb:
    vendor_id: 0x1234
    product_id: 0xABCD
capabilities:
  axes:
    count: 4
  buttons: 16
  force_feedback: true
quirks:
  - id: QUIRK_ONE
    description: "First quirk"
  - id: QUIRK_TWO
    description: "Second quirk"
support:
  tier: 1
  test_coverage:
    simulated: true
    hil: true
"#;
        let path = write_yaml(dir.path(), "test.yaml", yaml);
        let d = parse_device(&path).unwrap();
        assert_eq!(d.name, "Test Joystick");
        assert_eq!(d.vendor, "TestCorp");
        assert_eq!(d.vendor_id, "0x1234");
        assert_eq!(d.product_id, "0xABCD");
        assert_eq!(d.axes, 4);
        assert_eq!(d.buttons, 16);
        assert!(d.force_feedback);
        assert_eq!(d.tier, 1);
        assert_eq!(d.quirks, vec!["QUIRK_ONE", "QUIRK_TWO"]);
        assert!(d.test_coverage.simulated);
        assert!(d.test_coverage.hil);
    }

    #[test]
    fn test_parse_device_missing_optional_fields() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
device:
  name: Minimal Device
  vendor: MinVendor
  usb:
    vendor_id: 0x0001
    product_id: 0x0002
capabilities:
  axes:
    count: 0
  buttons: 0
  force_feedback: false
support:
  tier: 3
"#;
        let path = write_yaml(dir.path(), "minimal.yaml", yaml);
        let d = parse_device(&path).unwrap();
        assert_eq!(d.name, "Minimal Device");
        assert_eq!(d.quirks, Vec::<String>::new());
        assert!(!d.test_coverage.simulated);
        assert!(!d.test_coverage.hil);
    }

    #[test]
    fn test_validate_device_missing_fields() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
device:
  name: Incomplete
"#;
        let path = write_yaml(dir.path(), "bad.yaml", yaml);
        let mut errors = Vec::new();
        validate_device_manifest(&path, &mut errors);
        assert!(
            errors.len() >= 3,
            "Expected at least 3 validation errors, got {}",
            errors.len()
        );
        let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        assert!(msgs.iter().any(|m| m.contains("device.vendor")));
        assert!(msgs.iter().any(|m| m.contains("support.tier")));
    }

    #[test]
    fn test_validate_device_invalid_tier() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
device:
  name: Bad Tier
  vendor: V
  usb:
    vendor_id: 0x0001
    product_id: 0x0002
capabilities:
  axes:
    count: 1
  buttons: 1
  force_feedback: false
support:
  tier: 99
"#;
        let path = write_yaml(dir.path(), "bad_tier.yaml", yaml);
        let mut errors = Vec::new();
        validate_device_manifest(&path, &mut errors);
        let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("must be 1, 2, or 3")),
            "Expected tier validation error, got: {msgs:?}"
        );
    }

    // ---- game parser tests ----

    #[test]
    fn test_parse_valid_game() {
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
        let path = write_yaml(dir.path(), "game.yaml", yaml);
        let g = parse_game(&path).unwrap();
        assert_eq!(g.name, "Test Sim");
        assert_eq!(g.id, "test-sim");
        assert_eq!(g.mechanism, "SimConnect");
        assert_eq!(g.crate_name, "flight-simconnect");
        assert!(g.features.telemetry_read);
        assert!(g.features.control_injection);
        assert!(g.features.force_feedback_translation);
        assert!(g.features.aircraft_detection);
        assert!(g.test_coverage.trace_replay);
        assert!(!g.test_coverage.hil);
        assert_eq!(g.tier, 1);
    }

    #[test]
    fn test_parse_game_missing_optional_features() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
game:
  name: Minimal Sim
  id: minimal
integration:
  mechanism: UDP
  crate: flight-minimal
features:
  telemetry_read: false
  control_injection:
    direct: false
  force_feedback_translation: false
  aircraft_detection: false
test_coverage:
  trace_replay: false
  hil: false
support_tier: 3
"#;
        let path = write_yaml(dir.path(), "minimal_game.yaml", yaml);
        let g = parse_game(&path).unwrap();
        assert!(!g.features.telemetry_read);
        assert!(!g.features.control_injection);
        assert_eq!(g.tier, 3);
    }

    #[test]
    fn test_validate_game_missing_fields() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
game:
  name: Incomplete Sim
"#;
        let path = write_yaml(dir.path(), "bad_game.yaml", yaml);
        let mut errors = Vec::new();
        validate_game_manifest(&path, &mut errors);
        assert!(
            errors.len() >= 3,
            "Expected at least 3 validation errors, got {}",
            errors.len()
        );
        let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        assert!(msgs.iter().any(|m| m.contains("game.id")));
        assert!(msgs.iter().any(|m| m.contains("integration.mechanism")));
    }

    #[test]
    fn test_validate_game_invalid_tier() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"
game:
  name: Bad Tier Sim
  id: bad-tier
integration:
  mechanism: SimConnect
  crate: flight-test
features:
  telemetry_read: true
  control_injection:
    standard_events: true
test_coverage:
  hil: false
support_tier: 7
"#;
        let path = write_yaml(dir.path(), "bad_tier_game.yaml", yaml);
        let mut errors = Vec::new();
        validate_game_manifest(&path, &mut errors);
        let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("must be 1, 2, or 3")),
            "Expected tier validation error, got: {msgs:?}"
        );
    }

    // ---- generator output tests ----

    #[test]
    fn test_markdown_structure() {
        let devices = vec![
            DeviceEntry {
                name: "Alpha Stick".into(),
                vendor: "Bravo Corp".into(),
                vendor_id: "0x1234".into(),
                product_id: "0x0001".into(),
                axes: 3,
                buttons: 12,
                force_feedback: true,
                tier: 1,
                quirks: vec!["AXIS_BIPOLAR".into()],
                test_coverage: TestCoverage {
                    simulated: true,
                    hil: true,
                },
            },
            DeviceEntry {
                name: "Charlie Throttle".into(),
                vendor: "Alpha Inc".into(),
                vendor_id: "0x5678".into(),
                product_id: "0x0002".into(),
                axes: 6,
                buttons: 32,
                force_feedback: false,
                tier: 2,
                quirks: vec![],
                test_coverage: TestCoverage {
                    simulated: true,
                    hil: false,
                },
            },
        ];
        let games = vec![GameEntry {
            name: "Test Sim".into(),
            id: "test".into(),
            mechanism: "SimConnect".into(),
            crate_name: "flight-simconnect".into(),
            features: crate::compat::GameFeatures {
                telemetry_read: true,
                control_injection: true,
                force_feedback_translation: false,
                aircraft_detection: true,
            },
            test_coverage: GameTestCoverage {
                trace_replay: true,
                hil: false,
            },
            tier: 1,
        }];

        let mut vendors: BTreeMap<String, Vec<&DeviceEntry>> = BTreeMap::new();
        for d in &devices {
            vendors.entry(d.vendor.clone()).or_default().push(d);
        }
        let devices_by_tier = BTreeMap::from([
            ("tier_1".to_string(), 1usize),
            ("tier_2".to_string(), 1usize),
        ]);
        let games_by_tier = BTreeMap::from([("tier_1".to_string(), 1usize)]);

        let md =
            generate_markdown(&devices, &games, &vendors, &devices_by_tier, &games_by_tier)
                .unwrap();

        // Verify required sections
        assert!(md.contains("# OpenFlight Compatibility Matrix"));
        assert!(md.contains("## Summary"));
        assert!(md.contains("## Hardware Devices"));
        assert!(md.contains("## Game Integrations"));
        assert!(md.contains("## Support Tier Legend"));

        // Verify summary stats
        assert!(md.contains("**Total devices:** 2"));
        assert!(md.contains("**Total vendors:** 2"));
        assert!(md.contains("**Total games:** 1"));

        // Verify tier distribution
        assert!(md.contains("tier_1: 1"));
        assert!(md.contains("tier_2: 1"));

        // Verify device table has quirks column
        assert!(md.contains("| Quirks |"));
        assert!(md.contains("AXIS_BIPOLAR"));

        // Verify game table
        assert!(md.contains("Test Sim"));
        assert!(md.contains("SimConnect"));

        // Verify tier legend
        assert!(md.contains("**Tier 1**"));
        assert!(md.contains("**Tier 2**"));
        assert!(md.contains("**Tier 3**"));
    }

    #[test]
    fn test_tier_counts() {
        let devices = vec![
            DeviceEntry {
                name: "D1".into(),
                vendor: "V".into(),
                vendor_id: "0x0000".into(),
                product_id: "0x0001".into(),
                axes: 1,
                buttons: 1,
                force_feedback: false,
                tier: 1,
                quirks: vec![],
                test_coverage: TestCoverage {
                    simulated: true,
                    hil: true,
                },
            },
            DeviceEntry {
                name: "D2".into(),
                vendor: "V".into(),
                vendor_id: "0x0000".into(),
                product_id: "0x0002".into(),
                axes: 1,
                buttons: 1,
                force_feedback: false,
                tier: 2,
                quirks: vec![],
                test_coverage: TestCoverage {
                    simulated: true,
                    hil: false,
                },
            },
            DeviceEntry {
                name: "D3".into(),
                vendor: "V".into(),
                vendor_id: "0x0000".into(),
                product_id: "0x0003".into(),
                axes: 0,
                buttons: 0,
                force_feedback: false,
                tier: 2,
                quirks: vec![],
                test_coverage: TestCoverage {
                    simulated: false,
                    hil: false,
                },
            },
        ];

        let mut by_tier: BTreeMap<String, usize> = BTreeMap::new();
        for d in &devices {
            *by_tier.entry(format!("tier_{}", d.tier)).or_insert(0) += 1;
        }

        assert_eq!(by_tier["tier_1"], 1);
        assert_eq!(by_tier["tier_2"], 2);
        assert_eq!(by_tier.get("tier_3"), None);
    }

    #[test]
    fn test_devices_sorted_by_vendor_then_name() {
        let mut devices = vec![
            DeviceEntry {
                name: "Zeta".into(),
                vendor: "Alpha".into(),
                vendor_id: "0x0000".into(),
                product_id: "0x0001".into(),
                axes: 0,
                buttons: 0,
                force_feedback: false,
                tier: 3,
                quirks: vec![],
                test_coverage: TestCoverage {
                    simulated: false,
                    hil: false,
                },
            },
            DeviceEntry {
                name: "Alpha".into(),
                vendor: "Bravo".into(),
                vendor_id: "0x0000".into(),
                product_id: "0x0002".into(),
                axes: 0,
                buttons: 0,
                force_feedback: false,
                tier: 3,
                quirks: vec![],
                test_coverage: TestCoverage {
                    simulated: false,
                    hil: false,
                },
            },
            DeviceEntry {
                name: "Beta".into(),
                vendor: "Alpha".into(),
                vendor_id: "0x0000".into(),
                product_id: "0x0003".into(),
                axes: 0,
                buttons: 0,
                force_feedback: false,
                tier: 3,
                quirks: vec![],
                test_coverage: TestCoverage {
                    simulated: false,
                    hil: false,
                },
            },
        ];

        devices.sort_by(|a, b| a.vendor.cmp(&b.vendor).then_with(|| a.name.cmp(&b.name)));

        assert_eq!(devices[0].vendor, "Alpha");
        assert_eq!(devices[0].name, "Beta");
        assert_eq!(devices[1].vendor, "Alpha");
        assert_eq!(devices[1].name, "Zeta");
        assert_eq!(devices[2].vendor, "Bravo");
        assert_eq!(devices[2].name, "Alpha");
    }
}
