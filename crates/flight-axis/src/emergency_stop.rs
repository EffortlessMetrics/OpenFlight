// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Emergency stop (REQ-846).
//!
//! An [`EmergencyStop`] gate forces all axis output to zero when engaged.
//! The engaged flag is an [`AtomicBool`] so any thread can engage/disengage
//! without locking.
//!
//! RT-safe: no heap allocation.

use std::sync::atomic::{AtomicBool, Ordering};

/// Thread-safe emergency stop gate.
///
/// When engaged, [`apply`](EmergencyStop::apply) returns `0.0` regardless of
/// the input value.  The gate uses relaxed atomic ordering — callers see the
/// latest write within a short bounded window, which is acceptable for a
/// safety mechanism that clamps to zero.
///
/// RT-safe: no heap allocation.
pub struct EmergencyStop {
    engaged: AtomicBool,
}

impl Default for EmergencyStop {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for EmergencyStop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmergencyStop")
            .field("engaged", &self.is_engaged())
            .finish()
    }
}

impl EmergencyStop {
    /// Creates a new `EmergencyStop` in the disengaged state.
    ///
    /// RT-safe: no heap allocation.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            engaged: AtomicBool::new(false),
        }
    }

    /// Engages the emergency stop — all subsequent [`apply`](Self::apply)
    /// calls return `0.0`.
    pub fn engage(&self) {
        self.engaged.store(true, Ordering::Release);
    }

    /// Disengages the emergency stop — values pass through unchanged.
    pub fn disengage(&self) {
        self.engaged.store(false, Ordering::Release);
    }

    /// Returns `true` if the emergency stop is currently engaged.
    #[must_use]
    pub fn is_engaged(&self) -> bool {
        self.engaged.load(Ordering::Acquire)
    }

    /// Returns `0.0` when engaged, or `value` unchanged when disengaged.
    ///
    /// RT-safe: no heap allocation.
    #[inline]
    #[must_use]
    pub fn apply(&self, value: f64) -> f64 {
        if self.engaged.load(Ordering::Acquire) {
            0.0
        } else {
            value
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_disengaged_passthrough() {
        let es = EmergencyStop::new();
        assert!(!es.is_engaged());
        assert!((es.apply(0.75) - 0.75).abs() < 1e-12);
    }

    #[test]
    fn engaged_returns_zero() {
        let es = EmergencyStop::new();
        es.engage();
        assert!(es.is_engaged());
        assert_eq!(es.apply(0.9), 0.0);
        assert_eq!(es.apply(-0.5), 0.0);
    }

    #[test]
    fn disengage_restores_passthrough() {
        let es = EmergencyStop::new();
        es.engage();
        assert_eq!(es.apply(1.0), 0.0);
        es.disengage();
        assert!((es.apply(1.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn engage_is_idempotent() {
        let es = EmergencyStop::new();
        es.engage();
        es.engage();
        assert!(es.is_engaged());
        assert_eq!(es.apply(0.5), 0.0);
    }

    #[test]
    fn zero_input_stays_zero() {
        let es = EmergencyStop::new();
        assert_eq!(es.apply(0.0), 0.0);
        es.engage();
        assert_eq!(es.apply(0.0), 0.0);
    }

    #[test]
    fn thread_safe_engage() {
        use std::sync::Arc;
        let es = Arc::new(EmergencyStop::new());
        let es2 = Arc::clone(&es);
        let handle = std::thread::spawn(move || {
            es2.engage();
        });
        handle.join().unwrap();
        assert!(es.is_engaged());
        assert_eq!(es.apply(1.0), 0.0);
    }

    #[test]
    fn debug_format() {
        let es = EmergencyStop::new();
        let dbg = format!("{es:?}");
        assert!(dbg.contains("EmergencyStop"));
    }
}
