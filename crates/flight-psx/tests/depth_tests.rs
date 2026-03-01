// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for `flight-psx`.
//!
//! Comprehensive coverage of:
//! - TCP message parsing (text-based `key=value` protocol)
//! - Variable lookup and type conversion (`PsxVariable`)
//! - Telemetry snapshot accumulation (`PsxTelemetry`)
//! - Adapter state management (`PsxAdapter`)
//! - Error handling & edge cases
//! - Serialisation round-trips (serde)
//! - Property-based tests (proptest)

use flight_psx::{
    PSX_DEFAULT_PORT, PsxAdapter, PsxAdapterError, PsxTelemetry, PsxVariable, parse_psx_line,
};
use proptest::prelude::*;

// ═══════════════════════════════════════════════════════════════════════════════
// §1  TCP Message Parsing
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn parse_integer_value_without_decimal() {
    let (var, val) = parse_psx_line("Qi0200=1").unwrap();
    assert_eq!(var, PsxVariable::GearDown);
    assert!((val - 1.0).abs() < f64::EPSILON);
}

#[test]
fn parse_negative_value() {
    let (var, val) = parse_psx_line("Qi0001=-50.0").unwrap();
    assert_eq!(var, PsxVariable::FcuSpd);
    assert!((val - (-50.0)).abs() < f64::EPSILON);
}

#[test]
fn parse_zero_value() {
    let (var, val) = parse_psx_line("Qi0010=0").unwrap();
    assert_eq!(var, PsxVariable::N1Left);
    assert!((val).abs() < f64::EPSILON);
}

#[test]
fn parse_very_large_value() {
    let (_, val) = parse_psx_line("Qi0100=999999999.99").unwrap();
    assert!((val - 999_999_999.99).abs() < 0.01);
}

#[test]
fn parse_very_small_positive_value() {
    let (_, val) = parse_psx_line("Qi0100=0.0001").unwrap();
    assert!((val - 0.0001).abs() < f64::EPSILON);
}

#[test]
fn parse_scientific_notation() {
    let (_, val) = parse_psx_line("Qi0100=1.5e3").unwrap();
    assert!((val - 1500.0).abs() < f64::EPSILON);
}

#[test]
fn parse_negative_scientific_notation() {
    let (_, val) = parse_psx_line("Qi0100=-2.5e-2").unwrap();
    assert!((val - (-0.025)).abs() < f64::EPSILON);
}

#[test]
fn parse_leading_whitespace_only() {
    let (var, val) = parse_psx_line("   Qi0001=100.0").unwrap();
    assert_eq!(var, PsxVariable::FcuSpd);
    assert!((val - 100.0).abs() < f64::EPSILON);
}

#[test]
fn parse_trailing_whitespace_only() {
    let (var, val) = parse_psx_line("Qi0001=100.0   ").unwrap();
    assert_eq!(var, PsxVariable::FcuSpd);
    assert!((val - 100.0).abs() < f64::EPSILON);
}

#[test]
fn parse_tab_whitespace_trimmed() {
    let (var, val) = parse_psx_line("\tQi0002=45.0\t").unwrap();
    assert_eq!(var, PsxVariable::FcuHdg);
    assert!((val - 45.0).abs() < f64::EPSILON);
}

#[test]
fn parse_value_with_leading_plus_sign() {
    let (_, val) = parse_psx_line("Qi0001=+250.0").unwrap();
    assert!((val - 250.0).abs() < f64::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §2  Variable Lookup & Type Conversion
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn from_id_all_known_variants() {
    assert_eq!(PsxVariable::from_id("Qi0001"), PsxVariable::FcuSpd);
    assert_eq!(PsxVariable::from_id("Qi0002"), PsxVariable::FcuHdg);
    assert_eq!(PsxVariable::from_id("Qi0010"), PsxVariable::N1Left);
    assert_eq!(PsxVariable::from_id("Qi0011"), PsxVariable::N1Right);
    assert_eq!(PsxVariable::from_id("Qi0100"), PsxVariable::FuelLeft);
    assert_eq!(PsxVariable::from_id("Qi0101"), PsxVariable::FuelRight);
    assert_eq!(PsxVariable::from_id("Qi0200"), PsxVariable::GearDown);
}

#[test]
fn from_id_unknown_preserves_string() {
    let var = PsxVariable::from_id("Qi8888");
    assert_eq!(var, PsxVariable::Unknown("Qi8888".to_owned()));
}

#[test]
fn from_id_empty_string_is_unknown() {
    let var = PsxVariable::from_id("");
    assert_eq!(var, PsxVariable::Unknown(String::new()));
}

#[test]
fn from_id_case_sensitive() {
    // PSX IDs are case-sensitive; lowercase should not match
    let var = PsxVariable::from_id("qi0001");
    assert!(matches!(var, PsxVariable::Unknown(_)));
}

#[test]
fn id_round_trip_all_known_variables() {
    let known = [
        PsxVariable::FcuSpd,
        PsxVariable::FcuHdg,
        PsxVariable::N1Left,
        PsxVariable::N1Right,
        PsxVariable::FuelLeft,
        PsxVariable::FuelRight,
        PsxVariable::GearDown,
    ];
    for var in &known {
        let id = var.id().expect("known variable must have an ID");
        assert_eq!(&PsxVariable::from_id(id), var);
    }
}

#[test]
fn unknown_variable_id_returns_none() {
    assert_eq!(PsxVariable::Unknown("X".to_owned()).id(), None);
}

#[test]
fn variable_clone_and_eq() {
    let a = PsxVariable::FcuSpd;
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn variable_debug_format_contains_variant_name() {
    let dbg = format!("{:?}", PsxVariable::N1Left);
    assert!(dbg.contains("N1Left"));
}

#[test]
fn unknown_variable_debug_contains_id_string() {
    let dbg = format!("{:?}", PsxVariable::Unknown("Zz42".to_owned()));
    assert!(dbg.contains("Zz42"));
}

#[test]
fn variable_hash_consistency() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(PsxVariable::FcuSpd);
    set.insert(PsxVariable::FcuSpd);
    assert_eq!(set.len(), 1);
}

#[test]
fn different_unknown_variables_not_equal() {
    let a = PsxVariable::Unknown("A".to_owned());
    let b = PsxVariable::Unknown("B".to_owned());
    assert_ne!(a, b);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §3  Telemetry Snapshot
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn telemetry_default_all_zeroed() {
    let t = PsxTelemetry::default();
    assert_eq!(t.fcu_spd, 0.0);
    assert_eq!(t.fcu_hdg, 0.0);
    assert_eq!(t.n1_left, 0.0);
    assert_eq!(t.n1_right, 0.0);
    assert_eq!(t.fuel_left, 0.0);
    assert_eq!(t.fuel_right, 0.0);
    assert!(!t.gear_down);
}

#[test]
fn telemetry_apply_fcu_spd() {
    let mut t = PsxTelemetry::default();
    t.apply(&PsxVariable::FcuSpd, 310.0);
    assert!((t.fcu_spd - 310.0).abs() < f64::EPSILON);
}

#[test]
fn telemetry_apply_fcu_hdg() {
    let mut t = PsxTelemetry::default();
    t.apply(&PsxVariable::FcuHdg, 270.0);
    assert!((t.fcu_hdg - 270.0).abs() < f64::EPSILON);
}

#[test]
fn telemetry_apply_n1_left() {
    let mut t = PsxTelemetry::default();
    t.apply(&PsxVariable::N1Left, 92.3);
    assert!((t.n1_left - 92.3).abs() < f64::EPSILON);
}

#[test]
fn telemetry_apply_n1_right() {
    let mut t = PsxTelemetry::default();
    t.apply(&PsxVariable::N1Right, 91.7);
    assert!((t.n1_right - 91.7).abs() < f64::EPSILON);
}

#[test]
fn telemetry_apply_fuel_left() {
    let mut t = PsxTelemetry::default();
    t.apply(&PsxVariable::FuelLeft, 50000.0);
    assert!((t.fuel_left - 50000.0).abs() < f64::EPSILON);
}

#[test]
fn telemetry_apply_fuel_right() {
    let mut t = PsxTelemetry::default();
    t.apply(&PsxVariable::FuelRight, 48000.0);
    assert!((t.fuel_right - 48000.0).abs() < f64::EPSILON);
}

#[test]
fn telemetry_gear_down_exact_boundary() {
    let mut t = PsxTelemetry::default();
    t.apply(&PsxVariable::GearDown, 0.5);
    assert!(t.gear_down);
}

#[test]
fn telemetry_gear_up_just_below_boundary() {
    let mut t = PsxTelemetry::default();
    t.apply(&PsxVariable::GearDown, 0.499_999);
    assert!(!t.gear_down);
}

#[test]
fn telemetry_gear_toggle_sequence() {
    let mut t = PsxTelemetry::default();
    t.apply(&PsxVariable::GearDown, 1.0);
    assert!(t.gear_down);
    t.apply(&PsxVariable::GearDown, 0.0);
    assert!(!t.gear_down);
    t.apply(&PsxVariable::GearDown, 0.75);
    assert!(t.gear_down);
}

#[test]
fn telemetry_apply_unknown_is_noop() {
    let mut t = PsxTelemetry::default();
    let before = t.clone();
    t.apply(&PsxVariable::Unknown("Zz0000".to_owned()), 12345.0);
    assert_eq!(t, before);
}

#[test]
fn telemetry_overwrite_value() {
    let mut t = PsxTelemetry::default();
    t.apply(&PsxVariable::FcuSpd, 200.0);
    t.apply(&PsxVariable::FcuSpd, 350.0);
    assert!((t.fcu_spd - 350.0).abs() < f64::EPSILON);
}

#[test]
fn telemetry_clone_independence() {
    let mut t = PsxTelemetry::default();
    t.apply(&PsxVariable::N1Left, 80.0);
    let snapshot = t.clone();
    t.apply(&PsxVariable::N1Left, 95.0);
    assert!((snapshot.n1_left - 80.0).abs() < f64::EPSILON);
    assert!((t.n1_left - 95.0).abs() < f64::EPSILON);
}

#[test]
fn telemetry_partial_eq_same_values() {
    let mut a = PsxTelemetry::default();
    let mut b = PsxTelemetry::default();
    a.apply(&PsxVariable::FcuSpd, 100.0);
    b.apply(&PsxVariable::FcuSpd, 100.0);
    assert_eq!(a, b);
}

#[test]
fn telemetry_partial_eq_different_values() {
    let mut a = PsxTelemetry::default();
    let mut b = PsxTelemetry::default();
    a.apply(&PsxVariable::FcuSpd, 100.0);
    b.apply(&PsxVariable::FcuSpd, 200.0);
    assert_ne!(a, b);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §4  Adapter State Management
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn adapter_new_uses_default_port() {
    let a = PsxAdapter::new();
    assert_eq!(a.port, PSX_DEFAULT_PORT);
}

#[test]
fn adapter_default_trait_matches_new() {
    let a = PsxAdapter::default();
    assert_eq!(a.port, PSX_DEFAULT_PORT);
    assert_eq!(*a.telemetry(), PsxTelemetry::default());
}

#[test]
fn adapter_with_port_zero() {
    let a = PsxAdapter::with_port(0);
    assert_eq!(a.port, 0);
}

#[test]
fn adapter_with_port_max() {
    let a = PsxAdapter::with_port(u16::MAX);
    assert_eq!(a.port, u16::MAX);
}

#[test]
fn adapter_fresh_telemetry_is_default() {
    let a = PsxAdapter::new();
    assert_eq!(*a.telemetry(), PsxTelemetry::default());
}

#[test]
fn adapter_process_line_returns_parsed_pair() {
    let mut a = PsxAdapter::new();
    let (var, val) = a.process_line("Qi0002=90.0").unwrap();
    assert_eq!(var, PsxVariable::FcuHdg);
    assert!((val - 90.0).abs() < f64::EPSILON);
}

#[test]
fn adapter_process_line_error_leaves_telemetry_intact() {
    let mut a = PsxAdapter::new();
    a.process_line("Qi0001=250.0").unwrap();
    let _ = a.process_line("garbage");
    assert!((a.telemetry().fcu_spd - 250.0).abs() < f64::EPSILON);
}

#[test]
fn adapter_accumulates_all_variables() {
    let mut a = PsxAdapter::new();
    let lines = [
        "Qi0001=300.0",
        "Qi0002=180.0",
        "Qi0010=88.0",
        "Qi0011=87.5",
        "Qi0100=60000.0",
        "Qi0101=59000.0",
        "Qi0200=1",
    ];
    for line in lines {
        a.process_line(line).unwrap();
    }
    let t = a.telemetry();
    assert!((t.fcu_spd - 300.0).abs() < 0.001);
    assert!((t.fcu_hdg - 180.0).abs() < 0.001);
    assert!((t.n1_left - 88.0).abs() < 0.001);
    assert!((t.n1_right - 87.5).abs() < 0.001);
    assert!((t.fuel_left - 60000.0).abs() < 0.001);
    assert!((t.fuel_right - 59000.0).abs() < 0.001);
    assert!(t.gear_down);
}

#[test]
fn adapter_handles_interleaved_known_and_unknown() {
    let mut a = PsxAdapter::new();
    a.process_line("Qi0001=250.0").unwrap();
    a.process_line("QiAAAA=999.0").unwrap();
    a.process_line("Qi0010=91.0").unwrap();
    a.process_line("QiBBBB=0.0").unwrap();

    assert!((a.telemetry().fcu_spd - 250.0).abs() < f64::EPSILON);
    assert!((a.telemetry().n1_left - 91.0).abs() < f64::EPSILON);
}

#[test]
fn adapter_rapid_update_same_variable() {
    let mut a = PsxAdapter::new();
    for i in 0..100 {
        a.process_line(&format!("Qi0001={}.0", i)).unwrap();
    }
    assert!((a.telemetry().fcu_spd - 99.0).abs() < f64::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §5  Error Handling
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn error_missing_separator_empty_line() {
    let err = parse_psx_line("").unwrap_err();
    assert!(matches!(err, PsxAdapterError::MissingSeparator { .. }));
}

#[test]
fn error_missing_separator_no_equals() {
    let err = parse_psx_line("Qi0001:280.0").unwrap_err();
    assert!(matches!(err, PsxAdapterError::MissingSeparator { .. }));
}

#[test]
fn error_empty_id() {
    let err = parse_psx_line("=42.0").unwrap_err();
    assert!(matches!(err, PsxAdapterError::EmptyId));
}

#[test]
fn error_invalid_value_text() {
    let err = parse_psx_line("Qi0001=abc").unwrap_err();
    assert!(matches!(err, PsxAdapterError::InvalidValue { .. }));
}

#[test]
fn error_invalid_value_empty_after_equals() {
    let err = parse_psx_line("Qi0001=").unwrap_err();
    assert!(matches!(err, PsxAdapterError::InvalidValue { .. }));
}

#[test]
fn error_invalid_value_multiple_dots() {
    let err = parse_psx_line("Qi0001=1.2.3").unwrap_err();
    assert!(matches!(err, PsxAdapterError::InvalidValue { .. }));
}

#[test]
fn error_display_missing_separator() {
    let err = PsxAdapterError::MissingSeparator {
        line: "bad".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("missing"));
    assert!(msg.contains("bad"));
}

#[test]
fn error_display_invalid_value() {
    let err = PsxAdapterError::InvalidValue {
        raw: "xyz".to_owned(),
    };
    let msg = err.to_string();
    assert!(msg.contains("invalid"));
    assert!(msg.contains("xyz"));
}

#[test]
fn error_display_empty_id() {
    let err = PsxAdapterError::EmptyId;
    let msg = err.to_string();
    assert!(msg.contains("empty"));
}

#[test]
fn error_partial_eq() {
    let a = PsxAdapterError::EmptyId;
    let b = PsxAdapterError::EmptyId;
    assert_eq!(a, b);

    let c = PsxAdapterError::MissingSeparator {
        line: "x".to_owned(),
    };
    let d = PsxAdapterError::MissingSeparator {
        line: "x".to_owned(),
    };
    assert_eq!(c, d);
    assert_ne!(a, c);
}

#[test]
fn error_debug_format() {
    let err = PsxAdapterError::EmptyId;
    let dbg = format!("{err:?}");
    assert!(dbg.contains("EmptyId"));
}

#[test]
fn parse_line_with_multiple_equals_uses_first() {
    // "key=val=ue" should split at first `=`, value part "val=ue" is invalid f64
    let result = parse_psx_line("Qi0001=1.0=extra");
    assert!(result.is_err());
}

#[test]
fn parse_whitespace_only_line() {
    let err = parse_psx_line("   ").unwrap_err();
    assert!(matches!(err, PsxAdapterError::MissingSeparator { .. }));
}

// ═══════════════════════════════════════════════════════════════════════════════
// §6  Serde Round-Trips
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn telemetry_serde_round_trip() {
    let mut t = PsxTelemetry::default();
    t.apply(&PsxVariable::FcuSpd, 310.0);
    t.apply(&PsxVariable::GearDown, 1.0);
    let json = serde_json::to_string(&t).unwrap();
    let t2: PsxTelemetry = serde_json::from_str(&json).unwrap();
    assert_eq!(t, t2);
}

#[test]
fn variable_serde_round_trip_known() {
    let var = PsxVariable::FcuHdg;
    let json = serde_json::to_string(&var).unwrap();
    let var2: PsxVariable = serde_json::from_str(&json).unwrap();
    assert_eq!(var, var2);
}

#[test]
fn variable_serde_round_trip_unknown() {
    let var = PsxVariable::Unknown("Custom42".to_owned());
    let json = serde_json::to_string(&var).unwrap();
    let var2: PsxVariable = serde_json::from_str(&json).unwrap();
    assert_eq!(var, var2);
}

#[test]
fn telemetry_serde_all_fields_populated() {
    let t = PsxTelemetry {
        fcu_spd: 280.0,
        fcu_hdg: 90.0,
        n1_left: 85.5,
        n1_right: 86.0,
        fuel_left: 42000.0,
        fuel_right: 41000.0,
        gear_down: true,
    };
    let json = serde_json::to_string(&t).unwrap();
    let t2: PsxTelemetry = serde_json::from_str(&json).unwrap();
    assert_eq!(t, t2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §7  Constants
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn default_port_is_10747() {
    assert_eq!(PSX_DEFAULT_PORT, 10747);
}

// ═══════════════════════════════════════════════════════════════════════════════
// §8  Property-Based Tests (proptest)
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_valid_line_always_parses(
        id in "[A-Za-z][A-Za-z0-9]{2,8}",
        val in proptest::num::f64::NORMAL | proptest::num::f64::POSITIVE | proptest::num::f64::NEGATIVE,
    ) {
        let line = format!("{id}={val}");
        let result = parse_psx_line(&line);
        prop_assert!(result.is_ok(), "failed to parse: {line}");
        let (var, parsed_val) = result.unwrap();
        prop_assert_eq!(var, PsxVariable::from_id(&id));
        prop_assert!((parsed_val - val).abs() < 1e-6 || (parsed_val == val),
            "value mismatch: expected {val}, got {parsed_val}");
    }

    #[test]
    fn prop_known_ids_round_trip(idx in 0usize..7) {
        let known_ids = ["Qi0001", "Qi0002", "Qi0010", "Qi0011", "Qi0100", "Qi0101", "Qi0200"];
        let id = known_ids[idx];
        let var = PsxVariable::from_id(id);
        let back = var.id().unwrap();
        prop_assert_eq!(id, back);
    }

    #[test]
    fn prop_unknown_id_has_no_id(s in "[A-Z]{5,10}") {
        let var = PsxVariable::from_id(&s);
        if matches!(var, PsxVariable::Unknown(_)) {
            prop_assert!(var.id().is_none());
        }
    }

    #[test]
    fn prop_telemetry_apply_unknown_noop(
        id in "[A-Z]{5,10}",
        val in proptest::num::f64::NORMAL,
    ) {
        let mut t = PsxTelemetry::default();
        let before = t.clone();
        t.apply(&PsxVariable::Unknown(id), val);
        prop_assert_eq!(t, before);
    }

    #[test]
    fn prop_gear_threshold(val in 0.0f64..=2.0) {
        let mut t = PsxTelemetry::default();
        t.apply(&PsxVariable::GearDown, val);
        if val >= 0.5 {
            prop_assert!(t.gear_down, "gear should be down at val={val}");
        } else {
            prop_assert!(!t.gear_down, "gear should be up at val={val}");
        }
    }

    #[test]
    fn prop_missing_separator_always_errors(line in "[^=]+") {
        let result = parse_psx_line(&line);
        prop_assert!(result.is_err());
    }

    #[test]
    fn prop_empty_id_always_errors(val in proptest::num::f64::NORMAL) {
        let line = format!("={val}");
        let result = parse_psx_line(&line);
        prop_assert!(matches!(result, Err(PsxAdapterError::EmptyId)));
    }

    #[test]
    fn prop_adapter_telemetry_reflects_last_value(
        values in proptest::collection::vec(0.0f64..=400.0, 1..50),
    ) {
        let mut a = PsxAdapter::new();
        let mut last_val = 0.0f64;
        for v in &values {
            a.process_line(&format!("Qi0001={v}")).unwrap();
            last_val = *v;
        }
        prop_assert!((a.telemetry().fcu_spd - last_val).abs() < 1e-6,
            "expected {last_val}, got {}", a.telemetry().fcu_spd);
    }

    #[test]
    fn prop_whitespace_invariant(
        spaces_before in "[ \\t]{0,5}",
        spaces_after in "[ \\t]{0,5}",
        val in 0.0f64..1000.0,
    ) {
        let line = format!("{spaces_before}Qi0001={val}{spaces_after}");
        let result = parse_psx_line(&line);
        prop_assert!(result.is_ok(), "failed: {:?}", line);
        let (var, parsed) = result.unwrap();
        prop_assert_eq!(var, PsxVariable::FcuSpd);
        prop_assert!((parsed - val).abs() < 1e-6);
    }
}
