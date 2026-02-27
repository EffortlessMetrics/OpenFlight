// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! DCS World aircraft module configuration.

use serde::{Deserialize, Serialize};

/// Aircraft-specific axis configuration for a DCS World module.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DcsModule {
    /// Short aircraft identifier as used by DCS (e.g. `"F/A-18C"`).
    pub aircraft: String,
    /// Number of primary control axes exposed by the module.
    pub axis_count: u8,
    /// Throttle range as `[min, max]` normalised to `0.0` – `1.0`.
    pub throttle_range: [f32; 2],
    /// Maximum stick deflection in degrees (total throw, centre-to-stop).
    pub stick_throw: f32,
    /// Known behavioural quirks that require special-case handling.
    pub quirks: Vec<String>,
}
