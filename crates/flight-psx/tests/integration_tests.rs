// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Integration tests for `flight-psx`.
//!
//! Exercises the PSX TCP text-protocol parser and adapter using hand-crafted
//! lines — no real TCP connection or PSX simulator is needed.

use flight_psx::{
    PSX_DEFAULT_PORT, PsxAdapter, PsxAdapterError, PsxTelemetry, PsxVariable, parse_psx_line,
};

// ── Tests ──────────────────────────────────────────────────────────────────────

/// A valid PSX TCP message is parsed into the expected variable and value.
#[test]
fn valid_psx_tcp_message_parsed() {
    let (var, val) = parse_psx_line("Qi0001=280.0").unwrap();
    assert_eq!(var, PsxVariable::FcuSpd);
    assert!((val - 280.0).abs() < 0.001, "val={val}");
}

/// The `<id>=<value>` key-value line format is parsed correctly for heading.
#[test]
fn key_value_line_format_fcu_hdg_parsed() {
    let (var, val) = parse_psx_line("Qi0002=360.0").unwrap();
    assert_eq!(var, PsxVariable::FcuHdg);
    assert!((val - 360.0).abs() < 0.001, "val={val}");
}

/// Processing a batch of lines populates all telemetry fields correctly.
#[test]
fn multiple_values_in_packet_all_parsed() {
    let lines = [
        ("Qi0001=250.0", PsxVariable::FcuSpd),
        ("Qi0002=90.0", PsxVariable::FcuHdg),
        ("Qi0010=85.5", PsxVariable::N1Left),
        ("Qi0011=86.0", PsxVariable::N1Right),
        ("Qi0100=42000.0", PsxVariable::FuelLeft),
        ("Qi0101=41000.0", PsxVariable::FuelRight),
        ("Qi0200=1", PsxVariable::GearDown),
    ];
    let mut adapter = PsxAdapter::new();
    for (line, expected_var) in lines {
        let (var, _) = adapter.process_line(line).unwrap();
        assert_eq!(var, expected_var, "line: {line}");
    }
    let t = adapter.telemetry();
    assert!((t.fcu_spd - 250.0).abs() < 0.001, "fcu_spd={}", t.fcu_spd);
    assert!((t.fcu_hdg - 90.0).abs() < 0.001, "fcu_hdg={}", t.fcu_hdg);
    assert!((t.n1_left - 85.5).abs() < 0.001, "n1_left={}", t.n1_left);
    assert!((t.n1_right - 86.0).abs() < 0.001, "n1_right={}", t.n1_right);
    assert!(
        (t.fuel_left - 42_000.0).abs() < 0.001,
        "fuel_left={}",
        t.fuel_left
    );
    assert!(
        (t.fuel_right - 41_000.0).abs() < 0.001,
        "fuel_right={}",
        t.fuel_right
    );
    assert!(t.gear_down, "gear_down");
}

/// An unknown variable ID is wrapped in `PsxVariable::Unknown` and does NOT
/// modify any field in the telemetry snapshot.
#[test]
fn unknown_key_gracefully_ignored_no_state_corruption() {
    let mut adapter = PsxAdapter::new();
    adapter.process_line("Qi0001=300.0").unwrap();

    let (var, val) = adapter.process_line("Qi9999=42.0").unwrap();
    assert_eq!(var, PsxVariable::Unknown("Qi9999".to_owned()));
    assert!((val - 42.0).abs() < 0.001);

    // Known field must not have been altered by the unknown variable.
    assert!(
        (adapter.telemetry().fcu_spd - 300.0).abs() < 0.001,
        "fcu_spd should still be 300.0"
    );
}

/// FCU speed (Qi0001), heading (Qi0002), and gear-down (Qi0200) fields are
/// all applied to the telemetry snapshot.
#[test]
fn altitude_heading_speed_fields_parsed() {
    let mut adapter = PsxAdapter::new();
    adapter.process_line("Qi0001=320.0").unwrap();
    adapter.process_line("Qi0002=180.0").unwrap();
    adapter.process_line("Qi0200=1.0").unwrap();

    let t = adapter.telemetry();
    assert!((t.fcu_spd - 320.0).abs() < 0.001, "fcu_spd={}", t.fcu_spd);
    assert!((t.fcu_hdg - 180.0).abs() < 0.001, "fcu_hdg={}", t.fcu_hdg);
    assert!(t.gear_down, "gear_down should be true");
}

/// A disconnect / end-of-session message (no `=` separator) returns a
/// `MissingSeparator` error without panicking; telemetry is unchanged.
#[test]
fn end_of_session_disconnect_message_handled() {
    let mut adapter = PsxAdapter::new();
    let result = adapter.process_line("disconnect");
    assert!(
        matches!(result, Err(PsxAdapterError::MissingSeparator { .. })),
        "expected MissingSeparator error"
    );
    // Telemetry must still be at default values.
    assert_eq!(*adapter.telemetry(), PsxTelemetry::default());
}

/// Whitespace surrounding the line is trimmed before parsing.
#[test]
fn whitespace_trimmed_before_parsing() {
    let (var, val) = parse_psx_line("  Qi0010=75.0  ").unwrap();
    assert_eq!(var, PsxVariable::N1Left);
    assert!((val - 75.0).abs() < 0.001, "val={val}");
}

/// The gear-down threshold is exactly 0.5: values below are gear-up, ≥ 0.5 is
/// gear-down.
#[test]
fn gear_down_threshold_boundary_conditions() {
    let mut adapter = PsxAdapter::new();

    adapter.process_line("Qi0200=0.49").unwrap();
    assert!(!adapter.telemetry().gear_down, "0.49 → gear up");

    adapter.process_line("Qi0200=0.5").unwrap();
    assert!(adapter.telemetry().gear_down, "0.5 → gear down");

    adapter.process_line("Qi0200=0.0").unwrap();
    assert!(!adapter.telemetry().gear_down, "0.0 → gear up");

    adapter.process_line("Qi0200=1.0").unwrap();
    assert!(adapter.telemetry().gear_down, "1.0 → gear down");
}

/// The default PSX port constant and `with_port` constructor work correctly.
#[test]
fn adapter_port_configuration() {
    assert_eq!(PsxAdapter::default().port, PSX_DEFAULT_PORT);
    assert_eq!(PsxAdapter::with_port(9_000).port, 9_000);
}
