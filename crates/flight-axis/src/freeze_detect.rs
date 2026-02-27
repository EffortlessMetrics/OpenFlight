// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis engine freeze detection (REQ-634).
//!
//! Detects when an axis value has not changed for a configurable number of
//! ticks.  Default timeout is 500 ticks (2 seconds at 250 Hz).
//!
//! Zero-allocation. RT-safe (ADR-004).

/// Detects axis freeze conditions (no value change for N ticks).
///
/// # Real-time safety
/// - Zero allocations on the hot path.
/// - No locks or blocking operations.
#[derive(Debug, Clone, Copy)]
pub struct FreezeDetector {
    timeout_ticks: u32,
    last_value: f32,
    ticks_unchanged: u32,
    initialized: bool,
}

impl FreezeDetector {
    /// Creates a new `FreezeDetector` with the given timeout in ticks.
    ///
    /// At 250 Hz, 500 ticks ≈ 2 seconds.
    pub const fn new(timeout_ticks: u32) -> Self {
        Self {
            timeout_ticks,
            last_value: 0.0,
            ticks_unchanged: 0,
            initialized: false,
        }
    }

    /// Updates the detector with a new axis value.
    ///
    /// Returns `true` if the axis is considered frozen (value unchanged for
    /// at least `timeout_ticks` consecutive ticks).
    ///
    /// Zero-allocation — safe to call from RT code.
    #[inline]
    pub fn update(&mut self, value: f32) -> bool {
        if !self.initialized {
            self.last_value = value;
            self.initialized = true;
            self.ticks_unchanged = 1;
            return false;
        }

        if value == self.last_value {
            self.ticks_unchanged = self.ticks_unchanged.saturating_add(1);
        } else {
            self.last_value = value;
            self.ticks_unchanged = 1;
        }

        self.ticks_unchanged >= self.timeout_ticks
    }

    /// Returns the number of consecutive ticks with the same value.
    #[inline]
    pub fn ticks_unchanged(&self) -> u32 {
        self.ticks_unchanged
    }

    /// Returns the configured timeout in ticks.
    #[inline]
    pub fn timeout_ticks(&self) -> u32 {
        self.timeout_ticks
    }

    /// Resets the detector state.
    #[inline]
    pub fn reset(&mut self) {
        self.last_value = 0.0;
        self.ticks_unchanged = 0;
        self.initialized = false;
    }
}

impl Default for FreezeDetector {
    fn default() -> Self {
        Self::new(500)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_input_not_frozen() {
        let mut fd = FreezeDetector::new(3);
        assert!(!fd.update(0.1));
        assert!(!fd.update(0.2));
        assert!(!fd.update(0.3));
        assert!(!fd.update(0.4));
    }

    #[test]
    fn constant_input_after_timeout_is_frozen() {
        let mut fd = FreezeDetector::new(3);
        assert!(!fd.update(0.5)); // tick 1
        assert!(!fd.update(0.5)); // tick 2
        assert!(fd.update(0.5));  // tick 3 — frozen
        assert!(fd.update(0.5));  // tick 4 — still frozen
    }

    #[test]
    fn slight_change_resets_counter() {
        let mut fd = FreezeDetector::new(3);
        assert!(!fd.update(0.5));
        assert!(!fd.update(0.5));
        // Change just before timeout
        assert!(!fd.update(0.50001));
        assert!(!fd.update(0.50001));
        // Still not frozen because counter was reset
        assert!(fd.update(0.50001)); // tick 3 of 0.50001 — frozen
    }

    #[test]
    fn default_timeout_is_500() {
        let fd = FreezeDetector::default();
        assert_eq!(fd.timeout_ticks(), 500);
    }

    #[test]
    fn reset_clears_state() {
        let mut fd = FreezeDetector::new(3);
        fd.update(0.5);
        fd.update(0.5);
        fd.update(0.5);
        assert!(fd.ticks_unchanged() >= 3);

        fd.reset();
        assert_eq!(fd.ticks_unchanged(), 0);
        assert!(!fd.update(0.5)); // first update after reset is not frozen
    }
}
