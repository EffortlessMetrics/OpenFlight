// SPDX-License-Identifier: MIT OR Apache-2.0

//! Configuration constants for xtask automation.
//!
//! This module defines the single source of truth for which crates are included
//! in fast checks. It does not need to include every workspace member.

/// Core crates that are checked during fast validation (`cargo xtask check`).
///
/// These crates represent the critical path for the Flight Hub project and are
/// validated on every local check to ensure rapid feedback. The list is intentionally
/// limited to keep check times under 30 seconds.
///
/// This is the single source of truth for which crates are included in fast checks.
/// It does not need to include every workspace member.
pub const CORE_CRATES: &[&str] = &[
    "flight-core",
    "flight-virtual",
    "flight-hid",
    "flight-ipc",
];
