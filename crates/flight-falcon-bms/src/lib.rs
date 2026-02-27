// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Falcon BMS shared memory integration for OpenFlight.
//!
//! Reads [`FlightData`] from the `BMS-Data` Windows shared memory segment to
//! extract axis positions and flight state.

pub mod adapter;
pub mod flight_data;
pub mod shared_memory;

pub use adapter::FalconBmsAdapter;
pub use flight_data::FlightData;
pub use shared_memory::{MockSharedMemory, SharedMemoryReader};

/// Errors produced by the Falcon BMS adapter.
#[derive(Debug, thiserror::Error)]
pub enum BmsError {
    /// BMS shared memory is not available (BMS not running or incompatible version).
    #[error("BMS shared memory not available")]
    NotAvailable,
    /// The data read from shared memory is structurally invalid.
    #[error("Invalid data in shared memory")]
    InvalidData,
}
