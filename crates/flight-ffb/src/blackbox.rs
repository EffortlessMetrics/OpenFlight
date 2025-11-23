// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Blackbox recording system for fault analysis and diagnostics
//!
//! Provides continuous circular buffer recording with 2s pre-fault capture
//! capability for comprehensive fault analysis.

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Blackbox data entry types
#[derive(Debug, Clone)]
pub enum BlackboxEntry {
    /// Axis frame data
    AxisFrame {
        timestamp: Instant,
        device_id: String,
        raw_input: f32,
        processed_output: f32,
        torque_nm: f32,
    },
    /// FFB state change
    FfbState {
        timestamp: Instant,
        safety_state: String,
        torque_setpoint: f32,
        actual_torque: f32,
    },
    /// Fault event
    Fault {
        timestamp: Instant,
        fault_type: String,
        fault_code: String,
        context: String,
    },
    /// Soft-stop event
    SoftStop {
        timestamp: Instant,
        reason: String,
        initial_torque: f32,
        target_ramp_time: Duration,
    },
    /// System event
    SystemEvent {
        timestamp: Instant,
        event_type: String,
        details: String,
    },
    /// Telemetry synthesis output
    TelemetrySynth {
        timestamp: Instant,
        torque_nm: f32,
        frequency_hz: f32,
        intensity: f32,
        active_effects: String,
    },
}

impl BlackboxEntry {
    /// Get timestamp of this entry
    pub fn timestamp(&self) -> Instant {
        match self {
            BlackboxEntry::AxisFrame { timestamp, .. }
            | BlackboxEntry::FfbState { timestamp, .. }
            | BlackboxEntry::Fault { timestamp, .. }
            | BlackboxEntry::SoftStop { timestamp, .. }
            | BlackboxEntry::SystemEvent { timestamp, .. }
            | BlackboxEntry::TelemetrySynth { timestamp, .. } => *timestamp,
        }
    }

    /// Get entry type as string
    pub fn entry_type(&self) -> &'static str {
        match self {
            BlackboxEntry::AxisFrame { .. } => "AxisFrame",
            BlackboxEntry::FfbState { .. } => "FfbState",
            BlackboxEntry::Fault { .. } => "Fault",
            BlackboxEntry::SoftStop { .. } => "SoftStop",
            BlackboxEntry::SystemEvent { .. } => "SystemEvent",
            BlackboxEntry::TelemetrySynth { .. } => "TelemetrySynth",
        }
    }

    /// Serialize entry for storage (simplified format)
    pub fn serialize(&self) -> String {
        match self {
            BlackboxEntry::AxisFrame {
                timestamp,
                device_id,
                raw_input,
                processed_output,
                torque_nm,
            } => {
                format!(
                    "AXIS,{:?},{},{},{},{}",
                    timestamp.elapsed(),
                    device_id,
                    raw_input,
                    processed_output,
                    torque_nm
                )
            }
            BlackboxEntry::FfbState {
                timestamp,
                safety_state,
                torque_setpoint,
                actual_torque,
            } => {
                format!(
                    "FFB,{:?},{},{},{}",
                    timestamp.elapsed(),
                    safety_state,
                    torque_setpoint,
                    actual_torque
                )
            }
            BlackboxEntry::Fault {
                timestamp,
                fault_type,
                fault_code,
                context,
            } => {
                format!(
                    "FAULT,{:?},{},{},{}",
                    timestamp.elapsed(),
                    fault_type,
                    fault_code,
                    context
                )
            }
            BlackboxEntry::SoftStop {
                timestamp,
                reason,
                initial_torque,
                target_ramp_time,
            } => {
                format!(
                    "SOFTSTOP,{:?},{},{},{:?}",
                    timestamp.elapsed(),
                    reason,
                    initial_torque,
                    target_ramp_time
                )
            }
            BlackboxEntry::SystemEvent {
                timestamp,
                event_type,
                details,
            } => {
                format!(
                    "SYSTEM,{:?},{},{}",
                    timestamp.elapsed(),
                    event_type,
                    details
                )
            }
            BlackboxEntry::TelemetrySynth {
                timestamp,
                torque_nm,
                frequency_hz,
                intensity,
                active_effects,
            } => {
                format!(
                    "TELEMETRY,{:?},{},{},{},{}",
                    timestamp.elapsed(),
                    torque_nm,
                    frequency_hz,
                    intensity,
                    active_effects
                )
            }
        }
    }
}

/// Blackbox configuration
#[derive(Debug, Clone)]
pub struct BlackboxConfig {
    /// Pre-fault capture duration
    pub pre_fault_duration: Duration,
    /// Post-fault capture duration
    pub post_fault_duration: Duration,
    /// Maximum entries in circular buffer
    pub max_entries: usize,
    /// Whether to auto-save on fault
    pub auto_save_on_fault: bool,
    /// Directory for saved captures
    pub save_directory: Option<String>,
}

impl Default for BlackboxConfig {
    fn default() -> Self {
        Self {
            pre_fault_duration: Duration::from_secs(2),
            post_fault_duration: Duration::from_secs(5),
            max_entries: 10000, // ~2-5 seconds at 250Hz + other events
            auto_save_on_fault: true,
            save_directory: None,
        }
    }
}

/// Fault capture state
#[derive(Debug, Clone)]
pub struct FaultCapture {
    /// When the fault occurred
    pub fault_time: Instant,
    /// Fault details
    pub fault_entry: BlackboxEntry,
    /// Pre-fault entries
    pub pre_fault_entries: Vec<BlackboxEntry>,
    /// Post-fault entries (collected after fault)
    pub post_fault_entries: Vec<BlackboxEntry>,
    /// Whether capture is complete
    pub complete: bool,
}

/// Blackbox recorder with circular buffer and fault capture
#[derive(Debug)]
pub struct BlackboxRecorder {
    config: BlackboxConfig,
    /// Circular buffer for continuous recording
    circular_buffer: VecDeque<BlackboxEntry>,
    /// Active fault capture (if any)
    active_capture: Option<FaultCapture>,
    /// Completed fault captures
    completed_captures: Vec<FaultCapture>,
    /// Maximum completed captures to keep
    max_completed_captures: usize,
    /// Recording start time
    recording_start: Instant,
}

/// Blackbox errors
#[derive(Debug, Error)]
pub enum BlackboxError {
    #[error("Buffer overflow: too many entries")]
    BufferOverflow,
    #[error("No active fault capture")]
    NoActiveFaultCapture,
    #[error("Fault capture already active")]
    FaultCaptureAlreadyActive,
    #[error("Save failed: {message}")]
    SaveFailed { message: String },
    #[error("Invalid configuration: {message}")]
    InvalidConfig { message: String },
}

pub type BlackboxResult<T> = std::result::Result<T, BlackboxError>;

impl BlackboxRecorder {
    /// Create new blackbox recorder
    pub fn new(config: BlackboxConfig) -> BlackboxResult<Self> {
        if config.max_entries == 0 {
            return Err(BlackboxError::InvalidConfig {
                message: "max_entries must be > 0".to_string(),
            });
        }

        let max_entries = config.max_entries;
        Ok(Self {
            config,
            circular_buffer: VecDeque::with_capacity(max_entries),
            active_capture: None,
            completed_captures: Vec::new(),
            max_completed_captures: 10,
            recording_start: Instant::now(),
        })
    }

    /// Record an entry to the blackbox
    pub fn record(&mut self, entry: BlackboxEntry) -> BlackboxResult<()> {
        // Add to circular buffer
        if self.circular_buffer.len() >= self.config.max_entries {
            self.circular_buffer.pop_front();
        }
        self.circular_buffer.push_back(entry.clone());

        // If we have an active fault capture, add to post-fault entries
        if let Some(capture) = &mut self.active_capture {
            if !capture.complete {
                let entry_timestamp = entry.timestamp();
                capture.post_fault_entries.push(entry);

                // Check if post-fault capture is complete
                let post_fault_duration = entry_timestamp.duration_since(capture.fault_time);
                if post_fault_duration >= self.config.post_fault_duration {
                    capture.complete = true;

                    // Move to completed captures
                    let completed_capture = self.active_capture.take().unwrap();
                    self.completed_captures.push(completed_capture);

                    // Keep only recent completed captures
                    if self.completed_captures.len() > self.max_completed_captures {
                        self.completed_captures.remove(0);
                    }
                }
            }
        }

        Ok(())
    }

    /// Start fault capture (triggered when fault is detected)
    pub fn start_fault_capture(&mut self, fault_entry: BlackboxEntry) -> BlackboxResult<()> {
        // If there's already an active capture, complete it first
        if let Some(active_capture) = self.active_capture.take() {
            self.completed_captures.push(active_capture);
            if self.completed_captures.len() > self.max_completed_captures {
                self.completed_captures.remove(0);
            }
        }

        let fault_time = fault_entry.timestamp();

        // Extract pre-fault entries from circular buffer
        let cutoff_time = fault_time - self.config.pre_fault_duration;
        let pre_fault_entries: Vec<BlackboxEntry> = self
            .circular_buffer
            .iter()
            .filter(|entry| entry.timestamp() >= cutoff_time && entry.timestamp() < fault_time)
            .cloned()
            .collect();

        let capture = FaultCapture {
            fault_time,
            fault_entry,
            pre_fault_entries,
            post_fault_entries: Vec::new(),
            complete: false,
        };

        self.active_capture = Some(capture);

        Ok(())
    }

    /// Get current active fault capture
    pub fn get_active_capture(&self) -> Option<&FaultCapture> {
        self.active_capture.as_ref()
    }

    /// Get completed fault captures
    pub fn get_completed_captures(&self) -> &[FaultCapture] {
        &self.completed_captures
    }

    /// Get recent entries from circular buffer
    pub fn get_recent_entries(&self, duration: Duration) -> Vec<&BlackboxEntry> {
        let cutoff_time = Instant::now() - duration;
        self.circular_buffer
            .iter()
            .filter(|entry| entry.timestamp() >= cutoff_time)
            .collect()
    }

    /// Get all entries in circular buffer
    pub fn get_all_entries(&self) -> &VecDeque<BlackboxEntry> {
        &self.circular_buffer
    }

    /// Save fault capture to file (simplified implementation)
    pub fn save_fault_capture(&self, capture: &FaultCapture) -> BlackboxResult<String> {
        let filename = format!(
            "fault_capture_{}.txt",
            capture.fault_time.elapsed().as_millis()
        );

        // In a real implementation, this would write to actual file
        let mut content = String::new();
        content.push_str(&format!("Fault Capture Report\n"));
        content.push_str(&format!("Fault Time: {:?}\n", capture.fault_time.elapsed()));
        content.push_str(&format!(
            "Fault Entry: {}\n",
            capture.fault_entry.serialize()
        ));
        content.push_str(&format!(
            "\nPre-fault entries ({}):\n",
            capture.pre_fault_entries.len()
        ));

        for entry in &capture.pre_fault_entries {
            content.push_str(&format!("{}\n", entry.serialize()));
        }

        content.push_str(&format!(
            "\nPost-fault entries ({}):\n",
            capture.post_fault_entries.len()
        ));
        for entry in &capture.post_fault_entries {
            content.push_str(&format!("{}\n", entry.serialize()));
        }

        // TODO: Actually write to file system
        tracing::info!("Saved fault capture to {}", filename);
        tracing::debug!("Capture content:\n{}", content);

        Ok(filename)
    }

    /// Save all completed captures
    pub fn save_all_captures(&self) -> BlackboxResult<Vec<String>> {
        let mut saved_files = Vec::new();

        for capture in &self.completed_captures {
            let filename = self.save_fault_capture(capture)?;
            saved_files.push(filename);
        }

        Ok(saved_files)
    }

    /// Clear circular buffer
    pub fn clear_buffer(&mut self) {
        self.circular_buffer.clear();
    }

    /// Clear completed captures
    pub fn clear_completed_captures(&mut self) {
        self.completed_captures.clear();
    }

    /// Get buffer statistics
    pub fn get_statistics(&self) -> BlackboxStatistics {
        let total_entries = self.circular_buffer.len();
        let buffer_utilization = total_entries as f32 / self.config.max_entries as f32;

        let oldest_entry_age = self
            .circular_buffer
            .front()
            .map(|entry| entry.timestamp().elapsed());

        let newest_entry_age = self
            .circular_buffer
            .back()
            .map(|entry| entry.timestamp().elapsed());

        BlackboxStatistics {
            total_entries,
            buffer_utilization,
            oldest_entry_age,
            newest_entry_age,
            active_fault_capture: self.active_capture.is_some(),
            completed_captures: self.completed_captures.len(),
            recording_duration: self.recording_start.elapsed(),
        }
    }

    /// Update configuration
    pub fn update_config(&mut self, config: BlackboxConfig) -> BlackboxResult<()> {
        if config.max_entries == 0 {
            return Err(BlackboxError::InvalidConfig {
                message: "max_entries must be > 0".to_string(),
            });
        }

        // Resize buffer if needed
        if config.max_entries != self.config.max_entries {
            self.circular_buffer.reserve(config.max_entries);
            while self.circular_buffer.len() > config.max_entries {
                self.circular_buffer.pop_front();
            }
        }

        self.config = config;
        Ok(())
    }

    /// Get current configuration
    pub fn get_config(&self) -> &BlackboxConfig {
        &self.config
    }
}

/// Blackbox statistics
#[derive(Debug, Clone)]
pub struct BlackboxStatistics {
    pub total_entries: usize,
    pub buffer_utilization: f32,
    pub oldest_entry_age: Option<Duration>,
    pub newest_entry_age: Option<Duration>,
    pub active_fault_capture: bool,
    pub completed_captures: usize,
    pub recording_duration: Duration,
}

impl Default for BlackboxRecorder {
    fn default() -> Self {
        Self::new(BlackboxConfig::default()).expect("Default config should be valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn create_test_axis_entry(timestamp: Instant, torque: f32) -> BlackboxEntry {
        BlackboxEntry::AxisFrame {
            timestamp,
            device_id: "test_device".to_string(),
            raw_input: 0.5,
            processed_output: 0.6,
            torque_nm: torque,
        }
    }

    fn create_test_fault_entry(timestamp: Instant) -> BlackboxEntry {
        BlackboxEntry::Fault {
            timestamp,
            fault_type: "USB_STALL".to_string(),
            fault_code: "HID_OUT_STALL".to_string(),
            context: "Test fault".to_string(),
        }
    }

    #[test]
    fn test_circular_buffer() {
        let config = BlackboxConfig {
            max_entries: 3,
            ..Default::default()
        };

        let mut recorder = BlackboxRecorder::new(config).unwrap();

        // Add entries up to capacity
        let now = Instant::now();
        recorder.record(create_test_axis_entry(now, 1.0)).unwrap();
        recorder.record(create_test_axis_entry(now, 2.0)).unwrap();
        recorder.record(create_test_axis_entry(now, 3.0)).unwrap();

        assert_eq!(recorder.get_all_entries().len(), 3);

        // Add one more - should evict oldest
        recorder.record(create_test_axis_entry(now, 4.0)).unwrap();
        assert_eq!(recorder.get_all_entries().len(), 3);

        // First entry should be gone, last should be 4.0
        if let BlackboxEntry::AxisFrame { torque_nm, .. } =
            recorder.get_all_entries().back().unwrap()
        {
            assert_eq!(*torque_nm, 4.0);
        } else {
            panic!("Expected AxisFrame entry");
        }
    }

    #[test]
    fn test_fault_capture() {
        let config = BlackboxConfig {
            pre_fault_duration: Duration::from_millis(100),
            post_fault_duration: Duration::from_millis(100),
            max_entries: 100,
            ..Default::default()
        };

        let mut recorder = BlackboxRecorder::new(config).unwrap();

        let start_time = Instant::now();

        // Record some pre-fault entries
        recorder
            .record(create_test_axis_entry(start_time, 1.0))
            .unwrap();
        thread::sleep(Duration::from_millis(50));
        recorder
            .record(create_test_axis_entry(
                start_time + Duration::from_millis(50),
                2.0,
            ))
            .unwrap();

        // Record fault
        let fault_time = start_time + Duration::from_millis(100);
        let fault_entry = create_test_fault_entry(fault_time);
        recorder.start_fault_capture(fault_entry).unwrap();

        // Should have active capture
        assert!(recorder.get_active_capture().is_some());

        // Record some post-fault entries
        recorder
            .record(create_test_axis_entry(
                fault_time + Duration::from_millis(50),
                3.0,
            ))
            .unwrap();
        recorder
            .record(create_test_axis_entry(
                fault_time + Duration::from_millis(150),
                4.0,
            ))
            .unwrap();

        // Capture should be complete now
        assert!(recorder.get_completed_captures().len() == 1);
        assert!(recorder.get_active_capture().is_none());

        let capture = &recorder.get_completed_captures()[0];
        assert!(capture.complete);
        assert_eq!(capture.pre_fault_entries.len(), 2);
        assert_eq!(capture.post_fault_entries.len(), 2);
    }

    #[test]
    fn test_pre_fault_filtering() {
        let config = BlackboxConfig {
            pre_fault_duration: Duration::from_millis(50),
            max_entries: 100,
            ..Default::default()
        };

        let mut recorder = BlackboxRecorder::new(config).unwrap();

        let start_time = Instant::now();

        // Record entries at different times
        recorder
            .record(create_test_axis_entry(start_time, 1.0))
            .unwrap(); // Too old
        recorder
            .record(create_test_axis_entry(
                start_time + Duration::from_millis(60),
                2.0,
            ))
            .unwrap(); // Should be included
        recorder
            .record(create_test_axis_entry(
                start_time + Duration::from_millis(80),
                3.0,
            ))
            .unwrap(); // Should be included

        // Fault at 100ms
        let fault_time = start_time + Duration::from_millis(100);
        recorder
            .start_fault_capture(create_test_fault_entry(fault_time))
            .unwrap();

        let capture = recorder.get_active_capture().unwrap();

        // Should only have entries from last 50ms (60ms and 80ms entries)
        assert_eq!(capture.pre_fault_entries.len(), 2);

        if let BlackboxEntry::AxisFrame { torque_nm, .. } = &capture.pre_fault_entries[0] {
            assert_eq!(*torque_nm, 2.0);
        }

        if let BlackboxEntry::AxisFrame { torque_nm, .. } = &capture.pre_fault_entries[1] {
            assert_eq!(*torque_nm, 3.0);
        }
    }

    #[test]
    fn test_entry_serialization() {
        let now = Instant::now();
        let entry = BlackboxEntry::AxisFrame {
            timestamp: now,
            device_id: "test".to_string(),
            raw_input: 0.5,
            processed_output: 0.6,
            torque_nm: 1.5,
        };

        let serialized = entry.serialize();
        assert!(serialized.contains("AXIS"));
        assert!(serialized.contains("test"));
        assert!(serialized.contains("0.5"));
        assert!(serialized.contains("1.5"));
    }

    #[test]
    fn test_recent_entries() {
        let mut recorder = BlackboxRecorder::default();

        let now = Instant::now();
        recorder
            .record(create_test_axis_entry(
                now - Duration::from_millis(200),
                1.0,
            ))
            .unwrap();
        recorder
            .record(create_test_axis_entry(now - Duration::from_millis(50), 2.0))
            .unwrap();
        recorder.record(create_test_axis_entry(now, 3.0)).unwrap();

        let recent = recorder.get_recent_entries(Duration::from_millis(100));
        assert_eq!(recent.len(), 2); // Last two entries
    }

    #[test]
    fn test_statistics() {
        let config = BlackboxConfig {
            max_entries: 10,
            ..Default::default()
        };

        let mut recorder = BlackboxRecorder::new(config).unwrap();

        // Add some entries
        let now = Instant::now();
        for i in 0..5 {
            recorder
                .record(create_test_axis_entry(now, i as f32))
                .unwrap();
        }

        let stats = recorder.get_statistics();
        assert_eq!(stats.total_entries, 5);
        assert_eq!(stats.buffer_utilization, 0.5); // 5/10
        assert!(!stats.active_fault_capture);
        assert_eq!(stats.completed_captures, 0);
    }

    #[test]
    fn test_double_fault_capture() {
        let mut recorder = BlackboxRecorder::default();

        let now = Instant::now();
        recorder
            .start_fault_capture(create_test_fault_entry(now))
            .unwrap();

        // Second fault capture should fail
        let result = recorder.start_fault_capture(create_test_fault_entry(now));
        assert!(matches!(
            result,
            Err(BlackboxError::FaultCaptureAlreadyActive)
        ));
    }

    #[test]
    fn test_config_validation() {
        let invalid_config = BlackboxConfig {
            max_entries: 0, // Invalid
            ..Default::default()
        };

        let result = BlackboxRecorder::new(invalid_config);
        assert!(matches!(result, Err(BlackboxError::InvalidConfig { .. })));
    }
}
