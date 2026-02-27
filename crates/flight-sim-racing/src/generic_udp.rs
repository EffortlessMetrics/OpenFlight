// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Generic binary UDP racing telemetry parser.
//!
//! Implements a 42-byte little-endian packet format compatible with SimHub and
//! OpenSimHardware bridge tools. Packets are sent to UDP port [`RACING_UDP_PORT`].
//!
//! ## Packet layout
//!
//! | Offset | Size | Field            | Notes              |
//! |--------|------|------------------|--------------------|
//! |  0– 3  |  4   | magic `u32` LE   | [`RACING_MAGIC`]   |
//! |  4     |  1   | version `u8`     | [`RACING_VERSION`] |
//! |  5     |  1   | gear `i8`        | −1=R, 0=N, 1–8     |
//! |  6– 9  |  4   | speed_ms `f32`   | m/s                |
//! | 10–13  |  4   | lateral_g `f32`  | −5.0 – 5.0         |
//! | 14–17  |  4   | longitudinal_g   | negative=braking   |
//! | 18–21  |  4   | vertical_g       | bumps/kerbs        |
//! | 22–25  |  4   | throttle `f32`   | 0.0 – 1.0          |
//! | 26–29  |  4   | brake `f32`      | 0.0 – 1.0          |
//! | 30–33  |  4   | steering_angle   | −1.0 – 1.0         |
//! | 34–37  |  4   | rpm `f32`        |                    |
//! | 38–41  |  4   | rpm_max `f32`    | redline            |

use thiserror::Error;

use crate::telemetry::RacingTelemetry;

/// Default UDP port for generic racing telemetry (SimHub / OpenSimHardware compatible).
pub const RACING_UDP_PORT: u16 = 20777;

/// Magic number at the start of every packet (`"RACE"` in ASCII, little-endian `u32`).
pub const RACING_MAGIC: u32 = 0x5241_4345;

/// Protocol version byte accepted by this parser.
pub const RACING_VERSION: u8 = 0x01;

/// Minimum valid packet size in bytes (covers all defined fields).
pub const MIN_PACKET_SIZE: usize = 42;

/// Errors produced by the generic UDP racing telemetry parser.
#[derive(Debug, Error, PartialEq)]
pub enum RacingError {
    /// The packet is shorter than [`MIN_PACKET_SIZE`].
    #[error("packet too short: expected at least {MIN_PACKET_SIZE} bytes, got {found}")]
    TooShort { found: usize },

    /// The magic number did not match [`RACING_MAGIC`].
    #[error("bad magic: expected {RACING_MAGIC:#010x}, got {found:#010x}")]
    BadMagic { found: u32 },

    /// A field could not be read at the given byte offset.
    #[error("failed to read field at byte offset {offset}")]
    ReadError { offset: usize },
}

/// Parse a generic UDP racing telemetry packet into a [`RacingTelemetry`].
///
/// # Errors
///
/// - [`RacingError::TooShort`] — fewer than [`MIN_PACKET_SIZE`] bytes.
/// - [`RacingError::BadMagic`] — bytes 0–3 ≠ [`RACING_MAGIC`].
/// - [`RacingError::ReadError`] — a field could not be read at the expected offset.
pub fn parse_generic_udp(data: &[u8]) -> Result<RacingTelemetry, RacingError> {
    if data.len() < MIN_PACKET_SIZE {
        return Err(RacingError::TooShort { found: data.len() });
    }

    let magic = read_u32_le(data, 0)?;
    if magic != RACING_MAGIC {
        return Err(RacingError::BadMagic { found: magic });
    }

    // Byte 4: version (accepted but not version-gated for forward compatibility).
    let _version = data[4];
    let gear = data[5] as i8;
    let speed_ms = read_f32_le(data, 6)?;
    let lateral_g = read_f32_le(data, 10)?;
    let longitudinal_g = read_f32_le(data, 14)?;
    let vertical_g = read_f32_le(data, 18)?;
    let throttle = read_f32_le(data, 22)?.clamp(0.0, 1.0);
    let brake = read_f32_le(data, 26)?.clamp(0.0, 1.0);
    let steering_angle = read_f32_le(data, 30)?.clamp(-1.0, 1.0);
    let rpm = read_f32_le(data, 34)?.max(0.0);
    let rpm_max = read_f32_le(data, 38)?.max(0.0);

    tracing::trace!(
        speed_ms,
        lateral_g,
        gear,
        rpm,
        "parsed generic racing UDP packet"
    );

    Ok(RacingTelemetry {
        speed_ms,
        lateral_g,
        longitudinal_g,
        vertical_g,
        throttle,
        brake,
        steering_angle,
        gear,
        rpm,
        rpm_max,
        is_on_track: true,
        is_valid: true,
    })
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn read_f32_le(data: &[u8], offset: usize) -> Result<f32, RacingError> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(RacingError::ReadError { offset })?
        .try_into()
        .map_err(|_| RacingError::ReadError { offset })?;
    Ok(f32::from_le_bytes(bytes))
}

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, RacingError> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(RacingError::ReadError { offset })?
        .try_into()
        .map_err(|_| RacingError::ReadError { offset })?;
    Ok(u32::from_le_bytes(bytes))
}
