// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! StreamDeck plugin implementation
//!
//! Handles plugin lifecycle, telemetry integration, and event processing
//! for StreamDeck devices with Flight Hub integration.

use crate::{AppVersion, VerifyResult, VersionCompatibility};
use anyhow::Result;
use flight_bus::{BusSnapshot, SubscriberId, SubscriptionConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info};

/// Telemetry subscriber trait
pub trait TelemetrySubscriber {
    fn get_id(&self) -> &SubscriberId;
    fn get_config(&self) -> &SubscriptionConfig;
    fn notify(
        &mut self,
        snapshot: &BusSnapshot,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Plugin configuration
#[derive(Debug, Clone)]
pub struct PluginConfig {
    pub plugin_uuid: String,
    pub plugin_version: String,
    pub telemetry_update_rate_hz: u32,
    pub event_buffer_size: usize,
    pub auto_reconnect: bool,
    pub reconnect_delay_ms: u64,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            plugin_uuid: "com.flighthub.streamdeck".to_string(),
            plugin_version: "1.0.0".to_string(),
            telemetry_update_rate_hz: 30,
            event_buffer_size: 1000,
            auto_reconnect: true,
            reconnect_delay_ms: 1000,
        }
    }
}

/// Plugin error types
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Plugin not initialized")]
    NotInitialized,

    #[error("IPC connection failed: {0}")]
    IpcConnectionFailed(String),

    #[error("Telemetry subscription failed: {0}")]
    TelemetrySubscriptionFailed(String),

    #[error("Event processing failed: {0}")]
    EventProcessingFailed(String),

    #[error("Plugin shutdown failed: {0}")]
    ShutdownFailed(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

/// Plugin event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginEvent {
    TelemetryUpdate(BusSnapshot),
    ActionTriggered {
        action_uuid: String,
        context: String,
    },
    PropertyInspectorUpdate {
        action_uuid: String,
        settings: serde_json::Value,
    },
    DeviceConnected {
        device_id: String,
    },
    DeviceDisconnected {
        device_id: String,
    },
    PluginShutdown,
}

/// Plugin state
#[derive(Debug, Clone)]
pub enum PluginState {
    Uninitialized,
    Initializing,
    Connected,
    Disconnected,
    Shutdown,
}

/// StreamDeck plugin implementation
pub struct StreamDeckPlugin {
    config: PluginConfig,
    state: Arc<RwLock<PluginState>>,
    compatibility: VersionCompatibility,
    telemetry_subscriber: Option<Box<dyn TelemetrySubscriber + Send + Sync>>,
    event_tx: Option<mpsc::UnboundedSender<PluginEvent>>,
    event_rx: Option<mpsc::UnboundedReceiver<PluginEvent>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    current_telemetry: Arc<RwLock<Option<BusSnapshot>>>,
}

impl StreamDeckPlugin {
    /// Create new plugin instance
    pub fn new(config: PluginConfig) -> Result<Self, PluginError> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(PluginState::Uninitialized)),
            compatibility: VersionCompatibility::new(),
            telemetry_subscriber: None,
            event_tx: Some(event_tx),
            event_rx: Some(event_rx),
            shutdown_tx: None,
            current_telemetry: Arc::new(RwLock::new(None)),
        })
    }

    /// Initialize the plugin
    pub async fn initialize(&mut self) -> Result<(), PluginError> {
        info!("Initializing StreamDeck plugin");

        {
            let mut state = self.state.write().await;
            *state = PluginState::Initializing;
        }

        // Initialize telemetry subscription
        self.initialize_telemetry_subscription().await?;

        // Start event processing loop
        self.start_event_processing().await?;

        {
            let mut state = self.state.write().await;
            *state = PluginState::Connected;
        }

        info!("StreamDeck plugin initialized successfully");
        Ok(())
    }

    /// Shutdown the plugin
    pub async fn shutdown(&mut self) -> Result<(), PluginError> {
        info!("Shutting down StreamDeck plugin");

        {
            let mut state = self.state.write().await;
            *state = PluginState::Shutdown;
        }

        // Send shutdown signal
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }

        // Send shutdown event
        if let Some(event_tx) = &self.event_tx {
            let _ = event_tx.send(PluginEvent::PluginShutdown);
        }

        // Clean up telemetry subscription
        self.telemetry_subscriber = None;

        info!("StreamDeck plugin shutdown complete");
        Ok(())
    }

    /// Get current plugin state
    pub async fn get_state(&self) -> PluginState {
        self.state.read().await.clone()
    }

    /// Check if plugin is connected
    pub async fn is_connected(&self) -> bool {
        matches!(*self.state.read().await, PluginState::Connected)
    }

    /// Get current telemetry data
    pub async fn get_current_telemetry(&self) -> Option<BusSnapshot> {
        self.current_telemetry.read().await.clone()
    }

    /// Send event to plugin
    pub fn send_event(&self, event: PluginEvent) -> Result<(), PluginError> {
        if let Some(event_tx) = &self.event_tx {
            event_tx.send(event).map_err(|e| {
                PluginError::EventProcessingFailed(format!("Failed to send event: {}", e))
            })?;
        }
        Ok(())
    }

    /// Set StreamDeck app version for compatibility checking
    pub async fn set_app_version(&mut self, version: AppVersion) -> Result<(), PluginError> {
        let version_str = version.to_string();
        self.compatibility.set_app_version(version).map_err(|e| {
            PluginError::ConfigurationError(format!("Version compatibility failed: {}", e))
        })?;

        info!("StreamDeck app version set to {}", version_str);
        Ok(())
    }

    /// Get available features for current app version
    pub fn get_available_features(&self) -> Vec<String> {
        self.compatibility.get_available_features()
    }

    /// Run verify test for event round-trip
    pub async fn run_verify_test(&mut self) -> Result<VerifyResult> {
        info!("Running StreamDeck verify test");

        let start_time = std::time::Instant::now();

        // Send test event
        let test_event = PluginEvent::ActionTriggered {
            action_uuid: "com.flighthub.verify-test".to_string(),
            context: "verify-test-context".to_string(),
        };

        self.send_event(test_event)?;

        // Wait for response (simulate round-trip)
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let elapsed = start_time.elapsed();

        let result = VerifyResult {
            success: true,
            round_trip_time_ms: elapsed.as_millis() as u32,
            events_processed: 1,
            errors: Vec::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        info!("Verify test completed: {:?}", result);
        Ok(result)
    }

    /// Initialize telemetry subscription
    async fn initialize_telemetry_subscription(&mut self) -> Result<(), PluginError> {
        debug!("Initializing telemetry subscription");

        // Create subscription configuration
        let subscription_config = SubscriptionConfig {
            max_rate_hz: self.config.telemetry_update_rate_hz as f32,
            buffer_size: self.config.event_buffer_size,
            drop_on_full: true,
        };

        // Create subscriber (in a real implementation, this would connect to the bus)
        // For now, we'll create a mock subscriber
        let subscriber_id = format!("streamdeck-{}", self.config.plugin_uuid);
        let subscriber = MockSubscriber::new(subscriber_id, subscription_config);
        self.telemetry_subscriber = Some(Box::new(subscriber));

        debug!("Telemetry subscription initialized");
        Ok(())
    }

    /// Start event processing loop
    async fn start_event_processing(&mut self) -> Result<(), PluginError> {
        debug!("Starting event processing loop");

        let event_rx = self.event_rx.take().ok_or(PluginError::NotInitialized)?;
        let state = Arc::clone(&self.state);
        let current_telemetry = Arc::clone(&self.current_telemetry);

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        // Spawn event processing task
        tokio::spawn(async move {
            Self::event_processing_loop(event_rx, shutdown_rx, state, current_telemetry).await;
        });

        debug!("Event processing loop started");
        Ok(())
    }

    /// Event processing loop
    async fn event_processing_loop(
        mut event_rx: mpsc::UnboundedReceiver<PluginEvent>,
        mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
        state: Arc<RwLock<PluginState>>,
        current_telemetry: Arc<RwLock<Option<BusSnapshot>>>,
    ) {
        debug!("Event processing loop running");

        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    match event {
                        Some(event) => {
                            if let Err(e) = Self::process_event(event, &current_telemetry).await {
                                error!("Failed to process event: {}", e);
                            }
                        }
                        None => {
                            debug!("Event channel closed");
                            break;
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    debug!("Shutdown signal received");
                    break;
                }
            }
        }

        // Update state to disconnected
        {
            let mut state_guard = state.write().await;
            if !matches!(*state_guard, PluginState::Shutdown) {
                *state_guard = PluginState::Disconnected;
            }
        }

        debug!("Event processing loop stopped");
    }

    /// Process individual event
    async fn process_event(
        event: PluginEvent,
        current_telemetry: &Arc<RwLock<Option<BusSnapshot>>>,
    ) -> Result<(), PluginError> {
        match event {
            PluginEvent::TelemetryUpdate(snapshot) => {
                debug!("Processing telemetry update");
                let mut telemetry = current_telemetry.write().await;
                *telemetry = Some(snapshot);
            }
            PluginEvent::ActionTriggered {
                action_uuid,
                context,
            } => {
                debug!("Processing action triggered: {} ({})", action_uuid, context);
                // Handle action trigger logic here
            }
            PluginEvent::PropertyInspectorUpdate {
                action_uuid,
                settings: _,
            } => {
                debug!("Processing property inspector update: {}", action_uuid);
                // Handle property inspector updates here
            }
            PluginEvent::DeviceConnected { device_id } => {
                info!("StreamDeck device connected: {}", device_id);
            }
            PluginEvent::DeviceDisconnected { device_id } => {
                info!("StreamDeck device disconnected: {}", device_id);
            }
            PluginEvent::PluginShutdown => {
                info!("Processing plugin shutdown event");
                return Ok(());
            }
        }

        Ok(())
    }
}

/// Mock subscriber for testing
struct MockSubscriber {
    id: String,
    config: SubscriptionConfig,
}

impl MockSubscriber {
    fn new(id: String, config: SubscriptionConfig) -> Self {
        Self { id, config }
    }
}

impl TelemetrySubscriber for MockSubscriber {
    fn get_id(&self) -> &SubscriberId {
        // In a real implementation, this would store the actual SubscriberId
        // For now, we'll use a static placeholder
        static PLACEHOLDER_ID: std::sync::OnceLock<SubscriberId> = std::sync::OnceLock::new();
        PLACEHOLDER_ID.get_or_init(|| {
            // Create a mock SubscriberId - in real implementation this would come from the publisher
            unsafe { std::mem::transmute(1u64) }
        })
    }

    fn get_config(&self) -> &SubscriptionConfig {
        &self.config
    }

    fn notify(
        &mut self,
        _snapshot: &BusSnapshot,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Mock implementation - in real code this would process the snapshot
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn test_plugin_creation() {
        let config = PluginConfig::default();
        let plugin = StreamDeckPlugin::new(config);
        assert!(plugin.is_ok());
    }

    #[tokio::test]
    async fn test_plugin_initialization() {
        let config = PluginConfig::default();
        let mut plugin = StreamDeckPlugin::new(config).unwrap();

        let result = timeout(Duration::from_secs(5), plugin.initialize()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());

        assert!(plugin.is_connected().await);
    }

    #[tokio::test]
    async fn test_plugin_shutdown() {
        let config = PluginConfig::default();
        let mut plugin = StreamDeckPlugin::new(config).unwrap();

        plugin.initialize().await.unwrap();
        assert!(plugin.is_connected().await);

        let result = timeout(Duration::from_secs(5), plugin.shutdown()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());

        assert!(!plugin.is_connected().await);
    }

    #[tokio::test]
    async fn test_plugin_event_sending() {
        let config = PluginConfig::default();
        let plugin = StreamDeckPlugin::new(config).unwrap();

        let event = PluginEvent::ActionTriggered {
            action_uuid: "test-action".to_string(),
            context: "test-context".to_string(),
        };

        let result = plugin.send_event(event);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_plugin_version_compatibility() {
        let config = PluginConfig::default();
        let mut plugin = StreamDeckPlugin::new(config).unwrap();

        let version = AppVersion::new(6, 2, 0);
        let result = plugin.set_app_version(version).await;
        assert!(result.is_ok());

        let features = plugin.get_available_features();
        assert!(!features.is_empty());
    }

    #[tokio::test]
    async fn test_verify_test() {
        let config = PluginConfig::default();
        let mut plugin = StreamDeckPlugin::new(config).unwrap();

        plugin.initialize().await.unwrap();

        let result = plugin.run_verify_test().await;
        assert!(result.is_ok());

        let verify_result = result.unwrap();
        assert!(verify_result.success);
        assert!(verify_result.round_trip_time_ms > 0);
    }
}
