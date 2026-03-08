// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the DCS export adapter.
//!
//! Covers six areas with 30+ tests:
//!   1. Export.lua protocol parsing (Lua table format, types, escapes, boundaries)
//!   2. Telemetry extraction (position, attitude, airspeed, engines, gear, surfaces)
//!   3. State machine (lifecycle transitions, timeout, reconnect, multi-instance)
//!   4. Aircraft detection (name matching, module ID, flyable vs AI, variants)
//!   5. Input injection (button press, axis set, switch toggle, keyboard cmd, batch)
//!   6. Integration (full pipeline, snapshot format, field mapping)

use flight_dcs_export::{
    AircraftCategory, AxesProfile, CockpitSeat, DcsAdapterEvent, DcsAdapterState,
    DcsAdapterStateMachine, DcsTransitionError, ModuleFidelity,
    detect_aircraft, detect_axes_profile, detect_category,
};
use flight_dcs_export::protocol::{
    DcsFlightData, parse_device_arg_block, parse_export_line, parse_indicator_value,
    parse_instrument_block, parse_multi_value, parse_position_data, parse_telemetry_batch,
    parse_aircraft_type, dcs_to_ned, m_to_ft, ms_to_knots, rad_to_deg,
};
use flight_dcs_export::control_injection::{
    DcsActionType, DcsControlCommand, DcsControlInjector, Clickable,
    lookup_aircraft_axis, parse_wire_command, parse_wire_payload,
    fa18c, f16c, a10c, f14b, ah64d,
};
use flight_dcs_export::export_lua::{ExportLuaConfig, ExportLuaGenerator, DcsVariant};

// ============================================================================
// 1. Export.lua protocol parsing (8 tests)
// ============================================================================

/// Lua table format: `{key=val,key=val}` parsed as position data.
#[test]
fn proto_lua_table_format_curly_braces() {
    let (lat, lon, alt) = parse_position_data("{lat=42.36,lon=-71.06,alt=10.0}").unwrap();
    assert!((lat - 42.36).abs() < 1e-10);
    assert!((lon - (-71.06)).abs() < 1e-10);
    assert!((alt - 10.0).abs() < 1e-10);
}

/// Nested table values: multi-value semicolon-separated fields.
#[test]
fn proto_nested_table_multi_values() {
    let vals = parse_multi_value("95.0;94.5;93.0").unwrap();
    assert_eq!(vals.len(), 3);
    assert!((vals[0] - 95.0).abs() < 1e-10);
    assert!((vals[1] - 94.5).abs() < 1e-10);
    assert!((vals[2] - 93.0).abs() < 1e-10);
}

/// String, number, and boolean-like types in key=value lines.
#[test]
fn proto_string_number_types() {
    // Numeric value
    let entry = parse_export_line("altitude_m=5000.5").unwrap();
    assert_eq!(entry.key, "altitude_m");
    let val = parse_indicator_value(&entry.value).unwrap();
    assert!((val - 5000.5).abs() < 1e-10);

    // String value (non-numeric)
    let entry = parse_export_line("aircraft=F-16C_50").unwrap();
    assert_eq!(entry.value, "F-16C_50");

    // Lua special numeric: empty → 0.0
    assert!((parse_indicator_value("").unwrap()).abs() < f64::EPSILON);
    // Bare dash → 0.0
    assert!((parse_indicator_value("-").unwrap()).abs() < f64::EPSILON);
}

/// Array values via semicolon separator and device arg blocks.
#[test]
fn proto_array_values() {
    let block = "ARGS_BEGIN\n0:71:0.5\n0:85:1.0\n3:200:0.25\nARGS_END\n";
    let entries = parse_device_arg_block(block).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[2].device_id, 3);
    assert_eq!(entries[2].arg_number, 200);
    assert!((entries[2].value - 0.25).abs() < 1e-6);
}

/// Lua escape-like sequences: `--` comment stripping, Lua special values.
#[test]
fn proto_escape_sequences_and_comments() {
    // Comment stripping
    let entry = parse_export_line("mach=0.85 -- transonic region").unwrap();
    assert_eq!(entry.key, "mach");
    assert_eq!(entry.value, "0.85");

    // Lua inf/nan special values
    assert!(parse_indicator_value("inf").unwrap().is_infinite());
    assert!(parse_indicator_value("nan").unwrap().is_nan());
    assert!(parse_indicator_value("1/0").unwrap().is_infinite());
    assert!(parse_indicator_value("0/0").unwrap().is_nan());
    let neg_inf = parse_indicator_value("-1/0").unwrap();
    assert!(neg_inf.is_infinite() && neg_inf.is_sign_negative());
}

/// Malformed input rejection: various invalid formats.
#[test]
fn proto_malformed_input_rejection() {
    // Empty line
    assert!(parse_export_line("").is_err());
    // No separator
    assert!(parse_export_line("no_separator").is_err());
    // Empty key
    assert!(parse_export_line("=value").is_err());
    // Comment-only line
    assert!(parse_export_line("-- just a comment").is_err());
    // Invalid numeric
    assert!(parse_indicator_value("not_a_number").is_err());
    // Invalid multi-value
    assert!(parse_multi_value("1.0;abc;3.0").is_err());
    // Position data missing field
    assert!(parse_position_data("{lat=10.0,lon=20.0}").is_err());
    // Device arg: too few parts
    assert!(parse_export_line("only_key_no_equals").is_err());
    // Telemetry batch: no header
    assert!(parse_telemetry_batch("altitude_m=5000").is_err());
    // Telemetry batch: invalid header
    assert!(parse_telemetry_batch("NOT_A_HEADER:foo").is_err());
}

/// Packet boundaries: header + body separation, blank line handling.
#[test]
fn proto_packet_boundaries() {
    // Batch with blank lines interspersed
    let data =
        "HEADER:timestamp=1.0,model_time=2.0,aircraft=F-16C\n\naltitude_m=3000\n\nmach=0.7\n";
    let pkt = parse_telemetry_batch(data).unwrap();
    assert!((pkt.flight_data.altitude_m - 3000.0).abs() < f64::EPSILON);
    assert!((pkt.flight_data.mach - 0.7).abs() < 1e-10);

    // Minimal batch: header only, empty body
    let data = "HEADER:timestamp=0.0,model_time=0.0,aircraft=X\n";
    let pkt = parse_telemetry_batch(data).unwrap();
    assert_eq!(pkt.aircraft_name, "X");
    assert!(pkt.indicators.is_empty());
}

/// Multiline values: instrument block parsing across lines.
#[test]
fn proto_multiline_instrument_block() {
    let block = "\
        some preamble\n\
        INSTRUMENTS_BEGIN\n\
        AltimeterPressure=29.92\n\
        ADI_Pitch=5.3\n\
        HSI_Heading=270.0\n\
        INSTRUMENTS_END\n\
        trailing junk\n";
    let readings = parse_instrument_block(block).unwrap();
    assert_eq!(readings.len(), 3);
    assert_eq!(readings[0].name, "AltimeterPressure");
    assert!((readings[0].value - 29.92).abs() < 1e-6);
    assert_eq!(readings[2].name, "HSI_Heading");
    assert!((readings[2].value - 270.0).abs() < 1e-6);
}

// ============================================================================
// 2. Telemetry extraction (6 tests)
// ============================================================================

/// Position: lat/lon/alt extraction from Lua table format.
#[test]
fn telem_position_lat_lon_alt() {
    // Southern hemisphere, negative altitude (Dead Sea!)
    let (lat, lon, alt) = parse_position_data("{lat=-31.95,lon=115.86,alt=-400.0}").unwrap();
    assert!((lat - (-31.95)).abs() < 1e-10);
    assert!((lon - 115.86).abs() < 1e-10);
    assert!((alt - (-400.0)).abs() < 1e-10);

    // Coordinate conversion: DCS → NED
    let (n, e, d) = dcs_to_ned(1000.0, 500.0, 2000.0);
    assert!((n - 1000.0).abs() < f64::EPSILON);
    assert!((e - 2000.0).abs() < f64::EPSILON);
    assert!((d - (-500.0)).abs() < f64::EPSILON);
}

/// Attitude: pitch/roll/yaw from telemetry batch.
#[test]
fn telem_attitude_pitch_roll_yaw() {
    let data = [
        "HEADER:timestamp=100.0,model_time=50.0,aircraft=FA-18C_hornet",
        "pitch_deg=12.5",
        "roll_deg=-30.0",
        "heading_deg=270.0",
        "aoa_deg=8.2",
    ]
    .join("\n");
    let pkt = parse_telemetry_batch(&data).unwrap();
    assert!((pkt.flight_data.pitch_deg - 12.5).abs() < f64::EPSILON);
    assert!((pkt.flight_data.roll_deg - (-30.0)).abs() < f64::EPSILON);
    assert!((pkt.flight_data.heading_deg - 270.0).abs() < f64::EPSILON);
    assert!((pkt.flight_data.aoa_deg - 8.2).abs() < 1e-10);
}

/// Airspeed and Mach extraction with unit conversions.
#[test]
fn telem_airspeed_and_mach() {
    let data = [
        "HEADER:timestamp=1.0,model_time=1.0,aircraft=F-16C_50",
        "airspeed_ms=250.0",
        "mach=0.85",
        "vertical_speed_ms=-2.5",
    ]
    .join("\n");
    let pkt = parse_telemetry_batch(&data).unwrap();
    assert!((pkt.flight_data.airspeed_ms - 250.0).abs() < f64::EPSILON);
    assert!((pkt.flight_data.mach - 0.85).abs() < 1e-10);
    assert!((pkt.flight_data.vertical_speed_ms - (-2.5)).abs() < f64::EPSILON);

    // Unit conversions
    assert!((ms_to_knots(250.0) - 485.961).abs() < 0.1);
    assert!((m_to_ft(5000.0) - 16404.2).abs() < 1.0);
    assert!((rad_to_deg(std::f64::consts::FRAC_PI_2) - 90.0).abs() < 1e-10);
}

/// Engine data: multiple engines via prefixed keys.
#[test]
fn telem_engine_data() {
    let data = [
        "HEADER:timestamp=1.0,model_time=1.0,aircraft=F-15C",
        "engine_rpm_left=95.0",
        "engine_rpm_right=94.5",
    ]
    .join("\n");
    let pkt = parse_telemetry_batch(&data).unwrap();
    assert_eq!(pkt.flight_data.engine_rpm_percent.len(), 2);
    // Sorted output
    assert!((pkt.flight_data.engine_rpm_percent[0] - 94.5).abs() < 1e-10);
    assert!((pkt.flight_data.engine_rpm_percent[1] - 95.0).abs() < 1e-10);

    // Multi-value alternative
    let vals = parse_multi_value("87.0;88.5;86.0").unwrap();
    assert_eq!(vals.len(), 3);
}

/// Landing gear: multiple gear legs via prefixed keys.
#[test]
fn telem_landing_gear() {
    let data = [
        "HEADER:timestamp=1.0,model_time=1.0,aircraft=A-10C",
        "gear_nose=1.0",
        "gear_left=1.0",
        "gear_right=0.5",
    ]
    .join("\n");
    let pkt = parse_telemetry_batch(&data).unwrap();
    assert_eq!(pkt.flight_data.gear_position.len(), 3);
    // Sorted: 0.5, 1.0, 1.0
    assert!((pkt.flight_data.gear_position[0] - 0.5).abs() < 1e-10);
    assert!((pkt.flight_data.gear_position[1] - 1.0).abs() < 1e-10);
    assert!((pkt.flight_data.gear_position[2] - 1.0).abs() < 1e-10);
}

/// Control surfaces: fuel, g-load, and custom indicators.
#[test]
fn telem_fuel_gload_indicators() {
    let data = [
        "HEADER:timestamp=1.0,model_time=1.0,aircraft=FA-18C_hornet",
        "fuel_total_kg=4500.0",
        "g_load=3.2",
        "flap_position=0.75",
        "speedbrake=0.50",
    ]
    .join("\n");
    let pkt = parse_telemetry_batch(&data).unwrap();
    assert!((pkt.flight_data.fuel_total_kg - 4500.0).abs() < f64::EPSILON);
    assert!((pkt.flight_data.g_load - 3.2).abs() < 1e-10);
    // Non-flight-data keys go to indicators
    assert!((pkt.indicators["flap_position"] - 0.75).abs() < 1e-10);
    assert!((pkt.indicators["speedbrake"] - 0.50).abs() < 1e-10);
}

// ============================================================================
// 3. State machine (5 tests)
// ============================================================================

/// Full happy path: Disconnected → Connecting → Listening → Connected → Active.
#[test]
fn sm_full_lifecycle_happy_path() {
    let mut sm = DcsAdapterStateMachine::new(2000, 3);
    assert_eq!(sm.state(), DcsAdapterState::Disconnected);

    assert_eq!(
        sm.transition(DcsAdapterEvent::SocketBound).unwrap(),
        DcsAdapterState::Connecting
    );
    assert_eq!(
        sm.transition(DcsAdapterEvent::ListeningStarted).unwrap(),
        DcsAdapterState::Listening
    );
    assert_eq!(
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap(),
        DcsAdapterState::Connected
    );
    assert!(sm.is_healthy());

    assert_eq!(
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap(),
        DcsAdapterState::Active
    );
    assert!(sm.is_healthy());
    assert_eq!(sm.error_count(), 0);
}

/// Timeout detection: Active → Stale → (recovery or exhaustion).
#[test]
fn sm_timeout_detection_and_recovery() {
    let mut sm = DcsAdapterStateMachine::new(2000, 3).with_max_stale(3);

    // Bring to Active
    sm.transition(DcsAdapterEvent::SocketBound).unwrap();
    sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
    sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
    assert_eq!(sm.state(), DcsAdapterState::Active);

    // Go stale
    sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
    assert_eq!(sm.state(), DcsAdapterState::Stale);
    assert_eq!(sm.consecutive_stale_count(), 1);
    assert!(!sm.is_healthy());

    // Recovery: telemetry resumes
    sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
    assert_eq!(sm.state(), DcsAdapterState::Active);
    assert_eq!(sm.consecutive_stale_count(), 0);
    assert_eq!(sm.error_count(), 0);
}

/// Reconnect after error: Error → Connecting with exponential backoff.
#[test]
fn sm_reconnect_after_error_with_backoff() {
    let mut sm = DcsAdapterStateMachine::new(2000, 5);
    let initial_delay = sm.reconnect_delay();

    // First error
    sm.transition(DcsAdapterEvent::ConnectionError("err1".into()))
        .unwrap();
    assert_eq!(sm.state(), DcsAdapterState::Error);
    assert_eq!(sm.error_count(), 1);
    assert!(sm.should_reconnect());
    let delay_after_1 = sm.reconnect_delay();
    assert!(delay_after_1 >= initial_delay);

    // Recovery attempt
    sm.transition(DcsAdapterEvent::SocketBound).unwrap();
    assert_eq!(sm.state(), DcsAdapterState::Connecting);

    // Second error — delay should increase
    sm.transition(DcsAdapterEvent::ConnectionError("err2".into()))
        .unwrap();
    assert_eq!(sm.error_count(), 2);
    let delay_after_2 = sm.reconnect_delay();
    assert!(delay_after_2 >= delay_after_1);
}

/// Retries exhausted: transitions are refused after max retries.
#[test]
fn sm_retries_exhausted() {
    let mut sm = DcsAdapterStateMachine::new(2000, 2);

    // Error #1
    sm.transition(DcsAdapterEvent::ConnectionError("e1".into()))
        .unwrap();
    // Recover
    sm.transition(DcsAdapterEvent::SocketBound).unwrap();
    // Error #2
    sm.transition(DcsAdapterEvent::ConnectionError("e2".into()))
        .unwrap();
    assert_eq!(sm.error_count(), 2);
    assert!(!sm.is_recoverable());
    assert!(!sm.should_reconnect());

    // Should refuse reconnection
    let result = sm.transition(DcsAdapterEvent::SocketBound);
    assert!(matches!(
        result,
        Err(DcsTransitionError::RetriesExhausted { max_retries: 2 })
    ));
}

/// Stale exhaustion → Disconnected, then reset brings it back.
#[test]
fn sm_stale_exhaustion_and_reset() {
    let mut sm = DcsAdapterStateMachine::new(2000, 3).with_max_stale(2);

    // Bring to active via UDP shortcut (Listening → Active)
    sm.transition(DcsAdapterEvent::SocketBound).unwrap();
    sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
    sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
    assert_eq!(sm.state(), DcsAdapterState::Active);

    // Go stale repeatedly
    sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
    assert_eq!(sm.consecutive_stale_count(), 1);
    sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
    assert_eq!(sm.consecutive_stale_count(), 2);
    assert!(sm.is_stale_exhausted());

    // Stale exhausted → Disconnected
    sm.transition(DcsAdapterEvent::StaleExhausted).unwrap();
    assert_eq!(sm.state(), DcsAdapterState::Disconnected);

    // Reset clears everything
    sm.reset();
    assert_eq!(sm.state(), DcsAdapterState::Disconnected);
    assert_eq!(sm.error_count(), 0);
    assert_eq!(sm.consecutive_stale_count(), 0);
    assert!(sm.should_reconnect());
}

// ============================================================================
// 4. Aircraft detection (5 tests)
// ============================================================================

/// DCS aircraft name matching: exact module name → db lookup.
#[test]
fn aircraft_name_matching() {
    let det = detect_aircraft("FA-18C_hornet");
    assert_eq!(det.raw_name, "FA-18C_hornet");
    assert_eq!(det.base_name, "FA-18C_hornet");
    assert!(det.db_info.is_some());
    assert_eq!(det.db_info.unwrap().display_name, "F/A-18C Hornet");
    assert_eq!(det.fidelity, ModuleFidelity::FullFidelity);
}

/// Module identification: known vs unknown modules.
#[test]
fn aircraft_module_identification() {
    // Full fidelity
    let det = detect_aircraft("F-16C_50");
    assert_eq!(det.fidelity, ModuleFidelity::FullFidelity);
    assert!(det.db_info.is_some());

    // FC3 level
    let det = detect_aircraft("Su-25T");
    assert_eq!(det.fidelity, ModuleFidelity::Fc3);

    // Unknown community mod
    let det = detect_aircraft("MySuperMod_v42");
    assert_eq!(det.fidelity, ModuleFidelity::Mod);
    assert!(det.db_info.is_none());
    assert!(!det.multi_crew);
    assert_eq!(det.seat, CockpitSeat::Single);
}

/// Flyable vs AI: FC3 modules are detected, unknown treated as community mods.
#[test]
fn aircraft_flyable_vs_ai_detection() {
    // All FC3 modules should be recognized
    for name in &["Su-25T", "Su-27", "Su-33", "MiG-29A", "MiG-29S", "F-15C", "J-11A", "Su-25"]
    {
        let det = detect_aircraft(name);
        assert_eq!(
            det.fidelity,
            ModuleFidelity::Fc3,
            "{name} should be FC3"
        );
    }

    // AI aircraft (not in DB) → Mod fidelity
    let det = detect_aircraft("KC-135_AI");
    assert_eq!(det.fidelity, ModuleFidelity::Mod);
    assert!(det.db_info.is_none());
}

/// Variant detection: seat suffixes, multi-crew variants.
#[test]
fn aircraft_variant_detection() {
    // F-14B pilot (front seat)
    let det = detect_aircraft("F-14B");
    assert_eq!(det.seat, CockpitSeat::Front);
    assert!(det.multi_crew);

    // F-14B RIO (rear seat)
    let det = detect_aircraft("F-14B_RIO");
    assert_eq!(det.base_name, "F-14B");
    assert_eq!(det.seat, CockpitSeat::Rear);
    assert!(det.multi_crew);
    assert!(det.db_info.is_some());

    // Apache CPG (rear seat)
    let det = detect_aircraft("AH-64D_BLK_II_CPG");
    assert_eq!(det.base_name, "AH-64D_BLK_II");
    assert_eq!(det.seat, CockpitSeat::Rear);
    assert_eq!(det.db_info.unwrap().category, AircraftCategory::Helicopter);

    // F-15E WSO
    let det = detect_aircraft("F-15ESE_WSO");
    assert_eq!(det.base_name, "F-15ESE");
    assert_eq!(det.seat, CockpitSeat::Rear);

    // Case-insensitive suffix
    let det = detect_aircraft("F-14B_rio");
    assert_eq!(det.seat, CockpitSeat::Rear);
}

/// Axes profile and category convenience functions.
#[test]
fn aircraft_axes_profile_and_category() {
    assert_eq!(
        detect_axes_profile("AH-64D_BLK_II"),
        AxesProfile::HelicopterCollective
    );
    assert_eq!(detect_axes_profile("TF-51D"), AxesProfile::Warbird4Axis);
    assert_eq!(detect_axes_profile("F-16C_50"), AxesProfile::StandardJet);
    assert_eq!(
        detect_axes_profile("UnknownMod"),
        AxesProfile::StandardJet
    ); // fallback

    assert_eq!(
        detect_category("FA-18C_hornet"),
        Some(AircraftCategory::FixedWing)
    );
    assert_eq!(
        detect_category("AH-64D_BLK_II"),
        Some(AircraftCategory::Helicopter)
    );
    assert_eq!(
        detect_category("TF-51D"),
        Some(AircraftCategory::WarBird)
    );
    assert_eq!(detect_category("UnknownMod"), None);

    // Aircraft type parsing strips pilot/copilot suffixes
    assert_eq!(parse_aircraft_type("AH-64D_pilot"), "AH-64D");
    assert_eq!(parse_aircraft_type("AH-64D_copilot"), "AH-64D");
    assert_eq!(parse_aircraft_type("F-16C_50"), "F-16C_50");
}

// ============================================================================
// 5. Input injection (5 tests)
// ============================================================================

/// Cockpit button press: wire format and round-trip.
#[test]
fn inject_cockpit_button_press() {
    let cmd = DcsControlCommand::button_press(25, 3001);
    assert_eq!(cmd.action_type, DcsActionType::ButtonPress);
    assert!((cmd.value - 1.0).abs() < f64::EPSILON);
    assert_eq!(cmd.to_wire(), "BTN:25,3001,1.000000");

    // Round-trip
    let parsed = parse_wire_command(&cmd.to_wire()).unwrap();
    assert_eq!(parsed.device_id, 25);
    assert_eq!(parsed.command_id, 3001);
    assert_eq!(parsed.action_type, DcsActionType::ButtonPress);
}

/// Axis value set: clamping, precision, and named axis lookup.
#[test]
fn inject_axis_value_set() {
    // Clamping
    let cmd = DcsControlCommand::axis(0, 2001, 5.0);
    assert!((cmd.value - 1.0).abs() < f64::EPSILON);
    let cmd = DcsControlCommand::axis(0, 2001, -5.0);
    assert!((cmd.value - (-1.0)).abs() < f64::EPSILON);

    // Precision
    let cmd = DcsControlCommand::axis(0, 2001, 0.123456);
    assert!((cmd.value - 0.123456).abs() < 1e-10);
    assert!(cmd.to_wire().contains("0.123456"));

    // Named axis lookup
    let mut inj = DcsControlInjector::new(16);
    assert!(inj.set_axis("pitch", 0.5));
    assert!(inj.set_axis("roll", -0.3));
    assert!(inj.set_axis("throttle", 0.8));
    assert!(!inj.set_axis("nonexistent", 0.0));
    assert_eq!(inj.pending_count(), 3);

    let text = String::from_utf8(inj.flush()).unwrap();
    assert!(text.contains("CMD:0,2001,0.500000")); // pitch
    assert!(text.contains("CMD:0,2002,-0.300000")); // roll
    assert!(text.contains("CMD:0,2004,0.800000")); // throttle
}

/// Switch toggle: Clickable abstraction for cockpit switches.
#[test]
fn inject_switch_toggle() {
    let master_arm = Clickable {
        label: "Master Arm",
        device_id: 12,
        button: 3200,
        min_value: 0.0,
        max_value: 1.0,
    };

    // Press → value = max
    let press = master_arm.press();
    assert!((press.value - 1.0).abs() < f64::EPSILON);
    assert_eq!(press.device_id, 12);
    assert_eq!(press.command_id, 3200);

    // Release → value = min
    let release = master_arm.release();
    assert!(release.value.abs() < f64::EPSILON);

    // 3-position switch (min=-1, max=1)
    let three_pos = Clickable {
        label: "Fuel Selector",
        device_id: 6,
        button: 400,
        min_value: -1.0,
        max_value: 1.0,
    };
    let center = three_pos.command(0.0);
    assert!(center.value.abs() < f64::EPSILON);
    assert_eq!(center.action_type, DcsActionType::Axis);

    // Clamping
    let clamped = three_pos.command(5.0);
    assert!((clamped.value - 1.0).abs() < f64::EPSILON);
}

/// Keyboard/module-specific command lookup across aircraft modules.
#[test]
fn inject_module_specific_commands() {
    // F/A-18C UFC button
    let ufc1 = fa18c::lookup_command("UFC_1").unwrap();
    assert_eq!(ufc1.device_id, 25);
    assert_eq!(ufc1.command_id, 3001);

    // F-16C ICP enter
    let icp_entr = f16c::lookup_command("ICP_ENTR").unwrap();
    assert_eq!(icp_entr.device_id, 17);
    assert_eq!(icp_entr.command_id, 3011);

    // A-10C CDU
    let cdu5 = a10c::lookup_command("CDU_5").unwrap();
    assert_eq!(cdu5.device_id, 24);

    // F-14B RIO
    let rio = f14b::lookup_command("RIO_CAP_LAUNCH").unwrap();
    assert_eq!(rio.device_id, 42);

    // AH-64D pilot vs CPG have different device IDs, same command IDs
    let plt = ah64d::lookup_command("PLT_KU_ENT").unwrap();
    let cpg = ah64d::lookup_command("CPG_KU_ENT").unwrap();
    assert_ne!(plt.device_id, cpg.device_id);
    assert_eq!(plt.command_id, cpg.command_id);

    // Per-aircraft axis mapping
    let fa18_pitch = lookup_aircraft_axis("FA-18C", "pitch").unwrap();
    assert_eq!(fa18_pitch.command_id, 2001);
    let f16_throttle = lookup_aircraft_axis("F-16C", "throttle").unwrap();
    assert_eq!(f16_throttle.command_id, 2004);
    assert!(lookup_aircraft_axis("MiG-29", "pitch").is_none());
}

/// Multi-command batch: buffer, flush, and payload format.
#[test]
fn inject_multi_command_batch() {
    let mut inj = DcsControlInjector::new(8);

    // Queue multiple mixed commands
    assert!(inj.queue_command(DcsControlCommand::axis(0, 2001, 0.5)));
    assert!(inj.press_button(25, 3001));
    assert!(inj.queue_command(DcsControlCommand::toggle(2, 500)));
    assert!(inj.release_button(25, 3001));
    assert!(inj.set_axis("yaw", -0.2));
    assert_eq!(inj.pending_count(), 5);

    let payload = inj.flush();
    assert_eq!(inj.pending_count(), 0); // drained

    let text = String::from_utf8(payload.clone()).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 5);

    // Verify wire format round-trip
    let parsed = parse_wire_payload(&text);
    assert_eq!(parsed.len(), 5);
    assert!(parsed.iter().all(|r| r.is_ok()));

    // Buffer overflow: capacity=8, already flushed, now fill to cap
    for i in 0..8 {
        assert!(inj.queue_command(DcsControlCommand::axis(0, i, 0.0)));
    }
    assert!(!inj.queue_command(DcsControlCommand::axis(0, 99, 0.0))); // full
    assert_eq!(inj.pending_count(), 8);

    // Clear and refill
    inj.clear();
    assert_eq!(inj.pending_count(), 0);
    assert!(inj.queue_command(DcsControlCommand::axis(0, 1, 0.0)));
}

// ============================================================================
// 6. Integration (5 tests)
// ============================================================================

/// Full pipeline: UDP packet → parse → extract flight data.
#[test]
fn integ_full_parse_pipeline() {
    let raw_packet = [
        "HEADER:timestamp=12345.678,model_time=600.0,aircraft=FA-18C_hornet",
        "altitude_m=10000.0",
        "airspeed_ms=300.0",
        "heading_deg=180.0",
        "pitch_deg=-2.5",
        "roll_deg=15.0",
        "aoa_deg=8.0",
        "g_load=2.5",
        "mach=0.92",
        "vertical_speed_ms=-5.0",
        "fuel_total_kg=4500.0",
        "engine_rpm_left=95.0",
        "engine_rpm_right=94.5",
        "gear_nose=0.0",
        "gear_left=0.0",
        "gear_right=0.0",
        "hud_brightness=0.75",
    ]
    .join("\n");

    let pkt = parse_telemetry_batch(&raw_packet).unwrap();

    // Header
    assert!((pkt.timestamp - 12345.678).abs() < 1e-3);
    assert!((pkt.model_time - 600.0).abs() < f64::EPSILON);
    assert_eq!(pkt.aircraft_name, "FA-18C_hornet");

    // Flight data
    let fd = &pkt.flight_data;
    assert!((fd.altitude_m - 10000.0).abs() < f64::EPSILON);
    assert!((fd.airspeed_ms - 300.0).abs() < f64::EPSILON);
    assert!((fd.heading_deg - 180.0).abs() < f64::EPSILON);
    assert!((fd.pitch_deg - (-2.5)).abs() < f64::EPSILON);
    assert!((fd.roll_deg - 15.0).abs() < f64::EPSILON);
    assert!((fd.aoa_deg - 8.0).abs() < f64::EPSILON);
    assert!((fd.g_load - 2.5).abs() < f64::EPSILON);
    assert!((fd.mach - 0.92).abs() < 1e-10);
    assert!((fd.vertical_speed_ms - (-5.0)).abs() < f64::EPSILON);
    assert!((fd.fuel_total_kg - 4500.0).abs() < f64::EPSILON);
    assert_eq!(fd.engine_rpm_percent.len(), 2);
    assert_eq!(fd.gear_position.len(), 3);

    // Custom indicators
    assert!((pkt.indicators["hud_brightness"] - 0.75).abs() < 1e-10);

    // Aircraft detection on the parsed name
    let det = detect_aircraft(&pkt.aircraft_name);
    assert_eq!(det.db_info.unwrap().display_name, "F/A-18C Hornet");
    assert_eq!(det.fidelity, ModuleFidelity::FullFidelity);
}

/// Snapshot format: DcsFlightData default values are sensible.
#[test]
fn integ_snapshot_default_values() {
    let fd = DcsFlightData::default();
    assert!(fd.altitude_m.abs() < f64::EPSILON);
    assert!(fd.airspeed_ms.abs() < f64::EPSILON);
    assert!(fd.heading_deg.abs() < f64::EPSILON);
    assert!(fd.pitch_deg.abs() < f64::EPSILON);
    assert!(fd.roll_deg.abs() < f64::EPSILON);
    assert!(fd.aoa_deg.abs() < f64::EPSILON);
    assert!((fd.g_load - 1.0).abs() < f64::EPSILON); // 1g default
    assert!(fd.mach.abs() < f64::EPSILON);
    assert!(fd.vertical_speed_ms.abs() < f64::EPSILON);
    assert!(fd.fuel_total_kg.abs() < f64::EPSILON);
    assert!(fd.engine_rpm_percent.is_empty());
    assert!(fd.gear_position.is_empty());
}

/// Field mapping: keys that don't match well-known names go to indicators.
#[test]
fn integ_unknown_keys_to_indicators() {
    let data = [
        "HEADER:timestamp=1.0,model_time=1.0,aircraft=F-16C_50",
        "altitude_m=5000.0",
        "custom_gauge=0.42",
        "rwr_threat_count=3.0",
        "epu_fuel_pct=88.0",
    ]
    .join("\n");
    let pkt = parse_telemetry_batch(&data).unwrap();

    // Well-known key → flight data
    assert!((pkt.flight_data.altitude_m - 5000.0).abs() < f64::EPSILON);

    // Custom keys → indicators
    assert!((pkt.indicators["custom_gauge"] - 0.42).abs() < 1e-10);
    assert!((pkt.indicators["rwr_threat_count"] - 3.0).abs() < 1e-10);
    assert!((pkt.indicators["epu_fuel_pct"] - 88.0).abs() < 1e-10);

    // Well-known key NOT in indicators
    assert!(!pkt.indicators.contains_key("altitude_m"));
}

/// Wire format end-to-end: injector → flush → parse round-trip.
#[test]
fn integ_wire_format_roundtrip() {
    let mut inj = DcsControlInjector::new(16);
    inj.queue_command(DcsControlCommand::axis(0, 2001, 0.75));
    inj.queue_command(DcsControlCommand::button_press(25, 3001));
    inj.queue_command(DcsControlCommand::toggle(2, 500));
    inj.queue_command(DcsControlCommand::button_release(25, 3001));

    let payload = String::from_utf8(inj.flush()).unwrap();
    let commands = parse_wire_payload(&payload);

    assert_eq!(commands.len(), 4);
    let cmds: Vec<_> = commands.into_iter().map(|r| r.unwrap()).collect();

    assert_eq!(cmds[0].action_type, DcsActionType::Axis);
    assert!((cmds[0].value - 0.75).abs() < 1e-6);
    assert_eq!(cmds[1].action_type, DcsActionType::ButtonPress);
    assert_eq!(cmds[2].action_type, DcsActionType::Toggle);
    assert_eq!(cmds[3].action_type, DcsActionType::ButtonRelease);
}

/// Export.lua generator produces valid script with configured features.
#[test]
fn integ_export_lua_generation() {
    let config = ExportLuaConfig {
        socket_address: "127.0.0.1".to_string(),
        socket_port: 7778,
        update_interval: 0.05,
        enabled_features: vec![
            "telemetry_basic".to_string(),
            "telemetry_engines".to_string(),
        ],
        mp_safe_mode: true,
    };
    let generator = ExportLuaGenerator::new(config);
    let script = generator.generate_script();

    // Header present
    assert!(script.contains("Flight Hub DCS Export Script"));
    // Config values embedded
    assert!(script.contains("127.0.0.1"));
    assert!(script.contains("7778"));
    assert!(script.contains("mp_safe_mode = true"));
    // Feature flags
    assert!(script.contains("telemetry_basic"));
    assert!(script.contains("telemetry_engines"));
    // MP-blocked features annotated
    assert!(script.contains("MP-blocked"));

    // DCS variant detection
    assert_eq!(DcsVariant::Stable.as_str(), "DCS");
    assert_eq!(DcsVariant::OpenBeta.as_str(), "DCS.openbeta");
    assert_eq!(DcsVariant::OpenAlpha.as_str(), "DCS.openalpha");
}
