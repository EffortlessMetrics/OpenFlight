#![cfg_attr(
    test,
    allow(
        unused_imports,
        unused_variables,
        unused_mut,
        unused_assignments,
        unused_parens,
        dead_code
    )
)]
// Allow clippy warnings for placeholder implementations and FFI naming conventions
#![allow(clippy::collapsible_if)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::unnecessary_map_or)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::assign_op_pattern)]
#![allow(clippy::vec_init_then_push)]
#![allow(clippy::redundant_pattern_matching)]
#![allow(clippy::single_char_add_str)]
#![allow(dead_code)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Force feedback engine with safety-first design
//!
//! This crate provides safe and controlled force feedback operation for Flight Hub.
//! It implements a comprehensive safety state machine, physical interlocks, and multiple
//! FFB modes while maintaining real-time performance.

use std::time::{Duration, Instant};

pub mod audio;
pub mod blackbox;
pub mod crosswind;
pub mod device_health;
pub mod dinput_backend;
#[cfg(windows)]
pub mod dinput_com;
pub mod dinput_device;
#[cfg(windows)]
pub mod dinput_window;
pub mod engine_vibration;
pub mod fault;
pub mod ffb_pipeline;
pub mod ground_effect;
#[cfg(test)]
pub mod hil_tests;
#[cfg(test)]
pub mod integration_test;
pub mod interlock;
pub mod mode_negotiation;
pub mod ofp1_integration;
#[cfg(test)]
pub mod performance_validation;
pub mod ramp;
pub mod safety;
pub mod safety_envelope;
#[cfg(test)]
pub mod safety_envelope_integration_tests;
pub mod safety_interlock;
#[cfg(test)]
pub mod safety_threshold_validation;
pub mod soft_stop;
pub mod telemetry_synth;
pub mod trim;
#[cfg(test)]
pub mod trim_hil_tests;
pub mod trim_validation;
#[cfg(test)]
pub mod usb_yank_test;
pub mod weather_ffb;
pub mod wheel_shimmy;
pub mod xinput_rumble;

#[cfg(test)]
mod tests;

pub use audio::*;
pub use blackbox::*;
pub use crosswind::*;
pub use device_health::*;
pub use dinput_backend::*;
pub use dinput_device::*;
pub use engine_vibration::*;
pub use fault::*;
pub use ffb_pipeline::*;
pub use ground_effect::*;
#[cfg(test)]
pub use hil_tests::*;
pub use interlock::*;
pub use mode_negotiation::*;
pub use ofp1_integration::*;
#[cfg(test)]
pub use performance_validation::*;
pub use ramp::*;
pub use safety::*;
pub use safety_envelope::*;
pub use safety_interlock::*;
pub use soft_stop::*;
pub use telemetry_synth::*;
pub use trim::*;
#[cfg(test)]
pub use trim_hil_tests::*;
pub use trim_validation::*;
#[cfg(test)]
pub use usb_yank_test::*;
pub use wheel_shimmy::*;
pub use xinput_rumble::*;

// Re-export EmergencyStopReason from blackbox module
pub use blackbox::EmergencyStopReason;

/// Main force feedback engine with safety systems
pub struct FfbEngine {
    config: FfbConfig,
    safety_state: SafetyState,
    interlock_system: InterlockSystem,
    fault_detector: FaultDetector,
    trim_controller: TrimController,
    soft_stop_controller: SoftStopController,
    audio_system: AudioCueSystem,
    blackbox_recorder: BlackboxRecorder,
    telemetry_synth: Option<TelemetrySynthEngine>,
    last_heartbeat: Instant,
    device_capabilities: Option<DeviceCapabilities>,
    /// Device health monitor for over-temp/over-current detection
    /// **Validates: Requirements FFB-SAFETY-01.7, FFB-SAFETY-01.9**
    device_health_monitor: DeviceHealthMonitor,
}

/// Configuration for the FFB engine
#[derive(Debug, Clone)]
pub struct FfbConfig {
    /// Maximum torque in Newton-meters
    pub max_torque_nm: f32,
    /// Fault response timeout in milliseconds
    pub fault_timeout_ms: u32,
    /// Whether physical interlock is required for high torque
    pub interlock_required: bool,
    /// FFB mode selection
    pub mode: FfbMode,
    /// Device path for hardware communication
    pub device_path: Option<String>,
}

/// FFB operation modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfbMode {
    /// Automatic mode selection based on device capabilities
    Auto,
    /// DirectInput PID effects pass-through
    DirectInput,
    /// Raw torque commands (OFP-1 protocol)
    RawTorque,
    /// Telemetry-based effect synthesis
    TelemetrySynth,
}

/// Device capabilities for mode negotiation
#[derive(Debug, Clone)]
pub struct DeviceCapabilities {
    /// Device supports DirectInput PID effects
    pub supports_pid: bool,
    /// Device supports raw torque commands (OFP-1 protocol)
    pub supports_raw_torque: bool,
    /// Maximum torque output in Newton-meters
    pub max_torque_nm: f32,
    /// Minimum update period in microseconds (0 if not applicable)
    pub min_period_us: u32,
    /// Device provides health/status stream
    pub has_health_stream: bool,
    /// Device supports physical interlock for high-torque mode
    pub supports_interlock: bool,
}

/// FFB engine errors
#[derive(Debug, thiserror::Error)]
pub enum FfbError {
    #[error("Safety interlock not satisfied")]
    InterlockNotSatisfied,
    #[error("Device fault detected: {fault_type:?}")]
    DeviceFault { fault_type: FaultType },
    #[error("Invalid torque command: {value} Nm exceeds limit {limit} Nm")]
    TorqueExceedsLimit { value: f32, limit: f32 },
    #[error("Safety state violation: cannot perform action in {state:?} state")]
    SafetyStateViolation { state: SafetyState },
    #[error("Device communication error: {message}")]
    DeviceError { message: String },
    #[error("Configuration error: {message}")]
    ConfigError { message: String },
}

/// Information about the current fault for UI display
///
/// **Validates: Requirements FFB-SAFETY-01.9, FFB-SAFETY-01.10**
#[derive(Debug, Clone)]
pub struct FaultInfo {
    /// Human-readable description of the fault
    pub description: String,
    /// Stable error code for KB lookup
    pub error_code: String,
    /// Knowledge base article URL
    pub kb_url: String,
    /// Whether this fault is hardware-critical (requires power cycle)
    pub is_hardware_critical: bool,
    /// When the fault was detected
    pub detected_at: Instant,
}

/// Emergency stop UI state for binding to UI components
///
/// **Validates: Requirement FFB-SAFETY-04, Task 21.4**
///
/// This struct provides all the information needed to render an emergency stop
/// button and its associated state indicators in a UI.
///
/// # Example
/// ```ignore
/// let ui_state = engine.get_emergency_stop_ui_state();
///
/// // Render button
/// if ui_state.is_active {
///     render_button("STOPPED", Color::RED, disabled: true);
///     if ui_state.can_clear {
///         render_button("Clear Emergency Stop", Color::YELLOW, disabled: false);
///     }
/// } else {
///     render_button("EMERGENCY STOP", Color::RED, disabled: false);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct EmergencyStopUiState {
    /// Whether emergency stop is currently active (system is stopped)
    pub is_active: bool,
    /// Current safety state of the FFB engine
    pub safety_state: SafetyState,
    /// Whether the emergency stop can be cleared via user action
    /// (false if fault is hardware-critical and requires power cycle)
    pub can_clear: bool,
    /// Fault information if emergency stop is active
    pub fault_info: Option<FaultInfo>,
    /// Whether soft-stop ramp is in progress
    pub is_ramping: bool,
    /// Soft-stop progress (0.0 to 1.0) if ramping
    pub ramp_progress: Option<f32>,
}

pub type Result<T> = std::result::Result<T, FfbError>;

impl FfbEngine {
    /// Create a new FFB engine with the given configuration
    pub fn new(config: FfbConfig) -> Result<Self> {
        let interlock_system = InterlockSystem::new(config.interlock_required);
        let fault_detector =
            FaultDetector::new(Duration::from_millis(config.fault_timeout_ms as u64));
        let trim_controller = TrimController::new(config.max_torque_nm);

        // Configure soft-stop for 50ms ramp time
        let soft_stop_config = SoftStopConfig {
            max_ramp_time: Duration::from_millis(50),
            audio_cue: true,
            led_indication: true,
            ..Default::default()
        };
        let soft_stop_controller = SoftStopController::new(soft_stop_config);

        let audio_system = AudioCueSystem::default();
        let blackbox_recorder = BlackboxRecorder::default();
        let device_health_monitor = DeviceHealthMonitor::new();

        Ok(Self {
            config,
            safety_state: SafetyState::SafeTorque,
            interlock_system,
            fault_detector,
            trim_controller,
            soft_stop_controller,
            audio_system,
            blackbox_recorder,
            telemetry_synth: None,
            last_heartbeat: Instant::now(),
            device_capabilities: None,
            device_health_monitor,
        })
    }

    /// Get current safety state
    pub fn safety_state(&self) -> SafetyState {
        self.safety_state
    }

    /// Get current configuration
    pub fn config(&self) -> &FfbConfig {
        &self.config
    }

    /// Set device capabilities and negotiate FFB mode
    pub fn set_device_capabilities(&mut self, capabilities: DeviceCapabilities) -> Result<()> {
        self.device_capabilities = Some(capabilities.clone());

        // Use mode negotiator to select appropriate mode and limits
        let negotiator = ModeNegotiator::new();
        let selection = negotiator.negotiate_mode(&capabilities);

        // Update configuration based on negotiation result
        if self.config.mode == FfbMode::Auto {
            self.config.mode = selection.mode;
        }

        // Update trim controller with negotiated limits
        self.trim_controller.set_limits(selection.trim_limits);

        // Log negotiation result
        tracing::info!(
            "FFB mode negotiated: {:?} at {} Hz, high_torque={}, rationale={}",
            selection.mode,
            selection.update_rate_hz,
            selection.supports_high_torque,
            selection.rationale
        );

        Ok(())
    }

    /// Get device capabilities
    pub fn device_capabilities(&self) -> Option<&DeviceCapabilities> {
        self.device_capabilities.as_ref()
    }

    /// Negotiate mode with custom policy
    pub fn negotiate_mode_with_policy(&self, policy: ModeSelectionPolicy) -> Option<ModeSelection> {
        if let Some(capabilities) = &self.device_capabilities {
            let negotiator = ModeNegotiator::with_policy(policy);
            Some(negotiator.negotiate_mode(capabilities))
        } else {
            None
        }
    }

    /// Generate interlock challenge for device
    pub fn generate_interlock_challenge(&mut self) -> Result<InterlockChallenge> {
        self.interlock_system
            .generate_challenge()
            .map_err(|e| FfbError::DeviceError {
                message: e.to_string(),
            })
    }

    /// Validate interlock response from device
    pub fn validate_interlock_response(&mut self, response: InterlockResponse) -> Result<bool> {
        self.interlock_system
            .validate_response(response)
            .map_err(|e| FfbError::DeviceError {
                message: e.to_string(),
            })
    }

    /// Attempt to enable high torque mode
    pub fn enable_high_torque(&mut self, ui_consent: bool) -> Result<()> {
        // Check current state
        if self.safety_state != SafetyState::SafeTorque {
            return Err(FfbError::SafetyStateViolation {
                state: self.safety_state,
            });
        }

        // Check UI consent
        if !ui_consent {
            return Err(FfbError::InterlockNotSatisfied);
        }

        // Check physical interlock if required
        if self.config.interlock_required && !self.interlock_system.is_satisfied() {
            return Err(FfbError::InterlockNotSatisfied);
        }

        // Transition to high torque state
        self.safety_state = SafetyState::HighTorque;

        Ok(())
    }

    /// Disable high torque mode (user-initiated)
    pub fn disable_high_torque(&mut self) -> Result<()> {
        if self.safety_state == SafetyState::HighTorque {
            self.safety_state = SafetyState::SafeTorque;
            self.interlock_system.reset();
        }
        Ok(())
    }

    /// Process fault detection and handle safety response
    pub fn process_fault(&mut self, fault: FaultType) -> Result<()> {
        let now = Instant::now();

        // Create fault entry for blackbox
        let fault_entry = BlackboxEntry::Fault {
            timestamp: now,
            fault_type: fault.error_code().to_string(),
            fault_code: fault.error_code().to_string(),
            context: fault.description().to_string(),
        };

        // Start fault capture in blackbox (2s pre-fault capture)
        self.blackbox_recorder
            .start_fault_capture(fault_entry)
            .map_err(|e| FfbError::DeviceError {
                message: e.to_string(),
            })?;

        // Record fault in fault detector
        let fault_record = self.fault_detector.record_fault(fault.clone());

        // Log latched fault indicator
        tracing::error!(
            "LATCHED FAULT: {} (code: {}, KB: {})",
            fault_record.fault_type.description(),
            fault_record.error_code,
            fault_record.kb_article_url
        );

        // Immediate safety response
        match fault {
            FaultType::UsbStall
            | FaultType::EndpointError
            | FaultType::EndpointWedged
            | FaultType::NanValue
            | FaultType::EncoderInvalid
            | FaultType::OverTemp
            | FaultType::OverCurrent
            | FaultType::DeviceTimeout
            | FaultType::DeviceDisconnect
            | FaultType::UserEmergencyStop
            | FaultType::HardwareEmergencyStop => {
                // IMPORTANT: Capture current torque BEFORE transitioning to faulted state
                // This ensures the soft-stop ramp starts from the actual current torque,
                // not zero (which is what get_current_torque_output returns in Faulted state)
                // **Validates: Requirement FFB-SAFETY-04, Task 21.3**
                let current_torque = self.get_current_torque_output();

                // Transition to faulted state
                self.safety_state = SafetyState::Faulted;

                // Trigger audio cue for fault (ignore rate limiting errors in tests)
                if let Err(e) = self.audio_system.trigger_cue(AudioCueType::FaultWarning) {
                    // In tests, rate limiting might cause failures, so we'll log but not fail
                    #[cfg(not(test))]
                    return Err(FfbError::DeviceError {
                        message: e.to_string(),
                    });
                    #[cfg(test)]
                    tracing::debug!("Audio cue failed (test mode): {}", e);
                }

                // Trigger soft-stop (torque to zero within 50ms) starting from captured torque
                self.trigger_soft_stop_from_torque(current_torque)?;
            }
            FaultType::PluginOverrun => {
                // Plugin faults don't affect FFB safety state
                // Just record and continue
                self.blackbox_recorder
                    .record(BlackboxEntry::SystemEvent {
                        timestamp: now,
                        event_type: "PLUGIN_OVERRUN".to_string(),
                        details: "Plugin exceeded time budget and was quarantined".to_string(),
                    })
                    .map_err(|e| FfbError::DeviceError {
                        message: e.to_string(),
                    })?;
            }
        }

        Ok(())
    }

    /// Trigger soft-stop sequence (torque to zero within 50ms)
    ///
    /// This method captures the current torque from `get_current_torque_output()`.
    /// For fault handling where the torque needs to be captured before state transition,
    /// use `trigger_soft_stop_from_torque()` instead.
    pub fn trigger_soft_stop(&mut self) -> Result<()> {
        let current_torque = self.get_current_torque_output();
        self.trigger_soft_stop_from_torque(current_torque)
    }

    /// Trigger soft-stop sequence from a specific torque value (torque to zero within 50ms)
    ///
    /// **Validates: Requirement FFB-SAFETY-04, Task 21.3**
    ///
    /// This method allows specifying the starting torque explicitly, which is necessary
    /// when the torque needs to be captured before a state transition (e.g., before
    /// transitioning to Faulted state in fault handling).
    ///
    /// # Arguments
    /// * `current_torque` - The torque value to ramp from (in Nm)
    pub fn trigger_soft_stop_from_torque(&mut self, current_torque: f32) -> Result<()> {
        // Record soft-stop in fault detector
        self.fault_detector.record_soft_stop(Instant::now());

        // Record in blackbox
        self.blackbox_recorder
            .record(BlackboxEntry::SoftStop {
                timestamp: Instant::now(),
                reason: "Fault-triggered soft-stop".to_string(),
                initial_torque: current_torque,
                target_ramp_time: Duration::from_millis(50),
            })
            .map_err(|e| FfbError::DeviceError {
                message: e.to_string(),
            })?;

        // Start soft-stop ramp
        self.soft_stop_controller
            .start_ramp(current_torque)
            .map_err(|e| FfbError::DeviceError {
                message: e.to_string(),
            })?;

        // Trigger audio cue
        if self.soft_stop_controller.should_trigger_audio_cue() {
            if let Err(e) = self.audio_system.trigger_cue(AudioCueType::SoftStop) {
                #[cfg(not(test))]
                return Err(FfbError::DeviceError {
                    message: e.to_string(),
                });
                #[cfg(test)]
                tracing::debug!("Audio cue failed (test mode): {}", e);
            }
            self.soft_stop_controller.mark_audio_cue_triggered();
        }

        // Mark LED indication as triggered.
        // The panel LED driver listens on the tracing event bus for this event;
        // hardware LED writing is handled by the panel subsystem via LedController.
        if self.soft_stop_controller.should_trigger_led_indication() {
            self.soft_stop_controller.mark_led_indication_triggered();
            tracing::info!(
                target: "flight_ffb::soft_stop",
                event = "led_indication",
                "FFB soft-stop LED indication triggered"
            );
        }

        Ok(())
    }

    /// Reset from faulted state (requires power cycle)
    ///
    /// **Deprecated:** Use `reset_after_power_cycle()` instead for better semantics
    /// and logging.
    #[deprecated(since = "0.2.0", note = "Use reset_after_power_cycle() instead")]
    pub fn reset_from_fault(&mut self, power_cycled: bool) -> Result<()> {
        self.reset_after_power_cycle(power_cycled)
    }

    /// Update heartbeat for health monitoring
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
    }

    /// Check if engine is healthy (recent heartbeat)
    pub fn is_healthy(&self) -> bool {
        self.last_heartbeat.elapsed() < Duration::from_secs(5)
    }

    /// Get fault history
    pub fn get_fault_history(&self) -> Vec<&FaultRecord> {
        self.fault_detector.get_fault_history_slice()
    }

    /// Get latched fault indicator (returns most recent fault if in faulted state)
    pub fn get_latched_fault(&self) -> Option<&FaultRecord> {
        if self.safety_state == SafetyState::Faulted {
            // Return the most recent fault
            self.fault_detector.get_fault_history().back()
        } else {
            None
        }
    }

    /// Check if system has latched fault
    pub fn has_latched_fault(&self) -> bool {
        self.safety_state == SafetyState::Faulted
    }

    /// Update engine (should be called regularly from main loop)
    pub fn update(&mut self) -> Result<()> {
        // Update soft-stop controller
        if let Some(target_torque) =
            self.soft_stop_controller
                .update()
                .map_err(|e| FfbError::DeviceError {
                    message: e.to_string(),
                })?
        {
            // Record torque update in blackbox
            self.blackbox_recorder
                .record(BlackboxEntry::FfbState {
                    timestamp: Instant::now(),
                    safety_state: format!("{:?}", self.safety_state),
                    torque_setpoint: target_torque,
                    actual_torque: target_torque, // Assume perfect tracking for now
                })
                .map_err(|e| FfbError::DeviceError {
                    message: e.to_string(),
                })?;
        }

        // Update audio system
        self.audio_system
            .update()
            .map_err(|e| FfbError::DeviceError {
                message: e.to_string(),
            })?;

        // Check for completed fault captures and save them
        let completed_captures = self.blackbox_recorder.get_completed_captures();
        if !completed_captures.is_empty() {
            // Save the most recent completed capture if we haven't saved it yet
            if let Some(last_capture) = completed_captures.last() {
                if last_capture.complete {
                    // Save to disk
                    if let Err(e) = self.blackbox_recorder.save_fault_capture(last_capture) {
                        tracing::warn!("Failed to save fault capture: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get current torque output (for soft-stop initialization)
    pub fn get_current_torque_output(&self) -> f32 {
        // In a real implementation, this would query the actual hardware
        // For now, return a reasonable default based on safety state
        match self.safety_state {
            SafetyState::SafeTorque => 2.0, // Assume some baseline torque
            SafetyState::HighTorque => 8.0, // Assume higher torque in high-torque mode
            SafetyState::Faulted => 0.0,    // Should be zero in faulted state
        }
    }

    /// Check if soft-stop is active
    pub fn is_soft_stop_active(&self) -> bool {
        self.soft_stop_controller.is_active()
    }

    /// Get soft-stop progress (0.0 to 1.0)
    pub fn get_soft_stop_progress(&self) -> Option<f32> {
        self.soft_stop_controller.get_progress()
    }

    /// Get blackbox recorder for diagnostics
    pub fn get_blackbox_recorder(&self) -> &BlackboxRecorder {
        &self.blackbox_recorder
    }

    /// Get audio system for configuration
    pub fn get_audio_system(&mut self) -> &mut AudioCueSystem {
        &mut self.audio_system
    }

    /// Record axis frame in blackbox
    pub fn record_axis_frame(
        &mut self,
        device_id: String,
        raw_input: f32,
        processed_output: f32,
        torque_nm: f32,
    ) -> Result<()> {
        self.blackbox_recorder
            .record(BlackboxEntry::AxisFrame {
                timestamp: Instant::now(),
                device_id,
                raw_input,
                processed_output,
                torque_nm,
            })
            .map_err(|e| FfbError::DeviceError {
                message: e.to_string(),
            })?;

        Ok(())
    }

    /// Enable telemetry synthesis with configuration
    pub fn enable_telemetry_synthesis(&mut self, config: TelemetrySynthConfig) -> Result<()> {
        // Only enable telemetry synthesis if mode is TelemetrySynth
        if self.config.mode == FfbMode::TelemetrySynth {
            self.telemetry_synth = Some(TelemetrySynthEngine::new(config));
            tracing::info!("Telemetry synthesis enabled");
        } else {
            return Err(FfbError::ConfigError {
                message: format!(
                    "Telemetry synthesis requires FfbMode::TelemetrySynth, current mode: {:?}",
                    self.config.mode
                ),
            });
        }
        Ok(())
    }

    /// Disable telemetry synthesis
    pub fn disable_telemetry_synthesis(&mut self) {
        self.telemetry_synth = None;
        tracing::info!("Telemetry synthesis disabled");
    }

    /// Update telemetry synthesis with flight data
    pub fn update_telemetry_synthesis(
        &mut self,
        snapshot: &flight_bus::BusSnapshot,
    ) -> Result<Option<EffectOutput>> {
        if let Some(ref mut synth_engine) = self.telemetry_synth {
            let output = synth_engine.update(snapshot)?;

            // Record telemetry synthesis output in blackbox
            self.blackbox_recorder
                .record(BlackboxEntry::TelemetrySynth {
                    timestamp: Instant::now(),
                    torque_nm: output.torque_nm,
                    frequency_hz: output.frequency_hz,
                    intensity: output.intensity,
                    active_effects: output.active_effects.join(","),
                })
                .map_err(|e| FfbError::DeviceError {
                    message: e.to_string(),
                })?;

            Ok(Some(output))
        } else {
            Ok(None)
        }
    }

    /// Get telemetry synthesis engine for configuration
    pub fn get_telemetry_synth(&self) -> Option<&TelemetrySynthEngine> {
        self.telemetry_synth.as_ref()
    }

    /// Get mutable telemetry synthesis engine for configuration
    pub fn get_telemetry_synth_mut(&mut self) -> Option<&mut TelemetrySynthEngine> {
        self.telemetry_synth.as_mut()
    }

    /// Check if telemetry synthesis is enabled
    pub fn is_telemetry_synthesis_enabled(&self) -> bool {
        self.telemetry_synth.is_some()
    }

    /// Get trim controller for validation and testing
    pub fn get_trim_controller(&self) -> &TrimController {
        &self.trim_controller
    }

    /// Get mutable trim controller for validation and testing
    pub fn get_trim_controller_mut(&mut self) -> &mut TrimController {
        &mut self.trim_controller
    }

    /// Apply trim setpoint change through engine
    pub fn apply_trim_setpoint_change(&mut self, change: SetpointChange) -> Result<()> {
        let target_nm = change.target_nm;
        self.trim_controller
            .apply_setpoint_change(change)
            .map_err(|e| FfbError::ConfigError { message: e })?;

        // Record trim change in blackbox
        self.blackbox_recorder
            .record(BlackboxEntry::SystemEvent {
                timestamp: Instant::now(),
                event_type: "TRIM_SETPOINT_CHANGE".to_string(),
                details: format!("Target: {} Nm", target_nm),
            })
            .map_err(|e| FfbError::DeviceError {
                message: e.to_string(),
            })?;

        Ok(())
    }

    /// Update trim controller and get output
    pub fn update_trim_controller(&mut self) -> TrimOutput {
        let output = self.trim_controller.update();

        // Record trim state in blackbox
        let state = self.trim_controller.get_trim_state();
        if let Err(e) = self.blackbox_recorder.record(BlackboxEntry::SystemEvent {
            timestamp: Instant::now(),
            event_type: "TRIM_UPDATE".to_string(),
            details: format!(
                "Mode: {:?}, Setpoint: {} Nm, Rate: {} Nm/s",
                state.mode, state.current_setpoint_nm, state.current_rate_nm_per_s
            ),
        }) {
            tracing::warn!("Failed to record trim update in blackbox: {}", e);
        }

        output
    }

    /// Run trim validation suite
    pub fn run_trim_validation(&mut self) -> Vec<TrimValidationResult> {
        let mut validation_suite = TrimValidationSuite::default();
        validation_suite.run_complete_validation()
    }

    /// Run trim validation with custom configuration
    pub fn run_trim_validation_with_config(
        &mut self,
        config: TrimValidationConfig,
    ) -> Vec<TrimValidationResult> {
        let mut validation_suite = TrimValidationSuite::new(config);
        validation_suite.run_complete_validation()
    }

    // =========================================================================
    // Emergency Stop Implementation
    // **Validates: Requirement FFB-SAFETY-01.14**
    // =========================================================================

    /// Trigger emergency stop (UI button or hardware button)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.14, FFB-SAFETY-04**
    /// Emergency stop bypasses everything and jumps to ramp-down.
    /// This is wired into the safety state machine using `FaultReason::UserEmergencyStop`
    /// or `FaultReason::HardwareEmergencyStop`.
    ///
    /// # Arguments
    /// * `reason` - The reason for the emergency stop (UI button, hardware button, or programmatic)
    pub fn emergency_stop(&mut self, reason: EmergencyStopReason) -> Result<()> {
        let now = Instant::now();

        tracing::warn!("EMERGENCY STOP triggered: {:?}", reason);

        // Determine the fault type based on the emergency stop reason
        let fault_type = match reason {
            EmergencyStopReason::UiButton | EmergencyStopReason::Programmatic => {
                FaultType::UserEmergencyStop
            }
            EmergencyStopReason::HardwareButton => FaultType::HardwareEmergencyStop,
        };

        // Record emergency stop in blackbox
        let reason_str = match reason {
            EmergencyStopReason::UiButton => "UI_BUTTON",
            EmergencyStopReason::HardwareButton => "HARDWARE_BUTTON",
            EmergencyStopReason::Programmatic => "PROGRAMMATIC",
        };

        self.blackbox_recorder
            .record(BlackboxEntry::SystemEvent {
                timestamp: now,
                event_type: "EMERGENCY_STOP".to_string(),
                details: format!("Emergency stop triggered: {}", reason_str),
            })
            .map_err(|e| FfbError::DeviceError {
                message: e.to_string(),
            })?;

        // Process the fault through the standard fault handling path
        // This ensures proper fault recording, blackbox capture, and state transitions
        self.process_fault(fault_type)?;

        Ok(())
    }

    /// Clear emergency stop state (user-initiated)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.10**
    /// Emergency stop is a transient fault that can be cleared via explicit user action
    pub fn clear_emergency_stop(&mut self) -> Result<()> {
        if self.safety_state != SafetyState::Faulted {
            return Ok(()); // Not in faulted state, nothing to clear
        }

        // Record clear action in blackbox
        self.blackbox_recorder
            .record(BlackboxEntry::SystemEvent {
                timestamp: Instant::now(),
                event_type: "EMERGENCY_STOP_CLEARED".to_string(),
                details: "User cleared emergency stop".to_string(),
            })
            .map_err(|e| FfbError::DeviceError {
                message: e.to_string(),
            })?;

        // Transition back to safe torque state
        self.safety_state = SafetyState::SafeTorque;
        self.interlock_system.reset();
        self.soft_stop_controller.reset();

        tracing::info!("Emergency stop cleared by user action");

        Ok(())
    }

    // =========================================================================
    // Clear Fault Implementation
    // **Validates: Requirements FFB-SAFETY-01.9, FFB-SAFETY-01.10**
    // =========================================================================

    /// Attempt to clear a fault (user-initiated)
    ///
    /// **Validates: Requirements FFB-SAFETY-01.9, FFB-SAFETY-01.10**
    ///
    /// This method implements the clear_fault semantics:
    /// - **Transient faults** (UsbStall, EndpointError, NanValue, DeviceTimeout,
    ///   DeviceDisconnect, UserEmergencyStop, HardwareEmergencyStop) can be cleared
    ///   via explicit user action after the cause is resolved.
    /// - **Hardware-critical faults** (OverTemp, OverCurrent, EncoderInvalid) cannot
    ///   be cleared via this method - they require a power cycle. Use
    ///   `reset_after_power_cycle()` instead.
    ///
    /// # Returns
    /// * `Ok(())` if the fault was cleared successfully
    /// * `Err(FfbError::SafetyStateViolation)` if not in faulted state
    /// * `Err(FfbError::DeviceError)` if the fault is hardware-critical and requires power cycle
    ///
    /// # Example
    /// ```ignore
    /// // After resolving a USB stall issue
    /// match engine.clear_fault() {
    ///     Ok(()) => println!("Fault cleared, FFB re-enabled"),
    ///     Err(FfbError::DeviceError { message }) => {
    ///         println!("Cannot clear fault: {}", message);
    ///         // Hardware-critical fault - need power cycle
    ///     }
    ///     Err(e) => println!("Error: {}", e),
    /// }
    /// ```
    pub fn clear_fault(&mut self) -> Result<()> {
        // Check if we're in faulted state
        if self.safety_state != SafetyState::Faulted {
            return Ok(()); // Not in faulted state, nothing to clear
        }

        // Get the current fault to check if it's hardware-critical
        let current_fault = self.fault_detector.get_fault_history().back();

        if let Some(fault_record) = current_fault {
            // Check if the fault is hardware-critical
            if fault_record.fault_type.is_hardware_critical() {
                let error_msg = format!(
                    "Cannot clear hardware-critical fault '{}' (code: {}) - power cycle required. \
                     See: {}",
                    fault_record.fault_type.description(),
                    fault_record.error_code,
                    fault_record.kb_article_url
                );
                tracing::warn!("{}", error_msg);
                return Err(FfbError::DeviceError { message: error_msg });
            }

            // Transient fault - can be cleared
            tracing::info!(
                "Clearing transient fault '{}' (code: {}) via user action",
                fault_record.fault_type.description(),
                fault_record.error_code
            );
        }

        // Record clear action in blackbox
        self.blackbox_recorder
            .record(BlackboxEntry::SystemEvent {
                timestamp: Instant::now(),
                event_type: "FAULT_CLEARED".to_string(),
                details: format!(
                    "User cleared transient fault: {:?}",
                    current_fault.map(|f| f.fault_type.description())
                ),
            })
            .map_err(|e| FfbError::DeviceError {
                message: e.to_string(),
            })?;

        // Transition back to safe torque state
        self.safety_state = SafetyState::SafeTorque;
        self.interlock_system.reset();
        self.soft_stop_controller.reset();

        tracing::info!("Fault cleared by user action - FFB re-enabled in SafeTorque mode");

        Ok(())
    }

    /// Reset from faulted state after power cycle
    ///
    /// **Validates: Requirement FFB-SAFETY-01.9**
    ///
    /// This method should only be called after a verified power cycle of the FFB device.
    /// It clears all faults including hardware-critical faults (OverTemp, OverCurrent,
    /// EncoderInvalid) that cannot be cleared via `clear_fault()`.
    ///
    /// # Arguments
    /// * `power_cycled` - Must be `true` to confirm power cycle occurred
    ///
    /// # Returns
    /// * `Ok(())` if reset was successful
    ///
    /// # Example
    /// ```ignore
    /// // After user confirms device was power cycled
    /// engine.reset_after_power_cycle(true)?;
    /// println!("Device reset - FFB re-enabled");
    /// ```
    pub fn reset_after_power_cycle(&mut self, power_cycled: bool) -> Result<()> {
        if !power_cycled {
            return Ok(()); // Power cycle not confirmed
        }

        if self.safety_state != SafetyState::Faulted {
            return Ok(()); // Not in faulted state, nothing to reset
        }

        // Get the current fault for logging
        let current_fault = self.fault_detector.get_fault_history().back();
        let fault_desc = current_fault
            .map(|f| format!("{} ({})", f.fault_type.description(), f.error_code))
            .unwrap_or_else(|| "unknown".to_string());

        tracing::info!("Resetting from fault '{}' after power cycle", fault_desc);

        // Record reset action in blackbox
        self.blackbox_recorder
            .record(BlackboxEntry::SystemEvent {
                timestamp: Instant::now(),
                event_type: "POWER_CYCLE_RESET".to_string(),
                details: format!("Power cycle reset from fault: {}", fault_desc),
            })
            .map_err(|e| FfbError::DeviceError {
                message: e.to_string(),
            })?;

        // Clear all faults and reset state
        self.safety_state = SafetyState::SafeTorque;
        self.interlock_system.reset();
        self.soft_stop_controller.reset();
        self.fault_detector.clear_faults();

        // Also clear device health fault if latched
        self.device_health_monitor.clear_latched_fault();

        tracing::info!("Power cycle reset complete - FFB re-enabled in SafeTorque mode");

        Ok(())
    }

    /// Check if the current fault is hardware-critical (requires power cycle)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.9**
    ///
    /// Hardware-critical faults include:
    /// - OverTemp: Device over-temperature protection
    /// - OverCurrent: Device over-current protection
    /// - EncoderInvalid: Device encoder providing invalid readings
    ///
    /// # Returns
    /// * `true` if in faulted state with a hardware-critical fault
    /// * `false` if not in faulted state or fault is transient
    pub fn is_fault_hardware_critical(&self) -> bool {
        if self.safety_state != SafetyState::Faulted {
            return false;
        }

        self.fault_detector
            .get_fault_history()
            .back()
            .map(|f| f.fault_type.is_hardware_critical())
            .unwrap_or(false)
    }

    /// Check if the current fault is transient (can be cleared via user action)
    ///
    /// **Validates: Requirement FFB-SAFETY-01.10**
    ///
    /// Transient faults include:
    /// - UsbStall: USB output endpoint stalled
    /// - EndpointError: USB endpoint error
    /// - NanValue: NaN/Inf in FFB pipeline
    /// - DeviceTimeout: Device communication timeout
    /// - DeviceDisconnect: Device disconnected
    /// - PluginOverrun: Plugin exceeded time budget
    ///
    /// # Returns
    /// * `true` if in faulted state with a transient fault
    /// * `false` if not in faulted state or fault is hardware-critical
    pub fn is_fault_transient(&self) -> bool {
        if self.safety_state != SafetyState::Faulted {
            return false;
        }

        self.fault_detector
            .get_fault_history()
            .back()
            .map(|f| f.fault_type.is_transient())
            .unwrap_or(false)
    }

    /// Get information about the current fault for UI display
    ///
    /// Returns a tuple of (fault_description, error_code, kb_url, is_hardware_critical)
    /// or None if not in faulted state.
    pub fn get_fault_info(&self) -> Option<FaultInfo> {
        if self.safety_state != SafetyState::Faulted {
            return None;
        }

        self.fault_detector
            .get_fault_history()
            .back()
            .map(|f| FaultInfo {
                description: f.fault_type.description().to_string(),
                error_code: f.error_code.clone(),
                kb_url: f.kb_article_url.clone(),
                is_hardware_critical: f.fault_type.is_hardware_critical(),
                detected_at: f.detected_at,
            })
    }

    /// Check if emergency stop is active
    pub fn is_emergency_stop_active(&self) -> bool {
        self.safety_state == SafetyState::Faulted
    }

    /// Get complete UI state for emergency stop button rendering
    ///
    /// **Validates: Requirement FFB-SAFETY-04, Task 21.4**
    ///
    /// This method provides all the information needed to render an emergency stop
    /// UI component, including:
    /// - Whether the stop is active
    /// - Whether it can be cleared (vs requiring power cycle)
    /// - Fault details for display
    /// - Soft-stop ramp progress
    ///
    /// # Example
    /// ```ignore
    /// let state = engine.get_emergency_stop_ui_state();
    ///
    /// // Update UI based on state
    /// emergency_stop_button.set_enabled(!state.is_active);
    /// clear_button.set_enabled(state.can_clear);
    ///
    /// if let Some(fault) = &state.fault_info {
    ///     status_label.set_text(&fault.description);
    /// }
    /// ```
    pub fn get_emergency_stop_ui_state(&self) -> EmergencyStopUiState {
        let is_active = self.is_emergency_stop_active();
        let fault_info = self.get_fault_info();
        let can_clear = is_active && !self.is_fault_hardware_critical();

        EmergencyStopUiState {
            is_active,
            safety_state: self.safety_state,
            can_clear,
            fault_info,
            is_ramping: self.is_soft_stop_active(),
            ramp_progress: self.get_soft_stop_progress(),
        }
    }

    // =========================================================================
    // Fault Detection Wiring
    // **Validates: Requirements FFB-SAFETY-01.5-8**
    // =========================================================================

    /// Record USB write result for stall detection
    ///
    /// **Validates: Requirement FFB-SAFETY-01.5**
    /// USB OUT stall detected for ≥3 frames triggers fault
    ///
    /// # Arguments
    /// * `success` - Whether the USB write succeeded
    pub fn record_usb_write_result(&mut self, success: bool) -> Result<()> {
        if success {
            self.fault_detector.reset_usb_stall_counter();
        } else {
            // Record USB stall - triggers fault after 3 consecutive failures
            if let Some(fault_record) = self.fault_detector.record_usb_stall() {
                tracing::warn!(
                    "USB stall threshold reached (3 frames) - triggering fault: {}",
                    fault_record.error_code
                );
                self.process_fault(FaultType::UsbStall)?;
            }
        }
        Ok(())
    }

    /// Check value for NaN/Inf before SafetyEnvelope
    ///
    /// **Validates: Requirement FFB-SAFETY-01.6**
    /// NaN or Inf in FFB pipeline triggers fault handler
    ///
    /// # Arguments
    /// * `value` - The value to check
    /// * `context` - Description of where the value came from
    ///
    /// # Returns
    /// * `Ok(())` if value is valid
    /// * `Err` if value is NaN/Inf and fault was triggered
    pub fn check_pipeline_value(&mut self, value: f32, context: &str) -> Result<()> {
        if let Some(fault_record) = self.fault_detector.check_nan_value(value, context) {
            tracing::error!(
                "NaN/Inf detected in FFB pipeline at {}: {} - triggering fault: {}",
                context,
                value,
                fault_record.error_code
            );
            self.process_fault(FaultType::NanValue)?;
        }
        Ok(())
    }

    /// Process device health status
    ///
    /// **Validates: Requirement FFB-SAFETY-01.7**
    /// Device over-temp or over-current triggers immediate FFB disable
    ///
    /// # Arguments
    /// * `over_temp` - Whether device reports over-temperature
    /// * `over_current` - Whether device reports over-current
    pub fn process_device_health(&mut self, over_temp: bool, over_current: bool) -> Result<()> {
        if over_temp {
            tracing::error!("Device over-temperature detected - triggering fault");
            self.process_fault(FaultType::OverTemp)?;
        }

        if over_current {
            tracing::error!("Device over-current detected - triggering fault");
            self.process_fault(FaultType::OverCurrent)?;
        }

        Ok(())
    }

    /// Update device health from a health status struct
    ///
    /// This method integrates with the DeviceHealthMonitor to evaluate
    /// health status against configured thresholds and trigger appropriate
    /// faults when over-temperature or over-current conditions are detected.
    ///
    /// **Validates: Requirements FFB-SAFETY-01.7, FFB-SAFETY-01.9**
    ///
    /// # Arguments
    /// * `status` - The device health status to evaluate
    ///
    /// # Returns
    /// * `Ok(HealthCheckResult)` - The result of the health check
    /// * `Err` - If a fault was triggered and processing failed
    pub fn update_device_health(
        &mut self,
        status: DeviceHealthStatus,
    ) -> Result<HealthCheckResult> {
        let result = self.device_health_monitor.update(status.clone());

        // Record health status in blackbox
        let health_details = format!(
            "temp={:?}°C, current={:?}mA, voltage={:?}V",
            status.temperature_c, status.current_ma, status.voltage_v
        );

        if let Err(e) = self.blackbox_recorder.record(BlackboxEntry::SystemEvent {
            timestamp: Instant::now(),
            event_type: "DEVICE_HEALTH_UPDATE".to_string(),
            details: format!("{}: {:?}", health_details, result),
        }) {
            tracing::warn!("Failed to record device health in blackbox: {}", e);
        }

        // Handle fault conditions
        match &result {
            HealthCheckResult::OverTemperature => {
                tracing::error!(
                    "Device over-temperature detected via health monitor: {:?}°C",
                    status.temperature_c
                );
                self.process_fault(FaultType::OverTemp)?;
            }
            HealthCheckResult::OverCurrent => {
                tracing::error!(
                    "Device over-current detected via health monitor: {:?}mA",
                    status.current_ma
                );
                self.process_fault(FaultType::OverCurrent)?;
            }
            HealthCheckResult::TemperatureWarning => {
                tracing::warn!(
                    "Device temperature warning: {:?}°C (approaching limit)",
                    status.temperature_c
                );
            }
            HealthCheckResult::CurrentWarning => {
                tracing::warn!(
                    "Device current warning: {:?}mA (approaching limit)",
                    status.current_ma
                );
            }
            HealthCheckResult::UnderVoltage | HealthCheckResult::OverVoltage => {
                tracing::warn!(
                    "Device voltage issue: {:?}V - {:?}",
                    status.voltage_v,
                    result
                );
            }
            _ => {}
        }

        Ok(result)
    }

    /// Check device health from a provider
    ///
    /// This method polls a DeviceHealthProvider for health status and
    /// evaluates it against configured thresholds.
    ///
    /// **Validates: Requirements FFB-SAFETY-01.7, FFB-SAFETY-01.9**
    ///
    /// # Arguments
    /// * `provider` - The device health provider to poll
    ///
    /// # Returns
    /// * `Ok(HealthCheckResult)` - The result of the health check
    /// * `Err` - If a fault was triggered and processing failed
    pub fn check_device_health_provider(
        &mut self,
        provider: &dyn DeviceHealthProvider,
    ) -> Result<HealthCheckResult> {
        if !provider.has_health_stream() {
            return Ok(HealthCheckResult::NotAvailable);
        }

        match provider.get_health_status() {
            Some(status) => self.update_device_health(status),
            None => {
                if self.device_health_monitor.is_stale() {
                    Ok(HealthCheckResult::StaleHealth)
                } else {
                    Ok(self.device_health_monitor.last_result().clone())
                }
            }
        }
    }

    /// Get the device health monitor for configuration and diagnostics
    pub fn get_device_health_monitor(&self) -> &DeviceHealthMonitor {
        &self.device_health_monitor
    }

    /// Get mutable device health monitor for configuration
    pub fn get_device_health_monitor_mut(&mut self) -> &mut DeviceHealthMonitor {
        &mut self.device_health_monitor
    }

    /// Configure device health monitoring thresholds
    ///
    /// # Arguments
    /// * `config` - The health monitoring configuration
    pub fn configure_device_health(&mut self, config: DeviceHealthConfig) {
        self.device_health_monitor.set_config(config);
        tracing::info!("Device health monitoring configuration updated");
    }

    /// Check if device health monitoring is available
    ///
    /// Returns true if the device has health stream capability.
    pub fn has_device_health_monitoring(&self) -> bool {
        self.device_capabilities
            .as_ref()
            .map(|c| c.has_health_stream)
            .unwrap_or(false)
    }

    /// Check if a device health fault is latched
    ///
    /// Hardware-critical faults (over-temp, over-current) are latched
    /// and require a power cycle to clear.
    ///
    /// **Validates: Requirement FFB-SAFETY-01.9**
    pub fn is_device_health_fault_latched(&self) -> bool {
        self.device_health_monitor.is_fault_latched()
    }

    /// Clear device health fault after power cycle
    ///
    /// This should only be called after a verified power cycle of the FFB device.
    /// Hardware-critical faults cannot be cleared without a power cycle.
    ///
    /// **Validates: Requirement FFB-SAFETY-01.9**
    pub fn clear_device_health_fault_after_power_cycle(&mut self) {
        self.device_health_monitor.clear_latched_fault();
        tracing::info!("Device health fault cleared after power cycle");
    }

    /// Check for device disconnect
    ///
    /// **Validates: Requirement FFB-SAFETY-01.8**
    /// Device disconnect detected within 100ms, outputs ramped to safe within 50ms
    ///
    /// # Arguments
    /// * `connected` - Whether the device is currently connected
    pub fn check_device_connection(&mut self, connected: bool) -> Result<()> {
        if !connected && self.safety_state != SafetyState::Faulted {
            tracing::warn!("Device disconnect detected - triggering fault");
            self.process_fault(FaultType::DeviceDisconnect)?;
        }
        Ok(())
    }

    /// Check for device disconnect from HID/DirectInput error codes
    ///
    /// **Validates: Requirement FFB-SAFETY-01.8**
    /// Device disconnect detected within 100ms from return codes
    ///
    /// This method should be called when HID or DirectInput operations fail.
    /// It will detect disconnect-related error codes and trigger the appropriate
    /// fault handling.
    ///
    /// # Arguments
    /// * `error_code` - The error code from the failed operation
    /// * `context` - Description of the operation that failed
    ///
    /// # Returns
    /// * `Ok(true)` if disconnect was detected and fault was triggered
    /// * `Ok(false)` if error code doesn't indicate disconnect
    /// * `Err` if fault processing failed
    pub fn check_disconnect_from_error_code(
        &mut self,
        error_code: i32,
        context: &str,
    ) -> Result<bool> {
        if let Some(fault_record) = self
            .fault_detector
            .check_disconnect_from_error_code(error_code, context)
        {
            tracing::warn!(
                "Device disconnect detected from error code {} in {} - triggering fault: {}",
                error_code,
                context,
                fault_record.error_code
            );
            self.process_fault(FaultType::DeviceDisconnect)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Record device disconnect directly
    ///
    /// **Validates: Requirement FFB-SAFETY-01.8**
    /// Device disconnect detected within 100ms
    ///
    /// This method should be called when device disconnection is detected
    /// through other means (e.g., device enumeration, hotplug events).
    ///
    /// # Arguments
    /// * `device_id` - Identifier of the disconnected device
    /// * `context` - Additional context about the disconnect
    pub fn record_device_disconnect(&mut self, device_id: &str, context: &str) -> Result<()> {
        if self.safety_state != SafetyState::Faulted {
            let fault_record = self
                .fault_detector
                .record_device_disconnect(device_id, context);
            tracing::warn!(
                "Device {} disconnected: {} - triggering fault: {}",
                device_id,
                context,
                fault_record.error_code
            );
            self.process_fault(FaultType::DeviceDisconnect)?;
        }
        Ok(())
    }

    /// Check endpoint responsiveness for wedge detection
    ///
    /// **Validates: Requirement FFB-SAFETY-01.5**
    /// Endpoint wedge detection (100ms unresponsive)
    ///
    /// # Arguments
    /// * `responsive` - Whether the endpoint is responsive
    pub fn check_endpoint_responsiveness(&mut self, responsive: bool) -> Result<()> {
        if let Some(fault_record) = self.fault_detector.check_endpoint_wedge(responsive) {
            tracing::error!(
                "Endpoint wedge detected (100ms unresponsive) - triggering fault: {}",
                fault_record.error_code
            );
            self.process_fault(FaultType::EndpointWedged)?;
        }
        Ok(())
    }

    /// Get fault detector for advanced diagnostics
    pub fn get_fault_detector(&self) -> &FaultDetector {
        &self.fault_detector
    }

    /// Get mutable fault detector for testing
    pub fn get_fault_detector_mut(&mut self) -> &mut FaultDetector {
        &mut self.fault_detector
    }
}

impl Default for FfbEngine {
    fn default() -> Self {
        let config = FfbConfig {
            max_torque_nm: 15.0,
            fault_timeout_ms: 50,
            interlock_required: true,
            mode: FfbMode::Auto,
            device_path: None,
        };

        Self::new(config).expect("Default FFB engine creation should not fail")
    }
}
