// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Device health monitoring for force feedback safety
//!
//! This module provides a trait-based interface for monitoring FFB device health,
//! including over-temperature and over-current conditions. When these hardware-critical
//! conditions are detected, the FFB engine triggers appropriate fault handling.
//!
//! # Hardware-Critical Faults
//!
//! Over-temperature and over-current conditions are classified as hardware-critical faults
//! that require a power cycle to clear. This is because:
//! - They may indicate hardware damage or malfunction
//! - Continued operation could cause further damage
//! - A power cycle allows the device to reset to a known-good state
//!
//! **Validates: Requirements FFB-SAFETY-01.7, FFB-SAFETY-01.9**

use std::time::{Duration, Instant};

/// Device health status reported by FFB devices
///
/// This struct contains the health metrics that can be monitored from
/// FFB devices that support health streaming (OFP-1 devices with
/// `has_health_stream` capability).
#[derive(Debug, Clone)]
pub struct DeviceHealthStatus {
    /// Motor/driver temperature in degrees Celsius
    /// None if temperature sensor is not available
    pub temperature_c: Option<f32>,

    /// Motor current draw in milliamps
    /// None if current sensor is not available
    pub current_ma: Option<f32>,

    /// Supply voltage in volts
    /// None if voltage sensor is not available
    pub voltage_v: Option<f32>,

    /// Device uptime in milliseconds
    pub uptime_ms: u64,

    /// Timestamp when this status was captured
    pub timestamp: Instant,

    /// Raw status flags from device (if available)
    pub raw_status_flags: Option<u16>,
}

impl Default for DeviceHealthStatus {
    fn default() -> Self {
        Self {
            temperature_c: None,
            current_ma: None,
            voltage_v: None,
            uptime_ms: 0,
            timestamp: Instant::now(),
            raw_status_flags: None,
        }
    }
}

impl DeviceHealthStatus {
    /// Create a new health status with current timestamp
    pub fn new() -> Self {
        Self {
            timestamp: Instant::now(),
            ..Default::default()
        }
    }

    /// Create health status with temperature reading
    pub fn with_temperature(mut self, temp_c: f32) -> Self {
        self.temperature_c = Some(temp_c);
        self
    }

    /// Create health status with current reading
    pub fn with_current(mut self, current_ma: f32) -> Self {
        self.current_ma = Some(current_ma);
        self
    }

    /// Create health status with voltage reading
    pub fn with_voltage(mut self, voltage_v: f32) -> Self {
        self.voltage_v = Some(voltage_v);
        self
    }

    /// Create health status with uptime
    pub fn with_uptime(mut self, uptime_ms: u64) -> Self {
        self.uptime_ms = uptime_ms;
        self
    }

    /// Create health status with raw status flags
    pub fn with_raw_flags(mut self, flags: u16) -> Self {
        self.raw_status_flags = Some(flags);
        self
    }

    /// Get age of this health status
    pub fn age(&self) -> Duration {
        self.timestamp.elapsed()
    }
}

/// Configuration for device health monitoring thresholds
///
/// These thresholds determine when over-temperature and over-current
/// conditions are triggered. Default values are conservative and should
/// be safe for most devices.
#[derive(Debug, Clone)]
pub struct DeviceHealthConfig {
    /// Temperature threshold for over-temp fault (degrees Celsius)
    /// Default: 85°C (typical motor driver thermal limit)
    pub over_temp_threshold_c: f32,

    /// Temperature threshold for warning (degrees Celsius)
    /// Default: 70°C (allows time to reduce load before fault)
    pub temp_warning_threshold_c: f32,

    /// Current threshold for over-current fault (milliamps)
    /// Default: 5000mA (5A, typical for small FFB motors)
    pub over_current_threshold_ma: f32,

    /// Current threshold for warning (milliamps)
    /// Default: 4000mA (80% of fault threshold)
    pub current_warning_threshold_ma: f32,

    /// Minimum voltage threshold (volts)
    /// Default: 4.5V (USB minimum)
    pub under_voltage_threshold_v: f32,

    /// Maximum voltage threshold (volts)
    /// Default: 5.5V (USB maximum)
    pub over_voltage_threshold_v: f32,

    /// Maximum age of health status before considered stale
    /// Default: 500ms
    pub max_health_age: Duration,

    /// Whether to enable health monitoring
    /// Default: true
    pub enabled: bool,
}

impl Default for DeviceHealthConfig {
    fn default() -> Self {
        Self {
            over_temp_threshold_c: 85.0,
            temp_warning_threshold_c: 70.0,
            over_current_threshold_ma: 5000.0,
            current_warning_threshold_ma: 4000.0,
            under_voltage_threshold_v: 4.5,
            over_voltage_threshold_v: 5.5,
            max_health_age: Duration::from_millis(500),
            enabled: true,
        }
    }
}

/// Result of health status evaluation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthCheckResult {
    /// Device is healthy, no issues detected
    Healthy,

    /// Temperature warning (approaching limit)
    TemperatureWarning,

    /// Current warning (approaching limit)
    CurrentWarning,

    /// Over-temperature fault detected (hardware-critical)
    OverTemperature,

    /// Over-current fault detected (hardware-critical)
    OverCurrent,

    /// Under-voltage condition detected
    UnderVoltage,

    /// Over-voltage condition detected
    OverVoltage,

    /// Health status is stale (no recent updates)
    StaleHealth,

    /// Health monitoring not available for this device
    NotAvailable,
}

impl HealthCheckResult {
    /// Check if this result indicates a hardware-critical fault
    ///
    /// Hardware-critical faults require a power cycle to clear.
    /// **Validates: Requirement FFB-SAFETY-01.9**
    pub fn is_hardware_critical(&self) -> bool {
        matches!(
            self,
            HealthCheckResult::OverTemperature | HealthCheckResult::OverCurrent
        )
    }

    /// Check if this result indicates any fault condition
    pub fn is_fault(&self) -> bool {
        matches!(
            self,
            HealthCheckResult::OverTemperature
                | HealthCheckResult::OverCurrent
                | HealthCheckResult::UnderVoltage
                | HealthCheckResult::OverVoltage
        )
    }

    /// Check if this result indicates a warning condition
    pub fn is_warning(&self) -> bool {
        matches!(
            self,
            HealthCheckResult::TemperatureWarning | HealthCheckResult::CurrentWarning
        )
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            HealthCheckResult::Healthy => "Device health OK",
            HealthCheckResult::TemperatureWarning => "Temperature approaching limit",
            HealthCheckResult::CurrentWarning => "Current draw approaching limit",
            HealthCheckResult::OverTemperature => "Device over-temperature protection triggered",
            HealthCheckResult::OverCurrent => "Device over-current protection triggered",
            HealthCheckResult::UnderVoltage => "Supply voltage below minimum",
            HealthCheckResult::OverVoltage => "Supply voltage above maximum",
            HealthCheckResult::StaleHealth => "Health status is stale (no recent updates)",
            HealthCheckResult::NotAvailable => "Health monitoring not available",
        }
    }
}

/// Trait for devices that can report health status
///
/// Implement this trait for FFB devices that support health monitoring.
/// The FFB engine will poll this interface to detect over-temperature
/// and over-current conditions.
pub trait DeviceHealthProvider {
    /// Check if this device supports health monitoring
    fn has_health_stream(&self) -> bool;

    /// Get the latest health status from the device
    ///
    /// Returns None if health monitoring is not available or
    /// no health data has been received yet.
    fn get_health_status(&self) -> Option<DeviceHealthStatus>;

    /// Get device identifier for logging
    fn device_id(&self) -> &str;
}

/// Device health monitor that evaluates health status against thresholds
///
/// This struct maintains the health monitoring state and evaluates
/// incoming health status against configured thresholds.
#[derive(Debug)]
pub struct DeviceHealthMonitor {
    /// Configuration for health thresholds
    config: DeviceHealthConfig,

    /// Last received health status
    last_status: Option<DeviceHealthStatus>,

    /// Last check result
    last_result: HealthCheckResult,

    /// Timestamp of last health update
    last_update: Option<Instant>,

    /// Count of consecutive warnings (for hysteresis)
    warning_count: u32,

    /// Count of consecutive faults (for confirmation)
    fault_count: u32,

    /// Whether a fault has been latched
    fault_latched: bool,

    /// The latched fault type (if any)
    latched_fault: Option<HealthCheckResult>,
}

impl DeviceHealthMonitor {
    /// Create a new health monitor with default configuration
    pub fn new() -> Self {
        Self::with_config(DeviceHealthConfig::default())
    }

    /// Create a new health monitor with custom configuration
    pub fn with_config(config: DeviceHealthConfig) -> Self {
        Self {
            config,
            last_status: None,
            last_result: HealthCheckResult::NotAvailable,
            last_update: None,
            warning_count: 0,
            fault_count: 0,
            fault_latched: false,
            latched_fault: None,
        }
    }

    /// Update configuration
    pub fn set_config(&mut self, config: DeviceHealthConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn config(&self) -> &DeviceHealthConfig {
        &self.config
    }

    /// Update health status and evaluate against thresholds
    ///
    /// Returns the health check result. If a hardware-critical fault
    /// is detected, it will be latched until cleared by power cycle.
    ///
    /// **Validates: Requirements FFB-SAFETY-01.7, FFB-SAFETY-01.9**
    pub fn update(&mut self, status: DeviceHealthStatus) -> HealthCheckResult {
        if !self.config.enabled {
            return HealthCheckResult::NotAvailable;
        }

        // If a fault is already latched, return it
        if self.fault_latched {
            if let Some(ref fault) = self.latched_fault {
                return fault.clone();
            }
        }

        self.last_status = Some(status.clone());
        self.last_update = Some(Instant::now());

        // Evaluate health status
        let result = self.evaluate_status(&status);

        // Handle fault latching for hardware-critical faults
        if result.is_hardware_critical() {
            self.fault_count += 1;
            // Latch immediately on first detection (no confirmation needed for safety)
            self.fault_latched = true;
            self.latched_fault = Some(result.clone());
            tracing::error!("Hardware-critical fault detected and latched: {:?}", result);
        } else if result.is_warning() {
            self.warning_count += 1;
            self.fault_count = 0;
        } else {
            self.warning_count = 0;
            self.fault_count = 0;
        }

        self.last_result = result.clone();
        result
    }

    /// Evaluate health status against thresholds
    fn evaluate_status(&self, status: &DeviceHealthStatus) -> HealthCheckResult {
        // Check temperature (most critical)
        if let Some(temp) = status.temperature_c {
            if temp >= self.config.over_temp_threshold_c {
                return HealthCheckResult::OverTemperature;
            }
            if temp >= self.config.temp_warning_threshold_c {
                return HealthCheckResult::TemperatureWarning;
            }
        }

        // Check current
        if let Some(current) = status.current_ma {
            if current >= self.config.over_current_threshold_ma {
                return HealthCheckResult::OverCurrent;
            }
            if current >= self.config.current_warning_threshold_ma {
                return HealthCheckResult::CurrentWarning;
            }
        }

        // Check voltage
        if let Some(voltage) = status.voltage_v {
            if voltage < self.config.under_voltage_threshold_v {
                return HealthCheckResult::UnderVoltage;
            }
            if voltage > self.config.over_voltage_threshold_v {
                return HealthCheckResult::OverVoltage;
            }
        }

        HealthCheckResult::Healthy
    }

    /// Check if health data is stale
    pub fn is_stale(&self) -> bool {
        if let Some(last_update) = self.last_update {
            last_update.elapsed() > self.config.max_health_age
        } else {
            true // No data received yet
        }
    }

    /// Get the last health check result
    pub fn last_result(&self) -> &HealthCheckResult {
        &self.last_result
    }

    /// Get the last health status
    pub fn last_status(&self) -> Option<&DeviceHealthStatus> {
        self.last_status.as_ref()
    }

    /// Check if a fault is currently latched
    pub fn is_fault_latched(&self) -> bool {
        self.fault_latched
    }

    /// Get the latched fault (if any)
    pub fn latched_fault(&self) -> Option<&HealthCheckResult> {
        self.latched_fault.as_ref()
    }

    /// Clear latched fault (should only be called after power cycle)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.9**
    /// Hardware-critical faults require power cycle to clear.
    pub fn clear_latched_fault(&mut self) {
        self.fault_latched = false;
        self.latched_fault = None;
        self.fault_count = 0;
        self.warning_count = 0;
        tracing::info!("Device health fault cleared (power cycle acknowledged)");
    }

    /// Get warning count
    pub fn warning_count(&self) -> u32 {
        self.warning_count
    }

    /// Get fault count
    pub fn fault_count(&self) -> u32 {
        self.fault_count
    }

    /// Check health from a provider and return result
    ///
    /// This is a convenience method that gets health status from a provider
    /// and evaluates it.
    pub fn check_provider(&mut self, provider: &dyn DeviceHealthProvider) -> HealthCheckResult {
        if !provider.has_health_stream() {
            return HealthCheckResult::NotAvailable;
        }

        match provider.get_health_status() {
            Some(status) => self.update(status),
            None => {
                if self.is_stale() {
                    HealthCheckResult::StaleHealth
                } else {
                    self.last_result.clone()
                }
            }
        }
    }
}

impl Default for DeviceHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_creation() {
        let status = DeviceHealthStatus::new()
            .with_temperature(45.0)
            .with_current(1500.0)
            .with_voltage(5.0)
            .with_uptime(60000);

        assert_eq!(status.temperature_c, Some(45.0));
        assert_eq!(status.current_ma, Some(1500.0));
        assert_eq!(status.voltage_v, Some(5.0));
        assert_eq!(status.uptime_ms, 60000);
    }

    #[test]
    fn test_health_check_result_properties() {
        // Hardware-critical faults
        assert!(HealthCheckResult::OverTemperature.is_hardware_critical());
        assert!(HealthCheckResult::OverCurrent.is_hardware_critical());
        assert!(!HealthCheckResult::TemperatureWarning.is_hardware_critical());

        // Fault conditions
        assert!(HealthCheckResult::OverTemperature.is_fault());
        assert!(HealthCheckResult::OverCurrent.is_fault());
        assert!(HealthCheckResult::UnderVoltage.is_fault());
        assert!(!HealthCheckResult::Healthy.is_fault());

        // Warning conditions
        assert!(HealthCheckResult::TemperatureWarning.is_warning());
        assert!(HealthCheckResult::CurrentWarning.is_warning());
        assert!(!HealthCheckResult::Healthy.is_warning());
    }

    #[test]
    fn test_monitor_healthy_status() {
        let mut monitor = DeviceHealthMonitor::new();

        let status = DeviceHealthStatus::new()
            .with_temperature(45.0)
            .with_current(1500.0)
            .with_voltage(5.0);

        let result = monitor.update(status);
        assert_eq!(result, HealthCheckResult::Healthy);
        assert!(!monitor.is_fault_latched());
    }

    #[test]
    fn test_monitor_over_temperature() {
        let mut monitor = DeviceHealthMonitor::new();

        // Temperature above threshold (85°C default)
        let status = DeviceHealthStatus::new()
            .with_temperature(90.0)
            .with_current(1500.0);

        let result = monitor.update(status);
        assert_eq!(result, HealthCheckResult::OverTemperature);
        assert!(result.is_hardware_critical());
        assert!(monitor.is_fault_latched());
    }

    #[test]
    fn test_monitor_over_current() {
        let mut monitor = DeviceHealthMonitor::new();

        // Current above threshold (5000mA default)
        let status = DeviceHealthStatus::new()
            .with_temperature(45.0)
            .with_current(6000.0);

        let result = monitor.update(status);
        assert_eq!(result, HealthCheckResult::OverCurrent);
        assert!(result.is_hardware_critical());
        assert!(monitor.is_fault_latched());
    }

    #[test]
    fn test_monitor_temperature_warning() {
        let mut monitor = DeviceHealthMonitor::new();

        // Temperature at warning level (70°C default)
        let status = DeviceHealthStatus::new()
            .with_temperature(75.0)
            .with_current(1500.0);

        let result = monitor.update(status);
        assert_eq!(result, HealthCheckResult::TemperatureWarning);
        assert!(!monitor.is_fault_latched());
        assert_eq!(monitor.warning_count(), 1);
    }

    #[test]
    fn test_monitor_fault_latching() {
        let mut monitor = DeviceHealthMonitor::new();

        // Trigger over-temp fault
        let status = DeviceHealthStatus::new().with_temperature(90.0);
        let result = monitor.update(status);
        assert_eq!(result, HealthCheckResult::OverTemperature);
        assert!(monitor.is_fault_latched());

        // Even with healthy status, fault remains latched
        let healthy_status = DeviceHealthStatus::new().with_temperature(45.0);
        let result = monitor.update(healthy_status);
        assert_eq!(result, HealthCheckResult::OverTemperature);
        assert!(monitor.is_fault_latched());

        // Clear fault (simulating power cycle)
        monitor.clear_latched_fault();
        assert!(!monitor.is_fault_latched());

        // Now healthy status returns healthy
        let healthy_status = DeviceHealthStatus::new().with_temperature(45.0);
        let result = monitor.update(healthy_status);
        assert_eq!(result, HealthCheckResult::Healthy);
    }

    #[test]
    fn test_monitor_custom_config() {
        let config = DeviceHealthConfig {
            over_temp_threshold_c: 60.0, // Lower threshold
            temp_warning_threshold_c: 50.0,
            ..Default::default()
        };

        let mut monitor = DeviceHealthMonitor::with_config(config);

        // Temperature that would be OK with default config triggers fault
        let status = DeviceHealthStatus::new().with_temperature(65.0);
        let result = monitor.update(status);
        assert_eq!(result, HealthCheckResult::OverTemperature);
    }

    #[test]
    fn test_monitor_disabled() {
        let config = DeviceHealthConfig {
            enabled: false,
            ..Default::default()
        };

        let mut monitor = DeviceHealthMonitor::with_config(config);

        // Even with over-temp, returns NotAvailable when disabled
        let status = DeviceHealthStatus::new().with_temperature(90.0);
        let result = monitor.update(status);
        assert_eq!(result, HealthCheckResult::NotAvailable);
    }

    #[test]
    fn test_voltage_checks() {
        let mut monitor = DeviceHealthMonitor::new();

        // Under-voltage
        let status = DeviceHealthStatus::new().with_voltage(4.0);
        let result = monitor.update(status);
        assert_eq!(result, HealthCheckResult::UnderVoltage);

        // Clear and test over-voltage
        monitor.clear_latched_fault();
        let status = DeviceHealthStatus::new().with_voltage(6.0);
        let result = monitor.update(status);
        assert_eq!(result, HealthCheckResult::OverVoltage);
    }
}

/// Integration tests for device health monitoring with FFB engine
#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Mock device health provider for testing
    struct MockHealthProvider {
        health_status: Option<DeviceHealthStatus>,
        has_health: bool,
        device_id: String,
    }

    impl MockHealthProvider {
        fn new(has_health: bool) -> Self {
            Self {
                health_status: None,
                has_health,
                device_id: "mock_device".to_string(),
            }
        }

        fn set_health(&mut self, status: DeviceHealthStatus) {
            self.health_status = Some(status);
        }
    }

    impl DeviceHealthProvider for MockHealthProvider {
        fn has_health_stream(&self) -> bool {
            self.has_health
        }

        fn get_health_status(&self) -> Option<DeviceHealthStatus> {
            self.health_status.clone()
        }

        fn device_id(&self) -> &str {
            &self.device_id
        }
    }

    #[test]
    fn test_monitor_with_provider_healthy() {
        let mut monitor = DeviceHealthMonitor::new();
        let mut provider = MockHealthProvider::new(true);

        // Set healthy status
        provider.set_health(
            DeviceHealthStatus::new()
                .with_temperature(45.0)
                .with_current(1500.0)
                .with_voltage(5.0),
        );

        let result = monitor.check_provider(&provider);
        assert_eq!(result, HealthCheckResult::Healthy);
    }

    #[test]
    fn test_monitor_with_provider_over_temp() {
        let mut monitor = DeviceHealthMonitor::new();
        let mut provider = MockHealthProvider::new(true);

        // Set over-temp status
        provider.set_health(DeviceHealthStatus::new().with_temperature(90.0));

        let result = monitor.check_provider(&provider);
        assert_eq!(result, HealthCheckResult::OverTemperature);
        assert!(monitor.is_fault_latched());
    }

    #[test]
    fn test_monitor_with_provider_no_health_stream() {
        let mut monitor = DeviceHealthMonitor::new();
        let provider = MockHealthProvider::new(false);

        let result = monitor.check_provider(&provider);
        assert_eq!(result, HealthCheckResult::NotAvailable);
    }

    #[test]
    fn test_monitor_with_provider_no_data() {
        let mut monitor = DeviceHealthMonitor::new();
        let provider = MockHealthProvider::new(true);

        // No health data set
        let result = monitor.check_provider(&provider);
        // Should be stale since no data has been received
        assert_eq!(result, HealthCheckResult::StaleHealth);
    }

    #[test]
    fn test_hardware_critical_fault_requires_power_cycle() {
        let mut monitor = DeviceHealthMonitor::new();

        // Trigger over-temp fault
        let status = DeviceHealthStatus::new().with_temperature(90.0);
        let result = monitor.update(status);
        assert_eq!(result, HealthCheckResult::OverTemperature);
        assert!(result.is_hardware_critical());
        assert!(monitor.is_fault_latched());

        // Healthy status should still return latched fault
        let healthy_status = DeviceHealthStatus::new().with_temperature(45.0);
        let result = monitor.update(healthy_status);
        assert_eq!(result, HealthCheckResult::OverTemperature);
        assert!(monitor.is_fault_latched());

        // Clear fault (simulating power cycle)
        monitor.clear_latched_fault();
        assert!(!monitor.is_fault_latched());

        // Now healthy status returns healthy
        let healthy_status = DeviceHealthStatus::new().with_temperature(45.0);
        let result = monitor.update(healthy_status);
        assert_eq!(result, HealthCheckResult::Healthy);
    }

    #[test]
    fn test_over_current_is_hardware_critical() {
        let mut monitor = DeviceHealthMonitor::new();

        // Trigger over-current fault
        let status = DeviceHealthStatus::new().with_current(6000.0);
        let result = monitor.update(status);
        assert_eq!(result, HealthCheckResult::OverCurrent);
        assert!(result.is_hardware_critical());
        assert!(monitor.is_fault_latched());
    }

    #[test]
    fn test_warnings_do_not_latch() {
        let mut monitor = DeviceHealthMonitor::new();

        // Trigger temperature warning
        let status = DeviceHealthStatus::new().with_temperature(75.0);
        let result = monitor.update(status);
        assert_eq!(result, HealthCheckResult::TemperatureWarning);
        assert!(!monitor.is_fault_latched());

        // Healthy status should return healthy
        let healthy_status = DeviceHealthStatus::new().with_temperature(45.0);
        let result = monitor.update(healthy_status);
        assert_eq!(result, HealthCheckResult::Healthy);
    }

    #[test]
    fn test_consecutive_warnings_tracked() {
        let mut monitor = DeviceHealthMonitor::new();

        // Multiple warnings
        for _ in 0..5 {
            let status = DeviceHealthStatus::new().with_temperature(75.0);
            monitor.update(status);
        }

        assert_eq!(monitor.warning_count(), 5);

        // Healthy status resets warning count
        let healthy_status = DeviceHealthStatus::new().with_temperature(45.0);
        monitor.update(healthy_status);
        assert_eq!(monitor.warning_count(), 0);
    }
}
