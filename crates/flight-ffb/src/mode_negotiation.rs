// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! FFB mode negotiation and device capability management
//!
//! This module handles the negotiation of FFB modes based on device capabilities
//! and system requirements. It implements the policy for selecting between
//! DirectInput, Raw Torque, and Telemetry Synthesis modes.

use std::time::Duration;
use crate::{DeviceCapabilities, FfbMode, TrimLimits};

/// FFB mode selection policy
#[derive(Debug, Clone)]
pub struct ModeSelectionPolicy {
    /// Prefer raw torque when available
    pub prefer_raw_torque: bool,
    /// Minimum acceptable update rate in Hz
    pub min_update_rate_hz: u32,
    /// Maximum acceptable latency in microseconds
    pub max_latency_us: u32,
    /// Require health stream for high-torque operation
    pub require_health_stream_for_high_torque: bool,
}

impl Default for ModeSelectionPolicy {
    fn default() -> Self {
        Self {
            prefer_raw_torque: true,
            min_update_rate_hz: 250,
            max_latency_us: 2000, // 2ms max latency
            require_health_stream_for_high_torque: true,
        }
    }
}

/// Mode selection result
#[derive(Debug, Clone)]
pub struct ModeSelection {
    /// Selected FFB mode
    pub mode: FfbMode,
    /// Effective update rate in Hz
    pub update_rate_hz: u32,
    /// Trim limits for this mode
    pub trim_limits: TrimLimits,
    /// Whether high torque is supported
    pub supports_high_torque: bool,
    /// Selection rationale for diagnostics
    pub rationale: String,
}

/// Device capability negotiator
#[derive(Debug)]
pub struct ModeNegotiator {
    policy: ModeSelectionPolicy,
}

impl ModeNegotiator {
    /// Create new mode negotiator with default policy
    pub fn new() -> Self {
        Self {
            policy: ModeSelectionPolicy::default(),
        }
    }

    /// Create mode negotiator with custom policy
    pub fn with_policy(policy: ModeSelectionPolicy) -> Self {
        Self { policy }
    }

    /// Get current policy
    pub fn policy(&self) -> &ModeSelectionPolicy {
        &self.policy
    }

    /// Set new policy
    pub fn set_policy(&mut self, policy: ModeSelectionPolicy) {
        self.policy = policy;
    }

    /// Negotiate FFB mode based on device capabilities
    pub fn negotiate_mode(&self, capabilities: &DeviceCapabilities) -> ModeSelection {
        // Validate device capabilities
        if let Err(validation_error) = self.validate_capabilities(capabilities) {
            return ModeSelection {
                mode: FfbMode::TelemetrySynth,
                update_rate_hz: 60,
                trim_limits: TrimLimits::conservative(),
                supports_high_torque: false,
                rationale: format!("Fallback to telemetry synthesis: {}", validation_error),
            };
        }

        // Calculate effective update rate
        let max_rate_hz = if capabilities.min_period_us > 0 {
            1_000_000 / capabilities.min_period_us
        } else {
            1000 // Default to 1kHz if not specified
        };

        let effective_rate_hz = max_rate_hz.min(1000).max(self.policy.min_update_rate_hz);

        // Select mode based on policy and capabilities
        let (mode, rationale) = if self.policy.prefer_raw_torque && capabilities.supports_raw_torque {
            // Check if raw torque meets our requirements
            if max_rate_hz >= self.policy.min_update_rate_hz {
                (FfbMode::RawTorque, "Raw torque preferred and supported with adequate rate".to_string())
            } else if capabilities.supports_pid {
                (FfbMode::DirectInput, format!("Raw torque supported but rate {} Hz < required {} Hz, falling back to DirectInput", max_rate_hz, self.policy.min_update_rate_hz))
            } else {
                (FfbMode::TelemetrySynth, format!("Raw torque rate {} Hz insufficient, no DirectInput support", max_rate_hz))
            }
        } else if capabilities.supports_pid {
            (FfbMode::DirectInput, "DirectInput PID effects supported".to_string())
        } else {
            (FfbMode::TelemetrySynth, "No hardware FFB support, using telemetry synthesis".to_string())
        };

        // Adjust effective rate for telemetry synthesis
        let final_rate_hz = match mode {
            FfbMode::TelemetrySynth => 60, // Telemetry synthesis runs at 60Hz
            _ => effective_rate_hz,
        };

        // Determine trim limits based on mode and capabilities
        let trim_limits = self.calculate_trim_limits(&mode, capabilities, final_rate_hz);

        // Check high torque support
        let supports_high_torque = self.check_high_torque_support(capabilities, &mode);

        ModeSelection {
            mode,
            update_rate_hz: final_rate_hz,
            trim_limits,
            supports_high_torque,
            rationale,
        }
    }

    /// Validate device capabilities
    fn validate_capabilities(&self, capabilities: &DeviceCapabilities) -> Result<(), String> {
        // Check basic capability consistency
        if capabilities.max_torque_nm <= 0.0 {
            return Err("Invalid max torque: must be > 0".to_string());
        }

        if capabilities.max_torque_nm > 50.0 {
            return Err(format!("Excessive max torque: {} Nm > 50 Nm safety limit", capabilities.max_torque_nm));
        }

        if capabilities.supports_raw_torque && capabilities.min_period_us == 0 {
            return Err("Raw torque support requires valid min_period_us".to_string());
        }

        if capabilities.supports_raw_torque && capabilities.min_period_us > 0 {
            let max_rate_hz = 1_000_000 / capabilities.min_period_us;
            if max_rate_hz < 10 {
                return Err(format!("Raw torque update rate {} Hz too low (< 10 Hz)", max_rate_hz));
            }
        }

        Ok(())
    }

    /// Calculate appropriate trim limits for the selected mode
    fn calculate_trim_limits(&self, mode: &FfbMode, capabilities: &DeviceCapabilities, rate_hz: u32) -> TrimLimits {
        match mode {
            FfbMode::RawTorque => {
                // Raw torque mode allows more aggressive limits due to direct control
                let base_rate = (capabilities.max_torque_nm * 0.8).min(25.0); // 80% of max torque per second, capped at 25 Nm/s
                let rate_factor = (rate_hz as f32 / 500.0).min(1.5); // Scale with update rate, cap at 1.5x
                
                TrimLimits {
                    max_rate_nm_per_s: base_rate * rate_factor,
                    max_jerk_nm_per_s2: (base_rate * rate_factor * 6.0).min(150.0), // 6x rate for jerk, capped at 150 Nm/s²
                }
            }
            FfbMode::DirectInput => {
                // DirectInput is more conservative due to driver/OS latency
                let base_rate = capabilities.max_torque_nm * 0.5; // 50% of max torque per second
                
                TrimLimits {
                    max_rate_nm_per_s: base_rate,
                    max_jerk_nm_per_s2: base_rate * 4.0, // 4x rate for jerk
                }
            }
            FfbMode::TelemetrySynth | FfbMode::Auto => {
                // Most conservative for synthesized effects
                TrimLimits::conservative()
            }
        }
    }

    /// Check if high torque operation is supported
    fn check_high_torque_support(&self, capabilities: &DeviceCapabilities, mode: &FfbMode) -> bool {
        // Basic torque capability check
        if capabilities.max_torque_nm < 5.0 {
            return false;
        }

        // Health stream requirement for high torque (only for raw torque mode)
        if self.policy.require_health_stream_for_high_torque && !capabilities.has_health_stream && mode == &FfbMode::RawTorque {
            return false;
        }

        // Mode-specific checks
        match mode {
            FfbMode::RawTorque => {
                // Raw torque requires interlock support for high torque
                capabilities.supports_interlock
            }
            FfbMode::DirectInput => {
                // DirectInput supports high torque if device has sufficient capability
                // Health stream requirement is relaxed for DirectInput mode
                capabilities.max_torque_nm >= 8.0
            }
            FfbMode::TelemetrySynth => {
                // Telemetry synthesis doesn't support high torque
                false
            }
            FfbMode::Auto => {
                // Auto mode shouldn't reach here, but be conservative
                false
            }
        }
    }

    /// Create mode selection matrix for testing
    pub fn create_selection_matrix(&self) -> Vec<ModeSelectionTest> {
        vec![
            // High-end raw torque device
            ModeSelectionTest {
                name: "High-end raw torque device".to_string(),
                capabilities: DeviceCapabilities {
                    supports_pid: true,
                    supports_raw_torque: true,
                    max_torque_nm: 15.0,
                    min_period_us: 1000, // 1kHz
                    has_health_stream: true,
                    supports_interlock: true,
                },
                expected_mode: FfbMode::RawTorque,
                expected_high_torque: true,
            },
            
            // Mid-range DirectInput device
            ModeSelectionTest {
                name: "Mid-range DirectInput device".to_string(),
                capabilities: DeviceCapabilities {
                    supports_pid: true,
                    supports_raw_torque: false,
                    max_torque_nm: 10.0,
                    min_period_us: 0,
                    has_health_stream: true,
                    supports_interlock: false,
                },
                expected_mode: FfbMode::DirectInput,
                expected_high_torque: true,
            },
            
            // Low-end device without FFB
            ModeSelectionTest {
                name: "Low-end device without FFB".to_string(),
                capabilities: DeviceCapabilities {
                    supports_pid: false,
                    supports_raw_torque: false,
                    max_torque_nm: 3.0,
                    min_period_us: 0,
                    has_health_stream: false,
                    supports_interlock: false,
                },
                expected_mode: FfbMode::TelemetrySynth,
                expected_high_torque: false,
            },
            
            // Raw torque device with insufficient rate
            ModeSelectionTest {
                name: "Raw torque device with insufficient rate".to_string(),
                capabilities: DeviceCapabilities {
                    supports_pid: true,
                    supports_raw_torque: true,
                    max_torque_nm: 12.0,
                    min_period_us: 50000, // 20Hz - too slow
                    has_health_stream: true,
                    supports_interlock: true,
                },
                expected_mode: FfbMode::DirectInput,
                expected_high_torque: true,
            },
            
            // Device with excessive torque
            ModeSelectionTest {
                name: "Device with excessive torque".to_string(),
                capabilities: DeviceCapabilities {
                    supports_pid: true,
                    supports_raw_torque: true,
                    max_torque_nm: 60.0, // Exceeds safety limit
                    min_period_us: 1000,
                    has_health_stream: true,
                    supports_interlock: true,
                },
                expected_mode: FfbMode::TelemetrySynth,
                expected_high_torque: false,
            },
        ]
    }
}

/// Test case for mode selection matrix
#[derive(Debug, Clone)]
pub struct ModeSelectionTest {
    pub name: String,
    pub capabilities: DeviceCapabilities,
    pub expected_mode: FfbMode,
    pub expected_high_torque: bool,
}

impl TrimLimits {
    /// Conservative trim limits for fallback modes
    pub fn conservative() -> Self {
        Self {
            max_rate_nm_per_s: 2.0,   // 2 Nm/s - very conservative
            max_jerk_nm_per_s2: 8.0,  // 8 Nm/s² - conservative jerk
        }
    }

    /// Aggressive trim limits for high-performance modes
    pub fn aggressive() -> Self {
        Self {
            max_rate_nm_per_s: 15.0,  // 15 Nm/s - fast response
            max_jerk_nm_per_s2: 60.0, // 60 Nm/s² - high jerk
        }
    }

    /// Validate trim limits are reasonable
    pub fn validate(&self) -> Result<(), String> {
        if self.max_rate_nm_per_s <= 0.0 {
            return Err("Max rate must be > 0".to_string());
        }
        
        if self.max_jerk_nm_per_s2 <= 0.0 {
            return Err("Max jerk must be > 0".to_string());
        }
        
        if self.max_rate_nm_per_s > 50.0 {
            return Err("Max rate exceeds safety limit (50 Nm/s)".to_string());
        }
        
        if self.max_jerk_nm_per_s2 > 200.0 {
            return Err("Max jerk exceeds safety limit (200 Nm/s²)".to_string());
        }
        
        // Jerk should be at least 2x rate for reasonable response
        if self.max_jerk_nm_per_s2 < self.max_rate_nm_per_s * 2.0 {
            return Err("Max jerk should be at least 2x max rate for proper response".to_string());
        }
        
        Ok(())
    }
}

impl Default for ModeNegotiator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_negotiator_creation() {
        let negotiator = ModeNegotiator::new();
        assert_eq!(negotiator.policy().prefer_raw_torque, true);
        assert_eq!(negotiator.policy().min_update_rate_hz, 250);
    }

    #[test]
    fn test_custom_policy() {
        let policy = ModeSelectionPolicy {
            prefer_raw_torque: false,
            min_update_rate_hz: 500,
            max_latency_us: 1000,
            require_health_stream_for_high_torque: false,
        };
        
        let negotiator = ModeNegotiator::with_policy(policy.clone());
        assert_eq!(negotiator.policy().prefer_raw_torque, false);
        assert_eq!(negotiator.policy().min_update_rate_hz, 500);
    }

    #[test]
    fn test_capability_validation() {
        let negotiator = ModeNegotiator::new();
        
        // Valid capabilities
        let valid_caps = DeviceCapabilities {
            supports_pid: true,
            supports_raw_torque: true,
            max_torque_nm: 15.0,
            min_period_us: 1000,
            has_health_stream: true,
            supports_interlock: true,
        };
        assert!(negotiator.validate_capabilities(&valid_caps).is_ok());
        
        // Invalid torque
        let invalid_torque = DeviceCapabilities {
            max_torque_nm: 0.0,
            ..valid_caps.clone()
        };
        assert!(negotiator.validate_capabilities(&invalid_torque).is_err());
        
        // Excessive torque
        let excessive_torque = DeviceCapabilities {
            max_torque_nm: 60.0,
            ..valid_caps.clone()
        };
        assert!(negotiator.validate_capabilities(&excessive_torque).is_err());
        
        // Raw torque without period
        let raw_no_period = DeviceCapabilities {
            supports_raw_torque: true,
            min_period_us: 0,
            ..valid_caps.clone()
        };
        assert!(negotiator.validate_capabilities(&raw_no_period).is_err());
    }

    #[test]
    fn test_mode_selection_matrix() {
        let negotiator = ModeNegotiator::new();
        let test_matrix = negotiator.create_selection_matrix();
        
        for test_case in test_matrix {
            let selection = negotiator.negotiate_mode(&test_case.capabilities);
            
            assert_eq!(
                selection.mode, test_case.expected_mode,
                "Mode mismatch for test case: {}", test_case.name
            );
            
            assert_eq!(
                selection.supports_high_torque, test_case.expected_high_torque,
                "High torque support mismatch for test case: {}", test_case.name
            );
            
            // Validate trim limits
            assert!(selection.trim_limits.validate_trim_limits().is_ok(),
                "Invalid trim limits for test case: {}", test_case.name);
        }
    }

    #[test]
    fn test_raw_torque_selection() {
        let negotiator = ModeNegotiator::new();
        
        let capabilities = DeviceCapabilities {
            supports_pid: true,
            supports_raw_torque: true,
            max_torque_nm: 15.0,
            min_period_us: 1000, // 1kHz
            has_health_stream: true,
            supports_interlock: true,
        };
        
        let selection = negotiator.negotiate_mode(&capabilities);
        
        assert_eq!(selection.mode, FfbMode::RawTorque);
        assert!(selection.supports_high_torque);
        assert_eq!(selection.update_rate_hz, 1000);
        assert!(selection.trim_limits.max_rate_nm_per_s > 10.0); // Should be aggressive
    }

    #[test]
    fn test_directinput_fallback() {
        let negotiator = ModeNegotiator::new();
        
        let capabilities = DeviceCapabilities {
            supports_pid: true,
            supports_raw_torque: true,
            max_torque_nm: 12.0,
            min_period_us: 50000, // 20Hz - too slow for raw torque
            has_health_stream: true,
            supports_interlock: true,
        };
        
        let selection = negotiator.negotiate_mode(&capabilities);
        
        assert_eq!(selection.mode, FfbMode::DirectInput);
        assert!(selection.supports_high_torque);
        assert!(selection.rationale.contains("rate"));
    }

    #[test]
    fn test_telemetry_synthesis_fallback() {
        let negotiator = ModeNegotiator::new();
        
        let capabilities = DeviceCapabilities {
            supports_pid: false,
            supports_raw_torque: false,
            max_torque_nm: 3.0,
            min_period_us: 0,
            has_health_stream: false,
            supports_interlock: false,
        };
        
        let selection = negotiator.negotiate_mode(&capabilities);
        
        assert_eq!(selection.mode, FfbMode::TelemetrySynth);
        assert!(!selection.supports_high_torque);
        assert_eq!(selection.update_rate_hz, 60);
    }

    #[test]
    fn test_trim_limits_validation() {
        let valid_limits = TrimLimits {
            max_rate_nm_per_s: 10.0,
            max_jerk_nm_per_s2: 40.0,
        };
        assert!(valid_limits.validate_trim_limits().is_ok());
        
        let invalid_rate = TrimLimits {
            max_rate_nm_per_s: 0.0,
            max_jerk_nm_per_s2: 20.0,
        };
        assert!(invalid_rate.validate_trim_limits().is_err());
        
        let excessive_rate = TrimLimits {
            max_rate_nm_per_s: 60.0,
            max_jerk_nm_per_s2: 120.0,
        };
        assert!(excessive_rate.validate_trim_limits().is_err());
        
        let insufficient_jerk = TrimLimits {
            max_rate_nm_per_s: 10.0,
            max_jerk_nm_per_s2: 15.0, // Less than 2x rate
        };
        assert!(insufficient_jerk.validate_trim_limits().is_err());
    }

    #[test]
    fn test_conservative_vs_aggressive_limits() {
        let conservative = TrimLimits::conservative();
        let aggressive = TrimLimits::aggressive();
        
        assert!(conservative.validate_trim_limits().is_ok());
        assert!(aggressive.validate_trim_limits().is_ok());
        
        assert!(conservative.max_rate_nm_per_s < aggressive.max_rate_nm_per_s);
        assert!(conservative.max_jerk_nm_per_s2 < aggressive.max_jerk_nm_per_s2);
    }

    #[test]
    fn test_high_torque_support_logic() {
        let negotiator = ModeNegotiator::new();
        
        // High torque with all requirements
        let high_torque_caps = DeviceCapabilities {
            supports_pid: true,
            supports_raw_torque: true,
            max_torque_nm: 15.0,
            min_period_us: 1000,
            has_health_stream: true,
            supports_interlock: true,
        };
        assert!(negotiator.check_high_torque_support(&high_torque_caps, &FfbMode::RawTorque));
        
        // Insufficient torque
        let low_torque_caps = DeviceCapabilities {
            max_torque_nm: 3.0,
            ..high_torque_caps.clone()
        };
        assert!(!negotiator.check_high_torque_support(&low_torque_caps, &FfbMode::RawTorque));
        
        // No health stream
        let no_health_caps = DeviceCapabilities {
            has_health_stream: false,
            ..high_torque_caps.clone()
        };
        assert!(!negotiator.check_high_torque_support(&no_health_caps, &FfbMode::RawTorque));
        
        // DirectInput without interlock (but sufficient torque)
        let di_caps = DeviceCapabilities {
            supports_interlock: false,
            max_torque_nm: 10.0,
            ..high_torque_caps.clone()
        };
        assert!(negotiator.check_high_torque_support(&di_caps, &FfbMode::DirectInput));
        
        // Telemetry synthesis never supports high torque
        assert!(!negotiator.check_high_torque_support(&high_torque_caps, &FfbMode::TelemetrySynth));
    }
}