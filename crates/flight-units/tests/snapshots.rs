// SPDX-License-Identifier: MIT OR Apache-2.0

//! Snapshot tests for flight-units crate.

use flight_units::conversions;

#[test]
fn snapshot_speed_conversion_table() {
    let knots_values = [0.0_f32, 1.0, 60.0, 120.0, 250.0, 500.0];
    let table: Vec<String> = knots_values
        .iter()
        .map(|&kt| {
            format!(
                "{:>7.1} kt = {:>8.3} m/s = {:>8.3} kph",
                kt,
                conversions::knots_to_mps(kt),
                conversions::knots_to_kph(kt),
            )
        })
        .collect();
    insta::assert_snapshot!("speed_conversion_table", table.join("\n"));
}

#[test]
fn snapshot_altitude_conversion_table() {
    let feet_values = [0.0_f32, 100.0, 1000.0, 5000.0, 10000.0, 35000.0, 41000.0];
    let table: Vec<String> = feet_values
        .iter()
        .map(|&ft| {
            format!(
                "{:>8.0} ft = {:>10.3} m",
                ft,
                conversions::feet_to_meters(ft),
            )
        })
        .collect();
    insta::assert_snapshot!("altitude_conversion_table", table.join("\n"));
}

#[test]
fn snapshot_vertical_speed_conversion_table() {
    let fpm_values = [-2000.0_f32, -1000.0, -500.0, 0.0, 500.0, 1000.0, 2000.0];
    let table: Vec<String> = fpm_values
        .iter()
        .map(|&fpm| {
            format!(
                "{:>8.0} fpm = {:>8.3} m/s",
                fpm,
                conversions::fpm_to_mps(fpm),
            )
        })
        .collect();
    insta::assert_snapshot!("vertical_speed_conversion_table", table.join("\n"));
}

#[test]
fn snapshot_angle_conversion_table() {
    let degrees_values = [0.0_f32, 30.0, 45.0, 90.0, 180.0, 270.0, 360.0];
    let table: Vec<String> = degrees_values
        .iter()
        .map(|&deg| {
            format!(
                "{:>6.1} deg = {:>8.5} rad",
                deg,
                conversions::degrees_to_radians(deg),
            )
        })
        .collect();
    insta::assert_snapshot!("angle_conversion_table", table.join("\n"));
}

#[test]
fn snapshot_angle_normalization_table() {
    use flight_units::angles;

    let values = [
        -720.0_f32, -360.0, -270.0, -180.0, -90.0, 0.0, 90.0, 180.0, 270.0, 360.0, 450.0,
        720.0,
    ];
    let table: Vec<String> = values
        .iter()
        .map(|&deg| {
            format!(
                "{:>7.1} deg => signed={:>7.1}  unsigned={:>7.1}",
                deg,
                angles::normalize_degrees_signed(deg),
                angles::normalize_degrees_unsigned(deg),
            )
        })
        .collect();
    insta::assert_snapshot!("angle_normalization_table", table.join("\n"));
}
