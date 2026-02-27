// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! `FalconBmsAdapter` — polls flight data from a [`SharedMemoryReader`].

use crate::{FlightData, SharedMemoryReader};

/// Polls Falcon BMS flight data through a [`SharedMemoryReader`].
///
/// The last successful read is preserved through subsequent failures so callers
/// can continue working with stale data when BMS is temporarily unavailable.
pub struct FalconBmsAdapter<R: SharedMemoryReader> {
    reader: R,
    connected: bool,
    last_data: Option<FlightData>,
    read_count: u64,
    error_count: u64,
}

impl<R: SharedMemoryReader> FalconBmsAdapter<R> {
    /// Wrap `reader` in a new adapter (initially disconnected).
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            connected: false,
            last_data: None,
            read_count: 0,
            error_count: 0,
        }
    }

    /// Poll the reader for the latest flight data.
    ///
    /// Returns `Some(data)` on success; `None` when the reader is unavailable.
    /// The previous successful read is kept in [`last_data`](Self::last_data)
    /// through failures.
    pub fn poll(&mut self) -> Option<FlightData> {
        match self.reader.read_flight_data() {
            Ok(data) => {
                tracing::debug!("BMS flight data read ok");
                self.read_count += 1;
                self.connected = true;
                self.last_data = Some(data);
                Some(data)
            }
            Err(e) => {
                tracing::warn!("BMS read failed: {e}");
                self.error_count += 1;
                self.connected = false;
                None
            }
        }
    }

    /// `true` after at least one successful poll since the last failure.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Total successful reads since construction.
    pub fn read_count(&self) -> u64 {
        self.read_count
    }

    /// Total failed reads since construction.
    pub fn error_count(&self) -> u64 {
        self.error_count
    }

    /// The most recently polled flight data, if any.
    pub fn last_data(&self) -> Option<&FlightData> {
        self.last_data.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BmsError, MockSharedMemory};
    use bytemuck::Zeroable;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn make_data() -> FlightData {
        FlightData::zeroed()
    }

    #[test]
    fn test_adapter_reads_flight_data() {
        let mock = MockSharedMemory::new(Some(make_data()));
        let mut adapter = FalconBmsAdapter::new(mock);
        assert!(adapter.poll().is_some());
    }

    #[test]
    fn test_adapter_connected_when_data_available() {
        let mock = MockSharedMemory::new(Some(make_data()));
        let mut adapter = FalconBmsAdapter::new(mock);
        adapter.poll();
        assert!(adapter.is_connected());
    }

    #[test]
    fn test_adapter_disconnected_when_no_data() {
        let mock = MockSharedMemory::new(None);
        let mut adapter = FalconBmsAdapter::new(mock);
        adapter.poll();
        assert!(!adapter.is_connected());
    }

    #[test]
    fn test_read_count_increments() {
        let mock = MockSharedMemory::new(Some(make_data()));
        let mut adapter = FalconBmsAdapter::new(mock);
        adapter.poll();
        adapter.poll();
        assert_eq!(adapter.read_count(), 2);
    }

    #[test]
    fn test_error_count_increments_on_failure() {
        let mock = MockSharedMemory::new(None);
        let mut adapter = FalconBmsAdapter::new(mock);
        adapter.poll();
        adapter.poll();
        assert_eq!(adapter.error_count(), 2);
    }

    #[test]
    fn test_last_data_preserved_on_error() {
        // A reader that succeeds on the first call and fails on all subsequent ones.
        struct OnceReader {
            called: AtomicBool,
            data: FlightData,
        }
        impl SharedMemoryReader for OnceReader {
            fn read_flight_data(&self) -> Result<FlightData, BmsError> {
                if self.called.swap(true, Ordering::Relaxed) {
                    Err(BmsError::NotAvailable)
                } else {
                    Ok(self.data)
                }
            }
            fn is_available(&self) -> bool {
                !self.called.load(Ordering::Relaxed)
            }
        }

        let once = OnceReader {
            called: AtomicBool::new(false),
            data: make_data(),
        };
        let mut adapter = FalconBmsAdapter::new(once);
        assert!(adapter.poll().is_some(), "first poll must succeed");
        assert!(adapter.poll().is_none(), "second poll must fail");
        assert!(
            adapter.last_data().is_some(),
            "last_data preserved after failure"
        );
    }
}
