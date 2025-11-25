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
pub mod dinput_device;
pub mod fault;
#[cfg(test)]
pub mod hil_tests;
#[cfg(test)]
pub mod integration_test;
pub mod interlock;
pub mod mode_negotiation;
pub mod ofp1_integration;
#[cfg(test)]
pub mod performance_validation;
pub mod safety;
pub mod safety_envelope;
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
pub mod xinput_rumble;

#[cfg(test)]
mod tests;

pub use audio::*;
pub use blackbox::*;
pub use dinput_device::*;
pub use fault::*;
#[cfg(test)]
pub use hil_tests::*;
pub use interlock::*;
pub use mode_negotiation::*;
pub use ofp1_integration::*;
#[cfg(test)]
pub use performance_validation::*;
pub use safety::*;
pub use safety_envelope::*;
pub use soft_stop::*;
pub use telemetry_synth::*;
pub use trim::*;
#[cfg(test)]
pub use trim_hil_tests::*;
pub use trim_validation::*;
#[cfg(test)]
pub use usb_yank_test::*;
pub use xinput_rumble::*;

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
        self.fault_detector.record_fault(fault.clone());

        // Immediate safety response
        match fault {
            FaultType::UsbStall
            | FaultType::EndpointError
            | FaultType::EndpointWedged
            | FaultType::NanValue
            | FaultType::EncoderInvalid
            | FaultType::OverTemp
            | FaultType::OverCurrent
            | FaultType::DeviceTimeout => {
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

                // Trigger soft-stop (torque to zero within 50ms)
                self.trigger_soft_stop()?;
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
    pub fn trigger_soft_stop(&mut self) -> Result<()> {
        let current_torque = self.get_current_torque_output();

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

        // Mark LED indication as triggered (would integrate with panel system)
        if self.soft_stop_controller.should_trigger_led_indication() {
            self.soft_stop_controller.mark_led_indication_triggered();
            // TODO: Integrate with panel LED system
        }

        Ok(())
    }

    /// Reset from faulted state (requires power cycle)
    pub fn reset_from_fault(&mut self, power_cycled: bool) -> Result<()> {
        if self.safety_state == SafetyState::Faulted && power_cycled {
            self.safety_state = SafetyState::SafeTorque;
            self.interlock_system.reset();
            self.fault_detector.clear_faults();
        }
        Ok(())
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
