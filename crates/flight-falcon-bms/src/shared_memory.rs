// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! `SharedMemoryReader` trait and in-memory mock for tests.

use crate::{BmsError, FlightData};
use std::sync::atomic::{AtomicU32, Ordering};

/// Abstraction over platform-specific shared memory access.
///
/// On Windows a real implementation would use `OpenFileMapping` /
/// `MapViewOfFile`. The `Send + Sync` bound allows the reader to be used from
/// any thread.
pub trait SharedMemoryReader: Send + Sync {
    /// Read the current [`FlightData`] block from shared memory.
    fn read_flight_data(&self) -> Result<FlightData, BmsError>;

    /// Return `true` if the shared memory segment appears to be available.
    fn is_available(&self) -> bool;
}

/// In-memory mock for use in tests.
pub struct MockSharedMemory {
    /// Data returned by [`read_flight_data`], or `None` to simulate BMS not running.
    pub data: Option<FlightData>,
    /// Running count of [`read_flight_data`] calls.
    pub read_count: AtomicU32,
}

impl MockSharedMemory {
    /// Create a new mock backed by `data`.
    pub fn new(data: Option<FlightData>) -> Self {
        Self {
            data,
            read_count: AtomicU32::new(0),
        }
    }
}

impl SharedMemoryReader for MockSharedMemory {
    fn read_flight_data(&self) -> Result<FlightData, BmsError> {
        self.read_count.fetch_add(1, Ordering::Relaxed);
        self.data.ok_or(BmsError::NotAvailable)
    }

    fn is_available(&self) -> bool {
        self.data.is_some()
    }
}
