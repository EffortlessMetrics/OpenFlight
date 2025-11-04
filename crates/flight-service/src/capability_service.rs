// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Capability enforcement service for kid/demo mode management

use flight_core::profile::{CapabilityMode, CapabilityLimits};
use flight_axis::AxisEngine;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{info, warn};

/// Service for managing capability enforcement across axes
pub struct CapabilityService {
    /// Map of axis name to engine
    engines: Arc<RwLock<HashMap<String, Arc<AxisEngine>>>>,
    /// Global capability mode
    global_mode: Arc<RwLock<CapabilityMode>>,
    /// Per-axis capability overrides
    axis_overrides: Arc<RwLock<HashMap<String, CapabilityMode>>>,
}

/// Configuration for capability service
#[derive(Debug, Clone)]
pub struct CapabilityServiceConfig {
    /// Default capability mode for new axes
    pub default_mode: CapabilityMode,
    /// Enable audit logging by default
    pub audit_enabled: bool,
}

impl Default for CapabilityServiceConfig {
    fn default() -> Self {
        Self {
            default_mode: CapabilityMode::Full,
            audit_enabled: true,
        }
    }
}

/// Result of setting capability mode
#[derive(Debug, Clone)]
pub struct SetCapabilityResult {
    pub success: bool,
    pub error_message: Option<String>,
    pub affected_axes: Vec<String>,
    pub applied_limits: CapabilityLimits,
}

/// Status of axis capability enforcement
#[derive(Debug, Clone)]
pub struct AxisCapabilityStatus {
    pub axis_name: String,
    pub mode: CapabilityMode,
    pub limits: CapabilityLimits,
    pub audit_enabled: bool,
    pub clamp_events_count: u64,
    pub last_clamp_timestamp: Option<i64>,
}

impl CapabilityService {
    /// Create new capability service
    pub fn new() -> Self {
        Self {
            engines: Arc::new(RwLock::new(HashMap::new())),
            global_mode: Arc::new(RwLock::new(CapabilityMode::Full)),
            axis_overrides: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create capability service with configuration
    pub fn with_config(config: CapabilityServiceConfig) -> Self {
        Self {
            engines: Arc::new(RwLock::new(HashMap::new())),
            global_mode: Arc::new(RwLock::new(config.default_mode)),
            axis_overrides: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an axis engine with the service
    pub fn register_axis(&self, axis_name: String, engine: Arc<AxisEngine>) -> Result<(), String> {
        let mut engines = self.engines.write().map_err(|e| format!("Lock error: {}", e))?;
        
        // Set the engine to the current global mode or axis-specific override
        let mode = self.get_effective_mode(&axis_name)?;
        engine.set_capability_mode(mode);
        
        engines.insert(axis_name.clone(), engine);
        
        info!(axis = axis_name, mode = ?mode, "Registered axis with capability service");
        Ok(())
    }

    /// Unregister an axis engine
    pub fn unregister_axis(&self, axis_name: &str) -> Result<(), String> {
        let mut engines = self.engines.write().map_err(|e| format!("Lock error: {}", e))?;
        
        if engines.remove(axis_name).is_some() {
            info!(axis = axis_name, "Unregistered axis from capability service");
        }
        
        Ok(())
    }

    /// Set capability mode for all axes or specific axes
    pub fn set_capability_mode(
        &self,
        mode: CapabilityMode,
        axis_names: Option<Vec<String>>,
        audit_enabled: bool,
    ) -> Result<SetCapabilityResult, String> {
        let engines = self.engines.read().map_err(|e| format!("Lock error: {}", e))?;
        let mut affected_axes = Vec::new();
        
        match axis_names {
            Some(names) => {
                // Set mode for specific axes
                let mut overrides = self.axis_overrides.write().map_err(|e| format!("Lock error: {}", e))?;
                
                for axis_name in names {
                    if let Some(engine) = engines.get(&axis_name) {
                        engine.set_capability_mode(mode);
                        overrides.insert(axis_name.clone(), mode);
                        affected_axes.push(axis_name.clone());
                        
                        info!(
                            axis = axis_name,
                            mode = ?mode,
                            audit = audit_enabled,
                            "Set capability mode for axis"
                        );
                    } else {
                        warn!(axis = axis_name, "Axis not found when setting capability mode");
                    }
                }
            }
            None => {
                // Set global mode for all axes
                *self.global_mode.write().map_err(|e| format!("Lock error: {}", e))? = mode;
                
                // Clear any axis-specific overrides
                self.axis_overrides.write().map_err(|e| format!("Lock error: {}", e))?.clear();
                
                // Apply to all registered engines
                for (axis_name, engine) in engines.iter() {
                    engine.set_capability_mode(mode);
                    affected_axes.push(axis_name.clone());
                }
                
                info!(
                    mode = ?mode,
                    audit = audit_enabled,
                    axes_count = affected_axes.len(),
                    "Set global capability mode"
                );
            }
        }

        Ok(SetCapabilityResult {
            success: true,
            error_message: None,
            affected_axes,
            applied_limits: CapabilityLimits {
                max_axis_output: 1.0,
                max_ffb_torque: 10.0,
                allow_high_torque: true,
                max_expo: 1.0,
                max_slew_rate: 10.0,
            },
        })
    }

    /// Get capability mode for specific axes or all axes
    pub fn get_capability_status(
        &self,
        axis_names: Option<Vec<String>>,
    ) -> Result<Vec<AxisCapabilityStatus>, String> {
        let engines = self.engines.read().map_err(|e| format!("Lock error: {}", e))?;
        let mut status_list = Vec::new();

        let axes_to_check: Vec<String> = match axis_names {
            Some(names) => names,
            None => engines.keys().cloned().collect(),
        };

        for axis_name in axes_to_check {
            if let Some(engine) = engines.get(&axis_name) {
                let mode = engine.capability_mode();
                let context = engine.capability_context();
                
                status_list.push(AxisCapabilityStatus {
                    axis_name: axis_name.clone(),
                    mode,
                    limits: context.limits,
                    audit_enabled: context.audit_enabled,
                    clamp_events_count: 0, // TODO: Track this in engine counters
                    last_clamp_timestamp: None, // TODO: Track this in engine counters
                });
            }
        }

        Ok(status_list)
    }

    /// Get effective capability mode for an axis (considering overrides)
    fn get_effective_mode(&self, axis_name: &str) -> Result<CapabilityMode, String> {
        let overrides = self.axis_overrides.read().map_err(|e| format!("Lock error: {}", e))?;
        
        if let Some(&override_mode) = overrides.get(axis_name) {
            Ok(override_mode)
        } else {
            let global_mode = *self.global_mode.read().map_err(|e| format!("Lock error: {}", e))?;
            Ok(global_mode)
        }
    }

    /// Enable/disable kid mode (convenience method)
    pub fn set_kid_mode(&self, enabled: bool) -> Result<SetCapabilityResult, String> {
        let mode = if enabled {
            CapabilityMode::Kid
        } else {
            CapabilityMode::Full
        };
        
        self.set_capability_mode(mode, None, true)
    }

    /// Enable/disable demo mode (convenience method)
    pub fn set_demo_mode(&self, enabled: bool) -> Result<SetCapabilityResult, String> {
        let mode = if enabled {
            CapabilityMode::Demo
        } else {
            CapabilityMode::Full
        };
        
        self.set_capability_mode(mode, None, true)
    }

    /// Check if any axis is in restricted mode
    pub fn has_restricted_axes(&self) -> Result<bool, String> {
        let engines = self.engines.read().map_err(|e| format!("Lock error: {}", e))?;
        
        for engine in engines.values() {
            match engine.capability_mode() {
                CapabilityMode::Demo | CapabilityMode::Kid => return Ok(true),
                CapabilityMode::Full => continue,
            }
        }
        
        Ok(false)
    }

    /// Get list of axes in restricted modes
    pub fn get_restricted_axes(&self) -> Result<Vec<(String, CapabilityMode)>, String> {
        let engines = self.engines.read().map_err(|e| format!("Lock error: {}", e))?;
        let mut restricted = Vec::new();
        
        for (axis_name, engine) in engines.iter() {
            match engine.capability_mode() {
                CapabilityMode::Demo | CapabilityMode::Kid => {
                    restricted.push((axis_name.clone(), engine.capability_mode()));
                }
                CapabilityMode::Full => continue,
            }
        }
        
        Ok(restricted)
    }
}

impl Default for CapabilityService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_axis::AxisEngine;

    #[test]
    fn test_capability_service_creation() {
        let service = CapabilityService::new();
        assert!(service.get_capability_status(None).unwrap().is_empty());
    }

    #[test]
    fn test_axis_registration() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("test_axis".to_string()));
        
        let result = service.register_axis("test_axis".to_string(), engine);
        assert!(result.is_ok());
        
        let status = service.get_capability_status(None).unwrap();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].axis_name, "test_axis");
        assert_eq!(status[0].mode, CapabilityMode::Full);
    }

    #[test]
    fn test_global_mode_setting() {
        let service = CapabilityService::new();
        let engine1 = Arc::new(AxisEngine::new_for_axis("axis1".to_string()));
        let engine2 = Arc::new(AxisEngine::new_for_axis("axis2".to_string()));
        
        service.register_axis("axis1".to_string(), engine1.clone()).unwrap();
        service.register_axis("axis2".to_string(), engine2.clone()).unwrap();
        
        // Set global kid mode
        let result = service.set_capability_mode(CapabilityMode::Kid, None, true).unwrap();
        assert!(result.success);
        assert_eq!(result.affected_axes.len(), 2);
        
        // Verify both engines are in kid mode
        assert_eq!(engine1.capability_mode(), CapabilityMode::Kid);
        assert_eq!(engine2.capability_mode(), CapabilityMode::Kid);
    }

    #[test]
    fn test_axis_specific_mode_setting() {
        let service = CapabilityService::new();
        let engine1 = Arc::new(AxisEngine::new_for_axis("axis1".to_string()));
        let engine2 = Arc::new(AxisEngine::new_for_axis("axis2".to_string()));
        
        service.register_axis("axis1".to_string(), engine1.clone()).unwrap();
        service.register_axis("axis2".to_string(), engine2.clone()).unwrap();
        
        // Set only axis1 to demo mode
        let result = service.set_capability_mode(
            CapabilityMode::Demo,
            Some(vec!["axis1".to_string()]),
            true,
        ).unwrap();
        
        assert!(result.success);
        assert_eq!(result.affected_axes, vec!["axis1"]);
        
        // Verify only axis1 is in demo mode
        assert_eq!(engine1.capability_mode(), CapabilityMode::Demo);
        assert_eq!(engine2.capability_mode(), CapabilityMode::Full);
    }

    #[test]
    fn test_kid_mode_convenience() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("test_axis".to_string()));
        
        service.register_axis("test_axis".to_string(), engine.clone()).unwrap();
        
        // Enable kid mode
        let result = service.set_kid_mode(true).unwrap();
        assert!(result.success);
        assert_eq!(engine.capability_mode(), CapabilityMode::Kid);
        
        // Disable kid mode
        let result = service.set_kid_mode(false).unwrap();
        assert!(result.success);
        assert_eq!(engine.capability_mode(), CapabilityMode::Full);
    }

    #[test]
    fn test_demo_mode_convenience() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("test_axis".to_string()));
        
        service.register_axis("test_axis".to_string(), engine.clone()).unwrap();
        
        // Enable demo mode
        let result = service.set_demo_mode(true).unwrap();
        assert!(result.success);
        assert_eq!(engine.capability_mode(), CapabilityMode::Demo);
        
        // Disable demo mode
        let result = service.set_demo_mode(false).unwrap();
        assert!(result.success);
        assert_eq!(engine.capability_mode(), CapabilityMode::Full);
    }

    #[test]
    fn test_restricted_axes_detection() {
        let service = CapabilityService::new();
        let engine1 = Arc::new(AxisEngine::new_for_axis("axis1".to_string()));
        let engine2 = Arc::new(AxisEngine::new_for_axis("axis2".to_string()));
        
        service.register_axis("axis1".to_string(), engine1.clone()).unwrap();
        service.register_axis("axis2".to_string(), engine2.clone()).unwrap();
        
        // Initially no restricted axes
        assert!(!service.has_restricted_axes().unwrap());
        assert!(service.get_restricted_axes().unwrap().is_empty());
        
        // Set one axis to kid mode
        service.set_capability_mode(
            CapabilityMode::Kid,
            Some(vec!["axis1".to_string()]),
            true,
        ).unwrap();
        
        // Should detect restricted axes
        assert!(service.has_restricted_axes().unwrap());
        let restricted = service.get_restricted_axes().unwrap();
        assert_eq!(restricted.len(), 1);
        assert_eq!(restricted[0].0, "axis1");
        assert_eq!(restricted[0].1, CapabilityMode::Kid);
    }
}