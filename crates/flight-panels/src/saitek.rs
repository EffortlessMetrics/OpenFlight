//! Saitek/Logitech panel HID driver implementation
//!
//! Provides HID communication for common Saitek/Logitech flight panels including:
//! - Radio Panel
//! - Multi Panel  
//! - Switch Panel
//! - BIP (Backlighting Instrument Panel)
//!
//! Implements ≤20ms latency requirement with verify test patterns and drift detection.

use crate::led::{LedTarget, LedState, LatencyStats};
use flight_core::{Result, FlightError};
use flight_hid::{HidAdapter, HidOperationResult, HidDeviceInfo};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, warn, error, info};

/// Saitek/Logitech panel vendor ID
const SAITEK_VENDOR_ID: u16 = 0x06A3;
const LOGITECH_VENDOR_ID: u16 = 0x046D;

/// Known Saitek/Logitech panel product IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelType {
    RadioPanel = 0x0D05,
    MultiPanel = 0x0D06,
    SwitchPanel = 0x0D67,
    BIP = 0x0B4E,
    FIP = 0x0A2F,
}

impl PanelType {
    /// Get panel type from product ID
    pub fn from_product_id(pid: u16) -> Option<Self> {
        match pid {
            0x0D05 => Some(PanelType::RadioPanel),
            0x0D06 => Some(PanelType::MultiPanel),
            0x0D67 => Some(PanelType::SwitchPanel),
            0x0B4E => Some(PanelType::BIP),
            0x0A2F => Some(PanelType::FIP),
            _ => None,
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            PanelType::RadioPanel => "Radio Panel",
            PanelType::MultiPanel => "Multi Panel",
            PanelType::SwitchPanel => "Switch Panel",
            PanelType::BIP => "Backlighting Instrument Panel",
            PanelType::FIP => "Flight Instrument Panel",
        }
    }

    /// Get LED mapping for this panel type
    pub fn led_mapping(&self) -> &'static [&'static str] {
        match self {
            PanelType::RadioPanel => &["COM1", "COM2", "NAV1", "NAV2", "ADF", "DME", "XPDR"],
            PanelType::MultiPanel => &["ALT", "VS", "IAS", "HDG", "CRS", "AUTOTHROTTLE", "FLAPS", "PITCHTRIM"],
            PanelType::SwitchPanel => &["GEAR", "MASTER_BAT", "MASTER_ALT", "AVIONICS", "FUEL_PUMP", "DEICE", "PITOT_HEAT", "COWL"],
            PanelType::BIP => &["GEAR_L", "GEAR_N", "GEAR_R", "MASTER_WARNING", "MASTER_CAUTION", "FIRE_WARNING", "OIL_PRESSURE", "FUEL_PRESSURE"],
            PanelType::FIP => &["ATTITUDE", "AIRSPEED", "ALTITUDE", "HSI", "TURN_COORD", "VOR1", "VOR2", "ADF"],
        }
    }

    /// Get verify test pattern for this panel type
    pub fn verify_pattern(&self) -> Vec<VerifyStep> {
        match self {
            PanelType::RadioPanel => vec![
                VerifyStep::LedOn("COM1"),
                VerifyStep::Delay(Duration::from_millis(100)),
                VerifyStep::LedOff("COM1"),
                VerifyStep::LedOn("NAV1"),
                VerifyStep::Delay(Duration::from_millis(100)),
                VerifyStep::LedOff("NAV1"),
                VerifyStep::AllOff,
            ],
            PanelType::MultiPanel => vec![
                VerifyStep::LedOn("ALT"),
                VerifyStep::LedOn("VS"),
                VerifyStep::Delay(Duration::from_millis(150)),
                VerifyStep::AllOff,
                VerifyStep::LedBlink("AUTOTHROTTLE", 4.0),
                VerifyStep::Delay(Duration::from_millis(500)),
                VerifyStep::AllOff,
            ],
            PanelType::SwitchPanel => vec![
                VerifyStep::LedOn("GEAR"),
                VerifyStep::Delay(Duration::from_millis(100)),
                VerifyStep::LedOn("MASTER_BAT"),
                VerifyStep::Delay(Duration::from_millis(100)),
                VerifyStep::LedOn("AVIONICS"),
                VerifyStep::Delay(Duration::from_millis(100)),
                VerifyStep::AllOff,
            ],
            PanelType::BIP => vec![
                VerifyStep::AllOn,
                VerifyStep::Delay(Duration::from_millis(200)),
                VerifyStep::AllOff,
                VerifyStep::LedBlink("MASTER_WARNING", 6.0),
                VerifyStep::Delay(Duration::from_millis(500)),
                VerifyStep::AllOff,
            ],
            PanelType::FIP => vec![
                VerifyStep::LedOn("ATTITUDE"),
                VerifyStep::LedOn("AIRSPEED"),
                VerifyStep::LedOn("ALTITUDE"),
                VerifyStep::Delay(Duration::from_millis(200)),
                VerifyStep::AllOff,
            ],
        }
    }
}

/// Verify test pattern step
#[derive(Debug, Clone)]
pub enum VerifyStep {
    LedOn(&'static str),
    LedOff(&'static str),
    LedBlink(&'static str, f32), // LED name, rate_hz
    AllOn,
    AllOff,
    Delay(Duration),
}

/// Panel LED state for HID communication
#[derive(Debug, Clone)]
struct PanelLedState {
    /// LED index in HID report
    led_index: u8,
    /// Current brightness (0.0-1.0)
    brightness: f32,
    /// Whether LED is currently on
    is_on: bool,
    /// Blink rate if blinking
    blink_rate: Option<f32>,
    /// Last blink state change
    last_blink_toggle: Instant,
    /// Last HID write time
    last_write: Instant,
}

/// Saitek/Logitech panel writer with HID communication
pub struct SaitekPanelWriter {
    /// HID adapter for device communication
    hid_adapter: HidAdapter,
    /// Connected panels by device path
    panels: HashMap<String, PanelInfo>,
    /// LED states by panel and LED name
    led_states: HashMap<String, HashMap<String, PanelLedState>>,
    /// Latency tracking
    latency_samples: Vec<Duration>,
    /// Maximum latency samples to keep
    max_latency_samples: usize,
    /// Minimum write interval for rate limiting (≥8ms per requirements)
    min_write_interval: Duration,
    /// Verify test state
    verify_state: Option<VerifyTestState>,
}

/// Panel information
#[derive(Debug, Clone)]
pub struct PanelInfo {
    pub device_info: HidDeviceInfo,
    pub panel_type: PanelType,
    pub last_health_check: Instant,
}

/// Verify test execution state
#[derive(Debug)]
struct VerifyTestState {
    panel_path: String,
    steps: Vec<VerifyStep>,
    current_step: usize,
    step_start_time: Instant,
    test_start_time: Instant,
    results: Vec<VerifyStepResult>,
}

/// Result of a verify test step
#[derive(Debug, Clone)]
pub struct VerifyStepResult {
    pub step_index: usize,
    pub expected_latency: Duration,
    pub actual_latency: Duration,
    pub success: bool,
    pub error: Option<String>,
}

impl SaitekPanelWriter {
    /// Create new Saitek panel writer
    pub fn new(hid_adapter: HidAdapter) -> Self {
        Self {
            hid_adapter,
            panels: HashMap::new(),
            led_states: HashMap::new(),
            latency_samples: Vec::new(),
            max_latency_samples: 1000,
            min_write_interval: Duration::from_millis(8), // ≥8ms per requirements
            verify_state: None,
        }
    }

    /// Start the panel writer and enumerate devices
    pub fn start(&mut self) -> Result<()> {
        info!("Starting Saitek/Logitech panel writer");
        
        self.hid_adapter.start()?;
        self.enumerate_panels()?;
        
        Ok(())
    }

    /// Stop the panel writer
    pub fn stop(&mut self) {
        info!("Stopping Saitek/Logitech panel writer");
        
        // Turn off all LEDs before stopping
        let panel_paths: Vec<_> = self.panels.keys().cloned().collect();
        for panel_path in panel_paths {
            if let Err(e) = self.turn_off_all_leds(&panel_path) {
                warn!("Failed to turn off LEDs for panel {}: {}", panel_path, e);
            }
        }
        
        self.hid_adapter.stop();
        self.panels.clear();
        self.led_states.clear();
    }

    /// Enumerate and register Saitek/Logitech panels
    fn enumerate_panels(&mut self) -> Result<()> {
        debug!("Enumerating Saitek/Logitech panels");
        
        let devices: Vec<_> = self.hid_adapter.get_all_devices().into_iter().cloned().collect();
        
        for device_info in devices {
            if self.is_supported_panel(&device_info) {
                self.register_panel(device_info)?;
            }
        }
        
        info!("Found {} Saitek/Logitech panels", self.panels.len());
        Ok(())
    }

    /// Check if device is a supported Saitek/Logitech panel
    fn is_supported_panel(&self, device_info: &HidDeviceInfo) -> bool {
        let is_saitek_logitech = device_info.vendor_id == SAITEK_VENDOR_ID || 
                                device_info.vendor_id == LOGITECH_VENDOR_ID;
        
        if !is_saitek_logitech {
            return false;
        }
        
        PanelType::from_product_id(device_info.product_id).is_some()
    }

    /// Register a new panel
    fn register_panel(&mut self, device_info: HidDeviceInfo) -> Result<()> {
        let panel_type = PanelType::from_product_id(device_info.product_id)
            .ok_or_else(|| FlightError::Configuration(format!(
                "Unsupported panel product ID: {:04X}", device_info.product_id
            )))?;

        info!("Registering {} panel: {}", panel_type.name(), device_info.device_path);

        let panel_info = PanelInfo {
            device_info: device_info.clone(),
            panel_type,
            last_health_check: Instant::now(),
        };

        // Initialize LED states for this panel
        let mut panel_led_states = HashMap::new();
        for (index, &led_name) in panel_type.led_mapping().iter().enumerate() {
            panel_led_states.insert(led_name.to_string(), PanelLedState {
                led_index: index as u8,
                brightness: 0.0,
                is_on: false,
                blink_rate: None,
                last_blink_toggle: Instant::now(),
                last_write: Instant::now(),
            });
        }

        self.panels.insert(device_info.device_path.clone(), panel_info);
        self.led_states.insert(device_info.device_path.clone(), panel_led_states);

        // Initialize panel (turn off all LEDs)
        self.turn_off_all_leds(&device_info.device_path)?;

        Ok(())
    }

    /// Set LED state for a specific panel and LED
    pub fn set_led(&mut self, panel_path: &str, led_name: &str, _target: &LedTarget, state: &LedState) -> Result<()> {
        let _panel_info = self.panels.get(panel_path)
            .ok_or_else(|| FlightError::Configuration(format!("Panel not found: {}", panel_path)))?;

        // Check rate limiting and update LED state
        let now = Instant::now();
        let should_write = {
            let panel_led_states = self.led_states.get_mut(panel_path)
                .ok_or_else(|| FlightError::Configuration(format!("LED states not found for panel: {}", panel_path)))?;

            let led_state = panel_led_states.get_mut(led_name)
                .ok_or_else(|| FlightError::Configuration(format!("LED not found: {} on panel {}", led_name, panel_path)))?;

            // Check rate limiting
            if now.duration_since(led_state.last_write) < self.min_write_interval {
                debug!("Rate limiting LED update for {} on {}", led_name, panel_path);
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
                let panel_led_states = self.led_states.get(panel_path).unwrap();
                panel_led_states.get(led_name).unwrap().clone()
            };
            
            // Write to hardware
            self.write_led_to_hardware(panel_path, led_name, &led_state_copy)?;
        }

        Ok(())
    }

    /// Write LED state to hardware via HID
    fn write_led_to_hardware(&mut self, panel_path: &str, led_name: &str, led_state: &PanelLedState) -> Result<()> {
        let write_start = Instant::now();

        // Build HID report for this panel type
        let report = self.build_led_report(panel_path, led_name, led_state)?;

        // Write to HID device
        match self.hid_adapter.write_output(panel_path, &report)? {
            HidOperationResult::Success { bytes_transferred } => {
                let write_latency = write_start.elapsed();
                
                // Track latency
                self.latency_samples.push(write_latency);
                if self.latency_samples.len() > self.max_latency_samples {
                    self.latency_samples.remove(0);
                }

                // Validate latency requirement (≤20ms)
                if write_latency > Duration::from_millis(20) {
                    warn!("LED write latency exceeded 20ms: {:?} for {} on {}", 
                          write_latency, led_name, panel_path);
                }

                debug!("LED {} on {} updated: {} bytes in {:?}", 
                       led_name, panel_path, bytes_transferred, write_latency);
                Ok(())
            }
            HidOperationResult::Stall => {
                error!("HID stall writing LED {} on {}", led_name, panel_path);
                Err(FlightError::Hardware(format!("HID stall writing to {}", panel_path)))
            }
            HidOperationResult::Timeout => {
                error!("HID timeout writing LED {} on {}", led_name, panel_path);
                Err(FlightError::Hardware(format!("HID timeout writing to {}", panel_path)))
            }
            HidOperationResult::Error { error_code, description } => {
                error!("HID error writing LED {} on {}: {} - {}", 
                       led_name, panel_path, error_code, description);
                Err(FlightError::Hardware(format!("HID error {}: {}", error_code, description)))
            }
        }
    }

    /// Build HID report for LED update
    fn build_led_report(&self, panel_path: &str, led_name: &str, led_state: &PanelLedState) -> Result<Vec<u8>> {
        let panel_info = self.panels.get(panel_path)
            .ok_or_else(|| FlightError::Configuration(format!("Panel not found: {}", panel_path)))?;

        // Build report based on panel type
        match panel_info.panel_type {
            PanelType::RadioPanel => self.build_radio_panel_report(led_state),
            PanelType::MultiPanel => self.build_multi_panel_report(led_state),
            PanelType::SwitchPanel => self.build_switch_panel_report(led_state),
            PanelType::BIP => self.build_bip_report(led_state),
            PanelType::FIP => self.build_fip_report(led_state),
        }
    }

    /// Build HID report for Radio Panel
    fn build_radio_panel_report(&self, led_state: &PanelLedState) -> Result<Vec<u8>> {
        // Radio Panel HID report format (simplified)
        let mut report = vec![0u8; 8];
        report[0] = 0x00; // Report ID
        
        // LED brightness in report byte 1-2
        let brightness_value = if led_state.is_on {
            (led_state.brightness * 255.0) as u8
        } else {
            0
        };
        
        report[1 + led_state.led_index as usize] = brightness_value;
        
        Ok(report)
    }

    /// Build HID report for Multi Panel
    fn build_multi_panel_report(&self, led_state: &PanelLedState) -> Result<Vec<u8>> {
        // Multi Panel HID report format (simplified)
        let mut report = vec![0u8; 12];
        report[0] = 0x00; // Report ID
        
        let brightness_value = if led_state.is_on {
            (led_state.brightness * 255.0) as u8
        } else {
            0
        };
        
        report[1 + led_state.led_index as usize] = brightness_value;
        
        Ok(report)
    }

    /// Build HID report for Switch Panel
    fn build_switch_panel_report(&self, led_state: &PanelLedState) -> Result<Vec<u8>> {
        // Switch Panel HID report format (simplified)
        let mut report = vec![0u8; 8];
        report[0] = 0x00; // Report ID
        
        // Switch panel uses bit-packed LED states
        if led_state.is_on {
            let byte_index = 1 + (led_state.led_index / 8) as usize;
            let bit_index = led_state.led_index % 8;
            report[byte_index] |= 1 << bit_index;
        }
        
        Ok(report)
    }

    /// Build HID report for BIP (Backlighting Instrument Panel)
    fn build_bip_report(&self, led_state: &PanelLedState) -> Result<Vec<u8>> {
        // BIP HID report format (simplified)
        let mut report = vec![0u8; 16];
        report[0] = 0x00; // Report ID
        
        let brightness_value = if led_state.is_on {
            (led_state.brightness * 255.0) as u8
        } else {
            0
        };
        
        report[1 + led_state.led_index as usize] = brightness_value;
        
        Ok(report)
    }

    /// Build HID report for FIP (Flight Instrument Panel)
    fn build_fip_report(&self, led_state: &PanelLedState) -> Result<Vec<u8>> {
        // FIP HID report format (simplified)
        let mut report = vec![0u8; 32];
        report[0] = 0x00; // Report ID
        
        let brightness_value = if led_state.is_on {
            (led_state.brightness * 255.0) as u8
        } else {
            0
        };
        
        report[1 + led_state.led_index as usize] = brightness_value;
        
        Ok(report)
    }

    /// Turn off all LEDs on a panel
    fn turn_off_all_leds(&mut self, panel_path: &str) -> Result<()> {
        let panel_info = self.panels.get(panel_path)
            .ok_or_else(|| FlightError::Configuration(format!("Panel not found: {}", panel_path)))?;

        for &led_name in panel_info.panel_type.led_mapping() {
            let off_state = LedState {
                on: false,
                brightness: 0.0,
                blink_rate: None,
                last_update: Instant::now(),
            };
            
            let target = LedTarget::Panel(led_name.to_string());
            self.set_led(panel_path, led_name, &target, &off_state)?;
        }

        Ok(())
    }

    /// Update blinking LEDs (should be called regularly)
    pub fn update_blink_states(&mut self) -> Result<()> {
        let now = Instant::now();
        let mut updates = Vec::new();

        // Collect blink updates
        for (panel_path, panel_led_states) in &mut self.led_states {
            for (led_name, led_state) in panel_led_states {
                if let Some(rate_hz) = led_state.blink_rate {
                    let period = Duration::from_secs_f32(1.0 / rate_hz);
                    let elapsed = now.duration_since(led_state.last_blink_toggle);
                    
                    if elapsed >= period / 2 {
                        led_state.is_on = !led_state.is_on;
                        led_state.last_blink_toggle = now;
                        
                        // Check rate limiting
                        if now.duration_since(led_state.last_write) >= self.min_write_interval {
                            updates.push((panel_path.clone(), led_name.clone(), led_state.clone()));
                        }
                    }
                }
            }
        }

        // Apply blink updates
        for (panel_path, led_name, led_state) in updates {
            self.write_led_to_hardware(&panel_path, &led_name, &led_state)?;
        }

        Ok(())
    }

    /// Start verify test pattern for a panel
    pub fn start_verify_test(&mut self, panel_path: &str) -> Result<()> {
        let panel_info = self.panels.get(panel_path)
            .ok_or_else(|| FlightError::Configuration(format!("Panel not found: {}", panel_path)))?;

        if self.verify_state.is_some() {
            return Err(FlightError::Configuration("Verify test already in progress".to_string()));
        }

        info!("Starting verify test for {} panel: {}", panel_info.panel_type.name(), panel_path);

        let steps = panel_info.panel_type.verify_pattern();
        self.verify_state = Some(VerifyTestState {
            panel_path: panel_path.to_string(),
            steps,
            current_step: 0,
            step_start_time: Instant::now(),
            test_start_time: Instant::now(),
            results: Vec::new(),
        });

        Ok(())
    }

    /// Update verify test execution
    pub fn update_verify_test(&mut self) -> Result<Option<VerifyTestResult>> {
        // Extract necessary data to avoid borrowing conflicts
        let (current_step_index, steps_len, panel_path, step_start_time, test_start_time) = {
            match &self.verify_state {
                Some(state) => (
                    state.current_step,
                    state.steps.len(),
                    state.panel_path.clone(),
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
            
            return Ok(Some(VerifyTestResult {
                panel_path,
                total_duration,
                step_results: results,
                success,
            }));
        }

        let current_step = self.verify_state.as_ref().unwrap().steps[current_step_index].clone();
        let step_elapsed = now.duration_since(step_start_time);

        match current_step {
            VerifyStep::LedOn(led_name) => {
                let state = LedState {
                    on: true,
                    brightness: 1.0,
                    blink_rate: None,
                    last_update: now,
                };
                let target = LedTarget::Panel(led_name.to_string());
                
                let step_start = Instant::now();
                let result = self.set_led(&panel_path, &led_name, &target, &state);
                let actual_latency = step_start.elapsed();
                
                if let Some(verify_state) = &mut self.verify_state {
                    verify_state.results.push(VerifyStepResult {
                        step_index: current_step_index,
                        expected_latency: Duration::from_millis(20), // ≤20ms requirement
                        actual_latency,
                        success: result.is_ok() && actual_latency <= Duration::from_millis(20),
                        error: result.err().map(|e| e.to_string()),
                    });
                }
                
                self.advance_verify_step();
            }
            VerifyStep::LedOff(led_name) => {
                let state = LedState {
                    on: false,
                    brightness: 0.0,
                    blink_rate: None,
                    last_update: now,
                };
                let target = LedTarget::Panel(led_name.to_string());
                
                let step_start = Instant::now();
                let result = self.set_led(&panel_path, &led_name, &target, &state);
                let actual_latency = step_start.elapsed();
                
                if let Some(verify_state) = &mut self.verify_state {
                    verify_state.results.push(VerifyStepResult {
                        step_index: current_step_index,
                        expected_latency: Duration::from_millis(20),
                        actual_latency,
                        success: result.is_ok() && actual_latency <= Duration::from_millis(20),
                        error: result.err().map(|e| e.to_string()),
                    });
                }
                
                self.advance_verify_step();
            }
            VerifyStep::LedBlink(led_name, rate_hz) => {
                let state = LedState {
                    on: false,
                    brightness: 1.0,
                    blink_rate: Some(rate_hz),
                    last_update: now,
                };
                let target = LedTarget::Panel(led_name.to_string());
                
                let step_start = Instant::now();
                let result = self.set_led(&panel_path, &led_name, &target, &state);
                let actual_latency = step_start.elapsed();
                
                if let Some(verify_state) = &mut self.verify_state {
                    verify_state.results.push(VerifyStepResult {
                        step_index: current_step_index,
                        expected_latency: Duration::from_millis(20),
                        actual_latency,
                        success: result.is_ok() && actual_latency <= Duration::from_millis(20),
                        error: result.err().map(|e| e.to_string()),
                    });
                }
                
                self.advance_verify_step();
            }
            VerifyStep::AllOn => {
                let led_mapping = self.panels.get(&panel_path).unwrap().panel_type.led_mapping().to_vec();
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
                    
                    if self.set_led(&panel_path, led_name, &target, &state).is_err() {
                        all_success = false;
                    }
                }
                
                let actual_latency = step_start.elapsed();
                if let Some(verify_state) = &mut self.verify_state {
                    verify_state.results.push(VerifyStepResult {
                        step_index: current_step_index,
                        expected_latency: Duration::from_millis(20),
                        actual_latency,
                        success: all_success && actual_latency <= Duration::from_millis(20),
                        error: if all_success { None } else { Some("Failed to turn on all LEDs".to_string()) },
                    });
                }
                
                self.advance_verify_step();
            }
            VerifyStep::AllOff => {
                let step_start = Instant::now();
                let result = self.turn_off_all_leds(&panel_path);
                let actual_latency = step_start.elapsed();
                
                if let Some(verify_state) = &mut self.verify_state {
                    verify_state.results.push(VerifyStepResult {
                        step_index: current_step_index,
                        expected_latency: Duration::from_millis(20),
                        actual_latency,
                        success: result.is_ok() && actual_latency <= Duration::from_millis(20),
                        error: result.err().map(|e| e.to_string()),
                    });
                }
                
                self.advance_verify_step();
            }
            VerifyStep::Delay(duration) => {
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

    /// Get connected panels
    pub fn get_panels(&self) -> Vec<&PanelInfo> {
        self.panels.values().collect()
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

    /// Check panel health and detect drift
    pub fn check_panel_health(&mut self, panel_path: &str) -> Result<PanelHealthStatus> {
        let now = Instant::now();
        let panel_type = {
            let panel_info = self.panels.get_mut(panel_path)
                .ok_or_else(|| FlightError::Configuration(format!("Panel not found: {}", panel_path)))?;
            
            panel_info.last_health_check = now;
            panel_info.panel_type
        };

        // Check HID endpoint health
        let hid_events = self.hid_adapter.check_endpoint_health(panel_path)?;
        
        // Check LED state consistency (drift detection)
        let drift_detected = self.detect_led_drift(panel_path)?;

        Ok(PanelHealthStatus {
            panel_path: panel_path.to_string(),
            panel_type,
            is_responsive: hid_events.is_empty(),
            drift_detected,
            last_check: now,
            hid_events,
        })
    }

    /// Detect LED state drift
    fn detect_led_drift(&self, panel_path: &str) -> Result<bool> {
        // In a real implementation, this would:
        // 1. Read back LED states from hardware if supported
        // 2. Compare with expected states
        // 3. Detect configuration drift
        
        // For now, simulate drift detection
        Ok(false)
    }

    /// Repair panel configuration drift
    pub fn repair_panel_drift(&mut self, panel_path: &str) -> Result<()> {
        info!("Repairing panel configuration drift for: {}", panel_path);
        
        // Turn off all LEDs and reinitialize
        self.turn_off_all_leds(panel_path)?;
        
        // Reset LED states
        if let Some(panel_led_states) = self.led_states.get_mut(panel_path) {
            for led_state in panel_led_states.values_mut() {
                led_state.is_on = false;
                led_state.brightness = 0.0;
                led_state.blink_rate = None;
                led_state.last_write = Instant::now();
            }
        }
        
        Ok(())
    }
}

/// Verify test result
#[derive(Debug, Clone)]
pub struct VerifyTestResult {
    pub panel_path: String,
    pub total_duration: Duration,
    pub step_results: Vec<VerifyStepResult>,
    pub success: bool,
}

/// Panel health status
#[derive(Debug, Clone)]
pub struct PanelHealthStatus {
    pub panel_path: String,
    pub panel_type: PanelType,
    pub is_responsive: bool,
    pub drift_detected: bool,
    pub last_check: Instant,
    pub hid_events: Vec<flight_core::WatchdogEvent>,
}

impl VerifyTestResult {
    /// Check if latency requirement is met (≤20ms)
    pub fn meets_latency_requirement(&self) -> bool {
        self.step_results.iter().all(|result| {
            result.actual_latency <= Duration::from_millis(20)
        })
    }

    /// Get maximum latency from all steps
    pub fn max_latency(&self) -> Duration {
        self.step_results.iter()
            .map(|result| result.actual_latency)
            .max()
            .unwrap_or(Duration::ZERO)
    }

    /// Get average latency from all steps
    pub fn avg_latency(&self) -> Duration {
        if self.step_results.is_empty() {
            return Duration::ZERO;
        }

        let total_nanos: u128 = self.step_results.iter()
            .map(|result| result.actual_latency.as_nanos())
            .sum();
        
        Duration::from_nanos((total_nanos / self.step_results.len() as u128) as u64)
    }
}

#[cfg(test)]
pub mod tests;