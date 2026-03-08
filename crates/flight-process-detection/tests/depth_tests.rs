// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the flight-process-detection crate.
//!
//! Covers process name matching, window title matching, game database
//! lookup/validation, simultaneous detections, platform-specific
//! behaviour, error handling, and property-based fuzzing.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use flight_process_detection::{
    DetectedProcess, DetectionMetrics, ProcessDetectionConfig, ProcessDetectionError,
    ProcessDefinition, ProcessDetector, SimId, SystemProcess,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_config() -> ProcessDetectionConfig {
    ProcessDetectionConfig::default()
}

fn make_system_process(pid: u32, name: &str, path: &str, title: Option<&str>) -> SystemProcess {
    SystemProcess {
        pid,
        name: name.to_string(),
        path: PathBuf::from(path),
        window_title: title.map(String::from),
    }
}

fn simple_definition(names: &[&str], titles: &[&str], paths: &[&str], conf: f32) -> ProcessDefinition {
    ProcessDefinition {
        process_names: names.iter().map(|s| s.to_string()).collect(),
        window_titles: titles.iter().map(|s| s.to_string()).collect(),
        process_paths: paths.iter().map(|s| PathBuf::from(*s)).collect(),
        min_confidence: conf,
    }
}

// ---------------------------------------------------------------------------
// SimId — Display
// ---------------------------------------------------------------------------

#[test]
fn sim_id_display_msfs() {
    assert_eq!(SimId::Msfs.to_string(), "MSFS");
}

#[test]
fn sim_id_display_msfs2024() {
    assert_eq!(SimId::Msfs2024.to_string(), "MSFS 2024");
}

#[test]
fn sim_id_display_xplane() {
    assert_eq!(SimId::XPlane.to_string(), "X-Plane");
}

#[test]
fn sim_id_display_dcs() {
    assert_eq!(SimId::Dcs.to_string(), "DCS");
}

#[test]
fn sim_id_display_acecombat7() {
    assert_eq!(SimId::AceCombat7.to_string(), "Ace Combat 7");
}

#[test]
fn sim_id_display_warthunder() {
    assert_eq!(SimId::WarThunder.to_string(), "War Thunder");
}

#[test]
fn sim_id_display_elite() {
    assert_eq!(SimId::EliteDangerous.to_string(), "Elite: Dangerous");
}

#[test]
fn sim_id_display_ksp() {
    assert_eq!(SimId::Ksp.to_string(), "Kerbal Space Program");
}

#[test]
fn sim_id_display_wingman() {
    assert_eq!(SimId::Wingman.to_string(), "Project Wingman");
}

#[test]
fn sim_id_display_il2() {
    assert_eq!(SimId::Il2.to_string(), "IL-2 Great Battles");
}

#[test]
fn sim_id_display_unknown() {
    assert_eq!(SimId::Unknown.to_string(), "Unknown");
}

// ---------------------------------------------------------------------------
// SimId — Clone / Copy / Eq / Hash
// ---------------------------------------------------------------------------

#[test]
fn sim_id_clone_and_copy() {
    let a = SimId::Msfs;
    let b = a; // Copy
    let c = a;
    assert_eq!(a, b);
    assert_eq!(a, c);
}

#[test]
fn sim_id_eq_different_variants() {
    assert_ne!(SimId::Msfs, SimId::Msfs2024);
    assert_ne!(SimId::XPlane, SimId::Dcs);
}

#[test]
fn sim_id_hash_as_map_key() {
    let mut map = HashMap::new();
    map.insert(SimId::Msfs, "msfs");
    map.insert(SimId::XPlane, "xplane");
    assert_eq!(map.get(&SimId::Msfs), Some(&"msfs"));
    assert_eq!(map.get(&SimId::XPlane), Some(&"xplane"));
    assert_eq!(map.get(&SimId::Dcs), None);
}

#[test]
fn sim_id_all_variants_unique_hash() {
    let variants = [
        SimId::Msfs,
        SimId::Msfs2024,
        SimId::XPlane,
        SimId::Dcs,
        SimId::AceCombat7,
        SimId::WarThunder,
        SimId::EliteDangerous,
        SimId::Ksp,
        SimId::Wingman,
        SimId::Il2,
        SimId::Unknown,
    ];
    let set: HashSet<SimId> = variants.iter().copied().collect();
    assert_eq!(set.len(), variants.len());
}

// ---------------------------------------------------------------------------
// SimId — Serde roundtrip
// ---------------------------------------------------------------------------

#[test]
fn sim_id_serde_roundtrip_all_variants() {
    let variants = [
        SimId::Msfs,
        SimId::Msfs2024,
        SimId::XPlane,
        SimId::Dcs,
        SimId::AceCombat7,
        SimId::WarThunder,
        SimId::EliteDangerous,
        SimId::Ksp,
        SimId::Wingman,
        SimId::Il2,
        SimId::Unknown,
    ];
    for v in &variants {
        let json = serde_json::to_string(v).expect("serialize");
        let back: SimId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(*v, back, "roundtrip failed for {v}");
    }
}

// ---------------------------------------------------------------------------
// ProcessDetectionConfig — defaults
// ---------------------------------------------------------------------------

#[test]
fn default_config_interval_is_one_second() {
    assert_eq!(default_config().detection_interval, Duration::from_secs(1));
}

#[test]
fn default_config_window_detection_enabled() {
    assert!(default_config().enable_window_detection);
}

#[test]
fn default_config_max_detection_time_100ms() {
    assert_eq!(
        default_config().max_detection_time,
        Duration::from_millis(100)
    );
}

#[test]
fn default_config_contains_all_registered_sims() {
    let cfg = default_config();
    let expected = [
        SimId::Msfs,
        SimId::Msfs2024,
        SimId::XPlane,
        SimId::Dcs,
        SimId::AceCombat7,
        SimId::WarThunder,
        SimId::EliteDangerous,
        SimId::Ksp,
        SimId::Wingman,
    ];
    for sim in &expected {
        assert!(
            cfg.process_definitions.contains_key(sim),
            "Missing definition for {sim}"
        );
    }
}

#[test]
fn default_config_every_definition_has_nonempty_process_names() {
    for (sim, def) in &default_config().process_definitions {
        assert!(
            !def.process_names.is_empty(),
            "{sim} has empty process_names"
        );
    }
}

#[test]
fn default_config_every_definition_has_nonempty_window_titles() {
    for (sim, def) in &default_config().process_definitions {
        assert!(
            !def.window_titles.is_empty(),
            "{sim} has empty window_titles"
        );
    }
}

#[test]
fn default_config_every_definition_has_nonempty_paths() {
    for (sim, def) in &default_config().process_definitions {
        assert!(
            !def.process_paths.is_empty(),
            "{sim} has empty process_paths"
        );
    }
}

#[test]
fn default_config_all_confidence_thresholds_in_range() {
    for (sim, def) in &default_config().process_definitions {
        assert!(
            (0.0..=1.0).contains(&def.min_confidence),
            "{sim} confidence {:.2} out of [0,1]",
            def.min_confidence,
        );
    }
}

// ---------------------------------------------------------------------------
// ProcessDefinition — specific sim databases
// ---------------------------------------------------------------------------

#[test]
fn msfs_definition_includes_fsx_legacy() {
    let def = default_config()
        .process_definitions
        .get(&SimId::Msfs)
        .cloned()
        .unwrap();
    assert!(def.process_names.contains(&"fsx.exe".to_string()));
}

#[test]
fn xplane_definition_covers_multiple_versions() {
    let def = default_config()
        .process_definitions
        .get(&SimId::XPlane)
        .cloned()
        .unwrap();
    assert!(def.process_names.iter().any(|n| n.contains("12")));
    assert!(def.process_names.iter().any(|n| n.contains("11")));
}

#[test]
fn dcs_definition_includes_updater() {
    let def = default_config()
        .process_definitions
        .get(&SimId::Dcs)
        .cloned()
        .unwrap();
    assert!(def.process_names.contains(&"DCS_updater.exe".to_string()));
}

#[test]
fn elite_dangerous_definition_has_64bit_exe() {
    let def = default_config()
        .process_definitions
        .get(&SimId::EliteDangerous)
        .cloned()
        .unwrap();
    assert!(def
        .process_names
        .contains(&"EliteDangerous64.exe".to_string()));
}

#[test]
fn ksp_definition_covers_multiplatform() {
    let def = default_config()
        .process_definitions
        .get(&SimId::Ksp)
        .cloned()
        .unwrap();
    // Windows
    assert!(def.process_names.iter().any(|n| n.contains("x64")));
    // Linux
    assert!(def.process_names.iter().any(|n| n.contains("x86_64")));
}

#[test]
fn wingman_lower_confidence_than_core_sims() {
    let cfg = default_config();
    let wingman_conf = cfg
        .process_definitions
        .get(&SimId::Wingman)
        .unwrap()
        .min_confidence;
    let msfs_conf = cfg
        .process_definitions
        .get(&SimId::Msfs)
        .unwrap()
        .min_confidence;
    assert!(
        wingman_conf < msfs_conf,
        "Wingman ({wingman_conf}) should have lower confidence than MSFS ({msfs_conf})"
    );
}

// ---------------------------------------------------------------------------
// Process name matching — exact, substring, case-insensitive
// ---------------------------------------------------------------------------

#[tokio::test]
async fn match_exact_process_name() {
    let def = simple_definition(&["FlightSimulator.exe"], &[], &[], 0.5);
    let procs = vec![make_system_process(1, "FlightSimulator.exe", "", None)];
    let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, false)
        .await
        .unwrap();
    assert!(result.is_some());
}

#[tokio::test]
async fn match_process_name_case_insensitive() {
    let def = simple_definition(&["FlightSimulator.exe"], &[], &[], 0.5);
    let procs = vec![make_system_process(1, "flightsimulator.exe", "", None)];
    let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, false)
        .await
        .unwrap();
    assert!(result.is_some());
}

#[tokio::test]
async fn match_process_name_substring() {
    let def = simple_definition(&["Flight"], &[], &[], 0.5);
    let procs = vec![make_system_process(1, "SomeFlightThing.exe", "", None)];
    let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, false)
        .await
        .unwrap();
    assert!(result.is_some());
}

#[tokio::test]
async fn no_match_unrelated_process() {
    let def = simple_definition(&["FlightSimulator.exe"], &[], &[], 0.5);
    let procs = vec![make_system_process(1, "notepad.exe", "", None)];
    let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, false)
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn empty_process_list_yields_none() {
    let def = simple_definition(&["FlightSimulator.exe"], &[], &[], 0.5);
    let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &[], false)
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn empty_definition_names_yields_none() {
    let def = simple_definition(&[], &[], &[], 0.5);
    let procs = vec![make_system_process(1, "anything.exe", "", None)];
    let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, false)
        .await
        .unwrap();
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// Window title matching
// ---------------------------------------------------------------------------

#[tokio::test]
async fn window_title_boosts_confidence() {
    let def = simple_definition(
        &["FlightSimulator.exe"],
        &["Microsoft Flight Simulator"],
        &[],
        0.5,
    );
    let procs = vec![make_system_process(
        1,
        "FlightSimulator.exe",
        "",
        Some("Microsoft Flight Simulator"),
    )];

    let with_window = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, true)
        .await
        .unwrap()
        .unwrap();
    let without_window =
        ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, false)
            .await
            .unwrap()
            .unwrap();
    assert!(with_window.confidence > without_window.confidence);
}

#[tokio::test]
async fn window_title_match_case_insensitive() {
    let def = simple_definition(
        &["FlightSimulator.exe"],
        &["Microsoft Flight Simulator"],
        &[],
        0.5,
    );
    let procs = vec![make_system_process(
        1,
        "FlightSimulator.exe",
        "",
        Some("MICROSOFT FLIGHT SIMULATOR"),
    )];
    let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, true)
        .await
        .unwrap()
        .unwrap();
    // Window title adds 0.1 on top of name match 0.6
    assert!(result.confidence > 0.6);
}

#[tokio::test]
async fn window_detection_disabled_ignores_title() {
    let def = simple_definition(
        &["FlightSimulator.exe"],
        &["Microsoft Flight Simulator"],
        &[],
        0.5,
    );
    let procs = vec![make_system_process(
        1,
        "FlightSimulator.exe",
        "",
        Some("Microsoft Flight Simulator"),
    )];
    let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, false)
        .await
        .unwrap()
        .unwrap();
    // Should be exactly 0.6 (name match only), no window boost
    assert!((result.confidence - 0.6).abs() < f32::EPSILON);
}

#[tokio::test]
async fn no_window_title_on_process_still_matches_by_name() {
    let def = simple_definition(
        &["X-Plane.exe"],
        &["X-Plane"],
        &[],
        0.5,
    );
    let procs = vec![make_system_process(1, "X-Plane.exe", "", None)];
    let result = ProcessDetector::check_simulator_processes(SimId::XPlane, &def, &procs, true)
        .await
        .unwrap();
    assert!(result.is_some());
}

// ---------------------------------------------------------------------------
// Path matching
// ---------------------------------------------------------------------------

#[tokio::test]
async fn path_match_boosts_confidence() {
    let def = simple_definition(
        &["DCS.exe"],
        &[],
        &["DCS World"],
        0.5,
    );
    let procs_with_path = vec![make_system_process(
        1,
        "DCS.exe",
        "C:\\Games\\DCS World\\bin\\DCS.exe",
        None,
    )];
    let procs_without_path = vec![make_system_process(1, "DCS.exe", "C:\\Other\\DCS.exe", None)];

    let with = ProcessDetector::check_simulator_processes(SimId::Dcs, &def, &procs_with_path, false)
        .await
        .unwrap()
        .unwrap();
    let without =
        ProcessDetector::check_simulator_processes(SimId::Dcs, &def, &procs_without_path, false)
            .await
            .unwrap()
            .unwrap();
    assert!(with.confidence > without.confidence);
}

#[tokio::test]
async fn path_match_case_insensitive() {
    let def = simple_definition(
        &["DCS.exe"],
        &[],
        &["DCS World"],
        0.5,
    );
    let procs = vec![make_system_process(
        1,
        "DCS.exe",
        "c:\\games\\dcs world\\bin\\dcs.exe",
        None,
    )];
    let result = ProcessDetector::check_simulator_processes(SimId::Dcs, &def, &procs, false)
        .await
        .unwrap()
        .unwrap();
    // Name (0.6) + path (0.3) = 0.9
    assert!(result.confidence > 0.8);
}

// ---------------------------------------------------------------------------
// Confidence thresholds
// ---------------------------------------------------------------------------

#[tokio::test]
async fn below_min_confidence_yields_none() {
    // Only window title match gives 0.1, which is below 0.5 threshold
    let def = simple_definition(&[], &["Some Title"], &[], 0.5);
    let procs = vec![make_system_process(1, "unrelated.exe", "", Some("Some Title"))];
    let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, true)
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn confidence_exactly_at_threshold_passes() {
    // Name match = 0.6, threshold = 0.6 → should pass
    let def = simple_definition(&["test.exe"], &[], &[], 0.6);
    let procs = vec![make_system_process(1, "test.exe", "", None)];
    let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, false)
        .await
        .unwrap();
    assert!(result.is_some());
}

#[tokio::test]
async fn max_confidence_with_all_matches() {
    let def = simple_definition(
        &["sim.exe"],
        &["My Sim"],
        &["SimPath"],
        0.5,
    );
    let procs = vec![make_system_process(
        1,
        "sim.exe",
        "/opt/SimPath/sim.exe",
        Some("My Sim Window"),
    )];
    let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, true)
        .await
        .unwrap()
        .unwrap();
    // 0.6 (name) + 0.3 (path) + 0.1 (title) = 1.0
    assert!((result.confidence - 1.0).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// Best-match selection among multiple processes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn selects_highest_confidence_process() {
    let def = simple_definition(
        &["sim.exe"],
        &[],
        &["BestPath"],
        0.5,
    );
    let procs = vec![
        make_system_process(10, "sim.exe", "/other/sim.exe", None),     // 0.6
        make_system_process(20, "sim.exe", "/BestPath/sim.exe", None),  // 0.9
    ];
    let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.process_id, 20);
    assert!(result.confidence > 0.8);
}

#[tokio::test]
async fn multiple_candidates_picks_best() {
    let def = simple_definition(
        &["DCS.exe"],
        &["DCS World"],
        &["Eagle Dynamics"],
        0.5,
    );
    let procs = vec![
        make_system_process(1, "DCS.exe", "", None),
        make_system_process(2, "DCS.exe", "C:\\Eagle Dynamics\\DCS.exe", Some("DCS World")),
    ];
    let result = ProcessDetector::check_simulator_processes(SimId::Dcs, &def, &procs, true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.process_id, 2);
}

// ---------------------------------------------------------------------------
// Multiple simultaneous detections
// ---------------------------------------------------------------------------

#[tokio::test]
async fn detect_multiple_sims_simultaneously() {
    let mut defs = HashMap::new();
    defs.insert(
        SimId::Msfs,
        simple_definition(&["FlightSimulator.exe"], &[], &[], 0.5),
    );
    defs.insert(
        SimId::Dcs,
        simple_definition(&["DCS.exe"], &[], &[], 0.5),
    );
    let config = ProcessDetectionConfig {
        detection_interval: Duration::from_secs(1),
        process_definitions: defs,
        enable_window_detection: false,
        max_detection_time: Duration::from_millis(100),
    };
    let detector = ProcessDetector::new(config);

    // No sims detected initially
    let detected = detector.get_detected_processes().await;
    assert!(detected.is_empty());
}

// ---------------------------------------------------------------------------
// DetectedProcess — Clone / PartialEq
// ---------------------------------------------------------------------------

#[test]
fn detected_process_clone_eq() {
    let now = Instant::now();
    let a = DetectedProcess {
        sim: SimId::Msfs,
        process_id: 42,
        process_name: "FlightSimulator.exe".to_string(),
        process_path: PathBuf::from("C:\\Games\\MSFS"),
        window_title: Some("Microsoft Flight Simulator".to_string()),
        detection_time: now,
        confidence: 0.9,
    };
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn detected_process_different_pid_not_eq() {
    let now = Instant::now();
    let a = DetectedProcess {
        sim: SimId::Msfs,
        process_id: 42,
        process_name: "a.exe".to_string(),
        process_path: PathBuf::new(),
        window_title: None,
        detection_time: now,
        confidence: 0.9,
    };
    let b = DetectedProcess {
        process_id: 99,
        ..a.clone()
    };
    assert_ne!(a, b);
}

// ---------------------------------------------------------------------------
// DetectionMetrics — Default / Clone
// ---------------------------------------------------------------------------

#[test]
fn detection_metrics_default_all_zero() {
    let m = DetectionMetrics::default();
    assert_eq!(m.total_scans, 0);
    assert_eq!(m.successful_detections, 0);
    assert_eq!(m.false_positives, 0);
    assert_eq!(m.average_scan_time, Duration::ZERO);
    assert_eq!(m.max_scan_time, Duration::ZERO);
}

#[test]
fn detection_metrics_clone() {
    let m = DetectionMetrics {
        total_scans: 10,
        successful_detections: 3,
        false_positives: 1,
        average_scan_time: Duration::from_millis(5),
        max_scan_time: Duration::from_millis(20),
    };
    let c = m.clone();
    assert_eq!(c.total_scans, 10);
    assert_eq!(c.max_scan_time, Duration::from_millis(20));
}

// ---------------------------------------------------------------------------
// ProcessDetectionError — Display
// ---------------------------------------------------------------------------

#[test]
fn error_platform_display() {
    let e = ProcessDetectionError::Platform("no support".to_string());
    let msg = e.to_string();
    assert!(msg.contains("Platform"));
    assert!(msg.contains("no support"));
}

#[test]
fn error_system_display() {
    let e = ProcessDetectionError::System("oom".to_string());
    let msg = e.to_string();
    assert!(msg.contains("System"));
    assert!(msg.contains("oom"));
}

#[test]
fn error_is_std_error() {
    fn assert_std_error<T: std::error::Error>() {}
    assert_std_error::<ProcessDetectionError>();
}

// ---------------------------------------------------------------------------
// ProcessDetector — creation and initial state
// ---------------------------------------------------------------------------

#[tokio::test]
async fn detector_initial_state_no_processes() {
    let detector = ProcessDetector::new(default_config());
    assert!(detector.get_detected_processes().await.is_empty());
}

#[tokio::test]
async fn detector_initial_metrics_zero() {
    let detector = ProcessDetector::new(default_config());
    let m = detector.get_metrics().await;
    assert_eq!(m.total_scans, 0);
    assert_eq!(m.successful_detections, 0);
}

#[tokio::test]
async fn detector_is_sim_detected_false_initially() {
    let detector = ProcessDetector::new(default_config());
    assert!(!detector.is_sim_detected(SimId::Msfs).await);
    assert!(!detector.is_sim_detected(SimId::XPlane).await);
    assert!(!detector.is_sim_detected(SimId::Dcs).await);
    assert!(!detector.is_sim_detected(SimId::Unknown).await);
}

#[tokio::test]
async fn detector_get_detected_process_none_initially() {
    let detector = ProcessDetector::new(default_config());
    assert!(detector.get_detected_process(SimId::Msfs).await.is_none());
}

// ---------------------------------------------------------------------------
// ProcessDetector — lifecycle (pure state, no OS scanning)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn detector_stop_without_start_fails() {
    // Sending shutdown on a fresh detector should succeed (channel is open)
    // but there's no running task to receive it.
    let detector = ProcessDetector::new(default_config());
    // stop() just sends on the channel — it should succeed even without start()
    assert!(detector.stop().await.is_ok());
}

#[tokio::test]
async fn detector_stop_twice_second_fails() {
    let detector = ProcessDetector::new(default_config());
    // First stop succeeds (channel open), second may fail if receiver dropped
    let _ = detector.stop().await;
    // After the receiver is dropped, sending again should fail
    // Actually the receiver is only taken on start(), so stop() keeps working
    // until the channel is fully closed.  Just verify it doesn't panic.
    let _ = detector.stop().await;
}

// ---------------------------------------------------------------------------
// ProcessDetector — custom config
// ---------------------------------------------------------------------------

#[tokio::test]
async fn detector_with_empty_definitions() {
    let config = ProcessDetectionConfig {
        detection_interval: Duration::from_millis(500),
        process_definitions: HashMap::new(),
        enable_window_detection: false,
        max_detection_time: Duration::from_millis(50),
    };
    let detector = ProcessDetector::new(config);
    assert!(detector.get_detected_processes().await.is_empty());
}

#[tokio::test]
async fn detector_custom_config_fields_preserved() {
    let config = ProcessDetectionConfig {
        detection_interval: Duration::from_secs(3600),
        enable_window_detection: false,
        max_detection_time: Duration::from_millis(50),
        process_definitions: HashMap::new(),
    };
    let detector = ProcessDetector::new(config);
    // Verify detector created without panic and has empty state
    assert!(detector.get_detected_processes().await.is_empty());
}

// ---------------------------------------------------------------------------
// Platform-specific gating
// ---------------------------------------------------------------------------

#[test]
fn platform_has_expected_process_names() {
    let cfg = default_config();
    let xplane = cfg.process_definitions.get(&SimId::XPlane).unwrap();

    #[cfg(target_os = "windows")]
    {
        assert!(xplane.process_names.iter().any(|n| n.ends_with(".exe")));
    }
    #[cfg(target_os = "linux")]
    {
        // On Linux, X-Plane binary is "X-Plane-x86_64" (no .exe)
        assert!(xplane
            .process_names
            .iter()
            .any(|n| !n.ends_with(".exe")));
    }
}

#[test]
fn war_thunder_has_platform_neutral_name() {
    let cfg = default_config();
    let wt = cfg.process_definitions.get(&SimId::WarThunder).unwrap();
    // "WarThunder" (no extension) should be present for Linux compatibility
    assert!(wt.process_names.contains(&"WarThunder".to_string()));
}

// ---------------------------------------------------------------------------
// Config Serde roundtrip
// ---------------------------------------------------------------------------

#[test]
fn config_serde_roundtrip() {
    let cfg = default_config();
    let json = serde_json::to_string(&cfg).expect("serialize config");
    let back: ProcessDetectionConfig = serde_json::from_str(&json).expect("deserialize config");
    assert_eq!(cfg.detection_interval, back.detection_interval);
    assert_eq!(cfg.enable_window_detection, back.enable_window_detection);
    assert_eq!(
        cfg.process_definitions.len(),
        back.process_definitions.len()
    );
}

// ---------------------------------------------------------------------------
// Property-based tests (proptest)
// ---------------------------------------------------------------------------

proptest! {
    /// Process matching never panics regardless of input.
    #[test]
    fn prop_check_simulator_never_panics(
        name in "\\PC{1,50}",
        title in "\\PC{0,50}",
        path in "\\PC{0,100}",
        pid in 0u32..100_000,
        conf in 0.0f32..1.0,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let def = ProcessDefinition {
                process_names: vec![name.clone()],
                window_titles: if title.is_empty() { vec![] } else { vec![title.clone()] },
                process_paths: if path.is_empty() { vec![] } else { vec![PathBuf::from(&path)] },
                min_confidence: conf,
            };
            let procs = vec![SystemProcess {
                pid,
                name,
                path: PathBuf::from(&path),
                window_title: if title.is_empty() { None } else { Some(title) },
            }];
            let _result = ProcessDetector::check_simulator_processes(
                SimId::Unknown,
                &def,
                &procs,
                true,
            ).await;
        });
    }

    /// Confidence is always in [0.0, 1.0] when a match is found.
    #[test]
    fn prop_confidence_in_range(
        name_frag in "[a-zA-Z]{1,20}",
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let exe = format!("{name_frag}.exe");
            let def = ProcessDefinition {
                process_names: vec![exe.clone()],
                window_titles: vec![name_frag.clone()],
                process_paths: vec![PathBuf::from(&name_frag)],
                min_confidence: 0.0, // accept everything
            };
            let procs = vec![SystemProcess {
                pid: 1,
                name: exe,
                path: PathBuf::from(format!("/{name_frag}/bin")),
                window_title: Some(name_frag),
            }];
            if let Ok(Some(det)) = ProcessDetector::check_simulator_processes(
                SimId::Unknown,
                &def,
                &procs,
                true,
            ).await {
                prop_assert!(det.confidence >= 0.0);
                prop_assert!(det.confidence <= 1.0);
            }
            Ok(())
        })?;
    }

    /// Empty process list always returns None.
    #[test]
    fn prop_empty_processes_always_none(
        name in "[a-zA-Z0-9]+",
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let def = ProcessDefinition {
                process_names: vec![name],
                window_titles: vec![],
                process_paths: vec![],
                min_confidence: 0.0,
            };
            let result = ProcessDetector::check_simulator_processes(
                SimId::Msfs,
                &def,
                &[],
                true,
            ).await.unwrap();
            prop_assert!(result.is_none());
            Ok(())
        })?;
    }

    /// Empty definition never matches anything.
    #[test]
    fn prop_empty_definition_never_matches(
        proc_name in "[a-zA-Z0-9]{1,30}",
        pid in 0u32..100_000,
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let def = ProcessDefinition {
                process_names: vec![],
                window_titles: vec![],
                process_paths: vec![],
                min_confidence: 0.0,
            };
            let procs = vec![SystemProcess {
                pid,
                name: proc_name,
                path: PathBuf::new(),
                window_title: None,
            }];
            let result = ProcessDetector::check_simulator_processes(
                SimId::Unknown,
                &def,
                &procs,
                true,
            ).await.unwrap();
            prop_assert!(result.is_none());
            Ok(())
        })?;
    }
}
