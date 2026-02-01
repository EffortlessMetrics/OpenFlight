// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Policy gates for device I/O operations.
//!
//! This module provides centralized control over whether device output operations
//! (MFD, LED, RGB) are permitted. By default, all output I/O is blocked to prevent
//! speculative packet transmission that could cause issues with hardware.
//!
//! # Environment Variables
//!
//! - `OPENFLIGHT_ALLOW_DEVICE_IO`: Set to any value to enable device output I/O.
//!   This should only be enabled when:
//!   - Protocol has been verified via USB capture
//!   - Running in a controlled test environment
//!   - User explicitly acknowledges the risk
//!
//! # Example
//!
//! ```bash
//! # Enable device I/O for verified protocols
//! OPENFLIGHT_ALLOW_DEVICE_IO=1 flightd
//! ```

use std::sync::atomic::{AtomicBool, Ordering};

/// Cached result of environment check to avoid repeated lookups.
static DEVICE_IO_ALLOWED: AtomicBool = AtomicBool::new(false);
static DEVICE_IO_CHECKED: AtomicBool = AtomicBool::new(false);

/// Check if device output I/O is permitted.
///
/// Returns `true` if `OPENFLIGHT_ALLOW_DEVICE_IO` environment variable is set.
/// The result is cached after the first call.
pub fn allow_device_io() -> bool {
    // Fast path: already checked
    if DEVICE_IO_CHECKED.load(Ordering::Relaxed) {
        return DEVICE_IO_ALLOWED.load(Ordering::Relaxed);
    }

    // Slow path: check environment and cache
    let allowed = std::env::var_os("OPENFLIGHT_ALLOW_DEVICE_IO").is_some();
    DEVICE_IO_ALLOWED.store(allowed, Ordering::Relaxed);
    DEVICE_IO_CHECKED.store(true, Ordering::Release);

    if allowed {
        tracing::warn!(
            target: "hotas::policy",
            "OPENFLIGHT_ALLOW_DEVICE_IO is set - device output I/O is ENABLED. \
             Ensure protocols are verified before sending packets to hardware."
        );
    }

    allowed
}

/// Reset the cached policy state. Only for testing.
#[cfg(test)]
pub fn reset_policy_cache() {
    DEVICE_IO_CHECKED.store(false, Ordering::Release);
    DEVICE_IO_ALLOWED.store(false, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_denies_io() {
        reset_policy_cache();
        // Without env var set, should return false
        // Note: This test may fail if OPENFLIGHT_ALLOW_DEVICE_IO is set in test env
        // SAFETY: Single-threaded test environment. remove_var is unsafe in Rust 2024
        // due to potential data races, but test isolation makes this safe here.
        unsafe {
            std::env::remove_var("OPENFLIGHT_ALLOW_DEVICE_IO");
        }
        reset_policy_cache();
        assert!(!allow_device_io());
    }
}
