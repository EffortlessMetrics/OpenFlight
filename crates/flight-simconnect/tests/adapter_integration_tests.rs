#![cfg(windows)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for the MSFS SimConnect adapter.
//!
//! These tests exercise aircraft detection, database lookups, the byte-buffer
//! parser in `process_aircraft_data`, and AircraftDetector callback behaviour —
//! all without requiring a live MSFS / SimConnect connection.
//!
//! Coverage areas:
//!  1. AircraftDatabase — default entries and custom-mapping round-trip
//!  2. AircraftDetector — primary ID from ATC model, title fallback, UNKNOWN
//!  3. AircraftDetector callbacks — fires on new aircraft, silent on repeat
//!  4. process_aircraft_data — valid payload, helicopter, too-short buffer
//!  5. AircraftInfo — optional fields, engine-type variants

use flight_simconnect::{
    AircraftDetector, AircraftInfo,
    aircraft::{AircraftCategory, AircraftDatabase, AircraftMapping, EngineType},
};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Build an `AircraftInfo` with default piston-airplane values, overridable
/// via the caller-supplied title, model and type strings.
fn make_info(title: &str, atc_model: &str, atc_type: &str) -> AircraftInfo {
    AircraftInfo {
        title: title.to_string(),
        atc_model: atc_model.to_string(),
        atc_type: atc_type.to_string(),
        atc_airline: None,
        atc_flight_number: None,
        category: AircraftCategory::Airplane,
        engine_type: EngineType::Piston,
        engine_count: 1,
    }
}

/// Build the fixed-length byte buffer expected by `process_aircraft_data`.
///
/// Layout (matches `src/aircraft.rs`):
/// - 256 bytes  TITLE (STRING256)
/// -  32 bytes  ATC MODEL (STRING32)
/// -  32 bytes  ATC TYPE  (STRING32)
/// -  64 bytes  ATC AIRLINE (STRING64)
/// -  32 bytes  ATC FLIGHT NUMBER (STRING32)
/// -  32 bytes  CATEGORY (STRING32)
/// -   4 bytes  ENGINE TYPE (INT32 LE)
/// -   4 bytes  NUMBER OF ENGINES (INT32 LE)
fn make_payload(
    title: &str,
    atc_model: &str,
    atc_type: &str,
    atc_airline: &str,
    atc_flight_number: &str,
    category: &str,
    engine_type: i32,
    engine_count: i32,
) -> Vec<u8> {
    let mut buf = vec![0u8; 256 + 32 + 32 + 64 + 32 + 32 + 4 + 4];
    let mut off = 0;

    write_str(&mut buf, off, title, 256);
    off += 256;
    write_str(&mut buf, off, atc_model, 32);
    off += 32;
    write_str(&mut buf, off, atc_type, 32);
    off += 32;
    write_str(&mut buf, off, atc_airline, 64);
    off += 64;
    write_str(&mut buf, off, atc_flight_number, 32);
    off += 32;
    write_str(&mut buf, off, category, 32);
    off += 32;
    buf[off..off + 4].copy_from_slice(&engine_type.to_le_bytes());
    off += 4;
    buf[off..off + 4].copy_from_slice(&engine_count.to_le_bytes());

    buf
}

fn write_str(buf: &mut [u8], offset: usize, s: &str, max_len: usize) {
    let bytes = s.as_bytes();
    let len = bytes.len().min(max_len - 1); // always leave null terminator
    buf[offset..offset + len].copy_from_slice(&bytes[..len]);
    // byte at offset + len remains 0 (null terminator)
}

// ===========================================================================
// 1. AircraftDatabase — default entries
// ===========================================================================

/// C172 is in the default database as a single-engine piston airplane.
#[test]
fn aircraft_database_c172_is_piston_airplane() {
    let db = AircraftDatabase::new();
    let e = db
        .get_mapping("C172")
        .expect("C172 must be in default database");
    assert_eq!(e.icao, "C172");
    assert_eq!(e.category, AircraftCategory::Airplane);
    assert_eq!(e.engine_type, EngineType::Piston);
}

/// B738 is in the default database as a twin-jet Boeing with "boeing" hint.
#[test]
fn aircraft_database_b738_is_jet_with_boeing_hint() {
    let db = AircraftDatabase::new();
    let e = db
        .get_mapping("B738")
        .expect("B738 must be in default database");
    assert_eq!(e.icao, "B738");
    assert_eq!(e.engine_type, EngineType::Jet);
    assert!(
        e.profile_hints.contains(&"boeing".to_string()),
        "B738 must carry the 'boeing' profile hint"
    );
}

/// A320 is in the default database as a fly-by-wire jet with "fbw" hint.
#[test]
fn aircraft_database_a320_is_jet_with_fbw_hint() {
    let db = AircraftDatabase::new();
    let e = db
        .get_mapping("A320")
        .expect("A320 must be in default database");
    assert_eq!(e.engine_type, EngineType::Jet);
    assert!(
        e.profile_hints.contains(&"fbw".to_string()),
        "A320 must carry the 'fbw' profile hint"
    );
    assert!(
        e.profile_hints.contains(&"airliner".to_string()),
        "A320 must carry the 'airliner' profile hint"
    );
}

/// R22 is in the default database as a piston-engine helicopter.
#[test]
fn aircraft_database_r22_is_piston_helicopter() {
    let db = AircraftDatabase::new();
    let e = db
        .get_mapping("R22")
        .expect("R22 must be in default database");
    assert_eq!(e.category, AircraftCategory::Helicopter);
    assert_eq!(e.engine_type, EngineType::Piston);
    assert!(
        e.profile_hints.contains(&"helicopter".to_string()),
        "R22 must carry the 'helicopter' profile hint"
    );
}

/// Looking up an unmapped ICAO returns None, not a panic.
#[test]
fn aircraft_database_unknown_icao_returns_none() {
    let db = AircraftDatabase::new();
    assert!(
        db.get_mapping("XZ9999").is_none(),
        "unmapped ICAO must return None"
    );
}

/// A custom mapping added via `add_mapping` is retrievable immediately.
#[test]
fn aircraft_database_custom_mapping_round_trip() {
    let mut db = AircraftDatabase::new();
    db.add_mapping(
        "TBM9".to_string(),
        AircraftMapping {
            icao: "TBM9".to_string(),
            name: "Daher TBM 930".to_string(),
            category: AircraftCategory::Airplane,
            engine_type: EngineType::Turboprop,
            profile_hints: vec!["ga".to_string(), "turboprop".to_string()],
        },
    );

    let e = db
        .get_mapping("TBM9")
        .expect("just-added mapping must be retrievable");
    assert_eq!(e.icao, "TBM9");
    assert_eq!(e.engine_type, EngineType::Turboprop);
    assert!(e.profile_hints.contains(&"turboprop".to_string()));
}

/// The default database has at least five aircraft entries.
#[test]
fn aircraft_database_default_has_at_least_five_entries() {
    let db = AircraftDatabase::new();
    assert!(
        db.available_aircraft().len() >= 5,
        "default database must have ≥ 5 aircraft, got {}",
        db.available_aircraft().len()
    );
}

// ===========================================================================
// 2. AircraftDetector — to_aircraft_id
// ===========================================================================

/// A freshly created detector holds no aircraft information.
#[test]
fn aircraft_detector_starts_with_no_current_aircraft() {
    let detector = AircraftDetector::new();
    assert!(
        detector.current_aircraft().is_none(),
        "new detector must have no current aircraft"
    );
}

/// When ATC model is non-empty it is used as-is as the ICAO identifier.
#[test]
fn aircraft_detector_uses_atc_model_as_primary_identifier() {
    let detector = AircraftDetector::new();
    let id = detector.to_aircraft_id(&make_info("Cessna 172 Skyhawk", "C172", "CESSNA"));
    assert_eq!(id.icao, "C172");
}

/// When ATC model is empty, the Cessna title pattern produces "C172".
#[test]
fn aircraft_detector_fallback_cessna_title_yields_c172() {
    let detector = AircraftDetector::new();
    let id = detector.to_aircraft_id(&make_info("Cessna 172 Skyhawk", "", ""));
    assert_eq!(
        id.icao, "C172",
        "Cessna title regex must extract 'C172' from 'Cessna 172 Skyhawk'"
    );
}

/// When ATC model is empty and the title matches no known pattern,
/// the identifier falls back to "UNKNOWN".
#[test]
fn aircraft_detector_returns_unknown_for_unrecognizable_title() {
    let detector = AircraftDetector::new();
    let id = detector.to_aircraft_id(&make_info("Unidentified Mystery Prototype", "", ""));
    assert_eq!(
        id.icao, "UNKNOWN",
        "unrecognizable title with empty ATC model must produce 'UNKNOWN'"
    );
}

// ===========================================================================
// 3. AircraftDetector — callbacks
// ===========================================================================

/// A detection callback fires exactly once when a new aircraft is first parsed.
#[test]
fn aircraft_detector_callback_fires_on_first_detection() {
    let mut detector = AircraftDetector::new();
    let count = Arc::new(Mutex::new(0u32));
    let count_clone = count.clone();
    detector.add_detection_callback(move |_| {
        *count_clone.lock().unwrap() += 1;
    });

    let data = make_payload("Cessna 172", "C172", "CESSNA", "", "", "Airplane", 0, 1);
    let result = detector.process_aircraft_data(&data).unwrap();
    assert!(
        result.is_some(),
        "first detection must return Some(AircraftInfo)"
    );
    assert_eq!(*count.lock().unwrap(), 1, "callback must fire exactly once");
}

/// When the same ATC model is received again, no new detection is reported
/// and no callback fires.
#[test]
fn aircraft_detector_callback_silent_on_same_aircraft() {
    let mut detector = AircraftDetector::new();
    let count = Arc::new(Mutex::new(0u32));
    let count_clone = count.clone();
    detector.add_detection_callback(move |_| {
        *count_clone.lock().unwrap() += 1;
    });

    let data = make_payload("Cessna 172", "C172", "CESSNA", "", "", "Airplane", 0, 1);
    let _ = detector.process_aircraft_data(&data); // first: fires callback
    let result = detector.process_aircraft_data(&data); // second: same model
    assert!(
        result.unwrap().is_none(),
        "repeated same model must return None"
    );
    assert_eq!(
        *count.lock().unwrap(),
        1,
        "callback must not fire again for the same aircraft model"
    );
}

// ===========================================================================
// 4. process_aircraft_data — byte-buffer parsing
// ===========================================================================

/// A fully-populated payload parses into the expected AircraftInfo fields.
#[test]
fn process_aircraft_data_parses_complete_payload() {
    let mut detector = AircraftDetector::new();
    let data = make_payload(
        "Airbus A320neo",
        "A320",
        "AIRBUS",
        "Lufthansa",
        "LH123",
        "Airplane",
        1, // Jet
        2, // 2 engines
    );

    let info = detector
        .process_aircraft_data(&data)
        .expect("valid payload must not error")
        .expect("first detection must return Some");

    assert_eq!(info.title, "Airbus A320neo");
    assert_eq!(info.atc_model, "A320");
    assert_eq!(info.atc_type, "AIRBUS");
    assert_eq!(info.atc_airline, Some("Lufthansa".to_string()));
    assert_eq!(info.atc_flight_number, Some("LH123".to_string()));
    assert_eq!(info.category, AircraftCategory::Airplane);
    assert_eq!(info.engine_type, EngineType::Jet);
    assert_eq!(info.engine_count, 2);
}

/// A helicopter payload sets the Helicopter category and Piston engine type.
#[test]
fn process_aircraft_data_parses_helicopter_category() {
    let mut detector = AircraftDetector::new();
    let data = make_payload(
        "Robinson R22",
        "R22",
        "ROBINSON",
        "",
        "",
        "Helicopter",
        0,
        1,
    );

    let info = detector
        .process_aircraft_data(&data)
        .expect("helicopter payload must not error")
        .expect("first detection must return Some");

    assert_eq!(info.category, AircraftCategory::Helicopter);
    assert_eq!(info.engine_type, EngineType::Piston);
    assert_eq!(info.engine_count, 1);
}

/// A buffer shorter than the minimum required size produces an error.
#[test]
fn process_aircraft_data_rejects_too_short_buffer() {
    let mut detector = AircraftDetector::new();
    let short = vec![0u8; 100]; // minimum is 256+32+32+64+32+32+4+4 = 456 bytes
    assert!(
        detector.process_aircraft_data(&short).is_err(),
        "buffer smaller than 456 bytes must return an error"
    );
}

// ===========================================================================
// 5. AircraftInfo — optional fields and engine-type variants
// ===========================================================================

/// `atc_airline` and `atc_flight_number` are `None` when the sim provides no value.
#[test]
fn aircraft_info_optional_fields_are_none_by_default() {
    let info = make_info("Cessna 172 Skyhawk", "C172", "CESSNA");
    assert!(
        info.atc_airline.is_none(),
        "airline must be None when not set"
    );
    assert!(
        info.atc_flight_number.is_none(),
        "flight number must be None when not set"
    );
}

/// `EngineType::Turboprop` is a distinct variant from `Piston` and `Jet`.
#[test]
fn aircraft_info_turboprop_engine_type_is_distinct() {
    assert_ne!(EngineType::Turboprop, EngineType::Piston);
    assert_ne!(EngineType::Turboprop, EngineType::Jet);
    assert_ne!(EngineType::Turboprop, EngineType::Electric);
}
