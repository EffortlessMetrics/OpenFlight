// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Generate COMPATIBILITY.md and compatibility.json from `compat/` YAML manifests.
//!
//! Run with: `cargo xtask compat-matrix`, `cargo xtask gen-compat`,
//! or `cargo xtask generate-compat`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt::Write as FmtWrite,
    fs,
    path::{Path, PathBuf},
};

// ---------- validation types ----------

/// A manifest validation error.
pub(crate) struct ValidationError {
    pub(crate) path: PathBuf,
    pub(crate) message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

// ---------- JSON output types ----------

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) struct CompatJson {
    pub(crate) generated_by: String,
    pub(crate) devices: Vec<DeviceEntry>,
    pub(crate) games: Vec<GameEntry>,
    pub(crate) summary: Summary,
    pub(crate) badge: BadgeData,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct DeviceEntry {
    pub(crate) name: String,
    pub(crate) vendor: String,
    pub(crate) vendor_id: String,
    pub(crate) product_id: String,
    pub(crate) axes: u64,
    pub(crate) buttons: u64,
    pub(crate) force_feedback: bool,
    pub(crate) tier: u64,
    pub(crate) test_coverage: TestCoverage,
    pub(crate) last_validated: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct GameEntry {
    pub(crate) name: String,
    pub(crate) id: String,
    pub(crate) mechanism: String,
    pub(crate) crate_name: String,
    pub(crate) features: GameFeatures,
    pub(crate) test_coverage: GameTestCoverage,
    pub(crate) tier: u64,
    pub(crate) supported_versions: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct GameFeatures {
    pub(crate) telemetry_read: bool,
    pub(crate) control_injection: bool,
    pub(crate) force_feedback_translation: bool,
    pub(crate) aircraft_detection: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct TestCoverage {
    pub(crate) simulated: bool,
    pub(crate) hil: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct GameTestCoverage {
    pub(crate) trace_replay: bool,
    pub(crate) hil: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct Summary {
    pub(crate) total_devices: usize,
    pub(crate) total_games: usize,
    pub(crate) tier_distribution: BTreeMap<String, usize>,
    pub(crate) game_tier_distribution: BTreeMap<String, usize>,
}

/// CI badge data: device/game counts by tier and test coverage stats.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct BadgeData {
    pub(crate) devices_by_tier: BTreeMap<String, usize>,
    pub(crate) games_by_tier: BTreeMap<String, usize>,
    pub(crate) devices_with_sim_tests: usize,
    pub(crate) devices_with_hil_tests: usize,
    pub(crate) games_with_hil_tests: usize,
}

// ---------- entry point ----------

/// Entry point for `cargo xtask compat-matrix` / `gen-compat` / `generate-compat`.
pub fn run_gen_compat() -> Result<()> {
    let compat_dir = Path::new("compat");
    if !compat_dir.exists() {
        anyhow::bail!("compat/ directory not found. Run from workspace root.");
    }

    let device_paths = collect_manifests(&compat_dir.join("devices"))?;
    let game_paths = collect_manifests(&compat_dir.join("games"))?;

    // --- Validate manifests ---
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

    // --- Parse manifests into structured data ---
    let mut device_entries = Vec::new();
    let mut game_entries = Vec::new();

    for path in &device_paths {
        if let Ok(entry) = parse_device(path) {
            device_entries.push(entry);
        }
    }
    for path in &game_paths {
        if let Ok(entry) = parse_game(path) {
            game_entries.push(entry);
        }
    }

    let out = generate_markdown(&device_entries, &game_entries);

    let md_path = "COMPATIBILITY.md";
    fs::write(md_path, &out).with_context(|| format!("Failed to write {md_path}"))?;

    // --- Generate compat/compatibility.json ---
    let json_path = "compat/compatibility.json";
    let json_out = generate_json(device_entries, game_entries)?;
    fs::write(json_path, format!("{json_out}\n"))
        .with_context(|| format!("Failed to write {json_path}"))?;

    println!("✓ Written {md_path} ({} bytes)", out.len());
    println!("✓ Written {json_path} ({} bytes)", json_out.len());
    Ok(())
}

// ---------- generation helpers (testable) ----------

/// Generate Markdown content from parsed device and game entries.
pub(crate) fn generate_markdown(devices: &[DeviceEntry], games: &[GameEntry]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# OpenFlight Compatibility Matrix");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "> Auto-generated by `cargo xtask compat-matrix`. Do not edit manually."
    );
    let _ = writeln!(out);

    // Summary
    let tier_dist = tier_distribution(devices.iter().map(|d| d.tier));
    let game_tier_dist = tier_distribution(games.iter().map(|g| g.tier));

    let _ = writeln!(out, "## Summary");
    let _ = writeln!(out);
    let _ = writeln!(out, "- **Total devices:** {}", devices.len());
    let _ = writeln!(out, "- **Total games:** {}", games.len());
    let _ = writeln!(out, "- **Tier distribution (devices):**");
    for (tier, count) in &tier_dist {
        let _ = writeln!(out, "  - {tier}: {count}");
    }
    let _ = writeln!(out, "- **Tier distribution (games):**");
    for (tier, count) in &game_tier_dist {
        let _ = writeln!(out, "  - {tier}: {count}");
    }
    let _ = writeln!(out);

    // Device support matrix — grouped by vendor
    let _ = writeln!(out, "## Hardware Devices");
    let _ = writeln!(out);

    let vendors = group_by_vendor(devices);
    for (vendor, devs) in &vendors {
        let _ = writeln!(out, "### {vendor}");
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "| Device | VID | PID | Axes | Buttons | FFB | Tier | Coverage | Last Validated |"
        );
        let _ = writeln!(
            out,
            "|--------|-----|-----|------|---------|-----|------|----------|----------------|"
        );

        for d in devs {
            let ffb = if d.force_feedback { "✓" } else { "✗" };
            let coverage = match (d.test_coverage.simulated, d.test_coverage.hil) {
                (true, true) => "sim + HIL",
                (true, false) => "sim",
                (false, true) => "HIL",
                (false, false) => "none",
            };
            let validated = d.last_validated.as_deref().unwrap_or("—");
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {ffb} | {} | {coverage} | {validated} |",
                d.name, d.vendor_id, d.product_id, d.axes, d.buttons, d.tier,
            );
        }
        let _ = writeln!(out);
    }

    // Game support matrix
    let _ = writeln!(out, "## Game Integrations");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "| Game | Adapter | Integration | Versions | Telemetry | Control | FFB | Aircraft Detect | Tier | HIL |"
    );
    let _ = writeln!(
        out,
        "|------|---------|-------------|----------|-----------|---------|-----|-----------------|------|-----|"
    );

    for g in games {
        let telemetry = bool_to_check(g.features.telemetry_read);
        let control = bool_to_check(g.features.control_injection);
        let ffb = bool_to_check(g.features.force_feedback_translation);
        let ac_detect = bool_to_check(g.features.aircraft_detection);
        let hil = bool_to_check(g.test_coverage.hil);
        let versions = if g.supported_versions.is_empty() {
            "—".to_string()
        } else {
            g.supported_versions.join(", ")
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            g.name,
            g.crate_name,
            g.mechanism,
            versions,
            telemetry,
            control,
            ffb,
            ac_detect,
            g.tier,
            hil,
        );
    }
    let _ = writeln!(out);

    // Support tier legend
    let _ = writeln!(out, "## Support Tier Legend");
    let _ = writeln!(out);
    let _ = writeln!(out, "| Tier | Meaning |");
    let _ = writeln!(out, "|------|---------|");
    let _ = writeln!(out, "| 1 | Automated trace tests + recent HIL validation |");
    let _ = writeln!(
        out,
        "| 2 | Automated tests (no HIL) + community confirmation |"
    );
    let _ = writeln!(out, "| 3 | Compiles / best-effort — no guarantees |");

    out
}

/// Generate JSON string from parsed device and game entries.
pub(crate) fn generate_json(devices: Vec<DeviceEntry>, games: Vec<GameEntry>) -> Result<String> {
    let tier_dist = tier_distribution(devices.iter().map(|d| d.tier));
    let game_tier_dist = tier_distribution(games.iter().map(|g| g.tier));

    let badge = BadgeData {
        devices_by_tier: tier_dist.clone(),
        games_by_tier: game_tier_dist.clone(),
        devices_with_sim_tests: devices.iter().filter(|d| d.test_coverage.simulated).count(),
        devices_with_hil_tests: devices.iter().filter(|d| d.test_coverage.hil).count(),
        games_with_hil_tests: games.iter().filter(|g| g.test_coverage.hil).count(),
    };

    let summary = Summary {
        total_devices: devices.len(),
        total_games: games.len(),
        tier_distribution: tier_dist,
        game_tier_distribution: game_tier_dist,
    };

    let compat_json = CompatJson {
        generated_by: "cargo xtask compat-matrix".to_string(),
        devices,
        games,
        summary,
        badge,
    };

    serde_json::to_string_pretty(&compat_json).context("Failed to serialize compatibility.json")
}

// ---------- manifest validation ----------

pub(crate) fn validate_device_manifest(path: &Path, errors: &mut Vec<ValidationError>) {
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
    validate_device_yaml(&text, path, errors);
}

pub(crate) fn validate_device_yaml(text: &str, path: &Path, errors: &mut Vec<ValidationError>) {
    let doc: serde_yaml::Value = match serde_yaml::from_str(text) {
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

pub(crate) fn validate_game_manifest(path: &Path, errors: &mut Vec<ValidationError>) {
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
    validate_game_yaml(&text, path, errors);
}

pub(crate) fn validate_game_yaml(text: &str, path: &Path, errors: &mut Vec<ValidationError>) {
    let doc: serde_yaml::Value = match serde_yaml::from_str(text) {
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

// ---------- manifest parsing ----------

pub(crate) fn parse_device(path: &Path) -> Result<DeviceEntry> {
    let text = fs::read_to_string(path)?;
    parse_device_yaml(&text)
}

pub(crate) fn parse_device_yaml(text: &str) -> Result<DeviceEntry> {
    let doc: serde_yaml::Value = serde_yaml::from_str(text)?;

    let last_validated = doc["support"]["last_validated"].as_str().map(String::from);

    Ok(DeviceEntry {
        name: doc["device"]["name"].as_str().unwrap_or("?").to_string(),
        vendor: doc["device"]["vendor"].as_str().unwrap_or("?").to_string(),
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
        test_coverage: TestCoverage {
            simulated: doc["support"]["test_coverage"]["simulated"]
                .as_bool()
                .unwrap_or(false),
            hil: doc["support"]["test_coverage"]["hil"]
                .as_bool()
                .unwrap_or(false),
        },
        last_validated,
    })
}

pub(crate) fn parse_game(path: &Path) -> Result<GameEntry> {
    let text = fs::read_to_string(path)?;
    parse_game_yaml(&text)
}

pub(crate) fn parse_game_yaml(text: &str) -> Result<GameEntry> {
    let doc: serde_yaml::Value = serde_yaml::from_str(text)?;

    let control_injection = {
        let ci = &doc["features"]["control_injection"];
        let std_events = ci["standard_events"].as_bool().unwrap_or(false);
        let direct = ci["direct"].as_bool().unwrap_or(false);
        let dataref = ci["dataref_write"].as_bool().unwrap_or(false);
        let commands = ci["commands"].as_bool().unwrap_or(false);
        std_events || direct || dataref || commands
    };

    let supported_versions = doc["supported_versions"]
        .as_sequence()
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(GameEntry {
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
            telemetry_read: doc["features"]["telemetry_read"].as_bool().unwrap_or(false),
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
        supported_versions,
    })
}

// ---------- helpers ----------

/// Collect `.yaml` manifest paths from a directory tree, sorted.
pub(crate) fn collect_manifests(dir: &Path) -> Result<Vec<PathBuf>> {
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

/// Group devices by vendor name, preserving sort order within each group.
fn group_by_vendor(devices: &[DeviceEntry]) -> Vec<(String, Vec<&DeviceEntry>)> {
    let mut map: BTreeMap<String, Vec<&DeviceEntry>> = BTreeMap::new();
    for d in devices {
        map.entry(d.vendor.clone()).or_default().push(d);
    }
    map.into_iter().collect()
}

/// Compute tier distribution from an iterator of tier values.
fn tier_distribution(tiers: impl Iterator<Item = u64>) -> BTreeMap<String, usize> {
    let mut dist = BTreeMap::new();
    for t in tiers {
        *dist.entry(format!("tier_{t}")).or_insert(0) += 1;
    }
    dist
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const SAMPLE_DEVICE_YAML: &str = r#"
schema_version: "1"
device:
  name: "Test Joystick"
  vendor: "TestVendor"
  usb:
    vendor_id: 0x1234
    product_id: 0xABCD
capabilities:
  axes:
    count: 4
  buttons: 12
  force_feedback: true
support:
  tier: 1
  last_validated: "2025-06"
  test_coverage:
    simulated: true
    hil: true
"#;

    const SAMPLE_GAME_YAML: &str = r#"
schema_version: "1"
game:
  name: "Test Sim"
  id: testsim
  developer: "Dev Co"
integration:
  mechanism: SimConnect
  crate: flight-testsim
supported_versions:
  - "v1.0"
  - "v2.0"
features:
  telemetry_read: true
  control_injection:
    standard_events: true
  force_feedback_translation: false
  aircraft_detection: true
test_coverage:
  trace_replay: true
  hil: false
support_tier: 2
"#;

    #[test]
    fn parse_sample_device() {
        let entry = parse_device_yaml(SAMPLE_DEVICE_YAML).unwrap();
        assert_eq!(entry.name, "Test Joystick");
        assert_eq!(entry.vendor, "TestVendor");
        assert_eq!(entry.vendor_id, "0x1234");
        assert_eq!(entry.product_id, "0xABCD");
        assert_eq!(entry.axes, 4);
        assert_eq!(entry.buttons, 12);
        assert!(entry.force_feedback);
        assert_eq!(entry.tier, 1);
        assert!(entry.test_coverage.simulated);
        assert!(entry.test_coverage.hil);
        assert_eq!(entry.last_validated.as_deref(), Some("2025-06"));
    }

    #[test]
    fn parse_sample_game() {
        let entry = parse_game_yaml(SAMPLE_GAME_YAML).unwrap();
        assert_eq!(entry.name, "Test Sim");
        assert_eq!(entry.id, "testsim");
        assert_eq!(entry.mechanism, "SimConnect");
        assert_eq!(entry.crate_name, "flight-testsim");
        assert!(entry.features.telemetry_read);
        assert!(entry.features.control_injection);
        assert!(!entry.features.force_feedback_translation);
        assert!(entry.features.aircraft_detection);
        assert!(entry.test_coverage.trace_replay);
        assert!(!entry.test_coverage.hil);
        assert_eq!(entry.tier, 2);
        assert_eq!(entry.supported_versions, vec!["v1.0", "v2.0"]);
    }

    #[test]
    fn empty_directory_produces_empty_matrix() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = collect_manifests(tmp.path()).unwrap();
        assert!(paths.is_empty());

        let md = generate_markdown(&[], &[]);
        assert!(md.contains("# OpenFlight Compatibility Matrix"));
        assert!(md.contains("**Total devices:** 0"));
        assert!(md.contains("**Total games:** 0"));
    }

    #[test]
    fn nonexistent_directory_produces_empty_vec() {
        let paths = collect_manifests(Path::new("/nonexistent/path")).unwrap();
        assert!(paths.is_empty());
    }

    #[test]
    fn malformed_device_manifest_reports_error() {
        let mut errors = Vec::new();
        validate_device_yaml("not: valid: yaml: [", Path::new("bad.yaml"), &mut errors);
        assert!(!errors.is_empty());
        assert!(errors[0].message.contains("invalid YAML"));
    }

    #[test]
    fn missing_fields_device_manifest_reports_errors() {
        let yaml = "device:\n  name: Foo\n";
        let mut errors = Vec::new();
        validate_device_yaml(yaml, Path::new("partial.yaml"), &mut errors);
        // Should report missing vendor, usb ids, capabilities, tier
        assert!(
            errors.len() >= 4,
            "got {} errors: {:?}",
            errors.len(),
            errors.iter().map(|e| &e.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn malformed_game_manifest_reports_error() {
        let mut errors = Vec::new();
        validate_game_yaml("{{{bad", Path::new("bad.yaml"), &mut errors);
        assert!(!errors.is_empty());
        assert!(errors[0].message.contains("invalid YAML"));
    }

    #[test]
    fn json_round_trip() {
        let device = DeviceEntry {
            name: "Test".into(),
            vendor: "V".into(),
            vendor_id: "0x1234".into(),
            product_id: "0x5678".into(),
            axes: 3,
            buttons: 8,
            force_feedback: true,
            tier: 1,
            test_coverage: TestCoverage {
                simulated: true,
                hil: false,
            },
            last_validated: Some("2025-01".into()),
        };
        let game = GameEntry {
            name: "Sim".into(),
            id: "sim".into(),
            mechanism: "UDP".into(),
            crate_name: "flight-sim".into(),
            features: GameFeatures {
                telemetry_read: true,
                control_injection: false,
                force_feedback_translation: false,
                aircraft_detection: true,
            },
            test_coverage: GameTestCoverage {
                trace_replay: false,
                hil: false,
            },
            tier: 2,
            supported_versions: vec!["v1".into()],
        };

        let json_str = generate_json(vec![device.clone()], vec![game.clone()]).unwrap();
        let parsed: CompatJson = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed.devices.len(), 1);
        assert_eq!(parsed.games.len(), 1);
        assert_eq!(parsed.devices[0], device);
        assert_eq!(parsed.games[0], game);
        assert_eq!(parsed.summary.total_devices, 1);
        assert_eq!(parsed.summary.total_games, 1);
        assert_eq!(parsed.badge.devices_with_sim_tests, 1);
        assert_eq!(parsed.badge.devices_with_hil_tests, 0);
    }

    #[test]
    fn generate_markdown_groups_by_vendor() {
        let d1 = DeviceEntry {
            name: "Stick A".into(),
            vendor: "Bravo".into(),
            vendor_id: "0x0001".into(),
            product_id: "0x0001".into(),
            axes: 2,
            buttons: 4,
            force_feedback: false,
            tier: 2,
            test_coverage: TestCoverage {
                simulated: false,
                hil: false,
            },
            last_validated: None,
        };
        let d2 = DeviceEntry {
            name: "Stick B".into(),
            vendor: "Alpha".into(),
            vendor_id: "0x0002".into(),
            product_id: "0x0002".into(),
            axes: 3,
            buttons: 6,
            force_feedback: true,
            tier: 1,
            test_coverage: TestCoverage {
                simulated: true,
                hil: true,
            },
            last_validated: Some("2025-03".into()),
        };

        let md = generate_markdown(&[d1, d2], &[]);
        // Vendors should appear in alphabetical order: Alpha before Bravo
        let alpha_pos = md.find("### Alpha").unwrap();
        let bravo_pos = md.find("### Bravo").unwrap();
        assert!(alpha_pos < bravo_pos);
        assert!(md.contains("| Stick B |"));
        assert!(md.contains("| 2025-03 |"));
        assert!(md.contains("| — |")); // d1 has no last_validated
    }

    #[test]
    fn generate_markdown_shows_game_versions() {
        let g = GameEntry {
            name: "MySim".into(),
            id: "mysim".into(),
            mechanism: "UDP".into(),
            crate_name: "flight-mysim".into(),
            features: GameFeatures {
                telemetry_read: true,
                control_injection: false,
                force_feedback_translation: false,
                aircraft_detection: false,
            },
            test_coverage: GameTestCoverage {
                trace_replay: false,
                hil: false,
            },
            tier: 3,
            supported_versions: vec!["v1.0".into(), "v2.0".into()],
        };
        let md = generate_markdown(&[], &[g]);
        assert!(md.contains("v1.0, v2.0"));
    }

    #[test]
    fn badge_data_counts_coverage() {
        let devices = vec![
            DeviceEntry {
                name: "A".into(),
                vendor: "V".into(),
                vendor_id: "0x01".into(),
                product_id: "0x01".into(),
                axes: 1,
                buttons: 1,
                force_feedback: false,
                tier: 1,
                test_coverage: TestCoverage {
                    simulated: true,
                    hil: true,
                },
                last_validated: None,
            },
            DeviceEntry {
                name: "B".into(),
                vendor: "V".into(),
                vendor_id: "0x02".into(),
                product_id: "0x02".into(),
                axes: 1,
                buttons: 1,
                force_feedback: false,
                tier: 2,
                test_coverage: TestCoverage {
                    simulated: true,
                    hil: false,
                },
                last_validated: None,
            },
            DeviceEntry {
                name: "C".into(),
                vendor: "V".into(),
                vendor_id: "0x03".into(),
                product_id: "0x03".into(),
                axes: 1,
                buttons: 1,
                force_feedback: false,
                tier: 3,
                test_coverage: TestCoverage {
                    simulated: false,
                    hil: false,
                },
                last_validated: None,
            },
        ];
        let json_str = generate_json(devices, vec![]).unwrap();
        let parsed: CompatJson = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed.badge.devices_with_sim_tests, 2);
        assert_eq!(parsed.badge.devices_with_hil_tests, 1);
        assert_eq!(*parsed.badge.devices_by_tier.get("tier_1").unwrap(), 1);
        assert_eq!(*parsed.badge.devices_by_tier.get("tier_2").unwrap(), 1);
        assert_eq!(*parsed.badge.devices_by_tier.get("tier_3").unwrap(), 1);
    }

    #[test]
    fn collect_manifests_from_temp_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("vendor");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("dev.yaml"), SAMPLE_DEVICE_YAML).unwrap();
        fs::write(sub.join("readme.txt"), "not yaml").unwrap();

        let paths = collect_manifests(tmp.path()).unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("dev.yaml"));
    }

    #[test]
    fn parse_device_from_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.yaml");
        fs::write(&path, SAMPLE_DEVICE_YAML).unwrap();

        let entry = parse_device(&path).unwrap();
        assert_eq!(entry.name, "Test Joystick");
    }

    #[test]
    fn parse_game_from_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("game.yaml");
        fs::write(&path, SAMPLE_GAME_YAML).unwrap();

        let entry = parse_game(&path).unwrap();
        assert_eq!(entry.name, "Test Sim");
        assert_eq!(entry.supported_versions, vec!["v1.0", "v2.0"]);
    }
}
