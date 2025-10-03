// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Safety state machine for force feedback operation
//!
//! Implements the core safety state machine with three states:
//! - SafeTorque: Normal operation with limited torque
//! - HighTorque: High torque operation with full capabilities
//! - Faulted: Fault detected, torque disabled until power cycle

use std::time::Instant;

/// Safety states for force feedback operation
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
    FaultDetected { fault_type: String },
    /// Power cycle reset from faulted state
    PowerCycleReset,
    /// System initialization
    SystemInit,
}

/// Safety state manager
#[derive(Debug)]
pub struct SafetyStateManager {
    current_state: SafetyState,
    transition_history: Vec<SafetyTransition>,
    max_history: usize,
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
        }
    }

    /// Get current safety state
    pub fn current_state(&self) -> SafetyState {
        self.current_state
    }

    /// Attempt to transition to new state
    pub fn transition_to(&mut self, target: SafetyState, reason: TransitionReason) -> Result<(), String> {
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
            reason,
        };

        self.current_state = target;
        self.transition_history.push(transition);

        // Keep history bounded
        if self.transition_history.len() > self.max_history {
            self.transition_history.remove(0);
        }

        Ok(())
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
        manager.transition_to(
            SafetyState::HighTorque,
            TransitionReason::UserEnableHighTorque
        ).unwrap();
        assert_eq!(manager.current_state(), SafetyState::HighTorque);
        
        // Transition to Faulted
        manager.transition_to(
            SafetyState::Faulted,
            TransitionReason::FaultDetected { fault_type: "USB_STALL".to_string() }
        ).unwrap();
        assert_eq!(manager.current_state(), SafetyState::Faulted);
        
        // Invalid transition from Faulted to HighTorque
        assert!(manager.transition_to(
            SafetyState::HighTorque,
            TransitionReason::UserEnableHighTorque
        ).is_err());
        
        // Valid transition from Faulted to SafeTorque
        manager.transition_to(
            SafetyState::SafeTorque,
            TransitionReason::PowerCycleReset
        ).unwrap();
        assert_eq!(manager.current_state(), SafetyState::SafeTorque);
    }

    #[test]
    fn test_transition_history() {
        let mut manager = SafetyStateManager::new();
        
        // Should have initial transition
        assert_eq!(manager.get_transition_history().len(), 1);
        
        // Add more transitions
        manager.transition_to(
            SafetyState::HighTorque,
            TransitionReason::UserEnableHighTorque
        ).unwrap();
        
        manager.transition_to(
            SafetyState::SafeTorque,
            TransitionReason::UserDisableHighTorque
        ).unwrap();
        
        assert_eq!(manager.get_transition_history().len(), 3);
        
        // Check last transition
        let last = manager.last_transition().unwrap();
        assert_eq!(last.to, SafetyState::SafeTorque);
        assert_eq!(last.from, SafetyState::HighTorque);
    }
}