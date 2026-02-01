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

use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};

/// Environment variable name for device I/O policy
const DEVICE_IO_ENV_VAR: &str = "OPENFLIGHT_ALLOW_DEVICE_IO";

/// Cached result of environment check to avoid repeated lookups.
static DEVICE_IO_ALLOWED: AtomicBool = AtomicBool::new(false);
static DEVICE_IO_CHECKED: AtomicBool = AtomicBool::new(false);

/// Pure policy decision function - testable without environment mutation.
///
/// Returns `true` if the environment variable value is `Some(_)`.
#[inline]
fn allow_device_io_from_env(env_value: Option<OsString>) -> bool {
    env_value.is_some()
}

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
    let allowed = allow_device_io_from_env(std::env::var_os(DEVICE_IO_ENV_VAR));
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

    // Test the pure policy decision logic without environment mutation
    #[test]
    fn test_policy_denies_when_env_not_set() {
        // Pure function test - no env mutation needed
        assert!(!allow_device_io_from_env(None));
    }

    #[test]
    fn test_policy_allows_when_env_set() {
        // Pure function test - any value enables I/O
        assert!(allow_device_io_from_env(Some("1".into())));
        assert!(allow_device_io_from_env(Some("".into())));
        assert!(allow_device_io_from_env(Some("true".into())));
    }

    #[test]
    fn test_cache_reset() {
        // Verify cache reset works without env mutation
        DEVICE_IO_ALLOWED.store(true, Ordering::Relaxed);
        DEVICE_IO_CHECKED.store(true, Ordering::Relaxed);

        reset_policy_cache();

        assert!(!DEVICE_IO_CHECKED.load(Ordering::Relaxed));
        assert!(!DEVICE_IO_ALLOWED.load(Ordering::Relaxed));
    }
}
