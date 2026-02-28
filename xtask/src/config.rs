// SPDX-License-Identifier: MIT OR Apache-2.0

//! Configuration constants for xtask automation.
//!
//! This module defines the single source of truth for which crates are included
//! in fast checks. It does not need to include every workspace member.

/// Core crates that must pass strict clippy (`cargo clippy -p <crate> -- -D warnings`).
///
/// These 8 crates represent the critical path for the Flight Hub project and are
/// validated on every local check to ensure rapid feedback. This list must stay
/// in sync with the CI strict-clippy step in `.github/workflows/ci.yml`.
///
/// This is the single source of truth for which crates are included in fast checks.
/// It does not need to include every workspace member.
pub const CORE_CRATES: &[&str] = &[
    "flight-core",
    "flight-axis",
    "flight-bus",
    "flight-hid",
    "flight-ipc",
    "flight-service",
    "flight-simconnect",
    "flight-panels",
];
