// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for shared memory layout validation and bytemuck correctness.
//!
//! Covers:
//! - Struct size and alignment match the 768-byte shared memory block
//! - Byte-level round-trip via `bytemuck::bytes_of` / `try_from_bytes`
//! - Reject undersized or oversized byte slices
//! - Field offsets are stable after round-trip
//! - Clone / Copy semantics

use bytemuck::{bytes_of, try_from_bytes, Zeroable};
use flight_falcon_bms::FlightData;
use std::f32::consts;

// ── Layout constants ────────────────────────────────────────────────────────

/// Expected size: 17 × f32 (68 bytes) + 700-byte pad = 768 bytes.
const EXPECTED_SIZE: usize = 768;

#[test]
fn flight_data_struct_size() {
    assert_eq!(
        std::mem::size_of::<FlightData>(),
        EXPECTED_SIZE,
        "FlightData must be exactly {EXPECTED_SIZE} bytes"
    );
}

#[test]
fn flight_data_alignment() {
    assert_eq!(
        std::mem::align_of::<FlightData>(),
        std::mem::align_of::<f32>(),
        "FlightData alignment must match f32"
    );
}

// ── Byte round-trip ─────────────────────────────────────────────────────────

#[test]
fn round_trip_zeroed() {
    let original = FlightData::zeroed();
    let bytes = bytes_of(&original);
    let restored: &FlightData = try_from_bytes(bytes).expect("round-trip zeroed");
    assert_eq!(restored.pitch, 0.0);
    assert_eq!(restored.roll, 0.0);
    assert_eq!(restored.throttle, 0.0);
}

#[test]
fn round_trip_preserves_all_fields() {
    let mut original = FlightData::zeroed();
    original.x = 1.0;
    original.y = 2.0;
    original.z = -3.0;
    original.x_dot = 10.0;
    original.y_dot = 20.0;
    original.z_dot = -30.0;
    original.alpha = 0.1;
    original.beta = 0.2;
    original.gamma = 0.3;
    original.pitch = consts::FRAC_PI_4;
    original.roll = -consts::FRAC_PI_2;
    original.yaw = consts::FRAC_PI_4;
    original.mach = 0.85;
    original.cas = 250.0;
    original.alt = 35_000.0;
    original.throttle = 0.9;
    original.rpm = 0.95;

    let bytes = bytes_of(&original);
    let restored: &FlightData = try_from_bytes(bytes).expect("round-trip with data");

    assert_eq!(restored.x, 1.0);
    assert_eq!(restored.y, 2.0);
    assert_eq!(restored.z, -3.0);
    assert_eq!(restored.x_dot, 10.0);
    assert_eq!(restored.y_dot, 20.0);
    assert_eq!(restored.z_dot, -30.0);
    assert_eq!(restored.alpha, 0.1);
    assert_eq!(restored.beta, 0.2);
    assert_eq!(restored.gamma, 0.3);
    assert_eq!(restored.pitch, consts::FRAC_PI_4);
    assert_eq!(restored.roll, -consts::FRAC_PI_2);
    assert_eq!(restored.yaw, consts::FRAC_PI_4);
    assert_eq!(restored.mach, 0.85);
    assert_eq!(restored.cas, 250.0);
    assert_eq!(restored.alt, 35_000.0);
    assert_eq!(restored.throttle, 0.9);
    assert_eq!(restored.rpm, 0.95);
}

#[test]
fn round_trip_preserves_normalisation() {
    let mut original = FlightData::zeroed();
    original.pitch = consts::FRAC_PI_2;
    original.roll = -consts::PI;
    original.yaw = consts::FRAC_PI_4;
    original.throttle = 0.75;

    let bytes = bytes_of(&original);
    let restored: &FlightData = try_from_bytes(bytes).unwrap();

    assert_eq!(
        original.pitch_normalized(),
        restored.pitch_normalized(),
        "pitch normalisation must survive round-trip"
    );
    assert_eq!(
        original.roll_normalized(),
        restored.roll_normalized(),
        "roll normalisation must survive round-trip"
    );
    assert_eq!(
        original.yaw_normalized(),
        restored.yaw_normalized(),
        "yaw normalisation must survive round-trip"
    );
    assert_eq!(
        original.throttle_normalized(),
        restored.throttle_normalized(),
        "throttle normalisation must survive round-trip"
    );
}

// ── Reject invalid byte slices ──────────────────────────────────────────────

#[test]
fn reject_empty_bytes() {
    let result = try_from_bytes::<FlightData>(&[]);
    assert!(result.is_err(), "empty slice must be rejected");
}

#[test]
fn reject_undersized_bytes() {
    let bytes = vec![0u8; EXPECTED_SIZE - 1];
    let result = try_from_bytes::<FlightData>(&bytes);
    assert!(result.is_err(), "undersized slice must be rejected");
}

#[test]
fn reject_oversized_bytes() {
    let bytes = vec![0u8; EXPECTED_SIZE + 1];
    let result = try_from_bytes::<FlightData>(&bytes);
    assert!(result.is_err(), "oversized slice must be rejected");
}

#[test]
fn reject_one_byte() {
    let result = try_from_bytes::<FlightData>(&[0xFF]);
    assert!(result.is_err());
}

#[test]
fn accept_exact_size_all_zero() {
    let bytes = vec![0u8; EXPECTED_SIZE];
    let result = try_from_bytes::<FlightData>(&bytes);
    assert!(result.is_ok(), "exact-size zeroed slice must be accepted");
}

#[test]
fn accept_exact_size_all_ones() {
    let bytes = vec![0xFFu8; EXPECTED_SIZE];
    // All 0xFF bytes are valid for f32 (NaN) — bytemuck allows it
    let result = try_from_bytes::<FlightData>(&bytes);
    assert!(result.is_ok(), "exact-size 0xFF slice must be accepted");
}

// ── Copy / Clone semantics ──────────────────────────────────────────────────

#[test]
fn copy_produces_identical_struct() {
    let mut original = FlightData::zeroed();
    original.pitch = 1.23;
    original.throttle = 0.5;
    let copy = original;
    assert_eq!(copy.pitch, original.pitch);
    assert_eq!(copy.throttle, original.throttle);
}

#[test]
fn clone_produces_identical_struct() {
    let mut original = FlightData::zeroed();
    original.alt = 10_000.0;
    original.mach = 0.72;
    let cloned = original.clone();
    assert_eq!(cloned.alt, original.alt);
    assert_eq!(cloned.mach, original.mach);
}

// ── Byte-level field offset stability ───────────────────────────────────────

#[test]
fn field_offsets_via_bytes() {
    let mut fd = FlightData::zeroed();
    // Set the first field (x) to a known bit pattern
    fd.x = f32::from_bits(0xDEAD_BEEF);
    let bytes = bytes_of(&fd);
    let first_four: [u8; 4] = bytes[0..4].try_into().unwrap();
    assert_eq!(
        u32::from_ne_bytes(first_four),
        0xDEAD_BEEF,
        "x field must be at byte offset 0"
    );
}

#[test]
fn pitch_field_at_expected_offset() {
    // pitch is the 10th f32 field → byte offset 36
    let mut fd = FlightData::zeroed();
    fd.pitch = f32::from_bits(0x4242_4242);
    let bytes = bytes_of(&fd);
    let offset = 9 * 4; // 0-indexed: fields x,y,z,x_dot,y_dot,z_dot,alpha,beta,gamma → 9 fields before pitch
    let slice: [u8; 4] = bytes[offset..offset + 4].try_into().unwrap();
    assert_eq!(
        u32::from_ne_bytes(slice),
        0x4242_4242,
        "pitch must be at byte offset {offset}"
    );
}

#[test]
fn throttle_field_at_expected_offset() {
    // throttle is the 16th f32 field → byte offset 60
    let mut fd = FlightData::zeroed();
    fd.throttle = f32::from_bits(0xAAAA_BBBB);
    let bytes = bytes_of(&fd);
    let offset = 15 * 4;
    let slice: [u8; 4] = bytes[offset..offset + 4].try_into().unwrap();
    assert_eq!(
        u32::from_ne_bytes(slice),
        0xAAAA_BBBB,
        "throttle must be at byte offset {offset}"
    );
}

#[test]
fn rpm_field_at_expected_offset() {
    // rpm is the 17th f32 field → byte offset 64
    let mut fd = FlightData::zeroed();
    fd.rpm = f32::from_bits(0x1234_5678);
    let bytes = bytes_of(&fd);
    let offset = 16 * 4;
    let slice: [u8; 4] = bytes[offset..offset + 4].try_into().unwrap();
    assert_eq!(
        u32::from_ne_bytes(slice),
        0x1234_5678,
        "rpm must be at byte offset {offset}"
    );
}

// ── Padding region ──────────────────────────────────────────────────────────

#[test]
fn padding_is_zeroed_in_new_struct() {
    let fd = FlightData::zeroed();
    assert!(
        fd._pad.iter().all(|&b| b == 0),
        "padding must be zeroed in a zeroed struct"
    );
}

#[test]
fn padding_starts_after_rpm() {
    let fd = FlightData::zeroed();
    let bytes = bytes_of(&fd);
    let pad_start = 17 * 4; // 68 bytes
    let pad_region = &bytes[pad_start..];
    assert_eq!(pad_region.len(), 700, "padding region must be 700 bytes");
    assert!(pad_region.iter().all(|&b| b == 0));
}
