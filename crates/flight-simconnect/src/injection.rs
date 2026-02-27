// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis injection support for writing processed axis values back to MSFS.
//!
//! Axis injection uses `TransmitClientEvent` (AXIS_*_SET events) to write
//! processed axis values back into the simulator. Injection is disabled by
//! default for safety — callers must explicitly opt in by setting
//! `AxisInjectionConfig::enabled = true`.

use std::sync::atomic::{AtomicU64, Ordering};

/// Configuration for axis injection.
#[derive(Debug, Clone)]
pub struct AxisInjectionConfig {
    /// Whether injection is enabled (false by default for safety).
    pub enabled: bool,
    /// Maximum injection rate in Hz.
    pub max_rate_hz: f32,
}

impl Default for AxisInjectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_rate_hz: 50.0,
        }
    }
}

/// Injects processed axis values back into the simulator.
///
/// Tracks successful injection calls and errors via atomic counters so the
/// state can be read from any thread without taking a lock.
pub struct AxisInjector {
    config: AxisInjectionConfig,
    injection_count: AtomicU64,
    error_count: AtomicU64,
}

impl AxisInjector {
    /// Create a new axis injector with the given configuration.
    pub fn new(config: AxisInjectionConfig) -> Self {
        Self {
            config,
            injection_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
        }
    }

    /// Returns `true` if axis injection is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Returns the maximum injection rate in Hz.
    pub fn max_rate_hz(&self) -> f32 {
        self.config.max_rate_hz
    }

    /// Returns the total number of successful injection calls recorded.
    pub fn injection_count(&self) -> u64 {
        self.injection_count.load(Ordering::Relaxed)
    }

    /// Returns the total number of injection errors recorded.
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Record one successful axis injection (increments the injection counter).
    pub fn record_injection(&self) {
        self.injection_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record one injection error (increments the error counter).
    pub fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for AxisInjector {
    fn default() -> Self {
        Self::new(AxisInjectionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_axis_injection_config_defaults() {
        let config = AxisInjectionConfig::default();
        assert!(!config.enabled, "injection must be disabled by default");
        assert_eq!(config.max_rate_hz, 50.0, "default max rate must be 50 Hz");
    }

    #[test]
    fn test_injection_disabled_by_default() {
        let injector = AxisInjector::default();
        assert!(
            !injector.is_enabled(),
            "AxisInjector must be disabled by default"
        );
    }

    #[test]
    fn test_injection_count_starts_at_zero() {
        let injector = AxisInjector::default();
        assert_eq!(injector.injection_count(), 0);
    }

    #[test]
    fn test_error_count_starts_at_zero() {
        let injector = AxisInjector::default();
        assert_eq!(injector.error_count(), 0);
    }

    #[test]
    fn test_injection_count_increments() {
        let injector = AxisInjector::default();
        injector.record_injection();
        assert_eq!(injector.injection_count(), 1);
        injector.record_injection();
        assert_eq!(injector.injection_count(), 2);
    }

    #[test]
    fn test_error_count_increments() {
        let injector = AxisInjector::default();
        injector.record_error();
        assert_eq!(injector.error_count(), 1);
        injector.record_error();
        assert_eq!(injector.error_count(), 2);
    }

    #[test]
    fn test_injection_and_error_counts_are_independent() {
        let injector = AxisInjector::default();
        injector.record_injection();
        injector.record_injection();
        injector.record_injection();
        injector.record_error();
        assert_eq!(injector.injection_count(), 3);
        assert_eq!(injector.error_count(), 1);
    }

    #[test]
    fn test_axis_injection_config_custom() {
        let config = AxisInjectionConfig {
            enabled: true,
            max_rate_hz: 100.0,
        };
        let injector = AxisInjector::new(config);
        assert!(injector.is_enabled());
        assert_eq!(injector.max_rate_hz(), 100.0);
    }

    #[test]
    fn test_max_rate_hz_accessor() {
        let config = AxisInjectionConfig::default();
        let injector = AxisInjector::new(config);
        assert_eq!(injector.max_rate_hz(), 50.0);
    }
}
