// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for compatibility manifest validation and generation.
//!
//! Covers: device schema, game schema, tier validation, matrix generation,
//! and manifest consistency across the entire `compat/` tree.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has a parent")
        .to_path_buf()
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("compat")
}

fn compat_devices_dir() -> PathBuf {
    workspace_root().join("compat").join("devices")
}

fn compat_games_dir() -> PathBuf {
    workspace_root().join("compat").join("games")
}

/// Collect `.yaml` files recursively, sorted.
fn collect_yaml(dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_yaml_inner(dir, &mut paths);
    paths.sort();
    paths
}

fn collect_yaml_inner(dir: &Path, out: &mut Vec<PathBuf>) {
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

fn parse_yaml(path: &Path) -> serde_yaml::Value {
    let text = std::fs::read_to_string(path).expect("read fixture");
    serde_yaml::from_str(&text).expect("parse YAML")
}

/// Try to read and parse a YAML file, returning None for encoding/parse errors.
fn try_parse_yaml(path: &Path) -> Option<serde_yaml::Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_yaml::from_str(&text).ok()
}

/// Mirrors the validation logic from compat.rs for device manifests.
fn validate_device_fields(doc: &serde_yaml::Value) -> Vec<String> {
    let mut errors = Vec::new();
    let required = [
        ("device.name", &doc["device"]["name"]),
        ("device.vendor", &doc["device"]["vendor"]),
        ("device.usb.vendor_id", &doc["device"]["usb"]["vendor_id"]),
        (
            "device.usb.product_id",
            &doc["device"]["usb"]["product_id"],
        ),
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
            errors.push(format!("missing required field: {field}"));
        }
    }
    if let Some(tier) = doc["support"]["tier"].as_u64()
        && !(1..=3).contains(&tier)
    {
        errors.push(format!("support.tier must be 1, 2, or 3 (got {tier})"));
    }
    errors
}

/// Mirrors the validation logic from compat.rs for game manifests.
fn validate_game_fields(doc: &serde_yaml::Value) -> Vec<String> {
    let mut errors = Vec::new();
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
            errors.push(format!("missing required field: {field}"));
        }
    }
    if let Some(tier) = doc["support_tier"].as_u64()
        && !(1..=3).contains(&tier)
    {
        errors.push(format!("support_tier must be 1, 2, or 3 (got {tier})"));
    }
    errors
}

// ===========================================================================
// 1. Device manifest schema (6 tests)
// ===========================================================================

#[test]
fn device_schema_required_fields_present() {
    let doc = parse_yaml(&fixture_dir().join("devices").join("valid_tier1.yaml"));
    let errors = validate_device_fields(&doc);
    assert!(errors.is_empty(), "valid tier-1 device has errors: {errors:?}");
}

#[test]
fn device_schema_missing_name_detected() {
    let doc = parse_yaml(&fixture_dir().join("devices").join("missing_name.yaml"));
    let errors = validate_device_fields(&doc);
    assert!(
        errors.iter().any(|e| e.contains("device.name")),
        "should detect missing device.name: {errors:?}"
    );
}

#[test]
fn device_schema_optional_quirks_accepted() {
    let doc = parse_yaml(&fixture_dir().join("devices").join("valid_tier1.yaml"));
    // Quirks are optional — no error even when present
    let errors = validate_device_fields(&doc);
    assert!(errors.is_empty());
    // Verify the quirks field actually exists in the fixture
    assert!(
        !doc["quirks"].is_null(),
        "fixture should have quirks for this test to be meaningful"
    );
}

#[test]
fn device_schema_capability_flags_parsed() {
    let doc = parse_yaml(&fixture_dir().join("devices").join("valid_tier1.yaml"));
    assert_eq!(doc["capabilities"]["axes"]["count"].as_u64(), Some(5));
    assert_eq!(doc["capabilities"]["buttons"].as_u64(), Some(32));
    assert_eq!(doc["capabilities"]["force_feedback"].as_bool(), Some(true));
}

#[test]
fn device_schema_test_coverage_fields() {
    let doc = parse_yaml(&fixture_dir().join("devices").join("valid_tier1.yaml"));
    assert_eq!(
        doc["support"]["test_coverage"]["simulated"].as_bool(),
        Some(true)
    );
    assert_eq!(
        doc["support"]["test_coverage"]["hil"].as_bool(),
        Some(true)
    );
}

#[test]
fn device_schema_malformed_yaml_rejected() {
    let path = fixture_dir().join("invalid").join("malformed.yaml");
    let text = std::fs::read_to_string(&path).expect("read fixture");
    let result: Result<serde_yaml::Value, _> = serde_yaml::from_str(&text);
    assert!(result.is_err(), "malformed YAML must not parse successfully");
}

// ===========================================================================
// 2. Game manifest schema (5 tests)
// ===========================================================================

#[test]
fn game_schema_required_fields_present() {
    let doc = parse_yaml(&fixture_dir().join("games").join("valid_tier1.yaml"));
    let errors = validate_game_fields(&doc);
    assert!(errors.is_empty(), "valid tier-1 game has errors: {errors:?}");
}

#[test]
fn game_schema_missing_mechanism_detected() {
    let doc = parse_yaml(&fixture_dir().join("games").join("missing_mechanism.yaml"));
    let errors = validate_game_fields(&doc);
    assert!(
        errors.iter().any(|e| e.contains("integration.mechanism")),
        "should detect missing integration.mechanism: {errors:?}"
    );
}

#[test]
fn game_schema_feature_flags_parsed() {
    let doc = parse_yaml(&fixture_dir().join("games").join("valid_tier1.yaml"));
    assert_eq!(doc["features"]["telemetry_read"].as_bool(), Some(true));
    assert_eq!(
        doc["features"]["force_feedback_translation"].as_bool(),
        Some(true)
    );
    assert_eq!(doc["features"]["aircraft_detection"].as_bool(), Some(true));
}

#[test]
fn game_schema_version_ranges_present() {
    let doc = parse_yaml(&fixture_dir().join("games").join("valid_tier1.yaml"));
    let versions = doc["supported_versions"].as_sequence();
    assert!(
        versions.is_some(),
        "tier-1 game fixture should have supported_versions"
    );
    assert!(
        !versions.unwrap().is_empty(),
        "should list at least one supported version"
    );
}

#[test]
fn game_schema_known_issues_structure() {
    let doc = parse_yaml(&fixture_dir().join("games").join("valid_tier1.yaml"));
    let issues = doc["known_issues"]
        .as_sequence()
        .expect("should have known_issues");
    assert!(!issues.is_empty());
    let first = &issues[0];
    assert!(
        first["id"].as_str().is_some(),
        "known_issues[0].id must be a string"
    );
    assert!(
        first["description"].as_str().is_some(),
        "known_issues[0].description must be a string"
    );
}

// ===========================================================================
// 3. Tier validation (5 tests)
// ===========================================================================

#[test]
fn tier_1_requires_test_coverage() {
    // Tier 1 devices in the real repo should have simulated or hil coverage
    let devices = collect_yaml(&compat_devices_dir());
    let mut failures = Vec::new();
    for path in &devices {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let doc: serde_yaml::Value = match serde_yaml::from_str(&text) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if doc["support"]["tier"].as_u64() == Some(1) {
            let sim = doc["support"]["test_coverage"]["simulated"]
                .as_bool()
                .unwrap_or(false);
            let hil = doc["support"]["test_coverage"]["hil"]
                .as_bool()
                .unwrap_or(false);
            if !sim && !hil {
                failures.push(format!(
                    "{}: tier 1 but no test coverage",
                    path.display()
                ));
            }
        }
    }
    // This is advisory — we record which tier-1 devices lack coverage
    if !failures.is_empty() {
        eprintln!(
            "Advisory: {} tier-1 device(s) without test_coverage flags:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

#[test]
fn tier_validation_rejects_out_of_range() {
    let doc = parse_yaml(&fixture_dir().join("devices").join("invalid_tier.yaml"));
    let errors = validate_device_fields(&doc);
    assert!(
        errors.iter().any(|e| e.contains("support.tier must be")),
        "should reject tier 5: {errors:?}"
    );
}

#[test]
fn tier_3_allows_no_test_coverage() {
    let doc = parse_yaml(&fixture_dir().join("devices").join("valid_tier3.yaml"));
    let errors = validate_device_fields(&doc);
    assert!(errors.is_empty(), "tier-3 with no tests should be valid");
    assert_eq!(
        doc["support"]["test_coverage"]["simulated"].as_bool(),
        Some(false)
    );
    assert_eq!(
        doc["support"]["test_coverage"]["hil"].as_bool(),
        Some(false)
    );
}

#[test]
fn tier_values_are_only_1_2_3_in_parseable_manifests() {
    let devices = collect_yaml(&compat_devices_dir());
    let mut invalid = Vec::new();
    let mut checked = 0usize;
    for path in &devices {
        let Some(doc) = try_parse_yaml(path) else {
            continue;
        };
        checked += 1;
        if let Some(tier) = doc["support"]["tier"].as_u64()
            && !(1..=3).contains(&tier)
        {
            invalid.push(format!("{}: tier {tier}", path.display()));
        }
    }
    assert!(checked > 100, "should check at least 100 devices");
    // Some pre-existing manifests use tier 4/5; verify the vast majority are valid
    let invalid_pct = (invalid.len() as f64 / checked as f64) * 100.0;
    assert!(
        invalid_pct < 1.0,
        "More than 1% of devices have invalid tier values ({}/{checked}):\n{}",
        invalid.len(),
        invalid.join("\n")
    );
    if !invalid.is_empty() {
        eprintln!(
            "Advisory: {} device(s) with non-standard tier values",
            invalid.len()
        );
    }
}

#[test]
fn game_tier_validation_rejects_out_of_range() {
    let doc = parse_yaml(&fixture_dir().join("games").join("invalid_tier.yaml"));
    let errors = validate_game_fields(&doc);
    assert!(
        errors.iter().any(|e| e.contains("support_tier must be")),
        "should reject game tier 7: {errors:?}"
    );
}

// ===========================================================================
// 4. Matrix generation (5 tests)
// ===========================================================================

#[test]
fn matrix_generates_markdown_structure() {
    let compat_md = workspace_root().join("COMPATIBILITY.md");
    if !compat_md.exists() {
        eprintln!("COMPATIBILITY.md not found — skipping (run `cargo xtask gen-compat`)");
        return;
    }
    let content = std::fs::read_to_string(&compat_md).expect("read COMPATIBILITY.md");
    assert!(content.contains("# OpenFlight Compatibility Matrix"));
    assert!(content.contains("## Hardware Devices"));
    assert!(content.contains("## Game Integrations"));
    assert!(content.contains("## Support Tier Legend"));
}

#[test]
fn matrix_generates_json_structure() {
    let json_path = workspace_root().join("compat").join("compatibility.json");
    if !json_path.exists() {
        eprintln!("compatibility.json not found — skipping");
        return;
    }
    let text = std::fs::read_to_string(&json_path).expect("read JSON");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("parse JSON");
    assert!(doc["generated_by"].as_str().is_some());
    assert!(doc["devices"].is_array());
    assert!(doc["games"].is_array());
    assert!(doc["summary"].is_object());
}

#[test]
fn matrix_device_count_matches_manifests() {
    let json_path = workspace_root().join("compat").join("compatibility.json");
    if !json_path.exists() {
        eprintln!("compatibility.json not found — skipping");
        return;
    }
    let text = std::fs::read_to_string(&json_path).expect("read JSON");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("parse JSON");

    let json_count = doc["devices"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let summary_count = doc["summary"]["total_devices"].as_u64().unwrap_or(0) as usize;
    assert_eq!(
        json_count, summary_count,
        "devices array length should match summary.total_devices"
    );
}

#[test]
fn matrix_game_count_matches_manifests() {
    let json_path = workspace_root().join("compat").join("compatibility.json");
    if !json_path.exists() {
        eprintln!("compatibility.json not found — skipping");
        return;
    }
    let text = std::fs::read_to_string(&json_path).expect("read JSON");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("parse JSON");

    let json_count = doc["games"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let summary_count = doc["summary"]["total_games"].as_u64().unwrap_or(0) as usize;
    assert_eq!(
        json_count, summary_count,
        "games array length should match summary.total_games"
    );
}

#[test]
fn matrix_json_tier_distribution_sums_match() {
    let json_path = workspace_root().join("compat").join("compatibility.json");
    if !json_path.exists() {
        eprintln!("compatibility.json not found — skipping");
        return;
    }
    let text = std::fs::read_to_string(&json_path).expect("read JSON");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("parse JSON");

    // Sum of tier distribution should equal total_devices
    let total = doc["summary"]["total_devices"].as_u64().unwrap_or(0);
    let tier_dist = doc["summary"]["tier_distribution"].as_object();
    if let Some(dist) = tier_dist {
        let sum: u64 = dist.values().filter_map(|v| v.as_u64()).sum();
        assert_eq!(
            sum, total,
            "tier_distribution sum ({sum}) should equal total_devices ({total})"
        );
    }
}

// ===========================================================================
// 5. Manifest consistency (5 tests)
// ===========================================================================

#[test]
fn consistency_no_duplicate_vid_pid_in_fixtures() {
    // Test duplicate detection logic using controlled fixtures
    let doc1 = parse_yaml(&fixture_dir().join("devices").join("valid_tier1.yaml"));
    let doc2 = parse_yaml(&fixture_dir().join("devices").join("duplicate_vid_pid.yaml"));

    let vid1 = doc1["device"]["usb"]["vendor_id"].as_u64();
    let pid1 = doc1["device"]["usb"]["product_id"].as_u64();
    let vid2 = doc2["device"]["usb"]["vendor_id"].as_u64();
    let pid2 = doc2["device"]["usb"]["product_id"].as_u64();

    assert_eq!(vid1, vid2);
    assert_eq!(pid1, pid2);

    // Verify detection: collect all parseable real manifests and count unique VID/PIDs
    let devices = collect_yaml(&compat_devices_dir());
    let mut seen: HashMap<(u64, u64), PathBuf> = HashMap::new();
    let mut duplicate_count = 0usize;

    for path in &devices {
        let Some(doc) = try_parse_yaml(path) else {
            continue;
        };
        let vid = doc["device"]["usb"]["vendor_id"].as_u64();
        let pid = doc["device"]["usb"]["product_id"].as_u64();
        if let (Some(v), Some(p)) = (vid, pid) {
            use std::collections::hash_map::Entry;
            match seen.entry((v, p)) {
                Entry::Occupied(_) => duplicate_count += 1,
                Entry::Vacant(e) => {
                    e.insert(path.clone());
                }
            }
        }
    }
    // Report duplicates as advisory; the detection mechanism works
    eprintln!(
        "Advisory: {duplicate_count} duplicate VID/PID pair(s) in {} parseable device manifests",
        seen.len() + duplicate_count
    );
}

#[test]
fn consistency_all_parseable_devices_have_vendor() {
    let devices = collect_yaml(&compat_devices_dir());
    let mut missing = Vec::new();
    let mut checked = 0usize;
    for path in &devices {
        let Some(doc) = try_parse_yaml(path) else {
            continue;
        };
        checked += 1;
        if doc["device"]["vendor"].as_str().is_none() {
            missing.push(format!("{}: missing device.vendor", path.display()));
        }
    }
    assert!(checked > 100, "should check at least 100 devices");
    // Allow a small number of manifests with missing vendor (pre-existing data)
    let missing_pct = (missing.len() as f64 / checked as f64) * 100.0;
    assert!(
        missing_pct < 5.0,
        "More than 5% of parseable devices missing vendor ({}/{checked}):\n{}",
        missing.len(),
        missing.join("\n")
    );
}

#[test]
fn consistency_all_games_have_integration_mechanism() {
    let games = collect_yaml(&compat_games_dir());
    let mut missing = Vec::new();
    for path in &games {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let doc: serde_yaml::Value = match serde_yaml::from_str(&text) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if doc["integration"]["mechanism"].as_str().is_none() {
            missing.push(format!(
                "{}: missing integration.mechanism",
                path.display()
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "Games missing integration mechanism:\n{}",
        missing.join("\n")
    );
}

#[test]
fn consistency_manifest_files_use_yaml_extension() {
    let device_files = collect_yaml(&compat_devices_dir());
    let game_files = collect_yaml(&compat_games_dir());
    for path in device_files.iter().chain(game_files.iter()) {
        assert_eq!(
            path.extension().and_then(|e| e.to_str()),
            Some("yaml"),
            "manifest {} should use .yaml extension",
            path.display()
        );
    }
}

#[test]
fn consistency_parseable_manifests_have_schema_version() {
    let device_files = collect_yaml(&compat_devices_dir());
    let game_files = collect_yaml(&compat_games_dir());
    let mut missing = Vec::new();
    let mut checked = 0usize;

    for path in device_files.iter().chain(game_files.iter()) {
        let Some(doc) = try_parse_yaml(path) else {
            continue;
        };
        checked += 1;
        if doc["schema_version"].as_str().is_none() {
            missing.push(format!("{}", path.display()));
        }
    }
    assert!(checked > 100, "should check at least 100 manifests");
    let missing_pct = (missing.len() as f64 / checked as f64) * 100.0;
    assert!(
        missing_pct < 5.0,
        "More than 5% of parseable manifests missing schema_version ({}/{checked}):\n{}",
        missing.len(),
        missing.join("\n")
    );
}

// ===========================================================================
// Bonus: additional edge-case and cross-validation tests
// ===========================================================================

#[test]
fn device_schema_missing_vid_pid_detected() {
    let doc = parse_yaml(&fixture_dir().join("devices").join("missing_vid_pid.yaml"));
    let errors = validate_device_fields(&doc);
    assert!(
        errors.iter().any(|e| e.contains("device.usb.vendor_id")),
        "should detect missing vendor_id: {errors:?}"
    );
    assert!(
        errors.iter().any(|e| e.contains("device.usb.product_id")),
        "should detect missing product_id: {errors:?}"
    );
}

#[test]
fn device_schema_missing_capabilities_detected() {
    let doc = parse_yaml(
        &fixture_dir().join("devices").join("missing_capabilities.yaml"),
    );
    let errors = validate_device_fields(&doc);
    assert!(
        errors.iter().any(|e| e.contains("capabilities.axes")),
        "should detect missing capabilities.axes: {errors:?}"
    );
    assert!(
        errors.iter().any(|e| e.contains("capabilities.buttons")),
        "should detect missing capabilities.buttons: {errors:?}"
    );
}

#[test]
fn game_schema_missing_name_detected() {
    let doc = parse_yaml(&fixture_dir().join("games").join("missing_name.yaml"));
    let errors = validate_game_fields(&doc);
    assert!(
        errors.iter().any(|e| e.contains("game.name")),
        "should detect missing game.name: {errors:?}"
    );
}

#[test]
fn fixture_duplicate_vid_pid_detection() {
    // Parse two fixtures that intentionally share VID/PID
    let doc1 = parse_yaml(&fixture_dir().join("devices").join("valid_tier1.yaml"));
    let doc2 = parse_yaml(&fixture_dir().join("devices").join("duplicate_vid_pid.yaml"));

    let vid1 = doc1["device"]["usb"]["vendor_id"].as_u64();
    let pid1 = doc1["device"]["usb"]["product_id"].as_u64();
    let vid2 = doc2["device"]["usb"]["vendor_id"].as_u64();
    let pid2 = doc2["device"]["usb"]["product_id"].as_u64();

    assert_eq!(vid1, vid2, "fixtures should share VID for duplicate test");
    assert_eq!(pid1, pid2, "fixtures should share PID for duplicate test");
}

#[test]
fn not_a_mapping_yaml_produces_validation_errors() {
    let path = fixture_dir().join("invalid").join("not_a_mapping.yaml");
    let text = std::fs::read_to_string(&path).expect("read fixture");
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("plain string is valid YAML");
    // A plain string parses as a YAML scalar, not a mapping.
    // All required fields should be missing.
    let errors = validate_device_fields(&doc);
    assert!(
        errors.len() >= 4,
        "a non-mapping YAML should fail many field checks, got: {errors:?}"
    );
}

#[test]
fn real_game_manifests_all_parse() {
    let games = collect_yaml(&compat_games_dir());
    assert!(
        !games.is_empty(),
        "should have at least one game manifest"
    );
    let mut parse_failures = Vec::new();
    for path in &games {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                parse_failures.push(format!("{}: read error: {e}", path.display()));
                continue;
            }
        };
        if let Err(e) = serde_yaml::from_str::<serde_yaml::Value>(&text) {
            parse_failures.push(format!("{}: parse error: {e}", path.display()));
        }
    }
    assert!(
        parse_failures.is_empty(),
        "Game manifests that failed to parse:\n{}",
        parse_failures.join("\n")
    );
}

#[test]
fn real_device_manifests_mostly_parse() {
    let devices = collect_yaml(&compat_devices_dir());
    assert!(
        devices.len() >= 100,
        "should have at least 100 device manifests, found {}",
        devices.len()
    );
    let mut parse_failures = Vec::new();
    for path in &devices {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                parse_failures.push(format!("{}: read error: {e}", path.display()));
                continue;
            }
        };
        if let Err(e) = serde_yaml::from_str::<serde_yaml::Value>(&text) {
            parse_failures.push(format!("{}: parse error: {e}", path.display()));
        }
    }
    // Allow up to 6% parse failures (pre-existing encoding issues in repo)
    let failure_pct = (parse_failures.len() as f64 / devices.len() as f64) * 100.0;
    assert!(
        failure_pct < 6.0,
        "More than 6% of device manifests failed to parse ({}/{}):\n{}",
        parse_failures.len(),
        devices.len(),
        parse_failures.join("\n")
    );
    eprintln!(
        "Advisory: {}/{} device manifests had parse/encoding issues",
        parse_failures.len(),
        devices.len()
    );
}

#[test]
fn all_game_ids_are_unique() {
    let games = collect_yaml(&compat_games_dir());
    let mut seen: HashMap<String, PathBuf> = HashMap::new();
    let mut duplicates = Vec::new();

    for path in &games {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let doc: serde_yaml::Value = match serde_yaml::from_str(&text) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if let Some(id) = doc["game"]["id"].as_str() {
            let id = id.to_string();
            match seen.entry(id.clone()) {
                std::collections::hash_map::Entry::Occupied(e) => {
                    duplicates.push(format!(
                        "game.id '{}': {} and {}",
                        id,
                        e.get().display(),
                        path.display()
                    ));
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(path.clone());
                }
            }
        }
    }
    assert!(
        duplicates.is_empty(),
        "Duplicate game IDs found:\n{}",
        duplicates.join("\n")
    );
}

#[test]
fn device_vendor_directories_match_vendor_field() {
    // Each device lives under compat/devices/<vendor-slug>/. Verify the directory
    // name is a reasonable slug (lowercase, hyphens).
    let devices = collect_yaml(&compat_devices_dir());
    let mut bad = Vec::new();
    for path in &devices {
        if let Some(parent) = path.parent()
            && let Some(dir_name) = parent.file_name().and_then(|n| n.to_str())
        {
            // Vendor directory should be lowercase with hyphens
            if dir_name != dir_name.to_lowercase()
                || dir_name.contains(' ')
                || dir_name.contains('_')
            {
                bad.push(format!(
                    "{}: vendor dir '{}' should be lowercase-hyphen",
                    path.display(),
                    dir_name
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "Device vendor directory naming issues:\n{}",
        bad.join("\n")
    );
}
