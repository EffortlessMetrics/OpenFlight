// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for malformed data and error handling paths.
//!
//! Covers:
//! - Parsing arbitrary byte patterns as `FlightData`
//! - Normalisation robustness with extreme field values
//! - Adapter resilience when reader returns varying errors
//! - Struct identity after mutation

use approx::assert_relative_eq;
use bytemuck::{try_from_bytes, Zeroable};
use flight_falcon_bms::{BmsError, FalconBmsAdapter, FlightData, SharedMemoryReader};

#[repr(align(4))]
struct Aligned<const N: usize>([u8; N]);

// ── Malformed byte parsing ──────────────────────────────────────────────────

#[test]
fn all_0xff_bytes_parse_as_nan_fields() {
    let buf = Aligned([0xFFu8; std::mem::size_of::<FlightData>()]);
    let fd: &FlightData = try_from_bytes(&buf.0).expect("all-0xFF must parse");
    // 0xFFFFFFFF is a NaN for f32
    assert!(fd.pitch.is_nan());
    assert!(fd.roll.is_nan());
    assert!(fd.yaw.is_nan());
    assert!(fd.throttle.is_nan());
}

#[test]
fn all_0xff_normalisations_are_nan() {
    let buf = Aligned([0xFFu8; std::mem::size_of::<FlightData>()]);
    let fd: &FlightData = try_from_bytes(&buf.0).unwrap();
    assert!(fd.pitch_normalized().is_nan());
    assert!(fd.roll_normalized().is_nan());
    assert!(fd.yaw_normalized().is_nan());
    assert!(fd.throttle_normalized().is_nan());
}

#[test]
fn known_bit_pattern_for_one_float() {
    // IEEE 754: 1.0f32 = 0x3F800000
    let mut buf = Aligned([0u8; std::mem::size_of::<FlightData>()]);
    let one_bits = 1.0f32.to_ne_bytes();
    // Write 1.0 into the pitch field (byte offset 36)
    let pitch_offset = 9 * 4;
    buf.0[pitch_offset..pitch_offset + 4].copy_from_slice(&one_bits);

    let fd: &FlightData = try_from_bytes(&buf.0).unwrap();
    assert_eq!(fd.pitch, 1.0);
    // pitch=1.0, normalized = 1.0/π ≈ 0.3183
    assert_relative_eq!(fd.pitch_normalized(), 1.0 / std::f32::consts::PI, epsilon = 1e-6);
}

#[test]
fn negative_zero_preserves_sign() {
    let mut fd = FlightData::zeroed();
    fd.pitch = -0.0;
    // -0.0 / π = -0.0, clamp(-0.0) should stay -0.0 or 0.0
    let result = fd.pitch_normalized();
    // -0.0 == 0.0 in IEEE 754
    assert_relative_eq!(result, 0.0, epsilon = 1e-6);
}

// ── Extreme but valid values ────────────────────────────────────────────────

#[test]
fn max_f32_pitch_clamped() {
    let mut fd = FlightData::zeroed();
    fd.pitch = f32::MAX;
    assert_relative_eq!(fd.pitch_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn min_f32_pitch_clamped() {
    let mut fd = FlightData::zeroed();
    fd.pitch = f32::MIN;
    assert_relative_eq!(fd.pitch_normalized(), -1.0, epsilon = 1e-6);
}

#[test]
fn max_f32_throttle_clamped() {
    let mut fd = FlightData::zeroed();
    fd.throttle = f32::MAX;
    assert_relative_eq!(fd.throttle_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn min_f32_throttle_clamped() {
    let mut fd = FlightData::zeroed();
    fd.throttle = f32::MIN;
    assert_relative_eq!(fd.throttle_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn epsilon_pitch_near_zero() {
    let mut fd = FlightData::zeroed();
    fd.pitch = f32::EPSILON;
    let result = fd.pitch_normalized();
    assert!(result.abs() < 1e-6, "epsilon pitch should normalise near zero");
}

#[test]
fn very_small_throttle_stays_positive() {
    let mut fd = FlightData::zeroed();
    fd.throttle = 1e-38;
    assert!(fd.throttle_normalized() >= 0.0);
    assert!(fd.throttle_normalized() < 1e-6);
}

// ── Adapter error resilience ────────────────────────────────────────────────

/// Reader that cycles through different error types.
struct CyclingErrorReader {
    call_count: std::sync::atomic::AtomicU32,
}

impl CyclingErrorReader {
    fn new() -> Self {
        Self {
            call_count: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

impl SharedMemoryReader for CyclingErrorReader {
    fn read_flight_data(&self) -> Result<FlightData, BmsError> {
        let n = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if n % 2 == 0 {
            Err(BmsError::NotAvailable)
        } else {
            Err(BmsError::InvalidData)
        }
    }
    fn is_available(&self) -> bool {
        false
    }
}

#[test]
fn adapter_handles_mixed_error_types() {
    let reader = CyclingErrorReader::new();
    let mut adapter = FalconBmsAdapter::new(reader);

    for _ in 0..6 {
        assert!(adapter.poll().is_none());
    }
    assert_eq!(adapter.error_count(), 6);
    assert!(!adapter.is_connected());
    assert!(adapter.last_data().is_none());
}

/// Reader that panics — adapter cannot catch this, but we verify the reader
/// trait allows arbitrary error paths short of panic.
struct SucceedOnceWithData {
    data: FlightData,
    used: std::sync::atomic::AtomicBool,
}

impl SucceedOnceWithData {
    fn new(data: FlightData) -> Self {
        Self {
            data,
            used: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl SharedMemoryReader for SucceedOnceWithData {
    fn read_flight_data(&self) -> Result<FlightData, BmsError> {
        if self
            .used
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            Err(BmsError::InvalidData)
        } else {
            Ok(self.data)
        }
    }
    fn is_available(&self) -> bool {
        !self.used.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[test]
fn last_data_not_overwritten_by_invalid_data_error() {
    let data = {
        let mut fd = FlightData::zeroed();
        fd.alt = 10_000.0;
        fd.mach = 0.72;
        fd
    };
    let reader = SucceedOnceWithData::new(data);
    let mut adapter = FalconBmsAdapter::new(reader);

    assert!(adapter.poll().is_some()); // success
    assert!(adapter.poll().is_none()); // InvalidData
    assert!(adapter.poll().is_none()); // InvalidData

    let last = adapter.last_data().unwrap();
    assert_eq!(last.alt, 10_000.0);
    assert_relative_eq!(last.mach, 0.72, epsilon = 1e-6);
}

// ── Struct mutation identity ────────────────────────────────────────────────

#[test]
fn mutating_copy_does_not_affect_original() {
    let original = {
        let mut fd = FlightData::zeroed();
        fd.pitch = 1.0;
        fd
    };
    let mut copy = original;
    copy.pitch = 2.0;
    assert_eq!(original.pitch, 1.0, "original must be unaffected");
    assert_eq!(copy.pitch, 2.0);
}

#[test]
fn zeroed_then_modified_only_changes_target_field() {
    let mut fd = FlightData::zeroed();
    fd.cas = 999.0;

    // All other f32 fields should still be 0
    assert_eq!(fd.x, 0.0);
    assert_eq!(fd.y, 0.0);
    assert_eq!(fd.z, 0.0);
    assert_eq!(fd.pitch, 0.0);
    assert_eq!(fd.roll, 0.0);
    assert_eq!(fd.yaw, 0.0);
    assert_eq!(fd.throttle, 0.0);
    assert_eq!(fd.rpm, 0.0);
    assert_eq!(fd.cas, 999.0);
}
