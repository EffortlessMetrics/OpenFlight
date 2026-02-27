// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for `flight-session`.
//!
//! These tests use only the public API and verify end-to-end behaviour:
//! aircraft detection, profile loading from fixtures, cache invalidation,
//! serialization round-trips, and `ProfileSource` path operations.

use flight_session::{
    AircraftAutoSwitch, AircraftId, AutoSwitchConfig, DetectedAircraft, PhaseOfFlight,
    ProfileSource, SimId, TelemetrySnapshot,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return an `AutoSwitchConfig` whose profile paths all point at the fixtures
/// bundled with this crate (under `tests/fixtures/profiles/`).
fn fixture_config() -> AutoSwitchConfig {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/profiles");
    AutoSwitchConfig {
        profile_paths: vec![dir.clone(), dir.clone(), dir],
        ..AutoSwitchConfig::default()
    }
}

fn ground_snapshot() -> TelemetrySnapshot {
    TelemetrySnapshot {
        sim: SimId::Msfs,
        aircraft: AircraftId::new("C172"),
        timestamp: 0,
        ias_knots: 0.0,
        ground_speed_knots: 2.0,
        altitude_feet: 0.0,
        vertical_speed_fpm: 0.0,
        gear_down: true,
    }
}

// ---------------------------------------------------------------------------
// AircraftId – serialisation round-trip
// ---------------------------------------------------------------------------

#[test]
fn aircraft_id_serde_roundtrip() {
    let id = AircraftId::with_variant("A320", "NEO");
    let json = serde_json::to_string(&id).expect("serialise");
    let back: AircraftId = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(id, back);
}

#[test]
fn aircraft_id_no_variant_roundtrip() {
    let id = AircraftId::new("C172");
    let json = serde_json::to_string(&id).unwrap();
    let back: AircraftId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, back);
    assert_eq!(back.variant, None);
}

// ---------------------------------------------------------------------------
// TelemetrySnapshot – serialisation round-trip
// ---------------------------------------------------------------------------

#[test]
fn telemetry_snapshot_serde_roundtrip() {
    let snap = TelemetrySnapshot {
        sim: SimId::Msfs,
        aircraft: AircraftId::new("C172"),
        timestamp: 1_234_567_890,
        ias_knots: 120.5,
        ground_speed_knots: 118.0,
        altitude_feet: 2500.0,
        vertical_speed_fpm: -200.0,
        gear_down: false,
    };
    let json = serde_json::to_string(&snap).unwrap();
    let back: TelemetrySnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back.timestamp, snap.timestamp);
    assert!((back.ias_knots - snap.ias_knots).abs() < 0.01);
    assert_eq!(back.gear_down, snap.gear_down);
}

// ---------------------------------------------------------------------------
// PhaseOfFlight – display / parse round-trip
// ---------------------------------------------------------------------------

#[test]
fn phase_of_flight_display_parse_roundtrip() {
    use std::str::FromStr;

    let phases = [
        PhaseOfFlight::Ground,
        PhaseOfFlight::Taxi,
        PhaseOfFlight::Takeoff,
        PhaseOfFlight::Climb,
        PhaseOfFlight::Cruise,
        PhaseOfFlight::Descent,
        PhaseOfFlight::Approach,
        PhaseOfFlight::Landing,
        PhaseOfFlight::GoAround,
    ];
    for phase in phases {
        let s = phase.to_string();
        let parsed =
            PhaseOfFlight::from_str(&s).unwrap_or_else(|_| panic!("failed to parse '{}'", s));
        assert_eq!(phase, parsed, "round-trip failed for {:?}", phase);
    }
}

#[test]
fn phase_of_flight_unknown_string_is_error() {
    use std::str::FromStr;
    assert!("foobar".parse::<PhaseOfFlight>().is_err());
    assert!(PhaseOfFlight::from_str("").is_err());
}

// ---------------------------------------------------------------------------
// ProfileSource – path operations
// ---------------------------------------------------------------------------

#[test]
fn profile_source_in_memory() {
    let src = ProfileSource::InMemory;
    assert!(src.is_in_memory());
    assert!(!src.has_file());
    assert!(src.primary_path().is_none());
    assert!(src.all_paths().is_empty());
}

#[test]
fn profile_source_file_has_path() {
    let src = ProfileSource::File(PathBuf::from("/tmp/my_profile.json"));
    assert!(!src.is_in_memory());
    assert!(src.has_file());
    assert!(src.primary_path().is_some());
    assert_eq!(src.all_paths().len(), 1);
}

#[test]
fn profile_source_merged_returns_last_as_primary() {
    let paths = vec![
        PathBuf::from("/global.json"),
        PathBuf::from("/sim.json"),
        PathBuf::from("/aircraft.json"),
    ];
    let src = ProfileSource::Merged(paths.clone());
    assert!(!src.is_in_memory());
    assert!(src.has_file());
    assert_eq!(src.primary_path(), Some(paths[2].as_path()));
    assert_eq!(src.all_paths().len(), 3);
}

#[test]
fn profile_source_from_path_roundtrip() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/profiles/c172.json");
    let src = ProfileSource::from_path(&dir);
    assert!(src.has_file());
    // The primary_path should point to (a possibly canonicalised) c172.json.
    let pp = src.primary_path().expect("should have primary path");
    assert!(pp.file_name().unwrap().to_string_lossy().contains("c172"));
}

// ---------------------------------------------------------------------------
// AircraftAutoSwitch – fixture-based integration
// ---------------------------------------------------------------------------

/// The auto-switch system starts with no current aircraft.
#[tokio::test]
async fn auto_switch_starts_with_no_aircraft() {
    let config = AutoSwitchConfig::default();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    assert!(
        auto_switch.get_current_aircraft().await.is_none(),
        "no aircraft should be detected at start"
    );
    let metrics = auto_switch.get_metrics().await;
    assert_eq!(metrics.total_switches, 0);
    assert_eq!(metrics.successful_switches, 0);
}

/// Detecting an aircraft whose profile exists in the fixture directory loads
/// successfully and increments the metrics counters.
#[tokio::test]
async fn auto_switch_detects_aircraft_with_fixture_profile() {
    let config = fixture_config();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    let aircraft = DetectedAircraft {
        sim: SimId::Msfs,
        aircraft_id: AircraftId::new("C172"),
        process_name: "FlightSimulator.exe".to_string(),
        detection_time: Instant::now(),
        confidence: 0.95,
    };
    auto_switch.on_aircraft_detected(aircraft).await.unwrap();

    // Poll until the background task processes the event.
    let mut total = 0u64;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        total = auto_switch.get_metrics().await.total_switches;
        if total > 0 {
            break;
        }
    }
    assert!(total > 0, "at least one switch should have occurred");

    let current = auto_switch.get_current_aircraft().await;
    assert!(current.is_some());
    assert_eq!(current.unwrap().aircraft_id, AircraftId::new("C172"));
}

/// Multiple distinct aircraft detections result in individual metric increments.
#[tokio::test]
async fn multiple_aircraft_switches_update_metrics() {
    let config = fixture_config();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    // Switch to C172 first.
    let c172 = DetectedAircraft {
        sim: SimId::Msfs,
        aircraft_id: AircraftId::new("C172"),
        process_name: "FlightSimulator.exe".to_string(),
        detection_time: Instant::now(),
        confidence: 0.9,
    };
    auto_switch.on_aircraft_detected(c172).await.unwrap();

    // Poll until committed.
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if auto_switch.get_metrics().await.committed_switches >= 1 {
            break;
        }
    }
    let after_first = auto_switch.get_metrics().await;
    assert_eq!(
        after_first.committed_switches, 1,
        "first switch should be committed"
    );

    // Detecting the same aircraft again must NOT increment committed_switches.
    let c172_again = DetectedAircraft {
        sim: SimId::Msfs,
        aircraft_id: AircraftId::new("C172"),
        process_name: "FlightSimulator.exe".to_string(),
        detection_time: Instant::now(),
        confidence: 0.9,
    };
    auto_switch.on_aircraft_detected(c172_again).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after_same = auto_switch.get_metrics().await;
    assert_eq!(
        after_same.committed_switches, 1,
        "same aircraft should not increment committed_switches"
    );
}

/// Cache invalidation followed by re-detection: invalidating the cache for the
/// current aircraft does not break the system; the aircraft remains detected
/// because the same-aircraft guard prevents a redundant switch.
#[tokio::test]
async fn cache_invalidation_followed_by_redetect() {
    let config = fixture_config();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    // First detection: profile loaded and cached.
    let aircraft = DetectedAircraft {
        sim: SimId::Msfs,
        aircraft_id: AircraftId::new("C172"),
        process_name: "FlightSimulator.exe".to_string(),
        detection_time: Instant::now(),
        confidence: 0.9,
    };
    auto_switch
        .on_aircraft_detected(aircraft.clone())
        .await
        .unwrap();

    // Poll until committed.
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if auto_switch.get_metrics().await.committed_switches >= 1 {
            break;
        }
    }
    assert_eq!(
        auto_switch.get_metrics().await.committed_switches,
        1,
        "first detection should commit one switch"
    );

    // Invalidate the cache for this aircraft.
    auto_switch
        .invalidate_cache(Some(AircraftId::new("C172")))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Re-detect the same aircraft: the same-aircraft guard in
    // handle_aircraft_detection returns early, so total_switches stays at 1.
    auto_switch.on_aircraft_detected(aircraft).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let metrics = auto_switch.get_metrics().await;
    // committed_switches stays at 1 (same aircraft, no new commit).
    assert_eq!(
        metrics.committed_switches, 1,
        "re-detecting the same aircraft should not commit another switch"
    );

    // The system is still healthy: current aircraft is set.
    assert!(
        auto_switch.get_current_aircraft().await.is_some(),
        "current aircraft should still be set after cache invalidation"
    );
}

/// Sending telemetry that consistently describes ground phase sets current PoF.
///
/// The PoF tracker requires N+1 calls to confirm a transition with
/// `consecutive_frames_required = N` (first call queues the candidate,
/// subsequent calls accumulate the counter).  We send 3 frames to be safe.
#[tokio::test]
async fn telemetry_update_sets_phase_of_flight() {
    let mut config = fixture_config();
    // Require only 1 consecutive frame so the test is fast and deterministic.
    config.pof_hysteresis.consecutive_frames_required = 1;
    config.pof_hysteresis.min_phase_time = Duration::from_millis(0);

    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    let snap = ground_snapshot();
    // Send multiple frames: the first queues the candidate; subsequent ones
    // satisfy the consecutive-frames requirement.
    for _ in 0..3 {
        auto_switch.on_telemetry_update(snap.clone()).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(150)).await;

    let pof = auto_switch.get_current_pof().await;
    assert!(
        pof.is_some(),
        "phase of flight should be set after telemetry updates"
    );
    assert_eq!(pof, Some(PhaseOfFlight::Ground));
}

/// Global cache invalidation (None) succeeds and the system remains operational.
#[tokio::test]
async fn global_cache_invalidation_does_not_break_system() {
    let config = fixture_config();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    // Invalidate entire cache on a fresh system — should be a no-op.
    auto_switch.invalidate_cache(None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // System still works: we can detect an aircraft.
    let aircraft = DetectedAircraft {
        sim: SimId::Msfs,
        aircraft_id: AircraftId::new("C172"),
        process_name: "FlightSimulator.exe".to_string(),
        detection_time: Instant::now(),
        confidence: 0.9,
    };
    auto_switch.on_aircraft_detected(aircraft).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let metrics = auto_switch.get_metrics().await;
    assert!(
        metrics.total_switches > 0,
        "system still functional after global cache invalidation"
    );
}
