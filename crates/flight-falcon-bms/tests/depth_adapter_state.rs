// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for `FalconBmsAdapter` state machine transitions and telemetry
//! conversion.
//!
//! Covers:
//! - Adapter lifecycle: initial → connected → disconnected → reconnected
//! - Counter accuracy across many cycles
//! - `last_data` preservation semantics through failures
//! - Telemetry value fidelity through the adapter pipeline
//! - Alternating success/failure sequences

use approx::assert_relative_eq;
use bytemuck::Zeroable;
use flight_falcon_bms::{
    BmsError, FalconBmsAdapter, FlightData, MockSharedMemory, SharedMemoryReader,
};
use std::f32::consts;
use std::sync::atomic::{AtomicU32, Ordering};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn zeroed_data() -> FlightData {
    FlightData::zeroed()
}

fn data_with(f: impl FnOnce(&mut FlightData)) -> FlightData {
    let mut fd = FlightData::zeroed();
    f(&mut fd);
    fd
}

// ── Initial state ───────────────────────────────────────────────────────────

#[test]
fn initial_state_not_connected() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let adapter = FalconBmsAdapter::new(mock);
    assert!(!adapter.is_connected());
}

#[test]
fn initial_read_count_zero() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let adapter = FalconBmsAdapter::new(mock);
    assert_eq!(adapter.read_count(), 0);
}

#[test]
fn initial_error_count_zero() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let adapter = FalconBmsAdapter::new(mock);
    assert_eq!(adapter.error_count(), 0);
}

#[test]
fn initial_last_data_none() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let adapter = FalconBmsAdapter::new(mock);
    assert!(adapter.last_data().is_none());
}

// ── Connected state ─────────────────────────────────────────────────────────

#[test]
fn connected_after_successful_poll() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let mut adapter = FalconBmsAdapter::new(mock);
    adapter.poll();
    assert!(adapter.is_connected());
}

#[test]
fn read_count_increments_on_success() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let mut adapter = FalconBmsAdapter::new(mock);
    for _ in 0..5 {
        adapter.poll();
    }
    assert_eq!(adapter.read_count(), 5);
}

#[test]
fn error_count_stays_zero_on_success() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let mut adapter = FalconBmsAdapter::new(mock);
    for _ in 0..3 {
        adapter.poll();
    }
    assert_eq!(adapter.error_count(), 0);
}

// ── Disconnected state ──────────────────────────────────────────────────────

#[test]
fn disconnected_after_failed_poll() {
    let mock = MockSharedMemory::new(None);
    let mut adapter = FalconBmsAdapter::new(mock);
    adapter.poll();
    assert!(!adapter.is_connected());
}

#[test]
fn error_count_increments_on_failure() {
    let mock = MockSharedMemory::new(None);
    let mut adapter = FalconBmsAdapter::new(mock);
    for _ in 0..5 {
        adapter.poll();
    }
    assert_eq!(adapter.error_count(), 5);
}

#[test]
fn read_count_stays_zero_on_failure() {
    let mock = MockSharedMemory::new(None);
    let mut adapter = FalconBmsAdapter::new(mock);
    for _ in 0..3 {
        adapter.poll();
    }
    assert_eq!(adapter.read_count(), 0);
}

#[test]
fn poll_returns_none_on_failure() {
    let mock = MockSharedMemory::new(None);
    let mut adapter = FalconBmsAdapter::new(mock);
    assert!(adapter.poll().is_none());
}

// ── State transitions ───────────────────────────────────────────────────────

/// Reader that succeeds for `n` calls then fails forever.
struct SucceedThenFail {
    data: FlightData,
    remaining: AtomicU32,
}

impl SucceedThenFail {
    fn new(data: FlightData, successes: u32) -> Self {
        Self {
            data,
            remaining: AtomicU32::new(successes),
        }
    }
}

impl SharedMemoryReader for SucceedThenFail {
    fn read_flight_data(&self) -> Result<FlightData, BmsError> {
        let prev = self.remaining.fetch_sub(1, Ordering::Relaxed);
        if prev > 0 {
            Ok(self.data)
        } else {
            // Reset to 0 to avoid underflow accumulation
            self.remaining.store(0, Ordering::Relaxed);
            Err(BmsError::NotAvailable)
        }
    }
    fn is_available(&self) -> bool {
        self.remaining.load(Ordering::Relaxed) > 0
    }
}

#[test]
fn transition_connected_to_disconnected() {
    let reader = SucceedThenFail::new(zeroed_data(), 1);
    let mut adapter = FalconBmsAdapter::new(reader);

    assert!(adapter.poll().is_some());
    assert!(adapter.is_connected());

    assert!(adapter.poll().is_none());
    assert!(!adapter.is_connected());
}

#[test]
fn last_data_preserved_through_disconnect() {
    let data = data_with(|fd| fd.alt = 25_000.0);
    let reader = SucceedThenFail::new(data, 1);
    let mut adapter = FalconBmsAdapter::new(reader);

    adapter.poll(); // success
    adapter.poll(); // failure

    let last = adapter.last_data().expect("last_data must be preserved");
    assert_eq!(last.alt, 25_000.0);
}

#[test]
fn counters_correct_after_mixed_sequence() {
    let reader = SucceedThenFail::new(zeroed_data(), 3);
    let mut adapter = FalconBmsAdapter::new(reader);

    // 3 successes
    for _ in 0..3 {
        assert!(adapter.poll().is_some());
    }
    // 2 failures
    for _ in 0..2 {
        assert!(adapter.poll().is_none());
    }

    assert_eq!(adapter.read_count(), 3);
    assert_eq!(adapter.error_count(), 2);
}

/// Reader that alternates between success and failure.
struct AlternatingReader {
    data: FlightData,
    call_count: AtomicU32,
}

impl AlternatingReader {
    fn new(data: FlightData) -> Self {
        Self {
            data,
            call_count: AtomicU32::new(0),
        }
    }
}

impl SharedMemoryReader for AlternatingReader {
    fn read_flight_data(&self) -> Result<FlightData, BmsError> {
        let n = self.call_count.fetch_add(1, Ordering::Relaxed);
        if n % 2 == 0 {
            Ok(self.data)
        } else {
            Err(BmsError::NotAvailable)
        }
    }
    fn is_available(&self) -> bool {
        self.call_count.load(Ordering::Relaxed) % 2 == 0
    }
}

#[test]
fn alternating_success_failure_toggles_connected() {
    let reader = AlternatingReader::new(zeroed_data());
    let mut adapter = FalconBmsAdapter::new(reader);

    // Call 0: success → connected
    assert!(adapter.poll().is_some());
    assert!(adapter.is_connected());

    // Call 1: failure → disconnected
    assert!(adapter.poll().is_none());
    assert!(!adapter.is_connected());

    // Call 2: success → connected again
    assert!(adapter.poll().is_some());
    assert!(adapter.is_connected());

    // Call 3: failure → disconnected again
    assert!(adapter.poll().is_none());
    assert!(!adapter.is_connected());

    assert_eq!(adapter.read_count(), 2);
    assert_eq!(adapter.error_count(), 2);
}

#[test]
fn last_data_updates_on_each_success() {
    /// Reader that returns incrementing altitude on each call.
    struct IncrementingReader {
        call_count: AtomicU32,
    }
    impl SharedMemoryReader for IncrementingReader {
        fn read_flight_data(&self) -> Result<FlightData, BmsError> {
            let n = self.call_count.fetch_add(1, Ordering::Relaxed);
            let mut fd = FlightData::zeroed();
            fd.alt = (n + 1) as f32 * 1000.0;
            Ok(fd)
        }
        fn is_available(&self) -> bool {
            true
        }
    }

    let reader = IncrementingReader {
        call_count: AtomicU32::new(0),
    };
    let mut adapter = FalconBmsAdapter::new(reader);

    adapter.poll();
    assert_eq!(adapter.last_data().unwrap().alt, 1000.0);

    adapter.poll();
    assert_eq!(adapter.last_data().unwrap().alt, 2000.0);

    adapter.poll();
    assert_eq!(adapter.last_data().unwrap().alt, 3000.0);
}

// ── Telemetry value fidelity ────────────────────────────────────────────────

#[test]
fn polled_data_preserves_position() {
    let data = data_with(|fd| {
        fd.x = 100.0;
        fd.y = 200.0;
        fd.z = -300.0;
    });
    let mock = MockSharedMemory::new(Some(data));
    let mut adapter = FalconBmsAdapter::new(mock);
    let result = adapter.poll().unwrap();
    assert_eq!(result.x, 100.0);
    assert_eq!(result.y, 200.0);
    assert_eq!(result.z, -300.0);
}

#[test]
fn polled_data_preserves_velocity() {
    let data = data_with(|fd| {
        fd.x_dot = 10.0;
        fd.y_dot = 20.0;
        fd.z_dot = -30.0;
    });
    let mock = MockSharedMemory::new(Some(data));
    let mut adapter = FalconBmsAdapter::new(mock);
    let result = adapter.poll().unwrap();
    assert_eq!(result.x_dot, 10.0);
    assert_eq!(result.y_dot, 20.0);
    assert_eq!(result.z_dot, -30.0);
}

#[test]
fn polled_data_preserves_angles() {
    let data = data_with(|fd| {
        fd.alpha = 0.1;
        fd.beta = 0.2;
        fd.gamma = 0.3;
    });
    let mock = MockSharedMemory::new(Some(data));
    let mut adapter = FalconBmsAdapter::new(mock);
    let result = adapter.poll().unwrap();
    assert_relative_eq!(result.alpha, 0.1, epsilon = 1e-6);
    assert_relative_eq!(result.beta, 0.2, epsilon = 1e-6);
    assert_relative_eq!(result.gamma, 0.3, epsilon = 1e-6);
}

#[test]
fn polled_data_preserves_flight_instruments() {
    let data = data_with(|fd| {
        fd.mach = 0.85;
        fd.cas = 310.0;
        fd.alt = 35_000.0;
    });
    let mock = MockSharedMemory::new(Some(data));
    let mut adapter = FalconBmsAdapter::new(mock);
    let result = adapter.poll().unwrap();
    assert_relative_eq!(result.mach, 0.85, epsilon = 1e-6);
    assert_relative_eq!(result.cas, 310.0, epsilon = 1e-6);
    assert_relative_eq!(result.alt, 35_000.0, epsilon = 1e-6);
}

#[test]
fn polled_data_normalisations_match_original() {
    let data = data_with(|fd| {
        fd.pitch = consts::FRAC_PI_4;
        fd.roll = -consts::FRAC_PI_2;
        fd.yaw = consts::FRAC_PI_4;
        fd.throttle = 0.8;
    });
    let mock = MockSharedMemory::new(Some(data));
    let mut adapter = FalconBmsAdapter::new(mock);
    let result = adapter.poll().unwrap();

    assert_relative_eq!(result.pitch_normalized(), 0.25, epsilon = 1e-6);
    assert_relative_eq!(result.roll_normalized(), -0.5, epsilon = 1e-6);
    assert_relative_eq!(result.yaw_normalized(), 0.5, epsilon = 1e-6);
    assert_relative_eq!(result.throttle_normalized(), 0.8, epsilon = 1e-6);
}

// ── MockSharedMemory specifics ──────────────────────────────────────────────

#[test]
fn mock_is_available_when_data_present() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    assert!(mock.is_available());
}

#[test]
fn mock_not_available_when_data_absent() {
    let mock = MockSharedMemory::new(None);
    assert!(!mock.is_available());
}

#[test]
fn mock_read_count_increments_correctly() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    for _ in 0..10 {
        let _ = mock.read_flight_data();
    }
    assert_eq!(mock.read_count.load(Ordering::Relaxed), 10);
}

#[test]
fn mock_read_count_increments_on_failure_too() {
    let mock = MockSharedMemory::new(None);
    for _ in 0..5 {
        let _ = mock.read_flight_data();
    }
    assert_eq!(mock.read_count.load(Ordering::Relaxed), 5);
}

// ── Error variant checks ────────────────────────────────────────────────────

#[test]
fn not_available_error_has_expected_message() {
    let err = BmsError::NotAvailable;
    assert_eq!(format!("{err}"), "BMS shared memory not available");
}

#[test]
fn invalid_data_error_has_expected_message() {
    let err = BmsError::InvalidData;
    assert_eq!(format!("{err}"), "Invalid data in shared memory");
}

#[test]
fn errors_implement_debug() {
    let err = BmsError::NotAvailable;
    let debug = format!("{err:?}");
    assert!(debug.contains("NotAvailable"));
}

// ── High-volume polling ─────────────────────────────────────────────────────

#[test]
fn many_successful_polls() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let mut adapter = FalconBmsAdapter::new(mock);
    for _ in 0..1000 {
        assert!(adapter.poll().is_some());
    }
    assert_eq!(adapter.read_count(), 1000);
    assert_eq!(adapter.error_count(), 0);
    assert!(adapter.is_connected());
}

#[test]
fn many_failed_polls() {
    let mock = MockSharedMemory::new(None);
    let mut adapter = FalconBmsAdapter::new(mock);
    for _ in 0..1000 {
        assert!(adapter.poll().is_none());
    }
    assert_eq!(adapter.read_count(), 0);
    assert_eq!(adapter.error_count(), 1000);
    assert!(!adapter.is_connected());
}

// ── Adapter with custom reader returning InvalidData ────────────────────────

struct InvalidDataReader;

impl SharedMemoryReader for InvalidDataReader {
    fn read_flight_data(&self) -> Result<FlightData, BmsError> {
        Err(BmsError::InvalidData)
    }
    fn is_available(&self) -> bool {
        true
    }
}

#[test]
fn invalid_data_error_increments_error_count() {
    let mut adapter = FalconBmsAdapter::new(InvalidDataReader);
    adapter.poll();
    assert_eq!(adapter.error_count(), 1);
    assert!(!adapter.is_connected());
}

#[test]
fn invalid_data_error_returns_none() {
    let mut adapter = FalconBmsAdapter::new(InvalidDataReader);
    assert!(adapter.poll().is_none());
}
