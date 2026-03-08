// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for `flight-units` conversion tables and display output.
//!
//! These tests capture the exact numeric output of every conversion function
//! so that accidental constant changes are caught immediately.
//! Run `cargo insta review` to accept new or changed snapshots.

use flight_units::conversions;
use flight_units::{Angle, AngleUnit, Force, ForceUnit, Speed, SpeedUnit};

// ── Speed conversion table ───────────────────────────────────────────────────

#[test]
fn snapshot_speed_conversion_table() {
    let knots_inputs = [0.0, 1.0, 100.0, 250.0, 500.0];
    let mut table = String::new();
    table.push_str("knots → m/s → kph\n");
    table.push_str("─────────────────────────────────\n");
    for kts in &knots_inputs {
        let mps = conversions::knots_to_mps(*kts);
        let kph = conversions::knots_to_kph(*kts);
        table.push_str(&format!("{kts:>7.1} kt  →  {mps:>8.4} m/s  →  {kph:>8.4} kph\n"));
    }
    insta::assert_snapshot!("speed_conversion_table", table);
}

// ── Altitude conversion table ────────────────────────────────────────────────

#[test]
fn snapshot_altitude_conversion_table() {
    let feet_inputs = [0.0, 1000.0, 5000.0, 18000.0, 35000.0, 41000.0];
    let mut table = String::new();
    table.push_str("feet → meters\n");
    table.push_str("──────────────────────────\n");
    for ft in &feet_inputs {
        let m = conversions::feet_to_meters(*ft);
        table.push_str(&format!("{ft:>8.0} ft  →  {m:>10.2} m\n"));
    }
    insta::assert_snapshot!("altitude_conversion_table", table);
}

// ── Vertical speed conversion table ──────────────────────────────────────────

#[test]
fn snapshot_vertical_speed_conversion_table() {
    let fpm_inputs = [-2000.0, -500.0, 0.0, 500.0, 1800.0, 6000.0];
    let mut table = String::new();
    table.push_str("fpm → m/s\n");
    table.push_str("──────────────────────────\n");
    for fpm in &fpm_inputs {
        let mps = conversions::fpm_to_mps(*fpm);
        table.push_str(&format!("{fpm:>8.0} fpm  →  {mps:>8.4} m/s\n"));
    }
    insta::assert_snapshot!("vertical_speed_conversion_table", table);
}

// ── Angle conversion table ───────────────────────────────────────────────────

#[test]
fn snapshot_angle_conversion_table() {
    let degree_inputs = [0.0, 45.0, 90.0, 180.0, 270.0, 360.0];
    let mut table = String::new();
    table.push_str("degrees → radians\n");
    table.push_str("──────────────────────────\n");
    for deg in &degree_inputs {
        let rad = conversions::degrees_to_radians(*deg);
        table.push_str(&format!("{deg:>7.1}°  →  {rad:>8.6} rad\n"));
    }
    insta::assert_snapshot!("angle_conversion_table", table);
}

// ── Angle normalization table ────────────────────────────────────────────────

#[test]
fn snapshot_angle_normalization_table() {
    use flight_units::angles;
    let inputs = [-720.0, -270.0, -180.0, -90.0, 0.0, 90.0, 180.0, 270.0, 360.0, 450.0, 720.0];
    let mut table = String::new();
    table.push_str("input → signed [-180,180] → unsigned [0,360)\n");
    table.push_str("─────────────────────────────────────────────\n");
    for deg in &inputs {
        let signed = angles::normalize_degrees_signed(*deg);
        let unsigned = angles::normalize_degrees_unsigned(*deg);
        table.push_str(&format!(
            "{deg:>8.1}°  →  {signed:>8.1}° signed  →  {unsigned:>8.1}° unsigned\n"
        ));
    }
    insta::assert_snapshot!("angle_normalization_table", table);
}

// ── UnitValue formatting (Debug) ─────────────────────────────────────────────

#[test]
fn snapshot_unit_value_debug_formats() {
    let speed = Speed {
        value: 250.0,
        unit: SpeedUnit::Knots,
    };
    let angle = Angle {
        value: 45.0,
        unit: AngleUnit::Degrees,
    };
    let force = Force {
        value: 9.81,
        unit: ForceUnit::Newtons,
    };

    let output = format!(
        "Speed: {speed:?}\nAngle: {angle:?}\nForce: {force:?}"
    );
    insta::assert_snapshot!("unit_value_debug_formats", output);
}

// ── UnitValue serialization (JSON) ───────────────────────────────────────────

#[test]
fn snapshot_unit_value_json_serialization() {
    let speed = Speed {
        value: 250.0,
        unit: SpeedUnit::Knots,
    };
    let angle = Angle {
        value: 3.14159,
        unit: AngleUnit::Radians,
    };
    let force = Force {
        value: 12.5,
        unit: ForceUnit::NewtonMeters,
    };

    let speed_json = serde_json::to_string_pretty(&speed).unwrap();
    let angle_json = serde_json::to_string_pretty(&angle).unwrap();
    let force_json = serde_json::to_string_pretty(&force).unwrap();

    let output = format!(
        "Speed:\n{speed_json}\n\nAngle:\n{angle_json}\n\nForce:\n{force_json}"
    );
    insta::assert_snapshot!("unit_value_json_serialization", output);
}
