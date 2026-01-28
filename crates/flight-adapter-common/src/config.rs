// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Shared adapter configuration contract.

use std::time::Duration;

/// Configuration contract for simulator adapters.
///
/// This trait standardizes common adapter controls without forcing
/// a specific configuration struct layout.
pub trait AdapterConfig {
    /// Telemetry publishing rate in Hertz.
    fn publish_rate_hz(&self) -> f32;

    /// Connection timeout for the adapter to establish or verify connectivity.
    fn connection_timeout(&self) -> Duration;

    /// Maximum number of reconnection attempts before giving up.
    fn max_reconnect_attempts(&self) -> u32;

    /// Whether automatic reconnection is enabled.
    fn enable_auto_reconnect(&self) -> bool;
}
