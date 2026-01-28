// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Shared adapter state machine.

/// Common adapter lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterState {
    /// Disconnected from simulator.
    Disconnected,
    /// Connecting to simulator.
    Connecting,
    /// Connected but no aircraft detected.
    Connected,
    /// Aircraft detected, configuring data definitions.
    DetectingAircraft,
    /// Fully operational.
    Active,
    /// Error state.
    Error,
}
