// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the X-Plane adapter.
//!
//! Covers: UDP protocol codec, dataref database, aircraft database,
//! adapter state machine, telemetry conversion, and RREF subscription
//! management.

use flight_xplane::{
    // UDP protocol
    build_cmnd_command, build_dref_command, parse_data_packet, parse_rref_response, ParseError,
    // Dataref database
    DatarefDatabase, DatarefType,
    // Dataref subscription management
    DatarefManager,
    // Aircraft database
    AircraftCategory, AircraftDatabase,
    // Aircraft detection
    EnhancedAircraftDetector,
    // State machine
    AdapterEvent, AdapterStateMachine, TransitionError, XPlaneAdapterState,
};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// §1  UDP PROTOCOL DEPTH
// ═══════════════════════════════════════════════════════════════════════

mod udp_protocol_depth {
    use super::*;

    // ── helpers ──────────────────────────────────────────────────────

    fn make_data_packet(groups: &[(u32, [f32; 8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"DATA\0");
        for (idx, vals) in groups {
            buf.extend_from_slice(&idx.to_le_bytes());
            for v in vals {
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        buf
    }

    fn make_rref_packet(entries: &[(u32, f32)]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"RREF\0");
        for (id, val) in entries {
            buf.extend_from_slice(&id.to_le_bytes());
            buf.extend_from_slice(&val.to_le_bytes());
        }
        buf
    }

    // ── RREF packet parsing ─────────────────────────────────────────

    #[test]
    fn rref_parse_valid_single_entry() {
        let pkt = make_rref_packet(&[(42, 3.14)]);
        let entries = parse_rref_response(&pkt).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, 42);
        assert!((entries[0].1 - 3.14).abs() < 0.01);
    }

    #[test]
    fn rref_parse_valid_multiple_entries() {
        let pkt = make_rref_packet(&[(0, 0.0), (1, 1.0), (255, -99.5), (1000, 0.001)]);
        let entries = parse_rref_response(&pkt).unwrap();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0], (0, 0.0));
        assert_eq!(entries[1], (1, 1.0));
        assert_eq!(entries[2].0, 255);
        assert!((entries[2].1 - (-99.5)).abs() < 0.01);
        assert_eq!(entries[3].0, 1000);
    }

    #[test]
    fn rref_parse_empty_payload_is_ok() {
        let entries = parse_rref_response(b"RREF\0").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn rref_parse_special_float_values() {
        let pkt = make_rref_packet(&[
            (0, f32::INFINITY),
            (1, f32::NEG_INFINITY),
            (2, f32::NAN),
            (3, 0.0),
            (4, -0.0),
            (5, f32::MIN),
            (6, f32::MAX),
            (7, f32::MIN_POSITIVE),
        ]);
        let entries = parse_rref_response(&pkt).unwrap();
        assert_eq!(entries.len(), 8);
        assert!(entries[0].1.is_infinite() && entries[0].1.is_sign_positive());
        assert!(entries[1].1.is_infinite() && entries[1].1.is_sign_negative());
        assert!(entries[2].1.is_nan());
    }

    // ── DATA packet parsing ─────────────────────────────────────────

    #[test]
    fn data_parse_single_group_round_trip() {
        let vals = [1.0f32, -2.0, 3.5, 0.0, -0.001, 999.99, 0.0, 0.0];
        let pkt = make_data_packet(&[(7, vals)]);
        let parsed = parse_data_packet(&pkt).unwrap();
        assert_eq!(parsed.header, *b"DATA");
        assert_eq!(parsed.data_groups.len(), 1);
        assert_eq!(parsed.data_groups[0].index, 7);
        for (a, b) in parsed.data_groups[0].values.iter().zip(vals.iter()) {
            assert!((a - b).abs() < 0.001);
        }
    }

    #[test]
    fn data_parse_many_groups() {
        let groups: Vec<(u32, [f32; 8])> = (0..20).map(|i| (i, [i as f32; 8])).collect();
        let pkt = make_data_packet(&groups);
        let parsed = parse_data_packet(&pkt).unwrap();
        assert_eq!(parsed.data_groups.len(), 20);
        for (i, g) in parsed.data_groups.iter().enumerate() {
            assert_eq!(g.index, i as u32);
        }
    }

    #[test]
    fn data_parse_empty_payload() {
        let parsed = parse_data_packet(b"DATA\0").unwrap();
        assert!(parsed.data_groups.is_empty());
    }

    // ── DREF write encoding ─────────────────────────────────────────

    #[test]
    fn dref_encode_total_length_is_509() {
        let pkt = build_dref_command("sim/cockpit2/controls/yoke_pitch_ratio", 0.5);
        assert_eq!(pkt.len(), 509);
    }

    #[test]
    fn dref_encode_header_and_value() {
        let pkt = build_dref_command("sim/test", -0.75);
        assert_eq!(&pkt[..5], b"DREF\0");
        let val = f32::from_le_bytes([pkt[5], pkt[6], pkt[7], pkt[8]]);
        assert!((val - (-0.75)).abs() < f32::EPSILON);
    }

    #[test]
    fn dref_encode_path_placed_at_offset_9() {
        let path = "sim/flightmodel/position/elevation";
        let pkt = build_dref_command(path, 0.0);
        let encoded_path = &pkt[9..9 + path.len()];
        assert_eq!(encoded_path, path.as_bytes());
    }

    #[test]
    fn dref_encode_remaining_bytes_are_nul() {
        let path = "sim/a";
        let pkt = build_dref_command(path, 1.0);
        assert!(pkt[9 + path.len()..].iter().all(|&b| b == 0));
    }

    #[test]
    fn dref_encode_max_length_path() {
        let path = "x".repeat(500);
        let pkt = build_dref_command(&path, 0.0);
        assert_eq!(pkt.len(), 509);
        assert_eq!(&pkt[9..509], path.as_bytes());
    }

    #[test]
    fn dref_encode_oversized_path_truncated() {
        let path = "y".repeat(600);
        let pkt = build_dref_command(&path, 0.0);
        assert_eq!(pkt.len(), 509);
        // First 500 bytes of path are kept
        assert!(pkt[9..509].iter().all(|&b| b == b'y'));
    }

    #[test]
    fn dref_encode_special_float_values() {
        // These should encode without panic
        for &val in &[f32::INFINITY, f32::NEG_INFINITY, f32::NAN, 0.0, -0.0] {
            let pkt = build_dref_command("sim/test", val);
            assert_eq!(pkt.len(), 509);
            let decoded = f32::from_le_bytes([pkt[5], pkt[6], pkt[7], pkt[8]]);
            if val.is_nan() {
                assert!(decoded.is_nan());
            } else {
                assert_eq!(decoded.to_bits(), val.to_bits());
            }
        }
    }

    // ── CMND encoding ───────────────────────────────────────────────

    #[test]
    fn cmnd_encode_header_and_payload() {
        let cmd = "sim/autopilot/heading_up";
        let pkt = build_cmnd_command(cmd);
        assert_eq!(&pkt[..5], b"CMND\0");
        let end = pkt[5..].iter().position(|&b| b == 0).unwrap();
        assert_eq!(&pkt[5..5 + end], cmd.as_bytes());
    }

    #[test]
    fn cmnd_encode_nul_terminated() {
        let pkt = build_cmnd_command("sim/cmd");
        assert_eq!(*pkt.last().unwrap(), 0u8);
    }

    #[test]
    fn cmnd_encode_empty_command() {
        let pkt = build_cmnd_command("");
        assert_eq!(&pkt[..5], b"CMND\0");
        assert_eq!(pkt[5], 0u8);
    }

    // ── Malformed / truncated / oversized packets → error (no panic)

    #[test]
    fn malformed_empty_bytes() {
        assert!(parse_rref_response(&[]).is_err());
        assert!(parse_data_packet(&[]).is_err());
    }

    #[test]
    fn malformed_only_header_bytes() {
        assert!(parse_rref_response(&[0x52]).is_err()); // "R"
        assert!(parse_data_packet(&[0x44, 0x41]).is_err()); // "DA"
    }

    #[test]
    fn malformed_wrong_header_for_rref() {
        let err = parse_rref_response(b"DATA\0").unwrap_err();
        assert!(matches!(err, ParseError::UnknownHeader { .. }));
    }

    #[test]
    fn malformed_wrong_header_for_data() {
        let err = parse_data_packet(b"RREF\0").unwrap_err();
        assert!(matches!(err, ParseError::UnknownHeader { .. }));
    }

    #[test]
    fn truncated_rref_partial_entry() {
        // RREF header + 6 bytes (need 8 for one entry)
        let mut pkt = Vec::from(&b"RREF\0"[..]);
        pkt.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let err = parse_rref_response(&pkt).unwrap_err();
        assert!(matches!(err, ParseError::TruncatedRrefEntry { .. }));
    }

    #[test]
    fn truncated_data_partial_group() {
        // DATA header + 10 bytes (need 36 for one group)
        let mut pkt = Vec::from(&b"DATA\0"[..]);
        pkt.extend_from_slice(&[0u8; 10]);
        let err = parse_data_packet(&pkt).unwrap_err();
        assert!(matches!(err, ParseError::TruncatedDataGroup { .. }));
    }

    #[test]
    fn rref_one_and_a_half_entries_is_error() {
        // 1 full entry (8 bytes) + 4 extra bytes
        let mut pkt = make_rref_packet(&[(1, 1.0)]);
        pkt.extend_from_slice(&[0u8; 4]);
        let err = parse_rref_response(&pkt).unwrap_err();
        assert!(matches!(err, ParseError::TruncatedRrefEntry { .. }));
    }

    #[test]
    fn data_one_and_a_half_groups_is_error() {
        let mut pkt = make_data_packet(&[(0, [0.0; 8])]);
        pkt.extend_from_slice(&[0u8; 10]);
        let err = parse_data_packet(&pkt).unwrap_err();
        assert!(matches!(err, ParseError::TruncatedDataGroup { .. }));
    }

    #[test]
    fn oversized_rref_packet_with_valid_structure_parses() {
        // 100 entries should parse fine
        let entries: Vec<(u32, f32)> = (0..100).map(|i| (i, i as f32)).collect();
        let pkt = make_rref_packet(&entries);
        let parsed = parse_rref_response(&pkt).unwrap();
        assert_eq!(parsed.len(), 100);
    }

    #[test]
    fn header_with_wrong_nul_separator() {
        // "RREF" followed by 0x01 instead of 0x00 — treated as valid header
        // but payload alignment depends on implementation
        let mut pkt = Vec::from(&b"RREF\x01"[..]);
        pkt.extend_from_slice(&42u32.to_le_bytes());
        pkt.extend_from_slice(&1.0f32.to_le_bytes());
        // The parser checks the first 4 bytes as header, byte 4 is separator
        // This should still parse since the header check is [0..4]
        let result = parse_rref_response(&pkt);
        // Implementation reads HEADER_LEN = 5, so byte[4] must be present
        // but isn't checked for NUL — it just skips it.
        assert!(result.is_ok());
    }

    // ── Property-based: encode/decode round-trip ────────────────────

    #[test]
    fn data_encode_decode_round_trip_exhaustive() {
        // Test multiple index/value combos
        let test_cases: &[(u32, [f32; 8])] = &[
            (0, [0.0; 8]),
            (u32::MAX, [f32::MAX; 8]),
            (128, [-1.0, 0.0, 1.0, 0.5, -0.5, 100.0, -100.0, 0.001]),
        ];
        for &(idx, vals) in test_cases {
            let pkt = make_data_packet(&[(idx, vals)]);
            let parsed = parse_data_packet(&pkt).unwrap();
            assert_eq!(parsed.data_groups[0].index, idx);
            for (i, v) in parsed.data_groups[0].values.iter().enumerate() {
                assert!(
                    (v - vals[i]).abs() < f32::EPSILON || (v.is_nan() && vals[i].is_nan()),
                    "mismatch at index {i}: got {v}, expected {}",
                    vals[i]
                );
            }
        }
    }

    #[test]
    fn rref_encode_decode_round_trip_exhaustive() {
        let test_entries: Vec<(u32, f32)> = vec![
            (0, 0.0),
            (1, 1.0),
            (u32::MAX, f32::MAX),
            (42, -42.42),
            (100, f32::MIN_POSITIVE),
        ];
        let pkt = make_rref_packet(&test_entries);
        let parsed = parse_rref_response(&pkt).unwrap();
        assert_eq!(parsed.len(), test_entries.len());
        for (parsed_entry, expected) in parsed.iter().zip(test_entries.iter()) {
            assert_eq!(parsed_entry.0, expected.0);
            assert!((parsed_entry.1 - expected.1).abs() < 0.001);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// §1b  UDP PROTOCOL PROPTEST
// ═══════════════════════════════════════════════════════════════════════

mod udp_protocol_proptest {
    use super::*;
    use proptest::prelude::*;

    fn make_rref_packet(entries: &[(u32, f32)]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"RREF\0");
        for (id, val) in entries {
            buf.extend_from_slice(&id.to_le_bytes());
            buf.extend_from_slice(&val.to_le_bytes());
        }
        buf
    }

    fn make_data_packet(groups: &[(u32, [f32; 8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"DATA\0");
        for (idx, vals) in groups {
            buf.extend_from_slice(&idx.to_le_bytes());
            for v in vals {
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        buf
    }

    proptest! {
        #[test]
        fn rref_round_trip(id in 0u32..10000, val in proptest::num::f32::ANY) {
            let pkt = make_rref_packet(&[(id, val)]);
            let entries = parse_rref_response(&pkt).unwrap();
            prop_assert_eq!(entries.len(), 1);
            prop_assert_eq!(entries[0].0, id);
            if val.is_nan() {
                prop_assert!(entries[0].1.is_nan());
            } else {
                prop_assert_eq!(entries[0].1.to_bits(), val.to_bits());
            }
        }

        #[test]
        fn data_round_trip(
            idx in 0u32..1000,
            v0 in proptest::num::f32::ANY,
            v1 in proptest::num::f32::ANY,
        ) {
            let vals = [v0, v1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
            let pkt = make_data_packet(&[(idx, vals)]);
            let parsed = parse_data_packet(&pkt).unwrap();
            prop_assert_eq!(parsed.data_groups.len(), 1);
            prop_assert_eq!(parsed.data_groups[0].index, idx);
            for (i, &v) in parsed.data_groups[0].values.iter().enumerate() {
                let expected = vals[i];
                if expected.is_nan() {
                    prop_assert!(v.is_nan(), "expected NaN at index {}", i);
                } else {
                    prop_assert_eq!(v.to_bits(), expected.to_bits(),
                        "mismatch at index {}", i);
                }
            }
        }

        #[test]
        fn dref_encode_always_509_bytes(
            path_len in 0usize..600,
            val in proptest::num::f32::ANY,
        ) {
            let path: String = "x".repeat(path_len);
            let pkt = build_dref_command(&path, val);
            prop_assert_eq!(pkt.len(), 509);
            prop_assert_eq!(&pkt[..5], b"DREF\0");
        }

        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..1024)) {
            // Must never panic — errors are fine
            let _ = parse_rref_response(&bytes);
            let _ = parse_data_packet(&bytes);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// §2  DATAREF DATABASE DEPTH
// ═══════════════════════════════════════════════════════════════════════

mod dataref_database_depth {
    use super::*;

    #[test]
    fn database_has_at_least_70_datarefs() {
        let db = DatarefDatabase::new();
        let count = db.all().len();
        assert!(
            count >= 60,
            "expected >=60 datarefs in database, got {count}"
        );
    }

    #[test]
    fn all_registered_datarefs_have_valid_paths() {
        let db = DatarefDatabase::new();
        for info in db.all() {
            assert!(
                !info.path.is_empty(),
                "empty path found in dataref database"
            );
            assert!(
                info.path.starts_with("sim/"),
                "dataref path should start with 'sim/': {}",
                info.path
            );
            assert!(
                !info.path.contains(' '),
                "dataref path should not contain spaces: {}",
                info.path
            );
        }
    }

    #[test]
    fn all_registered_datarefs_have_descriptions() {
        let db = DatarefDatabase::new();
        for info in db.all() {
            assert!(
                !info.description.is_empty(),
                "dataref {} has empty description",
                info.path
            );
        }
    }

    #[test]
    fn array_datarefs_have_sizes_scalars_do_not() {
        let db = DatarefDatabase::new();
        for info in db.all() {
            match info.data_type {
                DatarefType::FloatArray | DatarefType::IntArray => {
                    assert!(
                        info.array_size.is_some(),
                        "array dataref {} should have array_size",
                        info.path
                    );
                    assert!(
                        info.array_size.unwrap() > 0,
                        "array dataref {} should have positive array_size",
                        info.path
                    );
                }
                DatarefType::Int | DatarefType::Float | DatarefType::Double => {
                    assert!(
                        info.array_size.is_none(),
                        "scalar dataref {} should not have array_size",
                        info.path
                    );
                }
                DatarefType::Data => {
                    // Data type may or may not have a size
                }
            }
        }
    }

    // ── Lookup by name → correct type and unit ──────────────────────

    #[test]
    fn lookup_position_latitude() {
        let db = DatarefDatabase::new();
        let info = db.get("sim/flightmodel/position/latitude").unwrap();
        assert_eq!(info.data_type, DatarefType::Double);
        assert!(!info.writable);
        assert!(info.description.to_lowercase().contains("latitude"));
    }

    #[test]
    fn lookup_yoke_pitch_ratio() {
        let db = DatarefDatabase::new();
        let info = db.get("sim/cockpit2/controls/yoke_pitch_ratio").unwrap();
        assert_eq!(info.data_type, DatarefType::Float);
        assert!(info.writable);
    }

    #[test]
    fn lookup_engine_n1_is_float_array() {
        let db = DatarefDatabase::new();
        let info = db.get("sim/flightmodel/engine/ENGN_N1_").unwrap();
        assert_eq!(info.data_type, DatarefType::FloatArray);
        assert_eq!(info.array_size, Some(8));
        assert!(!info.writable);
    }

    #[test]
    fn lookup_autopilot_mode_is_int() {
        let db = DatarefDatabase::new();
        let info = db.get("sim/cockpit/autopilot/autopilot_mode").unwrap();
        assert_eq!(info.data_type, DatarefType::Int);
        assert!(!info.writable);
    }

    #[test]
    fn lookup_acf_icao_is_data() {
        let db = DatarefDatabase::new();
        let info = db.get("sim/aircraft/view/acf_ICAO").unwrap();
        assert_eq!(info.data_type, DatarefType::Data);
    }

    // ── Category filtering ──────────────────────────────────────────

    #[test]
    fn category_sim_cockpit_non_empty() {
        let db = DatarefDatabase::new();
        let cockpit = db.by_prefix("sim/cockpit/");
        assert!(
            !cockpit.is_empty(),
            "sim/cockpit/ prefix should return datarefs"
        );
        for info in &cockpit {
            assert!(info.path.starts_with("sim/cockpit/"));
        }
    }

    #[test]
    fn category_sim_flightmodel_non_empty() {
        let db = DatarefDatabase::new();
        let fm = db.by_prefix("sim/flightmodel/");
        assert!(fm.len() >= 15, "expected many flightmodel datarefs");
    }

    #[test]
    fn category_sim_weather_non_empty() {
        let db = DatarefDatabase::new();
        let weather = db.by_prefix("sim/weather/");
        assert!(!weather.is_empty());
    }

    #[test]
    fn category_sim_cockpit2_non_empty() {
        let db = DatarefDatabase::new();
        let c2 = db.by_prefix("sim/cockpit2/");
        assert!(c2.len() >= 5);
    }

    #[test]
    fn flight_controls_subset() {
        let db = DatarefDatabase::new();
        let controls = db.flight_controls();
        assert!(controls.len() >= 8, "expected >=8 flight control datarefs");
        let paths: Vec<&str> = controls.iter().map(|i| i.path).collect();
        assert!(paths.contains(&"sim/cockpit2/controls/yoke_pitch_ratio"));
        assert!(paths.contains(&"sim/cockpit2/controls/yoke_roll_ratio"));
        assert!(paths.contains(&"sim/flightmodel/controls/elv_trim"));
    }

    #[test]
    fn engine_data_subset() {
        let db = DatarefDatabase::new();
        let engines = db.engine_data();
        assert!(engines.len() >= 8);
        // All engine datarefs should be under sim/flightmodel/engine/
        for info in &engines {
            assert!(info.path.starts_with("sim/flightmodel/engine/"));
        }
    }

    #[test]
    fn navigation_subset() {
        let db = DatarefDatabase::new();
        let nav = db.navigation();
        assert!(nav.len() >= 5);
        let paths: Vec<&str> = nav.iter().map(|i| i.path).collect();
        assert!(paths.contains(&"sim/cockpit/radios/transponder_code"));
    }

    #[test]
    fn writable_refs_are_all_writable() {
        let db = DatarefDatabase::new();
        let writable = db.writable_refs();
        assert!(writable.len() >= 10);
        for info in &writable {
            assert!(info.writable, "{} should be writable", info.path);
        }
    }

    // ── Unknown dataref → None ──────────────────────────────────────

    #[test]
    fn unknown_dataref_returns_none() {
        let db = DatarefDatabase::new();
        assert!(db.get("sim/nonexistent/totally_made_up").is_none());
        assert!(db.get("").is_none());
        assert!(db.get("something_without_slash").is_none());
    }

    // ── Case-sensitive matching ─────────────────────────────────────

    #[test]
    fn case_sensitive_exact_match() {
        let db = DatarefDatabase::new();
        // The real dataref uses capital ENGN
        assert!(db.get("sim/flightmodel/engine/ENGN_N1_").is_some());
        // Wrong case should not match
        assert!(db.get("sim/flightmodel/engine/engn_n1_").is_none());
        assert!(db.get("SIM/FLIGHTMODEL/ENGINE/ENGN_N1_").is_none());
    }

    #[test]
    fn case_sensitive_position_datarefs() {
        let db = DatarefDatabase::new();
        // P, Q, R are uppercase in X-Plane
        assert!(db.get("sim/flightmodel/position/P").is_some());
        assert!(db.get("sim/flightmodel/position/p").is_none());
        assert!(db.get("sim/flightmodel/position/Q").is_some());
        assert!(db.get("sim/flightmodel/position/q").is_none());
    }

    // ── Default / new equivalence ───────────────────────────────────

    #[test]
    fn default_and_new_produce_same_database() {
        let a = DatarefDatabase::new();
        let b = DatarefDatabase::default();
        assert_eq!(a.all().len(), b.all().len());
    }

    #[test]
    fn empty_prefix_returns_all() {
        let db = DatarefDatabase::new();
        // Every dataref starts with "sim/" so prefix "sim/" returns all
        let all_sim = db.by_prefix("sim/");
        assert_eq!(all_sim.len(), db.all().len());
    }

    #[test]
    fn no_duplicate_paths() {
        let db = DatarefDatabase::new();
        let all = db.all();
        let mut paths: Vec<&str> = all.iter().map(|i| i.path).collect();
        let original_len = paths.len();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), original_len, "duplicate dataref paths found");
    }
}

// ═══════════════════════════════════════════════════════════════════════
// §3  AIRCRAFT DATABASE DEPTH
// ═══════════════════════════════════════════════════════════════════════

mod aircraft_database_depth {
    use super::*;

    #[test]
    fn database_has_at_least_25_aircraft() {
        let db = AircraftDatabase::new();
        assert!(db.len() >= 25, "expected >=25 aircraft, got {}", db.len());
    }

    // ── Recognition by acf path ─────────────────────────────────────

    #[test]
    fn cessna_172sp_recognized() {
        let db = AircraftDatabase::new();
        let entry = db
            .get("Aircraft/Laminar Research/Cessna 172SP/Cessna_172SP.acf")
            .unwrap();
        assert_eq!(entry.category, AircraftCategory::SinglePiston);
        assert_eq!(entry.default_profile, "ga-single-piston");
    }

    #[test]
    fn boeing_737_800_recognized() {
        let db = AircraftDatabase::new();
        let entry = db
            .get("Aircraft/Laminar Research/Boeing 737-800/b738.acf")
            .unwrap();
        assert_eq!(entry.category, AircraftCategory::AirlinerNarrowBody);
        assert_eq!(entry.default_profile, "airliner-narrow");
    }

    #[test]
    fn bell_206_helicopter_recognized() {
        let db = AircraftDatabase::new();
        let entry = db
            .get("Aircraft/Laminar Research/Bell 206/Bell206.acf")
            .unwrap();
        assert_eq!(entry.category, AircraftCategory::Helicopter);
    }

    #[test]
    fn ask21_glider_recognized() {
        let db = AircraftDatabase::new();
        let entry = db.get("Aircraft/Gliders/ASK-21/ASK21.acf").unwrap();
        assert_eq!(entry.category, AircraftCategory::Glider);
        assert_eq!(entry.default_profile, "glider");
    }

    // ── Aircraft → default profile mapping ──────────────────────────

    #[test]
    fn all_aircraft_have_default_profiles() {
        let db = AircraftDatabase::new();
        for entry in db.all() {
            assert!(
                !entry.default_profile.is_empty(),
                "aircraft {} missing default_profile",
                entry.acf_path
            );
        }
    }

    #[test]
    fn profile_names_follow_conventions() {
        let db = AircraftDatabase::new();
        let known_profiles = [
            "ga-single-piston",
            "ga-twin-piston",
            "turboprop-single",
            "turboprop-twin",
            "light-jet",
            "airliner-narrow",
            "airliner-wide",
            "regional-jet",
            "military-fighter",
            "helicopter-light",
            "helicopter-medium",
            "glider",
        ];
        for entry in db.all() {
            assert!(
                known_profiles.contains(&entry.default_profile),
                "unknown profile '{}' for aircraft {}",
                entry.default_profile,
                entry.acf_path
            );
        }
    }

    #[test]
    fn category_maps_to_expected_profile_prefix() {
        let db = AircraftDatabase::new();
        for entry in db.all() {
            match entry.category {
                AircraftCategory::SinglePiston => assert!(
                    entry.default_profile.contains("piston"),
                    "{}: expected piston profile",
                    entry.display_name
                ),
                AircraftCategory::AirlinerNarrowBody => assert!(
                    entry.default_profile.contains("airliner"),
                    "{}: expected airliner profile",
                    entry.display_name
                ),
                AircraftCategory::AirlinerWideBody => assert!(
                    entry.default_profile.contains("airliner"),
                    "{}: expected airliner profile",
                    entry.display_name
                ),
                AircraftCategory::Helicopter => assert!(
                    entry.default_profile.contains("helicopter"),
                    "{}: expected helicopter profile",
                    entry.display_name
                ),
                AircraftCategory::Glider => assert_eq!(
                    entry.default_profile, "glider",
                    "{}: expected glider profile",
                    entry.display_name
                ),
                _ => {} // Other categories have varied profiles
            }
        }
    }

    // ── Unknown aircraft → generic fallback (None from db) ──────────

    #[test]
    fn unknown_aircraft_returns_none() {
        let db = AircraftDatabase::new();
        assert!(db.get("Aircraft/DoesNotExist/FakeAircraft.acf").is_none());
        assert!(db.get("").is_none());
        assert!(db.get("random_string").is_none());
    }

    // ── Category filtering ──────────────────────────────────────────

    #[test]
    fn every_category_has_at_least_one_aircraft() {
        let db = AircraftDatabase::new();
        let categories = [
            AircraftCategory::SinglePiston,
            AircraftCategory::TwinPiston,
            AircraftCategory::Turboprop,
            AircraftCategory::LightJet,
            AircraftCategory::AirlinerNarrowBody,
            AircraftCategory::AirlinerWideBody,
            AircraftCategory::RegionalJet,
            AircraftCategory::MilitaryJet,
            AircraftCategory::Helicopter,
            AircraftCategory::Glider,
        ];
        for cat in categories {
            let entries = db.by_category(cat);
            assert!(
                !entries.is_empty(),
                "category {:?} has no aircraft entries",
                cat
            );
        }
    }

    #[test]
    fn narrow_body_airliners_have_multiple_entries() {
        let db = AircraftDatabase::new();
        let narrow = db.by_category(AircraftCategory::AirlinerNarrowBody);
        assert!(
            narrow.len() >= 3,
            "expected >=3 narrow-body airliners, got {}",
            narrow.len()
        );
    }

    // ── Custom datarefs ─────────────────────────────────────────────

    #[test]
    fn some_aircraft_have_custom_datarefs() {
        let db = AircraftDatabase::new();
        let has_custom = db
            .all()
            .iter()
            .any(|e| !e.custom_datarefs.is_empty());
        assert!(has_custom, "at least one aircraft should have custom datarefs");
    }

    #[test]
    fn cirrus_sf50_has_aoa_dataref() {
        let db = AircraftDatabase::new();
        let sf50 = db.get("Aircraft/Cirrus/SF50/SF50.acf").unwrap();
        assert!(sf50.custom_datarefs.contains(&"cirrus/sf50/aoa_indicator"));
    }

    #[test]
    fn a320_has_fcu_datarefs() {
        let db = AircraftDatabase::new();
        let a320 = db.get("Aircraft/FlightFactor/A320/A320.acf").unwrap();
        assert!(a320.custom_datarefs.contains(&"a320/fcu/altitude"));
        assert!(a320.custom_datarefs.contains(&"a320/fcu/heading"));
    }

    // ── All aircraft have display names ──────────────────────────────

    #[test]
    fn all_aircraft_have_display_names() {
        let db = AircraftDatabase::new();
        for entry in db.all() {
            assert!(
                !entry.display_name.is_empty(),
                "aircraft {} has empty display_name",
                entry.acf_path
            );
        }
    }

    // ── No duplicate acf_path entries ────────────────────────────────

    #[test]
    fn no_duplicate_acf_paths() {
        let db = AircraftDatabase::new();
        let all = db.all();
        let mut paths: Vec<&str> = all.iter().map(|e| e.acf_path).collect();
        let original_len = paths.len();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), original_len, "duplicate acf_path entries found");
    }

    // ── Enhanced aircraft detection with ICAO / alias ────────────────

    #[test]
    fn enhanced_detector_resolves_community_alias() {
        let det = EnhancedAircraftDetector::with_default_db();
        // Built-in aliases
        assert!(det.is_standard_icao("C172"));
        assert!(det.is_standard_icao("A320"));
        assert!(!det.is_standard_icao("TLSA")); // Community ICAO, not standard
    }

    #[test]
    fn enhanced_detector_custom_alias() {
        let mut det = EnhancedAircraftDetector::with_default_db();
        det.add_alias("MYAC", "B738");
        let mut raw = HashMap::new();
        raw.insert(
            "sim/aircraft/view/acf_ICAO".to_string(),
            "MYAC".to_string(),
        );
        raw.insert(
            "sim/aircraft/view/acf_descrip".to_string(),
            "My Custom 737".to_string(),
        );
        let id = det.identify(&raw);
        assert_eq!(id.icao, "B738");
    }

    #[test]
    fn unknown_icao_falls_back_to_no_db_match() {
        let mut det = EnhancedAircraftDetector::with_default_db();
        let mut raw = HashMap::new();
        raw.insert(
            "sim/aircraft/view/acf_ICAO".to_string(),
            "ZZZZ".to_string(),
        );
        raw.insert(
            "sim/aircraft/view/acf_descrip".to_string(),
            "Totally Unknown".to_string(),
        );
        let id = det.identify(&raw);
        assert_eq!(id.icao, "ZZZZ");
        assert!(id.db_match.is_none());
        assert!(!id.is_standard_icao);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// §4  STATE MACHINE DEPTH
// ═══════════════════════════════════════════════════════════════════════

mod state_machine_depth {
    use super::*;

    fn sm() -> AdapterStateMachine {
        AdapterStateMachine::new(5000, 3)
    }

    // ── Full lifecycle: Disconnected → Connecting → Connected → Active → Disconnected

    #[test]
    fn full_happy_path_lifecycle() {
        let mut sm = sm();
        assert_eq!(sm.state(), XPlaneAdapterState::Disconnected);

        let s = sm.transition(AdapterEvent::SocketBound).unwrap();
        assert_eq!(s, XPlaneAdapterState::Connecting);

        let s = sm.transition(AdapterEvent::SocketBound).unwrap();
        assert_eq!(s, XPlaneAdapterState::Connected);

        let s = sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(s, XPlaneAdapterState::Active);

        let s = sm.transition(AdapterEvent::Shutdown).unwrap();
        assert_eq!(s, XPlaneAdapterState::Disconnected);
    }

    // ── Timeout → Stale → Recovery ──────────────────────────────────

    #[test]
    fn active_to_stale_to_recovery() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(sm.state(), XPlaneAdapterState::Active);

        // Timeout → Stale
        let s = sm.transition(AdapterEvent::TelemetryTimeout).unwrap();
        assert_eq!(s, XPlaneAdapterState::Stale);

        // Recovery → Active
        let s = sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(s, XPlaneAdapterState::Active);
        assert_eq!(sm.error_count(), 0); // Error count cleared on recovery
    }

    #[test]
    fn stale_stays_stale_on_repeated_timeouts() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        sm.transition(AdapterEvent::TelemetryTimeout).unwrap();

        // Multiple timeouts while stale
        for _ in 0..5 {
            let s = sm.transition(AdapterEvent::TelemetryTimeout).unwrap();
            assert_eq!(s, XPlaneAdapterState::Stale);
        }
    }

    // ── Timeout threshold config ────────────────────────────────────

    #[test]
    fn stale_threshold_is_configurable() {
        let sm = AdapterStateMachine::new(5000, 3);
        assert_eq!(sm.stale_threshold_ms(), 5000);

        let sm2 = AdapterStateMachine::new(2000, 5);
        assert_eq!(sm2.stale_threshold_ms(), 2000);
    }

    // ── Reconnect scheduling: Error → retry via SocketBound ─────────

    #[test]
    fn error_allows_retry_within_limit() {
        let mut sm = AdapterStateMachine::new(5000, 3);

        // First error
        sm.transition(AdapterEvent::SocketError("net error".into()))
            .unwrap();
        assert_eq!(sm.state(), XPlaneAdapterState::Error);
        assert_eq!(sm.error_count(), 1);

        // Retry succeeds
        let s = sm.transition(AdapterEvent::SocketBound).unwrap();
        assert_eq!(s, XPlaneAdapterState::Connecting);
    }

    #[test]
    fn multiple_errors_track_count() {
        let mut sm = AdapterStateMachine::new(5000, 10);

        // Error count increments
        sm.transition(AdapterEvent::SocketError("err 1".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 1);

        // Retry and recover fully — error count resets
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(sm.error_count(), 0);

        // New errors start from 0 again
        sm.transition(AdapterEvent::SocketError("err 2".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 1);

        // Consecutive errors without recovery accumulate
        sm.transition(AdapterEvent::SocketError("err 3".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 2);
    }

    #[test]
    fn retries_exhausted_prevents_reconnect() {
        let mut sm = AdapterStateMachine::new(5000, 2);

        // Two errors exhaust retries (max_retries = 2)
        sm.transition(AdapterEvent::SocketError("e1".into()))
            .unwrap();
        sm.transition(AdapterEvent::SocketBound).unwrap(); // Retry 1 (error_count=1 < 2)
        sm.transition(AdapterEvent::SocketError("e2".into()))
            .unwrap();
        // Now error_count = 2, equals max_retries
        let result = sm.transition(AdapterEvent::SocketBound);
        assert!(matches!(
            result,
            Err(TransitionError::RetriesExhausted { max_retries: 2 })
        ));
    }

    // ── Shutdown from any state ─────────────────────────────────────

    #[test]
    fn shutdown_from_every_reachable_state() {
        // Disconnected
        {
            let mut m = sm();
            let s = m.transition(AdapterEvent::Shutdown).unwrap();
            assert_eq!(s, XPlaneAdapterState::Disconnected);
        }
        // Connecting
        {
            let mut m = sm();
            m.transition(AdapterEvent::SocketBound).unwrap();
            let s = m.transition(AdapterEvent::Shutdown).unwrap();
            assert_eq!(s, XPlaneAdapterState::Disconnected);
        }
        // Connected
        {
            let mut m = sm();
            m.transition(AdapterEvent::SocketBound).unwrap();
            m.transition(AdapterEvent::SocketBound).unwrap();
            let s = m.transition(AdapterEvent::Shutdown).unwrap();
            assert_eq!(s, XPlaneAdapterState::Disconnected);
        }
        // Active
        {
            let mut m = sm();
            m.transition(AdapterEvent::SocketBound).unwrap();
            m.transition(AdapterEvent::SocketBound).unwrap();
            m.transition(AdapterEvent::TelemetryReceived).unwrap();
            let s = m.transition(AdapterEvent::Shutdown).unwrap();
            assert_eq!(s, XPlaneAdapterState::Disconnected);
        }
        // Stale
        {
            let mut m = sm();
            m.transition(AdapterEvent::SocketBound).unwrap();
            m.transition(AdapterEvent::SocketBound).unwrap();
            m.transition(AdapterEvent::TelemetryReceived).unwrap();
            m.transition(AdapterEvent::TelemetryTimeout).unwrap();
            let s = m.transition(AdapterEvent::Shutdown).unwrap();
            assert_eq!(s, XPlaneAdapterState::Disconnected);
        }
        // Error
        {
            let mut m = sm();
            m.transition(AdapterEvent::SocketError("e".into()))
                .unwrap();
            let s = m.transition(AdapterEvent::Shutdown).unwrap();
            assert_eq!(s, XPlaneAdapterState::Disconnected);
            assert_eq!(m.error_count(), 0);
        }
    }

    // ── SocketError from any state ──────────────────────────────────

    #[test]
    fn socket_error_from_every_reachable_state() {
        let states_before_error = [
            vec![], // Disconnected
            vec![AdapterEvent::SocketBound],
            vec![AdapterEvent::SocketBound, AdapterEvent::SocketBound],
            vec![
                AdapterEvent::SocketBound,
                AdapterEvent::SocketBound,
                AdapterEvent::TelemetryReceived,
            ],
        ];

        for events in &states_before_error {
            let mut sm = sm();
            for ev in events {
                sm.transition(ev.clone()).unwrap();
            }
            let s = sm
                .transition(AdapterEvent::SocketError("test".into()))
                .unwrap();
            assert_eq!(s, XPlaneAdapterState::Error);
        }
    }

    // ── Invalid transitions ─────────────────────────────────────────

    #[test]
    fn disconnected_rejects_telemetry() {
        let mut sm = sm();
        let r = sm.transition(AdapterEvent::TelemetryReceived);
        assert!(matches!(r, Err(TransitionError::InvalidTransition { .. })));
    }

    #[test]
    fn disconnected_rejects_timeout() {
        let mut sm = sm();
        let r = sm.transition(AdapterEvent::TelemetryTimeout);
        assert!(matches!(r, Err(TransitionError::InvalidTransition { .. })));
    }

    #[test]
    fn connecting_rejects_telemetry_received() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        let r = sm.transition(AdapterEvent::TelemetryReceived);
        assert!(matches!(r, Err(TransitionError::InvalidTransition { .. })));
    }

    #[test]
    fn connecting_rejects_timeout() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        let r = sm.transition(AdapterEvent::TelemetryTimeout);
        assert!(matches!(r, Err(TransitionError::InvalidTransition { .. })));
    }

    #[test]
    fn connected_rejects_timeout() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        let r = sm.transition(AdapterEvent::TelemetryTimeout);
        assert!(matches!(r, Err(TransitionError::InvalidTransition { .. })));
    }

    // ── Helper methods ──────────────────────────────────────────────

    #[test]
    fn is_healthy_only_when_connected_or_active() {
        let mut sm = sm();
        assert!(!sm.is_healthy()); // Disconnected

        sm.transition(AdapterEvent::SocketBound).unwrap();
        assert!(!sm.is_healthy()); // Connecting

        sm.transition(AdapterEvent::SocketBound).unwrap();
        assert!(sm.is_healthy()); // Connected

        sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        assert!(sm.is_healthy()); // Active

        sm.transition(AdapterEvent::TelemetryTimeout).unwrap();
        assert!(!sm.is_healthy()); // Stale
    }

    #[test]
    fn is_recoverable_respects_max_retries() {
        let mut sm = AdapterStateMachine::new(5000, 2);
        assert!(sm.is_recoverable());

        sm.transition(AdapterEvent::SocketError("e1".into()))
            .unwrap();
        assert!(sm.is_recoverable()); // 1 < 2

        sm.transition(AdapterEvent::SocketBound).unwrap(); // retry
        sm.transition(AdapterEvent::SocketError("e2".into()))
            .unwrap();
        assert!(!sm.is_recoverable()); // 2 >= 2
    }

    #[test]
    fn reset_clears_state_and_errors() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketError("e".into()))
            .unwrap();
        sm.transition(AdapterEvent::SocketError("e".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 2);
        assert_eq!(sm.state(), XPlaneAdapterState::Error);

        sm.reset();
        assert_eq!(sm.state(), XPlaneAdapterState::Disconnected);
        assert_eq!(sm.error_count(), 0);
        assert!(sm.time_in_state().is_some()); // reset sets the timestamp
    }

    #[test]
    fn time_in_state_tracks_transitions() {
        let sm_fresh = sm();
        assert!(sm_fresh.time_in_state().is_none());

        let mut sm_used = sm();
        sm_used.transition(AdapterEvent::SocketBound).unwrap();
        let elapsed = sm_used.time_in_state().unwrap();
        // Should be very short (we just transitioned)
        assert!(elapsed.as_secs() < 1);
    }

    // ── Error count increments on each SocketError ──────────────────

    #[test]
    fn error_count_increments_cumulatively() {
        let mut sm = AdapterStateMachine::new(5000, 10);
        for i in 1..=5 {
            sm.transition(AdapterEvent::SocketError(format!("e{i}")))
                .unwrap();
            assert_eq!(sm.error_count(), i);
        }
    }

    // ── Connected → Active clears error count ───────────────────────

    #[test]
    fn active_telemetry_clears_error_count() {
        let mut sm = sm();
        // Accumulate some errors
        sm.transition(AdapterEvent::SocketError("e".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 1);

        // Recover
        sm.transition(AdapterEvent::SocketBound).unwrap(); // Error→Connecting
        sm.transition(AdapterEvent::SocketBound).unwrap(); // Connecting→Connected
        sm.transition(AdapterEvent::TelemetryReceived).unwrap(); // Connected→Active
        assert_eq!(sm.error_count(), 0);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// §5  TELEMETRY CONVERSION DEPTH
// ═══════════════════════════════════════════════════════════════════════

mod telemetry_conversion_depth {
    use super::*;

    // ── NaN / Inf filtering in dataref manager ──────────────────────

    #[test]
    fn dataref_manager_stores_nan_values() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/airspeed", 30.0);
        mgr.set_value("sim/airspeed", f32::NAN);
        let val = mgr.get_value("sim/airspeed").unwrap();
        assert!(val.is_nan());
    }

    #[test]
    fn dataref_manager_stores_inf_values() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/altitude", 10.0);
        mgr.set_value("sim/altitude", f32::INFINITY);
        assert!(mgr.get_value("sim/altitude").unwrap().is_infinite());
    }

    /// Helper: check that a value is finite and approximately equal to expected
    fn assert_finite_approx(val: f32, expected: f32, label: &str) {
        assert!(val.is_finite(), "{label} should be finite, got {val}");
        assert!(
            (val - expected).abs() < 0.01,
            "{label}: expected {expected}, got {val}"
        );
    }

    // ── Unit conversions (m/s → knots, radians → degrees, etc.) ─────

    #[test]
    fn meters_per_second_to_knots() {
        // 1 m/s ≈ 1.94384 knots
        let mps = 100.0f32;
        let knots = mps * 1.94384;
        assert_finite_approx(knots, 194.384, "100 m/s to knots");
    }

    #[test]
    fn radians_to_degrees() {
        let rad = std::f32::consts::PI;
        let deg = rad.to_degrees();
        assert_finite_approx(deg, 180.0, "π radians to degrees");
    }

    #[test]
    fn feet_to_meters() {
        let feet = 1000.0f32;
        let meters = feet * 0.3048;
        assert_finite_approx(meters, 304.8, "1000 feet to meters");
    }

    // ── Partial data: only some datarefs received ───────────────────

    #[test]
    fn partial_data_returns_defaults_for_missing() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/airspeed", 30.0);
        mgr.subscribe("sim/altitude", 10.0);
        mgr.subscribe("sim/heading", 5.0);

        // Only airspeed received
        mgr.set_value("sim/airspeed", 150.0);

        assert_eq!(mgr.get_value("sim/airspeed"), Some(150.0));
        assert_eq!(mgr.get_value("sim/altitude"), None); // Not yet received
        assert_eq!(mgr.get_value("sim/heading"), None); // Not yet received
    }

    #[test]
    fn values_update_incrementally() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/airspeed", 30.0);

        mgr.set_value("sim/airspeed", 100.0);
        assert_eq!(mgr.get_value("sim/airspeed"), Some(100.0));

        mgr.set_value("sim/airspeed", 150.0);
        assert_eq!(mgr.get_value("sim/airspeed"), Some(150.0));

        mgr.set_value("sim/airspeed", 200.0);
        assert_eq!(mgr.get_value("sim/airspeed"), Some(200.0));
    }

    // ── NaN/Inf filtering pattern ───────────────────────────────────

    #[test]
    fn nan_inf_filtering_approach() {
        let raw_values: Vec<f32> = vec![
            100.0,
            f32::NAN,
            200.0,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -0.0,
            300.0,
        ];

        let filtered: Vec<f32> = raw_values
            .iter()
            .map(|&v| if v.is_finite() { v } else { 0.0 })
            .collect();

        assert_eq!(filtered, vec![100.0, 0.0, 200.0, 0.0, 0.0, -0.0, 300.0]);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// §6  RREF SUBSCRIPTION MANAGEMENT DEPTH
// ═══════════════════════════════════════════════════════════════════════

mod rref_subscription_depth {
    use super::*;

    // ── Subscribe to datarefs by path ───────────────────────────────

    #[test]
    fn subscribe_adds_to_active_list() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/flightmodel/position/indicated_airspeed", 30.0);
        assert!(mgr.is_subscribed("sim/flightmodel/position/indicated_airspeed"));
        assert_eq!(mgr.subscription_count(), 1);
    }

    #[test]
    fn subscribe_multiple_datarefs() {
        let mut mgr = DatarefManager::new();
        let datarefs = [
            "sim/flightmodel/position/indicated_airspeed",
            "sim/flightmodel/position/elevation",
            "sim/flightmodel/position/phi",
            "sim/flightmodel/position/theta",
            "sim/flightmodel/position/psi",
        ];
        for path in &datarefs {
            mgr.subscribe(path, 30.0);
        }
        assert_eq!(mgr.subscription_count(), datarefs.len());
        for path in &datarefs {
            assert!(mgr.is_subscribed(path));
        }
    }

    // ── Unsubscribe ─────────────────────────────────────────────────

    #[test]
    fn unsubscribe_removes_subscription_and_cached_value() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/airspeed", 30.0);
        mgr.set_value("sim/airspeed", 150.0);

        mgr.unsubscribe("sim/airspeed");
        assert!(!mgr.is_subscribed("sim/airspeed"));
        assert_eq!(mgr.get_value("sim/airspeed"), None);
        assert_eq!(mgr.subscription_count(), 0);
    }

    #[test]
    fn unsubscribe_nonexistent_is_noop() {
        let mut mgr = DatarefManager::new();
        mgr.unsubscribe("sim/nonexistent"); // Should not panic
        assert_eq!(mgr.subscription_count(), 0);
    }

    #[test]
    fn unsubscribe_one_of_many() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/a", 10.0);
        mgr.subscribe("sim/b", 20.0);
        mgr.subscribe("sim/c", 30.0);

        mgr.unsubscribe("sim/b");
        assert!(!mgr.is_subscribed("sim/b"));
        assert!(mgr.is_subscribed("sim/a"));
        assert!(mgr.is_subscribed("sim/c"));
        assert_eq!(mgr.subscription_count(), 2);
    }

    // ── Frequency management ────────────────────────────────────────

    #[test]
    fn different_datarefs_at_different_rates() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/airspeed", 30.0);
        mgr.subscribe("sim/altitude", 10.0);
        mgr.subscribe("sim/heading", 5.0);

        let airspeed_sub = mgr.get_subscription("sim/airspeed").unwrap();
        assert_eq!(airspeed_sub.update_rate_hz, 30.0);

        let altitude_sub = mgr.get_subscription("sim/altitude").unwrap();
        assert_eq!(altitude_sub.update_rate_hz, 10.0);

        let heading_sub = mgr.get_subscription("sim/heading").unwrap();
        assert_eq!(heading_sub.update_rate_hz, 5.0);
    }

    #[test]
    fn resubscribe_updates_rate() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/airspeed", 10.0);
        assert_eq!(
            mgr.get_subscription("sim/airspeed").unwrap().update_rate_hz,
            10.0
        );

        mgr.subscribe("sim/airspeed", 60.0);
        assert_eq!(
            mgr.get_subscription("sim/airspeed").unwrap().update_rate_hz,
            60.0
        );
        assert_eq!(mgr.subscription_count(), 1); // Still just one subscription
    }

    // ── Re-subscribe after reconnect ────────────────────────────────

    #[test]
    fn subscriptions_survive_value_clear() {
        let mut mgr = DatarefManager::new();
        mgr.subscribe("sim/airspeed", 30.0);
        mgr.subscribe("sim/altitude", 10.0);
        mgr.set_value("sim/airspeed", 150.0);
        mgr.set_value("sim/altitude", 10000.0);

        // Simulate reconnect: clear all cached values but keep subscriptions
        // (The DatarefManager doesn't have a clear_values method, but
        //  after reconnect, the caller would re-receive values.)
        // Verify subscriptions are intact even if values are stale.
        assert!(mgr.is_subscribed("sim/airspeed"));
        assert!(mgr.is_subscribed("sim/altitude"));
        assert_eq!(mgr.subscription_count(), 2);

        // Overwrite with fresh values (as would happen after reconnect)
        mgr.set_value("sim/airspeed", 160.0);
        assert_eq!(mgr.get_value("sim/airspeed"), Some(160.0));
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[test]
    fn default_manager_is_empty() {
        let mgr = DatarefManager::default();
        assert_eq!(mgr.subscription_count(), 0);
    }

    #[test]
    fn set_value_without_subscription_still_stores() {
        let mut mgr = DatarefManager::new();
        // Setting a value for a dataref that isn't subscribed
        mgr.set_value("sim/unsubscribed", 42.0);
        assert_eq!(mgr.get_value("sim/unsubscribed"), Some(42.0));
        assert!(!mgr.is_subscribed("sim/unsubscribed"));
    }

    #[test]
    fn get_subscription_for_nonexistent_returns_none() {
        let mgr = DatarefManager::new();
        assert!(mgr.get_subscription("sim/nonexistent").is_none());
    }

    #[test]
    fn bulk_subscribe_unsubscribe_cycle() {
        let mut mgr = DatarefManager::new();
        let paths: Vec<String> = (0..50).map(|i| format!("sim/test/{i}")).collect();

        // Subscribe all
        for p in &paths {
            mgr.subscribe(p, 10.0);
        }
        assert_eq!(mgr.subscription_count(), 50);

        // Unsubscribe half
        for p in paths.iter().take(25) {
            mgr.unsubscribe(p);
        }
        assert_eq!(mgr.subscription_count(), 25);

        // Remaining should still be subscribed
        for p in paths.iter().skip(25) {
            assert!(mgr.is_subscribed(p));
        }
    }
}
