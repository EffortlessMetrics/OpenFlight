// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Integration tests for `flight-falcon-bms`.
//!
//! These tests exercise the public adapter API end-to-end using a stub
//! [`SharedMemoryReader`] so that no real shared memory is needed.

use flight_falcon_bms::{
    AircraftState, BmsAdapterError, FalconAircraftType, FalconBmsAdapter, MIN_DATA_BLOCK_SIZE,
    SharedMemoryReader, detect_aircraft_type, parse_aircraft_state,
};
use std::f32::consts::PI;

// ── Stub shared-memory reader ──────────────────────────────────────────────────

struct StubReader {
    data: Vec<u8>,
    fail_open: bool,
}

impl StubReader {
    fn with_data(data: Vec<u8>) -> Self {
        Self {
            data,
            fail_open: false,
        }
    }

    fn failing() -> Self {
        Self {
            data: vec![],
            fail_open: true,
        }
    }
}

impl SharedMemoryReader for StubReader {
    fn open(&mut self, _name: &str) -> Result<(), BmsAdapterError> {
        if self.fail_open {
            Err(BmsAdapterError::BufferTooShort { found: 0 })
        } else {
            Ok(())
        }
    }

    fn read_block(&self) -> Result<Vec<u8>, BmsAdapterError> {
        Ok(self.data.clone())
    }

    fn close(&mut self) {}
}

// ── Helper ─────────────────────────────────────────────────────────────────────

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

// ── Tests ──────────────────────────────────────────────────────────────────────

/// Zero-filled shared memory must parse to defaults without panicking.
#[test]
fn zero_filled_memory_returns_defaults_no_panic() {
    let block = vec![0u8; MIN_DATA_BLOCK_SIZE];
    let state = parse_aircraft_state(&block).expect("zero-filled block must not fail");
    assert_eq!(state, AircraftState::default());
}

/// Altitude, airspeed, and pitch (used as AoA proxy) are decoded correctly.
#[test]
fn altitude_airspeed_pitch_parsed_correctly() {
    let pitch_rad = 0.15_f32;
    let block = build_block(0.0, pitch_rad, 0.0, 280.0, 35_000.0, 0, 0, 0);
    let state = parse_aircraft_state(&block).unwrap();
    assert!(
        (state.airspeed - 280.0).abs() < 0.01,
        "airspeed={}",
        state.airspeed
    );
    assert!(
        (state.altitude - 35_000.0).abs() < 0.01,
        "altitude={}",
        state.altitude
    );
    assert!(
        (state.pitch - pitch_rad.to_degrees()).abs() < 0.01,
        "pitch={}",
        state.pitch
    );
}

/// Gear-down flag is driven exclusively by bit 0 of `gear_bits`; bits 1 and 2
/// are reserved and must not set `gear_down`.
#[test]
fn gear_state_three_bit_scenarios() {
    // Bit 0 (nose / main gear down signal) → gear_down = true.
    let block0 = build_block(0.0, 0.0, 0.0, 150.0, 1_000.0, 0b0001, 0, 0);
    assert!(
        parse_aircraft_state(&block0).unwrap().gear_down,
        "bit 0 → gear down"
    );

    // Bit 1 alone must NOT set gear_down.
    let block1 = build_block(0.0, 0.0, 0.0, 150.0, 1_000.0, 0b0010, 0, 0);
    assert!(
        !parse_aircraft_state(&block1).unwrap().gear_down,
        "bit 1 alone → gear up"
    );

    // Bit 2 alone must NOT set gear_down.
    let block2 = build_block(0.0, 0.0, 0.0, 150.0, 1_000.0, 0b0100, 0, 0);
    assert!(
        !parse_aircraft_state(&block2).unwrap().gear_down,
        "bit 2 alone → gear up"
    );
}

/// Weapons-loaded count and flaps (master-warning proxy flags) are decoded.
#[test]
fn master_caution_weapons_and_flaps_flags_decoded() {
    let block = build_block(0.0, 0.0, 0.0, 200.0, 5_000.0, 0, 1, 8);
    let state = parse_aircraft_state(&block).unwrap();
    assert!(state.flaps, "flaps bit set");
    assert_eq!(state.weapons_loaded, 8, "weapons_loaded mismatch");

    // All flags clear.
    let clean = build_block(0.0, 0.0, 0.0, 200.0, 5_000.0, 0, 0, 0);
    let clean_state = parse_aircraft_state(&clean).unwrap();
    assert!(!clean_state.flaps);
    assert_eq!(clean_state.weapons_loaded, 0);
}

/// Multiple sequential calls with the same buffer must produce identical output.
#[test]
fn multiple_sequential_reads_consistent_output() {
    let block = build_block(PI / 4.0, 0.05, -0.1, 320.0, 20_000.0, 0, 0, 2);
    let state1 = parse_aircraft_state(&block).unwrap();
    let state2 = parse_aircraft_state(&block).unwrap();
    let state3 = parse_aircraft_state(&block).unwrap();
    assert_eq!(state1, state2, "read 1 vs read 2");
    assert_eq!(state2, state3, "read 2 vs read 3");
}

/// Full adapter lifecycle: connect → poll → verify state → disconnect.
#[test]
fn adapter_full_lifecycle_connect_poll_disconnect() {
    let block = build_block(PI, 0.0, 0.0, 400.0, 40_000.0, 0, 0, 4);
    let reader = StubReader::with_data(block);
    let mut adapter = FalconBmsAdapter::new(reader);

    assert!(adapter.last_state().is_none(), "no state before first poll");

    adapter.connect().expect("connect must succeed");
    let state = adapter.poll().expect("poll must succeed");

    assert!(
        (state.heading - 180.0).abs() < 0.01,
        "heading={}",
        state.heading
    );
    assert!((state.airspeed - 400.0).abs() < 0.01);
    assert!(
        adapter.last_state().is_some(),
        "last_state cached after poll"
    );

    adapter.disconnect();
}

/// An error from the reader's `open` must propagate through `connect`.
#[test]
fn connect_error_propagated_from_reader() {
    let reader = StubReader::failing();
    let mut adapter = FalconBmsAdapter::new(reader);
    let err = adapter.connect().unwrap_err();
    assert!(matches!(err, BmsAdapterError::BufferTooShort { .. }));
}

/// All three known aircraft types and the unknown fallback resolve correctly.
#[test]
fn aircraft_type_detected_for_all_known_ids() {
    assert_eq!(detect_aircraft_type(1), FalconAircraftType::F16C);
    assert_eq!(detect_aircraft_type(2), FalconAircraftType::F16A);
    assert_eq!(detect_aircraft_type(3), FalconAircraftType::F16D);
    assert_eq!(detect_aircraft_type(0), FalconAircraftType::Unknown);
    assert_eq!(detect_aircraft_type(9_999), FalconAircraftType::Unknown);
}
