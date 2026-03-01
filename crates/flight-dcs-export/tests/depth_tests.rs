// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the DCS Export adapter.
//!
//! Covers:
//! 1. Protocol parsing (Export.lua text protocol, device-arg blocks, instruments)
//! 2. Aircraft database (40+ modules, fuzzy/alias lookup, category filtering)
//! 3. State machine (valid transitions, invalid rejects, recovery, backoff)
//! 4. Control injection (wire format, clamping, injector lifecycle)
//! 5. Telemetry conversion (unit helpers, NaN/Inf filtering, safe defaults)
//! 6. Integration scenarios (full pipeline, reconnect, graceful degradation)

use flight_dcs_export::control_injection::{fa18c, f16c, a10c, f14b, ah64d};
use flight_dcs_export::{
    AircraftCategory, AxesProfile,
    DcsAdapterEvent, DcsAdapterState, DcsAdapterStateMachine, DcsTransitionError,
    DcsControlCommand, DcsControlInjector, DcsActionType,
    DcsFlightData,
    lookup_aircraft_axis, parse_wire_command, parse_wire_payload,
};
use flight_dcs_export::aircraft_db;
use flight_dcs_export::aircraft_detection::{classify_fidelity, ModuleFidelity};
use flight_dcs_export::protocol::{
    parse_export_line, parse_indicator_value, parse_telemetry_batch,
    parse_position_data, parse_multi_value, parse_device_arg_block,
    parse_instrument_block, parse_aircraft_type, dcs_to_ned, m_to_ft, ms_to_knots, rad_to_deg,
};

// ============================================================================
// 1. Protocol parsing depth
// ============================================================================

mod protocol_parsing {
    use super::*;

    // --- Export.lua text protocol: key=value parsing ---

    #[test]
    fn multiline_batch_preserves_all_entries() {
        let batch = [
            "HEADER:timestamp=100.0,model_time=50.0,aircraft=F-16C_50",
            "altitude_m=8000.0",
            "airspeed_ms=200.0",
            "heading_deg=45.0",
            "pitch_deg=10.0",
            "roll_deg=-5.0",
            "aoa_deg=6.0",
            "g_load=2.0",
            "mach=0.75",
            "vertical_speed_ms=10.0",
            "fuel_total_kg=2800.0",
            "engine_rpm_0=92.0",
            "engine_rpm_1=93.0",
            "gear_main_l=0.0",
            "gear_main_r=0.0",
            "gear_nose=0.0",
            "custom_gauge=0.5",
        ]
        .join("\n");

        let pkt = parse_telemetry_batch(&batch).unwrap();
        assert_eq!(pkt.aircraft_name, "F-16C_50");
        assert!((pkt.flight_data.altitude_m - 8000.0).abs() < f64::EPSILON);
        assert!((pkt.flight_data.mach - 0.75).abs() < 1e-10);
        assert_eq!(pkt.flight_data.engine_rpm_percent.len(), 2);
        assert_eq!(pkt.flight_data.gear_position.len(), 3);
        assert!(pkt.indicators.contains_key("custom_gauge"));
    }

    #[test]
    fn lua_comment_stripping_in_body_lines() {
        let batch = [
            "HEADER:timestamp=1.0,model_time=0.0,aircraft=Su-25T",
            "altitude_m=500.0 -- terrain following",
            "mach=0.5 -- subsonic",
        ]
        .join("\n");
        let pkt = parse_telemetry_batch(&batch).unwrap();
        assert!((pkt.flight_data.altitude_m - 500.0).abs() < f64::EPSILON);
        assert!((pkt.flight_data.mach - 0.5).abs() < 1e-10);
    }

    // --- Malformed packets: graceful error, no panic ---

    #[test]
    fn empty_packet_returns_error() {
        assert!(parse_telemetry_batch("").is_err());
    }

    #[test]
    fn header_only_no_crash() {
        let pkt = parse_telemetry_batch(
            "HEADER:timestamp=1.0,model_time=1.0,aircraft=Test\n",
        )
        .unwrap();
        assert!(pkt.indicators.is_empty());
    }

    #[test]
    fn truncated_header_returns_error() {
        assert!(parse_telemetry_batch("HEADER:timestamp=1.0").is_err());
    }

    #[test]
    fn missing_header_prefix_returns_error() {
        assert!(parse_telemetry_batch("altitude_m=5000").is_err());
    }

    #[test]
    fn oversized_value_parsed_as_numeric() {
        let batch = format!(
            "HEADER:timestamp=1.0,model_time=1.0,aircraft=T\naltitude_m={}",
            f64::MAX
        );
        let pkt = parse_telemetry_batch(&batch).unwrap();
        assert!(pkt.flight_data.altitude_m.is_finite());
    }

    #[test]
    fn nan_and_inf_in_body_parsed_correctly() {
        let batch = [
            "HEADER:timestamp=1.0,model_time=1.0,aircraft=T",
            "altitude_m=nan",
            "mach=inf",
        ]
        .join("\n");
        let pkt = parse_telemetry_batch(&batch).unwrap();
        assert!(pkt.flight_data.altitude_m.is_nan());
        assert!(pkt.flight_data.mach.is_infinite());
    }

    #[test]
    fn garbage_body_lines_skipped_gracefully() {
        let batch = [
            "HEADER:timestamp=1.0,model_time=1.0,aircraft=T",
            "not_key_value",
            "altitude_m=1000.0",
            "=missingkey",
            "heading_deg=90.0",
        ]
        .join("\n");
        let pkt = parse_telemetry_batch(&batch).unwrap();
        assert!((pkt.flight_data.altitude_m - 1000.0).abs() < f64::EPSILON);
        assert!((pkt.flight_data.heading_deg - 90.0).abs() < f64::EPSILON);
    }

    // --- Device argument block parsing ---

    #[test]
    fn device_arg_block_empty() {
        let block = "ARGS_BEGIN\nARGS_END\n";
        let entries = parse_device_arg_block(block).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn device_arg_block_multiple_entries() {
        let block = "ARGS_BEGIN\n0:71:0.5\n0:85:1.0\n4:200:0.75\nARGS_END\n";
        let entries = parse_device_arg_block(block).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].device_id, 0);
        assert_eq!(entries[0].arg_number, 71);
        assert!((entries[0].value - 0.5).abs() < 1e-10);
        assert_eq!(entries[2].device_id, 4);
    }

    #[test]
    fn device_arg_block_with_leading_text_ignored() {
        let block = "some preamble\nmore text\nARGS_BEGIN\n1:10:0.0\nARGS_END\n";
        let entries = parse_device_arg_block(block).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn device_arg_block_malformed_entry_returns_error() {
        let block = "ARGS_BEGIN\nbadline\nARGS_END\n";
        assert!(parse_device_arg_block(block).is_err());
    }

    // --- Instrument block parsing ---

    #[test]
    fn instrument_block_basic() {
        let block = [
            "INSTRUMENTS_BEGIN",
            "AltimeterPressure=29.92",
            "ADI_Pitch=5.3",
            "INSTRUMENTS_END",
        ]
        .join("\n");
        let readings = parse_instrument_block(&block).unwrap();
        assert_eq!(readings.len(), 2);
        assert_eq!(readings[0].name, "AltimeterPressure");
        assert!((readings[0].value - 29.92).abs() < 1e-10);
    }

    #[test]
    fn instrument_block_empty() {
        let block = "INSTRUMENTS_BEGIN\nINSTRUMENTS_END\n";
        let readings = parse_instrument_block(block).unwrap();
        assert!(readings.is_empty());
    }

    // --- Position data parsing ---

    #[test]
    fn position_data_with_negative_lat_lon() {
        let (lat, lon, alt) =
            parse_position_data("{lat=-34.0,lon=-58.0,alt=25.0}").unwrap();
        assert!((lat - (-34.0)).abs() < 1e-10);
        assert!((lon - (-58.0)).abs() < 1e-10);
        assert!((alt - 25.0).abs() < 1e-10);
    }

    #[test]
    fn position_data_zero_altitude() {
        let (_, _, alt) =
            parse_position_data("{lat=0.0,lon=0.0,alt=0.0}").unwrap();
        assert!(alt.abs() < 1e-10);
    }

    // --- Multi-value parsing ---

    #[test]
    fn multi_value_four_engines() {
        let vals = parse_multi_value("95.0;94.5;96.0;93.0").unwrap();
        assert_eq!(vals.len(), 4);
    }

    #[test]
    fn multi_value_special_values() {
        let vals = parse_multi_value("nan;inf;-inf").unwrap();
        assert!(vals[0].is_nan());
        assert!(vals[1].is_infinite() && vals[1].is_sign_positive());
        assert!(vals[2].is_infinite() && vals[2].is_sign_negative());
    }

    // --- Aircraft type parsing ---

    #[test]
    fn parse_aircraft_type_strips_pilot_suffix() {
        assert_eq!(parse_aircraft_type("AH-64D_BLK_II_pilot"), "AH-64D_BLK_II");
    }

    #[test]
    fn parse_aircraft_type_strips_copilot_suffix() {
        assert_eq!(parse_aircraft_type("AH-64D_BLK_II_copilot"), "AH-64D_BLK_II");
    }

    #[test]
    fn parse_aircraft_type_strips_player_suffix() {
        assert_eq!(parse_aircraft_type("F-16C_50_player"), "F-16C_50");
    }

    #[test]
    fn parse_aircraft_type_preserves_normal_name() {
        assert_eq!(parse_aircraft_type("F-16C_50"), "F-16C_50");
    }

    #[test]
    fn parse_aircraft_type_trims_whitespace() {
        assert_eq!(parse_aircraft_type("  F-14B  "), "F-14B");
    }

    // --- Coordinate conversion ---

    #[test]
    fn dcs_to_ned_conversion() {
        let (n, e, d) = dcs_to_ned(100.0, 50.0, 200.0);
        assert!((n - 100.0).abs() < f64::EPSILON);
        assert!((e - 200.0).abs() < f64::EPSILON);
        assert!((d - (-50.0)).abs() < f64::EPSILON);
    }

    // --- Unit conversion helpers ---

    #[test]
    fn metres_to_feet() {
        let ft = m_to_ft(1000.0);
        assert!((ft - 3280.84).abs() < 0.01);
    }

    #[test]
    fn ms_to_knots_conversion() {
        let kts = ms_to_knots(100.0);
        assert!((kts - 194.3844).abs() < 0.001);
    }

    #[test]
    fn rad_to_deg_conversion() {
        let deg = rad_to_deg(std::f64::consts::PI);
        assert!((deg - 180.0).abs() < 1e-10);
    }

    #[test]
    fn rad_to_deg_zero() {
        assert!((rad_to_deg(0.0)).abs() < 1e-10);
    }

    // --- Property-like round-trip: encode → decode ---

    #[test]
    fn export_line_roundtrip_various_values() {
        let test_cases = [
            ("key1", "42.0"),
            ("altitude_m", "-100.5"),
            ("mach", "0.0"),
            ("heading", "359.99"),
        ];
        for (key, value) in &test_cases {
            let line = format!("{key}={value}");
            let entry = parse_export_line(&line).unwrap();
            assert_eq!(entry.key, *key);
            assert_eq!(entry.value, *value);
        }
    }

    #[test]
    fn indicator_value_roundtrip_special_cases() {
        // Empty → 0.0
        assert!((parse_indicator_value("").unwrap()).abs() < f64::EPSILON);
        // Dash → 0.0
        assert!((parse_indicator_value("-").unwrap()).abs() < f64::EPSILON);
        // Lua fractions
        assert!(parse_indicator_value("1/0").unwrap().is_infinite());
        assert!(parse_indicator_value("0/0").unwrap().is_nan());
        assert!(parse_indicator_value("-1/0").unwrap().is_sign_negative());
    }
}

// ============================================================================
// 2. Aircraft database depth
// ============================================================================

mod aircraft_database {
    use super::*;

    #[test]
    fn database_has_at_least_30_aircraft() {
        assert!(
            aircraft_db::all_aircraft().len() >= 30,
            "DB has {} entries, expected ≥ 30",
            aircraft_db::all_aircraft().len()
        );
    }

    #[test]
    fn all_dcs_names_are_unique() {
        let names: Vec<_> = aircraft_db::all_aircraft().iter().map(|a| a.dcs_name).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names.len(), sorted.len(), "duplicate dcs_name detected");
    }

    #[test]
    fn all_entries_have_non_empty_names() {
        for a in aircraft_db::all_aircraft() {
            assert!(!a.dcs_name.is_empty(), "empty dcs_name");
            assert!(!a.display_name.is_empty(), "empty display_name for {}", a.dcs_name);
        }
    }

    // --- Exact lookup for all major DCS modules ---

    #[test]
    fn lookup_modern_jets() {
        let jets = [
            ("F-16C_50", AircraftCategory::FixedWing, AxesProfile::StandardJet),
            ("FA-18C_hornet", AircraftCategory::FixedWing, AxesProfile::StandardJet),
            ("A-10C_2", AircraftCategory::FixedWing, AxesProfile::StandardJet),
            ("F-14B", AircraftCategory::FixedWing, AxesProfile::StandardJet),
            ("F-15ESE", AircraftCategory::FixedWing, AxesProfile::StandardJet),
            ("JF-17", AircraftCategory::FixedWing, AxesProfile::StandardJet),
            ("AJS37", AircraftCategory::FixedWing, AxesProfile::StandardJet),
            ("M-2000C", AircraftCategory::FixedWing, AxesProfile::StandardJet),
            ("AV8BNA", AircraftCategory::FixedWing, AxesProfile::StandardJet),
            ("F-5E-3", AircraftCategory::FixedWing, AxesProfile::StandardJet),
            ("MiG-21Bis", AircraftCategory::FixedWing, AxesProfile::StandardJet),
        ];
        for (name, cat, axes) in &jets {
            let info = aircraft_db::lookup(name)
                .unwrap_or_else(|| panic!("missing aircraft: {name}"));
            assert_eq!(info.category, *cat, "{name}: wrong category");
            assert_eq!(info.axes_config, *axes, "{name}: wrong axes");
        }
    }

    #[test]
    fn lookup_fc3_aircraft() {
        let fc3 = ["Su-25T", "Su-27", "Su-33", "MiG-29A", "MiG-29S", "F-15C"];
        for name in &fc3 {
            assert!(
                aircraft_db::lookup(name).is_some(),
                "FC3 aircraft {name} missing from DB"
            );
        }
    }

    #[test]
    fn lookup_helicopters() {
        let helis = [
            ("AH-64D_BLK_II", AxesProfile::HelicopterCollective),
            ("Mi-8MT", AxesProfile::HelicopterCollective),
            ("Mi-24P", AxesProfile::HelicopterCollective),
            ("UH-1H", AxesProfile::HelicopterCollective),
            ("Ka-50_3", AxesProfile::HelicopterCollective),
            ("SA342M", AxesProfile::HelicopterCollective),
            ("OH58D", AxesProfile::HelicopterCollective),
        ];
        for (name, axes) in &helis {
            let info = aircraft_db::lookup(name)
                .unwrap_or_else(|| panic!("missing heli: {name}"));
            assert_eq!(info.category, AircraftCategory::Helicopter, "{name}");
            assert_eq!(info.axes_config, *axes, "{name}");
        }
    }

    #[test]
    fn lookup_warbirds() {
        let warbirds = [
            ("TF-51D", AxesProfile::Warbird4Axis),
            ("SpitfireLFMkIX", AxesProfile::Warbird4Axis),
            ("Bf-109K-4", AxesProfile::Warbird4Axis),
            ("FW-190D9", AxesProfile::Warbird4Axis),
            ("P-47D-30", AxesProfile::Warbird4Axis),
            ("MosquitoFBMkVI", AxesProfile::Warbird4Axis),
            ("I-16", AxesProfile::Warbird4Axis),
        ];
        for (name, axes) in &warbirds {
            let info = aircraft_db::lookup(name)
                .unwrap_or_else(|| panic!("missing warbird: {name}"));
            assert_eq!(info.category, AircraftCategory::WarBird, "{name}");
            assert_eq!(info.axes_config, *axes, "{name}");
        }
    }

    #[test]
    fn lookup_transports_and_trainers() {
        let info = aircraft_db::lookup("Hercules").unwrap();
        assert_eq!(info.category, AircraftCategory::TransportCargo);
        assert_eq!(info.axes_config, AxesProfile::YokeThrottle);

        let info = aircraft_db::lookup("C-101CC").unwrap();
        assert_eq!(info.category, AircraftCategory::TransportCargo);

        let info = aircraft_db::lookup("L-39C").unwrap();
        assert_eq!(info.category, AircraftCategory::TrainerJet);

        let info = aircraft_db::lookup("MB-339A").unwrap();
        assert_eq!(info.category, AircraftCategory::TrainerJet);
    }

    // --- Fuzzy / alias lookup ---

    #[test]
    fn fuzzy_lookup_partial_match() {
        assert_eq!(
            aircraft_db::lookup_fuzzy("mosquito").unwrap().dcs_name,
            "MosquitoFBMkVI"
        );
    }

    #[test]
    fn fuzzy_lookup_case_insensitive() {
        assert_eq!(
            aircraft_db::lookup_fuzzy("KA-50").unwrap().dcs_name,
            "Ka-50_3"
        );
        assert_eq!(
            aircraft_db::lookup_fuzzy("f-16").unwrap().dcs_name,
            "F-16C_50"
        );
    }

    #[test]
    fn fuzzy_lookup_unknown_module_returns_none() {
        assert!(aircraft_db::lookup_fuzzy("Boeing747").is_none());
        assert!(aircraft_db::lookup_fuzzy("Airbus A320").is_none());
    }

    #[test]
    fn exact_lookup_unknown_returns_none() {
        assert!(aircraft_db::lookup("UnknownMod").is_none());
        assert!(aircraft_db::lookup("").is_none());
    }

    // --- Category filtering ---

    #[test]
    fn by_category_returns_only_matching() {
        for cat in [
            AircraftCategory::FixedWing,
            AircraftCategory::Helicopter,
            AircraftCategory::WarBird,
            AircraftCategory::TrainerJet,
            AircraftCategory::TransportCargo,
        ] {
            let results = aircraft_db::by_category(cat);
            assert!(
                !results.is_empty(),
                "category {cat} has no aircraft"
            );
            for a in &results {
                assert_eq!(a.category, cat);
            }
        }
    }

    #[test]
    fn with_ffb_profiles_at_least_five() {
        let ffb = aircraft_db::with_ffb_profiles();
        assert!(ffb.len() >= 5, "only {} FFB profiles", ffb.len());
        for a in &ffb {
            assert!(a.has_ffb_profile);
        }
    }

    // --- Fidelity classification ---

    #[test]
    fn fc3_modules_classified_correctly() {
        for name in ["Su-25T", "Su-27", "Su-33", "MiG-29A", "MiG-29S", "F-15C"] {
            assert_eq!(
                classify_fidelity(name),
                ModuleFidelity::Fc3,
                "{name} should be FC3"
            );
        }
    }

    #[test]
    fn full_fidelity_modules_classified_correctly() {
        for name in ["F-16C_50", "FA-18C_hornet", "A-10C_2", "AH-64D_BLK_II"] {
            assert_eq!(
                classify_fidelity(name),
                ModuleFidelity::FullFidelity,
                "{name} should be FullFidelity"
            );
        }
    }

    #[test]
    fn unknown_module_classified_as_mod() {
        assert_eq!(classify_fidelity("CommunityMod_X"), ModuleFidelity::Mod);
    }

    // --- Display impls ---

    #[test]
    fn category_display_all_variants() {
        assert_eq!(AircraftCategory::FixedWing.to_string(), "Fixed Wing");
        assert_eq!(AircraftCategory::Helicopter.to_string(), "Helicopter");
        assert_eq!(AircraftCategory::TrainerJet.to_string(), "Trainer Jet");
        assert_eq!(AircraftCategory::WarBird.to_string(), "Warbird");
        assert_eq!(AircraftCategory::TransportCargo.to_string(), "Transport/Cargo");
    }

    #[test]
    fn axes_profile_display_all_variants() {
        assert_eq!(AxesProfile::StandardJet.to_string(), "Standard Jet");
        assert_eq!(AxesProfile::HelicopterCollective.to_string(), "Helicopter Collective");
        assert_eq!(AxesProfile::YokeThrottle.to_string(), "Yoke + Throttle");
        assert_eq!(AxesProfile::Warbird4Axis.to_string(), "Warbird 4-Axis");
    }
}

// ============================================================================
// 3. State machine depth
// ============================================================================

mod state_machine {
    use super::*;

    fn sm() -> DcsAdapterStateMachine {
        DcsAdapterStateMachine::new(2000, 3)
    }

    fn to_active(sm: &mut DcsAdapterStateMachine) {
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
    }

    // --- Full happy-path: Disconnected → Connecting → Listening → Connected → Active ---

    #[test]
    fn full_happy_path_with_listening() {
        let mut sm = sm();
        assert_eq!(sm.state(), DcsAdapterState::Disconnected);

        let s = sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        assert_eq!(s, DcsAdapterState::Connecting);

        let s = sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
        assert_eq!(s, DcsAdapterState::Listening);

        let s = sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        assert_eq!(s, DcsAdapterState::Connected);

        let s = sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(s, DcsAdapterState::Active);

        assert!(sm.is_healthy());
        assert_eq!(sm.error_count(), 0);
    }

    // --- UDP shortcut: Listening → Active (skip Connected on first telemetry) ---

    #[test]
    fn listening_to_active_on_first_telemetry() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
        let s = sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(s, DcsAdapterState::Active);
    }

    // --- Timeout transitions ---

    #[test]
    fn active_to_stale_on_timeout() {
        let mut sm = sm();
        to_active(&mut sm);
        let s = sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        assert_eq!(s, DcsAdapterState::Stale);
        assert_eq!(sm.consecutive_stale_count(), 1);
    }

    #[test]
    fn stale_accumulates_on_repeated_timeout() {
        let mut sm = sm();
        to_active(&mut sm);
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        assert_eq!(sm.consecutive_stale_count(), 3);
        assert_eq!(sm.state(), DcsAdapterState::Stale);
    }

    #[test]
    fn stale_exhausted_to_disconnected() {
        let mut sm = sm();
        to_active(&mut sm);
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        let s = sm.transition(DcsAdapterEvent::StaleExhausted).unwrap();
        assert_eq!(s, DcsAdapterState::Disconnected);
    }

    // --- Connection loss recovery ---

    #[test]
    fn stale_to_active_on_telemetry_recovery() {
        let mut sm = sm();
        to_active(&mut sm);
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        assert_eq!(sm.consecutive_stale_count(), 2);
        let s = sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(s, DcsAdapterState::Active);
        assert_eq!(sm.consecutive_stale_count(), 0);
        assert_eq!(sm.error_count(), 0);
    }

    #[test]
    fn error_recovery_then_full_reconnect() {
        let mut sm = sm();
        // First connection fails
        sm.transition(DcsAdapterEvent::ConnectionError("timeout".into()))
            .unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Error);
        assert_eq!(sm.error_count(), 1);
        assert!(sm.is_recoverable());

        // Retry succeeds
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Active);
        assert_eq!(sm.error_count(), 0);
    }

    // --- Invalid state transitions rejected ---

    #[test]
    fn disconnected_rejects_telemetry() {
        let mut sm = sm();
        assert!(matches!(
            sm.transition(DcsAdapterEvent::TelemetryReceived),
            Err(DcsTransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn disconnected_rejects_handshake() {
        let mut sm = sm();
        assert!(matches!(
            sm.transition(DcsAdapterEvent::HandshakeCompleted),
            Err(DcsTransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn disconnected_rejects_timeout() {
        let mut sm = sm();
        assert!(matches!(
            sm.transition(DcsAdapterEvent::TelemetryTimeout),
            Err(DcsTransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn connecting_rejects_telemetry_received() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        assert!(matches!(
            sm.transition(DcsAdapterEvent::TelemetryReceived),
            Err(DcsTransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn connecting_rejects_timeout() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        assert!(matches!(
            sm.transition(DcsAdapterEvent::TelemetryTimeout),
            Err(DcsTransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn connected_rejects_timeout() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        assert!(matches!(
            sm.transition(DcsAdapterEvent::TelemetryTimeout),
            Err(DcsTransitionError::InvalidTransition { .. })
        ));
    }

    // --- Retries exhausted ---

    #[test]
    fn retries_exhausted_prevents_reconnect() {
        let mut sm = DcsAdapterStateMachine::new(2000, 2);
        sm.transition(DcsAdapterEvent::ConnectionError("e1".into()))
            .unwrap();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::ConnectionError("e2".into()))
            .unwrap();
        // 2 errors = max_retries, should fail
        let res = sm.transition(DcsAdapterEvent::SocketBound);
        assert!(matches!(
            res,
            Err(DcsTransitionError::RetriesExhausted { max_retries: 2 })
        ));
        assert!(!sm.is_recoverable());
    }

    // --- Shutdown from any state ---

    #[test]
    fn shutdown_from_active_resets_everything() {
        let mut sm = sm();
        to_active(&mut sm);
        sm.transition(DcsAdapterEvent::Shutdown).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Disconnected);
        assert_eq!(sm.error_count(), 0);
        assert_eq!(sm.consecutive_stale_count(), 0);
    }

    #[test]
    fn shutdown_from_error_clears_counters() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::ConnectionError("e".into()))
            .unwrap();
        sm.transition(DcsAdapterEvent::Shutdown).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Disconnected);
        assert_eq!(sm.error_count(), 0);
    }

    #[test]
    fn shutdown_from_stale_clears_stale_count() {
        let mut sm = sm();
        to_active(&mut sm);
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        sm.transition(DcsAdapterEvent::Shutdown).unwrap();
        assert_eq!(sm.consecutive_stale_count(), 0);
    }

    // --- DcsDisconnected from connected states ---

    #[test]
    fn dcs_disconnect_from_active() {
        let mut sm = sm();
        to_active(&mut sm);
        let s = sm.transition(DcsAdapterEvent::DcsDisconnected).unwrap();
        assert_eq!(s, DcsAdapterState::Disconnected);
    }

    #[test]
    fn dcs_disconnect_from_listening() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
        let s = sm.transition(DcsAdapterEvent::DcsDisconnected).unwrap();
        assert_eq!(s, DcsAdapterState::Disconnected);
    }

    #[test]
    fn dcs_disconnect_from_connected() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        let s = sm.transition(DcsAdapterEvent::DcsDisconnected).unwrap();
        assert_eq!(s, DcsAdapterState::Disconnected);
    }

    // --- ConnectionError from any state → Error ---

    #[test]
    fn connection_error_from_active() {
        let mut sm = sm();
        to_active(&mut sm);
        let s = sm
            .transition(DcsAdapterEvent::ConnectionError("oops".into()))
            .unwrap();
        assert_eq!(s, DcsAdapterState::Error);
        assert_eq!(sm.error_count(), 1);
    }

    #[test]
    fn connection_error_from_disconnected() {
        let mut sm = sm();
        let s = sm
            .transition(DcsAdapterEvent::ConnectionError("no socket".into()))
            .unwrap();
        assert_eq!(s, DcsAdapterState::Error);
    }

    // --- Helper method coverage ---

    #[test]
    fn is_healthy_only_when_connected_or_active() {
        let mut sm = sm();
        assert!(!sm.is_healthy());

        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        assert!(!sm.is_healthy());

        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        assert!(sm.is_healthy()); // Connected

        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert!(sm.is_healthy()); // Active
    }

    #[test]
    fn should_reconnect_when_disconnected_and_recoverable() {
        let sm = sm();
        assert!(sm.should_reconnect()); // Disconnected with 0 errors
    }

    #[test]
    fn reset_clears_all_state() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::ConnectionError("e".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 1);
        sm.reset();
        assert_eq!(sm.state(), DcsAdapterState::Disconnected);
        assert_eq!(sm.error_count(), 0);
        assert_eq!(sm.consecutive_stale_count(), 0);
    }

    #[test]
    fn stale_threshold_and_max_retries_accessors() {
        let sm = DcsAdapterStateMachine::new(5000, 10);
        assert_eq!(sm.stale_threshold_ms(), 5000);
        assert_eq!(sm.max_retries(), 10);
    }

    #[test]
    fn with_max_stale_builder() {
        let sm = DcsAdapterStateMachine::new(2000, 3).with_max_stale(5);
        assert_eq!(sm.max_stale_before_disconnect(), 5);
    }

    #[test]
    fn reconnect_delay_increases_with_errors() {
        let mut sm = sm();
        let d0 = sm.reconnect_delay();
        sm.transition(DcsAdapterEvent::ConnectionError("e".into()))
            .unwrap();
        let d1 = sm.reconnect_delay();
        assert!(d1 > d0, "delay should increase after error");
    }

    #[test]
    fn time_in_state_starts_none() {
        let sm = sm();
        assert!(sm.time_in_state().is_none());
    }

    #[test]
    fn time_in_state_some_after_transition() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        assert!(sm.time_in_state().is_some());
    }
}

// ============================================================================
// 4. Control injection depth
// ============================================================================

mod control_injection_depth {
    use super::*;

    // --- DCS command format validation ---

    #[test]
    fn axis_command_wire_format() {
        let cmd = DcsControlCommand::axis(0, 2001, 0.5);
        assert_eq!(cmd.to_wire(), "CMD:0,2001,0.500000");
    }

    #[test]
    fn button_press_wire_format() {
        let cmd = DcsControlCommand::button_press(4, 3001);
        assert_eq!(cmd.to_wire(), "BTN:4,3001,1.000000");
    }

    #[test]
    fn button_release_wire_format() {
        let cmd = DcsControlCommand::button_release(4, 3001);
        assert_eq!(cmd.to_wire(), "BTN:4,3001,0.000000");
    }

    #[test]
    fn toggle_wire_format() {
        let cmd = DcsControlCommand::toggle(2, 500);
        assert_eq!(cmd.to_wire(), "TGL:2,500,1.000000");
    }

    // --- Value range clamping ---

    #[test]
    fn axis_clamps_above_one() {
        let cmd = DcsControlCommand::axis(0, 1, 5.0);
        assert!((cmd.value - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn axis_clamps_below_negative_one() {
        let cmd = DcsControlCommand::axis(0, 1, -5.0);
        assert!((cmd.value - (-1.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn axis_preserves_value_in_range() {
        let cmd = DcsControlCommand::axis(0, 1, 0.75);
        assert!((cmd.value - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn axis_clamps_negative_boundary() {
        let cmd = DcsControlCommand::axis(0, 1, -1.0);
        assert!((cmd.value - (-1.0)).abs() < f64::EPSILON);
    }

    // --- Wire command round-trip (encode → decode) ---

    #[test]
    fn wire_roundtrip_axis() {
        let original = DcsControlCommand::axis(0, 2001, 0.5);
        let wire = original.to_wire();
        let parsed = parse_wire_command(&wire).unwrap();
        assert_eq!(parsed.device_id, 0);
        assert_eq!(parsed.command_id, 2001);
        assert!((parsed.value - 0.5).abs() < 1e-6);
        assert_eq!(parsed.action_type, DcsActionType::Axis);
    }

    #[test]
    fn wire_roundtrip_button_press() {
        let original = DcsControlCommand::button_press(4, 3001);
        let wire = original.to_wire();
        let parsed = parse_wire_command(&wire).unwrap();
        assert_eq!(parsed.device_id, 4);
        assert_eq!(parsed.command_id, 3001);
        assert!((parsed.value - 1.0).abs() < 1e-6);
        // BTN with value > 0.5 → ButtonPress
        assert_eq!(parsed.action_type, DcsActionType::ButtonPress);
    }

    #[test]
    fn wire_roundtrip_button_release() {
        let original = DcsControlCommand::button_release(4, 3001);
        let wire = original.to_wire();
        let parsed = parse_wire_command(&wire).unwrap();
        assert!((parsed.value).abs() < 1e-6);
        assert_eq!(parsed.action_type, DcsActionType::ButtonRelease);
    }

    #[test]
    fn wire_roundtrip_toggle() {
        let original = DcsControlCommand::toggle(2, 500);
        let wire = original.to_wire();
        let parsed = parse_wire_command(&wire).unwrap();
        assert_eq!(parsed.action_type, DcsActionType::Toggle);
    }

    // --- Malformed wire commands ---

    #[test]
    fn wire_parse_unknown_prefix() {
        assert!(parse_wire_command("XYZ:0,1,0.5").is_err());
    }

    #[test]
    fn wire_parse_missing_colon() {
        assert!(parse_wire_command("CMD0,1,0.5").is_err());
    }

    #[test]
    fn wire_parse_too_few_fields() {
        assert!(parse_wire_command("CMD:0,1").is_err());
    }

    #[test]
    fn wire_parse_too_many_fields() {
        assert!(parse_wire_command("CMD:0,1,0.5,extra").is_err());
    }

    #[test]
    fn wire_parse_non_numeric_device_id() {
        assert!(parse_wire_command("CMD:abc,1,0.5").is_err());
    }

    #[test]
    fn wire_parse_empty_string() {
        assert!(parse_wire_command("").is_err());
    }

    // --- Multi-line payload parsing ---

    #[test]
    fn parse_wire_payload_multiple_commands() {
        let payload = "CMD:0,2001,0.500000\nBTN:4,3001,1.000000\nTGL:2,500,1.000000\n";
        let results = parse_wire_payload(payload);
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn parse_wire_payload_skips_blank_lines() {
        let payload = "CMD:0,2001,0.5\n\n\nBTN:4,3001,1.0\n";
        let results = parse_wire_payload(payload);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn parse_wire_payload_with_bad_lines() {
        let payload = "CMD:0,2001,0.5\ngarbage\nBTN:4,3001,1.0\n";
        let results = parse_wire_payload(payload);
        assert_eq!(results.len(), 3);
        assert!(results[0].is_ok());
        assert!(results[1].is_err());
        assert!(results[2].is_ok());
    }

    // --- Injector lifecycle ---

    #[test]
    fn injector_queue_and_flush() {
        let mut inj = DcsControlInjector::new(8);
        assert_eq!(inj.pending_count(), 0);

        inj.queue_command(DcsControlCommand::axis(0, 2001, 0.5));
        inj.queue_command(DcsControlCommand::button_press(4, 3001));
        assert_eq!(inj.pending_count(), 2);

        let payload = inj.flush();
        let text = String::from_utf8(payload).unwrap();
        assert!(text.contains("CMD:0,2001,0.500000\n"));
        assert!(text.contains("BTN:4,3001,1.000000\n"));
        assert_eq!(inj.pending_count(), 0);
    }

    #[test]
    fn injector_rejects_when_full() {
        let mut inj = DcsControlInjector::new(2);
        assert!(inj.queue_command(DcsControlCommand::axis(0, 1, 0.0)));
        assert!(inj.queue_command(DcsControlCommand::axis(0, 2, 0.0)));
        assert!(!inj.queue_command(DcsControlCommand::axis(0, 3, 0.0)));
    }

    #[test]
    fn injector_flush_empty_returns_empty() {
        let mut inj = DcsControlInjector::new(8);
        assert!(inj.flush().is_empty());
    }

    #[test]
    fn injector_clear() {
        let mut inj = DcsControlInjector::new(8);
        inj.queue_command(DcsControlCommand::axis(0, 1, 0.0));
        inj.clear();
        assert_eq!(inj.pending_count(), 0);
    }

    #[test]
    fn injector_set_axis_by_name() {
        let mut inj = DcsControlInjector::new(8);
        assert!(inj.set_axis("pitch", 0.5));
        assert!(inj.set_axis("roll", -0.3));
        assert!(!inj.set_axis("nonexistent_axis", 0.0));
        assert_eq!(inj.pending_count(), 2);
    }

    #[test]
    fn injector_press_release_helpers() {
        let mut inj = DcsControlInjector::new(8);
        assert!(inj.press_button(4, 3001));
        assert!(inj.release_button(4, 3001));
        assert_eq!(inj.pending_count(), 2);
    }

    #[test]
    fn injector_multiple_flushes_independent() {
        let mut inj = DcsControlInjector::new(8);
        inj.queue_command(DcsControlCommand::axis(0, 1, 0.1));
        let p1 = String::from_utf8(inj.flush()).unwrap();

        inj.queue_command(DcsControlCommand::axis(0, 2, 0.2));
        let p2 = String::from_utf8(inj.flush()).unwrap();

        assert!(p1.contains("CMD:0,1,"));
        assert!(!p1.contains("CMD:0,2,"));
        assert!(p2.contains("CMD:0,2,"));
    }

    // --- Module-specific command tables ---

    #[test]
    fn fa18c_ufc_commands_exist() {
        assert!(fa18c::lookup_command("UFC_1").is_some());
        assert!(fa18c::lookup_command("UFC_ENT").is_some());
        assert!(fa18c::lookup_command("UFC_CLR").is_some());
        assert!(fa18c::lookup_command("MASTER_ARM_ON").is_some());
        assert!(fa18c::lookup_command("NONEXISTENT").is_none());
    }

    #[test]
    fn f16c_icp_commands_exist() {
        assert!(f16c::lookup_command("ICP_0").is_some());
        assert!(f16c::lookup_command("ICP_ENTR").is_some());
        assert!(f16c::lookup_command("MASTER_ARM_TOGGLE").is_some());
    }

    #[test]
    fn a10c_cdu_commands_exist() {
        assert!(a10c::lookup_command("CDU_1").is_some());
        assert!(a10c::lookup_command("CMSP_JMR").is_some());
    }

    #[test]
    fn f14b_commands_exist() {
        assert!(f14b::lookup_command("WING_SWEEP_AUTO").is_some());
        assert!(f14b::lookup_command("RIO_CAP_TID_MODE").is_some());
    }

    #[test]
    fn ah64d_commands_exist() {
        assert!(ah64d::lookup_command("PLT_KU_A").is_some());
        assert!(ah64d::lookup_command("CPG_KU_ENT").is_some());
    }

    // --- Per-aircraft axis mapping ---

    #[test]
    fn fa18c_axis_mapping() {
        let m = lookup_aircraft_axis("FA-18C_hornet", "pitch").unwrap();
        assert_eq!(m.device_id, 0);
        assert_eq!(m.command_id, 2001);

        // Alias works too
        let m2 = lookup_aircraft_axis("FA-18C", "roll").unwrap();
        assert_eq!(m2.command_id, 2002);
    }

    #[test]
    fn f16c_axis_mapping_single_throttle() {
        let m = lookup_aircraft_axis("F-16C_50", "throttle").unwrap();
        assert_eq!(m.command_id, 2004);

        // F-16C alias
        let m2 = lookup_aircraft_axis("F-16C", "yaw").unwrap();
        assert_eq!(m2.command_id, 2003);
    }

    #[test]
    fn a10c_axis_mapping_twin_throttle() {
        assert!(lookup_aircraft_axis("A-10C", "throttle_left").is_some());
        assert!(lookup_aircraft_axis("A-10C", "throttle_right").is_some());
        assert!(lookup_aircraft_axis("A-10C_2", "pitch").is_some());
    }

    #[test]
    fn unknown_aircraft_returns_none() {
        assert!(lookup_aircraft_axis("Unknown_Mod", "pitch").is_none());
    }

    #[test]
    fn unknown_axis_returns_none() {
        assert!(lookup_aircraft_axis("FA-18C", "nonexistent").is_none());
    }

    // --- Clickable cockpit control ---

    #[test]
    fn clickable_press_release() {
        use flight_dcs_export::Clickable;

        let sw = Clickable {
            label: "Master Arm",
            device_id: 12,
            button: 3200,
            min_value: 0.0,
            max_value: 1.0,
        };

        let press = sw.press();
        assert_eq!(press.device_id, 12);
        assert_eq!(press.command_id, 3200);
        assert!((press.value - 1.0).abs() < f64::EPSILON);

        let release = sw.release();
        assert!(release.value.abs() < f64::EPSILON);
    }

    #[test]
    fn clickable_clamps_value() {
        use flight_dcs_export::Clickable;

        let knob = Clickable {
            label: "Volume",
            device_id: 5,
            button: 100,
            min_value: 0.0,
            max_value: 1.0,
        };

        let cmd = knob.command(1.5);
        assert!((cmd.value - 1.0).abs() < f64::EPSILON);

        let cmd2 = knob.command(-0.5);
        assert!(cmd2.value.abs() < f64::EPSILON);
    }
}

// ============================================================================
// 5. Telemetry conversion depth
// ============================================================================

mod telemetry_conversion {
    use super::*;

    // --- NaN/Inf filtering via parse_indicator_value ---

    #[test]
    fn nan_detected_from_lua_literals() {
        assert!(parse_indicator_value("nan").unwrap().is_nan());
        assert!(parse_indicator_value("0/0").unwrap().is_nan());
    }

    #[test]
    fn inf_detected_from_lua_literals() {
        assert!(parse_indicator_value("inf").unwrap().is_infinite());
        assert!(parse_indicator_value("-inf").unwrap().is_infinite());
        assert!(parse_indicator_value("1/0").unwrap().is_infinite());
        assert!(parse_indicator_value("-1/0").unwrap().is_sign_negative());
    }

    // --- Unit conversion: DCS native → OpenFlight standard ---

    #[test]
    fn metres_to_feet_round_trip() {
        let m = 10000.0;
        let ft = m_to_ft(m);
        assert!((ft - 32808.4).abs() < 0.1);
    }

    #[test]
    fn ms_to_knots_known_value() {
        // 1 m/s ≈ 1.94384 knots
        let kts = ms_to_knots(1.0);
        assert!((kts - 1.943844).abs() < 1e-5);
    }

    #[test]
    fn coordinate_conversion_all_quadrants() {
        // NE quadrant: x=north, z=east, y=up
        let (n, e, d) = dcs_to_ned(1000.0, 500.0, 2000.0);
        assert!((n - 1000.0).abs() < f64::EPSILON);
        assert!((e - 2000.0).abs() < f64::EPSILON);
        assert!((d - (-500.0)).abs() < f64::EPSILON);

        // Negative values
        let (n, e, d) = dcs_to_ned(-1000.0, -500.0, -2000.0);
        assert!((n - (-1000.0)).abs() < f64::EPSILON);
        assert!((e - (-2000.0)).abs() < f64::EPSILON);
        assert!((d - 500.0).abs() < f64::EPSILON);
    }

    // --- Missing telemetry fields → safe defaults ---

    #[test]
    fn default_flight_data_has_safe_values() {
        let fd = DcsFlightData::default();
        assert!((fd.altitude_m).abs() < f64::EPSILON);
        assert!((fd.airspeed_ms).abs() < f64::EPSILON);
        assert!((fd.heading_deg).abs() < f64::EPSILON);
        assert!((fd.pitch_deg).abs() < f64::EPSILON);
        assert!((fd.roll_deg).abs() < f64::EPSILON);
        assert!((fd.aoa_deg).abs() < f64::EPSILON);
        // g_load defaults to 1.0 (1G = sitting on the ground)
        assert!((fd.g_load - 1.0).abs() < f64::EPSILON);
        assert!((fd.mach).abs() < f64::EPSILON);
        assert!((fd.vertical_speed_ms).abs() < f64::EPSILON);
        assert!(fd.engine_rpm_percent.is_empty());
        assert!((fd.fuel_total_kg).abs() < f64::EPSILON);
        assert!(fd.gear_position.is_empty());
    }

    #[test]
    fn batch_with_only_header_returns_defaults() {
        let data = "HEADER:timestamp=0.0,model_time=0.0,aircraft=T\n";
        let pkt = parse_telemetry_batch(data).unwrap();
        assert!((pkt.flight_data.altitude_m).abs() < f64::EPSILON);
        assert!((pkt.flight_data.g_load - 1.0).abs() < f64::EPSILON);
    }

    // --- Partial telemetry populates only provided fields ---

    #[test]
    fn partial_telemetry_mixed_with_defaults() {
        let batch = [
            "HEADER:timestamp=1.0,model_time=1.0,aircraft=F-16C_50",
            "altitude_m=5000.0",
            "mach=0.9",
        ]
        .join("\n");
        let pkt = parse_telemetry_batch(&batch).unwrap();
        assert!((pkt.flight_data.altitude_m - 5000.0).abs() < f64::EPSILON);
        assert!((pkt.flight_data.mach - 0.9).abs() < 1e-10);
        // Unprovided fields are default
        assert!((pkt.flight_data.heading_deg).abs() < f64::EPSILON);
        assert!((pkt.flight_data.g_load - 1.0).abs() < f64::EPSILON);
    }
}

// ============================================================================
// 6. Integration scenarios
// ============================================================================

mod integration_scenarios {
    use super::*;

    /// Full pipeline: parse raw UDP-style text → telemetry packet → verify fields.
    #[test]
    fn full_pipeline_parse_and_verify() {
        // Simulate a raw UDP packet as DCS Export.lua would send it
        let raw = [
            "HEADER:timestamp=12345.678,model_time=1200.5,aircraft=FA-18C_hornet",
            "altitude_m=10000.0",
            "airspeed_ms=300.0",
            "heading_deg=90.0",
            "pitch_deg=5.0",
            "roll_deg=-3.0",
            "aoa_deg=6.5",
            "g_load=1.8",
            "mach=0.92",
            "vertical_speed_ms=-2.0",
            "fuel_total_kg=4200.0",
            "engine_rpm_l=95.0",
            "engine_rpm_r=94.5",
            "gear_nose=0.0",
            "gear_left=0.0",
            "gear_right=0.0",
            "vsi_gauge=0.5",
        ]
        .join("\n");

        let pkt = parse_telemetry_batch(&raw).unwrap();

        // Verify header
        assert!((pkt.timestamp - 12345.678).abs() < 1e-3);
        assert!((pkt.model_time - 1200.5).abs() < 1e-3);
        assert_eq!(pkt.aircraft_name, "FA-18C_hornet");

        // Verify flight data
        assert!((pkt.flight_data.altitude_m - 10000.0).abs() < f64::EPSILON);
        assert!((pkt.flight_data.airspeed_ms - 300.0).abs() < f64::EPSILON);
        assert!((pkt.flight_data.heading_deg - 90.0).abs() < f64::EPSILON);
        assert!((pkt.flight_data.aoa_deg - 6.5).abs() < f64::EPSILON);
        assert!((pkt.flight_data.g_load - 1.8).abs() < f64::EPSILON);
        assert!((pkt.flight_data.mach - 0.92).abs() < 1e-10);

        // Verify aircraft DB lookup
        let aircraft = aircraft_db::lookup(&pkt.aircraft_name).unwrap();
        assert_eq!(aircraft.category, AircraftCategory::FixedWing);
        assert!(aircraft.has_ffb_profile);

        // Verify indicators (non-flight-data fields)
        assert!(pkt.indicators.contains_key("vsi_gauge"));
    }

    /// Simulate reconnect: state machine goes through error → recovery cycle.
    #[test]
    fn reconnect_after_timeout_scenario() {
        let mut sm = DcsAdapterStateMachine::new(5000, 5);

        // Initial connection
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Active);

        // Connection drops
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Stale);
        sm.transition(DcsAdapterEvent::StaleExhausted).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Disconnected);

        // Reconnect
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Active);
    }

    /// DCS not running: adapter stays in Disconnected, ready to reconnect.
    #[test]
    fn graceful_degradation_dcs_not_running() {
        let sm = DcsAdapterStateMachine::new(2000, 3);
        assert_eq!(sm.state(), DcsAdapterState::Disconnected);
        assert!(sm.should_reconnect());
        assert!(sm.is_recoverable());
    }

    /// Multiple aircraft switches within a session (no restart needed).
    #[test]
    fn aircraft_switch_during_session() {
        let aircraft = ["F-16C_50", "FA-18C_hornet", "A-10C_2", "Ka-50_3"];

        for name in &aircraft {
            let batch = format!(
                "HEADER:timestamp=1.0,model_time=1.0,aircraft={name}\naltitude_m=5000.0"
            );
            let pkt = parse_telemetry_batch(&batch).unwrap();
            assert_eq!(pkt.aircraft_name, *name);
            // DB lookup succeeds for all known aircraft
            assert!(
                aircraft_db::lookup(name).is_some(),
                "DB missing {name}"
            );
        }
    }

    /// Inject commands for multiple frames, verifying independent flush cycles.
    #[test]
    fn multi_frame_command_injection() {
        let mut inj = DcsControlInjector::new(16);

        // Frame 1: pitch + roll
        inj.set_axis("pitch", 0.3);
        inj.set_axis("roll", -0.2);
        let p1 = String::from_utf8(inj.flush()).unwrap();
        assert!(p1.contains("CMD:"));
        assert_eq!(inj.pending_count(), 0);

        // Frame 2: throttle + button
        inj.set_axis("throttle", 0.8);
        inj.press_button(4, 3001);
        let p2 = String::from_utf8(inj.flush()).unwrap();
        assert!(p2.contains("CMD:"));
        assert!(p2.contains("BTN:"));

        // Frame 3: empty (no commands queued)
        let p3 = inj.flush();
        assert!(p3.is_empty());
    }

    /// Full control pipeline: build command → serialize → parse back.
    #[test]
    fn control_roundtrip_pipeline() {
        let mut inj = DcsControlInjector::new(16);
        inj.queue_command(DcsControlCommand::axis(0, 2001, 0.75));
        inj.queue_command(DcsControlCommand::button_press(4, 3001));
        inj.queue_command(DcsControlCommand::toggle(2, 500));

        let payload = String::from_utf8(inj.flush()).unwrap();
        let results = parse_wire_payload(&payload);

        assert_eq!(results.len(), 3);
        let cmds: Vec<_> = results.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(cmds[0].action_type, DcsActionType::Axis);
        assert_eq!(cmds[1].action_type, DcsActionType::ButtonPress);
        assert_eq!(cmds[2].action_type, DcsActionType::Toggle);
    }
}
