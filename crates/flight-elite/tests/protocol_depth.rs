// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for Elite Dangerous protocol types:
//! round-trip serialization, edge cases, flag combinations, and parsing robustness.

use flight_elite::protocol::{EliteFlags, FuelStatus, JournalEvent, StatusJson, parse_journal_line};

// ── EliteFlags ──────────────────────────────────────────────────────────────

#[test]
fn flags_all_bits_independent() {
    let all = [
        EliteFlags::DOCKED,
        EliteFlags::LANDED,
        EliteFlags::GEAR_DOWN,
        EliteFlags::SHIELDS_UP,
        EliteFlags::SUPERCRUISE,
        EliteFlags::FLIGHT_ASSIST_OFF,
        EliteFlags::HARDPOINTS_DEPLOYED,
        EliteFlags::IN_WING,
        EliteFlags::LIGHTS_ON,
        EliteFlags::CARGO_SCOOP,
        EliteFlags::SILENT_RUNNING,
        EliteFlags::SCOOPING_FUEL,
        EliteFlags::IN_SRV,
        EliteFlags::FSD_JUMP,
    ];
    for (i, &flag) in all.iter().enumerate() {
        let f = EliteFlags::from_bits_truncate(flag.bits());
        assert!(f.contains(flag), "flag at index {i} should self-contain");
        for (j, &other) in all.iter().enumerate() {
            if i != j {
                assert!(
                    !EliteFlags::from_bits_truncate(flag.bits()).contains(other),
                    "flag {i} should not contain flag {j}"
                );
            }
        }
    }
}

#[test]
fn flags_combined_bitmask() {
    let bits = EliteFlags::DOCKED.bits()
        | EliteFlags::GEAR_DOWN.bits()
        | EliteFlags::LIGHTS_ON.bits()
        | EliteFlags::SHIELDS_UP.bits();
    let f = EliteFlags::from_bits_truncate(bits);
    assert!(f.contains(EliteFlags::DOCKED));
    assert!(f.contains(EliteFlags::GEAR_DOWN));
    assert!(f.contains(EliteFlags::LIGHTS_ON));
    assert!(f.contains(EliteFlags::SHIELDS_UP));
    assert!(!f.contains(EliteFlags::SUPERCRUISE));
    assert!(!f.contains(EliteFlags::FSD_JUMP));
}

#[test]
fn flags_zero_contains_nothing() {
    let f = EliteFlags::from_bits_truncate(0);
    assert!(!f.contains(EliteFlags::DOCKED));
    assert!(!f.contains(EliteFlags::GEAR_DOWN));
    assert!(!f.contains(EliteFlags::FSD_JUMP));
}

#[test]
fn flags_max_u64_contains_all() {
    let f = EliteFlags::from_bits_truncate(u64::MAX);
    assert!(f.contains(EliteFlags::DOCKED));
    assert!(f.contains(EliteFlags::LANDED));
    assert!(f.contains(EliteFlags::FSD_JUMP));
    assert!(f.contains(EliteFlags::IN_SRV));
}

#[test]
fn flags_bits_roundtrip() {
    let original = EliteFlags::SUPERCRUISE.bits() | EliteFlags::SHIELDS_UP.bits();
    let reconstructed = EliteFlags::from_bits_truncate(original);
    assert_eq!(reconstructed.bits(), original);
}

// ── StatusJson serialization ────────────────────────────────────────────────

#[test]
fn status_json_roundtrip_all_fields() {
    let status = StatusJson {
        schema_version: Some(4),
        event: Some("Status".to_string()),
        flags: EliteFlags::GEAR_DOWN.bits() | EliteFlags::SHIELDS_UP.bits(),
        pips: Some([4, 4, 4]),
        fire_group: Some(2),
        gui_focus: Some(0),
        fuel: Some(FuelStatus {
            fuel_main: 32.0,
            fuel_reservoir: 0.63,
        }),
        cargo: Some(12.0),
        legal_state: Some("Clean".to_string()),
    };
    let json = serde_json::to_string(&status).unwrap();
    let parsed: StatusJson = serde_json::from_str(&json).unwrap();
    assert_eq!(status, parsed);
}

#[test]
fn status_json_roundtrip_minimal() {
    let status = StatusJson {
        flags: 0,
        ..Default::default()
    };
    let json = serde_json::to_string(&status).unwrap();
    let parsed: StatusJson = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.flags, 0);
    assert!(parsed.fuel.is_none());
    assert!(parsed.pips.is_none());
}

#[test]
fn status_json_deserializes_real_game_output() {
    // Realistic Status.json from Elite Dangerous
    let raw = r#"{
        "timestamp":"2025-06-15T18:32:00Z",
        "event":"Status",
        "Flags":16842765,
        "Flags2":0,
        "Pips":[4,8,0],
        "FireGroup":1,
        "GuiFocus":0,
        "Fuel":{"FuelMain":28.43,"FuelReservoir":0.57},
        "Cargo":0.0,
        "LegalState":"Clean",
        "Balance":1250000,
        "Destination":{"System":128029044,"Body":0,"Name":"Hutton Orbital"}
    }"#;
    let s: StatusJson = serde_json::from_str(raw).unwrap();
    assert_eq!(s.flags, 16_842_765);
    assert_eq!(s.pips, Some([4, 8, 0]));
    assert_eq!(s.fire_group, Some(1));
    assert_eq!(s.legal_state.as_deref(), Some("Clean"));
    let fuel = s.fuel.unwrap();
    assert!((fuel.fuel_main - 28.43).abs() < 0.01);
    assert!((fuel.fuel_reservoir - 0.57).abs() < 0.01);
}

#[test]
fn status_json_unknown_fields_ignored() {
    // Extra fields that don't exist in our struct should be silently ignored.
    let raw = r#"{
        "Flags": 4,
        "SomeNewField": "hello",
        "AnotherThing": [1,2,3]
    }"#;
    let s: StatusJson = serde_json::from_str(raw).unwrap();
    assert_eq!(s.flags, EliteFlags::GEAR_DOWN.bits());
}

#[test]
fn status_json_empty_object_uses_defaults() {
    let s: StatusJson = serde_json::from_str("{}").unwrap();
    assert_eq!(s.flags, 0);
    assert!(s.fuel.is_none());
    assert!(s.pips.is_none());
    assert!(s.event.is_none());
    assert!(s.schema_version.is_none());
}

#[test]
fn status_json_legal_state_wanted() {
    let raw = r#"{"Flags": 0, "LegalState": "Wanted"}"#;
    let s: StatusJson = serde_json::from_str(raw).unwrap();
    assert_eq!(s.legal_state.as_deref(), Some("Wanted"));
}

// ── FuelStatus ──────────────────────────────────────────────────────────────

#[test]
fn fuel_status_roundtrip() {
    let fuel = FuelStatus {
        fuel_main: 32.0,
        fuel_reservoir: 0.63,
    };
    let json = serde_json::to_string(&fuel).unwrap();
    let parsed: FuelStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(fuel, parsed);
}

#[test]
fn fuel_status_zero_values() {
    let fuel = FuelStatus {
        fuel_main: 0.0,
        fuel_reservoir: 0.0,
    };
    let json = serde_json::to_string(&fuel).unwrap();
    let parsed: FuelStatus = serde_json::from_str(&json).unwrap();
    assert!((parsed.fuel_main).abs() < f32::EPSILON);
    assert!((parsed.fuel_reservoir).abs() < f32::EPSILON);
}

#[test]
fn fuel_status_uses_pascal_case_rename() {
    let fuel = FuelStatus {
        fuel_main: 16.0,
        fuel_reservoir: 0.57,
    };
    let json = serde_json::to_string(&fuel).unwrap();
    assert!(json.contains("FuelMain"));
    assert!(json.contains("FuelReservoir"));
    assert!(!json.contains("fuel_main"));
}

// ── JournalEvent serialization round-trips ──────────────────────────────────

#[test]
fn journal_event_load_game_roundtrip() {
    let event = JournalEvent::LoadGame {
        ship: "Python".to_string(),
        commander: Some("CMDR Jameson".to_string()),
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: JournalEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

#[test]
fn journal_event_location_roundtrip() {
    let event = JournalEvent::Location {
        star_system: "Alpha Centauri".to_string(),
        star_pos: Some([3.03125, -0.09375, 3.15625]),
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: JournalEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

#[test]
fn journal_event_fsd_jump_roundtrip() {
    let event = JournalEvent::FsdJump {
        star_system: "Sagittarius A*".to_string(),
        star_pos: Some([25.21875, -20.90625, 25899.96875]),
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: JournalEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

#[test]
fn journal_event_docked_roundtrip() {
    let event = JournalEvent::Docked {
        station_name: "Jameson Memorial".to_string(),
        star_system: "Shinrarta Dezhra".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: JournalEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

#[test]
fn journal_event_undocked_roundtrip() {
    let event = JournalEvent::Undocked {
        station_name: "Orbis Station".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: JournalEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

#[test]
fn journal_event_touchdown_roundtrip() {
    let event = JournalEvent::Touchdown {
        latitude: Some(-12.345),
        longitude: Some(67.89),
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: JournalEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

#[test]
fn journal_event_liftoff_roundtrip() {
    let event = JournalEvent::Liftoff {
        latitude: Some(45.0),
        longitude: Some(-90.0),
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: JournalEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

#[test]
fn journal_event_refuel_all_roundtrip() {
    let event = JournalEvent::RefuelAll {
        amount: Some(24.5),
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: JournalEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

#[test]
fn journal_event_refuel_all_without_amount_roundtrip() {
    let event = JournalEvent::RefuelAll { amount: None };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: JournalEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
}

// ── parse_journal_line edge cases ───────────────────────────────────────────

#[test]
fn parse_empty_string_returns_none() {
    assert!(parse_journal_line("").is_none());
}

#[test]
fn parse_whitespace_only_returns_none() {
    assert!(parse_journal_line("   ").is_none());
    assert!(parse_journal_line("\t\n").is_none());
}

#[test]
fn parse_bare_braces_returns_none() {
    assert!(parse_journal_line("{}").is_none());
}

#[test]
fn parse_malformed_json_returns_none() {
    assert!(parse_journal_line("{not json at all}").is_none());
    assert!(parse_journal_line("{'single': 'quotes'}").is_none());
    assert!(parse_journal_line("{\"unterminated").is_none());
}

#[test]
fn parse_json_array_returns_none() {
    assert!(parse_journal_line("[1, 2, 3]").is_none());
}

#[test]
fn parse_json_scalar_returns_none() {
    assert!(parse_journal_line("true").is_none());
    assert!(parse_journal_line("42").is_none());
    assert!(parse_journal_line("null").is_none());
    assert!(parse_journal_line(r#""just a string""#).is_none());
}

#[test]
fn parse_event_with_extra_fields_succeeds() {
    // Real journal lines have many extra fields we don't model.
    let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"LoadGame","Ship":"SideWinder","Commander":"CMDR Test","ShipID":7,"GameMode":"Open","Credits":5000000,"Loan":0}"#;
    let event = parse_journal_line(line).expect("should parse despite extra fields");
    match event {
        JournalEvent::LoadGame { ship, commander } => {
            assert_eq!(ship, "SideWinder");
            assert_eq!(commander.as_deref(), Some("CMDR Test"));
        }
        other => panic!("expected LoadGame, got {other:?}"),
    }
}

#[test]
fn parse_docked_event() {
    let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Docked","StationName":"Coriolis","StationEconomy":"Industrial","StarSystem":"LHS 3447","MarketID":128929344}"#;
    match parse_journal_line(line) {
        Some(JournalEvent::Docked {
            station_name,
            star_system,
        }) => {
            assert_eq!(station_name, "Coriolis");
            assert_eq!(star_system, "LHS 3447");
        }
        other => panic!("expected Docked, got {other:?}"),
    }
}

#[test]
fn parse_undocked_event() {
    let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Undocked","StationName":"Coriolis"}"#;
    match parse_journal_line(line) {
        Some(JournalEvent::Undocked { station_name }) => {
            assert_eq!(station_name, "Coriolis");
        }
        other => panic!("expected Undocked, got {other:?}"),
    }
}

#[test]
fn parse_liftoff_with_coordinates() {
    let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Liftoff","Latitude":12.5,"Longitude":-45.3}"#;
    match parse_journal_line(line) {
        Some(JournalEvent::Liftoff {
            latitude,
            longitude,
        }) => {
            assert!((latitude.unwrap() - 12.5).abs() < 0.01);
            assert!((longitude.unwrap() - (-45.3)).abs() < 0.01);
        }
        other => panic!("expected Liftoff, got {other:?}"),
    }
}

#[test]
fn parse_liftoff_without_coordinates() {
    let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Liftoff"}"#;
    match parse_journal_line(line) {
        Some(JournalEvent::Liftoff {
            latitude,
            longitude,
        }) => {
            assert!(latitude.is_none());
            assert!(longitude.is_none());
        }
        other => panic!("expected Liftoff, got {other:?}"),
    }
}

#[test]
fn parse_location_without_star_pos() {
    let line =
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Location","StarSystem":"Sol"}"#;
    match parse_journal_line(line) {
        Some(JournalEvent::Location {
            star_system,
            star_pos,
        }) => {
            assert_eq!(star_system, "Sol");
            assert!(star_pos.is_none());
        }
        other => panic!("expected Location, got {other:?}"),
    }
}

#[test]
fn parse_unicode_system_name() {
    let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"FsdJump","StarSystem":"Cöelestia Ⅲ","StarPos":[0.0,0.0,0.0]}"#;
    match parse_journal_line(line) {
        Some(JournalEvent::FsdJump { star_system, .. }) => {
            assert_eq!(star_system, "Cöelestia Ⅲ");
        }
        other => panic!("expected FsdJump, got {other:?}"),
    }
}

// ── Untracked event types return None ───────────────────────────────────────

#[test]
fn untracked_events_return_none() {
    let events = [
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Music","MusicTrack":"Exploration"}"#,
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Scan","BodyName":"Earth"}"#,
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"StartJump","JumpType":"Hyperspace"}"#,
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"SupercruiseEntry","StarSystem":"Sol"}"#,
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"SupercruiseExit","StarSystem":"Sol"}"#,
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"ApproachBody","Body":"Moon"}"#,
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"LeaveBody","Body":"Moon"}"#,
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"CommitCrime","CrimeType":"assault"}"#,
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Died","KillerName":"NPC"}"#,
        r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Resurrect","Option":"rebuy"}"#,
    ];
    for (i, line) in events.iter().enumerate() {
        assert!(
            parse_journal_line(line).is_none(),
            "untracked event at index {i} should return None"
        );
    }
}

// ── StatusJson with flag combinations ───────────────────────────────────────

#[test]
fn status_json_preserves_combined_flags_through_serde() {
    let flags = EliteFlags::DOCKED.bits()
        | EliteFlags::GEAR_DOWN.bits()
        | EliteFlags::SHIELDS_UP.bits()
        | EliteFlags::LIGHTS_ON.bits();
    let status = StatusJson {
        flags,
        ..Default::default()
    };
    let json = serde_json::to_string(&status).unwrap();
    let parsed: StatusJson = serde_json::from_str(&json).unwrap();
    let f = EliteFlags::from_bits_truncate(parsed.flags);
    assert!(f.contains(EliteFlags::DOCKED));
    assert!(f.contains(EliteFlags::GEAR_DOWN));
    assert!(f.contains(EliteFlags::SHIELDS_UP));
    assert!(f.contains(EliteFlags::LIGHTS_ON));
    assert!(!f.contains(EliteFlags::SUPERCRUISE));
}

#[test]
fn status_json_with_pips_distributions() {
    // Pips always sum to 12 in-game (each pip = 2 half-pips).
    for pips in [[12, 0, 0], [0, 12, 0], [0, 0, 12], [4, 4, 4], [8, 2, 2]] {
        let status = StatusJson {
            pips: Some(pips),
            ..Default::default()
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: StatusJson = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pips, Some(pips));
    }
}

#[test]
fn status_json_gui_focus_values() {
    // GuiFocus: 0=NoFocus, 1=Panel, 2=Panel, 3=CommsPanel, etc.
    for focus in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
        let status = StatusJson {
            gui_focus: Some(focus),
            ..Default::default()
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: StatusJson = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.gui_focus, Some(focus));
    }
}
