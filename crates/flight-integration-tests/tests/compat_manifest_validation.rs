// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Validation tests for device compatibility manifests in `compat/devices/`.
//!
//! These tests walk all YAML files under compat/devices/ and verify that
//! each manifest:
//!   - Parses as valid YAML
//!   - Contains the required top-level fields
//!   - Has VID/PID values in the valid USB range (0x0000–0xFFFF)
//!   - Has a support tier of 1, 2, or 3
//!
//! The count regression guard ensures the manifest corpus does not shrink.

use std::path::PathBuf;
use walkdir::WalkDir;

fn compat_devices_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/flight-integration-tests; go up to workspace root.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..")
        .join("..")
        .join("compat")
        .join("devices")
}

fn yaml_files() -> Vec<PathBuf> {
    WalkDir::new(compat_devices_dir())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().map(|x| x == "yaml").unwrap_or(false)
        })
        .map(|e| e.into_path())
        .collect()
}

/// Regression guard: the manifest corpus must not shrink below 100 entries.
#[test]
fn compat_manifest_count_regression_guard() {
    let files = yaml_files();
    assert!(
        files.len() >= 100,
        "Expected >= 100 device manifests in compat/devices/, found {}",
        files.len()
    );
}

/// Every manifest must parse as valid YAML without errors.
#[test]
fn compat_manifest_all_parse_as_valid_yaml() {
    let files = yaml_files();
    assert!(
        !files.is_empty(),
        "No YAML files found in compat/devices/ — check the path"
    );

    for path in &files {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));
        serde_yaml::from_str::<serde_yaml::Value>(&content).unwrap_or_else(|e| {
            panic!("YAML parse error in {}: {e}", path.display());
        });
    }
}

/// Returns true when a manifest represents a virtual bundle or pack-level logical
/// entry (e.g. VIRTUAL_BUNDLE, MFD pack). These entries intentionally have no
/// single USB product_id; the real PIDs are listed in component manifests.
fn is_virtual_bundle(doc: &serde_yaml::Value) -> bool {
    let virtual_quirk_ids = ["VIRTUAL_BUNDLE", "MFD_PACK_SHARED_USB_HUB"];
    if let Some(quirks) = doc["quirks"].as_sequence() {
        return quirks.iter().any(|q| {
            q["id"]
                .as_str()
                .map(|id| virtual_quirk_ids.contains(&id))
                .unwrap_or(false)
        });
    }
    false
}

/// Every manifest must have the required fields: device.name, device.usb.vendor_id,
/// device.usb.product_id (unless virtual bundle), support.tier, and support.test_coverage.
#[test]
fn compat_manifest_required_fields_present() {
    let mut errors: Vec<String> = Vec::new();

    for path in yaml_files() {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));
        let doc: serde_yaml::Value = serde_yaml::from_str(&content)
            .unwrap_or_else(|e| panic!("YAML parse error in {}: {e}", path.display()));

        let label = path.display().to_string();

        // device.name must be a non-empty string
        match doc["device"]["name"].as_str() {
            None | Some("") => errors.push(format!("{label}: missing or empty device.name")),
            _ => {}
        }

        // device.usb.vendor_id must be present
        if doc["device"]["usb"]["vendor_id"].is_null() {
            errors.push(format!("{label}: missing device.usb.vendor_id"));
        }

        // device.usb.product_id must be present unless this is a virtual bundle/pack
        // whose component PIDs live in separate manifests.
        if doc["device"]["usb"]["product_id"].is_null() && !is_virtual_bundle(&doc) {
            errors.push(format!("{label}: missing device.usb.product_id"));
        }

        // support.tier must be present
        if doc["support"]["tier"].is_null() {
            errors.push(format!("{label}: missing support.tier"));
        }

        // support.test_coverage must be a mapping
        if doc["support"]["test_coverage"].is_null()
            || !doc["support"]["test_coverage"].is_mapping()
        {
            errors.push(format!("{label}: missing or invalid support.test_coverage"));
        }
    }

    assert!(
        errors.is_empty(),
        "Required-field validation failures:\n{}",
        errors.join("\n")
    );
}

/// VID and PID must be integers in the valid USB range 0x0000–0xFFFF.
#[test]
fn compat_manifest_vid_pid_in_valid_usb_range() {
    let mut errors: Vec<String> = Vec::new();

    for path in yaml_files() {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));
        let doc: serde_yaml::Value = serde_yaml::from_str(&content)
            .unwrap_or_else(|e| panic!("YAML parse error in {}: {e}", path.display()));

        let label = path.display().to_string();

        for (field_name, value) in [
            ("device.usb.vendor_id", &doc["device"]["usb"]["vendor_id"]),
            ("device.usb.product_id", &doc["device"]["usb"]["product_id"]),
        ] {
            if value.is_null() {
                // Already caught by required-fields test; skip here.
                continue;
            }
            match value.as_u64() {
                None => errors.push(format!(
                    "{label}: {field_name} is not a numeric value (got {value:?})"
                )),
                Some(v) if v > 0xFFFF => errors.push(format!(
                    "{label}: {field_name} 0x{v:04X} exceeds USB VID/PID range 0x0000–0xFFFF"
                )),
                _ => {}
            }
        }
    }

    assert!(
        errors.is_empty(),
        "VID/PID range validation failures:\n{}",
        errors.join("\n")
    );
}

/// support.tier must be one of 1, 2, or 3.
#[test]
fn compat_manifest_support_tier_is_valid() {
    let mut errors: Vec<String> = Vec::new();

    for path in yaml_files() {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Could not read {}: {e}", path.display()));
        let doc: serde_yaml::Value = serde_yaml::from_str(&content)
            .unwrap_or_else(|e| panic!("YAML parse error in {}: {e}", path.display()));

        let label = path.display().to_string();
        let tier = &doc["support"]["tier"];

        if tier.is_null() {
            // Already caught by required-fields test; skip here.
            continue;
        }

        match tier.as_u64() {
            Some(1) | Some(2) | Some(3) => {}
            Some(v) => errors.push(format!(
                "{label}: support.tier must be 1, 2, or 3 — got {v}"
            )),
            None => errors.push(format!(
                "{label}: support.tier is not a number (got {tier:?})"
            )),
        }
    }

    assert!(
        errors.is_empty(),
        "Support-tier validation failures:\n{}",
        errors.join("\n")
    );
}
