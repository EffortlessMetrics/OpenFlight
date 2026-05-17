// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Shared VKB HID report builders for integration tests.

#![allow(dead_code)]

pub const JOYSTICK_REPORT_LEN: usize = 21;
pub const STECS_REPORT_LEN: usize = 14;
pub const STECS_MT_REPORT_LEN: usize = 17;
pub const SEM_THQ_REPORT_LEN: usize = 16;
pub const T_RUDDER_REPORT_LEN: usize = 6;

fn write_u16_fields<const N: usize>(dst: &mut [u8], values: [u16; N]) {
    for (slot, value) in dst.chunks_exact_mut(2).zip(values) {
        slot.copy_from_slice(&value.to_le_bytes());
    }
}

/// Build a standard 21-byte VKB joystick report from individual axis fields.
#[allow(clippy::too_many_arguments)]
pub fn joystick_report_fields(
    roll: u16,
    pitch: u16,
    yaw: u16,
    mini_x: u16,
    mini_y: u16,
    throttle: u16,
    btn_lo: u32,
    btn_hi: u32,
    hat_byte: u8,
) -> [u8; JOYSTICK_REPORT_LEN] {
    joystick_report(
        [roll, pitch, yaw, mini_x, mini_y, throttle],
        btn_lo,
        btn_hi,
        hat_byte,
    )
}

/// Build a standard 21-byte VKB joystick report (Gladiator / Gunfighter layout).
pub fn joystick_report(
    axes: [u16; 6],
    btn_lo: u32,
    btn_hi: u32,
    hat_byte: u8,
) -> [u8; JOYSTICK_REPORT_LEN] {
    let mut report = [0u8; JOYSTICK_REPORT_LEN];
    write_u16_fields(&mut report[..12], axes);
    report[12..16].copy_from_slice(&btn_lo.to_le_bytes());
    report[16..20].copy_from_slice(&btn_hi.to_le_bytes());
    report[20] = hat_byte;
    report
}

/// Build a STECS interface report from individual axis fields.
pub fn stecs_report_fields(
    rx: u16,
    ry: u16,
    x: u16,
    y: u16,
    z: u16,
    buttons: u32,
) -> [u8; STECS_REPORT_LEN] {
    stecs_report([rx, ry, x, y, z], buttons)
}

/// Build a STECS interface report: 14 bytes (5 axes + 4 button bytes).
pub fn stecs_report(axes: [u16; 5], buttons: u32) -> [u8; STECS_REPORT_LEN] {
    let mut report = [0u8; STECS_REPORT_LEN];
    write_u16_fields(&mut report[..10], axes);
    report[10..14].copy_from_slice(&buttons.to_le_bytes());
    report
}

/// Build a STECS Modern Throttle report (17 bytes, including report ID byte).
pub fn stecs_mt_report(
    throttle: u16,
    mini_left: u16,
    mini_right: u16,
    rotary: u16,
    word0: u32,
    word1: u32,
) -> [u8; STECS_MT_REPORT_LEN] {
    let mut report = [0u8; STECS_MT_REPORT_LEN];
    report[0] = 0x01;
    write_u16_fields(&mut report[1..9], [throttle, mini_left, mini_right, rotary]);
    report[9..13].copy_from_slice(&word0.to_le_bytes());
    report[13..17].copy_from_slice(&word1.to_le_bytes());
    report
}

/// Build a 16-byte SEM THQ report.
pub fn sem_thq_report(axes: [u16; 4], btn_lo: u32, btn_hi: u32) -> [u8; SEM_THQ_REPORT_LEN] {
    let mut report = [0u8; SEM_THQ_REPORT_LEN];
    write_u16_fields(&mut report[..8], axes);
    report[8..12].copy_from_slice(&btn_lo.to_le_bytes());
    report[12..16].copy_from_slice(&btn_hi.to_le_bytes());
    report
}

/// Build a 6-byte T-Rudder report.
pub fn t_rudder_report(left: u16, right: u16, rudder: u16) -> [u8; T_RUDDER_REPORT_LEN] {
    let mut report = [0u8; T_RUDDER_REPORT_LEN];
    write_u16_fields(&mut report, [left, right, rudder]);
    report
}

/// Prepend a report-ID byte to a payload.
pub fn with_report_id(id: u8, payload: &[u8]) -> Vec<u8> {
    let mut report = Vec::with_capacity(payload.len() + 1);
    report.push(id);
    report.extend_from_slice(payload);
    report
}
