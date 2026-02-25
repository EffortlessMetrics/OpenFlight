// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Cougar MFD HID driver implementation
//!
//! Provides HID communication for Thrustmaster Cougar MFD (Multi-Function Display) panels.
//! Implements ≤20ms latency requirement with verify test patterns and drift detection.
//!
//! The Cougar MFD is a specialized display panel with programmable buttons and LED indicators
//! commonly used in high-fidelity flight simulation setups.

use crate::led::{LatencyStats, LedState, LedTarget};
use flight_core::{FlightError, Result};
use flight_hid::{HidAdapter, HidDeviceInfo, HidOperationResult};
use flight_metrics::{
    MetricsRegistry,
    common::{DeviceMetricNames, PANEL_DEVICE_METRICS},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Thrustmaster Cougar MFD vendor ID
const COUGAR_VENDOR_ID: u16 = 0x044F;

/// Cougar MFD product IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CougarMfdType {
    MfdLeft = 0x0404,
    MfdRight = 0x0405,
    MfdCenter = 0x0406,
}

impl CougarMfdType {
    /// Get MFD type from product ID
    pub fn from_product_id(pid: u16) -> Option<Self> {
        match pid {
            0x0404 => Some(CougarMfdType::MfdLeft),
            0x0405 => Some(CougarMfdType::MfdRight),
            0x0406 => Some(CougarMfdType::MfdCenter),
            _ => None,
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            CougarMfdType::MfdLeft => "Cougar MFD Left",
            CougarMfdType::MfdRight => "Cougar MFD Right",
            CougarMfdType::MfdCenter => "Cougar MFD Center",
        }
    }

    /// Get LED mapping for this MFD type
    pub fn led_mapping(&self) -> &'static [&'static str] {
        // Cougar MFDs have programmable button LEDs and status indicators
        match self {
            CougarMfdType::MfdLeft => &[
                "OSB1",
                "OSB2",
                "OSB3",
                "OSB4",
                "OSB5",
                "OSB6",
                "OSB7",
                "OSB8",
                "OSB9",
                "OSB10",
                "OSB11",
                "OSB12",
                "OSB13",
                "OSB14",
                "OSB15",
                "OSB16",
                "OSB17",
                "OSB18",
                "OSB19",
                "OSB20",
                "BRIGHTNESS",
                "CONTRAST",
                "SYM",
                "CON",
                "BRT",
            ],
            CougarMfdType::MfdRight => &[
                "OSB1",
                "OSB2",
                "OSB3",
                "OSB4",
                "OSB5",
                "OSB6",
                "OSB7",
                "OSB8",
                "OSB9",
                "OSB10",
                "OSB11",
                "OSB12",
                "OSB13",
                "OSB14",
                "OSB15",
                "OSB16",
                "OSB17",
                "OSB18",
                "OSB19",
                "OSB20",
                "BRIGHTNESS",
                "CONTRAST",
                "SYM",
                "CON",
                "BRT",
            ],
            CougarMfdType::MfdCenter => &[
                "OSB1",
                "OSB2",
                "OSB3",
                "OSB4",
                "OSB5",
                "OSB6",
                "OSB7",
                "OSB8",
                "OSB9",
                "OSB10",
                "BRIGHTNESS",
                "CONTRAST",
                "POWER",
            ],
        }
    }

    /// Get verify test pattern for this MFD type
    pub fn verify_pattern(&self) -> Vec<CougarVerifyStep> {
        match self {
            CougarMfdType::MfdLeft | CougarMfdType::MfdRight => vec![
                CougarVerifyStep::LedOn("OSB1"),
                CougarVerifyStep::Delay(Duration::from_millis(100)),
                CougarVerifyStep::LedOn("OSB5"),
                CougarVerifyStep::Delay(Duration::from_millis(100)),
                CougarVerifyStep::LedOn("OSB10"),
                CougarVerifyStep::Delay(Duration::from_millis(100)),
                CougarVerifyStep::LedOn("OSB15"),
                CougarVerifyStep::Delay(Duration::from_millis(100)),
                CougarVerifyStep::LedOn("OSB20"),
                CougarVerifyStep::Delay(Duration::from_millis(100)),
                CougarVerifyStep::AllOff,
                CougarVerifyStep::LedBlink("BRIGHTNESS", 4.0),
                CougarVerifyStep::Delay(Duration::from_millis(500)),
                CougarVerifyStep::AllOff,
            ],
            CougarMfdType::MfdCenter => vec![
                CougarVerifyStep::LedOn("OSB1"),
                CougarVerifyStep::Delay(Duration::from_millis(100)),
                CougarVerifyStep::LedOn("OSB5"),
                CougarVerifyStep::Delay(Duration::from_millis(100)),
                CougarVerifyStep::LedOn("OSB10"),
                CougarVerifyStep::Delay(Duration::from_millis(100)),
                CougarVerifyStep::AllOff,
                CougarVerifyStep::LedBlink("POWER", 6.0),
                CougarVerifyStep::Delay(Duration::from_millis(500)),
                CougarVerifyStep::AllOff,
            ],
        }
    }
}

/// Verify test pattern step for Cougar MFD
#[derive(Debug, Clone)]
pub enum CougarVerifyStep {
    LedOn(&'static str),
    LedOff(&'static str),
    LedBlink(&'static str, f32), // LED name, rate_hz
    AllOn,
    AllOff,
    Delay(Duration),
}

/// MFD LED state for HID communication
#[derive(Debug, Clone)]
pub struct MfdLedState {
    /// LED index in HID report
    pub led_index: u8,
    /// Current brightness (0.0-1.0)
    pub brightness: f32,
    /// Whether LED is currently on
    pub is_on: bool,
    /// Blink rate if blinking
    pub blink_rate: Option<f32>,
    /// Last blink state change
    pub last_blink_toggle: Instant,
    /// Last HID write time
    pub last_write: Instant,
}

/// Cougar MFD writer with HID communication
pub struct CougarMfdWriter {
    /// HID adapter for device communication
    hid_adapter: HidAdapter,
    /// Shared metrics registry
    metrics_registry: Arc<MetricsRegistry>,
    /// Device metric names
    device_metrics: DeviceMetricNames,
    /// Connected MFDs by device path
    mfds: HashMap<String, MfdInfo>,
    /// LED states by MFD and LED name
    led_states: HashMap<String, HashMap<String, MfdLedState>>,
    /// Latency tracking
    latency_samples: Vec<Duration>,
    /// Maximum latency samples to keep
    max_latency_samples: usize,
    /// Minimum write interval for rate limiting (≥8ms per requirements)
    min_write_interval: Duration,
    /// Verify test state
    verify_state: Option<CougarVerifyTestState>,
}

/// MFD information
#[derive(Debug, Clone)]
pub struct MfdInfo {
    pub device_info: HidDeviceInfo,
    pub mfd_type: CougarMfdType,
    pub last_health_check: Instant,
}

/// Verify test execution state
#[derive(Debug)]
struct CougarVerifyTestState {
    mfd_path: String,
    steps: Vec<CougarVerifyStep>,
    current_step: usize,
    step_start_time: Instant,
    test_start_time: Instant,
    results: Vec<CougarVerifyStepResult>,
}

/// Result of a verify test step
#[derive(Debug, Clone)]
pub struct CougarVerifyStepResult {
    pub step_index: usize,
    pub expected_latency: Duration,
    pub actual_latency: Duration,
    pub success: bool,
    pub error: Option<String>,
}

/// Verify test result
#[derive(Debug, Clone)]
pub struct CougarVerifyTestResult {
    pub mfd_path: String,
    pub total_duration: Duration,
    pub step_results: Vec<CougarVerifyStepResult>,
    pub success: bool,
}

/// MFD health status
#[derive(Debug, Clone)]
pub struct CougarMfdHealthStatus {
    pub mfd_path: String,
    pub mfd_type: CougarMfdType,
    pub is_responsive: bool,
    pub drift_detected: bool,
    pub last_check: Instant,
    pub hid_events: Vec<flight_core::WatchdogEvent>,
}

impl CougarMfdWriter {
    /// Create new Cougar MFD writer
    pub fn new(hid_adapter: HidAdapter) -> Self {
        Self {
            hid_adapter,
            metrics_registry: Arc::new(MetricsRegistry::new()),
            device_metrics: PANEL_DEVICE_METRICS,
            mfds: HashMap::new(),
            led_states: HashMap::new(),
            latency_samples: Vec::new(),
            max_latency_samples: 1000,
            min_write_interval: Duration::from_millis(8), // ≥8ms per requirements
            verify_state: None,
        }
    }

    /// Create new Cougar MFD writer with shared metrics registry
    pub fn new_with_metrics(
        hid_adapter: HidAdapter,
        metrics_registry: Arc<MetricsRegistry>,
    ) -> Self {
        Self {
            hid_adapter,
            metrics_registry,
            device_metrics: PANEL_DEVICE_METRICS,
            mfds: HashMap::new(),
            led_states: HashMap::new(),
            latency_samples: Vec::new(),
            max_latency_samples: 1000,
            min_write_interval: Duration::from_millis(8),
            verify_state: None,
        }
    }

    /// Create new Cougar MFD writer with shared metrics registry and custom metric names
    pub fn new_with_metrics_and_device_metrics(
        hid_adapter: HidAdapter,
        metrics_registry: Arc<MetricsRegistry>,
        device_metrics: DeviceMetricNames,
    ) -> Self {
        Self {
            hid_adapter,
            metrics_registry,
            device_metrics,
            mfds: HashMap::new(),
            led_states: HashMap::new(),
            latency_samples: Vec::new(),
            max_latency_samples: 1000,
            min_write_interval: Duration::from_millis(8),
            verify_state: None,
        }
    }

    /// Get shared metrics registry
    pub fn metrics_registry(&self) -> Arc<MetricsRegistry> {
        self.metrics_registry.clone()
    }

    /// Start the MFD writer and enumerate devices
    pub fn start(&mut self) -> Result<()> {
        info!("Starting Cougar MFD writer");

        self.hid_adapter.start()?;
        self.enumerate_mfds()?;

        Ok(())
    }

    /// Stop the MFD writer
    pub fn stop(&mut self) {
        info!("Stopping Cougar MFD writer");

        // Turn off all LEDs before stopping
        let mfd_paths: Vec<_> = self.mfds.keys().cloned().collect();
        for mfd_path in mfd_paths {
            if let Err(e) = self.turn_off_all_leds(&mfd_path) {
                warn!("Failed to turn off LEDs for MFD {}: {}", mfd_path, e);
            }
        }

        self.hid_adapter.stop();
        self.mfds.clear();
        self.led_states.clear();
    }

    /// Enumerate and register Cougar MFDs
    fn enumerate_mfds(&mut self) -> Result<()> {
        debug!("Enumerating Cougar MFDs");

        let devices: Vec<_> = self
            .hid_adapter
            .get_all_devices()
            .into_iter()
            .cloned()
            .collect();

        for device_info in devices {
            if self.is_supported_mfd(&device_info) {
                self.register_mfd(device_info)?;
            }
        }

        info!("Found {} Cougar MFDs", self.mfds.len());
        Ok(())
    }

    /// Check if device is a supported Cougar MFD
    fn is_supported_mfd(&self, device_info: &HidDeviceInfo) -> bool {
        device_info.vendor_id == COUGAR_VENDOR_ID
            && CougarMfdType::from_product_id(device_info.product_id).is_some()
    }

    /// Register a new MFD
    fn register_mfd(&mut self, device_info: HidDeviceInfo) -> Result<()> {
        let mfd_type = CougarMfdType::from_product_id(device_info.product_id).ok_or_else(|| {
            FlightError::Configuration(format!(
                "Unsupported MFD product ID: {:04X}",
                device_info.product_id
            ))
        })?;

        info!(
            "Registering {} MFD: {}",
            mfd_type.name(),
            device_info.device_path
        );

        let mfd_info = MfdInfo {
            device_info: device_info.clone(),
            mfd_type,
            last_health_check: Instant::now(),
        };

        // Initialize LED states for this MFD
        let mut mfd_led_states = HashMap::new();
        for (index, &led_name) in mfd_type.led_mapping().iter().enumerate() {
            mfd_led_states.insert(
                led_name.to_string(),
                MfdLedState {
                    led_index: index as u8,
                    brightness: 0.0,
                    is_on: false,
                    blink_rate: None,
                    last_blink_toggle: Instant::now(),
                    last_write: Instant::now(),
                },
            );
        }

        self.mfds.insert(device_info.device_path.clone(), mfd_info);
        self.led_states
            .insert(device_info.device_path.clone(), mfd_led_states);

        // Initialize MFD (turn off all LEDs)
        self.turn_off_all_leds(&device_info.device_path)?;

        Ok(())
    }

    /// Set LED state for a specific MFD and LED
    pub fn set_led(
        &mut self,
        mfd_path: &str,
        led_name: &str,
        _target: &LedTarget,
        state: &LedState,
    ) -> Result<()> {
        let _mfd_info = self
            .mfds
            .get(mfd_path)
            .ok_or_else(|| FlightError::Configuration(format!("MFD not found: {}", mfd_path)))?;

        // Check rate limiting and update LED state
        let now = Instant::now();
        let should_write = {
            let mfd_led_states = self.led_states.get_mut(mfd_path).ok_or_else(|| {
                FlightError::Configuration(format!("LED states not found for MFD: {}", mfd_path))
            })?;

            let led_state = mfd_led_states.get_mut(led_name).ok_or_else(|| {
                FlightError::Configuration(format!(
                    "LED not found: {} on MFD {}",
                    led_name, mfd_path
                ))
            })?;

            // Check rate limiting
            if now.duration_since(led_state.last_write) < self.min_write_interval {
                debug!("Rate limiting LED update for {} on {}", led_name, mfd_path);
                return Ok(());
            }

            // Update LED state
            led_state.is_on = state.on;
            led_state.brightness = state.brightness;
            led_state.blink_rate = state.blink_rate;
            led_state.last_write = now;

            true
        };

        if should_write {
            // Get the updated LED state for hardware write
            let led_state_copy = {
                let mfd_led_states = self.led_states.get(mfd_path).unwrap();
                mfd_led_states.get(led_name).unwrap().clone()
            };

            // Write to hardware
            self.write_led_to_hardware(mfd_path, led_name, &led_state_copy)?;
        }

        Ok(())
    }

    /// Write LED state to hardware via HID
    fn write_led_to_hardware(
        &mut self,
        mfd_path: &str,
        led_name: &str,
        led_state: &MfdLedState,
    ) -> Result<()> {
        let write_start = Instant::now();
        self.metrics_registry
            .inc_counter(self.device_metrics.operations_total, 1);

        // Build HID report for this MFD type
        let report = match self.build_led_report(mfd_path, led_name, led_state) {
            Ok(report) => report,
            Err(err) => {
                let write_latency = write_start.elapsed();
                self.metrics_registry.observe(
                    self.device_metrics.operation_latency_ms,
                    write_latency.as_secs_f64() * 1000.0,
                );
                self.metrics_registry
                    .inc_counter(self.device_metrics.errors_total, 1);
                return Err(err);
            }
        };

        // Write to HID device
        let write_result = self.hid_adapter.write_output(mfd_path, &report);
        let write_latency = write_start.elapsed();
        self.metrics_registry.observe(
            self.device_metrics.operation_latency_ms,
            write_latency.as_secs_f64() * 1000.0,
        );

        match write_result {
            Ok(HidOperationResult::Success { bytes_transferred }) => {
                // Track latency
                self.latency_samples.push(write_latency);
                if self.latency_samples.len() > self.max_latency_samples {
                    self.latency_samples.remove(0);
                }

                // Validate latency requirement (≤20ms)
                if write_latency > Duration::from_millis(20) {
                    warn!(
                        "LED write latency exceeded 20ms: {:?} for {} on {}",
                        write_latency, led_name, mfd_path
                    );
                }

                debug!(
                    "LED {} on {} updated: {} bytes in {:?}",
                    led_name, mfd_path, bytes_transferred, write_latency
                );
                Ok(())
            }
            Ok(HidOperationResult::Stall) => {
                self.metrics_registry
                    .inc_counter(self.device_metrics.errors_total, 1);
                error!("HID stall writing LED {} on {}", led_name, mfd_path);
                Err(FlightError::Hardware(format!(
                    "HID stall writing to {}",
                    mfd_path
                )))
            }
            Ok(HidOperationResult::Timeout) => {
                self.metrics_registry
                    .inc_counter(self.device_metrics.errors_total, 1);
                error!("HID timeout writing LED {} on {}", led_name, mfd_path);
                Err(FlightError::Hardware(format!(
                    "HID timeout writing to {}",
                    mfd_path
                )))
            }
            Ok(HidOperationResult::Error {
                error_code,
                description,
            }) => {
                self.metrics_registry
                    .inc_counter(self.device_metrics.errors_total, 1);
                error!(
                    "HID error writing LED {} on {}: {} - {}",
                    led_name, mfd_path, error_code, description
                );
                Err(FlightError::Hardware(format!(
                    "HID error {}: {}",
                    error_code, description
                )))
            }
            Err(err) => {
                self.metrics_registry
                    .inc_counter(self.device_metrics.errors_total, 1);
                Err(err)
            }
        }
    }

    /// Build HID report for LED update
    fn build_led_report(
        &self,
        mfd_path: &str,
        _led_name: &str,
        led_state: &MfdLedState,
    ) -> Result<Vec<u8>> {
        let mfd_info = self
            .mfds
            .get(mfd_path)
            .ok_or_else(|| FlightError::Configuration(format!("MFD not found: {}", mfd_path)))?;

        // Build report based on MFD type
        match mfd_info.mfd_type {
            CougarMfdType::MfdLeft => self.build_mfd_left_report(led_state),
            CougarMfdType::MfdRight => self.build_mfd_right_report(led_state),
            CougarMfdType::MfdCenter => self.build_mfd_center_report(led_state),
        }
    }

    /// Build HID report for Left MFD
    pub fn build_mfd_left_report(&self, led_state: &MfdLedState) -> Result<Vec<u8>> {
        // Cougar MFD Left HID report format
        let mut report = vec![0u8; 32];
        report[0] = 0x01; // Report ID for LED control

        // LED brightness in report bytes 1-25 (20 OSBs + 5 control LEDs)
        let brightness_value = if led_state.is_on {
            (led_state.brightness * 255.0) as u8
        } else {
            0
        };

        if (led_state.led_index as usize) < 25 {
            report[1 + led_state.led_index as usize] = brightness_value;
        }

        Ok(report)
    }

    /// Build HID report for Right MFD
    pub fn build_mfd_right_report(&self, led_state: &MfdLedState) -> Result<Vec<u8>> {
        // Cougar MFD Right HID report format (same as left)
        let mut report = vec![0u8; 32];
        report[0] = 0x01; // Report ID for LED control

        let brightness_value = if led_state.is_on {
            (led_state.brightness * 255.0) as u8
        } else {
            0
        };

        if (led_state.led_index as usize) < 25 {
            report[1 + led_state.led_index as usize] = brightness_value;
        }

        Ok(report)
    }

    /// Build HID report for Center MFD
    pub fn build_mfd_center_report(&self, led_state: &MfdLedState) -> Result<Vec<u8>> {
        // Cougar MFD Center HID report format (fewer LEDs)
        let mut report = vec![0u8; 16];
        report[0] = 0x01; // Report ID for LED control

        let brightness_value = if led_state.is_on {
            (led_state.brightness * 255.0) as u8
        } else {
            0
        };

        if (led_state.led_index as usize) < 13 {
            report[1 + led_state.led_index as usize] = brightness_value;
        }

        Ok(report)
    }

    /// Turn off all LEDs on an MFD
    fn turn_off_all_leds(&mut self, mfd_path: &str) -> Result<()> {
        let mfd_info = self
            .mfds
            .get(mfd_path)
            .ok_or_else(|| FlightError::Configuration(format!("MFD not found: {}", mfd_path)))?;

        for &led_name in mfd_info.mfd_type.led_mapping() {
            let off_state = LedState {
                on: false,
                brightness: 0.0,
                blink_rate: None,
                last_update: Instant::now(),
            };

            let target = LedTarget::Panel(led_name.to_string());
            self.set_led(mfd_path, led_name, &target, &off_state)?;
        }

        Ok(())
    }

    /// Update blinking LEDs (should be called regularly)
    pub fn update_blink_states(&mut self) -> Result<()> {
        let now = Instant::now();
        let mut updates = Vec::new();

        // Collect blink updates
        for (mfd_path, mfd_led_states) in &mut self.led_states {
            for (led_name, led_state) in mfd_led_states {
                if let Some(rate_hz) = led_state.blink_rate {
                    let period = Duration::from_secs_f32(1.0 / rate_hz);
                    let elapsed = now.duration_since(led_state.last_blink_toggle);

                    if elapsed >= period / 2 {
                        led_state.is_on = !led_state.is_on;
                        led_state.last_blink_toggle = now;

                        // Check rate limiting
                        if now.duration_since(led_state.last_write) >= self.min_write_interval {
                            updates.push((mfd_path.clone(), led_name.clone(), led_state.clone()));
                        }
                    }
                }
            }
        }

        // Apply blink updates
        for (mfd_path, led_name, led_state) in updates {
            self.write_led_to_hardware(&mfd_path, &led_name, &led_state)?;
        }

        Ok(())
    }

    /// Start verify test pattern for an MFD
    pub fn start_verify_test(&mut self, mfd_path: &str) -> Result<()> {
        let mfd_info = self
            .mfds
            .get(mfd_path)
            .ok_or_else(|| FlightError::Configuration(format!("MFD not found: {}", mfd_path)))?;

        if self.verify_state.is_some() {
            return Err(FlightError::Configuration(
                "Verify test already in progress".to_string(),
            ));
        }

        info!(
            "Starting verify test for {} MFD: {}",
            mfd_info.mfd_type.name(),
            mfd_path
        );

        let steps = mfd_info.mfd_type.verify_pattern();
        self.verify_state = Some(CougarVerifyTestState {
            mfd_path: mfd_path.to_string(),
            steps,
            current_step: 0,
            step_start_time: Instant::now(),
            test_start_time: Instant::now(),
            results: Vec::new(),
        });

        Ok(())
    }

    /// Update verify test execution
    pub fn update_verify_test(&mut self) -> Result<Option<CougarVerifyTestResult>> {
        // Extract necessary data to avoid borrowing conflicts
        let (current_step_index, steps_len, mfd_path, step_start_time, test_start_time) = {
            match &self.verify_state {
                Some(state) => (
                    state.current_step,
                    state.steps.len(),
                    state.mfd_path.clone(),
                    state.step_start_time,
                    state.test_start_time,
                ),
                None => return Ok(None),
            }
        };

        let now = Instant::now();

        if current_step_index >= steps_len {
            // Test complete
            let total_duration = now.duration_since(test_start_time);
            let results = self.verify_state.as_ref().unwrap().results.clone();
            let success = results.iter().all(|r| r.success);

            self.verify_state = None;

            return Ok(Some(CougarVerifyTestResult {
                mfd_path,
                total_duration,
                step_results: results,
                success,
            }));
        }

        let current_step = self.verify_state.as_ref().unwrap().steps[current_step_index].clone();
        let step_elapsed = now.duration_since(step_start_time);

        match current_step {
            CougarVerifyStep::LedOn(led_name) => {
                let state = LedState {
                    on: true,
                    brightness: 1.0,
                    blink_rate: None,
                    last_update: now,
                };
                let target = LedTarget::Panel(led_name.to_string());

                let step_start = Instant::now();
                let result = self.set_led(&mfd_path, led_name, &target, &state);
                let actual_latency = step_start.elapsed();

                if let Some(verify_state) = &mut self.verify_state {
                    verify_state.results.push(CougarVerifyStepResult {
                        step_index: current_step_index,
                        expected_latency: Duration::from_millis(20), // ≤20ms requirement
                        actual_latency,
                        success: result.is_ok() && actual_latency <= Duration::from_millis(20),
                        error: result.err().map(|e| e.to_string()),
                    });
                }

                self.advance_verify_step();
            }
            CougarVerifyStep::LedOff(led_name) => {
                let state = LedState {
                    on: false,
                    brightness: 0.0,
                    blink_rate: None,
                    last_update: now,
                };
                let target = LedTarget::Panel(led_name.to_string());

                let step_start = Instant::now();
                let result = self.set_led(&mfd_path, led_name, &target, &state);
                let actual_latency = step_start.elapsed();

                if let Some(verify_state) = &mut self.verify_state {
                    verify_state.results.push(CougarVerifyStepResult {
                        step_index: current_step_index,
                        expected_latency: Duration::from_millis(20),
                        actual_latency,
                        success: result.is_ok() && actual_latency <= Duration::from_millis(20),
                        error: result.err().map(|e| e.to_string()),
                    });
                }

                self.advance_verify_step();
            }
            CougarVerifyStep::LedBlink(led_name, rate_hz) => {
                let state = LedState {
                    on: false,
                    brightness: 1.0,
                    blink_rate: Some(rate_hz),
                    last_update: now,
                };
                let target = LedTarget::Panel(led_name.to_string());

                let step_start = Instant::now();
                let result = self.set_led(&mfd_path, led_name, &target, &state);
                let actual_latency = step_start.elapsed();

                if let Some(verify_state) = &mut self.verify_state {
                    verify_state.results.push(CougarVerifyStepResult {
                        step_index: current_step_index,
                        expected_latency: Duration::from_millis(20),
                        actual_latency,
                        success: result.is_ok() && actual_latency <= Duration::from_millis(20),
                        error: result.err().map(|e| e.to_string()),
                    });
                }

                self.advance_verify_step();
            }
            CougarVerifyStep::AllOn => {
                let led_mapping = self
                    .mfds
                    .get(&mfd_path)
                    .unwrap()
                    .mfd_type
                    .led_mapping()
                    .to_vec();
                let step_start = Instant::now();
                let mut all_success = true;

                for led_name in led_mapping {
                    let state = LedState {
                        on: true,
                        brightness: 1.0,
                        blink_rate: None,
                        last_update: now,
                    };
                    let target = LedTarget::Panel(led_name.to_string());

                    if self.set_led(&mfd_path, led_name, &target, &state).is_err() {
                        all_success = false;
                    }
                }

                let actual_latency = step_start.elapsed();
                if let Some(verify_state) = &mut self.verify_state {
                    verify_state.results.push(CougarVerifyStepResult {
                        step_index: current_step_index,
                        expected_latency: Duration::from_millis(20),
                        actual_latency,
                        success: all_success && actual_latency <= Duration::from_millis(20),
                        error: if all_success {
                            None
                        } else {
                            Some("Failed to turn on all LEDs".to_string())
                        },
                    });
                }

                self.advance_verify_step();
            }
            CougarVerifyStep::AllOff => {
                let step_start = Instant::now();
                let result = self.turn_off_all_leds(&mfd_path);
                let actual_latency = step_start.elapsed();

                if let Some(verify_state) = &mut self.verify_state {
                    verify_state.results.push(CougarVerifyStepResult {
                        step_index: current_step_index,
                        expected_latency: Duration::from_millis(20),
                        actual_latency,
                        success: result.is_ok() && actual_latency <= Duration::from_millis(20),
                        error: result.err().map(|e| e.to_string()),
                    });
                }

                self.advance_verify_step();
            }
            CougarVerifyStep::Delay(duration) => {
                if step_elapsed >= duration {
                    self.advance_verify_step();
                }
            }
        }

        Ok(None)
    }

    /// Advance to next verify test step
    fn advance_verify_step(&mut self) {
        if let Some(verify_state) = &mut self.verify_state {
            verify_state.current_step += 1;
            verify_state.step_start_time = Instant::now();
        }
    }

    /// Get connected MFDs
    pub fn get_mfds(&self) -> Vec<&MfdInfo> {
        self.mfds.values().collect()
    }

    /// Get minimum write interval for rate limiting
    pub fn get_min_write_interval(&self) -> Duration {
        self.min_write_interval
    }

    /// Get latency statistics
    pub fn get_latency_stats(&self) -> Option<LatencyStats> {
        if self.latency_samples.is_empty() {
            return None;
        }

        let mut samples: Vec<_> = self.latency_samples.iter().map(|d| d.as_nanos()).collect();
        samples.sort_unstable();

        let len = samples.len();
        let mean = samples.iter().sum::<u128>() / len as u128;
        let p99_index = (len as f64 * 0.99) as usize;
        let p99 = samples.get(p99_index).copied().unwrap_or(samples[len - 1]);
        let max = samples[len - 1];

        Some(LatencyStats {
            mean_ns: mean,
            p99_ns: p99,
            max_ns: max,
            sample_count: len,
        })
    }

    /// Check MFD health and detect drift
    pub fn check_mfd_health(&mut self, mfd_path: &str) -> Result<CougarMfdHealthStatus> {
        let now = Instant::now();
        let mfd_type = {
            let mfd_info = self.mfds.get_mut(mfd_path).ok_or_else(|| {
                FlightError::Configuration(format!("MFD not found: {}", mfd_path))
            })?;

            mfd_info.last_health_check = now;
            mfd_info.mfd_type
        };

        // Check if MFD is responsive by attempting a simple LED operation
        let is_responsive = self.check_mfd_responsiveness(mfd_path)?;

        // Check for configuration drift
        let drift_detected = self.detect_mfd_drift(mfd_path)?;

        Ok(CougarMfdHealthStatus {
            mfd_path: mfd_path.to_string(),
            mfd_type,
            is_responsive,
            drift_detected,
            last_check: now,
            hid_events: self
                .hid_adapter
                .check_endpoint_health(mfd_path)
                .unwrap_or_default(),
        })
    }

    /// Check if MFD is responsive
    fn check_mfd_responsiveness(&mut self, mfd_path: &str) -> Result<bool> {
        // Try to write a simple LED state and check for errors
        let test_state = LedState {
            on: false,
            brightness: 0.0,
            blink_rate: None,
            last_update: Instant::now(),
        };

        let target = LedTarget::Panel("OSB1".to_string());
        match self.set_led(mfd_path, "OSB1", &target, &test_state) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Detect MFD configuration drift
    fn detect_mfd_drift(&mut self, _mfd_path: &str) -> Result<bool> {
        // TODO: Implement drift detection logic
        // 1. Read current LED states from hardware
        // 2. Compare with expected states
        // 3. Detect configuration drift

        // For now, simulate drift detection
        Ok(false)
    }

    /// Repair MFD configuration drift
    pub fn repair_mfd_drift(&mut self, mfd_path: &str) -> Result<()> {
        info!("Repairing MFD configuration drift for: {}", mfd_path);

        // Turn off all LEDs and reinitialize
        self.turn_off_all_leds(mfd_path)?;

        // Reset LED states
        if let Some(mfd_led_states) = self.led_states.get_mut(mfd_path) {
            for led_state in mfd_led_states.values_mut() {
                led_state.is_on = false;
                led_state.brightness = 0.0;
                led_state.blink_rate = None;
                led_state.last_write = Instant::now();
            }
        }

        Ok(())
    }
}

impl CougarVerifyTestResult {
    /// Check if latency requirement is met (≤20ms)
    pub fn meets_latency_requirement(&self) -> bool {
        self.step_results
            .iter()
            .all(|result| result.actual_latency <= Duration::from_millis(20))
    }

    /// Get maximum latency from all steps
    pub fn max_latency(&self) -> Duration {
        self.step_results
            .iter()
            .map(|result| result.actual_latency)
            .max()
            .unwrap_or(Duration::ZERO)
    }

    /// Get average latency from all steps
    pub fn avg_latency(&self) -> Duration {
        if self.step_results.is_empty() {
            return Duration::ZERO;
        }

        let total_nanos: u128 = self
            .step_results
            .iter()
            .map(|result| result.actual_latency.as_nanos())
            .sum();

        Duration::from_nanos((total_nanos / self.step_results.len() as u128) as u64)
    }
}

#[cfg(test)]
mod tests;
