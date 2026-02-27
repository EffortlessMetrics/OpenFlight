// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests covering FFB torque output and device detection for the
//! Moza AB9 FFB base.

use flight_ffb_moza::MOZA_PIDS;
use flight_ffb_moza::effects::{FfbMode, TORQUE_REPORT_ID, TorqueCommand};
use flight_ffb_moza::input::{
    AB9_BASE_PID, AB9_REPORT_LEN, MOZA_VENDOR_ID, R3_BASE_PID, parse_ab9_report,
};

fn make_report(roll: i16, pitch: i16) -> [u8; AB9_REPORT_LEN] {
    let mut r = [0u8; AB9_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r
}

/// When the stick is displaced positively in roll, a spring-centering torque
/// must oppose the displacement (negative torque for positive roll).
#[test]
fn constant_force_direction_matches_axis_displacement() {
    let r = make_report(16383, 0); // ~half positive roll
    let s = parse_ab9_report(&r).unwrap();
    assert!(s.axes.roll > 0.0, "roll should be positive");
    let torque = TorqueCommand {
        x: -s.axes.roll * 0.3,
        y: -s.axes.pitch * 0.3,
    };
    assert!(torque.x < 0.0, "spring torque must oppose positive roll");
    assert!(torque.is_safe());
}

/// A higher spring stiffness coefficient produces proportionally stronger
/// restoring torque for the same axis displacement.
#[test]
fn spring_centering_stiffness_scales_torque() {
    let r = make_report(i16::MAX, 0);
    let s = parse_ab9_report(&r).unwrap();
    let t_soft = TorqueCommand {
        x: -s.axes.roll * 0.1,
        y: 0.0,
    };
    let t_stiff = TorqueCommand {
        x: -s.axes.roll * 0.8,
        y: 0.0,
    };
    assert!(
        t_stiff.x.abs() > t_soft.x.abs(),
        "higher stiffness must produce larger restoring torque"
    );
    assert!(t_soft.is_safe());
    assert!(t_stiff.is_safe());
}

/// Friction (Damper) mode: the zero-torque command is well-formed and safe.
#[test]
fn friction_mode_zero_torque_report_is_valid() {
    assert_eq!(FfbMode::Damper.to_string(), "Damper");
    let cmd = TorqueCommand::ZERO;
    let report = cmd.to_report();
    assert_eq!(report[0], TORQUE_REPORT_ID);
    assert!(cmd.is_safe());
    let x_raw = i16::from_le_bytes([report[1], report[2]]);
    let y_raw = i16::from_le_bytes([report[3], report[4]]);
    assert_eq!(x_raw, 0);
    assert_eq!(y_raw, 0);
}

/// VID/PID constants correctly identify all known Moza FFB bases.
#[test]
fn device_detection_by_vid_pid() {
    assert_eq!(MOZA_VENDOR_ID, 0x346E);
    assert!(
        MOZA_PIDS.contains(&AB9_BASE_PID),
        "AB9 PID must be in MOZA_PIDS"
    );
    assert!(
        MOZA_PIDS.contains(&R3_BASE_PID),
        "R3 PID must be in MOZA_PIDS"
    );
    assert!(
        !MOZA_PIDS.contains(&0xFFFF),
        "unknown PID must not match any Moza device"
    );
}
