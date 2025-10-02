//! Health Stream and Monitoring
//!
//! Provides real-time health monitoring and event streaming for all
//! Flight Hub components with stable error codes and diagnostics.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, warn, error};
use crate::error_taxonomy::{ErrorCode, ErrorCategory, StableError};

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
        if let Err(_) = self.event_tx.send(event.clone()) {
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
        ).await;
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
        ).await;
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
        ).await;
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
        ).await;
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
            health.status_info.insert(
                "last_error_code".to_string(),
                error_code.to_string(),
            );
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
            health.status_info.insert(key.to_string(), value.to_string());
            
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
        } else if components.values().any(|c| c.state == HealthState::Degraded) {
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
}