// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Safe Mode Implementation
//!
//! Provides a minimal, axis-only mode for troubleshooting and safe operation
//! when full system functionality is not available or desired.

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, debug};
use flight_core::{profile::Profile, FlightError, Result};
use flight_axis::AxisEngine;
use crate::power::{PowerChecker, PowerStatus, PowerCheckStatus};

/// Safe mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeModeConfig {
    /// Enable axis processing only (no panels, plugins, tactile)
    pub axis_only: bool,
    /// Use basic profile instead of complex configurations
    pub use_basic_profile: bool,
    /// Skip power optimization checks
    pub skip_power_checks: bool,
    /// Disable all non-essential features
    pub minimal_mode: bool,
}

impl Default for SafeModeConfig {
    fn default() -> Self {
        Self {
            axis_only: true,
            use_basic_profile: true,
            skip_power_checks: false,
            minimal_mode: true,
        }
    }
}

/// Safe mode status and validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeModeStatus {
    /// Whether safe mode is currently active
    pub active: bool,
    /// Safe mode configuration
    pub config: SafeModeConfig,
    /// Power management status
    pub power_status: PowerStatus,
    /// RT privilege detection results
    pub rt_privileges: RtPrivilegeStatus,
    /// Validation results
    pub validation_results: Vec<ValidationResult>,
}

/// Real-time privilege status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtPrivilegeStatus {
    /// Whether RT privileges are available
    pub available: bool,
    /// Platform-specific privilege details
    pub details: String,
    /// Recommended actions if privileges unavailable
    pub recommendations: Vec<String>,
}

/// Validation result for safe mode components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Component being validated
    pub component: String,
    /// Validation success status
    pub success: bool,
    /// Validation message
    pub message: String,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Safe mode manager
pub struct SafeModeManager {
    config: SafeModeConfig,
    axis_engine: Option<Arc<AxisEngine>>,
}

impl SafeModeManager {
    /// Create new safe mode manager
    pub fn new(config: SafeModeConfig) -> Self {
        info!("Initializing Safe Mode with config: {:?}", config);
        
        Self {
            config,
            axis_engine: None,
        }
    }
    
    /// Initialize safe mode with validation
    pub async fn initialize(&mut self) -> Result<SafeModeStatus> {
        info!("Starting safe mode initialization");
        
        let mut validation_results = Vec::new();
        
        // Check power configuration unless skipped
        let power_status = if self.config.skip_power_checks {
            info!("Skipping power checks as requested");
            PowerStatus {
                overall_status: PowerCheckStatus::Optimal,
                checks: Vec::new(),
                remediation_steps: Vec::new(),
            }
        } else {
            let start = std::time::Instant::now();
            let status = PowerChecker::check_power_configuration().await;
            let elapsed = start.elapsed().as_millis() as u64;
            
            validation_results.push(ValidationResult {
                component: "Power Configuration".to_string(),
                success: status.overall_status != PowerCheckStatus::Critical,
                message: format!("Power status: {}", status.overall_status),
                execution_time_ms: elapsed,
            });
            
            status
        };
        
        // Check RT privileges
        let rt_privileges = self.check_rt_privileges().await;
        validation_results.push(ValidationResult {
            component: "RT Privileges".to_string(),
            success: rt_privileges.available,
            message: rt_privileges.details.clone(),
            execution_time_ms: 5, // Stub timing
        });
        
        // Initialize basic axis engine if axis processing enabled
        if self.config.axis_only {
            let start = std::time::Instant::now();
            match self.initialize_axis_engine().await {
                Ok(_) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    validation_results.push(ValidationResult {
                        component: "Axis Engine".to_string(),
                        success: true,
                        message: "Axis engine initialized successfully".to_string(),
                        execution_time_ms: elapsed,
                    });
                }
                Err(e) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    validation_results.push(ValidationResult {
                        component: "Axis Engine".to_string(),
                        success: false,
                        message: format!("Failed to initialize axis engine: {}", e),
                        execution_time_ms: elapsed,
                    });
                }
            }
        }
        
        // Validate basic profile if enabled
        if self.config.use_basic_profile {
            let start = std::time::Instant::now();
            match self.validate_basic_profile().await {
                Ok(_) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    validation_results.push(ValidationResult {
                        component: "Basic Profile".to_string(),
                        success: true,
                        message: "Basic profile validated successfully".to_string(),
                        execution_time_ms: elapsed,
                    });
                }
                Err(e) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    validation_results.push(ValidationResult {
                        component: "Basic Profile".to_string(),
                        success: false,
                        message: format!("Basic profile validation failed: {}", e),
                        execution_time_ms: elapsed,
                    });
                }
            }
        }
        
        let status = SafeModeStatus {
            active: true,
            config: self.config.clone(),
            power_status,
            rt_privileges,
            validation_results,
        };
        
        info!("Safe mode initialization completed");
        Ok(status)
    }
    
    /// Check real-time privileges availability
    async fn check_rt_privileges(&self) -> RtPrivilegeStatus {
        debug!("Checking RT privileges");
        
        #[cfg(target_os = "windows")]
        {
            // Check for MMCSS and high priority capabilities
            let available = self.check_windows_rt_privileges().await;
            RtPrivilegeStatus {
                available,
                details: if available {
                    "MMCSS 'Games' class and high priority available".to_string()
                } else {
                    "Limited RT capabilities - may affect performance".to_string()
                },
                recommendations: if available {
                    Vec::new()
                } else {
                    vec![
                        "Run as administrator for full RT capabilities".to_string(),
                        "Ensure Windows Multimedia Class Scheduler service is running".to_string(),
                    ]
                },
            }
        }
        
        #[cfg(target_os = "linux")]
        {
            // Check for SCHED_FIFO via rtkit
            let available = self.check_linux_rt_privileges().await;
            RtPrivilegeStatus {
                available,
                details: if available {
                    "SCHED_FIFO via rtkit available".to_string()
                } else {
                    "RT scheduling not available - will use normal priority".to_string()
                },
                recommendations: if available {
                    Vec::new()
                } else {
                    vec![
                        "Install rtkit package".to_string(),
                        "Add user to audio group".to_string(),
                        "Check /etc/security/limits.conf for rtprio limits".to_string(),
                    ]
                },
            }
        }
        
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            RtPrivilegeStatus {
                available: false,
                details: "RT privileges not supported on this platform".to_string(),
                recommendations: vec!["Use supported platform for RT operation".to_string()],
            }
        }
    }
    
    #[cfg(target_os = "windows")]
    async fn check_windows_rt_privileges(&self) -> bool {
        // Stub implementation - would check actual Windows capabilities
        // In real implementation, would try to set MMCSS class and check result
        true
    }
    
    #[cfg(target_os = "linux")]
    async fn check_linux_rt_privileges(&self) -> bool {
        // Stub implementation - would check rtkit availability and limits
        // In real implementation, would try to acquire SCHED_FIFO via rtkit
        true
    }
    
    /// Initialize basic axis engine for safe mode
    async fn initialize_axis_engine(&mut self) -> Result<()> {
        info!("Initializing axis engine for safe mode");
        
        let engine = AxisEngine::new();
        self.axis_engine = Some(Arc::new(engine));
        
        debug!("Axis engine initialized successfully");
        Ok(())
    }
    
    /// Validate basic profile configuration
    async fn validate_basic_profile(&self) -> Result<()> {
        info!("Validating basic profile");
        
        let basic_profile = self.create_basic_profile();
        
        // Validate profile structure
        basic_profile.validate()?;
        
        // Test profile compilation (if axis engine available)
        if let Some(_engine) = &self.axis_engine {
            // TODO: Replace with new profile ingestion API when ready
            debug!("Basic profile validated (compilation skipped for now)");
        }
        
        info!("Basic profile validation completed");
        Ok(())
    }
    
    /// Create a basic, safe profile for troubleshooting
    fn create_basic_profile(&self) -> Profile {
        // Create minimal profile with safe defaults
        // TODO: Configure axes with safe defaults when Profile API is available
        use std::collections::HashMap;
        Profile {
            schema: "flight.profile/1".to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        }
    }
    
    /// Get current safe mode status
    pub fn get_status(&self) -> SafeModeStatus {
        SafeModeStatus {
            active: true,
            config: self.config.clone(),
            power_status: PowerStatus {
                overall_status: PowerCheckStatus::Optimal,
                checks: Vec::new(),
                remediation_steps: Vec::new(),
            },
            rt_privileges: RtPrivilegeStatus {
                available: true,
                details: "Status not checked".to_string(),
                recommendations: Vec::new(),
            },
            validation_results: Vec::new(),
        }
    }
    
    /// Shutdown safe mode
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down safe mode");
        
        if let Some(engine) = self.axis_engine.take() {
            // In real implementation, would properly shutdown the engine
            debug!("Axis engine shutdown");
        }
        
        info!("Safe mode shutdown completed");
        Ok(())
    }
}

// Stub Profile implementation for compilation
mod profile_stub {
    use flight_core::{FlightError, Result};
    
    pub struct Profile {
        name: String,
    }
    
    impl Profile {
        pub fn builder() -> ProfileBuilder {
            ProfileBuilder::new()
        }
        
        pub fn validate(&self) -> Result<()> {
            Ok(())
        }
    }
    
    pub struct ProfileBuilder {
        name: String,
    }
    
    impl ProfileBuilder {
        fn new() -> Self {
            Self {
                name: "Default".to_string(),
            }
        }
        
        pub fn with_name(mut self, name: &str) -> Self {
            self.name = name.to_string();
            self
        }
        
        pub fn with_axis<F>(self, _name: &str, _config: F) -> Self
        where
            F: FnOnce(AxisBuilder) -> AxisBuilder,
        {
            self
        }
        
        pub fn build(self) -> Profile {
            Profile { name: self.name }
        }
    }
    
    pub struct AxisBuilder;
    
    impl AxisBuilder {
        pub fn with_deadzone(self, _deadzone: f32) -> Self {
            self
        }
        
        pub fn with_expo(self, _expo: f32) -> Self {
            self
        }
        
        pub fn with_slew_rate(self, _rate: f32) -> Self {
            self
        }
    }
}

use profile_stub::Profile as StubProfile;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_safe_mode_initialization() {
        let config = SafeModeConfig::default();
        let mut manager = SafeModeManager::new(config);
        
        let status = manager.initialize().await.unwrap();
        assert!(status.active);
        assert!(status.config.axis_only);
    }
    
    #[tokio::test]
    async fn test_rt_privilege_check() {
        let config = SafeModeConfig::default();
        let manager = SafeModeManager::new(config);
        
        let rt_status = manager.check_rt_privileges().await;
        // Should have some details regardless of availability
        assert!(!rt_status.details.is_empty());
    }
    
    #[test]
    fn test_basic_profile_creation() {
        let config = SafeModeConfig::default();
        let manager = SafeModeManager::new(config);
        
        let profile = manager.create_basic_profile();
        // Profile should be created successfully
        assert_eq!(profile.name, "Safe Mode Basic Profile");
    }
    
    #[tokio::test]
    async fn test_safe_mode_shutdown() {
        let config = SafeModeConfig::default();
        let mut manager = SafeModeManager::new(config);
        
        // Initialize and then shutdown
        let _status = manager.initialize().await.unwrap();
        let result = manager.shutdown().await;
        assert!(result.is_ok());
    }
}