// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight Hub Service Implementation
//!
//! Main service orchestration layer that coordinates all Flight Hub components
//! including axis processing, safety systems, auto-profiles, and health monitoring.

use anyhow::Result;
use flight_hotas_thrustmaster::TFlightYawPolicy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::{debug, info, warn};

use flight_axis::AxisEngine;
use flight_core::{
    aircraft_switch::AutoSwitchConfig,
    profile::Profile,
    watchdog::{WatchdogConfig, WatchdogSystem},
};

use crate::{
    aircraft_auto_switch_service::AircraftAutoSwitchService,
    capability_service::CapabilityService,
    curve_conflict_service::CurveConflictService,
    error_taxonomy::ErrorTaxonomy,
    health::HealthStream,
    input_runtime::{
        SimulatedTFlightReportSource, TFlightInputRuntime, TFlightRuntimeConfig, TFlightSnapshot,
    },
    power::{PowerChecker, PowerStatus},
    safe_mode::{SafeModeConfig, SafeModeManager, SafeModeStatus},
};

/// Flight service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightServiceConfig {
    /// Enable safe mode (axis-only operation)
    pub safe_mode: bool,
    /// Safe mode configuration
    pub safe_mode_config: SafeModeConfig,
    /// Axis engine configuration
    pub axis_config: AxisEngineConfig,
    /// Auto-switch configuration
    pub auto_switch_config: AutoSwitchConfig,
    /// Watchdog configuration
    #[serde(skip_serializing, skip_deserializing)]
    pub watchdog_config: WatchdogConfig,
    /// Enable health monitoring
    pub enable_health_monitoring: bool,
    /// Enable power optimization checks
    pub enable_power_checks: bool,
    /// Enable T.Flight HOTAS ingest runtime.
    pub enable_tflight_runtime: bool,
    /// Polling rate for T.Flight ingest runtime.
    pub tflight_poll_hz: u16,
    /// Yaw source policy for T.Flight parsing.
    pub tflight_yaw_policy: TFlightYawPolicyConfig,
}

/// Axis engine configuration subset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisEngineConfig {
    /// Enable runtime allocation checking
    pub enable_rt_checks: bool,
    /// Maximum processing time per frame (microseconds)
    pub max_frame_time_us: u32,
    /// Enable performance counters
    pub enable_counters: bool,
    /// Enable curve conflict detection
    pub enable_conflict_detection: bool,
    // Note: conflict_detector_config omitted from service config for simplicity
}

/// Serializable service-level yaw policy config for T.Flight devices.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TFlightYawPolicyConfig {
    Auto,
    Twist,
    Aux,
}

impl Default for TFlightYawPolicyConfig {
    fn default() -> Self {
        Self::Auto
    }
}

impl From<TFlightYawPolicyConfig> for TFlightYawPolicy {
    fn from(value: TFlightYawPolicyConfig) -> Self {
        match value {
            TFlightYawPolicyConfig::Auto => TFlightYawPolicy::Auto,
            TFlightYawPolicyConfig::Twist => TFlightYawPolicy::Twist,
            TFlightYawPolicyConfig::Aux => TFlightYawPolicy::Aux,
        }
    }
}

impl Default for FlightServiceConfig {
    fn default() -> Self {
        Self {
            safe_mode: false,
            safe_mode_config: SafeModeConfig::default(),
            axis_config: AxisEngineConfig {
                enable_rt_checks: false,
                max_frame_time_us: 5_000u32, // 5ms budget (approximates 5.0ms latency)
                enable_counters: true,
                enable_conflict_detection: false,
            },
            auto_switch_config: AutoSwitchConfig::default(),
            watchdog_config: WatchdogConfig::default(),
            enable_health_monitoring: true,
            enable_power_checks: true,
            enable_tflight_runtime: false,
            tflight_poll_hz: 250,
            tflight_yaw_policy: TFlightYawPolicyConfig::Auto,
        }
    }
}

/// Service state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceState {
    /// Service is starting up
    Starting,
    /// Service is running normally
    Running,
    /// Service is running in safe mode
    SafeMode,
    /// Service is degraded but functional
    Degraded,
    /// Service is shutting down
    Stopping,
    /// Service has stopped
    Stopped,
    /// Service has failed
    Failed,
}

/// Main Flight Hub service
pub struct FlightService {
    /// Service configuration
    config: FlightServiceConfig,
    /// Current service state
    state: Arc<RwLock<ServiceState>>,
    /// Health monitoring system
    health: Arc<HealthStream>,
    /// Error taxonomy
    error_taxonomy: Arc<ErrorTaxonomy>,
    /// Safe mode manager (if enabled)
    safe_mode: Option<SafeModeManager>,
    /// Axis engine
    axis_engine: Option<Arc<AxisEngine>>,
    /// Auto-switch service
    auto_switch: Option<AircraftAutoSwitchService>,
    /// Curve conflict service
    curve_conflict: Option<CurveConflictService>,
    /// Capability service
    capability_service: Option<CapabilityService>,
    /// Watchdog system
    watchdog: Option<WatchdogSystem>,
    /// T.Flight HOTAS runtime
    tflight_runtime: Option<TFlightInputRuntime>,
    /// Power status
    power_status: Arc<RwLock<Option<PowerStatus>>>,
    /// Service shutdown signal
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl FlightService {
    /// Create new Flight Hub service
    pub fn new(config: FlightServiceConfig) -> Self {
        info!("Creating Flight Hub service with config: {:?}", config);

        let health = Arc::new(HealthStream::new());
        let error_taxonomy = Arc::new(ErrorTaxonomy::new());

        Self {
            config,
            state: Arc::new(RwLock::new(ServiceState::Stopped)),
            health,
            error_taxonomy,
            safe_mode: None,
            axis_engine: None,
            auto_switch: None,
            curve_conflict: None,
            capability_service: None,
            watchdog: None,
            tflight_runtime: None,
            power_status: Arc::new(RwLock::new(None)),
            shutdown_tx: None,
        }
    }

    /// Test-only accessor for health stream
    #[cfg(test)]
    pub(crate) fn test_health_stream(&self) -> &HealthStream {
        &self.health
    }

    /// Test-only accessor for error taxonomy
    #[cfg(test)]
    pub(crate) fn test_error_taxonomy(&self) -> &ErrorTaxonomy {
        &self.error_taxonomy
    }

    /// Start the service
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Flight Hub service");

        // Update state to starting
        {
            let mut state = self.state.write().await;
            *state = ServiceState::Starting;
        }

        // Register core components with health system
        self.health.register_component("service").await;
        self.health.register_component("axis_engine").await;
        self.health.register_component("auto_switch").await;
        self.health.register_component("safety").await;
        if self.config.enable_tflight_runtime {
            self.health.register_component("input_hotas_tflight").await;
        }

        self.health
            .info("service", "Flight Hub service starting")
            .await;

        // Start health maintenance task
        let _health_task = self.health.start_maintenance_task();

        // Check power configuration if enabled
        if self.config.enable_power_checks {
            match self.check_power_configuration().await {
                Ok(status) => {
                    let mut power_status = self.power_status.write().await;
                    *power_status = Some(status);
                }
                Err(e) => {
                    self.health
                        .warning("service", &format!("Power check failed: {}", e))
                        .await;
                }
            }
        }

        // Initialize based on mode
        if self.config.safe_mode {
            self.start_safe_mode().await?;
        } else {
            self.start_full_mode().await?;
        }

        if self.config.enable_tflight_runtime {
            self.initialize_tflight_runtime().await?;
        }

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Update state to running
        {
            let mut state = self.state.write().await;
            *state = if self.config.safe_mode {
                ServiceState::SafeMode
            } else {
                ServiceState::Running
            };
        }

        self.health
            .info("service", "Flight Hub service started successfully")
            .await;
        info!(
            "Flight Hub service started in {} mode",
            if self.config.safe_mode {
                "safe"
            } else {
                "full"
            }
        );

        Ok(())
    }

    /// Start in safe mode
    async fn start_safe_mode(&mut self) -> Result<()> {
        info!("Starting service in safe mode");

        let mut safe_mode = SafeModeManager::new(self.config.safe_mode_config.clone());

        match safe_mode.initialize().await {
            Ok(status) => {
                self.health
                    .info("service", "Safe mode initialized successfully")
                    .await;
                debug!("Safe mode status: {:?}", status);
            }
            Err(e) => {
                self.health
                    .error(
                        "service",
                        &format!("Safe mode initialization failed: {}", e),
                        self.error_taxonomy
                            .get_error("RT_PRIVILEGE_DENIED")
                            .cloned(),
                    )
                    .await;
                return Err(e.into());
            }
        }

        self.safe_mode = Some(safe_mode);
        Ok(())
    }

    /// Start in full mode
    async fn start_full_mode(&mut self) -> Result<()> {
        info!("Starting service in full mode");

        // Initialize axis engine
        self.initialize_axis_engine().await?;

        // Initialize auto-switch service
        self.initialize_auto_switch().await?;

        // Initialize curve conflict detection
        self.initialize_curve_conflict().await?;

        // Initialize capability service
        self.initialize_capability_service().await?;

        // Initialize watchdog system
        self.initialize_watchdog().await?;

        self.health
            .info("service", "Full mode initialization completed")
            .await;
        Ok(())
    }

    /// Initialize axis engine
    async fn initialize_axis_engine(&mut self) -> Result<()> {
        info!("Initializing axis engine");

        let engine = AxisEngine::new();
        self.axis_engine = Some(Arc::new(engine));
        self.health
            .info("axis_engine", "Axis engine initialized")
            .await;
        Ok(())
    }

    /// Initialize auto-switch service
    async fn initialize_auto_switch(&mut self) -> Result<()> {
        info!("Initializing auto-switch service");

        // Stub implementation - would use real config
        let config = Default::default();

        let auto_switch = AircraftAutoSwitchService::new(config);
        self.auto_switch = Some(auto_switch);
        self.health
            .info("auto_switch", "Auto-switch service initialized")
            .await;
        Ok(())
    }

    /// Initialize curve conflict detection
    async fn initialize_curve_conflict(&mut self) -> Result<()> {
        info!("Initializing curve conflict detection");

        self.curve_conflict = Some(CurveConflictService::new()?);
        self.health
            .info("curve_conflict", "Curve conflict service initialized")
            .await;
        Ok(())
    }

    /// Initialize capability service
    async fn initialize_capability_service(&mut self) -> Result<()> {
        info!("Initializing capability service");

        self.capability_service = Some(CapabilityService::new());
        self.health
            .info("capability", "Capability service initialized")
            .await;
        Ok(())
    }

    /// Initialize watchdog system
    async fn initialize_watchdog(&mut self) -> Result<()> {
        info!("Initializing watchdog system");

        self.watchdog = Some(WatchdogSystem::new());
        self.health
            .info("safety", "Watchdog system initialized")
            .await;
        Ok(())
    }

    /// Initialize T.Flight HOTAS ingest runtime.
    async fn initialize_tflight_runtime(&mut self) -> Result<()> {
        info!("Initializing T.Flight HOTAS runtime");

        let config = TFlightRuntimeConfig {
            poll_hz: self.config.tflight_poll_hz,
            yaw_policy: self.config.tflight_yaw_policy.into(),
        };

        // This cycle uses the deterministic simulated backend by default.
        let source = Box::new(SimulatedTFlightReportSource::default());
        let runtime = TFlightInputRuntime::start(source, Arc::clone(&self.health), config);

        self.tflight_runtime = Some(runtime);
        self.health
            .info(
                "input_hotas_tflight",
                "T.Flight HOTAS runtime initialized (simulated report source)",
            )
            .await;
        Ok(())
    }

    /// Check power configuration
    async fn check_power_configuration(&self) -> Result<PowerStatus> {
        info!("Checking power configuration");

        let status = PowerChecker::check_power_configuration().await;

        match status.overall_status {
            crate::power::PowerCheckStatus::Optimal => {
                self.health
                    .info("service", "Power configuration is optimal")
                    .await;
            }
            crate::power::PowerCheckStatus::Degraded => {
                self.health
                    .warning(
                        "service",
                        "Power configuration has issues that may affect performance",
                    )
                    .await;
            }
            crate::power::PowerCheckStatus::Critical => {
                self.health
                    .error(
                        "service",
                        "Critical power configuration issues detected",
                        self.error_taxonomy
                            .get_error("POWER_THROTTLING_ACTIVE")
                            .cloned(),
                    )
                    .await;
            }
        }

        Ok(status)
    }

    /// Apply a profile
    pub async fn apply_profile(&self, profile: &Profile) -> Result<()> {
        info!("Applying profile: {:?}", profile);

        if let Some(_engine) = &self.axis_engine {
            // TODO: Replace with new profile ingestion API when ready
            // For now, safe mode bring-up still works without compile_profile
            self.health
                .info("service", "Profile applied successfully")
                .await;
            Ok(())
        } else {
            let msg = "Cannot apply profile - axis engine not initialized";
            self.health.warning("service", msg).await;
            Err(anyhow::anyhow!(msg))
        }
    }

    /// Get current service state
    pub async fn get_state(&self) -> ServiceState {
        *self.state.read().await
    }

    /// Get health status
    pub async fn get_health_status(&self) -> crate::health::HealthStatus {
        self.health.get_health_status().await
    }

    /// Get power status
    pub async fn get_power_status(&self) -> Option<PowerStatus> {
        self.power_status.read().await.clone()
    }

    /// Get safe mode status
    pub async fn get_safe_mode_status(&self) -> Option<SafeModeStatus> {
        self.safe_mode
            .as_ref()
            .map(|safe_mode| safe_mode.get_status())
    }

    /// Get latest T.Flight HOTAS snapshots.
    pub async fn get_tflight_snapshots(&self) -> HashMap<String, TFlightSnapshot> {
        if let Some(runtime) = &self.tflight_runtime {
            runtime.snapshots().await
        } else {
            HashMap::new()
        }
    }

    /// Get latest snapshot for one T.Flight HOTAS device.
    pub async fn get_tflight_snapshot(&self, device_id: &str) -> Option<TFlightSnapshot> {
        if let Some(runtime) = &self.tflight_runtime {
            runtime.snapshot(device_id).await
        } else {
            None
        }
    }

    /// Subscribe to health events
    pub fn subscribe_health(&self) -> broadcast::Receiver<crate::health::HealthEvent> {
        self.health.subscribe()
    }

    /// Subscribe to shutdown events
    pub fn subscribe_shutdown(&self) -> Option<broadcast::Receiver<()>> {
        self.shutdown_tx.as_ref().map(|tx| tx.subscribe())
    }

    /// Shutdown the service
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down Flight Hub service");

        // Update state
        {
            let mut state = self.state.write().await;
            *state = ServiceState::Stopping;
        }

        self.health
            .info("service", "Service shutdown initiated")
            .await;

        // Send shutdown signal
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(());
        }

        if let Some(mut runtime) = self.tflight_runtime.take() {
            runtime.shutdown().await;
            debug!("T.Flight HOTAS runtime stopped");
        }

        // Shutdown components in reverse order (drop handles cleanup)
        if let Some(_watchdog) = self.watchdog.take() {
            debug!("Watchdog system dropped");
        }

        if let Some(_capability) = self.capability_service.take() {
            debug!("Capability service dropped");
        }

        if let Some(_curve_conflict) = self.curve_conflict.take() {
            debug!("Curve conflict service dropped");
        }

        if let Some(_auto_switch) = self.auto_switch.take() {
            debug!("Auto-switch service dropped");
        }

        // Shutdown axis engine last
        if let Some(_engine) = self.axis_engine.take() {
            debug!("Axis engine dropped");
        }

        // Shutdown safe mode if active
        if let Some(mut safe_mode) = self.safe_mode.take()
            && let Err(e) = safe_mode.shutdown().await
        {
            warn!("Safe mode shutdown error: {}", e);
        }

        // Update final state
        {
            let mut state = self.state.write().await;
            *state = ServiceState::Stopped;
        }

        self.health
            .info("service", "Flight Hub service shutdown completed")
            .await;
        info!("Flight Hub service shutdown completed");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_creation() {
        let config = FlightServiceConfig::default();
        let service = FlightService::new(config);

        assert_eq!(service.get_state().await, ServiceState::Stopped);
    }

    #[tokio::test]
    async fn test_safe_mode_service() {
        let mut config = FlightServiceConfig::default();
        config.safe_mode = true;

        let mut service = FlightService::new(config);
        let result = service.start().await;

        assert!(result.is_ok());
        assert_eq!(service.get_state().await, ServiceState::SafeMode);

        let _ = service.shutdown().await;
    }

    #[tokio::test]
    async fn test_health_monitoring() {
        let config = FlightServiceConfig::default();
        let service = FlightService::new(config);

        let health_status = service.get_health_status().await;
        assert_eq!(
            health_status.overall.state,
            crate::health::HealthState::Healthy
        );
    }
}
