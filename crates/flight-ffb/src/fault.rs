// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fault detection and handling for force feedback safety
//!
//! Implements comprehensive fault detection matrix with immediate safety responses.
//! All faults trigger torque-to-zero within 50ms and appropriate recovery actions.
//! Includes pre-fault capture system for diagnostics and stable error codes.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::TimeSource;

/// Types of faults that can be detected
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FaultType {
    /// USB output endpoint stalled for 3+ frames
    UsbStall,
    /// USB endpoint error or wedged state
    EndpointError,
    /// NaN or invalid value in pipeline
    NanValue,
    /// Device over-temperature condition
    OverTemp,
    /// Device over-current condition
    OverCurrent,
    /// Plugin exceeded time budget
    PluginOverrun,
    /// USB endpoint completely wedged
    EndpointWedged,
    /// Device encoder providing invalid readings
    EncoderInvalid,
    /// General device communication timeout
    DeviceTimeout,
    /// Device disconnected unexpectedly
    ///
    /// **Category:** Transient - can be cleared after reconnection
    /// **Response:** 50ms ramp to zero (if possible), audio cue
    /// **Detection:** Within 100ms of actual disconnect
    ///
    /// **Validates: Requirement FFB-SAFETY-01.8**
    DeviceDisconnect,
    /// User-initiated emergency stop (UI button)
    ///
    /// **Category:** Transient - can be cleared via user action
    /// **Response:** Immediate 50ms ramp to zero, audio cue
    ///
    /// **Validates: Requirement FFB-SAFETY-04**
    UserEmergencyStop,
    /// Hardware emergency stop button pressed
    ///
    /// **Category:** Transient - can be cleared via user action
    /// **Response:** Immediate 50ms ramp to zero, audio cue
    ///
    /// **Validates: Requirement FFB-SAFETY-04**
    HardwareEmergencyStop,
}

impl FaultType {
    /// Get stable error code for this fault type
    pub fn error_code(&self) -> &'static str {
        match self {
            FaultType::UsbStall => "HID_OUT_STALL",
            FaultType::EndpointError => "HID_ENDPOINT_ERROR",
            FaultType::NanValue => "AXIS_NAN_VALUE",
            FaultType::OverTemp => "FFB_OVER_TEMP",
            FaultType::OverCurrent => "FFB_OVER_CURRENT",
            FaultType::PluginOverrun => "PLUG_OVERRUN",
            FaultType::EndpointWedged => "HID_ENDPOINT_WEDGED",
            FaultType::EncoderInvalid => "ENCODER_INVALID",
            FaultType::DeviceTimeout => "DEVICE_TIMEOUT",
            FaultType::DeviceDisconnect => "DEVICE_DISCONNECT",
            FaultType::UserEmergencyStop => "FFB_USER_ESTOP",
            FaultType::HardwareEmergencyStop => "FFB_HW_ESTOP",
        }
    }

    /// Get knowledge base article URL for this fault
    pub fn kb_article_url(&self) -> &'static str {
        match self {
            FaultType::UsbStall => "https://docs.flight-hub.dev/kb/hid-out-stall",
            FaultType::EndpointError => "https://docs.flight-hub.dev/kb/hid-endpoint-error",
            FaultType::NanValue => "https://docs.flight-hub.dev/kb/axis-nan-value",
            FaultType::OverTemp => "https://docs.flight-hub.dev/kb/ffb-over-temp",
            FaultType::OverCurrent => "https://docs.flight-hub.dev/kb/ffb-over-current",
            FaultType::PluginOverrun => "https://docs.flight-hub.dev/kb/plug-overrun",
            FaultType::EndpointWedged => "https://docs.flight-hub.dev/kb/hid-endpoint-wedged",
            FaultType::EncoderInvalid => "https://docs.flight-hub.dev/kb/encoder-invalid",
            FaultType::DeviceTimeout => "https://docs.flight-hub.dev/kb/device-timeout",
            FaultType::DeviceDisconnect => "https://docs.flight-hub.dev/kb/device-disconnect",
            FaultType::UserEmergencyStop => "https://docs.flight-hub.dev/kb/ffb-emergency-stop",
            FaultType::HardwareEmergencyStop => "https://docs.flight-hub.dev/kb/ffb-emergency-stop",
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            FaultType::UsbStall => "USB output endpoint stalled",
            FaultType::EndpointError => "USB endpoint error or wedged",
            FaultType::NanValue => "Invalid NaN value in axis pipeline",
            FaultType::OverTemp => "Device over-temperature protection",
            FaultType::OverCurrent => "Device over-current protection",
            FaultType::PluginOverrun => "Plugin exceeded time budget",
            FaultType::EndpointWedged => "USB endpoint completely wedged",
            FaultType::EncoderInvalid => "Device encoder providing invalid readings",
            FaultType::DeviceTimeout => "Device communication timeout",
            FaultType::DeviceDisconnect => "Device disconnected unexpectedly",
            FaultType::UserEmergencyStop => "User-initiated emergency stop",
            FaultType::HardwareEmergencyStop => "Hardware emergency stop button pressed",
        }
    }

    /// Check if this fault requires immediate torque cutoff
    pub fn requires_torque_cutoff(&self) -> bool {
        match self {
            FaultType::UsbStall
            | FaultType::EndpointError
            | FaultType::NanValue
            | FaultType::OverTemp
            | FaultType::OverCurrent
            | FaultType::EndpointWedged
            | FaultType::EncoderInvalid
            | FaultType::DeviceTimeout
            | FaultType::DeviceDisconnect
            | FaultType::UserEmergencyStop
            | FaultType::HardwareEmergencyStop => true,
            FaultType::PluginOverrun => false, // Plugin faults don't affect FFB
        }
    }

    /// Get maximum allowed response time for this fault
    ///
    /// **Validates: Requirement FFB-SAFETY-01.8**
    /// Device disconnect detection should be within 100ms
    pub fn max_response_time(&self) -> Duration {
        match self {
            FaultType::UsbStall
            | FaultType::EndpointError
            | FaultType::NanValue
            | FaultType::OverTemp
            | FaultType::OverCurrent
            | FaultType::EncoderInvalid
            | FaultType::DeviceTimeout
            | FaultType::UserEmergencyStop
            | FaultType::HardwareEmergencyStop => Duration::from_millis(50),
            FaultType::EndpointWedged => Duration::from_millis(100),
            FaultType::PluginOverrun => Duration::from_millis(100),
            // Device disconnect detection should be within 100ms
            FaultType::DeviceDisconnect => Duration::from_millis(100),
        }
    }

    /// Get detection threshold for this fault type
    pub fn detection_threshold(&self) -> FaultThreshold {
        match self {
            FaultType::UsbStall => FaultThreshold::FrameCount(3),
            FaultType::EndpointError => FaultThreshold::Immediate,
            FaultType::NanValue => FaultThreshold::Immediate,
            FaultType::OverTemp => FaultThreshold::Immediate,
            FaultType::OverCurrent => FaultThreshold::Immediate,
            FaultType::PluginOverrun => FaultThreshold::Duration(Duration::from_micros(100)),
            FaultType::EndpointWedged => FaultThreshold::Duration(Duration::from_millis(100)),
            FaultType::EncoderInvalid => FaultThreshold::Immediate,
            FaultType::DeviceTimeout => FaultThreshold::Duration(Duration::from_secs(1)),
            // Device disconnect is detected immediately from return codes
            FaultType::DeviceDisconnect => FaultThreshold::Immediate,
            // Emergency stops are immediate
            FaultType::UserEmergencyStop => FaultThreshold::Immediate,
            FaultType::HardwareEmergencyStop => FaultThreshold::Immediate,
        }
    }

    /// Check if this fault is a transient fault (can be cleared after recovery)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.10**
    pub fn is_transient(&self) -> bool {
        match self {
            FaultType::UsbStall
            | FaultType::EndpointError
            | FaultType::EndpointWedged
            | FaultType::DeviceTimeout
            | FaultType::DeviceDisconnect
            | FaultType::PluginOverrun
            | FaultType::NanValue
            | FaultType::UserEmergencyStop
            | FaultType::HardwareEmergencyStop => true,
            // Hardware-critical faults require power cycle
            FaultType::OverTemp | FaultType::OverCurrent | FaultType::EncoderInvalid => false,
        }
    }

    /// Check if this fault is hardware-critical (requires power cycle)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.9**
    pub fn is_hardware_critical(&self) -> bool {
        !self.is_transient()
    }
}

/// Fault detection thresholds
#[derive(Debug, Clone, PartialEq)]
pub enum FaultThreshold {
    /// Immediate detection (single occurrence)
    Immediate,
    /// Frame count threshold (e.g., 3 USB frames)
    FrameCount(u32),
    /// Duration threshold
    Duration(Duration),
}

/// Fault detection and response actions
#[derive(Debug, Clone, PartialEq)]
pub enum FaultAction {
    /// Ramp torque to zero within specified time
    TorqueZero50ms,
    /// Reset device connection
    DeviceReset,
    /// Quarantine component for session
    QuarantineComponent,
    /// Log and continue operation
    LogAndContinue,
}

/// Pre-fault capture data for diagnostics
#[derive(Debug, Clone)]
pub struct PreFaultCapture {
    /// Timestamp when capture started
    pub capture_start: Instant,
    /// Duration of pre-fault data captured
    pub capture_duration: Duration,
    /// Axis data samples before fault
    pub axis_samples: VecDeque<AxisSample>,
    /// FFB state samples before fault
    pub ffb_samples: VecDeque<FfbSample>,
    /// System events before fault
    pub system_events: VecDeque<SystemEvent>,
    /// Maximum samples to keep
    max_samples: usize,
    /// Time source for deterministic capture
    time_source: Arc<dyn TimeSource>,
}

/// Axis data sample for pre-fault capture
#[derive(Debug, Clone)]
pub struct AxisSample {
    pub timestamp: Instant,
    pub device_id: String,
    pub raw_input: f32,
    pub processed_output: f32,
    pub pipeline_stage: String,
}

/// FFB state sample for pre-fault capture
#[derive(Debug, Clone)]
pub struct FfbSample {
    pub timestamp: Instant,
    pub torque_setpoint: f32,
    pub actual_torque: f32,
    pub safety_state: String,
    pub device_health: Option<String>,
}

/// System event for pre-fault capture
#[derive(Debug, Clone)]
pub struct SystemEvent {
    pub timestamp: Instant,
    pub event_type: String,
    pub details: String,
    pub severity: EventSeverity,
}

/// Event severity levels
#[derive(Debug, Clone, PartialEq)]
pub enum EventSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Record of a detected fault
#[derive(Debug, Clone)]
pub struct FaultRecord {
    /// Type of fault detected
    pub fault_type: FaultType,
    /// When the fault was detected
    pub detected_at: Instant,
    /// Action taken in response
    pub action_taken: FaultAction,
    /// Time taken to respond to fault
    pub response_time: Option<Duration>,
    /// Additional context about the fault
    pub context: String,
    /// Whether fault caused safety state transition
    pub caused_safety_transition: bool,
    /// Pre-fault capture data (2s before fault)
    pub pre_fault_capture: Option<PreFaultCapture>,
    /// Stable error code for KB lookup
    pub error_code: String,
    /// KB article URL
    pub kb_article_url: String,
}

/// Soft-stop event record
#[derive(Debug, Clone)]
pub struct SoftStopRecord {
    /// When soft-stop was triggered
    pub triggered_at: Instant,
    /// Reason for soft-stop
    pub reason: String,
    /// Time to complete torque ramp to zero
    pub ramp_duration: Option<Duration>,
    /// Whether audio cue was triggered
    pub audio_cue_triggered: bool,
}

/// Fault detection and response system
#[derive(Debug)]
pub struct FaultDetector {
    /// Maximum response time for critical faults
    _max_response_time: Duration,
    /// History of detected faults
    fault_history: VecDeque<FaultRecord>,
    /// History of soft-stop events
    soft_stop_history: VecDeque<SoftStopRecord>,
    /// Maximum history size
    max_history_size: usize,
    /// Fault counters by type
    fault_counters: HashMap<FaultType, u32>,
    /// Last fault detection time
    last_fault_time: Option<Instant>,
    /// Pre-fault capture system
    pre_fault_capture: PreFaultCapture,
    /// USB frame stall counter
    usb_stall_counter: u32,
    /// Endpoint wedge detection timer
    endpoint_wedge_timer: Option<Instant>,
    /// Plugin overrun counters by plugin ID
    plugin_overrun_counters: HashMap<String, u32>,
    /// Time source for deterministic detection
    time_source: Arc<dyn TimeSource>,
}

impl PreFaultCapture {
    /// Create new pre-fault capture system
    pub fn new(capture_duration: Duration, time_source: Arc<dyn TimeSource>) -> Self {
        Self {
            capture_start: time_source.now(),
            capture_duration,
            axis_samples: VecDeque::new(),
            ffb_samples: VecDeque::new(),
            system_events: VecDeque::new(),
            max_samples: 1000, // Limit memory usage
            time_source,
        }
    }

    /// Add axis sample to pre-fault capture
    pub fn add_axis_sample(&mut self, sample: AxisSample) {
        self.axis_samples.push_back(sample);

        // Keep only samples within capture duration
        let cutoff = self.time_source.now() - self.capture_duration;
        while let Some(front) = self.axis_samples.front() {
            if front.timestamp < cutoff {
                self.axis_samples.pop_front();
            } else {
                break;
            }
        }

        // Enforce max samples limit
        if self.axis_samples.len() > self.max_samples {
            self.axis_samples.pop_front();
        }
    }

    /// Add FFB sample to pre-fault capture
    pub fn add_ffb_sample(&mut self, sample: FfbSample) {
        self.ffb_samples.push_back(sample);

        // Keep only samples within capture duration
        let cutoff = self.time_source.now() - self.capture_duration;
        while let Some(front) = self.ffb_samples.front() {
            if front.timestamp < cutoff {
                self.ffb_samples.pop_front();
            } else {
                break;
            }
        }

        // Enforce max samples limit
        if self.ffb_samples.len() > self.max_samples {
            self.ffb_samples.pop_front();
        }
    }

    /// Add system event to pre-fault capture
    pub fn add_system_event(&mut self, event: SystemEvent) {
        self.system_events.push_back(event);

        // Keep only events within capture duration
        let cutoff = self.time_source.now() - self.capture_duration;
        while let Some(front) = self.system_events.front() {
            if front.timestamp < cutoff {
                self.system_events.pop_front();
            } else {
                break;
            }
        }

        // Enforce max samples limit
        if self.system_events.len() > self.max_samples {
            self.system_events.pop_front();
        }
    }

    /// Get snapshot of current pre-fault data
    pub fn get_snapshot(&self) -> PreFaultCapture {
        self.clone()
    }

    /// Clear all captured data
    pub fn clear(&mut self) {
        self.axis_samples.clear();
        self.ffb_samples.clear();
        self.system_events.clear();
        self.capture_start = self.time_source.now();
    }
}

impl FaultDetector {
    /// Create new fault detector
    pub fn new(max_response_time: Duration, time_source: Arc<dyn TimeSource>) -> Self {
        Self {
            _max_response_time: max_response_time,
            fault_history: VecDeque::new(),
            soft_stop_history: VecDeque::new(),
            max_history_size: 1000,
            fault_counters: HashMap::new(),
            last_fault_time: None,
            pre_fault_capture: PreFaultCapture::new(Duration::from_secs(2), time_source.clone()),
            usb_stall_counter: 0,
            endpoint_wedge_timer: None,
            plugin_overrun_counters: HashMap::new(),
            time_source,
        }
    }

    /// Record a detected fault
    pub fn record_fault(&mut self, fault_type: FaultType) -> FaultRecord {
        let detected_at = self.time_source.now();
        self.last_fault_time = Some(detected_at);

        // Increment counter
        *self.fault_counters.entry(fault_type.clone()).or_insert(0) += 1;

        // Determine action based on fault type
        let action_taken = match fault_type {
            FaultType::UsbStall
            | FaultType::EndpointError
            | FaultType::NanValue
            | FaultType::OverTemp
            | FaultType::OverCurrent
            | FaultType::EndpointWedged
            | FaultType::EncoderInvalid
            | FaultType::DeviceTimeout
            | FaultType::DeviceDisconnect
            | FaultType::UserEmergencyStop
            | FaultType::HardwareEmergencyStop => FaultAction::TorqueZero50ms,
            FaultType::PluginOverrun => FaultAction::QuarantineComponent,
        };

        // Capture pre-fault data (2s before fault)
        let pre_fault_capture = if fault_type.requires_torque_cutoff() {
            Some(self.pre_fault_capture.get_snapshot())
        } else {
            None
        };

        let record = FaultRecord {
            fault_type: fault_type.clone(),
            detected_at,
            action_taken,
            response_time: None, // Will be filled in when response completes
            context: format!("Fault detected: {}", fault_type.description()),
            caused_safety_transition: fault_type.requires_torque_cutoff(),
            pre_fault_capture,
            error_code: fault_type.error_code().to_string(),
            kb_article_url: fault_type.kb_article_url().to_string(),
        };

        // Add to history
        self.fault_history.push_back(record.clone());

        // Keep history bounded
        if self.fault_history.len() > self.max_history_size {
            self.fault_history.pop_front();
        }

        record
    }

    /// Record completion of fault response
    pub fn record_fault_response_complete(
        &mut self,
        fault_type: FaultType,
        response_time: Duration,
    ) {
        // Find the most recent fault of this type and update response time
        if let Some(record) = self
            .fault_history
            .iter_mut()
            .rev()
            .find(|r| r.fault_type == fault_type && r.response_time.is_none())
        {
            record.response_time = Some(response_time);
        }
    }

    /// Record a soft-stop event
    pub fn record_soft_stop(&mut self, triggered_at: Instant) -> SoftStopRecord {
        let record = SoftStopRecord {
            triggered_at,
            reason: "Fault-triggered soft-stop".to_string(),
            ramp_duration: None, // Will be filled when ramp completes
            audio_cue_triggered: true,
        };

        self.soft_stop_history.push_back(record.clone());

        // Keep history bounded
        if self.soft_stop_history.len() > self.max_history_size {
            self.soft_stop_history.pop_front();
        }

        record
    }

    /// Record completion of soft-stop ramp
    pub fn record_soft_stop_complete(&mut self, ramp_duration: Duration) {
        if let Some(record) = self.soft_stop_history.back_mut() {
            if record.ramp_duration.is_none() {
                record.ramp_duration = Some(ramp_duration);
            }
        }
    }

    /// Get fault history
    pub fn get_fault_history(&self) -> &VecDeque<FaultRecord> {
        &self.fault_history
    }

    /// Get fault history as slice
    pub fn get_fault_history_slice(&self) -> Vec<&FaultRecord> {
        self.fault_history.iter().collect()
    }

    /// Get soft-stop history
    pub fn get_soft_stop_history(&self) -> &VecDeque<SoftStopRecord> {
        &self.soft_stop_history
    }

    /// Get fault counters
    pub fn get_fault_counters(&self) -> &std::collections::HashMap<FaultType, u32> {
        &self.fault_counters
    }

    /// Get recent faults (within specified duration)
    pub fn get_recent_faults(&self, within: Duration) -> Vec<&FaultRecord> {
        let cutoff = self.time_source.now() - within;
        self.fault_history
            .iter()
            .filter(|record| record.detected_at > cutoff)
            .collect()
    }

    /// Check if fault rate is excessive
    pub fn is_fault_rate_excessive(
        &self,
        fault_type: &FaultType,
        within: Duration,
        max_count: u32,
    ) -> bool {
        let recent_count = self
            .get_recent_faults(within)
            .iter()
            .filter(|record| &record.fault_type == fault_type)
            .count() as u32;

        recent_count > max_count
    }

    /// Clear all fault history (used after power cycle reset)
    pub fn clear_faults(&mut self) {
        self.fault_history.clear();
        self.soft_stop_history.clear();
        self.fault_counters.clear();
        self.last_fault_time = None;
        self.pre_fault_capture.clear();
        self.usb_stall_counter = 0;
        self.endpoint_wedge_timer = None;
        self.plugin_overrun_counters.clear();
    }

    /// Record USB frame stall
    pub fn record_usb_stall(&mut self) -> Option<FaultRecord> {
        self.usb_stall_counter += 1;

        // Trigger fault after 3 stalls
        if self.usb_stall_counter >= 3 {
            self.usb_stall_counter = 0; // Reset counter
            Some(self.record_fault(FaultType::UsbStall))
        } else {
            None
        }
    }

    /// Reset USB stall counter (called on successful frame)
    pub fn reset_usb_stall_counter(&mut self) {
        self.usb_stall_counter = 0;
    }

    /// Check for endpoint wedge condition
    pub fn check_endpoint_wedge(&mut self, endpoint_responsive: bool) -> Option<FaultRecord> {
        if !endpoint_responsive {
            if self.endpoint_wedge_timer.is_none() {
                self.endpoint_wedge_timer = Some(self.time_source.now());
            } else if let Some(timer) = self.endpoint_wedge_timer {
                if self.time_source.now().duration_since(timer) >= Duration::from_millis(100) {
                    self.endpoint_wedge_timer = None;
                    return Some(self.record_fault(FaultType::EndpointWedged));
                }
            }
        } else {
            self.endpoint_wedge_timer = None;
        }
        None
    }

    /// Record plugin overrun
    pub fn record_plugin_overrun(
        &mut self,
        plugin_id: String,
        execution_time: Duration,
    ) -> Option<FaultRecord> {
        if execution_time > Duration::from_micros(100) {
            *self
                .plugin_overrun_counters
                .entry(plugin_id.clone())
                .or_insert(0) += 1;

            // Add system event to pre-fault capture
            self.pre_fault_capture.add_system_event(SystemEvent {
                timestamp: self.time_source.now(),
                event_type: "PLUGIN_OVERRUN".to_string(),
                details: format!(
                    "Plugin {} exceeded 100μs budget: {:?}",
                    plugin_id, execution_time
                ),
                severity: EventSeverity::Warning,
            });

            Some(self.record_fault(FaultType::PluginOverrun))
        } else {
            None
        }
    }

    /// Add axis sample to pre-fault capture
    pub fn add_axis_sample(
        &mut self,
        device_id: String,
        raw_input: f32,
        processed_output: f32,
        pipeline_stage: String,
    ) {
        let sample = AxisSample {
            timestamp: self.time_source.now(),
            device_id,
            raw_input,
            processed_output,
            pipeline_stage,
        };
        self.pre_fault_capture.add_axis_sample(sample);
    }

    /// Add FFB sample to pre-fault capture
    pub fn add_ffb_sample(
        &mut self,
        torque_setpoint: f32,
        actual_torque: f32,
        safety_state: String,
        device_health: Option<String>,
    ) {
        let sample = FfbSample {
            timestamp: self.time_source.now(),
            torque_setpoint,
            actual_torque,
            safety_state,
            device_health,
        };
        self.pre_fault_capture.add_ffb_sample(sample);
    }

    /// Add system event to pre-fault capture
    pub fn add_system_event(
        &mut self,
        event_type: String,
        details: String,
        severity: EventSeverity,
    ) {
        let event = SystemEvent {
            timestamp: self.time_source.now(),
            event_type,
            details,
            severity,
        };
        self.pre_fault_capture.add_system_event(event);
    }

    /// Check for NaN values in axis data
    pub fn check_nan_value(&mut self, value: f32, context: &str) -> Option<FaultRecord> {
        if value.is_nan() || value.is_infinite() {
            self.add_system_event(
                "NAN_DETECTED".to_string(),
                format!("NaN/Infinite value detected in {}: {}", context, value),
                EventSeverity::Critical,
            );
            Some(self.record_fault(FaultType::NanValue))
        } else {
            None
        }
    }

    /// Get plugin overrun counters
    pub fn get_plugin_overrun_counters(&self) -> &HashMap<String, u32> {
        &self.plugin_overrun_counters
    }

    /// Get time since last fault
    pub fn time_since_last_fault(&self) -> Option<Duration> {
        self.last_fault_time.map(|t| t.elapsed())
    }

    /// Check if system is in fault storm (too many faults recently)
    pub fn is_in_fault_storm(&self) -> bool {
        let recent_faults = self.get_recent_faults(Duration::from_secs(60));
        recent_faults.len() > 10 // More than 10 faults in last minute
    }

    /// Get fault statistics
    pub fn get_fault_statistics(&self) -> FaultStatistics {
        let total_faults = self.fault_counters.values().sum();
        let unique_fault_types = self.fault_counters.len();

        let avg_response_time = if !self.fault_history.is_empty() {
            let total_response_time: Duration = self
                .fault_history
                .iter()
                .filter_map(|r| r.response_time)
                .sum();
            let count = self
                .fault_history
                .iter()
                .filter(|r| r.response_time.is_some())
                .count();

            if count > 0 {
                Some(total_response_time / count as u32)
            } else {
                None
            }
        } else {
            None
        };

        FaultStatistics {
            total_faults,
            unique_fault_types,
            avg_response_time,
            max_response_time: self
                .fault_history
                .iter()
                .filter_map(|r| r.response_time)
                .max(),
            fault_storm_detected: self.is_in_fault_storm(),
        }
    }

    // =========================================================================
    // Device Disconnect Detection
    // **Validates: Requirement FFB-SAFETY-01.8**
    // =========================================================================

    /// Record device disconnect from HID/DirectInput return codes
    ///
    /// This method should be called when HID or DirectInput operations return
    /// error codes indicating device disconnection. Common disconnect indicators:
    ///
    /// **HID (Windows):**
    /// - `ERROR_DEVICE_NOT_CONNECTED` (0x48F / 1167)
    /// - `ERROR_GEN_FAILURE` (0x1F / 31) - often indicates disconnect
    /// - `ERROR_NO_SUCH_DEVICE` (0x2 / 2)
    ///
    /// **DirectInput:**
    /// - `DIERR_INPUTLOST` (0x8007001E) - device lost
    /// - `DIERR_NOTACQUIRED` (0x8007000C) - device not acquired (may indicate disconnect)
    /// - `DIERR_UNPLUGGED` (0x80040209) - device unplugged
    ///
    /// **Validates: Requirement FFB-SAFETY-01.8**
    /// Device disconnect detected within 100ms, outputs ramped to safe within 50ms
    ///
    /// # Arguments
    /// * `error_code` - The error code from HID/DirectInput operation
    /// * `context` - Description of the operation that failed
    ///
    /// # Returns
    /// * `Some(FaultRecord)` if disconnect was detected
    /// * `None` if error code doesn't indicate disconnect
    pub fn check_disconnect_from_error_code(
        &mut self,
        error_code: i32,
        context: &str,
    ) -> Option<FaultRecord> {
        // Check for HID disconnect error codes (Windows)
        let is_disconnect = matches!(
            error_code,
            // ERROR_DEVICE_NOT_CONNECTED
            1167 |
            // ERROR_NO_SUCH_DEVICE / ERROR_FILE_NOT_FOUND
            2 |
            // ERROR_GEN_FAILURE (often indicates disconnect)
            31 |
            // DIERR_INPUTLOST (0x8007001E as i32)
            -2147024866 |
            // DIERR_UNPLUGGED (0x80040209 as i32)
            -2147220983 |
            // DIERR_NOTACQUIRED (0x8007000C as i32)
            -2147024884
        );

        if is_disconnect {
            self.add_system_event(
                "DEVICE_DISCONNECT".to_string(),
                format!(
                    "Device disconnect detected from error code {} in {}",
                    error_code, context
                ),
                EventSeverity::Critical,
            );
            Some(self.record_fault(FaultType::DeviceDisconnect))
        } else {
            None
        }
    }

    /// Record device disconnect directly
    ///
    /// This method should be called when device disconnection is detected
    /// through other means (e.g., device enumeration, hotplug events).
    ///
    /// **Validates: Requirement FFB-SAFETY-01.8**
    ///
    /// # Arguments
    /// * `device_id` - Identifier of the disconnected device
    /// * `context` - Additional context about the disconnect
    ///
    /// # Returns
    /// * `FaultRecord` for the disconnect fault
    pub fn record_device_disconnect(&mut self, device_id: &str, context: &str) -> FaultRecord {
        self.add_system_event(
            "DEVICE_DISCONNECT".to_string(),
            format!("Device {} disconnected: {}", device_id, context),
            EventSeverity::Critical,
        );
        self.record_fault(FaultType::DeviceDisconnect)
    }

    /// Check if an HRESULT indicates device disconnection
    ///
    /// This is a helper for DirectInput error handling.
    ///
    /// # Arguments
    /// * `hresult` - The HRESULT from a DirectInput operation
    ///
    /// # Returns
    /// * `true` if the HRESULT indicates device disconnection
    pub fn is_disconnect_hresult(hresult: i32) -> bool {
        matches!(
            hresult,
            // DIERR_INPUTLOST
            -2147024866 |
            // DIERR_UNPLUGGED
            -2147220983 |
            // DIERR_NOTACQUIRED (may indicate disconnect)
            -2147024884 |
            // E_HANDLE (invalid handle, device may be gone)
            -2147024890
        )
    }

    /// Check if a Windows error code indicates device disconnection
    ///
    /// This is a helper for HID error handling.
    ///
    /// # Arguments
    /// * `error_code` - The Windows error code
    ///
    /// # Returns
    /// * `true` if the error code indicates device disconnection
    pub fn is_disconnect_win32_error(error_code: u32) -> bool {
        matches!(
            error_code,
            // ERROR_DEVICE_NOT_CONNECTED
            1167 |
            // ERROR_NO_SUCH_DEVICE / ERROR_FILE_NOT_FOUND
            2 |
            // ERROR_GEN_FAILURE
            31 |
            // ERROR_BAD_COMMAND
            22 |
            // ERROR_NOT_READY
            21
        )
    }
}

/// Fault statistics summary
#[derive(Debug, Clone)]
pub struct FaultStatistics {
    pub total_faults: u32,
    pub unique_fault_types: usize,
    pub avg_response_time: Option<Duration>,
    pub max_response_time: Option<Duration>,
    pub fault_storm_detected: bool,
}

impl Default for FaultDetector {
    fn default() -> Self {
        Self::new(Duration::from_millis(50), Arc::new(crate::DefaultTimeSource))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fault_type_properties() {
        assert_eq!(FaultType::UsbStall.error_code(), "HID_OUT_STALL");
        assert!(FaultType::UsbStall.requires_torque_cutoff());
        assert_eq!(
            FaultType::UsbStall.max_response_time(),
            Duration::from_millis(50)
        );
        assert_eq!(
            FaultType::UsbStall.kb_article_url(),
            "https://docs.flight-hub.dev/kb/hid-out-stall"
        );

        assert_eq!(FaultType::PluginOverrun.error_code(), "PLUG_OVERRUN");
        assert!(!FaultType::PluginOverrun.requires_torque_cutoff());
        assert_eq!(
            FaultType::PluginOverrun.max_response_time(),
            Duration::from_millis(100)
        );
        assert_eq!(
            FaultType::PluginOverrun.kb_article_url(),
            "https://docs.flight-hub.dev/kb/plug-overrun"
        );

        // Test new fault types
        assert_eq!(
            FaultType::EndpointWedged.error_code(),
            "HID_ENDPOINT_WEDGED"
        );
        assert!(FaultType::EndpointWedged.requires_torque_cutoff());
        assert_eq!(
            FaultType::EndpointWedged.max_response_time(),
            Duration::from_millis(100)
        );

        assert_eq!(FaultType::EncoderInvalid.error_code(), "ENCODER_INVALID");
        assert!(FaultType::EncoderInvalid.requires_torque_cutoff());
    }

    #[test]
    fn test_fault_recording() {
        let time_source = Arc::new(crate::DefaultTimeSource);
        let mut detector = FaultDetector::new(Duration::from_millis(50), time_source);

        let record = detector.record_fault(FaultType::UsbStall);

        assert_eq!(record.fault_type, FaultType::UsbStall);
        assert_eq!(record.action_taken, FaultAction::TorqueZero50ms);
        assert!(record.caused_safety_transition);
        assert_eq!(record.error_code, "HID_OUT_STALL");
        assert_eq!(
            record.kb_article_url,
            "https://docs.flight-hub.dev/kb/hid-out-stall"
        );
        assert!(record.pre_fault_capture.is_some()); // Should have pre-fault capture for torque cutoff faults

        assert_eq!(detector.get_fault_history().len(), 1);
        assert_eq!(detector.get_fault_counters()[&FaultType::UsbStall], 1);
    }

    #[test]
    fn test_fault_response_completion() {
        let time_source = Arc::new(crate::DefaultTimeSource);
        let mut detector = FaultDetector::new(Duration::from_millis(50), time_source);

        detector.record_fault(FaultType::UsbStall);
        detector.record_fault_response_complete(FaultType::UsbStall, Duration::from_millis(30));

        let record = &detector.get_fault_history()[0];
        assert_eq!(record.response_time, Some(Duration::from_millis(30)));
    }

    #[test]
    fn test_soft_stop_recording() {
        let time_source = Arc::new(crate::DefaultTimeSource);
        let mut detector = FaultDetector::new(Duration::from_millis(50), time_source.clone());

        let triggered_at = time_source.now();
        let record = detector.record_soft_stop(triggered_at);

        assert_eq!(record.triggered_at, triggered_at);
        assert!(record.audio_cue_triggered);
        assert_eq!(detector.get_soft_stop_history().len(), 1);

        detector.record_soft_stop_complete(Duration::from_millis(45));

        let updated_record = &detector.get_soft_stop_history()[0];
        assert_eq!(
            updated_record.ramp_duration,
            Some(Duration::from_millis(45))
        );
    }

    #[test]
    fn test_recent_faults() {
        let time_source = Arc::new(crate::FakeTimeSource::new());
        let mut detector = FaultDetector::new(Duration::from_millis(50), time_source.clone());

        detector.record_fault(FaultType::UsbStall);
        time_source.advance(Duration::from_millis(10));
        detector.record_fault(FaultType::OverTemp);

        let recent = detector.get_recent_faults(Duration::from_millis(100));
        assert_eq!(recent.len(), 2);

        let very_recent = detector.get_recent_faults(Duration::from_millis(5));
        assert_eq!(very_recent.len(), 1);
        assert_eq!(very_recent[0].fault_type, FaultType::OverTemp);
    }

    #[test]
    fn test_fault_rate_detection() {
        let time_source = Arc::new(crate::DefaultTimeSource);
        let mut detector = FaultDetector::new(Duration::from_millis(50), time_source);

        // Record multiple faults of same type
        for _ in 0..5 {
            detector.record_fault(FaultType::UsbStall);
        }

        assert!(detector.is_fault_rate_excessive(&FaultType::UsbStall, Duration::from_secs(60), 3));

        assert!(!detector.is_fault_rate_excessive(
            &FaultType::OverTemp,
            Duration::from_secs(60),
            3
        ));
    }

    #[test]
    fn test_fault_storm_detection() {
        let time_source = Arc::new(crate::DefaultTimeSource);
        let mut detector = FaultDetector::new(Duration::from_millis(50), time_source);

        // Record many faults to trigger storm detection
        for i in 0..15 {
            let fault_type = if i % 2 == 0 {
                FaultType::UsbStall
            } else {
                FaultType::OverTemp
            };
            detector.record_fault(fault_type);
        }

        assert!(detector.is_in_fault_storm());
    }

    #[test]
    fn test_fault_statistics() {
        let time_source = Arc::new(crate::DefaultTimeSource);
        let mut detector = FaultDetector::new(Duration::from_millis(50), time_source);

        detector.record_fault(FaultType::UsbStall);
        detector.record_fault_response_complete(FaultType::UsbStall, Duration::from_millis(30));

        detector.record_fault(FaultType::OverTemp);
        detector.record_fault_response_complete(FaultType::OverTemp, Duration::from_millis(40));

        let stats = detector.get_fault_statistics();

        assert_eq!(stats.total_faults, 2);
        assert_eq!(stats.unique_fault_types, 2);
        assert_eq!(stats.avg_response_time, Some(Duration::from_millis(35)));
        assert_eq!(stats.max_response_time, Some(Duration::from_millis(40)));
        assert!(!stats.fault_storm_detected);
    }

    #[test]
    fn test_clear_faults() {
        let time_source = Arc::new(crate::DefaultTimeSource);
        let mut detector = FaultDetector::new(Duration::from_millis(50), time_source.clone());

        detector.record_fault(FaultType::UsbStall);
        detector.record_soft_stop(time_source.now());

        assert!(!detector.get_fault_history().is_empty());
        assert!(!detector.get_soft_stop_history().is_empty());

        detector.clear_faults();

        assert!(detector.get_fault_history().is_empty());
        assert!(detector.get_soft_stop_history().is_empty());
        assert!(detector.get_fault_counters().is_empty());
        assert!(detector.time_since_last_fault().is_none());
    }
}
