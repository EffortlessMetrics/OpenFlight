// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Capability enforcement service for kid/demo mode management

use flight_axis::AxisEngine;
use flight_core::profile::{CapabilityContext, CapabilityLimits, CapabilityMode};
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
    /// Global capability audit logging state
    audit_enabled: Arc<RwLock<bool>>,
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
    /// Maximum absolute output value seen before a clamp (diagnostics)
    pub max_value_before_clamp: f32,
}

impl CapabilityService {
    /// Create new capability service
    pub fn new() -> Self {
        Self::with_config(CapabilityServiceConfig::default())
    }

    /// Create capability service with configuration
    pub fn with_config(config: CapabilityServiceConfig) -> Self {
        Self {
            engines: Arc::new(RwLock::new(HashMap::new())),
            global_mode: Arc::new(RwLock::new(config.default_mode)),
            axis_overrides: Arc::new(RwLock::new(HashMap::new())),
            audit_enabled: Arc::new(RwLock::new(config.audit_enabled)),
        }
    }

    /// Register an axis engine with the service
    pub fn register_axis(&self, axis_name: String, engine: Arc<AxisEngine>) -> Result<(), String> {
        let mut engines = self
            .engines
            .write()
            .map_err(|e| format!("Lock error: {}", e))?;

        // Set the engine to the current global mode or axis-specific override
        let mode = self.get_effective_mode(&axis_name)?;
        engine.set_capability_mode(mode);
        let audit_enabled = *self
            .audit_enabled
            .read()
            .map_err(|e| format!("Lock error: {}", e))?;
        engine.set_capability_audit_enabled(audit_enabled);

        engines.insert(axis_name.clone(), engine);

        info!(axis = axis_name, mode = ?mode, "Registered axis with capability service");
        Ok(())
    }

    /// Unregister an axis engine
    pub fn unregister_axis(&self, axis_name: &str) -> Result<(), String> {
        let mut engines = self
            .engines
            .write()
            .map_err(|e| format!("Lock error: {}", e))?;

        if engines.remove(axis_name).is_some() {
            info!(
                axis = axis_name,
                "Unregistered axis from capability service"
            );
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
        let mut context = CapabilityContext::for_mode(mode);
        context.audit_enabled = audit_enabled;
        let applied_limits = context.limits;

        *self
            .audit_enabled
            .write()
            .map_err(|e| format!("Lock error: {}", e))? = audit_enabled;

        let engines = self
            .engines
            .read()
            .map_err(|e| format!("Lock error: {}", e))?;
        let mut affected_axes = Vec::new();

        match axis_names {
            Some(names) => {
                // Set mode for specific axes
                let mut overrides = self
                    .axis_overrides
                    .write()
                    .map_err(|e| format!("Lock error: {}", e))?;

                for axis_name in names {
                    if let Some(engine) = engines.get(&axis_name) {
                        engine.set_capability_mode(mode);
                        engine.set_capability_audit_enabled(audit_enabled);
                        overrides.insert(axis_name.clone(), mode);
                        affected_axes.push(axis_name.clone());

                        info!(
                            axis = axis_name,
                            mode = ?mode,
                            audit = audit_enabled,
                            "Set capability mode for axis"
                        );
                    } else {
                        warn!(
                            axis = axis_name,
                            "Axis not found when setting capability mode"
                        );
                    }
                }
            }
            None => {
                // Set global mode for all axes
                *self
                    .global_mode
                    .write()
                    .map_err(|e| format!("Lock error: {}", e))? = mode;

                // Clear any axis-specific overrides
                self.axis_overrides
                    .write()
                    .map_err(|e| format!("Lock error: {}", e))?
                    .clear();

                // Apply to all registered engines
                for (axis_name, engine) in engines.iter() {
                    engine.set_capability_mode(mode);
                    engine.set_capability_audit_enabled(audit_enabled);
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
            applied_limits,
        })
    }

    /// Get capability mode for specific axes or all axes
    pub fn get_capability_status(
        &self,
        axis_names: Option<Vec<String>>,
    ) -> Result<Vec<AxisCapabilityStatus>, String> {
        let engines = self
            .engines
            .read()
            .map_err(|e| format!("Lock error: {}", e))?;
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
                    clamp_events_count: engine.counters().capability_clamp_events(),
                    last_clamp_timestamp: {
                        let ns = engine.counters().last_capability_clamp_ns();
                        if ns == 0 { None } else { Some(ns as i64) }
                    },
                    max_value_before_clamp: engine.counters().max_value_before_clamp(),
                });
            }
        }

        Ok(status_list)
    }

    /// Get effective capability mode for an axis (considering overrides)
    fn get_effective_mode(&self, axis_name: &str) -> Result<CapabilityMode, String> {
        let overrides = self
            .axis_overrides
            .read()
            .map_err(|e| format!("Lock error: {}", e))?;

        if let Some(&override_mode) = overrides.get(axis_name) {
            Ok(override_mode)
        } else {
            let global_mode = *self
                .global_mode
                .read()
                .map_err(|e| format!("Lock error: {}", e))?;
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
        let engines = self
            .engines
            .read()
            .map_err(|e| format!("Lock error: {}", e))?;

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
        let engines = self
            .engines
            .read()
            .map_err(|e| format!("Lock error: {}", e))?;
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

    /// Get the total number of clamp events across all registered axes.
    pub fn total_clamp_events(&self) -> Result<u64, String> {
        let engines = self
            .engines
            .read()
            .map_err(|e| format!("Lock error: {}", e))?;

        let total = engines
            .values()
            .map(|e| e.counters().capability_clamp_events())
            .sum();
        Ok(total)
    }

    /// Get the highest absolute output value observed before any clamp, across
    /// all registered axes.  Returns 0.0 if no clamp has ever occurred.
    pub fn max_value_before_clamp(&self) -> Result<f32, String> {
        let engines = self
            .engines
            .read()
            .map_err(|e| format!("Lock error: {}", e))?;

        let max = engines
            .values()
            .map(|e| e.counters().max_value_before_clamp())
            .fold(0.0_f32, f32::max);
        Ok(max)
    }

    /// Reset clamp counters for all axes (or a specific set of axes).
    pub fn reset_clamp_counters(
        &self,
        axis_names: Option<&[String]>,
    ) -> Result<Vec<String>, String> {
        let engines = self
            .engines
            .read()
            .map_err(|e| format!("Lock error: {}", e))?;

        let mut reset_axes = Vec::new();
        match axis_names {
            Some(names) => {
                for name in names {
                    if let Some(engine) = engines.get(name) {
                        engine.reset_counters();
                        reset_axes.push(name.clone());
                    }
                }
            }
            None => {
                for (name, engine) in engines.iter() {
                    engine.reset_counters();
                    reset_axes.push(name.clone());
                }
            }
        }
        Ok(reset_axes)
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
    use flight_axis::{AxisEngine, AxisFrame};

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

        service
            .register_axis("axis1".to_string(), engine1.clone())
            .unwrap();
        service
            .register_axis("axis2".to_string(), engine2.clone())
            .unwrap();

        // Set global kid mode
        let result = service
            .set_capability_mode(CapabilityMode::Kid, None, true)
            .unwrap();
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

        service
            .register_axis("axis1".to_string(), engine1.clone())
            .unwrap();
        service
            .register_axis("axis2".to_string(), engine2.clone())
            .unwrap();

        // Set only axis1 to demo mode
        let result = service
            .set_capability_mode(CapabilityMode::Demo, Some(vec!["axis1".to_string()]), true)
            .unwrap();

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

        service
            .register_axis("test_axis".to_string(), engine.clone())
            .unwrap();

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

        service
            .register_axis("test_axis".to_string(), engine.clone())
            .unwrap();

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

        service
            .register_axis("axis1".to_string(), engine1.clone())
            .unwrap();
        service
            .register_axis("axis2".to_string(), engine2.clone())
            .unwrap();

        // Initially no restricted axes
        assert!(!service.has_restricted_axes().unwrap());
        assert!(service.get_restricted_axes().unwrap().is_empty());

        // Set one axis to kid mode
        service
            .set_capability_mode(CapabilityMode::Kid, Some(vec!["axis1".to_string()]), true)
            .unwrap();

        // Should detect restricted axes
        assert!(service.has_restricted_axes().unwrap());
        let restricted = service.get_restricted_axes().unwrap();
        assert_eq!(restricted.len(), 1);
        assert_eq!(restricted[0].0, "axis1");
        assert_eq!(restricted[0].1, CapabilityMode::Kid);
    }

    #[test]
    fn axis_capability_status_defaults_are_reasonable() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));
        service.register_axis("pitch".to_string(), engine).unwrap();

        let status_list = service.get_capability_status(None).unwrap();
        assert_eq!(status_list.len(), 1);
        let status = &status_list[0];
        assert_eq!(status.axis_name, "pitch");
        assert_eq!(status.mode, CapabilityMode::Full);
        assert_eq!(status.clamp_events_count, 0);
        assert!(status.last_clamp_timestamp.is_none());
        assert_eq!(status.max_value_before_clamp, 0.0);
        assert_eq!(status.limits.max_axis_output, 1.0);
        assert_eq!(status.limits.max_ffb_torque, 50.0);
    }

    #[test]
    fn clamp_counter_increments_when_axis_clamped() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("throttle".to_string()));
        service
            .register_axis("throttle".to_string(), engine.clone())
            .unwrap();

        // Engage kid mode (max 50% output)
        service.set_kid_mode(true).unwrap();

        // Process a frame whose output exceeds the kid-mode limit
        let mut frame = AxisFrame::new(0.9, 1000);
        frame.out = 0.9;
        engine.process(&mut frame).unwrap();

        let status = service.get_capability_status(None).unwrap();
        assert_eq!(status[0].clamp_events_count, 1);
        assert!(status[0].last_clamp_timestamp.is_some());
    }

    #[test]
    fn demo_mode_reports_restricted_axes() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("roll".to_string()));
        service.register_axis("roll".to_string(), engine).unwrap();

        service.set_demo_mode(true).unwrap();

        let status = service.get_capability_status(None).unwrap();
        assert_eq!(status[0].mode, CapabilityMode::Demo);
        assert_eq!(status[0].limits.max_axis_output, 0.8);
        assert!(service.has_restricted_axes().unwrap());
    }

    #[test]
    fn kid_mode_reports_restricted_axes() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("yaw".to_string()));
        service.register_axis("yaw".to_string(), engine).unwrap();

        service.set_kid_mode(true).unwrap();

        let status = service.get_capability_status(None).unwrap();
        assert_eq!(status[0].mode, CapabilityMode::Kid);
        assert_eq!(status[0].limits.max_axis_output, 0.5);
        assert!(service.has_restricted_axes().unwrap());
    }

    #[test]
    fn demo_mode_clamp_counter_increments_when_axis_clamped() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("roll".to_string()));
        service
            .register_axis("roll".to_string(), engine.clone())
            .unwrap();

        // Engage demo mode (max 80% output)
        service.set_demo_mode(true).unwrap();

        // Process a frame whose output exceeds the demo-mode limit
        let mut frame = AxisFrame::new(0.9, 1000);
        frame.out = 0.9;
        engine.process(&mut frame).unwrap();

        let status = service.get_capability_status(None).unwrap();
        assert_eq!(status[0].clamp_events_count, 1);
        assert!(status[0].last_clamp_timestamp.is_some());
        assert!((status[0].max_value_before_clamp - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn demo_mode_clamps_and_records_evidence() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));
        service
            .register_axis("pitch".to_string(), engine.clone())
            .unwrap();

        service.set_demo_mode(true).unwrap();

        // Frame within demo limits — no clamp expected
        let mut frame_ok = AxisFrame::new(0.7, 1000);
        frame_ok.out = 0.7;
        engine.process(&mut frame_ok).unwrap();
        assert_eq!(frame_ok.out, 0.7);
        assert_eq!(service.total_clamp_events().unwrap(), 0);

        // Frame exceeding demo limit (0.8) — clamp expected
        let mut frame_over = AxisFrame::new(0.95, 2000);
        frame_over.out = 0.95;
        engine.process(&mut frame_over).unwrap();
        assert_eq!(frame_over.out, 0.8);

        let status = service.get_capability_status(None).unwrap();
        assert_eq!(status[0].clamp_events_count, 1);
        assert!(status[0].last_clamp_timestamp.is_some());
        assert!((status[0].max_value_before_clamp - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn kid_mode_clamps_and_records_evidence() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("roll".to_string()));
        service
            .register_axis("roll".to_string(), engine.clone())
            .unwrap();

        service.set_kid_mode(true).unwrap();

        // Frame within kid limits — no clamp expected
        let mut frame_ok = AxisFrame::new(0.4, 1000);
        frame_ok.out = 0.4;
        engine.process(&mut frame_ok).unwrap();
        assert_eq!(frame_ok.out, 0.4);
        assert_eq!(service.total_clamp_events().unwrap(), 0);

        // Frame exceeding kid limit (0.5) — clamp expected
        let mut frame_over = AxisFrame::new(0.8, 2000);
        frame_over.out = 0.8;
        engine.process(&mut frame_over).unwrap();
        assert_eq!(frame_over.out, 0.5);

        let status = service.get_capability_status(None).unwrap();
        assert_eq!(status[0].clamp_events_count, 1);
        assert!(status[0].last_clamp_timestamp.is_some());
        assert!((status[0].max_value_before_clamp - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn counters_increment_across_multiple_clamp_events() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("throttle".to_string()));
        service
            .register_axis("throttle".to_string(), engine.clone())
            .unwrap();

        service.set_kid_mode(true).unwrap();

        // Send three frames that all exceed the kid-mode limit
        for i in 0..3 {
            let mut frame = AxisFrame::new(0.9, 1000 + i * 100);
            frame.out = 0.9;
            engine.process(&mut frame).unwrap();
        }

        let status = service.get_capability_status(None).unwrap();
        assert_eq!(status[0].clamp_events_count, 3);
        assert!(status[0].last_clamp_timestamp.is_some());
    }

    #[test]
    fn timestamps_update_on_successive_clamp_events() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("yaw".to_string()));
        service
            .register_axis("yaw".to_string(), engine.clone())
            .unwrap();

        service.set_kid_mode(true).unwrap();

        // First clamp
        let mut f1 = AxisFrame::new(0.9, 1000);
        f1.out = 0.9;
        engine.process(&mut f1).unwrap();

        let ts1 = service.get_capability_status(None).unwrap()[0]
            .last_clamp_timestamp
            .expect("timestamp should be set after first clamp");

        // Small delay to ensure monotonic advance
        std::thread::sleep(std::time::Duration::from_millis(2));

        // Second clamp
        let mut f2 = AxisFrame::new(0.9, 2000);
        f2.out = 0.9;
        engine.process(&mut f2).unwrap();

        let ts2 = service.get_capability_status(None).unwrap()[0]
            .last_clamp_timestamp
            .expect("timestamp should be set after second clamp");

        assert!(ts2 > ts1, "second clamp timestamp must be later than first");
    }

    #[test]
    fn total_clamp_events_aggregates_across_axes() {
        let service = CapabilityService::new();
        let engine1 = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));
        let engine2 = Arc::new(AxisEngine::new_for_axis("roll".to_string()));
        service
            .register_axis("pitch".to_string(), engine1.clone())
            .unwrap();
        service
            .register_axis("roll".to_string(), engine2.clone())
            .unwrap();

        service.set_kid_mode(true).unwrap();

        // Clamp on pitch
        let mut f1 = AxisFrame::new(0.9, 1000);
        f1.out = 0.9;
        engine1.process(&mut f1).unwrap();

        // Two clamps on roll
        for ts in [2000, 3000] {
            let mut f = AxisFrame::new(0.8, ts);
            f.out = 0.8;
            engine2.process(&mut f).unwrap();
        }

        assert_eq!(service.total_clamp_events().unwrap(), 3);
    }

    #[test]
    fn max_value_before_clamp_tracks_worst_case() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("throttle".to_string()));
        service
            .register_axis("throttle".to_string(), engine.clone())
            .unwrap();

        service.set_kid_mode(true).unwrap();

        // Moderate overshoot
        let mut f1 = AxisFrame::new(0.7, 1000);
        f1.out = 0.7;
        engine.process(&mut f1).unwrap();

        assert!((service.max_value_before_clamp().unwrap() - 0.7).abs() < f32::EPSILON);

        // Larger overshoot — max should update
        let mut f2 = AxisFrame::new(0.95, 2000);
        f2.out = 0.95;
        engine.process(&mut f2).unwrap();

        assert!((service.max_value_before_clamp().unwrap() - 0.95).abs() < f32::EPSILON);

        // Smaller overshoot — max should stay at 0.95
        let mut f3 = AxisFrame::new(0.6, 3000);
        f3.out = 0.6;
        engine.process(&mut f3).unwrap();

        assert!((service.max_value_before_clamp().unwrap() - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn reset_clamp_counters_clears_all_axes() {
        let service = CapabilityService::new();
        let engine1 = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));
        let engine2 = Arc::new(AxisEngine::new_for_axis("roll".to_string()));
        service
            .register_axis("pitch".to_string(), engine1.clone())
            .unwrap();
        service
            .register_axis("roll".to_string(), engine2.clone())
            .unwrap();

        service.set_kid_mode(true).unwrap();

        // Generate clamp events on both axes
        let mut f1 = AxisFrame::new(0.9, 1000);
        f1.out = 0.9;
        engine1.process(&mut f1).unwrap();

        let mut f2 = AxisFrame::new(0.8, 2000);
        f2.out = 0.8;
        engine2.process(&mut f2).unwrap();

        assert_eq!(service.total_clamp_events().unwrap(), 2);

        // Reset all
        let reset = service.reset_clamp_counters(None).unwrap();
        assert_eq!(reset.len(), 2);
        assert_eq!(service.total_clamp_events().unwrap(), 0);
        assert_eq!(service.max_value_before_clamp().unwrap(), 0.0);
    }

    #[test]
    fn reset_clamp_counters_targets_specific_axis() {
        let service = CapabilityService::new();
        let engine1 = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));
        let engine2 = Arc::new(AxisEngine::new_for_axis("roll".to_string()));
        service
            .register_axis("pitch".to_string(), engine1.clone())
            .unwrap();
        service
            .register_axis("roll".to_string(), engine2.clone())
            .unwrap();

        service.set_kid_mode(true).unwrap();

        let mut f1 = AxisFrame::new(0.9, 1000);
        f1.out = 0.9;
        engine1.process(&mut f1).unwrap();

        let mut f2 = AxisFrame::new(0.8, 2000);
        f2.out = 0.8;
        engine2.process(&mut f2).unwrap();

        // Reset only pitch
        let reset = service
            .reset_clamp_counters(Some(&["pitch".to_string()]))
            .unwrap();
        assert_eq!(reset, vec!["pitch"]);

        // pitch should be 0, roll should still be 1
        let statuses = service.get_capability_status(None).unwrap();
        let pitch = statuses.iter().find(|s| s.axis_name == "pitch").unwrap();
        let roll = statuses.iter().find(|s| s.axis_name == "roll").unwrap();
        assert_eq!(pitch.clamp_events_count, 0);
        assert_eq!(roll.clamp_events_count, 1);
    }
}
