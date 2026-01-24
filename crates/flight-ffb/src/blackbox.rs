// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Blackbox recording system for fault analysis and diagnostics
//!
//! **Validates: Requirements FFB-SAFETY-01.12-13, FFB-BLACKBOX-01**
//!
//! Provides continuous circular buffer recording with:
//! - ≥3s window at ≥250 Hz (750+ samples minimum)
//! - 2s pre-fault capture
//! - 1s post-fault capture
//! - Bounded, rotating log (size/age-limited)
//!
//! # Architecture
//!
//! The blackbox system uses two complementary data structures:
//!
//! - [`BlackboxSample`]: High-rate telemetry samples captured at ≥250 Hz.
//!   Contains BusSnapshot data, FFB setpoints, and device status for each tick.
//!
//! - [`BlackboxEntry`]: General event entries for state changes, faults, and
//!   system events. Lower rate but richer context.
//!
//! Both are stored in the same ring buffer for unified fault analysis.

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use thiserror::Error;

/// High-rate telemetry sample for blackbox recording
///
/// **Validates: Requirement FFB-SAFETY-01.12**
/// Captures BusSnapshot at ≥250 Hz, FFB setpoints and actual device feedback
///
/// This struct is optimized for high-rate capture (≥250 Hz) and contains
/// the essential telemetry data needed for fault analysis:
/// - Axis input/output values
/// - FFB torque setpoints and actual values
/// - Device status flags
///
/// # Example
/// ```ignore
/// let sample = BlackboxSample::new(
///     "pitch_axis",
///     0.5,   // raw input
///     0.6,   // processed output
///     5.0,   // torque setpoint (Nm)
///     4.8,   // actual torque (Nm)
/// );
/// recorder.record_sample(sample)?;
/// ```
#[derive(Debug, Clone)]
pub struct BlackboxSample {
    /// Timestamp when sample was captured
    pub timestamp: Instant,
    /// Device/axis identifier
    pub device_id: String,
    /// Raw input value from device (-1.0 to 1.0)
    pub raw_input: f32,
    /// Processed output value after filtering/mapping
    pub processed_output: f32,
    /// FFB torque setpoint in Newton-meters
    pub torque_setpoint_nm: f32,
    /// Actual torque output in Newton-meters (from device feedback if available)
    pub actual_torque_nm: f32,
    /// Safety state at time of sample
    pub safety_state: BlackboxSafetyState,
    /// Device health flags
    pub device_flags: DeviceFlags,
}

/// Safety state for blackbox samples
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlackboxSafetyState {
    /// Safe torque mode (limited envelope)
    SafeTorque,
    /// High torque mode (full envelope)
    HighTorque,
    /// Faulted state (zero torque)
    Faulted,
    /// Unknown/not available
    Unknown,
}

impl Default for BlackboxSafetyState {
    fn default() -> Self {
        Self::Unknown
    }
}

impl std::fmt::Display for BlackboxSafetyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SafeTorque => write!(f, "SafeTorque"),
            Self::HighTorque => write!(f, "HighTorque"),
            Self::Faulted => write!(f, "Faulted"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Device health flags for blackbox samples
#[derive(Debug, Clone, Copy, Default)]
pub struct DeviceFlags {
    /// Device is connected
    pub connected: bool,
    /// Device is responsive (no USB stalls)
    pub responsive: bool,
    /// Over-temperature warning
    pub over_temp_warning: bool,
    /// Over-current warning
    pub over_current_warning: bool,
}

impl BlackboxSample {
    /// Create a new blackbox sample
    pub fn new(
        device_id: &str,
        raw_input: f32,
        processed_output: f32,
        torque_setpoint_nm: f32,
        actual_torque_nm: f32,
    ) -> Self {
        Self {
            timestamp: Instant::now(),
            device_id: device_id.to_string(),
            raw_input,
            processed_output,
            torque_setpoint_nm,
            actual_torque_nm,
            safety_state: BlackboxSafetyState::Unknown,
            device_flags: DeviceFlags::default(),
        }
    }

    /// Create a new blackbox sample with full details
    pub fn with_details(
        device_id: &str,
        raw_input: f32,
        processed_output: f32,
        torque_setpoint_nm: f32,
        actual_torque_nm: f32,
        safety_state: BlackboxSafetyState,
        device_flags: DeviceFlags,
    ) -> Self {
        Self {
            timestamp: Instant::now(),
            device_id: device_id.to_string(),
            raw_input,
            processed_output,
            torque_setpoint_nm,
            actual_torque_nm,
            safety_state,
            device_flags,
        }
    }

    /// Serialize sample for storage
    pub fn serialize(&self) -> String {
        format!(
            "SAMPLE,{:?},{},{},{},{},{},{},{}",
            self.timestamp.elapsed(),
            self.device_id,
            self.raw_input,
            self.processed_output,
            self.torque_setpoint_nm,
            self.actual_torque_nm,
            self.safety_state,
            if self.device_flags.connected {
                "C"
            } else {
                "-"
            },
        )
    }
}

impl From<BlackboxSample> for BlackboxEntry {
    fn from(sample: BlackboxSample) -> Self {
        BlackboxEntry::AxisFrame {
            timestamp: sample.timestamp,
            device_id: sample.device_id,
            raw_input: sample.raw_input,
            processed_output: sample.processed_output,
            torque_nm: sample.torque_setpoint_nm,
        }
    }
}

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
///
/// **Validates: Requirements FFB-SAFETY-01.12-13, FFB-BLACKBOX-01**
///
/// # Buffer Size Calculation
///
/// The ring buffer must hold ≥3s of data at ≥250 Hz:
/// - Minimum samples = 3s × 250 Hz = 750 samples
/// - Default `max_entries` = 10000 (provides ~40s at 250 Hz)
///
/// The extra capacity allows for:
/// - Burst events and state changes
/// - Multiple fault captures without data loss
/// - Margin for timing variations
#[derive(Debug, Clone)]
pub struct BlackboxConfig {
    /// Pre-fault capture duration (requirement: 2s)
    pub pre_fault_duration: Duration,
    /// Post-fault capture duration (requirement: 1s)
    pub post_fault_duration: Duration,
    /// Maximum entries in circular buffer
    ///
    /// **Requirement:** Must be ≥750 for 3s window at 250 Hz
    /// Default: 10000 entries (~40s at 250 Hz)
    pub max_entries: usize,
    /// Whether to auto-save on fault
    pub auto_save_on_fault: bool,
    /// Directory for saved captures
    pub save_directory: Option<String>,
    /// Target capture rate in Hz (requirement: ≥250 Hz)
    pub target_capture_rate_hz: u32,
    /// Maximum total log size in bytes (for rotation)
    pub max_log_size_bytes: u64,
    /// Maximum age of log files in seconds (for rotation)
    pub max_log_age_secs: u64,
    /// Maximum number of log files to keep
    pub max_log_files: usize,
}

/// Minimum buffer size for 3s window at 250 Hz
pub const MIN_BUFFER_SIZE_3S_250HZ: usize = 750;

/// Default buffer size (provides ~40s at 250 Hz)
pub const DEFAULT_BUFFER_SIZE: usize = 10000;

/// Minimum capture rate in Hz
pub const MIN_CAPTURE_RATE_HZ: u32 = 250;

impl Default for BlackboxConfig {
    fn default() -> Self {
        Self {
            pre_fault_duration: Duration::from_secs(2), // FFB-SAFETY-01.12: 2s pre-fault
            post_fault_duration: Duration::from_secs(1), // FFB-SAFETY-01.12: 1s post-fault
            // At 250Hz for 3s (2s pre + 1s post), we need 750 samples minimum
            // Default to 10000 entries (~40s at 250 Hz) for margin
            max_entries: DEFAULT_BUFFER_SIZE,
            auto_save_on_fault: true,
            save_directory: None,
            target_capture_rate_hz: MIN_CAPTURE_RATE_HZ, // FFB-SAFETY-01.12: ≥250 Hz
            max_log_size_bytes: 100 * 1024 * 1024,       // 100 MB max total
            max_log_age_secs: 7 * 24 * 60 * 60,          // 7 days max age
            max_log_files: 50,                           // Keep at most 50 files
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
///
/// **Validates: Requirements FFB-SAFETY-01.12-13**
/// - Captures BusSnapshot at ≥250 Hz
/// - Captures FFB setpoints and device status
/// - 2s pre-fault + 1s post-fault dump on fault
/// - Bounded, rotating log (size/age-limited)
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
    /// Last sample timestamp for rate tracking
    last_sample_time: Option<Instant>,
    /// Sample count for rate calculation
    sample_count: u64,
    /// Actual capture rate (calculated)
    actual_capture_rate_hz: f32,
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
    ///
    /// **Validates: Requirements FFB-SAFETY-01.12-13, FFB-BLACKBOX-01**
    ///
    /// # Errors
    /// Returns `InvalidConfig` if:
    /// - `max_entries` is 0
    /// - `max_entries` is less than `MIN_BUFFER_SIZE_3S_250HZ` (750) - warning only
    pub fn new(config: BlackboxConfig) -> BlackboxResult<Self> {
        if config.max_entries == 0 {
            return Err(BlackboxError::InvalidConfig {
                message: "max_entries must be > 0".to_string(),
            });
        }

        // Validate buffer size meets 3s @ 250 Hz requirement
        if config.max_entries < MIN_BUFFER_SIZE_3S_250HZ {
            tracing::warn!(
                "Buffer size {} is below minimum {} for 3s window at 250 Hz. \
                 Fault captures may be incomplete.",
                config.max_entries,
                MIN_BUFFER_SIZE_3S_250HZ
            );
        }

        // Validate capture rate meets requirement
        if config.target_capture_rate_hz < MIN_CAPTURE_RATE_HZ {
            tracing::warn!(
                "Target capture rate {} Hz is below required {} Hz minimum",
                config.target_capture_rate_hz,
                MIN_CAPTURE_RATE_HZ
            );
        }

        let max_entries = config.max_entries;
        Ok(Self {
            config,
            circular_buffer: VecDeque::with_capacity(max_entries),
            active_capture: None,
            completed_captures: Vec::new(),
            max_completed_captures: 10,
            recording_start: Instant::now(),
            last_sample_time: None,
            sample_count: 0,
            actual_capture_rate_hz: 0.0,
        })
    }

    /// Record a high-rate telemetry sample
    ///
    /// **Validates: Requirement FFB-SAFETY-01.12, FFB-BLACKBOX-01**
    /// Captures BusSnapshot at ≥250 Hz, FFB setpoints and actual device feedback
    ///
    /// This is the primary method for high-rate recording in the FFB loop.
    /// Call this at ≥250 Hz for each axis tick.
    ///
    /// # Example
    /// ```ignore
    /// let sample = BlackboxSample::new(
    ///     "pitch_axis",
    ///     raw_input,
    ///     processed_output,
    ///     torque_setpoint,
    ///     actual_torque,
    /// );
    /// recorder.record_sample(sample)?;
    /// ```
    pub fn record_sample(&mut self, sample: BlackboxSample) -> BlackboxResult<()> {
        // Convert sample to entry and record
        self.record(sample.into())
    }

    /// Record an entry to the blackbox
    ///
    /// **Validates: Requirement FFB-SAFETY-01.12**
    /// Captures BusSnapshot, FFB setpoints, and device status at ≥250 Hz
    pub fn record(&mut self, entry: BlackboxEntry) -> BlackboxResult<()> {
        // Track capture rate
        let now = Instant::now();
        if let Some(last_time) = self.last_sample_time {
            let elapsed = now.duration_since(last_time);
            if elapsed.as_secs_f32() > 0.0 {
                // Exponential moving average for rate calculation
                let instant_rate = 1.0 / elapsed.as_secs_f32();
                self.actual_capture_rate_hz =
                    0.9 * self.actual_capture_rate_hz + 0.1 * instant_rate;
            }
        }
        self.last_sample_time = Some(now);
        self.sample_count += 1;

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

                // Check if post-fault capture is complete (1s post-fault per FFB-SAFETY-01.12)
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

    /// Record a BusSnapshot sample (convenience method for high-rate telemetry)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.12**
    /// Captures BusSnapshot at ≥250 Hz
    pub fn record_bus_snapshot(
        &mut self,
        device_id: &str,
        raw_input: f32,
        processed_output: f32,
        torque_nm: f32,
    ) -> BlackboxResult<()> {
        self.record(BlackboxEntry::AxisFrame {
            timestamp: Instant::now(),
            device_id: device_id.to_string(),
            raw_input,
            processed_output,
            torque_nm,
        })
    }

    /// Record FFB setpoint (convenience method)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.12**
    /// Captures FFB setpoints and actual device feedback
    pub fn record_ffb_setpoint(
        &mut self,
        safety_state: &str,
        torque_setpoint: f32,
        actual_torque: f32,
    ) -> BlackboxResult<()> {
        self.record(BlackboxEntry::FfbState {
            timestamp: Instant::now(),
            safety_state: safety_state.to_string(),
            torque_setpoint,
            actual_torque,
        })
    }

    /// Record device status (convenience method)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.12**
    /// Captures device status
    pub fn record_device_status(&mut self, event_type: &str, details: &str) -> BlackboxResult<()> {
        self.record(BlackboxEntry::SystemEvent {
            timestamp: Instant::now(),
            event_type: event_type.to_string(),
            details: details.to_string(),
        })
    }

    /// Start fault capture (triggered when fault is detected)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.12**
    /// 2s pre-fault + 1s post-fault dump on fault
    pub fn start_fault_capture(&mut self, fault_entry: BlackboxEntry) -> BlackboxResult<()> {
        // If there's already an active capture, complete it first
        if let Some(active_capture) = self.active_capture.take() {
            self.completed_captures.push(active_capture);
            if self.completed_captures.len() > self.max_completed_captures {
                self.completed_captures.remove(0);
            }
        }

        let fault_time = fault_entry.timestamp();

        // Extract pre-fault entries from circular buffer (2s pre-fault per FFB-SAFETY-01.12)
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

    /// Save fault capture to file (uncompressed)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.13**
    /// Stored in bounded, rotating log (size/age-limited)
    ///
    /// For compressed exports, use [`save_fault_capture_compressed`] instead.
    pub fn save_fault_capture(&self, capture: &FaultCapture) -> BlackboxResult<String> {
        use std::fs;
        use std::io::Write;

        // Determine save directory
        let save_dir = if let Some(ref dir) = self.config.save_directory {
            std::path::PathBuf::from(dir)
        } else {
            // Default to logs/blackbox in current directory
            std::path::PathBuf::from("logs/blackbox")
        };

        // Create directory if it doesn't exist
        if let Err(e) = fs::create_dir_all(&save_dir) {
            return Err(BlackboxError::SaveFailed {
                message: format!("Failed to create blackbox directory: {}", e),
            });
        }

        // Perform log rotation before saving
        self.rotate_logs(&save_dir)?;

        // Generate filename with timestamp
        let filename = format!(
            "fault_capture_{}.txt",
            capture.fault_time.elapsed().as_millis()
        );
        let filepath = save_dir.join(&filename);

        // Build content
        let content = self.build_fault_capture_content(capture);

        // Write to file
        let mut file = fs::File::create(&filepath).map_err(|e| BlackboxError::SaveFailed {
            message: format!("Failed to create file: {}", e),
        })?;

        file.write_all(content.as_bytes())
            .map_err(|e| BlackboxError::SaveFailed {
                message: format!("Failed to write to file: {}", e),
            })?;

        tracing::info!("Saved fault capture to {}", filepath.display());

        Ok(filename)
    }

    /// Save fault capture to compressed gzip file
    ///
    /// **Validates: Requirements FFB-SAFETY-01.12-13, FFB-BLACKBOX-01**
    /// - Exports 3s window (2s pre-fault + 1s post-fault)
    /// - Uses gzip compression to reduce file size
    /// - Stored in bounded, rotating log (size/age-limited)
    ///
    /// The compressed file contains the same content as [`save_fault_capture`]
    /// but with gzip compression applied, typically achieving 70-90% size reduction
    /// for telemetry data.
    ///
    /// # Returns
    /// The filename of the saved compressed file (e.g., "fault_capture_12345.txt.gz")
    ///
    /// # Example
    /// ```ignore
    /// let recorder = BlackboxRecorder::default();
    /// // ... record samples and trigger fault capture ...
    /// for capture in recorder.get_completed_captures() {
    ///     let filename = recorder.save_fault_capture_compressed(capture)?;
    ///     println!("Saved compressed capture to: {}", filename);
    /// }
    /// ```
    pub fn save_fault_capture_compressed(&self, capture: &FaultCapture) -> BlackboxResult<String> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::fs;
        use std::io::Write;

        // Determine save directory
        let save_dir = if let Some(ref dir) = self.config.save_directory {
            std::path::PathBuf::from(dir)
        } else {
            // Default to logs/blackbox in current directory
            std::path::PathBuf::from("logs/blackbox")
        };

        // Create directory if it doesn't exist
        if let Err(e) = fs::create_dir_all(&save_dir) {
            return Err(BlackboxError::SaveFailed {
                message: format!("Failed to create blackbox directory: {}", e),
            });
        }

        // Perform log rotation before saving
        self.rotate_logs(&save_dir)?;

        // Generate filename with timestamp and .gz extension
        let filename = format!(
            "fault_capture_{}.txt.gz",
            capture.fault_time.elapsed().as_millis()
        );
        let filepath = save_dir.join(&filename);

        // Build content
        let content = self.build_fault_capture_content(capture);

        // Create gzip-compressed file
        let file = fs::File::create(&filepath).map_err(|e| BlackboxError::SaveFailed {
            message: format!("Failed to create compressed file: {}", e),
        })?;

        // Use default compression level (good balance of speed and ratio)
        let mut encoder = GzEncoder::new(file, Compression::default());

        encoder
            .write_all(content.as_bytes())
            .map_err(|e| BlackboxError::SaveFailed {
                message: format!("Failed to write compressed data: {}", e),
            })?;

        encoder.finish().map_err(|e| BlackboxError::SaveFailed {
            message: format!("Failed to finalize compressed file: {}", e),
        })?;

        // Log compression statistics
        if let Ok(metadata) = fs::metadata(&filepath) {
            let compressed_size = metadata.len();
            let uncompressed_size = content.len() as u64;
            let ratio = if uncompressed_size > 0 {
                (compressed_size as f64 / uncompressed_size as f64) * 100.0
            } else {
                100.0
            };
            tracing::info!(
                "Saved compressed fault capture to {} ({} bytes → {} bytes, {:.1}% of original)",
                filepath.display(),
                uncompressed_size,
                compressed_size,
                ratio
            );
        } else {
            tracing::info!("Saved compressed fault capture to {}", filepath.display());
        }

        Ok(filename)
    }

    /// Build the content string for a fault capture report
    ///
    /// This is used by both compressed and uncompressed save methods.
    fn build_fault_capture_content(&self, capture: &FaultCapture) -> String {
        let mut content = String::new();
        content.push_str("Fault Capture Report\n");
        content.push_str("======================\n\n");
        content.push_str(&format!("Fault Time: {:?}\n", capture.fault_time.elapsed()));
        content.push_str(&format!(
            "Fault Entry: {}\n",
            capture.fault_entry.serialize()
        ));
        content.push_str(&format!(
            "Capture Rate: {:.1} Hz (target: {} Hz)\n",
            self.actual_capture_rate_hz, self.config.target_capture_rate_hz
        ));
        content.push_str(&format!(
            "Capture Window: {:.1}s pre-fault + {:.1}s post-fault = {:.1}s total\n",
            self.config.pre_fault_duration.as_secs_f32(),
            self.config.post_fault_duration.as_secs_f32(),
            (self.config.pre_fault_duration + self.config.post_fault_duration).as_secs_f32()
        ));
        content.push_str(&format!(
            "\nPre-fault entries ({}, {:.1}s window):\n",
            capture.pre_fault_entries.len(),
            self.config.pre_fault_duration.as_secs_f32()
        ));
        content.push_str("------------------------\n");

        for entry in &capture.pre_fault_entries {
            content.push_str(&format!("{}\n", entry.serialize()));
        }

        content.push_str(&format!(
            "\nPost-fault entries ({}, {:.1}s window):\n",
            capture.post_fault_entries.len(),
            self.config.post_fault_duration.as_secs_f32()
        ));
        content.push_str("------------------------\n");
        for entry in &capture.post_fault_entries {
            content.push_str(&format!("{}\n", entry.serialize()));
        }

        content
    }

    /// Rotate logs to enforce size and age limits
    ///
    /// **Validates: Requirement FFB-SAFETY-01.13**
    /// Bounded, rotating log (size/age-limited) to prevent unbounded disk usage
    ///
    /// This method enforces three rotation criteria:
    /// 1. **Max file count**: Removes oldest files when count exceeds `max_log_files`
    /// 2. **Max total size**: Removes oldest files when total size exceeds `max_log_size_bytes`
    /// 3. **Max age**: Removes files older than `max_log_age_secs`
    ///
    /// Handles both uncompressed (.txt) and compressed (.txt.gz) log files.
    /// Files are processed oldest-first to ensure the most recent logs are preserved.
    pub fn rotate_logs(&self, save_dir: &std::path::Path) -> BlackboxResult<()> {
        use std::fs;

        // Get all log files in directory
        let entries = match fs::read_dir(save_dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(()), // Directory doesn't exist yet, nothing to rotate
        };

        let mut log_files: Vec<(std::path::PathBuf, std::fs::Metadata)> = Vec::new();
        let mut total_size: u64 = 0;

        for entry in entries.flatten() {
            let path = entry.path();
            // Handle both .txt and .txt.gz files
            let is_log_file = path
                .file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |name| {
                    name.starts_with("fault_capture_")
                        && (name.ends_with(".txt") || name.ends_with(".txt.gz"))
                });

            if is_log_file {
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();
                    log_files.push((path, metadata));
                }
            }
        }

        // Sort by modification time (oldest first)
        log_files.sort_by(|a, b| {
            let time_a = a.1.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let time_b = b.1.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            time_a.cmp(&time_b)
        });

        let now = std::time::SystemTime::now();
        let max_age = std::time::Duration::from_secs(self.config.max_log_age_secs);

        // Remove files that exceed limits
        // Process oldest files first (already sorted by modification time)
        let mut files_to_remove = Vec::new();
        let mut remaining_files = log_files.len();
        let mut remaining_size = total_size;

        for (path, metadata) in &log_files {
            let file_size = metadata.len();

            // Check if this file should be removed
            let is_too_old = metadata.modified().ok().map_or(false, |modified| {
                now.duration_since(modified).unwrap_or_default() > max_age
            });

            // Check if we exceed limits (file count or total size)
            // We need to remove oldest files until we're under the limits
            let exceeds_file_count = remaining_files > self.config.max_log_files;
            let exceeds_total_size = remaining_size > self.config.max_log_size_bytes;

            let should_remove = is_too_old || exceeds_file_count || exceeds_total_size;

            if should_remove {
                files_to_remove.push((path.clone(), file_size));
                remaining_files = remaining_files.saturating_sub(1);
                remaining_size = remaining_size.saturating_sub(file_size);
            }
        }

        // Actually remove the files
        for (path, _file_size) in files_to_remove {
            if let Ok(()) = fs::remove_file(&path) {
                tracing::debug!("Rotated old blackbox log: {}", path.display());
            }
        }

        Ok(())
    }

    /// Save all completed captures (uncompressed)
    pub fn save_all_captures(&self) -> BlackboxResult<Vec<String>> {
        let mut saved_files = Vec::new();

        for capture in &self.completed_captures {
            let filename = self.save_fault_capture(capture)?;
            saved_files.push(filename);
        }

        Ok(saved_files)
    }

    /// Save all completed captures with gzip compression
    ///
    /// **Validates: Requirements FFB-SAFETY-01.12-13, FFB-BLACKBOX-01**
    /// - Exports 3s window (2s pre-fault + 1s post-fault) for each capture
    /// - Uses gzip compression to reduce file size
    /// - Stored in bounded, rotating log (size/age-limited)
    ///
    /// # Returns
    /// A vector of filenames for all saved compressed captures
    pub fn save_all_captures_compressed(&self) -> BlackboxResult<Vec<String>> {
        let mut saved_files = Vec::new();

        for capture in &self.completed_captures {
            let filename = self.save_fault_capture_compressed(capture)?;
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
            actual_capture_rate_hz: self.actual_capture_rate_hz,
            target_capture_rate_hz: self.config.target_capture_rate_hz,
            total_samples: self.sample_count,
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
    /// Actual capture rate in Hz (should be ≥250 Hz per FFB-SAFETY-01.12)
    pub actual_capture_rate_hz: f32,
    /// Target capture rate in Hz
    pub target_capture_rate_hz: u32,
    /// Total samples recorded
    pub total_samples: u64,
}

impl Default for BlackboxRecorder {
    fn default() -> Self {
        Self::new(BlackboxConfig::default()).expect("Default config should be valid")
    }
}

/// Emergency stop reason
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmergencyStopReason {
    /// User pressed UI emergency stop button
    UiButton,
    /// Hardware emergency stop button pressed
    HardwareButton,
    /// Programmatic emergency stop (e.g., from external system)
    Programmatic,
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

        // Second fault capture should complete the first and start a new one
        // (changed behavior: we now allow starting a new capture, completing the old one)
        let result = recorder.start_fault_capture(create_test_fault_entry(now));
        assert!(result.is_ok());

        // Should have one completed capture and one active
        assert_eq!(recorder.get_completed_captures().len(), 1);
        assert!(recorder.get_active_capture().is_some());
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

    // =========================================================================
    // Ring Buffer Requirement Tests
    // **Validates: Requirement FFB-BLACKBOX-01**
    // Ring buffer should hold ≥3s of data at ≥250 Hz (750+ samples)
    // =========================================================================

    /// Test that default configuration meets 3s @ 250 Hz requirement
    ///
    /// **Validates: Requirement FFB-BLACKBOX-01**
    /// Ring buffer should hold ≥3s of data at ≥250 Hz (750+ samples)
    #[test]
    fn test_default_config_meets_3s_250hz_requirement() {
        let config = BlackboxConfig::default();

        // Verify default buffer size meets requirement
        assert!(
            config.max_entries >= MIN_BUFFER_SIZE_3S_250HZ,
            "Default buffer size {} must be >= {} for 3s @ 250 Hz",
            config.max_entries,
            MIN_BUFFER_SIZE_3S_250HZ
        );

        // Verify default capture rate meets requirement
        assert!(
            config.target_capture_rate_hz >= MIN_CAPTURE_RATE_HZ,
            "Default capture rate {} Hz must be >= {} Hz",
            config.target_capture_rate_hz,
            MIN_CAPTURE_RATE_HZ
        );

        // Verify pre-fault + post-fault = 3s
        let total_capture_duration = config.pre_fault_duration + config.post_fault_duration;
        assert_eq!(
            total_capture_duration,
            Duration::from_secs(3),
            "Pre-fault ({:?}) + post-fault ({:?}) should equal 3s",
            config.pre_fault_duration,
            config.post_fault_duration
        );
    }

    /// Test that buffer can hold exactly 750 samples (minimum for 3s @ 250 Hz)
    ///
    /// **Validates: Requirement FFB-BLACKBOX-01**
    #[test]
    fn test_buffer_holds_minimum_750_samples() {
        let config = BlackboxConfig {
            max_entries: MIN_BUFFER_SIZE_3S_250HZ, // Exactly 750
            ..Default::default()
        };

        let mut recorder = BlackboxRecorder::new(config).unwrap();

        // Fill buffer with exactly 750 samples
        let now = Instant::now();
        for i in 0..MIN_BUFFER_SIZE_3S_250HZ {
            recorder
                .record(create_test_axis_entry(now, i as f32))
                .unwrap();
        }

        // Verify all samples are stored
        assert_eq!(
            recorder.get_all_entries().len(),
            MIN_BUFFER_SIZE_3S_250HZ,
            "Buffer should hold exactly {} samples",
            MIN_BUFFER_SIZE_3S_250HZ
        );

        // Verify buffer utilization is 100%
        let stats = recorder.get_statistics();
        assert!(
            (stats.buffer_utilization - 1.0).abs() < 0.001,
            "Buffer utilization should be 100%"
        );
    }

    /// Test that buffer correctly rotates when full
    ///
    /// **Validates: Requirement FFB-BLACKBOX-01**
    #[test]
    fn test_buffer_rotation_preserves_recent_samples() {
        let config = BlackboxConfig {
            max_entries: MIN_BUFFER_SIZE_3S_250HZ,
            ..Default::default()
        };

        let mut recorder = BlackboxRecorder::new(config).unwrap();

        // Fill buffer with 750 samples, then add 250 more (simulating 1s of new data)
        let now = Instant::now();
        for i in 0..(MIN_BUFFER_SIZE_3S_250HZ + 250) {
            recorder
                .record(create_test_axis_entry(now, i as f32))
                .unwrap();
        }

        // Buffer should still be at capacity
        assert_eq!(recorder.get_all_entries().len(), MIN_BUFFER_SIZE_3S_250HZ);

        // Oldest samples should be evicted, newest should be preserved
        // First entry should be sample 250 (0-249 were evicted)
        if let BlackboxEntry::AxisFrame { torque_nm, .. } =
            recorder.get_all_entries().front().unwrap()
        {
            assert_eq!(
                *torque_nm, 250.0,
                "Oldest sample should be 250 (first 250 evicted)"
            );
        }

        // Last entry should be sample 999
        if let BlackboxEntry::AxisFrame { torque_nm, .. } =
            recorder.get_all_entries().back().unwrap()
        {
            assert_eq!(
                *torque_nm,
                (MIN_BUFFER_SIZE_3S_250HZ + 249) as f32,
                "Newest sample should be the last one recorded"
            );
        }
    }

    /// Test BlackboxSample creation and recording
    ///
    /// **Validates: Requirement FFB-BLACKBOX-01**
    #[test]
    fn test_blackbox_sample_creation() {
        let sample = BlackboxSample::new(
            "pitch_axis",
            0.5, // raw input
            0.6, // processed output
            5.0, // torque setpoint
            4.8, // actual torque
        );

        assert_eq!(sample.device_id, "pitch_axis");
        assert_eq!(sample.raw_input, 0.5);
        assert_eq!(sample.processed_output, 0.6);
        assert_eq!(sample.torque_setpoint_nm, 5.0);
        assert_eq!(sample.actual_torque_nm, 4.8);
        assert_eq!(sample.safety_state, BlackboxSafetyState::Unknown);
    }

    /// Test BlackboxSample with full details
    ///
    /// **Validates: Requirement FFB-BLACKBOX-01**
    #[test]
    fn test_blackbox_sample_with_details() {
        let flags = DeviceFlags {
            connected: true,
            responsive: true,
            over_temp_warning: false,
            over_current_warning: false,
        };

        let sample = BlackboxSample::with_details(
            "roll_axis",
            -0.3,
            -0.25,
            3.0,
            2.9,
            BlackboxSafetyState::HighTorque,
            flags,
        );

        assert_eq!(sample.device_id, "roll_axis");
        assert_eq!(sample.safety_state, BlackboxSafetyState::HighTorque);
        assert!(sample.device_flags.connected);
        assert!(sample.device_flags.responsive);
    }

    /// Test recording BlackboxSample via record_sample method
    ///
    /// **Validates: Requirement FFB-BLACKBOX-01**
    #[test]
    fn test_record_sample_method() {
        let mut recorder = BlackboxRecorder::default();

        let sample = BlackboxSample::new("test_axis", 0.5, 0.6, 5.0, 4.8);
        recorder.record_sample(sample).unwrap();

        assert_eq!(recorder.get_all_entries().len(), 1);

        // Verify the sample was converted to AxisFrame entry
        if let BlackboxEntry::AxisFrame {
            device_id,
            raw_input,
            torque_nm,
            ..
        } = recorder.get_all_entries().front().unwrap()
        {
            assert_eq!(device_id, "test_axis");
            assert_eq!(*raw_input, 0.5);
            assert_eq!(*torque_nm, 5.0); // torque_setpoint_nm becomes torque_nm
        } else {
            panic!("Expected AxisFrame entry");
        }
    }

    /// Test BlackboxSample serialization
    ///
    /// **Validates: Requirement FFB-BLACKBOX-01**
    #[test]
    fn test_blackbox_sample_serialization() {
        let sample = BlackboxSample::new("test_axis", 0.5, 0.6, 5.0, 4.8);
        let serialized = sample.serialize();

        assert!(serialized.contains("SAMPLE"));
        assert!(serialized.contains("test_axis"));
        assert!(serialized.contains("0.5"));
        assert!(serialized.contains("5")); // torque setpoint
    }

    /// Test that constants are correctly defined
    ///
    /// **Validates: Requirement FFB-BLACKBOX-01**
    #[test]
    fn test_buffer_constants() {
        // 3s @ 250 Hz = 750 samples
        assert_eq!(MIN_BUFFER_SIZE_3S_250HZ, 750);

        // Default buffer should be larger than minimum
        assert!(DEFAULT_BUFFER_SIZE >= MIN_BUFFER_SIZE_3S_250HZ);

        // Minimum capture rate
        assert_eq!(MIN_CAPTURE_RATE_HZ, 250);
    }

    /// Test buffer size warning for undersized configuration
    ///
    /// **Validates: Requirement FFB-BLACKBOX-01**
    #[test]
    fn test_undersized_buffer_warning() {
        // This should succeed but log a warning
        let config = BlackboxConfig {
            max_entries: 500, // Below minimum 750
            ..Default::default()
        };

        // Should still create successfully (warning only)
        let result = BlackboxRecorder::new(config);
        assert!(result.is_ok());
    }

    /// Test capture rate tracking
    ///
    /// **Validates: Requirement FFB-BLACKBOX-01**
    #[test]
    fn test_capture_rate_tracking() {
        let config = BlackboxConfig {
            target_capture_rate_hz: 250,
            ..Default::default()
        };

        let mut recorder = BlackboxRecorder::new(config).unwrap();

        // Record samples with simulated timing
        let now = Instant::now();
        for i in 0..10 {
            recorder
                .record(create_test_axis_entry(now, i as f32))
                .unwrap();
            // Small delay to simulate real recording
            thread::sleep(Duration::from_micros(100));
        }

        let stats = recorder.get_statistics();
        assert_eq!(stats.target_capture_rate_hz, 250);
        assert!(stats.total_samples >= 10);
    }

    /// Test 2s pre-fault + 1s post-fault capture window
    ///
    /// **Validates: Requirement FFB-SAFETY-01.12**
    #[test]
    fn test_3s_capture_window() {
        let config = BlackboxConfig {
            pre_fault_duration: Duration::from_secs(2),
            post_fault_duration: Duration::from_secs(1),
            max_entries: DEFAULT_BUFFER_SIZE,
            ..Default::default()
        };

        let recorder = BlackboxRecorder::new(config).unwrap();
        let cfg = recorder.get_config();

        // Verify capture window configuration
        assert_eq!(cfg.pre_fault_duration, Duration::from_secs(2));
        assert_eq!(cfg.post_fault_duration, Duration::from_secs(1));

        // Total capture window should be 3s
        let total = cfg.pre_fault_duration + cfg.post_fault_duration;
        assert_eq!(total, Duration::from_secs(3));
    }

    // =========================================================================
    // Compressed Export Tests
    // **Validates: Requirements FFB-SAFETY-01.12-13, FFB-BLACKBOX-01**
    // =========================================================================

    /// Test compressed fault capture export
    ///
    /// **Validates: Requirements FFB-SAFETY-01.12-13, FFB-BLACKBOX-01**
    #[test]
    fn test_save_fault_capture_compressed() {
        use flate2::read::GzDecoder;
        use std::fs;
        use std::io::Read;

        let temp_dir = tempfile::tempdir().unwrap();
        let config = BlackboxConfig {
            pre_fault_duration: Duration::from_millis(100),
            post_fault_duration: Duration::from_millis(100),
            max_entries: 100,
            save_directory: Some(temp_dir.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        let mut recorder = BlackboxRecorder::new(config).unwrap();

        let start_time = Instant::now();

        // Record some pre-fault entries
        recorder
            .record(create_test_axis_entry(start_time, 1.0))
            .unwrap();
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

        // Record post-fault entries
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

        // Capture should be complete
        assert_eq!(recorder.get_completed_captures().len(), 1);

        // Save compressed
        let capture = &recorder.get_completed_captures()[0];
        let filename = recorder.save_fault_capture_compressed(capture).unwrap();

        // Verify filename has .gz extension
        assert!(
            filename.ends_with(".txt.gz"),
            "Filename should end with .txt.gz"
        );

        // Verify file exists
        let filepath = temp_dir.path().join(&filename);
        assert!(filepath.exists(), "Compressed file should exist");

        // Verify file can be decompressed and contains expected content
        let file = fs::File::open(&filepath).unwrap();
        let mut decoder = GzDecoder::new(file);
        let mut content = String::new();
        decoder.read_to_string(&mut content).unwrap();

        // Verify content structure
        assert!(content.contains("Fault Capture Report"));
        assert!(content.contains("Pre-fault entries"));
        assert!(content.contains("Post-fault entries"));
        assert!(content.contains("Capture Window:"));
        assert!(content.contains("AXIS"));
        assert!(content.contains("FAULT"));
    }

    /// Test that compressed files are smaller than uncompressed
    ///
    /// **Validates: Requirement FFB-BLACKBOX-01**
    #[test]
    fn test_compression_reduces_file_size() {
        use std::fs;

        let temp_dir = tempfile::tempdir().unwrap();
        let config = BlackboxConfig {
            pre_fault_duration: Duration::from_millis(100),
            post_fault_duration: Duration::from_millis(100),
            max_entries: 1000,
            save_directory: Some(temp_dir.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        let mut recorder = BlackboxRecorder::new(config).unwrap();

        let start_time = Instant::now();

        // Record many entries to get meaningful compression
        for i in 0..100 {
            recorder
                .record(create_test_axis_entry(
                    start_time + Duration::from_millis(i),
                    i as f32 * 0.1,
                ))
                .unwrap();
        }

        // Record fault
        let fault_time = start_time + Duration::from_millis(100);
        recorder
            .start_fault_capture(create_test_fault_entry(fault_time))
            .unwrap();

        // Record post-fault entries
        for i in 0..100 {
            recorder
                .record(create_test_axis_entry(
                    fault_time + Duration::from_millis(i + 1),
                    (100 + i) as f32 * 0.1,
                ))
                .unwrap();
        }

        // Complete the capture
        recorder
            .record(create_test_axis_entry(
                fault_time + Duration::from_millis(150),
                999.0,
            ))
            .unwrap();

        assert_eq!(recorder.get_completed_captures().len(), 1);
        let capture = &recorder.get_completed_captures()[0];

        // Save both compressed and uncompressed
        let uncompressed_filename = recorder.save_fault_capture(capture).unwrap();
        let compressed_filename = recorder.save_fault_capture_compressed(capture).unwrap();

        let uncompressed_path = temp_dir.path().join(&uncompressed_filename);
        let compressed_path = temp_dir.path().join(&compressed_filename);

        let uncompressed_size = fs::metadata(&uncompressed_path).unwrap().len();
        let compressed_size = fs::metadata(&compressed_path).unwrap().len();

        // Compressed should be smaller (telemetry data compresses well)
        assert!(
            compressed_size < uncompressed_size,
            "Compressed size ({}) should be smaller than uncompressed size ({})",
            compressed_size,
            uncompressed_size
        );

        // Typically expect at least 50% compression for repetitive telemetry data
        let compression_ratio = compressed_size as f64 / uncompressed_size as f64;
        assert!(
            compression_ratio < 0.8,
            "Expected at least 20% compression, got {:.1}%",
            (1.0 - compression_ratio) * 100.0
        );
    }

    /// Test save_all_captures_compressed
    ///
    /// **Validates: Requirements FFB-SAFETY-01.12-13, FFB-BLACKBOX-01**
    #[test]
    fn test_save_all_captures_compressed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = BlackboxConfig {
            pre_fault_duration: Duration::from_millis(50),
            post_fault_duration: Duration::from_millis(50),
            max_entries: 100,
            save_directory: Some(temp_dir.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        let mut recorder = BlackboxRecorder::new(config).unwrap();

        // Create two fault captures
        let start_time = Instant::now();

        // First fault
        recorder
            .record(create_test_axis_entry(start_time, 1.0))
            .unwrap();
        let fault_time1 = start_time + Duration::from_millis(60);
        recorder
            .start_fault_capture(create_test_fault_entry(fault_time1))
            .unwrap();
        recorder
            .record(create_test_axis_entry(
                fault_time1 + Duration::from_millis(60),
                2.0,
            ))
            .unwrap();

        // Second fault
        let fault_time2 = fault_time1 + Duration::from_millis(100);
        recorder
            .start_fault_capture(create_test_fault_entry(fault_time2))
            .unwrap();
        recorder
            .record(create_test_axis_entry(
                fault_time2 + Duration::from_millis(60),
                3.0,
            ))
            .unwrap();

        // Should have 2 completed captures
        assert_eq!(recorder.get_completed_captures().len(), 2);

        // Save all compressed
        let filenames = recorder.save_all_captures_compressed().unwrap();
        assert_eq!(filenames.len(), 2);

        // Verify all files exist and are compressed
        for filename in &filenames {
            assert!(filename.ends_with(".txt.gz"));
            let filepath = temp_dir.path().join(filename);
            assert!(filepath.exists());
        }
    }

    /// Test log rotation handles both .txt and .txt.gz files
    ///
    /// **Validates: Requirement FFB-SAFETY-01.13**
    #[test]
    fn test_log_rotation_handles_compressed_files() {
        use std::fs;
        use std::io::Write;

        let temp_dir = tempfile::tempdir().unwrap();
        let config = BlackboxConfig {
            max_log_files: 3,
            save_directory: Some(temp_dir.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        let recorder = BlackboxRecorder::new(config).unwrap();

        // Create some old log files (both compressed and uncompressed)
        for i in 0..5 {
            let txt_path = temp_dir.path().join(format!("fault_capture_{}.txt", i));
            let gz_path = temp_dir
                .path()
                .join(format!("fault_capture_{}.txt.gz", i + 10));

            let mut txt_file = fs::File::create(&txt_path).unwrap();
            txt_file.write_all(b"test content").unwrap();

            let mut gz_file = fs::File::create(&gz_path).unwrap();
            gz_file.write_all(b"compressed content").unwrap();
        }

        // Trigger rotation
        recorder.rotate_logs(temp_dir.path()).unwrap();

        // Count remaining files
        let remaining: Vec<_> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("fault_capture_")
            })
            .collect();

        // Should have at most max_log_files (3) remaining
        assert!(
            remaining.len() <= 3,
            "Expected at most 3 files, got {}",
            remaining.len()
        );
    }

    /// Test build_fault_capture_content includes 3s window info
    ///
    /// **Validates: Requirements FFB-SAFETY-01.12, FFB-BLACKBOX-01**
    #[test]
    fn test_fault_capture_content_includes_window_info() {
        let config = BlackboxConfig {
            pre_fault_duration: Duration::from_secs(2),
            post_fault_duration: Duration::from_secs(1),
            max_entries: 100,
            ..Default::default()
        };

        let mut recorder = BlackboxRecorder::new(config).unwrap();

        let start_time = Instant::now();
        recorder
            .record(create_test_axis_entry(start_time, 1.0))
            .unwrap();

        let fault_time = start_time + Duration::from_millis(100);
        recorder
            .start_fault_capture(create_test_fault_entry(fault_time))
            .unwrap();

        recorder
            .record(create_test_axis_entry(
                fault_time + Duration::from_secs(2),
                2.0,
            ))
            .unwrap();

        let capture = &recorder.get_completed_captures()[0];
        let content = recorder.build_fault_capture_content(capture);

        // Verify content includes capture window information
        assert!(content.contains("Capture Window:"));
        assert!(content.contains("2.0s pre-fault"));
        assert!(content.contains("1.0s post-fault"));
        assert!(content.contains("3.0s total"));
    }

    // =========================================================================
    // Log Rotation Tests
    // **Validates: Requirement FFB-SAFETY-01.13, FFB-BLACKBOX-01**
    // Log rotation should enforce max file count, max total size, and max age
    // =========================================================================

    /// Test log rotation enforces max file count limit
    ///
    /// **Validates: Requirement FFB-SAFETY-01.13**
    /// Log rotation SHALL enforce max N files limit
    #[test]
    fn test_log_rotation_max_file_count() {
        use std::fs;
        use std::io::Write;

        let temp_dir = tempfile::tempdir().unwrap();
        let config = BlackboxConfig {
            max_log_files: 3,
            max_log_size_bytes: u64::MAX, // Disable size limit
            max_log_age_secs: u64::MAX,   // Disable age limit
            save_directory: Some(temp_dir.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        let recorder = BlackboxRecorder::new(config).unwrap();

        // Create 5 log files (exceeds max of 3)
        for i in 0..5 {
            let path = temp_dir.path().join(format!("fault_capture_{}.txt", i));
            let mut file = fs::File::create(&path).unwrap();
            file.write_all(format!("test content {}", i).as_bytes())
                .unwrap();
            // Add small delay to ensure different modification times
            thread::sleep(Duration::from_millis(10));
        }

        // Verify we have 5 files before rotation
        let count_before: usize = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("fault_capture_")
            })
            .count();
        assert_eq!(count_before, 5);

        // Trigger rotation
        recorder.rotate_logs(temp_dir.path()).unwrap();

        // Count remaining files
        let remaining: Vec<_> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("fault_capture_")
            })
            .collect();

        // Should have exactly max_log_files (3) remaining
        assert_eq!(
            remaining.len(),
            3,
            "Expected exactly 3 files after rotation, got {}",
            remaining.len()
        );

        // Verify the newest files were kept (files 2, 3, 4)
        let remaining_names: Vec<String> = remaining
            .iter()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();

        assert!(remaining_names.contains(&"fault_capture_2.txt".to_string()));
        assert!(remaining_names.contains(&"fault_capture_3.txt".to_string()));
        assert!(remaining_names.contains(&"fault_capture_4.txt".to_string()));
    }

    /// Test log rotation enforces max total size limit
    ///
    /// **Validates: Requirement FFB-SAFETY-01.13**
    /// Log rotation SHALL enforce max total size limit
    #[test]
    fn test_log_rotation_max_total_size() {
        use std::fs;
        use std::io::Write;

        let temp_dir = tempfile::tempdir().unwrap();

        // Set max size to 50 bytes
        let config = BlackboxConfig {
            max_log_files: usize::MAX,  // Disable file count limit
            max_log_size_bytes: 50,     // 50 bytes max
            max_log_age_secs: u64::MAX, // Disable age limit
            save_directory: Some(temp_dir.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        let recorder = BlackboxRecorder::new(config).unwrap();

        // Create files with known sizes (each ~20 bytes)
        // Total will be ~100 bytes, exceeding 50 byte limit
        for i in 0..5 {
            let path = temp_dir.path().join(format!("fault_capture_{}.txt", i));
            let mut file = fs::File::create(&path).unwrap();
            file.write_all(format!("content_{:010}", i).as_bytes())
                .unwrap(); // ~20 bytes each
            thread::sleep(Duration::from_millis(10));
        }

        // Calculate total size before rotation
        let total_before: u64 = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("fault_capture_")
            })
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum();

        assert!(total_before > 50, "Total size before should exceed limit");

        // Trigger rotation
        recorder.rotate_logs(temp_dir.path()).unwrap();

        // Calculate total size after rotation
        let total_after: u64 = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("fault_capture_")
            })
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum();

        // Total size should now be at or below the limit
        assert!(
            total_after <= 50,
            "Total size after rotation ({}) should be <= 50 bytes",
            total_after
        );
    }

    /// Test log rotation enforces max age limit
    ///
    /// **Validates: Requirement FFB-SAFETY-01.13**
    /// Log rotation SHALL enforce max age limit
    #[test]
    fn test_log_rotation_max_age() {
        use filetime::{FileTime, set_file_mtime};
        use std::fs;
        use std::io::Write;

        let temp_dir = tempfile::tempdir().unwrap();

        // Set max age to 1 second
        let config = BlackboxConfig {
            max_log_files: usize::MAX,    // Disable file count limit
            max_log_size_bytes: u64::MAX, // Disable size limit
            max_log_age_secs: 1,          // 1 second max age
            save_directory: Some(temp_dir.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        let recorder = BlackboxRecorder::new(config).unwrap();

        // Create files - some old, some new
        let now = std::time::SystemTime::now();
        let old_time = now - std::time::Duration::from_secs(10); // 10 seconds ago

        // Create 2 old files
        for i in 0..2 {
            let path = temp_dir.path().join(format!("fault_capture_old_{}.txt", i));
            let mut file = fs::File::create(&path).unwrap();
            file.write_all(b"old content").unwrap();
            drop(file);

            // Set modification time to 10 seconds ago
            let old_filetime = FileTime::from_system_time(old_time);
            set_file_mtime(&path, old_filetime).unwrap();
        }

        // Create 2 new files (current time)
        for i in 0..2 {
            let path = temp_dir.path().join(format!("fault_capture_new_{}.txt", i));
            let mut file = fs::File::create(&path).unwrap();
            file.write_all(b"new content").unwrap();
        }

        // Verify we have 4 files before rotation
        let count_before: usize = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("fault_capture_")
            })
            .count();
        assert_eq!(count_before, 4);

        // Trigger rotation
        recorder.rotate_logs(temp_dir.path()).unwrap();

        // Count remaining files
        let remaining: Vec<_> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("fault_capture_")
            })
            .collect();

        // Should have only the 2 new files remaining
        assert_eq!(
            remaining.len(),
            2,
            "Expected 2 files after rotation (old files removed), got {}",
            remaining.len()
        );

        // Verify only new files remain
        for entry in &remaining {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                name.contains("new"),
                "Only new files should remain, found: {}",
                name
            );
        }
    }

    /// Test log rotation handles empty directory gracefully
    ///
    /// **Validates: Requirement FFB-SAFETY-01.13**
    #[test]
    fn test_log_rotation_empty_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = BlackboxConfig {
            max_log_files: 3,
            save_directory: Some(temp_dir.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        let recorder = BlackboxRecorder::new(config).unwrap();

        // Rotation on empty directory should succeed
        let result = recorder.rotate_logs(temp_dir.path());
        assert!(result.is_ok());
    }

    /// Test log rotation handles non-existent directory gracefully
    ///
    /// **Validates: Requirement FFB-SAFETY-01.13**
    #[test]
    fn test_log_rotation_nonexistent_directory() {
        let config = BlackboxConfig::default();
        let recorder = BlackboxRecorder::new(config).unwrap();

        // Rotation on non-existent directory should succeed (no-op)
        let result = recorder.rotate_logs(std::path::Path::new(
            "/nonexistent/path/that/does/not/exist",
        ));
        assert!(result.is_ok());
    }

    /// Test log rotation ignores non-log files
    ///
    /// **Validates: Requirement FFB-SAFETY-01.13**
    #[test]
    fn test_log_rotation_ignores_non_log_files() {
        use std::fs;
        use std::io::Write;

        let temp_dir = tempfile::tempdir().unwrap();
        let config = BlackboxConfig {
            max_log_files: 2,
            save_directory: Some(temp_dir.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        let recorder = BlackboxRecorder::new(config).unwrap();

        // Create log files
        for i in 0..4 {
            let path = temp_dir.path().join(format!("fault_capture_{}.txt", i));
            let mut file = fs::File::create(&path).unwrap();
            file.write_all(b"log content").unwrap();
            thread::sleep(Duration::from_millis(10));
        }

        // Create non-log files that should be ignored
        let other_path = temp_dir.path().join("other_file.txt");
        let mut other_file = fs::File::create(&other_path).unwrap();
        other_file.write_all(b"other content").unwrap();

        let readme_path = temp_dir.path().join("README.md");
        let mut readme_file = fs::File::create(&readme_path).unwrap();
        readme_file.write_all(b"readme content").unwrap();

        // Trigger rotation
        recorder.rotate_logs(temp_dir.path()).unwrap();

        // Non-log files should still exist
        assert!(other_path.exists(), "other_file.txt should not be deleted");
        assert!(readme_path.exists(), "README.md should not be deleted");

        // Only 2 log files should remain
        let log_count: usize = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("fault_capture_")
            })
            .count();
        assert_eq!(log_count, 2);
    }

    /// Test log rotation is called automatically when saving captures
    ///
    /// **Validates: Requirement FFB-SAFETY-01.13**
    #[test]
    fn test_log_rotation_automatic_on_save() {
        use std::fs;
        use std::io::Write;

        let temp_dir = tempfile::tempdir().unwrap();
        let config = BlackboxConfig {
            max_log_files: 3,
            pre_fault_duration: Duration::from_millis(50),
            post_fault_duration: Duration::from_millis(50),
            max_entries: 100,
            save_directory: Some(temp_dir.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        let mut recorder = BlackboxRecorder::new(config).unwrap();

        // Create existing log files (will exceed limit after save)
        for i in 0..3 {
            let path = temp_dir.path().join(format!("fault_capture_{}.txt", i));
            let mut file = fs::File::create(&path).unwrap();
            file.write_all(b"existing content").unwrap();
            thread::sleep(Duration::from_millis(10));
        }

        // Create a fault capture
        let start_time = Instant::now();
        recorder
            .record(create_test_axis_entry(start_time, 1.0))
            .unwrap();

        let fault_time = start_time + Duration::from_millis(60);
        recorder
            .start_fault_capture(create_test_fault_entry(fault_time))
            .unwrap();

        recorder
            .record(create_test_axis_entry(
                fault_time + Duration::from_millis(60),
                2.0,
            ))
            .unwrap();

        // Save the capture (should trigger rotation)
        let capture = &recorder.get_completed_captures()[0];
        let _filename = recorder.save_fault_capture(capture).unwrap();

        // Count total log files
        let log_count: usize = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("fault_capture_") && name.ends_with(".txt")
            })
            .count();

        // Should have at most max_log_files (3) after rotation
        assert!(
            log_count <= 3,
            "Expected at most 3 log files after save with rotation, got {}",
            log_count
        );
    }

    /// Test default log directory is logs/blackbox
    ///
    /// **Validates: Requirement FFB-BLACKBOX-01**
    #[test]
    fn test_default_log_directory() {
        let config = BlackboxConfig::default();

        // Default save_directory should be None (uses logs/blackbox)
        assert!(config.save_directory.is_none());

        // Verify default values for rotation
        assert_eq!(config.max_log_size_bytes, 100 * 1024 * 1024); // 100 MB
        assert_eq!(config.max_log_age_secs, 7 * 24 * 60 * 60); // 7 days
        assert_eq!(config.max_log_files, 50);
    }
}
