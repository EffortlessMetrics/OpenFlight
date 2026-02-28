// SPDX-License-Identifier: MIT OR Apache-2.0

//! Device coverage report from `compat/devices/` manifests.
//!
//! Run with: `cargo xtask device-report [--json]`

use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Parsed device info from a manifest.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub name: String,
    pub vendor: String,
    pub tier: u64,
    pub simulated_test: bool,
    pub hil_test: bool,
    pub manifest_path: String,
}

/// Aggregated device report.
#[derive(Debug, Serialize)]
pub struct DeviceReport {
    pub total_devices: usize,
    pub per_vendor: BTreeMap<String, usize>,
    pub tier_distribution: BTreeMap<String, usize>,
    pub devices_missing_tests: Vec<DeviceInfo>,
    pub devices: Vec<DeviceInfo>,
}

/// Entry point for `cargo xtask device-report`.
pub fn run_device_report(json_output: bool) -> Result<()> {
    let compat_dir = Path::new("compat").join("devices");
    if !compat_dir.exists() {
        anyhow::bail!("compat/devices/ directory not found. Run from workspace root.");
    }

    let manifests = collect_yaml_manifests(&compat_dir)?;
    let report = build_report(&manifests)?;

    if json_output {
        let json =
            serde_json::to_string_pretty(&report).context("Failed to serialize report to JSON")?;
        println!("{json}");
    } else {
        print_table(&report);
    }

    Ok(())
}

/// Collect all `.yaml` files recursively from a directory.
pub fn collect_yaml_manifests(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_yaml_recursive(dir, &mut paths);
    paths.sort();
    Ok(paths)
}

fn collect_yaml_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_yaml_recursive(&p, out);
        } else if p.extension().is_some_and(|e| e == "yaml") {
            out.push(p);
        }
    }
}

/// Parse a single device manifest YAML into `DeviceInfo`.
pub fn parse_device_manifest(path: &Path) -> Result<DeviceInfo> {
    let text =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&text)
        .with_context(|| format!("Invalid YAML in {}", path.display()))?;

    Ok(DeviceInfo {
        name: doc["device"]["name"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        vendor: doc["device"]["vendor"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        tier: doc["support"]["tier"].as_u64().unwrap_or(0),
        simulated_test: doc["support"]["test_coverage"]["simulated"]
            .as_bool()
            .unwrap_or(false),
        hil_test: doc["support"]["test_coverage"]["hil"]
            .as_bool()
            .unwrap_or(false),
        manifest_path: path.display().to_string(),
    })
}

/// Build a report from a list of manifest paths.
pub fn build_report(manifests: &[PathBuf]) -> Result<DeviceReport> {
    let mut devices = Vec::new();
    let mut per_vendor: BTreeMap<String, usize> = BTreeMap::new();
    let mut tier_distribution: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing_tests = Vec::new();

    for path in manifests {
        match parse_device_manifest(path) {
            Ok(info) => {
                *per_vendor.entry(info.vendor.clone()).or_insert(0) += 1;
                *tier_distribution
                    .entry(format!("tier_{}", info.tier))
                    .or_insert(0) += 1;

                if !info.simulated_test && !info.hil_test {
                    missing_tests.push(info.clone());
                }

                devices.push(info);
            }
            Err(e) => {
                eprintln!("  ⚠ Skipping {}: {}", path.display(), e);
            }
        }
    }

    Ok(DeviceReport {
        total_devices: devices.len(),
        per_vendor,
        tier_distribution,
        devices_missing_tests: missing_tests,
        devices,
    })
}

/// Print the report as a formatted table.
fn print_table(report: &DeviceReport) {
    println!("📋 Device Coverage Report\n");

    // Summary
    println!("  Total devices: {}", report.total_devices);
    println!(
        "  Devices with tests: {}",
        report.total_devices - report.devices_missing_tests.len()
    );
    println!(
        "  Devices missing tests: {}\n",
        report.devices_missing_tests.len()
    );

    // Tier distribution
    println!("  Tier Distribution:");
    println!("  {:<12} {:>6}", "Tier", "Count");
    println!("  {}", "-".repeat(20));
    for (tier, count) in &report.tier_distribution {
        println!("  {:<12} {:>6}", tier, count);
    }

    // Per-vendor counts
    println!("\n  Per-Vendor Counts:");
    println!("  {:<30} {:>6}", "Vendor", "Count");
    println!("  {}", "-".repeat(38));
    for (vendor, count) in &report.per_vendor {
        println!("  {:<30} {:>6}", vendor, count);
    }

    // Missing test coverage
    if !report.devices_missing_tests.is_empty() {
        println!("\n  ⚠ Devices Missing Test Coverage:");
        println!("  {:<35} {:<25} {:>5}", "Device", "Vendor", "Tier");
        println!("  {}", "-".repeat(67));
        for device in &report.devices_missing_tests {
            println!(
                "  {:<35} {:<25} {:>5}",
                device.name, device.vendor, device.tier
            );
        }
    } else {
        println!("\n  ✅ All devices have test coverage!");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_manifest(dir: &Path, vendor_dir: &str, filename: &str, content: &str) {
        let vendor_path = dir.join(vendor_dir);
        fs::create_dir_all(&vendor_path).unwrap();
        fs::write(vendor_path.join(filename), content).unwrap();
    }

    #[test]
    fn test_parse_device_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("test_device.yaml");
        fs::write(
            &manifest,
            r#"
schema_version: "1"
device:
  name: "Test Stick"
  vendor: "TestVendor"
  usb:
    vendor_id: 0x1234
    product_id: 0x5678
capabilities:
  axes:
    count: 3
  buttons: 12
  force_feedback: false
support:
  tier: 2
  test_coverage:
    simulated: true
    hil: false
"#,
        )
        .unwrap();

        let info = parse_device_manifest(&manifest).unwrap();
        assert_eq!(info.name, "Test Stick");
        assert_eq!(info.vendor, "TestVendor");
        assert_eq!(info.tier, 2);
        assert!(info.simulated_test);
        assert!(!info.hil_test);
    }

    #[test]
    fn test_build_report_aggregation() {
        let dir = tempfile::tempdir().unwrap();
        let devices_dir = dir.path().join("devices");

        create_test_manifest(
            &devices_dir,
            "vendor-a",
            "stick.yaml",
            r#"
device:
  name: "Stick A"
  vendor: "Vendor A"
support:
  tier: 1
  test_coverage:
    simulated: true
    hil: true
"#,
        );

        create_test_manifest(
            &devices_dir,
            "vendor-a",
            "throttle.yaml",
            r#"
device:
  name: "Throttle A"
  vendor: "Vendor A"
support:
  tier: 2
  test_coverage:
    simulated: true
    hil: false
"#,
        );

        create_test_manifest(
            &devices_dir,
            "vendor-b",
            "pedals.yaml",
            r#"
device:
  name: "Pedals B"
  vendor: "Vendor B"
support:
  tier: 3
  test_coverage:
    simulated: false
    hil: false
"#,
        );

        let manifests = collect_yaml_manifests(&devices_dir).unwrap();
        assert_eq!(manifests.len(), 3);

        let report = build_report(&manifests).unwrap();
        assert_eq!(report.total_devices, 3);
        assert_eq!(report.per_vendor["Vendor A"], 2);
        assert_eq!(report.per_vendor["Vendor B"], 1);
        assert_eq!(report.tier_distribution["tier_1"], 1);
        assert_eq!(report.tier_distribution["tier_2"], 1);
        assert_eq!(report.tier_distribution["tier_3"], 1);
        assert_eq!(report.devices_missing_tests.len(), 1);
        assert_eq!(report.devices_missing_tests[0].name, "Pedals B");
    }

    #[test]
    fn test_collect_yaml_manifests_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let manifests = collect_yaml_manifests(dir.path()).unwrap();
        assert!(manifests.is_empty());
    }

    #[test]
    fn test_collect_yaml_manifests_ignores_non_yaml() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("readme.md"), "# not yaml").unwrap();
        fs::write(dir.path().join("device.yaml"), "device:\n  name: Test").unwrap();
        fs::write(dir.path().join("data.json"), "{}").unwrap();

        let manifests = collect_yaml_manifests(dir.path()).unwrap();
        assert_eq!(manifests.len(), 1);
        assert!(manifests[0].to_str().unwrap().ends_with("device.yaml"));
    }

    #[test]
    fn test_build_report_missing_fields_graceful() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("minimal.yaml");
        fs::write(&manifest, "device:\n  name: Minimal\n").unwrap();

        let info = parse_device_manifest(&manifest).unwrap();
        assert_eq!(info.name, "Minimal");
        assert_eq!(info.vendor, "unknown");
        assert_eq!(info.tier, 0);
        assert!(!info.simulated_test);
        assert!(!info.hil_test);
    }
}
