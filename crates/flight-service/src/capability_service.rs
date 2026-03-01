// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Capability enforcement service for kid/demo mode management

use flight_axis::AxisEngine;
use flight_core::profile::{CapabilityContext, CapabilityLimits, CapabilityMode};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
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
    /// Service-level capability clamp metrics
    metrics: Arc<CapabilityMetrics>,
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

/// Atomic clamp event counter for a single axis.
pub struct ClampCounter {
    /// Total number of clamp events recorded
    clamp_events: AtomicU64,
    /// Timestamp of the last clamp event (UNIX millis)
    last_clamp_timestamp: AtomicU64,
    /// Minimum absolute clamped output value (stored as f64 bits)
    min_clamped_value: AtomicU64,
    /// Maximum absolute original value before clamping (stored as f64 bits)
    max_clamped_value: AtomicU64,
}

impl ClampCounter {
    /// Create a new zeroed clamp counter.
    pub fn new() -> Self {
        Self {
            clamp_events: AtomicU64::new(0),
            last_clamp_timestamp: AtomicU64::new(0),
            min_clamped_value: AtomicU64::new(f64::to_bits(f64::MAX)),
            max_clamped_value: AtomicU64::new(f64::to_bits(0.0)),
        }
    }

    /// Record a clamp event with the original and clamped values.
    pub fn record(&self, original_value: f64, clamped_value: f64) {
        self.clamp_events.fetch_add(1, Ordering::Relaxed);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_clamp_timestamp.store(now, Ordering::Relaxed);

        // Update min clamped value (CAS loop)
        let clamped_abs = clamped_value.abs();
        loop {
            let current = f64::from_bits(self.min_clamped_value.load(Ordering::Relaxed));
            if clamped_abs >= current {
                break;
            }
            if self
                .min_clamped_value
                .compare_exchange_weak(
                    current.to_bits(),
                    clamped_abs.to_bits(),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }

        // Update max clamped value (original value before clamp)
        let original_abs = original_value.abs();
        loop {
            let current = f64::from_bits(self.max_clamped_value.load(Ordering::Relaxed));
            if original_abs <= current {
                break;
            }
            if self
                .max_clamped_value
                .compare_exchange_weak(
                    current.to_bits(),
                    original_abs.to_bits(),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }
    }

    /// Number of recorded clamp events.
    pub fn clamp_events(&self) -> u64 {
        self.clamp_events.load(Ordering::Relaxed)
    }

    /// Timestamp (UNIX millis) of the last clamp event, or 0 if none.
    pub fn last_clamp_timestamp(&self) -> u64 {
        self.last_clamp_timestamp.load(Ordering::Relaxed)
    }

    /// Minimum absolute clamped output value, or 0.0 if none recorded.
    pub fn min_clamped_value(&self) -> f64 {
        let v = f64::from_bits(self.min_clamped_value.load(Ordering::Relaxed));
        if v == f64::MAX {
            0.0
        } else {
            v
        }
    }

    /// Maximum absolute original value observed before a clamp.
    pub fn max_clamped_value(&self) -> f64 {
        f64::from_bits(self.max_clamped_value.load(Ordering::Relaxed))
    }

    /// Reset all counters to initial state.
    pub fn reset(&self) {
        self.clamp_events.store(0, Ordering::Relaxed);
        self.last_clamp_timestamp.store(0, Ordering::Relaxed);
        self.min_clamped_value
            .store(f64::to_bits(f64::MAX), Ordering::Relaxed);
        self.max_clamped_value
            .store(f64::to_bits(0.0), Ordering::Relaxed);
    }
}

impl Default for ClampCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ClampCounter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClampCounter")
            .field("clamp_events", &self.clamp_events())
            .field("last_clamp_timestamp", &self.last_clamp_timestamp())
            .field("min_clamped_value", &self.min_clamped_value())
            .field("max_clamped_value", &self.max_clamped_value())
            .finish()
    }
}

/// Per-capability clamp statistics across all axes.
pub struct CapabilityMetrics {
    counters: RwLock<HashMap<String, Arc<ClampCounter>>>,
}

impl CapabilityMetrics {
    /// Create empty metrics.
    pub fn new() -> Self {
        Self {
            counters: RwLock::new(HashMap::new()),
        }
    }

    /// Record a clamp event for the given axis.
    pub fn record_clamp_event(&self, axis_id: &str, original_value: f64, clamped_value: f64) {
        // Fast path: read lock
        {
            let counters = self.counters.read().expect("metrics read lock");
            if let Some(counter) = counters.get(axis_id) {
                counter.record(original_value, clamped_value);
                return;
            }
        }
        // Slow path: write lock to insert new counter
        let mut counters = self.counters.write().expect("metrics write lock");
        let counter = counters
            .entry(axis_id.to_string())
            .or_insert_with(|| Arc::new(ClampCounter::new()));
        counter.record(original_value, clamped_value);
    }

    /// Get the clamp counter for a specific axis.
    pub fn get_counter(&self, axis_id: &str) -> Option<Arc<ClampCounter>> {
        self.counters
            .read()
            .expect("metrics read lock")
            .get(axis_id)
            .cloned()
    }

    /// Snapshot of all per-axis counters.
    pub fn all_counters(&self) -> HashMap<String, Arc<ClampCounter>> {
        self.counters.read().expect("metrics read lock").clone()
    }

    /// Reset counters for all axes.
    pub fn reset_all(&self) {
        for counter in self.counters.read().expect("metrics read lock").values() {
            counter.reset();
        }
    }

    /// Reset the counter for a specific axis.
    pub fn reset_axis(&self, axis_id: &str) {
        if let Some(counter) = self
            .counters
            .read()
            .expect("metrics read lock")
            .get(axis_id)
        {
            counter.reset();
        }
    }
}

impl Default for CapabilityMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-axis clamp statistics snapshot.
#[derive(Debug, Clone)]
pub struct AxisClampStats {
    pub axis_id: String,
    pub clamp_events: u64,
    pub last_clamp_timestamp_ms: u64,
    pub min_clamped_value: f64,
    pub max_clamped_value: f64,
}

/// Summary report of all capability clamp activity.
#[derive(Debug, Clone)]
pub struct CapabilityReport {
    /// Total clamp events across all axes (from engine counters).
    pub total_clamp_events: u64,
    /// Maximum pre-clamp value across all axes (from engine counters).
    pub max_value_before_clamp: f32,
    /// Per-axis clamp statistics (from service-level metrics).
    pub axes: Vec<AxisClampStats>,
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
            metrics: Arc::new(CapabilityMetrics::new()),
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

    /// Record a clamp event for a specific axis in the service-level metrics.
    pub fn record_clamp_event(&self, axis_id: &str, original_value: f64, clamped_value: f64) {
        self.metrics
            .record_clamp_event(axis_id, original_value, clamped_value);
    }

    /// Get the service-level capability metrics.
    pub fn metrics(&self) -> &CapabilityMetrics {
        &self.metrics
    }

    /// Build a capability report combining engine counters and service-level
    /// metrics into a single observability snapshot.
    pub fn get_capability_report(&self) -> Result<CapabilityReport, String> {
        let engines = self
            .engines
            .read()
            .map_err(|e| format!("Lock error: {}", e))?;

        let mut total_clamp_events: u64 = 0;
        let mut max_before_clamp: f32 = 0.0;

        // Collect engine-level totals
        for engine in engines.values() {
            total_clamp_events += engine.counters().capability_clamp_events();
            let v = engine.counters().max_value_before_clamp();
            if v > max_before_clamp {
                max_before_clamp = v;
            }
        }

        // Collect service-level per-axis stats
        let service_counters = self.metrics.all_counters();

        // Also add service-level clamp events to total
        for counter in service_counters.values() {
            total_clamp_events += counter.clamp_events();
        }

        let mut axes: Vec<AxisClampStats> = service_counters
            .iter()
            .map(|(axis_id, counter)| AxisClampStats {
                axis_id: axis_id.clone(),
                clamp_events: counter.clamp_events(),
                last_clamp_timestamp_ms: counter.last_clamp_timestamp(),
                min_clamped_value: counter.min_clamped_value(),
                max_clamped_value: counter.max_clamped_value(),
            })
            .collect();
        axes.sort_by(|a, b| a.axis_id.cmp(&b.axis_id));

        Ok(CapabilityReport {
            total_clamp_events,
            max_value_before_clamp: max_before_clamp,
            axes,
        })
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

    // --- New ClampCounter / CapabilityMetrics / report tests ---

    #[test]
    fn clamp_counter_struct_tracks_events() {
        let counter = ClampCounter::new();
        assert_eq!(counter.clamp_events(), 0);
        assert_eq!(counter.last_clamp_timestamp(), 0);
        assert_eq!(counter.min_clamped_value(), 0.0);
        assert_eq!(counter.max_clamped_value(), 0.0);

        counter.record(0.9, 0.5);
        assert_eq!(counter.clamp_events(), 1);
        assert!(counter.last_clamp_timestamp() > 0);
        assert!((counter.min_clamped_value() - 0.5).abs() < f64::EPSILON);
        assert!((counter.max_clamped_value() - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn clamp_counter_tracks_min_max() {
        let counter = ClampCounter::new();
        counter.record(0.9, 0.8);
        counter.record(0.95, 0.5);
        counter.record(0.7, 0.6);

        assert_eq!(counter.clamp_events(), 3);
        // min clamped is 0.5 (smallest output after clamp)
        assert!((counter.min_clamped_value() - 0.5).abs() < f64::EPSILON);
        // max clamped is 0.95 (largest original before clamp)
        assert!((counter.max_clamped_value() - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn clamp_counter_reset_clears_state() {
        let counter = ClampCounter::new();
        counter.record(0.9, 0.5);
        assert_eq!(counter.clamp_events(), 1);

        counter.reset();
        assert_eq!(counter.clamp_events(), 0);
        assert_eq!(counter.last_clamp_timestamp(), 0);
        assert_eq!(counter.min_clamped_value(), 0.0);
        assert_eq!(counter.max_clamped_value(), 0.0);
    }

    #[test]
    fn capability_metrics_records_per_axis() {
        let metrics = CapabilityMetrics::new();
        metrics.record_clamp_event("pitch", 0.9, 0.5);
        metrics.record_clamp_event("roll", 0.85, 0.8);

        let pitch = metrics.get_counter("pitch").expect("pitch counter");
        assert_eq!(pitch.clamp_events(), 1);
        assert!((pitch.max_clamped_value() - 0.9).abs() < f64::EPSILON);

        let roll = metrics.get_counter("roll").expect("roll counter");
        assert_eq!(roll.clamp_events(), 1);
        assert!((roll.max_clamped_value() - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn capability_metrics_creates_counter_on_first_event() {
        let metrics = CapabilityMetrics::new();
        assert!(metrics.get_counter("throttle").is_none());

        metrics.record_clamp_event("throttle", 0.9, 0.5);
        assert!(metrics.get_counter("throttle").is_some());
    }

    #[test]
    fn capability_metrics_reset_all() {
        let metrics = CapabilityMetrics::new();
        metrics.record_clamp_event("pitch", 0.9, 0.5);
        metrics.record_clamp_event("roll", 0.8, 0.5);
        metrics.reset_all();

        let pitch = metrics.get_counter("pitch").unwrap();
        assert_eq!(pitch.clamp_events(), 0);
        let roll = metrics.get_counter("roll").unwrap();
        assert_eq!(roll.clamp_events(), 0);
    }

    #[test]
    fn capability_metrics_reset_single_axis() {
        let metrics = CapabilityMetrics::new();
        metrics.record_clamp_event("pitch", 0.9, 0.5);
        metrics.record_clamp_event("roll", 0.8, 0.5);
        metrics.reset_axis("pitch");

        let pitch = metrics.get_counter("pitch").unwrap();
        assert_eq!(pitch.clamp_events(), 0);
        let roll = metrics.get_counter("roll").unwrap();
        assert_eq!(roll.clamp_events(), 1);
    }

    #[test]
    fn record_clamp_event_on_service() {
        let service = CapabilityService::new();
        service.record_clamp_event("pitch", 0.9, 0.5);
        service.record_clamp_event("pitch", 0.95, 0.5);
        service.record_clamp_event("roll", 0.8, 0.5);

        let pitch = service.metrics().get_counter("pitch").unwrap();
        assert_eq!(pitch.clamp_events(), 2);
        let roll = service.metrics().get_counter("roll").unwrap();
        assert_eq!(roll.clamp_events(), 1);
    }

    #[test]
    fn record_clamp_event_sets_timestamps() {
        let service = CapabilityService::new();
        service.record_clamp_event("yaw", 0.9, 0.5);

        let yaw = service.metrics().get_counter("yaw").unwrap();
        assert!(yaw.last_clamp_timestamp() > 0);
    }

    #[test]
    fn demo_mode_clamping_records_service_event() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("roll".to_string()));
        service
            .register_axis("roll".to_string(), engine.clone())
            .unwrap();

        service.set_demo_mode(true).unwrap();

        let mut frame = AxisFrame::new(0.95, 1000);
        frame.out = 0.95;
        engine.process(&mut frame).unwrap();

        // Engine-level counter should fire
        assert_eq!(engine.counters().capability_clamp_events(), 1);

        // Also record at service level for report
        service.record_clamp_event("roll", 0.95, f64::from(frame.out));

        let report = service.get_capability_report().unwrap();
        // 1 from engine + 1 from service-level
        assert_eq!(report.total_clamp_events, 2);
        assert_eq!(report.axes.len(), 1);
        assert_eq!(report.axes[0].axis_id, "roll");
        assert_eq!(report.axes[0].clamp_events, 1);
    }

    #[test]
    fn kid_mode_clamping_records_service_event() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("throttle".to_string()));
        service
            .register_axis("throttle".to_string(), engine.clone())
            .unwrap();

        service.set_kid_mode(true).unwrap();

        let mut frame = AxisFrame::new(0.8, 1000);
        frame.out = 0.8;
        engine.process(&mut frame).unwrap();

        assert_eq!(engine.counters().capability_clamp_events(), 1);

        service.record_clamp_event("throttle", 0.8, f64::from(frame.out));

        let report = service.get_capability_report().unwrap();
        assert_eq!(report.total_clamp_events, 2);
        assert_eq!(report.axes[0].axis_id, "throttle");
    }

    #[test]
    fn capability_report_format_with_no_events() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));
        service
            .register_axis("pitch".to_string(), engine)
            .unwrap();

        let report = service.get_capability_report().unwrap();
        assert_eq!(report.total_clamp_events, 0);
        assert_eq!(report.max_value_before_clamp, 0.0);
        assert!(report.axes.is_empty());
    }

    #[test]
    fn capability_report_axes_sorted_by_id() {
        let service = CapabilityService::new();
        service.record_clamp_event("yaw", 0.9, 0.5);
        service.record_clamp_event("pitch", 0.9, 0.5);
        service.record_clamp_event("roll", 0.9, 0.5);

        let report = service.get_capability_report().unwrap();
        assert_eq!(report.axes.len(), 3);
        assert_eq!(report.axes[0].axis_id, "pitch");
        assert_eq!(report.axes[1].axis_id, "roll");
        assert_eq!(report.axes[2].axis_id, "yaw");
    }

    #[test]
    fn multiple_axes_tracked_independently_in_report() {
        let service = CapabilityService::new();
        service.record_clamp_event("pitch", 0.9, 0.5);
        service.record_clamp_event("pitch", 0.95, 0.5);
        service.record_clamp_event("roll", 0.85, 0.8);

        let report = service.get_capability_report().unwrap();
        assert_eq!(report.axes.len(), 2);

        let pitch = report.axes.iter().find(|a| a.axis_id == "pitch").unwrap();
        assert_eq!(pitch.clamp_events, 2);
        assert!((pitch.max_clamped_value - 0.95).abs() < f64::EPSILON);
        assert!((pitch.min_clamped_value - 0.5).abs() < f64::EPSILON);

        let roll = report.axes.iter().find(|a| a.axis_id == "roll").unwrap();
        assert_eq!(roll.clamp_events, 1);
        assert!((roll.max_clamped_value - 0.85).abs() < f64::EPSILON);
        assert!((roll.min_clamped_value - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn clamp_counter_timestamps_advance() {
        let counter = ClampCounter::new();
        counter.record(0.9, 0.5);
        let ts1 = counter.last_clamp_timestamp();

        std::thread::sleep(std::time::Duration::from_millis(2));

        counter.record(0.9, 0.5);
        let ts2 = counter.last_clamp_timestamp();
        assert!(ts2 >= ts1, "timestamps must be non-decreasing");
    }
}
