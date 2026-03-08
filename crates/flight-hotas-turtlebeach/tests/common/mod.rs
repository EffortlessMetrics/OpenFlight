// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Shared report builders for VelocityOne depth tests.

/// Builder for VelocityOne Flight HID reports with sensible centre defaults.
pub struct FlightInput {
    pub roll: u16,
    pub pitch: u16,
    pub rudder: u16,
    pub tl: u8,
    pub tr: u8,
    pub trim: u16,
    pub buttons: u64,
    pub hat: u8,
    pub toggles: u8,
}

impl Default for FlightInput {
    fn default() -> Self {
        Self {
            roll: 2048, pitch: 2048, rudder: 2048,
            tl: 0, tr: 0, trim: 2048,
            buttons: 0, hat: 15, toggles: 0,
        }
    }
}

pub fn build_flight(input: &FlightInput) -> [u8; 20] {
    let mut b = [0u8; 20];
    b[0..2].copy_from_slice(&input.roll.to_le_bytes());
    b[2..4].copy_from_slice(&input.pitch.to_le_bytes());
    b[4..6].copy_from_slice(&input.rudder.to_le_bytes());
    b[6] = input.tl;
    b[7] = input.tr;
    b[8..10].copy_from_slice(&input.trim.to_le_bytes());
    b[10..18].copy_from_slice(&input.buttons.to_le_bytes());
    b[18] = input.hat;
    b[19] = input.toggles;
    b
}

pub fn make_flightstick(
    x: u16, y: u16, twist: u16,
    throttle: u8, buttons: u16, hat: u8,
) -> [u8; 12] {
    let mut b = [0u8; 12];
    b[0..2].copy_from_slice(&x.to_le_bytes());
    b[2..4].copy_from_slice(&y.to_le_bytes());
    b[4..6].copy_from_slice(&twist.to_le_bytes());
    b[6] = throttle;
    b[7..9].copy_from_slice(&buttons.to_le_bytes());
    b[9] = hat;
    b
}

pub fn make_flightdeck(roll: u16, pitch: u16, tl: u8, tr: u8, buttons: u32) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[0..2].copy_from_slice(&roll.to_le_bytes());
    b[2..4].copy_from_slice(&pitch.to_le_bytes());
    b[4] = tl;
    b[5] = tr;
    b[6..10].copy_from_slice(&buttons.to_le_bytes());
    b
}

pub fn make_rudder(rudder: u16, bl: u8, br: u8) -> [u8; 8] {
    let mut b = [0u8; 8];
    b[0..2].copy_from_slice(&rudder.to_le_bytes());
    b[2] = bl;
    b[3] = br;
    b
}
