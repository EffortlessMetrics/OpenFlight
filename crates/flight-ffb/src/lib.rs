//! Force feedback engine with safety-first design
//!
//! This crate provides safe and controlled force feedback operation for Flight Hub.
//! It implements a comprehensive safety state machine, physical interlocks, and multiple
//! FFB modes while maintaining real-time performance.

use std::time::{Duration, Instant};

pub mod safety;
pub mod interlock;
pub mod fault;
pub mod trim;

#[cfg(test)]
mod tests;

pub use safety::*;
pub use interlock::*;
pub use fault::*;
pub use trim::*;

/// Main force feedback engine with safety systems
pub struct FfbEngine {
    config: FfbConfig,
    safety_state: SafetyState,
    interlock_system: InterlockSystem,
    fault_detector: FaultDetector,
    trim_controller: TrimController,
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
    pub supports_pid: bool,
    pub supports_raw_torque: bool,
    pub max_torque_nm: f32,
    pub min_period_us: u32,
    pub has_health_stream: bool,
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
        let fault_detector = FaultDetector::new(Duration::from_millis(config.fault_timeout_ms as u64));
        let trim_controller = TrimController::new(config.max_torque_nm);
        
        Ok(Self {
            config,
            safety_state: SafetyState::SafeTorque,
            interlock_system,
            fault_detector,
            trim_controller,
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

    /// Set device capabilities after negotiation
    pub fn set_device_capabilities(&mut self, capabilities: DeviceCapabilities) -> Result<()> {
        self.device_capabilities = Some(capabilities.clone());
        
        // Auto-select mode based on capabilities if in Auto mode
        if self.config.mode == FfbMode::Auto {
            let selected_mode = if capabilities.supports_raw_torque {
                FfbMode::RawTorque
            } else if capabilities.supports_pid {
                FfbMode::DirectInput
            } else {
                FfbMode::TelemetrySynth
            };
            
            self.config.mode = selected_mode;
        }
        
        Ok(())
    }

    /// Generate interlock challenge for device
    pub fn generate_interlock_challenge(&mut self) -> Result<InterlockChallenge> {
        self.interlock_system.generate_challenge()
            .map_err(|e| FfbError::DeviceError { message: e.to_string() })
    }

    /// Validate interlock response from device
    pub fn validate_interlock_response(&mut self, response: InterlockResponse) -> Result<bool> {
        self.interlock_system.validate_response(response)
            .map_err(|e| FfbError::DeviceError { message: e.to_string() })
    }

    /// Attempt to enable high torque mode
    pub fn enable_high_torque(&mut self, ui_consent: bool) -> Result<()> {
        // Check current state
        if self.safety_state != SafetyState::SafeTorque {
            return Err(FfbError::SafetyStateViolation { 
                state: self.safety_state 
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
        // Record fault
        self.fault_detector.record_fault(fault.clone());
        
        // Immediate safety response
        match fault {
            FaultType::UsbStall | 
            FaultType::EndpointError | 
            FaultType::NanValue | 
            FaultType::OverTemp | 
            FaultType::OverCurrent => {
                // Transition to faulted state
                self.safety_state = SafetyState::Faulted;
                
                // Trigger soft-stop (torque to zero within 50ms)
                self.trigger_soft_stop()?;
            }
            FaultType::PluginOverrun => {
                // Plugin faults don't affect FFB safety state
                // Just record and continue
            }
        }
        
        Ok(())
    }

    /// Trigger soft-stop sequence (torque to zero within 50ms)
    pub fn trigger_soft_stop(&mut self) -> Result<()> {
        // This would interface with the actual hardware to ramp torque to zero
        // For now, we'll just record the action
        self.fault_detector.record_soft_stop(Instant::now());
        
        // In a real implementation, this would:
        // 1. Start torque ramp to zero over 50ms
        // 2. Trigger audio cue
        // 3. Update LED indicators
        // 4. Log the event for diagnostics
        
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
