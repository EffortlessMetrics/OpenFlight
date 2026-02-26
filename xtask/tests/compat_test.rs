// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot and regression tests for compat manifest parsing and generation.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has a parent")
        .to_path_buf()
}

fn compat_devices_dir() -> PathBuf {
    workspace_root().join("compat").join("devices")
}

/// Returns all `.yaml` files under `dir`, sorted.
fn collect_yaml(dir: &PathBuf) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_yaml_inner(dir, &mut paths);
    paths.sort();
    paths
}

fn collect_yaml_inner(dir: &PathBuf, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_yaml_inner(&p, out);
        } else if p.extension().is_some_and(|e| e == "yaml") {
            out.push(p);
        }
    }
}

#[test]
fn test_device_count_above_100() {
    let devices = collect_yaml(&compat_devices_dir());
    assert!(
        devices.len() >= 100,
        "Expected at least 100 device manifests, found {}",
        devices.len()
    );
}

#[test]
fn test_warthog_joystick_manifest_parses() {
    let path = compat_devices_dir()
        .join("thrustmaster")
        .join("warthog-joystick.yaml");
    assert!(path.exists(), "warthog-joystick.yaml should exist at {path:?}");

    let text = std::fs::read_to_string(&path).expect("should read manifest");
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("should parse YAML");

    assert_eq!(
        doc["device"]["name"].as_str().unwrap_or(""),
        "Thrustmaster HOTAS Warthog Joystick"
    );
    assert_eq!(
        doc["device"]["usb"]["vendor_id"].as_u64(),
        Some(0x044F),
        "vendor_id should be 0x044F (Thrustmaster)"
    );
    assert_eq!(
        doc["device"]["usb"]["product_id"].as_u64(),
        Some(0x0402),
        "product_id should be 0x0402 (Warthog Joystick)"
    );
}

#[test]
fn test_schema_version_is_string_not_integer() {
    // schema_version must be stored as a quoted string "1", not a bare integer.
    // This ensures future numeric comparisons don't silently pass for wrong types.
    let path = compat_devices_dir()
        .join("thrustmaster")
        .join("warthog-joystick.yaml");
    let text = std::fs::read_to_string(&path).expect("should read manifest");
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("should parse YAML");

    // schema_version: "1"  → parsed as a String in serde_yaml
    assert_eq!(
        doc["schema_version"].as_str(),
        Some("1"),
        "schema_version must be a quoted string \"1\", not an integer"
    );
    assert!(
        doc["schema_version"].as_u64().is_none(),
        "schema_version must not parse as an integer"
    );
}

#[test]
fn test_all_device_manifests_have_required_fields() {
    let devices = collect_yaml(&compat_devices_dir());
    let mut failures = Vec::new();

    for path in &devices {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                failures.push(format!("{}: read error: {e}", path.display()));
                continue;
            }
        };
        let doc: serde_yaml::Value = match serde_yaml::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!("{}: parse error: {e}", path.display()));
                continue;
            }
        };
        if doc["device"]["name"].as_str().is_none() {
            failures.push(format!("{}: missing device.name", path.display()));
        }
        if doc["support"]["tier"].as_u64().is_none() {
            failures.push(format!("{}: missing support.tier", path.display()));
        }
    }

    assert!(
        failures.is_empty(),
        "Device manifests with missing required fields:\n{}",
        failures.join("\n")
    );
}

#[test]
fn test_warthog_joystick_snapshot() {
    let path = compat_devices_dir()
        .join("thrustmaster")
        .join("warthog-joystick.yaml");
    let text = std::fs::read_to_string(&path).expect("should read manifest");
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("should parse YAML");

    // Snapshot the fields that matter for COMPATIBILITY.md generation.
    let name = doc["device"]["name"].as_str().unwrap_or("?");
    let vid = doc["device"]["usb"]["vendor_id"]
        .as_u64()
        .map_or_else(|| "?".to_string(), |v| format!("0x{v:04X}"));
    let pid = doc["device"]["usb"]["product_id"]
        .as_u64()
        .map_or_else(|| "?".to_string(), |v| format!("0x{v:04X}"));
    let axes = doc["capabilities"]["axes"]["count"]
        .as_u64()
        .map_or_else(|| "?".to_string(), |v| v.to_string());
    let buttons = doc["capabilities"]["buttons"]
        .as_u64()
        .map_or_else(|| "?".to_string(), |v| v.to_string());
    let ffb = doc["capabilities"]["force_feedback"]
        .as_bool()
        .unwrap_or(false);
    let tier = doc["support"]["tier"]
        .as_u64()
        .map_or_else(|| "?".to_string(), |v| v.to_string());

    let row = format!("| {name} | {vid} | {pid} | {axes} | {buttons} | {ffb} | {tier} |");
    insta::assert_snapshot!("warthog_joystick_compat_row", row);
}
