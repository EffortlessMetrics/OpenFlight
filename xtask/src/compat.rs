// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Shared types and parsing helpers for `compat/` YAML manifests.
//!
//! Used by `compat_matrix` and `generate_compat` to generate compatibility
//! documentation and JSON exports from device/game manifests.

use anyhow::Result;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

// ---------- shared types ----------

#[derive(Serialize, Clone)]
pub struct DeviceEntry {
    pub name: String,
    pub vendor: String,
    pub vendor_id: String,
    pub product_id: String,
    pub axes: u64,
    pub buttons: u64,
    pub force_feedback: bool,
    pub tier: u64,
    pub quirks: Vec<String>,
    pub test_coverage: TestCoverage,
}

#[derive(Serialize, Clone)]
pub struct GameEntry {
    pub name: String,
    pub id: String,
    pub mechanism: String,
    pub crate_name: String,
    pub features: GameFeatures,
    pub test_coverage: GameTestCoverage,
    pub tier: u64,
}

#[derive(Serialize, Clone)]
pub struct GameFeatures {
    pub telemetry_read: bool,
    pub control_injection: bool,
    pub force_feedback_translation: bool,
    pub aircraft_detection: bool,
}

#[derive(Serialize, Clone)]
pub struct TestCoverage {
    pub simulated: bool,
    pub hil: bool,
}

#[derive(Serialize, Clone)]
pub struct GameTestCoverage {
    pub trace_replay: bool,
    pub hil: bool,
}

// ---------- manifest parsing ----------

pub fn parse_device(path: &Path) -> Result<DeviceEntry> {
    let text = fs::read_to_string(path)?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&text)?;

    let quirks = doc["quirks"]
        .as_sequence()
        .map(|seq| {
            seq.iter()
                .filter_map(|q| q["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

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
        quirks,
        test_coverage: TestCoverage {
            simulated: doc["support"]["test_coverage"]["simulated"]
                .as_bool()
                .unwrap_or(false),
            hil: doc["support"]["test_coverage"]["hil"]
                .as_bool()
                .unwrap_or(false),
        },
    })
}

pub fn parse_game(path: &Path) -> Result<GameEntry> {
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
    })
}

// ---------- helpers ----------

/// Compute tier distribution for devices: `{ "tier_1": count, "tier_2": count, … }`.
pub fn compute_devices_by_tier(devices: &[DeviceEntry]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for d in devices {
        *m.entry(format!("tier_{}", d.tier)).or_insert(0) += 1;
    }
    m
}

/// Compute tier distribution for games: `{ "tier_1": count, "tier_2": count, … }`.
pub fn compute_games_by_tier(games: &[GameEntry]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for g in games {
        *m.entry(format!("tier_{}", g.tier)).or_insert(0) += 1;
    }
    m
}

/// Collect `.yaml` manifest paths from a directory tree, sorted.
pub fn collect_manifests(dir: &Path) -> Result<Vec<PathBuf>> {
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

pub fn bool_to_check(v: bool) -> &'static str {
    if v { "✓" } else { "✗" }
}
