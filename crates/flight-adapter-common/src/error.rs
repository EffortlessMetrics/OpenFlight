// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Shared adapter error types.

use thiserror::Error;

/// Generic adapter errors that can be reused across simulator integrations.
#[derive(Debug, Error)]
pub enum AdapterError {
    /// Adapter is not connected to the simulator.
    #[error("Not connected")]
    NotConnected,
    /// Connection or operation timed out.
    #[error("Timeout: {0}")]
    Timeout(String),
    /// Adapter could not detect an aircraft.
    #[error("Aircraft not detected")]
    AircraftNotDetected,
    /// Configuration or setup error.
    #[error("Configuration error: {0}")]
    Configuration(String),
    /// Reconnection attempts exhausted.
    #[error("Reconnect attempts exhausted")]
    ReconnectExhausted,
    /// Fallback error for adapter-specific failures.
    #[error("Adapter error: {0}")]
    Other(String),
}
