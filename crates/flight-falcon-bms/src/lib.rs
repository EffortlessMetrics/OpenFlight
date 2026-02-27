// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Falcon BMS simulator adapter for OpenFlight.
//!
//! Falcon BMS exposes real-time telemetry through two Windows shared memory
//! segments:
//!
//! - `BMS-Data` — primary flight data (pitch, roll, heading, airspeed, …)
//! - `BMS-ATC-Briefing` — ATC and briefing text
//!
//! This crate provides binary parsing for the `BMS-Data` block and a
//! [`SharedMemoryReader`] trait as a placeholder for platform-specific
//! shared-memory access.
//!
//! ## Data layout
//!
//! The `BMS-Data` block is a packed, little-endian C struct.
//! The fields parsed by this crate occupy the following offsets:
//!
//! | Offset | Size | Field            | Unit    |
//! |--------|------|------------------|---------|
//! | 0      | 4    | `heading`        | radians |
//! | 4      | 4    | `pitch`          | radians |
//! | 8      | 4    | `roll`           | radians |
//! | 12     | 4    | `airspeed`       | knots   |
//! | 16     | 4    | `altitude`       | feet    |
//! | 20     | 4    | `gear_bits`      | bitmask |
//! | 24     | 4    | `flap_bits`      | bitmask |
//! | 28     | 4    | `weapons_loaded` | count   |
//! | 32     | 4    | `aircraft_id`    | u32     |
//!
//! Minimum valid block size: [`MIN_DATA_BLOCK_SIZE`] bytes.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Suppress the lint for the dep that is reserved for future real implementation.
#[allow(unused_extern_crates)]
extern crate flight_core;

/// Minimum bytes required in a valid `BMS-Data` block.
pub const MIN_DATA_BLOCK_SIZE: usize = 36;

/// Bit 0 of `gear_bits`: landing gear is down.
const GEAR_DOWN_BIT: u32 = 1 << 0;

/// Bit 0 of `flap_bits`: flaps are deployed.
const FLAPS_DEPLOYED_BIT: u32 = 1 << 0;

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors produced by the Falcon BMS adapter.
#[derive(Debug, Error, PartialEq)]
pub enum BmsAdapterError {
    /// The data buffer is shorter than [`MIN_DATA_BLOCK_SIZE`].
    #[error("data block too short: expected at least {MIN_DATA_BLOCK_SIZE} bytes, got {found}")]
    BufferTooShort { found: usize },

    /// A field could not be read at the given byte offset.
    #[error("failed to read field at offset {offset}")]
    SliceConversion { offset: usize },
}

// ── Domain types ──────────────────────────────────────────────────────────────

/// Snapshot of Falcon BMS flight state parsed from the `BMS-Data` block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AircraftState {
    /// Magnetic heading in degrees (0–360).
    pub heading: f32,
    /// Pitch angle in degrees (positive = nose-up).
    pub pitch: f32,
    /// Roll / bank angle in degrees (positive = right-wing-down).
    pub roll: f32,
    /// Indicated airspeed in knots.
    pub airspeed: f32,
    /// Altitude in feet MSL.
    pub altitude: f32,
    /// `true` when the landing gear is fully down.
    pub gear_down: bool,
    /// `true` when flaps are deployed (any position).
    pub flaps: bool,
    /// Number of weapons currently loaded.
    pub weapons_loaded: u32,
}

impl Default for AircraftState {
    fn default() -> Self {
        Self {
            heading: 0.0,
            pitch: 0.0,
            roll: 0.0,
            airspeed: 0.0,
            altitude: 0.0,
            gear_down: false,
            flaps: false,
            weapons_loaded: 0,
        }
    }
}

/// Aircraft types modelled in Falcon BMS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FalconAircraftType {
    /// F-16C Block 50 / 52.
    F16C,
    /// F-16A (earlier blocks, e.g. Block 15 / 20).
    F16A,
    /// F-16D two-seat trainer / combat variant.
    F16D,
    /// Vehicle ID not recognised by this adapter.
    Unknown,
}

// ── SharedMemoryReader trait ──────────────────────────────────────────────────

/// Placeholder trait for platform-specific shared memory access.
///
/// On Windows a real implementation would use `OpenFileMapping` /
/// `MapViewOfFile`. On non-Windows platforms a stub that always returns
/// [`BmsAdapterError::BufferTooShort`] is expected.
pub trait SharedMemoryReader {
    /// Open (or attach to) the shared memory segment identified by `name`.
    fn open(&mut self, name: &str) -> Result<(), BmsAdapterError>;

    /// Read the entire contents of the mapped block.
    fn read_block(&self) -> Result<Vec<u8>, BmsAdapterError>;

    /// Close and unmap the segment.
    fn close(&mut self);
}

// ── Adapter ───────────────────────────────────────────────────────────────────

/// Falcon BMS adapter.
///
/// Wraps a [`SharedMemoryReader`] and exposes a polling API.  In a real
/// deployment the reader is backed by Windows shared memory; in tests a
/// `Vec<u8>` stub is sufficient.
pub struct FalconBmsAdapter<R: SharedMemoryReader> {
    reader: R,
    last_state: Option<AircraftState>,
}

impl<R: SharedMemoryReader> FalconBmsAdapter<R> {
    /// Create a new adapter wrapping `reader` (not yet connected).
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            last_state: None,
        }
    }

    /// Open the `BMS-Data` shared memory segment.
    pub fn connect(&mut self) -> Result<(), BmsAdapterError> {
        tracing::info!("connecting to BMS-Data shared memory");
        self.reader.open("BMS-Data")
    }

    /// Read the latest flight state from shared memory.
    ///
    /// Caches the result; retrieve it later with [`last_state`](Self::last_state).
    pub fn poll(&mut self) -> Result<AircraftState, BmsAdapterError> {
        tracing::debug!("polling BMS-Data shared memory");
        let block = self.reader.read_block()?;
        let state = parse_aircraft_state(&block)?;
        self.last_state = Some(state.clone());
        Ok(state)
    }

    /// Return the most recently polled state, if any.
    pub fn last_state(&self) -> Option<&AircraftState> {
        self.last_state.as_ref()
    }

    /// Disconnect from shared memory.
    pub fn disconnect(&mut self) {
        tracing::info!("disconnecting from BMS-Data shared memory");
        self.reader.close();
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

/// Parse an [`AircraftState`] from a raw `BMS-Data` block.
///
/// Angles stored as radians in the block are converted to degrees on output.
/// All multi-byte values are read as little-endian, matching the native
/// Windows `x86` / `x86-64` layout produced by BMS.
///
/// # Errors
///
/// Returns [`BmsAdapterError::BufferTooShort`] when
/// `data.len() < MIN_DATA_BLOCK_SIZE`.
pub fn parse_aircraft_state(data: &[u8]) -> Result<AircraftState, BmsAdapterError> {
    if data.len() < MIN_DATA_BLOCK_SIZE {
        return Err(BmsAdapterError::BufferTooShort { found: data.len() });
    }

    let heading_rad = read_f32_le(data, 0)?;
    let pitch_rad = read_f32_le(data, 4)?;
    let roll_rad = read_f32_le(data, 8)?;
    let airspeed = read_f32_le(data, 12)?;
    let altitude = read_f32_le(data, 16)?;
    let gear_bits = read_u32_le(data, 20)?;
    let flap_bits = read_u32_le(data, 24)?;
    let weapons_loaded = read_u32_le(data, 28)?;

    let state = AircraftState {
        heading: heading_rad.to_degrees(),
        pitch: pitch_rad.to_degrees(),
        roll: roll_rad.to_degrees(),
        airspeed,
        altitude,
        gear_down: (gear_bits & GEAR_DOWN_BIT) != 0,
        flaps: (flap_bits & FLAPS_DEPLOYED_BIT) != 0,
        weapons_loaded,
    };

    tracing::trace!(
        heading = state.heading,
        pitch = state.pitch,
        airspeed = state.airspeed,
        "parsed BMS aircraft state"
    );

    Ok(state)
}

/// Map a BMS vehicle-type ID to a [`FalconAircraftType`].
///
/// IDs are representative values from the BMS aircraft database.
/// Unknown IDs map to [`FalconAircraftType::Unknown`].
pub fn detect_aircraft_type(id: u32) -> FalconAircraftType {
    match id {
        1 => FalconAircraftType::F16C,
        2 => FalconAircraftType::F16A,
        3 => FalconAircraftType::F16D,
        _ => FalconAircraftType::Unknown,
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn read_f32_le(data: &[u8], offset: usize) -> Result<f32, BmsAdapterError> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(BmsAdapterError::SliceConversion { offset })?
        .try_into()
        .map_err(|_| BmsAdapterError::SliceConversion { offset })?;
    Ok(f32::from_le_bytes(bytes))
}

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, BmsAdapterError> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(BmsAdapterError::SliceConversion { offset })?
        .try_into()
        .map_err(|_| BmsAdapterError::SliceConversion { offset })?;
    Ok(u32::from_le_bytes(bytes))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    /// Build a minimal valid `BMS-Data` block from individual fields.
    fn build_block(
        heading_rad: f32,
        pitch_rad: f32,
        roll_rad: f32,
        airspeed: f32,
        altitude: f32,
        gear_bits: u32,
        flap_bits: u32,
        weapons_loaded: u32,
    ) -> Vec<u8> {
        let mut buf = vec![0u8; MIN_DATA_BLOCK_SIZE];
        buf[0..4].copy_from_slice(&heading_rad.to_le_bytes());
        buf[4..8].copy_from_slice(&pitch_rad.to_le_bytes());
        buf[8..12].copy_from_slice(&roll_rad.to_le_bytes());
        buf[12..16].copy_from_slice(&airspeed.to_le_bytes());
        buf[16..20].copy_from_slice(&altitude.to_le_bytes());
        buf[20..24].copy_from_slice(&gear_bits.to_le_bytes());
        buf[24..28].copy_from_slice(&flap_bits.to_le_bytes());
        buf[28..32].copy_from_slice(&weapons_loaded.to_le_bytes());
        buf
    }

    // ── parse_aircraft_state ─────────────────────────────────────────────────

    #[test]
    fn parse_basic_flight_state() {
        let block = build_block(PI / 2.0, 0.1, -0.2, 350.0, 25_000.0, 0, 0, 4);
        let state = parse_aircraft_state(&block).unwrap();
        // π/2 rad = 90°
        assert!(
            (state.heading - 90.0).abs() < 0.01,
            "heading={}",
            state.heading
        );
        assert!((state.airspeed - 350.0).abs() < 0.01);
        assert!((state.altitude - 25_000.0).abs() < 0.01);
        assert_eq!(state.weapons_loaded, 4);
        assert!(!state.gear_down);
        assert!(!state.flaps);
    }

    #[test]
    fn buffer_too_short_returns_error() {
        let short = vec![0u8; MIN_DATA_BLOCK_SIZE - 1];
        let err = parse_aircraft_state(&short).unwrap_err();
        assert_eq!(
            err,
            BmsAdapterError::BufferTooShort {
                found: MIN_DATA_BLOCK_SIZE - 1
            }
        );
    }

    #[test]
    fn empty_buffer_returns_error() {
        let err = parse_aircraft_state(&[]).unwrap_err();
        assert!(matches!(err, BmsAdapterError::BufferTooShort { found: 0 }));
    }

    #[test]
    fn heading_converted_from_radians_to_degrees() {
        // π rad = 180°
        let block = build_block(PI, 0.0, 0.0, 0.0, 0.0, 0, 0, 0);
        let state = parse_aircraft_state(&block).unwrap();
        assert!(
            (state.heading - 180.0).abs() < 0.01,
            "heading={}",
            state.heading
        );
    }

    #[test]
    fn pitch_and_roll_converted_from_radians() {
        let pitch_rad = 0.2_f32;
        let roll_rad = -0.5_f32;
        let block = build_block(0.0, pitch_rad, roll_rad, 0.0, 0.0, 0, 0, 0);
        let state = parse_aircraft_state(&block).unwrap();
        assert!((state.pitch - pitch_rad.to_degrees()).abs() < 0.001);
        assert!((state.roll - roll_rad.to_degrees()).abs() < 0.001);
    }

    #[test]
    fn gear_down_bit_set() {
        let block = build_block(0.0, 0.0, 0.0, 150.0, 1_000.0, 1, 0, 0);
        let state = parse_aircraft_state(&block).unwrap();
        assert!(state.gear_down);
    }

    #[test]
    fn gear_up_bit_clear() {
        let block = build_block(0.0, 0.0, 0.0, 150.0, 1_000.0, 0, 0, 0);
        let state = parse_aircraft_state(&block).unwrap();
        assert!(!state.gear_down);
    }

    #[test]
    fn flaps_deployed_bit_set() {
        let block = build_block(0.0, 0.0, 0.0, 200.0, 5_000.0, 0, 1, 0);
        let state = parse_aircraft_state(&block).unwrap();
        assert!(state.flaps);
    }

    #[test]
    fn flaps_not_deployed_bit_clear() {
        let block = build_block(0.0, 0.0, 0.0, 200.0, 5_000.0, 0, 0, 0);
        let state = parse_aircraft_state(&block).unwrap();
        assert!(!state.flaps);
    }

    // ── detect_aircraft_type ─────────────────────────────────────────────────

    #[test]
    fn detect_f16c_by_id() {
        assert_eq!(detect_aircraft_type(1), FalconAircraftType::F16C);
    }

    #[test]
    fn detect_f16a_by_id() {
        assert_eq!(detect_aircraft_type(2), FalconAircraftType::F16A);
    }

    #[test]
    fn detect_f16d_by_id() {
        assert_eq!(detect_aircraft_type(3), FalconAircraftType::F16D);
    }

    #[test]
    fn detect_unknown_aircraft_type() {
        assert_eq!(detect_aircraft_type(0), FalconAircraftType::Unknown);
        assert_eq!(detect_aircraft_type(9_999), FalconAircraftType::Unknown);
    }

    // ── AircraftState ────────────────────────────────────────────────────────

    #[test]
    fn aircraft_state_default_values() {
        let state = AircraftState::default();
        assert_eq!(state.heading, 0.0);
        assert_eq!(state.pitch, 0.0);
        assert_eq!(state.roll, 0.0);
        assert_eq!(state.airspeed, 0.0);
        assert_eq!(state.altitude, 0.0);
        assert!(!state.gear_down);
        assert!(!state.flaps);
        assert_eq!(state.weapons_loaded, 0);
    }

    #[test]
    fn aircraft_state_serde_round_trip() {
        let state = AircraftState {
            heading: 270.0,
            pitch: -3.5,
            roll: 12.0,
            airspeed: 450.0,
            altitude: 30_000.0,
            gear_down: false,
            flaps: true,
            weapons_loaded: 6,
        };
        let json = serde_json::to_string(&state).expect("serialize");
        let back: AircraftState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, state);
    }

    // ── Oversized buffer ────────────────────────────────────────────────────

    #[test]
    fn larger_buffer_is_accepted() {
        // BMS-Data is several kilobytes; extra bytes must not cause an error.
        let mut block = build_block(0.0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0);
        block.extend_from_slice(&[0u8; 512]);
        assert!(parse_aircraft_state(&block).is_ok());
    }
}
