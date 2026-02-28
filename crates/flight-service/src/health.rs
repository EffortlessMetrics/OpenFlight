// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Health Stream and Monitoring
//!
//! Provides real-time health monitoring and event streaming for all
//! Flight Hub components with stable error codes and diagnostics.

use crate::error_taxonomy::ErrorCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, broadcast};
use tracing::{debug, error, warn};

/// Health event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthEvent {
    /// Unique event ID
    pub id: String,
    /// Timestamp when event occurred
    pub timestamp: u64,
    /// Component that generated the event
    pub component: String,
    /// Event severity level
    pub severity: HealthSeverity,
    /// Event category
    pub category: HealthCategory,
    /// Human-readable message
    pub message: String,
    /// Stable error code if applicable
    pub error_code: Option<ErrorCode>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Health severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthSeverity {
    /// Informational events
    Info,
    /// Warning conditions that don't affect operation
    Warning,
    /// Error conditions that may affect operation
    Error,
    /// Critical conditions requiring immediate attention
    Critical,
}

/// Health event categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthCategory {
    /// System startup and shutdown events
    System,
    /// Real-time performance events
    Performance,
    /// Device connection and communication events
    Device,
    /// Safety system events
    Safety,
    /// Configuration and profile events
    Configuration,
    /// Plugin and extension events
    Plugin,
    /// Simulator integration events
    Simulator,
}

/// Overall health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall system health
    pub overall: ComponentHealth,
    /// Individual component health
    pub components: HashMap<String, ComponentHealth>,
    /// Recent events (last 100)
    pub recent_events: Vec<HealthEvent>,
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Last update timestamp
    pub last_update: u64,
}

/// Component health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,
    /// Current health state
    pub state: HealthState,
    /// Last seen timestamp
    pub last_seen: u64,
    /// Error count in last minute
    pub error_count: u32,
    /// Warning count in last minute
    pub warning_count: u32,
    /// Additional status information
    pub status_info: HashMap<String, String>,
}

/// Component health states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthState {
    /// Component is healthy and operating normally
    Healthy,
    /// Component has warnings but is still functional
    Warning,
    /// Component has errors but may still be partially functional
    Degraded,
    /// Component is not functional
    Failed,
    /// Component status is unknown
    Unknown,
}

/// Health stream manager
pub struct HealthStream {
    /// Event broadcaster
    event_tx: broadcast::Sender<HealthEvent>,
    /// Component health tracking
    components: Arc<RwLock<HashMap<String, ComponentHealth>>>,
    /// Recent events buffer
    recent_events: Arc<RwLock<Vec<HealthEvent>>>,
    /// System start time
    start_time: Instant,
    /// Event counter for unique IDs
    event_counter: Arc<RwLock<u64>>,
}

impl HealthStream {
    /// Create new health stream
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(1000);

        Self {
            event_tx,
            components: Arc::new(RwLock::new(HashMap::new())),
            recent_events: Arc::new(RwLock::new(Vec::new())),
            start_time: Instant::now(),
            event_counter: Arc::new(RwLock::new(0)),
        }
    }

    /// Subscribe to health events
    pub fn subscribe(&self) -> broadcast::Receiver<HealthEvent> {
        self.event_tx.subscribe()
    }

    /// Emit a health event
    pub async fn emit_event(
        &self,
        component: &str,
        severity: HealthSeverity,
        category: HealthCategory,
        message: &str,
        error_code: Option<ErrorCode>,
        metadata: HashMap<String, String>,
    ) {
        let mut counter = self.event_counter.write().await;
        *counter += 1;
        let event_id = format!("evt_{:08x}", *counter);
        drop(counter);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let event = HealthEvent {
            id: event_id,
            timestamp,
            component: component.to_string(),
            severity,
            category,
            message: message.to_string(),
            error_code,
            metadata,
        };

        // Update component health
        self.update_component_health(&event).await;

        // Add to recent events
        let mut recent = self.recent_events.write().await;
        recent.push(event.clone());
        if recent.len() > 100 {
            recent.remove(0);
        }
        drop(recent);

        // Broadcast event
        if self.event_tx.send(event.clone()).is_err() {
            // No subscribers, which is fine
            debug!("No health event subscribers");
        }

        // Log based on severity
        match severity {
            HealthSeverity::Info => debug!("[{}] {}", component, message),
            HealthSeverity::Warning => warn!("[{}] {}", component, message),
            HealthSeverity::Error | HealthSeverity::Critical => {
                error!("[{}] {}", component, message)
            }
        }
    }

    /// Emit info event
    pub async fn info(&self, component: &str, message: &str) {
        self.emit_event(
            component,
            HealthSeverity::Info,
            HealthCategory::System,
            message,
            None,
            HashMap::new(),
        )
        .await;
    }

    /// Emit warning event
    pub async fn warning(&self, component: &str, message: &str) {
        self.emit_event(
            component,
            HealthSeverity::Warning,
            HealthCategory::System,
            message,
            None,
            HashMap::new(),
        )
        .await;
    }

    /// Emit error event
    pub async fn error(&self, component: &str, message: &str, error_code: Option<ErrorCode>) {
        self.emit_event(
            component,
            HealthSeverity::Error,
            HealthCategory::System,
            message,
            error_code,
            HashMap::new(),
        )
        .await;
    }

    /// Emit critical event
    pub async fn critical(&self, component: &str, message: &str, error_code: Option<ErrorCode>) {
        self.emit_event(
            component,
            HealthSeverity::Critical,
            HealthCategory::Safety,
            message,
            error_code,
            HashMap::new(),
        )
        .await;
    }

    /// Update component health based on event
    async fn update_component_health(&self, event: &HealthEvent) {
        let mut components = self.components.write().await;

        let health = components
            .entry(event.component.clone())
            .or_insert_with(|| ComponentHealth {
                name: event.component.clone(),
                state: HealthState::Healthy,
                last_seen: event.timestamp,
                error_count: 0,
                warning_count: 0,
                status_info: HashMap::new(),
            });

        health.last_seen = event.timestamp;

        // Update counters and state based on severity
        match event.severity {
            HealthSeverity::Info => {
                // Info events don't change health state
            }
            HealthSeverity::Warning => {
                health.warning_count += 1;
                if health.state == HealthState::Healthy {
                    health.state = HealthState::Warning;
                }
            }
            HealthSeverity::Error => {
                health.error_count += 1;
                if matches!(health.state, HealthState::Healthy | HealthState::Warning) {
                    health.state = HealthState::Degraded;
                }
            }
            HealthSeverity::Critical => {
                health.error_count += 1;
                health.state = HealthState::Failed;
            }
        }

        // Add error code to status info if present
        if let Some(error_code) = &event.error_code {
            health
                .status_info
                .insert("last_error_code".to_string(), error_code.to_string());
        }
    }

    /// Register a component
    pub async fn register_component(&self, name: &str) {
        let mut components = self.components.write().await;

        if !components.contains_key(name) {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            components.insert(
                name.to_string(),
                ComponentHealth {
                    name: name.to_string(),
                    state: HealthState::Healthy,
                    last_seen: timestamp,
                    error_count: 0,
                    warning_count: 0,
                    status_info: HashMap::new(),
                },
            );

            debug!("Registered component: {}", name);
        }
    }

    /// Update component status information
    pub async fn update_component_status(&self, name: &str, key: &str, value: &str) {
        let mut components = self.components.write().await;

        if let Some(health) = components.get_mut(name) {
            health
                .status_info
                .insert(key.to_string(), value.to_string());

            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            health.last_seen = timestamp;
        }
    }

    /// Get current health status
    pub async fn get_health_status(&self) -> HealthStatus {
        let components = self.components.read().await;
        let recent_events = self.recent_events.read().await;

        // Determine overall health
        let overall_state = if components.values().any(|c| c.state == HealthState::Failed) {
            HealthState::Failed
        } else if components
            .values()
            .any(|c| c.state == HealthState::Degraded)
        {
            HealthState::Degraded
        } else if components.values().any(|c| c.state == HealthState::Warning) {
            HealthState::Warning
        } else {
            HealthState::Healthy
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        HealthStatus {
            overall: ComponentHealth {
                name: "System".to_string(),
                state: overall_state,
                last_seen: timestamp,
                error_count: components.values().map(|c| c.error_count).sum(),
                warning_count: components.values().map(|c| c.warning_count).sum(),
                status_info: HashMap::new(),
            },
            components: components.clone(),
            recent_events: recent_events.clone(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            last_update: timestamp,
        }
    }

    /// Reset component health counters (called periodically)
    pub async fn reset_counters(&self) {
        let mut components = self.components.write().await;

        for health in components.values_mut() {
            health.error_count = 0;
            health.warning_count = 0;

            // Reset state to healthy if no recent issues
            if matches!(health.state, HealthState::Warning | HealthState::Degraded) {
                health.state = HealthState::Healthy;
            }
        }

        debug!("Reset health counters for all components");
    }

    /// Start periodic health maintenance
    pub fn start_maintenance_task(&self) -> tokio::task::JoinHandle<()> {
        let components = Arc::clone(&self.components);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                // Reset counters every minute
                let mut comps = components.write().await;
                for health in comps.values_mut() {
                    health.error_count = 0;
                    health.warning_count = 0;

                    // Auto-heal warnings and degraded states after 1 minute
                    if matches!(health.state, HealthState::Warning | HealthState::Degraded) {
                        health.state = HealthState::Healthy;
                    }
                }
                drop(comps);

                debug!("Health maintenance completed");
            }
        })
    }
}

impl Default for HealthStream {
    fn default() -> Self {
        Self::new()
    }
}

// ── Health checker ────────────────────────────────────────────────────────

/// Overall system status determined by health checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverallStatus {
    Healthy,
    Degraded,
    Critical,
}

impl OverallStatus {
    fn severity(self) -> u8 {
        match self {
            Self::Healthy => 0,
            Self::Degraded => 1,
            Self::Critical => 2,
        }
    }

    /// Return the worse of two statuses.
    #[must_use]
    pub fn worse(self, other: Self) -> Self {
        if self.severity() >= other.severity() {
            self
        } else {
            other
        }
    }
}

impl std::fmt::Display for OverallStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => f.write_str("Healthy"),
            Self::Degraded => f.write_str("Degraded"),
            Self::Critical => f.write_str("Critical"),
        }
    }
}

/// Result of a single health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: OverallStatus,
    pub message: String,
    pub latency_ms: f64,
}

/// Aggregated health check report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckReport {
    pub status: OverallStatus,
    pub checks: Vec<HealthCheck>,
    pub recommendations: Vec<String>,
}

impl HealthCheckReport {
    /// Serialize the report as a JSON string.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

/// Input data for device health evaluation.
#[derive(Debug, Clone)]
pub struct DeviceHealthInput {
    pub name: String,
    pub connected: bool,
    pub error: Option<String>,
}

/// Input data for adapter health evaluation.
#[derive(Debug, Clone)]
pub struct AdapterHealthInput {
    pub name: String,
    pub connected: bool,
    pub error: Option<String>,
}

/// Input data for scheduler health evaluation.
#[derive(Debug, Clone)]
pub struct SchedulerHealthInput {
    pub running: bool,
    pub jitter_p99_us: f64,
    pub overrun_count: u64,
}

/// Input data for memory health evaluation.
#[derive(Debug, Clone)]
pub struct MemoryHealthInput {
    pub used_mb: u64,
    pub total_mb: u64,
}

/// Runs health checks against provided subsystem data.
pub struct HealthChecker {
    devices: Vec<DeviceHealthInput>,
    adapters: Vec<AdapterHealthInput>,
    scheduler: Option<SchedulerHealthInput>,
    memory: Option<MemoryHealthInput>,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            adapters: Vec::new(),
            scheduler: None,
            memory: None,
        }
    }

    pub fn set_devices(&mut self, devices: Vec<DeviceHealthInput>) -> &mut Self {
        self.devices = devices;
        self
    }

    pub fn set_adapters(&mut self, adapters: Vec<AdapterHealthInput>) -> &mut Self {
        self.adapters = adapters;
        self
    }

    pub fn set_scheduler(&mut self, input: SchedulerHealthInput) -> &mut Self {
        self.scheduler = Some(input);
        self
    }

    pub fn set_memory(&mut self, input: MemoryHealthInput) -> &mut Self {
        self.memory = Some(input);
        self
    }

    /// Run all health checks and produce an aggregated report.
    pub fn check_all(&self) -> HealthCheckReport {
        let mut checks = Vec::new();
        let mut recommendations = Vec::new();

        let device_check = self.device_health();
        if device_check.status != OverallStatus::Healthy {
            recommendations.push(format!("Device issue: {}", device_check.message));
        }
        checks.push(device_check);

        let adapter_check = self.adapter_health();
        if adapter_check.status != OverallStatus::Healthy {
            recommendations.push(format!("Adapter issue: {}", adapter_check.message));
        }
        checks.push(adapter_check);

        let sched_check = self.scheduler_health();
        if sched_check.status != OverallStatus::Healthy {
            recommendations.push(format!("Scheduler issue: {}", sched_check.message));
        }
        checks.push(sched_check);

        let mem_check = self.memory_health();
        if mem_check.status != OverallStatus::Healthy {
            recommendations.push(format!("Memory issue: {}", mem_check.message));
        }
        checks.push(mem_check);

        let status = checks
            .iter()
            .fold(OverallStatus::Healthy, |acc, c| acc.worse(c.status));

        HealthCheckReport {
            status,
            checks,
            recommendations,
        }
    }

    /// Check the health of connected devices.
    pub fn device_health(&self) -> HealthCheck {
        let start = std::time::Instant::now();
        if self.devices.is_empty() {
            return HealthCheck {
                name: "devices".into(),
                status: OverallStatus::Healthy,
                message: "No devices configured".into(),
                latency_ms: start.elapsed().as_secs_f64() * 1000.0,
            };
        }

        let disconnected: Vec<_> = self.devices.iter().filter(|d| !d.connected).collect();
        let with_errors: Vec<_> = self.devices.iter().filter(|d| d.error.is_some()).collect();

        let (status, message) = if disconnected.len() == self.devices.len() {
            (OverallStatus::Critical, "All devices disconnected".into())
        } else if !disconnected.is_empty() || !with_errors.is_empty() {
            (
                OverallStatus::Degraded,
                format!(
                    "{} disconnected, {} with errors",
                    disconnected.len(),
                    with_errors.len()
                ),
            )
        } else {
            (
                OverallStatus::Healthy,
                format!("All {} devices connected", self.devices.len()),
            )
        };

        HealthCheck {
            name: "devices".into(),
            status,
            message,
            latency_ms: start.elapsed().as_secs_f64() * 1000.0,
        }
    }

    /// Check the health of simulator adapters.
    pub fn adapter_health(&self) -> HealthCheck {
        let start = std::time::Instant::now();
        if self.adapters.is_empty() {
            return HealthCheck {
                name: "adapters".into(),
                status: OverallStatus::Healthy,
                message: "No adapters configured".into(),
                latency_ms: start.elapsed().as_secs_f64() * 1000.0,
            };
        }

        let disconnected: Vec<_> = self.adapters.iter().filter(|a| !a.connected).collect();

        let (status, message) = if disconnected.len() == self.adapters.len() {
            (OverallStatus::Critical, "All adapters disconnected".into())
        } else if !disconnected.is_empty() {
            (
                OverallStatus::Degraded,
                format!(
                    "{} of {} adapters disconnected",
                    disconnected.len(),
                    self.adapters.len()
                ),
            )
        } else {
            (
                OverallStatus::Healthy,
                format!("All {} adapters connected", self.adapters.len()),
            )
        };

        HealthCheck {
            name: "adapters".into(),
            status,
            message,
            latency_ms: start.elapsed().as_secs_f64() * 1000.0,
        }
    }

    /// Check the health of the RT scheduler.
    pub fn scheduler_health(&self) -> HealthCheck {
        let start = std::time::Instant::now();
        let Some(ref sched) = self.scheduler else {
            return HealthCheck {
                name: "scheduler".into(),
                status: OverallStatus::Healthy,
                message: "No scheduler data".into(),
                latency_ms: start.elapsed().as_secs_f64() * 1000.0,
            };
        };

        let (status, message) = if !sched.running {
            (OverallStatus::Critical, "Scheduler is not running".into())
        } else if sched.jitter_p99_us > 500.0 || sched.overrun_count > 10 {
            (
                OverallStatus::Degraded,
                format!(
                    "High jitter ({:.0} µs p99) or overruns ({})",
                    sched.jitter_p99_us, sched.overrun_count
                ),
            )
        } else {
            (
                OverallStatus::Healthy,
                format!(
                    "Running, jitter {:.0} µs p99, {} overruns",
                    sched.jitter_p99_us, sched.overrun_count
                ),
            )
        };

        HealthCheck {
            name: "scheduler".into(),
            status,
            message,
            latency_ms: start.elapsed().as_secs_f64() * 1000.0,
        }
    }

    /// Check the memory health of the system.
    pub fn memory_health(&self) -> HealthCheck {
        let start = std::time::Instant::now();
        let Some(ref mem) = self.memory else {
            return HealthCheck {
                name: "memory".into(),
                status: OverallStatus::Healthy,
                message: "No memory data".into(),
                latency_ms: start.elapsed().as_secs_f64() * 1000.0,
            };
        };

        let usage_pct = if mem.total_mb > 0 {
            (mem.used_mb as f64 / mem.total_mb as f64) * 100.0
        } else {
            0.0
        };

        let (status, message) = if usage_pct > 95.0 {
            (
                OverallStatus::Critical,
                format!("Memory usage critical: {:.1}%", usage_pct),
            )
        } else if usage_pct > 80.0 {
            (
                OverallStatus::Degraded,
                format!("Memory usage high: {:.1}%", usage_pct),
            )
        } else {
            (
                OverallStatus::Healthy,
                format!("Memory usage normal: {:.1}%", usage_pct),
            )
        };

        HealthCheck {
            name: "memory".into(),
            status,
            message,
            latency_ms: start.elapsed().as_secs_f64() * 1000.0,
        }
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_stream_creation() {
        let health = HealthStream::new();
        let status = health.get_health_status().await;

        assert_eq!(status.overall.state, HealthState::Healthy);
        assert!(status.components.is_empty());
        assert!(status.recent_events.is_empty());
    }

    #[tokio::test]
    async fn test_component_registration() {
        let health = HealthStream::new();

        health.register_component("test_component").await;
        let status = health.get_health_status().await;

        assert!(status.components.contains_key("test_component"));
        assert_eq!(
            status.components["test_component"].state,
            HealthState::Healthy
        );
    }

    #[tokio::test]
    async fn test_event_emission() {
        let health = HealthStream::new();
        let mut rx = health.subscribe();

        health.register_component("test").await;
        health.warning("test", "Test warning").await;

        let event = rx.recv().await.unwrap();
        assert_eq!(event.component, "test");
        assert_eq!(event.severity, HealthSeverity::Warning);
        assert_eq!(event.message, "Test warning");
    }

    #[tokio::test]
    async fn test_health_state_transitions() {
        let health = HealthStream::new();

        health.register_component("test").await;

        // Start healthy
        let status = health.get_health_status().await;
        assert_eq!(status.components["test"].state, HealthState::Healthy);

        // Warning should change to Warning state
        health.warning("test", "Warning").await;
        let status = health.get_health_status().await;
        assert_eq!(status.components["test"].state, HealthState::Warning);

        // Error should change to Degraded state
        health.error("test", "Error", None).await;
        let status = health.get_health_status().await;
        assert_eq!(status.components["test"].state, HealthState::Degraded);

        // Critical should change to Failed state
        health.critical("test", "Critical", None).await;
        let status = health.get_health_status().await;
        assert_eq!(status.components["test"].state, HealthState::Failed);
    }

    // ── HealthChecker tests ──────────────────────────────────────────────

    #[test]
    fn test_check_all_healthy() {
        let mut checker = HealthChecker::new();
        checker
            .set_devices(vec![
                DeviceHealthInput {
                    name: "Stick".into(),
                    connected: true,
                    error: None,
                },
                DeviceHealthInput {
                    name: "Throttle".into(),
                    connected: true,
                    error: None,
                },
            ])
            .set_adapters(vec![AdapterHealthInput {
                name: "MSFS".into(),
                connected: true,
                error: None,
            }])
            .set_scheduler(SchedulerHealthInput {
                running: true,
                jitter_p99_us: 100.0,
                overrun_count: 0,
            })
            .set_memory(MemoryHealthInput {
                used_mb: 4096,
                total_mb: 32768,
            });

        let report = checker.check_all();
        assert_eq!(report.status, OverallStatus::Healthy);
        assert_eq!(report.checks.len(), 4);
        assert!(report.recommendations.is_empty());
    }

    #[test]
    fn test_check_all_degraded() {
        let mut checker = HealthChecker::new();
        checker
            .set_devices(vec![
                DeviceHealthInput {
                    name: "Stick".into(),
                    connected: true,
                    error: None,
                },
                DeviceHealthInput {
                    name: "Throttle".into(),
                    connected: false,
                    error: Some("USB disconnect".into()),
                },
            ])
            .set_adapters(vec![AdapterHealthInput {
                name: "MSFS".into(),
                connected: true,
                error: None,
            }])
            .set_scheduler(SchedulerHealthInput {
                running: true,
                jitter_p99_us: 200.0,
                overrun_count: 0,
            })
            .set_memory(MemoryHealthInput {
                used_mb: 4096,
                total_mb: 32768,
            });

        let report = checker.check_all();
        assert_eq!(report.status, OverallStatus::Degraded);
        assert!(!report.recommendations.is_empty());
    }

    #[test]
    fn test_check_all_critical() {
        let mut checker = HealthChecker::new();
        checker
            .set_devices(vec![DeviceHealthInput {
                name: "Stick".into(),
                connected: false,
                error: Some("missing".into()),
            }])
            .set_adapters(vec![AdapterHealthInput {
                name: "MSFS".into(),
                connected: false,
                error: Some("timeout".into()),
            }])
            .set_scheduler(SchedulerHealthInput {
                running: false,
                jitter_p99_us: 0.0,
                overrun_count: 0,
            })
            .set_memory(MemoryHealthInput {
                used_mb: 31000,
                total_mb: 32768,
            });

        let report = checker.check_all();
        assert_eq!(report.status, OverallStatus::Critical);
        assert!(report.recommendations.len() >= 3);
    }

    #[test]
    fn test_device_health_all_connected() {
        let mut checker = HealthChecker::new();
        checker.set_devices(vec![
            DeviceHealthInput {
                name: "A".into(),
                connected: true,
                error: None,
            },
            DeviceHealthInput {
                name: "B".into(),
                connected: true,
                error: None,
            },
        ]);
        let check = checker.device_health();
        assert_eq!(check.status, OverallStatus::Healthy);
        assert!(check.message.contains("2 devices connected"));
    }

    #[test]
    fn test_device_health_some_disconnected() {
        let mut checker = HealthChecker::new();
        checker.set_devices(vec![
            DeviceHealthInput {
                name: "A".into(),
                connected: true,
                error: None,
            },
            DeviceHealthInput {
                name: "B".into(),
                connected: false,
                error: None,
            },
        ]);
        let check = checker.device_health();
        assert_eq!(check.status, OverallStatus::Degraded);
    }

    #[test]
    fn test_device_health_all_disconnected() {
        let mut checker = HealthChecker::new();
        checker.set_devices(vec![DeviceHealthInput {
            name: "A".into(),
            connected: false,
            error: None,
        }]);
        let check = checker.device_health();
        assert_eq!(check.status, OverallStatus::Critical);
    }

    #[test]
    fn test_device_health_empty() {
        let checker = HealthChecker::new();
        let check = checker.device_health();
        assert_eq!(check.status, OverallStatus::Healthy);
        assert!(check.message.contains("No devices"));
    }

    #[test]
    fn test_adapter_health_all_connected() {
        let mut checker = HealthChecker::new();
        checker.set_adapters(vec![AdapterHealthInput {
            name: "MSFS".into(),
            connected: true,
            error: None,
        }]);
        let check = checker.adapter_health();
        assert_eq!(check.status, OverallStatus::Healthy);
    }

    #[test]
    fn test_adapter_health_disconnected() {
        let mut checker = HealthChecker::new();
        checker.set_adapters(vec![
            AdapterHealthInput {
                name: "MSFS".into(),
                connected: true,
                error: None,
            },
            AdapterHealthInput {
                name: "XP".into(),
                connected: false,
                error: Some("timeout".into()),
            },
        ]);
        let check = checker.adapter_health();
        assert_eq!(check.status, OverallStatus::Degraded);
    }

    #[test]
    fn test_adapter_health_all_disconnected() {
        let mut checker = HealthChecker::new();
        checker.set_adapters(vec![AdapterHealthInput {
            name: "MSFS".into(),
            connected: false,
            error: None,
        }]);
        let check = checker.adapter_health();
        assert_eq!(check.status, OverallStatus::Critical);
    }

    #[test]
    fn test_scheduler_health_running() {
        let mut checker = HealthChecker::new();
        checker.set_scheduler(SchedulerHealthInput {
            running: true,
            jitter_p99_us: 100.0,
            overrun_count: 0,
        });
        let check = checker.scheduler_health();
        assert_eq!(check.status, OverallStatus::Healthy);
    }

    #[test]
    fn test_scheduler_health_not_running() {
        let mut checker = HealthChecker::new();
        checker.set_scheduler(SchedulerHealthInput {
            running: false,
            jitter_p99_us: 0.0,
            overrun_count: 0,
        });
        let check = checker.scheduler_health();
        assert_eq!(check.status, OverallStatus::Critical);
    }

    #[test]
    fn test_scheduler_health_high_jitter() {
        let mut checker = HealthChecker::new();
        checker.set_scheduler(SchedulerHealthInput {
            running: true,
            jitter_p99_us: 600.0,
            overrun_count: 0,
        });
        let check = checker.scheduler_health();
        assert_eq!(check.status, OverallStatus::Degraded);
    }

    #[test]
    fn test_scheduler_health_many_overruns() {
        let mut checker = HealthChecker::new();
        checker.set_scheduler(SchedulerHealthInput {
            running: true,
            jitter_p99_us: 100.0,
            overrun_count: 20,
        });
        let check = checker.scheduler_health();
        assert_eq!(check.status, OverallStatus::Degraded);
    }

    #[test]
    fn test_memory_health_normal() {
        let mut checker = HealthChecker::new();
        checker.set_memory(MemoryHealthInput {
            used_mb: 8000,
            total_mb: 32768,
        });
        let check = checker.memory_health();
        assert_eq!(check.status, OverallStatus::Healthy);
    }

    #[test]
    fn test_memory_health_high() {
        let mut checker = HealthChecker::new();
        checker.set_memory(MemoryHealthInput {
            used_mb: 27000,
            total_mb: 32768,
        });
        let check = checker.memory_health();
        assert_eq!(check.status, OverallStatus::Degraded);
    }

    #[test]
    fn test_memory_health_critical() {
        let mut checker = HealthChecker::new();
        checker.set_memory(MemoryHealthInput {
            used_mb: 31500,
            total_mb: 32768,
        });
        let check = checker.memory_health();
        assert_eq!(check.status, OverallStatus::Critical);
    }

    #[test]
    fn test_health_check_latency_recorded() {
        let checker = HealthChecker::new();
        let check = checker.device_health();
        assert!(check.latency_ms >= 0.0);
    }

    #[test]
    fn test_overall_status_ordering() {
        assert_eq!(
            OverallStatus::Healthy.worse(OverallStatus::Degraded),
            OverallStatus::Degraded
        );
        assert_eq!(
            OverallStatus::Critical.worse(OverallStatus::Healthy),
            OverallStatus::Critical
        );
        assert_eq!(
            OverallStatus::Healthy.worse(OverallStatus::Healthy),
            OverallStatus::Healthy
        );
        assert_eq!(
            OverallStatus::Degraded.worse(OverallStatus::Critical),
            OverallStatus::Critical
        );
    }

    #[test]
    fn test_health_check_report_json_roundtrip() {
        let report = HealthCheckReport {
            status: OverallStatus::Degraded,
            checks: vec![HealthCheck {
                name: "devices".into(),
                status: OverallStatus::Degraded,
                message: "1 device disconnected".into(),
                latency_ms: 0.5,
            }],
            recommendations: vec!["Check USB connections".into()],
        };
        let json = report.to_json().unwrap();
        let restored: HealthCheckReport = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.status, OverallStatus::Degraded);
        assert_eq!(restored.checks.len(), 1);
        assert_eq!(restored.recommendations.len(), 1);
    }

    #[test]
    fn test_empty_health_checker() {
        let checker = HealthChecker::new();
        let report = checker.check_all();
        assert_eq!(report.status, OverallStatus::Healthy);
        assert_eq!(report.checks.len(), 4);
        assert!(report.recommendations.is_empty());
    }

    #[test]
    fn test_overall_status_display() {
        assert_eq!(OverallStatus::Healthy.to_string(), "Healthy");
        assert_eq!(OverallStatus::Degraded.to_string(), "Degraded");
        assert_eq!(OverallStatus::Critical.to_string(), "Critical");
    }

    #[test]
    fn test_recommendations_generated_for_failures() {
        let mut checker = HealthChecker::new();
        checker
            .set_devices(vec![DeviceHealthInput {
                name: "Stick".into(),
                connected: false,
                error: None,
            }])
            .set_scheduler(SchedulerHealthInput {
                running: true,
                jitter_p99_us: 800.0,
                overrun_count: 0,
            });
        let report = checker.check_all();
        assert!(report.recommendations.len() >= 2);
        assert!(report.recommendations.iter().any(|r| r.contains("Device")));
        assert!(
            report
                .recommendations
                .iter()
                .any(|r| r.contains("Scheduler"))
        );
    }
}
