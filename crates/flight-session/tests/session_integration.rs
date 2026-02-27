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

/// Like `fixture_config`, but the global/sim search paths point to an empty
/// directory so that only the aircraft-specific profile is found.
/// This prevents `global.json` from acting as a fallback for unknown ICAOs.
fn aircraft_only_fixture_config() -> AutoSwitchConfig {
    let profiles_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/profiles");
    let empty_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/empty_dir");
    AutoSwitchConfig {
        // global/sim paths point to an empty dir → no fallback profile
        profile_paths: vec![empty_dir.clone(), empty_dir, profiles_dir],
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

// ---------------------------------------------------------------------------
// force_switch
// ---------------------------------------------------------------------------

/// `force_switch` is equivalent to `on_aircraft_detected` for a manually
/// chosen ICAO.  The fixture directory contains `c172.json`, so the load
/// should succeed and increment `committed_switches`.
#[tokio::test]
async fn force_switch_c172_commits_switch_and_sets_aircraft() {
    let config = fixture_config();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    auto_switch
        .force_switch(AircraftId::new("C172"))
        .await
        .unwrap();

    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if auto_switch.get_metrics().await.committed_switches >= 1 {
            break;
        }
    }

    let metrics = auto_switch.get_metrics().await;
    assert_eq!(metrics.committed_switches, 1, "force_switch should commit");
    let current = auto_switch.get_current_aircraft().await;
    assert!(
        current.is_some(),
        "current aircraft should be set after force_switch"
    );
    assert_eq!(current.unwrap().aircraft_id.icao, "C172");
}

/// Forcing a switch to an ICAO for which no profile file exists must
/// increment `failed_switches` and leave `current_aircraft` unchanged
/// (still `None` when started fresh).
#[tokio::test]
async fn unknown_aircraft_force_switch_increments_failed_switches() {
    // Use the aircraft-only config so there is no global.json fallback.
    // "ZZZZ" has no aircraft-specific fixture → load fails → failed_switches++.
    let config = aircraft_only_fixture_config();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    auto_switch
        .force_switch(AircraftId::new("ZZZZ"))
        .await
        .unwrap();

    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if auto_switch.get_metrics().await.failed_switches >= 1 {
            break;
        }
    }

    let metrics = auto_switch.get_metrics().await;
    assert!(
        metrics.failed_switches >= 1,
        "a missing profile should increment failed_switches"
    );
    assert!(
        auto_switch.get_current_aircraft().await.is_none(),
        "current aircraft should remain None after a failed switch"
    );
}

/// Once a profile is loaded for C172, switching to C172 again with a
/// different sim ID still reuses the cached profile (same aircraft ICAO)
/// so `committed_switches` stays at 1 while `total_switches` reaches 2.
#[tokio::test]
async fn same_icao_different_sim_total_switches_twice_committed_once() {
    let config = fixture_config();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    // First detection: MSFS
    let c172_msfs = DetectedAircraft {
        sim: SimId::Msfs,
        aircraft_id: AircraftId::new("C172"),
        process_name: "FlightSimulator.exe".to_string(),
        detection_time: Instant::now(),
        confidence: 0.9,
    };
    auto_switch.on_aircraft_detected(c172_msfs).await.unwrap();

    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if auto_switch.get_metrics().await.committed_switches >= 1 {
            break;
        }
    }
    assert_eq!(auto_switch.get_metrics().await.committed_switches, 1);

    // Second detection: XPlane — same ICAO, different sim.
    // The same-aircraft guard only fires when *both* aircraft_id and sim match,
    // so this detection proceeds and increments total_switches.
    // committed_switches stays at 1 because the aircraft ICAO did not change.
    let c172_xplane = DetectedAircraft {
        sim: SimId::XPlane,
        aircraft_id: AircraftId::new("C172"),
        process_name: "X-Plane.exe".to_string(),
        detection_time: Instant::now(),
        confidence: 0.85,
    };
    auto_switch.on_aircraft_detected(c172_xplane).await.unwrap();

    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if auto_switch.get_metrics().await.total_switches >= 2 {
            break;
        }
    }

    let metrics = auto_switch.get_metrics().await;
    assert_eq!(
        metrics.total_switches, 2,
        "switching same ICAO on a new sim should increment total_switches"
    );
    assert_eq!(
        metrics.committed_switches, 1,
        "committed_switches should not increment when ICAO is unchanged"
    );
}

/// Switching from C172 to A320 (both with fixture profiles) should
/// increment `committed_switches` twice — once per distinct ICAO.
#[tokio::test]
async fn switching_two_different_aircraft_commits_twice() {
    let config = fixture_config();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    // First: C172
    let c172 = DetectedAircraft {
        sim: SimId::Msfs,
        aircraft_id: AircraftId::new("C172"),
        process_name: "FlightSimulator.exe".to_string(),
        detection_time: Instant::now(),
        confidence: 0.9,
    };
    auto_switch.on_aircraft_detected(c172).await.unwrap();
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if auto_switch.get_metrics().await.committed_switches >= 1 {
            break;
        }
    }
    assert_eq!(auto_switch.get_metrics().await.committed_switches, 1);

    // Second: A320 (different ICAO)
    let a320 = DetectedAircraft {
        sim: SimId::Msfs,
        aircraft_id: AircraftId::new("A320"),
        process_name: "FlightSimulator.exe".to_string(),
        detection_time: Instant::now(),
        confidence: 0.88,
    };
    auto_switch.on_aircraft_detected(a320).await.unwrap();
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if auto_switch.get_metrics().await.committed_switches >= 2 {
            break;
        }
    }

    let metrics = auto_switch.get_metrics().await;
    assert_eq!(
        metrics.committed_switches, 2,
        "switching to a different ICAO should commit a second time"
    );
    let current = auto_switch.get_current_aircraft().await;
    assert_eq!(current.unwrap().aircraft_id.icao, "A320");
}

/// After a failed switch to an unknown aircraft the previously-loaded
/// profile is retained.  We first switch to C172 (succeeds), then
/// attempt a switch to UNKN (fails), and the current aircraft remains C172.
#[tokio::test]
async fn failed_switch_preserves_previous_profile() {
    // Use a config where global.json is not on the search path, so that a
    // missing aircraft-specific file produces a genuine load error.
    let config = aircraft_only_fixture_config();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    // Establish C172 as current aircraft.
    let c172 = DetectedAircraft {
        sim: SimId::Msfs,
        aircraft_id: AircraftId::new("C172"),
        process_name: "FlightSimulator.exe".to_string(),
        detection_time: Instant::now(),
        confidence: 0.9,
    };
    auto_switch.on_aircraft_detected(c172).await.unwrap();
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if auto_switch.get_metrics().await.committed_switches >= 1 {
            break;
        }
    }
    assert_eq!(
        auto_switch
            .get_current_aircraft()
            .await
            .unwrap()
            .aircraft_id
            .icao,
        "C172"
    );

    // Now switch to an ICAO with no fixture profile — should fail.
    auto_switch
        .force_switch(AircraftId::new("UNKN"))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // The current aircraft should still be C172.
    let current = auto_switch.get_current_aircraft().await;
    assert!(current.is_some(), "current aircraft should still be set");
    assert_eq!(
        current.unwrap().aircraft_id.icao,
        "C172",
        "a failed switch must not clear the previously loaded profile"
    );
}

// ---------------------------------------------------------------------------
// PoF hysteresis (integration-level, via public on_telemetry_update)
// ---------------------------------------------------------------------------

/// A single outlier frame must not trigger a phase transition.
///
/// With `consecutive_frames_required = 2` (needing 3 actual calls to confirm),
/// sending one climb-like frame sandwiched between ground frames keeps the PoF
/// at Ground.
#[tokio::test]
async fn pof_hysteresis_single_outlier_frame_does_not_transition() {
    let mut config = fixture_config();
    config.pof_hysteresis.consecutive_frames_required = 2;
    config.pof_hysteresis.min_phase_time = Duration::from_millis(0);

    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    let ground = ground_snapshot();

    // Establish Ground with 5 consistent frames.
    for _ in 0..5 {
        auto_switch
            .on_telemetry_update(ground.clone())
            .await
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        auto_switch.get_current_pof().await,
        Some(PhaseOfFlight::Ground)
    );

    // One outlier climb frame — not enough to confirm transition.
    let climb = TelemetrySnapshot {
        ias_knots: 80.0,
        vertical_speed_fpm: 600.0,
        altitude_feet: 1500.0,
        ground_speed_knots: 90.0,
        gear_down: false,
        ..ground.clone()
    };
    auto_switch.on_telemetry_update(climb).await.unwrap();

    // Immediately return to ground conditions — resets the candidate.
    for _ in 0..3 {
        auto_switch
            .on_telemetry_update(ground.clone())
            .await
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(150)).await;

    assert_eq!(
        auto_switch.get_current_pof().await,
        Some(PhaseOfFlight::Ground),
        "single outlier climb frame must not trigger a phase transition"
    );
}

/// Consistent approach-like telemetry must eventually confirm the Approach phase.
#[tokio::test]
async fn pof_transitions_to_approach_after_consecutive_frames() {
    let mut config = fixture_config();
    config.pof_hysteresis.consecutive_frames_required = 1;
    config.pof_hysteresis.min_phase_time = Duration::from_millis(0);

    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    // Approach: IAS < 120, altitude < 2000, gear up, VS near zero.
    let approach = TelemetrySnapshot {
        sim: SimId::Msfs,
        aircraft: AircraftId::new("C172"),
        timestamp: 0,
        ias_knots: 90.0,
        ground_speed_knots: 88.0,
        altitude_feet: 1200.0,
        vertical_speed_fpm: -100.0,
        gear_down: false,
    };

    for _ in 0..3 {
        auto_switch
            .on_telemetry_update(approach.clone())
            .await
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(200)).await;

    let pof = auto_switch.get_current_pof().await;
    assert_eq!(
        pof,
        Some(PhaseOfFlight::Approach),
        "consistent approach telemetry should confirm Approach PoF"
    );
}

// ---------------------------------------------------------------------------
// Profile cascade — global.json + aircraft profile
// ---------------------------------------------------------------------------

/// When both `global.json` and `c172.json` exist in the fixture directory the
/// cascade loader merges them without error, and the switch is committed.
#[tokio::test]
async fn cascade_load_global_and_aircraft_profile_succeeds() {
    // fixture_config() points all three search paths to tests/fixtures/profiles/,
    // which now contains both global.json and c172.json.
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

    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if auto_switch.get_metrics().await.committed_switches >= 1 {
            break;
        }
    }

    let metrics = auto_switch.get_metrics().await;
    assert_eq!(metrics.failed_switches, 0, "cascade load must not fail");
    assert_eq!(
        metrics.committed_switches, 1,
        "cascade load should commit exactly one switch"
    );
}

// ---------------------------------------------------------------------------
// detected_aircraft confidence
// ---------------------------------------------------------------------------

/// The `confidence` value set on the `DetectedAircraft` event is stored
/// verbatim and readable via `get_current_aircraft()`.
#[tokio::test]
async fn detected_aircraft_confidence_preserved() {
    let config = fixture_config();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    let aircraft = DetectedAircraft {
        sim: SimId::Msfs,
        aircraft_id: AircraftId::new("C172"),
        process_name: "FlightSimulator.exe".to_string(),
        detection_time: Instant::now(),
        confidence: 0.987,
    };
    auto_switch.on_aircraft_detected(aircraft).await.unwrap();

    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if auto_switch.get_current_aircraft().await.is_some() {
            break;
        }
    }

    let current = auto_switch.get_current_aircraft().await.unwrap();
    assert!(
        (current.confidence - 0.987).abs() < 1e-4,
        "confidence should be stored unchanged"
    );
}

// ---------------------------------------------------------------------------
// ProfileSource — additional unit tests
// ---------------------------------------------------------------------------

#[test]
fn profile_source_merged_empty_vec_no_primary_path() {
    let src = ProfileSource::Merged(vec![]);
    // A Merged variant always reports has_file() = true regardless of contents.
    assert!(src.has_file());
    assert!(!src.is_in_memory());
    // But primary_path() and all_paths() correctly reflect the empty list.
    assert!(src.primary_path().is_none());
    assert!(src.all_paths().is_empty());
}

#[test]
fn profile_source_merged_single_path_all_paths_length_one() {
    let path = PathBuf::from("/etc/profiles/default.json");
    let src = ProfileSource::Merged(vec![path.clone()]);
    assert_eq!(src.all_paths().len(), 1);
    assert_eq!(src.primary_path(), Some(path.as_path()));
}

#[test]
fn profile_source_update_path_converts_in_memory_to_file() {
    let mut src = ProfileSource::InMemory;
    assert!(src.is_in_memory());

    src.update_path(PathBuf::from("/tmp/new_profile.json"));

    assert!(
        !src.is_in_memory(),
        "update_path should switch to File variant"
    );
    assert!(src.has_file());
    assert!(src.primary_path().is_some());
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
