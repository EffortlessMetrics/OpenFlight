// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Comprehensive depth tests for `flight-falcon-bms`.
//!
//! Covers areas complementary to the existing depth test files:
//! - Trait bound verification (Send, Sync, Copy, Clone, Debug)
//! - Normalisation symmetry and antisymmetry properties
//! - Roll / yaw edge cases (Inf, -Inf, subnormal) matching pitch coverage
//! - Adapter reconnection lifecycle and state consistency
//! - RPM, velocity, and position fields through the adapter pipeline
//! - Numerical stability at IEEE 754 boundaries
//! - Error type ergonomics (Send, Sync, source chain)
//! - `MockSharedMemory` edge cases

use approx::assert_relative_eq;
use bytemuck::Zeroable;
use flight_falcon_bms::{
    BmsError, FalconBmsAdapter, FlightData, MockSharedMemory, SharedMemoryReader,
};
use std::f32::consts;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn zeroed() -> FlightData {
    FlightData::zeroed()
}

fn with(f: impl FnOnce(&mut FlightData)) -> FlightData {
    let mut fd = zeroed();
    f(&mut fd);
    fd
}

// ── Trait bound verification ────────────────────────────────────────────────

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

#[test]
fn flight_data_is_send() {
    assert_send::<FlightData>();
}

#[test]
fn flight_data_is_sync() {
    assert_sync::<FlightData>();
}

#[test]
fn bms_error_is_send() {
    assert_send::<BmsError>();
}

#[test]
fn bms_error_is_sync() {
    assert_sync::<BmsError>();
}

#[test]
fn mock_shared_memory_is_send() {
    assert_send::<MockSharedMemory>();
}

#[test]
fn mock_shared_memory_is_sync() {
    assert_sync::<MockSharedMemory>();
}

#[test]
fn adapter_with_mock_is_send() {
    assert_send::<FalconBmsAdapter<MockSharedMemory>>();
}

#[test]
fn flight_data_debug_output_contains_fields() {
    let fd = with(|fd| {
        fd.pitch = 1.0;
        fd.alt = 5000.0;
    });
    let debug = format!("{fd:?}");
    assert!(debug.contains("pitch"), "Debug must mention pitch");
    assert!(debug.contains("alt"), "Debug must mention alt");
}

#[test]
fn bms_error_implements_std_error() {
    fn assert_error<T: std::error::Error>() {}
    assert_error::<BmsError>();
}

// ── Normalisation symmetry / antisymmetry ───────────────────────────────────

#[test]
fn pitch_normalisation_is_antisymmetric() {
    for &angle in &[0.5, 1.0, consts::FRAC_PI_4, consts::FRAC_PI_2, 2.5] {
        let pos = with(|fd| fd.pitch = angle).pitch_normalized();
        let neg = with(|fd| fd.pitch = -angle).pitch_normalized();
        assert_relative_eq!(pos, -neg, epsilon = 1e-6);
    }
}

#[test]
fn roll_normalisation_is_antisymmetric() {
    for &angle in &[0.5, 1.0, consts::FRAC_PI_4, consts::FRAC_PI_2, 2.5] {
        let pos = with(|fd| fd.roll = angle).roll_normalized();
        let neg = with(|fd| fd.roll = -angle).roll_normalized();
        assert_relative_eq!(pos, -neg, epsilon = 1e-6);
    }
}

#[test]
fn yaw_normalisation_is_antisymmetric() {
    for &angle in &[0.25, 0.5, consts::FRAC_PI_4, 1.0] {
        let pos = with(|fd| fd.yaw = angle).yaw_normalized();
        let neg = with(|fd| fd.yaw = -angle).yaw_normalized();
        assert_relative_eq!(pos, -neg, epsilon = 1e-6);
    }
}

#[test]
fn pitch_and_roll_use_same_scale() {
    let angle = consts::FRAC_PI_4;
    let p = with(|fd| fd.pitch = angle).pitch_normalized();
    let r = with(|fd| fd.roll = angle).roll_normalized();
    assert_relative_eq!(p, r, epsilon = 1e-6);
}

#[test]
fn yaw_has_tighter_range_than_pitch() {
    // Same raw angle should produce a larger normalised value for yaw (÷π/2)
    // than for pitch (÷π).
    let angle = consts::FRAC_PI_4;
    let p = with(|fd| fd.pitch = angle).pitch_normalized();
    let y = with(|fd| fd.yaw = angle).yaw_normalized();
    assert!(y > p, "yaw normalised ({y}) must exceed pitch normalised ({p})");
}

// ── Roll edge cases (mirroring pitch coverage) ──────────────────────────────

#[test]
fn roll_inf_clamped_to_one() {
    let fd = with(|fd| fd.roll = f32::INFINITY);
    assert_relative_eq!(fd.roll_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn roll_neg_inf_clamped_to_minus_one() {
    let fd = with(|fd| fd.roll = f32::NEG_INFINITY);
    assert_relative_eq!(fd.roll_normalized(), -1.0, epsilon = 1e-6);
}

#[test]
fn roll_subnormal_near_zero() {
    let fd = with(|fd| fd.roll = f32::MIN_POSITIVE);
    assert_relative_eq!(fd.roll_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn roll_max_f32_clamped() {
    let fd = with(|fd| fd.roll = f32::MAX);
    assert_relative_eq!(fd.roll_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn roll_min_f32_clamped() {
    let fd = with(|fd| fd.roll = f32::MIN);
    assert_relative_eq!(fd.roll_normalized(), -1.0, epsilon = 1e-6);
}

// ── Yaw edge cases ──────────────────────────────────────────────────────────

#[test]
fn yaw_nan_returns_nan() {
    let fd = with(|fd| fd.yaw = f32::NAN);
    assert!(fd.yaw_normalized().is_nan());
}

#[test]
fn yaw_neg_inf_clamped() {
    let fd = with(|fd| fd.yaw = f32::NEG_INFINITY);
    assert_relative_eq!(fd.yaw_normalized(), -1.0, epsilon = 1e-6);
}

#[test]
fn yaw_subnormal_near_zero() {
    let fd = with(|fd| fd.yaw = f32::MIN_POSITIVE);
    assert_relative_eq!(fd.yaw_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn yaw_max_f32_clamped() {
    let fd = with(|fd| fd.yaw = f32::MAX);
    assert_relative_eq!(fd.yaw_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn yaw_min_f32_clamped() {
    let fd = with(|fd| fd.yaw = f32::MIN);
    assert_relative_eq!(fd.yaw_normalized(), -1.0, epsilon = 1e-6);
}

// ── Throttle edge cases ─────────────────────────────────────────────────────

#[test]
fn throttle_inf_clamped_to_one() {
    let fd = with(|fd| fd.throttle = f32::INFINITY);
    assert_relative_eq!(fd.throttle_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn throttle_neg_inf_clamped_to_zero() {
    let fd = with(|fd| fd.throttle = f32::NEG_INFINITY);
    assert_relative_eq!(fd.throttle_normalized(), 0.0, epsilon = 1e-6);
}

// ── Adapter reconnection lifecycle ──────────────────────────────────────────

/// A reader that can be toggled between available and unavailable at runtime.
struct ToggleReader {
    data: FlightData,
    available: AtomicBool,
    call_count: AtomicU32,
}

impl ToggleReader {
    fn new(data: FlightData) -> Self {
        Self {
            data,
            available: AtomicBool::new(true),
            call_count: AtomicU32::new(0),
        }
    }

    fn set_available(&self, v: bool) {
        self.available.store(v, Ordering::Relaxed);
    }
}

impl SharedMemoryReader for ToggleReader {
    fn read_flight_data(&self) -> Result<FlightData, BmsError> {
        self.call_count.fetch_add(1, Ordering::Relaxed);
        if self.available.load(Ordering::Relaxed) {
            Ok(self.data)
        } else {
            Err(BmsError::NotAvailable)
        }
    }
    fn is_available(&self) -> bool {
        self.available.load(Ordering::Relaxed)
    }
}

#[test]
fn reconnection_restores_connected_state() {
    let data = with(|fd| fd.alt = 10_000.0);
    let reader = ToggleReader::new(data);
    let mut adapter = FalconBmsAdapter::new(&reader);

    // Phase 1: connected
    assert!(adapter.poll().is_some());
    assert!(adapter.is_connected());

    // Phase 2: disconnect
    reader.set_available(false);
    assert!(adapter.poll().is_none());
    assert!(!adapter.is_connected());

    // Phase 3: reconnect
    reader.set_available(true);
    assert!(adapter.poll().is_some());
    assert!(adapter.is_connected());
}

#[test]
fn reconnection_updates_last_data() {
    let data_a = with(|fd| fd.alt = 5_000.0);
    let reader = ToggleReader::new(data_a);
    let mut adapter = FalconBmsAdapter::new(&reader);

    adapter.poll(); // alt = 5000
    assert_eq!(adapter.last_data().unwrap().alt, 5_000.0);

    reader.set_available(false);
    adapter.poll(); // failure — last_data preserved
    assert_eq!(adapter.last_data().unwrap().alt, 5_000.0);

    reader.set_available(true);
    adapter.poll(); // success — last_data still 5000 since reader returns same data
    assert_eq!(adapter.last_data().unwrap().alt, 5_000.0);
}

#[test]
fn counters_accumulate_across_reconnections() {
    let reader = ToggleReader::new(zeroed());
    let mut adapter = FalconBmsAdapter::new(&reader);

    // 3 successes
    for _ in 0..3 {
        adapter.poll();
    }
    // 2 failures
    reader.set_available(false);
    for _ in 0..2 {
        adapter.poll();
    }
    // 4 more successes
    reader.set_available(true);
    for _ in 0..4 {
        adapter.poll();
    }

    assert_eq!(adapter.read_count(), 7);
    assert_eq!(adapter.error_count(), 2);
}

// We need a reference-based SharedMemoryReader impl for ToggleReader
impl SharedMemoryReader for &ToggleReader {
    fn read_flight_data(&self) -> Result<FlightData, BmsError> {
        (**self).read_flight_data()
    }
    fn is_available(&self) -> bool {
        (**self).is_available()
    }
}

// ── RPM and velocity through adapter pipeline ───────────────────────────────

#[test]
fn polled_data_preserves_rpm() {
    let data = with(|fd| fd.rpm = 0.87);
    let mock = MockSharedMemory::new(Some(data));
    let mut adapter = FalconBmsAdapter::new(mock);
    let result = adapter.poll().unwrap();
    assert_relative_eq!(result.rpm, 0.87, epsilon = 1e-6);
}

#[test]
fn polled_data_preserves_mach() {
    let data = with(|fd| fd.mach = 1.2);
    let mock = MockSharedMemory::new(Some(data));
    let mut adapter = FalconBmsAdapter::new(mock);
    let result = adapter.poll().unwrap();
    assert_relative_eq!(result.mach, 1.2, epsilon = 1e-6);
}

#[test]
fn polled_data_preserves_all_velocity_components() {
    let data = with(|fd| {
        fd.x_dot = 150.0;
        fd.y_dot = -75.0;
        fd.z_dot = 3.5;
    });
    let mock = MockSharedMemory::new(Some(data));
    let mut adapter = FalconBmsAdapter::new(mock);
    let result = adapter.poll().unwrap();
    assert_eq!(result.x_dot, 150.0);
    assert_eq!(result.y_dot, -75.0);
    assert_relative_eq!(result.z_dot, 3.5, epsilon = 1e-6);
}

#[test]
fn polled_data_preserves_aero_angles() {
    let data = with(|fd| {
        fd.alpha = 0.15;
        fd.beta = -0.05;
        fd.gamma = 0.02;
    });
    let mock = MockSharedMemory::new(Some(data));
    let mut adapter = FalconBmsAdapter::new(mock);
    let result = adapter.poll().unwrap();
    assert_relative_eq!(result.alpha, 0.15, epsilon = 1e-6);
    assert_relative_eq!(result.beta, -0.05, epsilon = 1e-6);
    assert_relative_eq!(result.gamma, 0.02, epsilon = 1e-6);
}

// ── Full-field round-trip through adapter ───────────────────────────────────

#[test]
fn all_17_fields_survive_adapter_poll() {
    let data = with(|fd| {
        fd.x = 1.0;
        fd.y = 2.0;
        fd.z = 3.0;
        fd.x_dot = 4.0;
        fd.y_dot = 5.0;
        fd.z_dot = 6.0;
        fd.alpha = 7.0;
        fd.beta = 8.0;
        fd.gamma = 9.0;
        fd.pitch = 10.0;
        fd.roll = 11.0;
        fd.yaw = 12.0;
        fd.mach = 13.0;
        fd.cas = 14.0;
        fd.alt = 15.0;
        fd.throttle = 16.0;
        fd.rpm = 17.0;
    });
    let mock = MockSharedMemory::new(Some(data));
    let mut adapter = FalconBmsAdapter::new(mock);
    let r = adapter.poll().unwrap();
    assert_eq!(r.x, 1.0);
    assert_eq!(r.y, 2.0);
    assert_eq!(r.z, 3.0);
    assert_eq!(r.x_dot, 4.0);
    assert_eq!(r.y_dot, 5.0);
    assert_eq!(r.z_dot, 6.0);
    assert_eq!(r.alpha, 7.0);
    assert_eq!(r.beta, 8.0);
    assert_eq!(r.gamma, 9.0);
    assert_eq!(r.pitch, 10.0);
    assert_eq!(r.roll, 11.0);
    assert_eq!(r.yaw, 12.0);
    assert_eq!(r.mach, 13.0);
    assert_eq!(r.cas, 14.0);
    assert_eq!(r.alt, 15.0);
    assert_eq!(r.throttle, 16.0);
    assert_eq!(r.rpm, 17.0);
}

// ── Numerical stability ─────────────────────────────────────────────────────

#[test]
fn negative_zero_pitch_normalises_to_zero() {
    let fd = with(|fd| fd.pitch = -0.0);
    assert_relative_eq!(fd.pitch_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn negative_zero_roll_normalises_to_zero() {
    let fd = with(|fd| fd.roll = -0.0);
    assert_relative_eq!(fd.roll_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn negative_zero_yaw_normalises_to_zero() {
    let fd = with(|fd| fd.yaw = -0.0);
    assert_relative_eq!(fd.yaw_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn negative_zero_throttle_normalises_to_zero() {
    let fd = with(|fd| fd.throttle = -0.0);
    assert_relative_eq!(fd.throttle_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn very_small_positive_angles_normalise_near_zero() {
    let tiny = 1e-30_f32;
    assert_relative_eq!(with(|fd| fd.pitch = tiny).pitch_normalized(), 0.0, epsilon = 1e-6);
    assert_relative_eq!(with(|fd| fd.roll = tiny).roll_normalized(), 0.0, epsilon = 1e-6);
    assert_relative_eq!(with(|fd| fd.yaw = tiny).yaw_normalized(), 0.0, epsilon = 1e-6);
}

// ── Error type ergonomics ───────────────────────────────────────────────────

#[test]
fn bms_error_not_available_display() {
    let msg = format!("{}", BmsError::NotAvailable);
    assert_eq!(msg, "BMS shared memory not available");
}

#[test]
fn bms_error_invalid_data_display() {
    let msg = format!("{}", BmsError::InvalidData);
    assert_eq!(msg, "Invalid data in shared memory");
}

#[test]
fn bms_error_variants_are_distinct_in_debug() {
    let a = format!("{:?}", BmsError::NotAvailable);
    let b = format!("{:?}", BmsError::InvalidData);
    assert_ne!(a, b, "error variants must have distinct debug output");
}

// ── MockSharedMemory edge cases ─────────────────────────────────────────────

#[test]
fn mock_returns_same_data_on_repeated_reads() {
    let data = with(|fd| fd.alt = 42_000.0);
    let mock = MockSharedMemory::new(Some(data));
    for _ in 0..5 {
        let fd = mock.read_flight_data().unwrap();
        assert_eq!(fd.alt, 42_000.0);
    }
}

#[test]
fn mock_none_always_returns_not_available() {
    let mock = MockSharedMemory::new(None);
    for _ in 0..5 {
        let err = mock.read_flight_data().unwrap_err();
        assert_eq!(format!("{err}"), "BMS shared memory not available");
    }
}

#[test]
fn mock_read_count_matches_total_attempts() {
    let mock = MockSharedMemory::new(Some(zeroed()));
    for _ in 0..7 {
        let _ = mock.read_flight_data();
    }
    assert_eq!(mock.read_count.load(Ordering::Relaxed), 7);
}

// ── Adapter: multiple polls with data verification ──────────────────────────

/// Reader returning different altitudes on each call.
struct AltitudeSequenceReader {
    altitudes: Vec<f32>,
    index: AtomicU32,
}

impl SharedMemoryReader for AltitudeSequenceReader {
    fn read_flight_data(&self) -> Result<FlightData, BmsError> {
        let i = self.index.fetch_add(1, Ordering::Relaxed) as usize;
        if i < self.altitudes.len() {
            Ok(with(|fd| fd.alt = self.altitudes[i]))
        } else {
            Err(BmsError::NotAvailable)
        }
    }
    fn is_available(&self) -> bool {
        (self.index.load(Ordering::Relaxed) as usize) < self.altitudes.len()
    }
}

#[test]
fn adapter_receives_sequential_data_correctly() {
    let reader = AltitudeSequenceReader {
        altitudes: vec![1000.0, 2000.0, 3000.0, 4000.0, 5000.0],
        index: AtomicU32::new(0),
    };
    let mut adapter = FalconBmsAdapter::new(reader);

    for expected in [1000.0, 2000.0, 3000.0, 4000.0, 5000.0] {
        let fd = adapter.poll().unwrap();
        assert_eq!(fd.alt, expected);
    }

    // Exhausted — next poll fails
    assert!(adapter.poll().is_none());
    // But last_data preserved
    assert_eq!(adapter.last_data().unwrap().alt, 5000.0);
}

#[test]
fn adapter_last_data_reflects_most_recent_success_not_first() {
    let reader = AltitudeSequenceReader {
        altitudes: vec![100.0, 200.0],
        index: AtomicU32::new(0),
    };
    let mut adapter = FalconBmsAdapter::new(reader);

    adapter.poll(); // alt=100
    adapter.poll(); // alt=200
    adapter.poll(); // failure

    assert_eq!(adapter.last_data().unwrap().alt, 200.0);
}

// ── Normalisation consistency across all four axes ──────────────────────────

#[test]
fn zero_input_produces_zero_for_all_normalisations() {
    let fd = zeroed();
    assert_eq!(fd.pitch_normalized(), 0.0);
    assert_eq!(fd.roll_normalized(), 0.0);
    assert_eq!(fd.yaw_normalized(), 0.0);
    assert_eq!(fd.throttle_normalized(), 0.0);
}

#[test]
fn positive_saturation_for_all_angle_normalisations() {
    // Give each axis a value well beyond its ±1.0 normalised range.
    let fd = with(|fd| {
        fd.pitch = 100.0;
        fd.roll = 100.0;
        fd.yaw = 100.0;
    });
    assert_relative_eq!(fd.pitch_normalized(), 1.0, epsilon = 1e-6);
    assert_relative_eq!(fd.roll_normalized(), 1.0, epsilon = 1e-6);
    assert_relative_eq!(fd.yaw_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn negative_saturation_for_all_angle_normalisations() {
    let fd = with(|fd| {
        fd.pitch = -100.0;
        fd.roll = -100.0;
        fd.yaw = -100.0;
    });
    assert_relative_eq!(fd.pitch_normalized(), -1.0, epsilon = 1e-6);
    assert_relative_eq!(fd.roll_normalized(), -1.0, epsilon = 1e-6);
    assert_relative_eq!(fd.yaw_normalized(), -1.0, epsilon = 1e-6);
}

// ── Realistic flight scenario values ────────────────────────────────────────

#[test]
fn realistic_cruise_data_normalisations() {
    let fd = with(|fd| {
        fd.pitch = 0.05;  // ~3° nose up
        fd.roll = 0.0;    // wings level
        fd.yaw = 0.0;     // no sideslip
        fd.throttle = 0.7; // cruise power
        fd.mach = 0.82;
        fd.cas = 280.0;
        fd.alt = 35_000.0;
    });

    let pn = fd.pitch_normalized();
    assert!(pn > 0.0 && pn < 0.1, "cruise pitch should be small positive: {pn}");
    assert_relative_eq!(fd.roll_normalized(), 0.0, epsilon = 1e-6);
    assert_relative_eq!(fd.throttle_normalized(), 0.7, epsilon = 1e-6);
}

#[test]
fn realistic_steep_turn_normalisations() {
    let fd = with(|fd| {
        fd.pitch = 0.1;                  // slight nose up
        fd.roll = consts::FRAC_PI_4;     // 45° bank
        fd.yaw = 0.02;                   // slight coordination
        fd.throttle = 0.85;
    });

    assert_relative_eq!(fd.roll_normalized(), 0.25, epsilon = 1e-6);
    assert!(fd.throttle_normalized() > 0.8);
}
