// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Safety state machine for force feedback operation
//!
//! Implements the core safety state machine with three states:
//! - SafeTorque: Normal operation with limited torque
//! - HighTorque: High torque operation with full capabilities
//! - Faulted: Fault detected, torque disabled until power cycle
//!
//! # Fault Handling
//!
//! The safety state machine distinguishes between:
//! - **Hardware-critical faults** (OverTemp, OverCurrent): Require power cycle to re-enable high-torque mode
//! - **Transient faults** (UsbStall, NanValue, etc.): May be cleared via explicit user action after cause is resolved
//!
//! **Validates: Requirements FFB-SAFETY-02, FFB-SAFETY-03**

use std::time::Instant;

/// Reasons for entering the faulted state
///
/// This enum categorizes faults by their source and severity, enabling
/// appropriate recovery actions. Hardware-critical faults require power cycle,
/// while transient faults can be cleared via user action.
///
/// **Validates: Requirements FFB-SAFETY-02, FFB-SAFETY-03**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FaultReason {
    /// USB output endpoint stalled for 3+ consecutive frames
    ///
    /// **Category:** Transient - can be cleared after USB recovery
    /// **Response:** 50ms ramp to zero, audio cue
    UsbStall,

    /// USB endpoint error or wedged state
    ///
    /// **Category:** Transient - can be cleared after USB recovery
    /// **Response:** 50ms ramp to zero, audio cue
    EndpointError,

    /// NaN or Inf value detected in FFB pipeline
    ///
    /// **Category:** Transient - can be cleared after source is fixed
    /// **Response:** 50ms ramp to zero, audio cue
    NanInPipeline,

    /// Device over-temperature protection triggered
    ///
    /// **Category:** Hardware-critical - requires power cycle
    /// **Response:** Immediate disable, latch fault state
    OverTemp,

    /// Device over-current protection triggered
    ///
    /// **Category:** Hardware-critical - requires power cycle
    /// **Response:** Immediate disable, latch fault state
    OverCurrent,

    /// Device encoder providing invalid readings
    ///
    /// **Category:** Hardware-critical - requires power cycle
    /// **Response:** 50ms ramp to zero, latch fault state
    EncoderInvalid,

    /// Device communication timeout
    ///
    /// **Category:** Transient - can be cleared after reconnection
    /// **Response:** 50ms ramp to zero, audio cue
    DeviceTimeout,

    /// Device disconnected unexpectedly
    ///
    /// **Category:** Transient - can be cleared after reconnection
    /// **Response:** 50ms ramp to zero (if possible), audio cue
    DeviceDisconnect,

    /// User-initiated emergency stop (UI button)
    ///
    /// **Category:** Transient - can be cleared via user action
    /// **Response:** Immediate 50ms ramp to zero, audio cue
    UserEmergencyStop,

    /// Hardware emergency stop button pressed
    ///
    /// **Category:** Transient - can be cleared via user action
    /// **Response:** Immediate 50ms ramp to zero, audio cue
    HardwareEmergencyStop,

    /// Plugin exceeded time budget
    ///
    /// **Category:** Non-critical - does not affect FFB safety state
    /// **Response:** Quarantine plugin, log warning
    PluginOverrun,
}

impl FaultReason {
    /// Check if this fault is hardware-critical (requires power cycle)
    ///
    /// Hardware-critical faults indicate potential hardware damage or malfunction
    /// that cannot be safely recovered from without a full power cycle.
    ///
    /// **Validates: Requirement FFB-SAFETY-01.9**
    pub fn is_hardware_critical(&self) -> bool {
        matches!(
            self,
            FaultReason::OverTemp | FaultReason::OverCurrent | FaultReason::EncoderInvalid
        )
    }

    /// Check if this fault is transient (can be cleared via user action)
    ///
    /// Transient faults can be recovered from after the underlying cause
    /// is resolved, without requiring a power cycle.
    ///
    /// **Validates: Requirement FFB-SAFETY-01.10**
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            FaultReason::UsbStall
                | FaultReason::EndpointError
                | FaultReason::NanInPipeline
                | FaultReason::DeviceTimeout
                | FaultReason::DeviceDisconnect
                | FaultReason::UserEmergencyStop
                | FaultReason::HardwareEmergencyStop
        )
    }

    /// Check if this fault requires immediate torque cutoff
    ///
    /// Most faults require immediate torque cutoff for safety.
    /// Plugin overruns are the exception as they don't affect FFB output.
    pub fn requires_torque_cutoff(&self) -> bool {
        !matches!(self, FaultReason::PluginOverrun)
    }

    /// Get stable error code for this fault reason
    pub fn error_code(&self) -> &'static str {
        match self {
            FaultReason::UsbStall => "FFB_USB_STALL",
            FaultReason::EndpointError => "FFB_ENDPOINT_ERROR",
            FaultReason::NanInPipeline => "FFB_NAN_VALUE",
            FaultReason::OverTemp => "FFB_OVER_TEMP",
            FaultReason::OverCurrent => "FFB_OVER_CURRENT",
            FaultReason::EncoderInvalid => "FFB_ENCODER_INVALID",
            FaultReason::DeviceTimeout => "FFB_DEVICE_TIMEOUT",
            FaultReason::DeviceDisconnect => "FFB_DEVICE_DISCONNECT",
            FaultReason::UserEmergencyStop => "FFB_USER_ESTOP",
            FaultReason::HardwareEmergencyStop => "FFB_HW_ESTOP",
            FaultReason::PluginOverrun => "FFB_PLUGIN_OVERRUN",
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            FaultReason::UsbStall => "USB output endpoint stalled",
            FaultReason::EndpointError => "USB endpoint error or wedged",
            FaultReason::NanInPipeline => "Invalid NaN/Inf value in FFB pipeline",
            FaultReason::OverTemp => "Device over-temperature protection",
            FaultReason::OverCurrent => "Device over-current protection",
            FaultReason::EncoderInvalid => "Device encoder providing invalid readings",
            FaultReason::DeviceTimeout => "Device communication timeout",
            FaultReason::DeviceDisconnect => "Device disconnected unexpectedly",
            FaultReason::UserEmergencyStop => "User-initiated emergency stop",
            FaultReason::HardwareEmergencyStop => "Hardware emergency stop button pressed",
            FaultReason::PluginOverrun => "Plugin exceeded time budget",
        }
    }

    /// Get knowledge base article URL for this fault
    pub fn kb_article_url(&self) -> &'static str {
        match self {
            FaultReason::UsbStall => "https://docs.flight-hub.dev/kb/ffb-usb-stall",
            FaultReason::EndpointError => "https://docs.flight-hub.dev/kb/ffb-endpoint-error",
            FaultReason::NanInPipeline => "https://docs.flight-hub.dev/kb/ffb-nan-value",
            FaultReason::OverTemp => "https://docs.flight-hub.dev/kb/ffb-over-temp",
            FaultReason::OverCurrent => "https://docs.flight-hub.dev/kb/ffb-over-current",
            FaultReason::EncoderInvalid => "https://docs.flight-hub.dev/kb/ffb-encoder-invalid",
            FaultReason::DeviceTimeout => "https://docs.flight-hub.dev/kb/ffb-device-timeout",
            FaultReason::DeviceDisconnect => "https://docs.flight-hub.dev/kb/ffb-device-disconnect",
            FaultReason::UserEmergencyStop => "https://docs.flight-hub.dev/kb/ffb-emergency-stop",
            FaultReason::HardwareEmergencyStop => {
                "https://docs.flight-hub.dev/kb/ffb-emergency-stop"
            }
            FaultReason::PluginOverrun => "https://docs.flight-hub.dev/kb/ffb-plugin-overrun",
        }
    }

    /// Get maximum allowed response time for this fault
    pub fn max_response_time(&self) -> std::time::Duration {
        match self {
            // Most faults require 50ms response
            FaultReason::UsbStall
            | FaultReason::EndpointError
            | FaultReason::NanInPipeline
            | FaultReason::OverTemp
            | FaultReason::OverCurrent
            | FaultReason::EncoderInvalid
            | FaultReason::DeviceTimeout
            | FaultReason::UserEmergencyStop
            | FaultReason::HardwareEmergencyStop => std::time::Duration::from_millis(50),
            // Device disconnect detection should be within 100ms
            FaultReason::DeviceDisconnect => std::time::Duration::from_millis(100),
            // Plugin overruns don't require immediate response
            FaultReason::PluginOverrun => std::time::Duration::from_millis(100),
        }
    }
}

impl std::fmt::Display for FaultReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.description(), self.error_code())
    }
}

/// Safety states for force feedback operation
///
/// The safety state machine ensures FFB never surprises the user:
/// - SafeTorque: Limited torque envelope, safe for testing
/// - HighTorque: Full torque envelope, requires interlock
/// - Faulted: Zero torque, requires recovery action
///
/// **Validates: Requirements FFB-SAFETY-02, FFB-SAFETY-03**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyState {
    /// Safe torque operation - limited torque, no special requirements
    SafeTorque,
    /// High torque operation - requires UI consent and physical interlock
    HighTorque,
    /// Faulted state - torque disabled, requires power cycle to reset
    Faulted,
}

impl SafetyState {
    /// Check if torque output is allowed in this state
    pub fn allows_torque(&self) -> bool {
        match self {
            SafetyState::SafeTorque | SafetyState::HighTorque => true,
            SafetyState::Faulted => false,
        }
    }

    /// Check if high torque is allowed in this state
    pub fn allows_high_torque(&self) -> bool {
        matches!(self, SafetyState::HighTorque)
    }

    /// Get maximum allowed torque for this state
    pub fn max_torque_nm(&self, device_max: f32) -> f32 {
        match self {
            SafetyState::SafeTorque => (device_max * 0.3).min(5.0), // 30% or 5Nm, whichever is lower
            SafetyState::HighTorque => device_max,
            SafetyState::Faulted => 0.0,
        }
    }

    /// Check if transition to target state is valid
    pub fn can_transition_to(&self, target: SafetyState) -> bool {
        match (self, target) {
            // From SafeTorque
            (SafetyState::SafeTorque, SafetyState::HighTorque) => true,
            (SafetyState::SafeTorque, SafetyState::Faulted) => true,
            (SafetyState::SafeTorque, SafetyState::SafeTorque) => true,

            // From HighTorque
            (SafetyState::HighTorque, SafetyState::SafeTorque) => true,
            (SafetyState::HighTorque, SafetyState::Faulted) => true,
            (SafetyState::HighTorque, SafetyState::HighTorque) => true,

            // From Faulted - only to SafeTorque after power cycle
            (SafetyState::Faulted, SafetyState::SafeTorque) => true,
            (SafetyState::Faulted, _) => false,
        }
    }
}

/// Safety state transition event
#[derive(Debug, Clone)]
pub struct SafetyTransition {
    pub from: SafetyState,
    pub to: SafetyState,
    pub timestamp: Instant,
    pub reason: TransitionReason,
}

/// Reasons for safety state transitions
#[derive(Debug, Clone)]
pub enum TransitionReason {
    /// User requested high torque enable with UI consent
    UserEnableHighTorque,
    /// User requested high torque disable
    UserDisableHighTorque,
    /// Fault detected requiring safety response
    FaultDetected { fault_reason: FaultReason },
    /// Power cycle reset from faulted state
    PowerCycleReset,
    /// User cleared transient fault
    UserClearedFault,
    /// System initialization
    SystemInit,
}

impl TransitionReason {
    /// Get the fault reason if this transition was due to a fault
    pub fn fault_reason(&self) -> Option<FaultReason> {
        match self {
            TransitionReason::FaultDetected { fault_reason } => Some(*fault_reason),
            _ => None,
        }
    }
}

/// Safety state manager
#[derive(Debug)]
pub struct SafetyStateManager {
    current_state: SafetyState,
    transition_history: Vec<SafetyTransition>,
    max_history: usize,
    /// Current fault reason (if in Faulted state)
    current_fault: Option<FaultInfo>,
}

/// Information about the current fault
#[derive(Debug, Clone)]
pub struct FaultInfo {
    /// The reason for the fault
    pub reason: FaultReason,
    /// When the fault was detected
    pub timestamp: Instant,
    /// Whether this fault has been acknowledged by the user
    pub acknowledged: bool,
}

impl SafetyStateManager {
    /// Create new safety state manager
    pub fn new() -> Self {
        let initial_transition = SafetyTransition {
            from: SafetyState::SafeTorque, // Arbitrary, this is the first state
            to: SafetyState::SafeTorque,
            timestamp: Instant::now(),
            reason: TransitionReason::SystemInit,
        };

        Self {
            current_state: SafetyState::SafeTorque,
            transition_history: vec![initial_transition],
            max_history: 100,
            current_fault: None,
        }
    }

    /// Get current safety state
    pub fn current_state(&self) -> SafetyState {
        self.current_state
    }

    /// Get current fault info (if in Faulted state)
    pub fn current_fault(&self) -> Option<&FaultInfo> {
        self.current_fault.as_ref()
    }

    /// Attempt to transition to new state
    pub fn transition_to(
        &mut self,
        target: SafetyState,
        reason: TransitionReason,
    ) -> Result<(), String> {
        if !self.current_state.can_transition_to(target) {
            return Err(format!(
                "Invalid transition from {:?} to {:?}",
                self.current_state, target
            ));
        }

        let transition = SafetyTransition {
            from: self.current_state,
            to: target,
            timestamp: Instant::now(),
            reason: reason.clone(),
        };

        // Update fault info based on transition
        match (&target, &reason) {
            (SafetyState::Faulted, TransitionReason::FaultDetected { fault_reason }) => {
                self.current_fault = Some(FaultInfo {
                    reason: *fault_reason,
                    timestamp: Instant::now(),
                    acknowledged: false,
                });
            }
            (SafetyState::SafeTorque, TransitionReason::PowerCycleReset)
            | (SafetyState::SafeTorque, TransitionReason::UserClearedFault) => {
                self.current_fault = None;
            }
            _ => {}
        }

        self.current_state = target;
        self.transition_history.push(transition);

        // Keep history bounded
        if self.transition_history.len() > self.max_history {
            self.transition_history.remove(0);
        }

        Ok(())
    }

    /// Transition to faulted state due to a fault
    ///
    /// This is a convenience method that handles the fault transition logic.
    pub fn enter_faulted(&mut self, fault_reason: FaultReason) -> Result<(), String> {
        self.transition_to(
            SafetyState::Faulted,
            TransitionReason::FaultDetected { fault_reason },
        )
    }

    /// Attempt to clear a transient fault
    ///
    /// Returns Ok(()) if the fault was cleared, Err if the fault is hardware-critical
    /// and requires a power cycle.
    ///
    /// **Validates: Requirements FFB-SAFETY-01.9, FFB-SAFETY-01.10**
    pub fn clear_fault(&mut self) -> Result<(), String> {
        if self.current_state != SafetyState::Faulted {
            return Ok(()); // Not in faulted state, nothing to clear
        }

        if let Some(fault_info) = &self.current_fault {
            if fault_info.reason.is_hardware_critical() {
                return Err(format!(
                    "Cannot clear hardware-critical fault '{}' - power cycle required",
                    fault_info.reason.description()
                ));
            }
        }

        self.transition_to(SafetyState::SafeTorque, TransitionReason::UserClearedFault)
    }

    /// Reset from faulted state after power cycle
    ///
    /// This should only be called after a verified power cycle of the FFB device.
    pub fn reset_after_power_cycle(&mut self) -> Result<(), String> {
        if self.current_state != SafetyState::Faulted {
            return Ok(()); // Not in faulted state, nothing to reset
        }

        self.transition_to(SafetyState::SafeTorque, TransitionReason::PowerCycleReset)
    }

    /// Acknowledge the current fault (for UI purposes)
    pub fn acknowledge_fault(&mut self) {
        if let Some(fault_info) = &mut self.current_fault {
            fault_info.acknowledged = true;
        }
    }

    /// Check if the current fault is hardware-critical
    pub fn is_fault_hardware_critical(&self) -> bool {
        self.current_fault
            .as_ref()
            .map(|f| f.reason.is_hardware_critical())
            .unwrap_or(false)
    }

    /// Get recent transition history
    pub fn get_transition_history(&self) -> &[SafetyTransition] {
        &self.transition_history
    }

    /// Get last transition
    pub fn last_transition(&self) -> Option<&SafetyTransition> {
        self.transition_history.last()
    }

    /// Check how long we've been in current state
    pub fn time_in_current_state(&self) -> std::time::Duration {
        self.last_transition()
            .map(|t| t.timestamp.elapsed())
            .unwrap_or_default()
    }

    /// Get time since fault occurred (if in faulted state)
    pub fn time_since_fault(&self) -> Option<std::time::Duration> {
        self.current_fault.as_ref().map(|f| f.timestamp.elapsed())
    }
}

impl Default for SafetyStateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fault_reason_properties() {
        // Test hardware-critical faults
        assert!(FaultReason::OverTemp.is_hardware_critical());
        assert!(FaultReason::OverCurrent.is_hardware_critical());
        assert!(FaultReason::EncoderInvalid.is_hardware_critical());

        // Test transient faults
        assert!(FaultReason::UsbStall.is_transient());
        assert!(FaultReason::NanInPipeline.is_transient());
        assert!(FaultReason::UserEmergencyStop.is_transient());

        // Test torque cutoff requirements
        assert!(FaultReason::UsbStall.requires_torque_cutoff());
        assert!(FaultReason::OverTemp.requires_torque_cutoff());
        assert!(!FaultReason::PluginOverrun.requires_torque_cutoff());

        // Test error codes
        assert_eq!(FaultReason::UsbStall.error_code(), "FFB_USB_STALL");
        assert_eq!(FaultReason::OverTemp.error_code(), "FFB_OVER_TEMP");

        // Test response times
        assert_eq!(
            FaultReason::UsbStall.max_response_time(),
            std::time::Duration::from_millis(50)
        );
        assert_eq!(
            FaultReason::DeviceDisconnect.max_response_time(),
            std::time::Duration::from_millis(100)
        );
    }

    #[test]
    fn test_fault_reason_display() {
        let reason = FaultReason::UsbStall;
        let display = format!("{}", reason);
        assert!(display.contains("USB output endpoint stalled"));
        assert!(display.contains("FFB_USB_STALL"));
    }

    #[test]
    fn test_safety_state_torque_limits() {
        let device_max = 15.0;

        assert_eq!(SafetyState::SafeTorque.max_torque_nm(device_max), 4.5); // 30% of 15
        assert_eq!(SafetyState::HighTorque.max_torque_nm(device_max), 15.0);
        assert_eq!(SafetyState::Faulted.max_torque_nm(device_max), 0.0);
    }

    #[test]
    fn test_safety_state_transitions() {
        // Valid transitions from SafeTorque
        assert!(SafetyState::SafeTorque.can_transition_to(SafetyState::HighTorque));
        assert!(SafetyState::SafeTorque.can_transition_to(SafetyState::Faulted));

        // Valid transitions from HighTorque
        assert!(SafetyState::HighTorque.can_transition_to(SafetyState::SafeTorque));
        assert!(SafetyState::HighTorque.can_transition_to(SafetyState::Faulted));

        // Faulted can only go to SafeTorque
        assert!(SafetyState::Faulted.can_transition_to(SafetyState::SafeTorque));
        assert!(!SafetyState::Faulted.can_transition_to(SafetyState::HighTorque));
    }

    #[test]
    fn test_safety_state_manager() {
        let mut manager = SafetyStateManager::new();

        // Initial state should be SafeTorque
        assert_eq!(manager.current_state(), SafetyState::SafeTorque);

        // Transition to HighTorque
        manager
            .transition_to(
                SafetyState::HighTorque,
                TransitionReason::UserEnableHighTorque,
            )
            .unwrap();
        assert_eq!(manager.current_state(), SafetyState::HighTorque);

        // Transition to Faulted
        manager
            .transition_to(
                SafetyState::Faulted,
                TransitionReason::FaultDetected {
                    fault_reason: FaultReason::UsbStall,
                },
            )
            .unwrap();
        assert_eq!(manager.current_state(), SafetyState::Faulted);

        // Verify fault info is set
        let fault_info = manager.current_fault().unwrap();
        assert_eq!(fault_info.reason, FaultReason::UsbStall);
        assert!(!fault_info.acknowledged);

        // Invalid transition from Faulted to HighTorque
        assert!(
            manager
                .transition_to(
                    SafetyState::HighTorque,
                    TransitionReason::UserEnableHighTorque
                )
                .is_err()
        );

        // Valid transition from Faulted to SafeTorque
        manager
            .transition_to(SafetyState::SafeTorque, TransitionReason::PowerCycleReset)
            .unwrap();
        assert_eq!(manager.current_state(), SafetyState::SafeTorque);
        assert!(manager.current_fault().is_none());
    }

    #[test]
    fn test_enter_faulted_convenience() {
        let mut manager = SafetyStateManager::new();

        // Use convenience method to enter faulted state
        manager.enter_faulted(FaultReason::NanInPipeline).unwrap();

        assert_eq!(manager.current_state(), SafetyState::Faulted);
        let fault_info = manager.current_fault().unwrap();
        assert_eq!(fault_info.reason, FaultReason::NanInPipeline);
    }

    #[test]
    fn test_clear_transient_fault() {
        let mut manager = SafetyStateManager::new();

        // Enter faulted state with transient fault
        manager.enter_faulted(FaultReason::UsbStall).unwrap();
        assert_eq!(manager.current_state(), SafetyState::Faulted);

        // Clear the transient fault
        manager.clear_fault().unwrap();
        assert_eq!(manager.current_state(), SafetyState::SafeTorque);
        assert!(manager.current_fault().is_none());
    }

    #[test]
    fn test_cannot_clear_hardware_critical_fault() {
        let mut manager = SafetyStateManager::new();

        // Enter faulted state with hardware-critical fault
        manager.enter_faulted(FaultReason::OverTemp).unwrap();
        assert_eq!(manager.current_state(), SafetyState::Faulted);

        // Attempt to clear should fail
        let result = manager.clear_fault();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("power cycle required"));

        // Should still be in faulted state
        assert_eq!(manager.current_state(), SafetyState::Faulted);

        // Power cycle reset should work
        manager.reset_after_power_cycle().unwrap();
        assert_eq!(manager.current_state(), SafetyState::SafeTorque);
    }

    #[test]
    fn test_acknowledge_fault() {
        let mut manager = SafetyStateManager::new();

        manager.enter_faulted(FaultReason::DeviceTimeout).unwrap();

        // Initially not acknowledged
        assert!(!manager.current_fault().unwrap().acknowledged);

        // Acknowledge the fault
        manager.acknowledge_fault();
        assert!(manager.current_fault().unwrap().acknowledged);
    }

    #[test]
    fn test_transition_history() {
        let mut manager = SafetyStateManager::new();

        // Should have initial transition
        assert_eq!(manager.get_transition_history().len(), 1);

        // Add more transitions
        manager
            .transition_to(
                SafetyState::HighTorque,
                TransitionReason::UserEnableHighTorque,
            )
            .unwrap();

        manager
            .transition_to(
                SafetyState::SafeTorque,
                TransitionReason::UserDisableHighTorque,
            )
            .unwrap();

        assert_eq!(manager.get_transition_history().len(), 3);

        // Check last transition
        let last = manager.last_transition().unwrap();
        assert_eq!(last.to, SafetyState::SafeTorque);
        assert_eq!(last.from, SafetyState::HighTorque);
    }

    #[test]
    fn test_transition_reason_fault_reason() {
        let reason = TransitionReason::FaultDetected {
            fault_reason: FaultReason::OverCurrent,
        };
        assert_eq!(reason.fault_reason(), Some(FaultReason::OverCurrent));

        let reason = TransitionReason::UserEnableHighTorque;
        assert_eq!(reason.fault_reason(), None);
    }

    #[test]
    fn test_is_fault_hardware_critical() {
        let mut manager = SafetyStateManager::new();

        // Not in faulted state
        assert!(!manager.is_fault_hardware_critical());

        // Transient fault
        manager.enter_faulted(FaultReason::UsbStall).unwrap();
        assert!(!manager.is_fault_hardware_critical());

        // Reset and enter hardware-critical fault
        manager.clear_fault().unwrap();
        manager.enter_faulted(FaultReason::OverTemp).unwrap();
        assert!(manager.is_fault_hardware_critical());
    }
}
