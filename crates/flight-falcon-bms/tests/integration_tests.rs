// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Integration tests for `flight-falcon-bms` using the public adapter API.

use bytemuck::Zeroable;
use flight_falcon_bms::{
    BmsError, FalconBmsAdapter, FlightData, MockSharedMemory, SharedMemoryReader,
};
use std::f32::consts;

fn zeroed_data() -> FlightData {
    FlightData::zeroed()
}

fn data_with_pitch(pitch: f32) -> FlightData {
    let mut fd = FlightData::zeroed();
    fd.pitch = pitch;
    fd
}

fn data_with_throttle(throttle: f32) -> FlightData {
    let mut fd = FlightData::zeroed();
    fd.throttle = throttle;
    fd
}

// ── Adapter lifecycle ─────────────────────────────────────────────────────────

#[test]
fn adapter_poll_returns_some_when_data_available() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let mut adapter = FalconBmsAdapter::new(mock);
    assert!(adapter.poll().is_some());
}

#[test]
fn adapter_poll_returns_none_when_no_data() {
    let mock = MockSharedMemory::new(None);
    let mut adapter = FalconBmsAdapter::new(mock);
    assert!(adapter.poll().is_none());
}

#[test]
fn adapter_last_data_none_before_first_poll() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let adapter = FalconBmsAdapter::new(mock);
    assert!(adapter.last_data().is_none());
}

#[test]
fn adapter_last_data_set_after_successful_poll() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let mut adapter = FalconBmsAdapter::new(mock);
    adapter.poll();
    assert!(adapter.last_data().is_some());
}

#[test]
fn adapter_not_connected_before_any_poll() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let adapter = FalconBmsAdapter::new(mock);
    assert!(!adapter.is_connected());
}

#[test]
fn adapter_connected_after_successful_poll() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let mut adapter = FalconBmsAdapter::new(mock);
    adapter.poll();
    assert!(adapter.is_connected());
}

#[test]
fn adapter_disconnected_after_failed_poll() {
    let mock = MockSharedMemory::new(None);
    let mut adapter = FalconBmsAdapter::new(mock);
    adapter.poll();
    assert!(!adapter.is_connected());
}

// ── Normalization via polled data ─────────────────────────────────────────────

#[test]
fn polled_pitch_normalized_correctly() {
    let mock = MockSharedMemory::new(Some(data_with_pitch(consts::FRAC_PI_2)));
    let mut adapter = FalconBmsAdapter::new(mock);
    let fd = adapter.poll().unwrap();
    let normalized = fd.pitch_normalized();
    assert!((normalized - 0.5).abs() < 1e-6, "normalized={normalized}");
}

#[test]
fn polled_throttle_clamped_to_one() {
    let mock = MockSharedMemory::new(Some(data_with_throttle(2.0)));
    let mut adapter = FalconBmsAdapter::new(mock);
    let fd = adapter.poll().unwrap();
    assert!((fd.throttle_normalized() - 1.0).abs() < 1e-6);
}

// ── MockSharedMemory read_count ───────────────────────────────────────────────

#[test]
fn mock_read_count_tracks_all_calls() {
    let mock = MockSharedMemory::new(Some(zeroed_data()));
    let mut adapter = FalconBmsAdapter::new(mock);
    adapter.poll();
    adapter.poll();
    adapter.poll();
    assert_eq!(adapter.read_count(), 3);
}

// ── Custom reader for failure-mode tests ──────────────────────────────────────

struct FailingReader;
impl SharedMemoryReader for FailingReader {
    fn read_flight_data(&self) -> Result<FlightData, BmsError> {
        Err(BmsError::NotAvailable)
    }
    fn is_available(&self) -> bool {
        false
    }
}

#[test]
fn error_count_tracks_failed_reads() {
    let mut adapter = FalconBmsAdapter::new(FailingReader);
    adapter.poll();
    adapter.poll();
    assert_eq!(adapter.error_count(), 2);
}
