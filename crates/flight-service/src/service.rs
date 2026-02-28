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
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::{debug, error, info, warn};

use flight_axis::{
    AxisEngine, DetentRole, DetentZone as AxisDetentZone, EngineConfig, PipelineBuilder,
    UpdateResult,
};
use flight_bus::BusPublisher;
use flight_core::{
    profile::{AxisConfig, Profile},
    watchdog::{WatchdogConfig, WatchdogSystem},
};

use crate::{
    aircraft_auto_switch_service::{AircraftAutoSwitchService, AircraftAutoSwitchServiceConfig},
    capability_service::CapabilityService,
    curve_conflict_service::CurveConflictService,
    error_taxonomy::ErrorTaxonomy,
    health::HealthStream,
    input_runtime::{
        SimulatedTFlightReportSource, TFlightInputRuntime, TFlightRuntimeConfig, TFlightSnapshot,
    },
    power::{PowerChecker, PowerStatus},
    safe_mode::{SafeModeConfig, SafeModeManager, SafeModeStatus},
    stecs_runtime::{
        SimulatedVkbStecsReportSource, VkbStecsInputRuntime, VkbStecsRuntimeConfig,
        VkbStecsSnapshot,
    },
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
    /// Auto-switch service configuration (includes process detection, bus, and adapter settings)
    pub auto_switch_config: AircraftAutoSwitchServiceConfig,
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
    /// Invert T.Flight throttle axis. Off by default; enable only after
    /// hardware receipts confirm inversion is needed for your device/driver.
    pub tflight_throttle_inversion: bool,
    /// Strip leading Report ID byte from T.Flight HID reports. Off by default.
    /// Enable if your OS/driver stack prepends a Report ID (typically 0x01)
    /// before the payload. Confirm with `receipts/hid/thrustmaster/tflight-hotas4/`.
    pub tflight_strip_report_id: bool,
    /// Enable VKB STECS ingest runtime.
    pub enable_stecs_runtime: bool,
    /// Polling rate for VKB STECS ingest runtime.
    pub stecs_poll_hz: u16,
    /// Strip leading Report ID byte from VKB STECS HID reports.
    pub stecs_strip_report_id: bool,
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TFlightYawPolicyConfig {
    #[default]
    Auto,
    Twist,
    Aux,
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
            auto_switch_config: AircraftAutoSwitchServiceConfig::default(),
            watchdog_config: WatchdogConfig::default(),
            enable_health_monitoring: true,
            enable_power_checks: true,
            enable_tflight_runtime: false,
            tflight_poll_hz: 250,
            tflight_yaw_policy: TFlightYawPolicyConfig::Auto,
            tflight_throttle_inversion: false,
            tflight_strip_report_id: false,
            enable_stecs_runtime: false,
            stecs_poll_hz: 250,
            stecs_strip_report_id: false,
        }
    }
}

impl FlightServiceConfig {
    /// Load configuration from a JSON file.
    ///
    /// Falls back to defaults on any error (missing file, parse failure) and
    /// logs a warning so the service can still start in a degraded state.
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file '{}': {e}", path.display()))?;
        Self::load_from_str(&content)
    }

    /// Parse configuration from a JSON string.
    pub fn load_from_str(json: &str) -> Result<Self> {
        let config: Self = serde_json::from_str(json)
            .map_err(|e| anyhow::anyhow!("Failed to parse service config: {e}"))?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration values are within acceptable ranges.
    pub fn validate(&self) -> Result<()> {
        if self.axis_config.max_frame_time_us == 0 {
            return Err(anyhow::anyhow!("axis_config.max_frame_time_us must be > 0"));
        }
        if self.tflight_poll_hz == 0 {
            return Err(anyhow::anyhow!("tflight_poll_hz must be > 0"));
        }
        if self.stecs_poll_hz == 0 {
            return Err(anyhow::anyhow!("stecs_poll_hz must be > 0"));
        }
        Ok(())
    }

    /// Try to load from `path`; on failure log a warning and return defaults.
    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        match Self::load_from_file(path.as_ref()) {
            Ok(cfg) => {
                info!("Loaded service config from '{}'", path.as_ref().display());
                cfg
            }
            Err(e) => {
                warn!(
                    "Could not load config from '{}': {e}; using defaults",
                    path.as_ref().display()
                );
                Self::default()
            }
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

impl ServiceState {
    /// Returns `true` if transitioning from `self` to `target` is a valid
    /// state machine move.
    pub fn can_transition_to(self, target: Self) -> bool {
        matches!(
            (self, target),
            (Self::Stopped, Self::Starting)
                | (Self::Starting, Self::Running)
                | (Self::Starting, Self::SafeMode)
                | (Self::Starting, Self::Failed)
                | (Self::Running, Self::Degraded)
                | (Self::Running, Self::Stopping)
                | (Self::SafeMode, Self::Stopping)
                | (Self::Degraded, Self::Running)
                | (Self::Degraded, Self::Stopping)
                | (Self::Stopping, Self::Stopped)
        )
    }
}

/// Build an `AxisEngine` pipeline from an `AxisConfig`.
///
/// Nodes are added in processing order: deadzone → detents → expo curve →
/// slew rate limiter → EMA filter. If the config specifies no nodes, a
/// trivial pass-through deadzone (threshold = 0.0) is added so the
/// `PipelineCompiler` receives at least one node.
pub(crate) fn build_pipeline_for_axis(
    axis_name: &str,
    config: &AxisConfig,
) -> anyhow::Result<flight_axis::Pipeline> {
    let mut builder = PipelineBuilder::new();
    let mut has_nodes = false;

    if let Some(deadzone) = config.deadzone {
        builder = builder.deadzone(deadzone);
        has_nodes = true;
    }

    if !config.detents.is_empty() {
        let zones: Vec<AxisDetentZone> = config
            .detents
            .iter()
            .map(|d| {
                let role = match d.role.to_lowercase().as_str() {
                    "idle" => DetentRole::Idle,
                    "taxi" => DetentRole::Taxi,
                    "takeoff" => DetentRole::Takeoff,
                    "climb" => DetentRole::Climb,
                    "cruise" => DetentRole::Cruise,
                    "approach" => DetentRole::Approach,
                    "landing" => DetentRole::Landing,
                    "reverse" => DetentRole::Reverse,
                    "emergency" => DetentRole::Emergency,
                    _ => DetentRole::Custom(0),
                };
                // Profile uses (position, width) while axis engine uses (center, half_width)
                AxisDetentZone::new(d.position, d.width / 2.0, 0.0, role)
            })
            .collect();
        builder = builder.detent(zones);
        has_nodes = true;
    }

    if let Some(expo) = config.expo {
        builder = builder
            .curve(expo)
            .map_err(|e| anyhow::anyhow!("Invalid expo for axis '{axis_name}': {e}"))?;
        has_nodes = true;
    }

    if let Some(slew_rate) = config.slew_rate {
        builder = builder.slew(slew_rate);
        has_nodes = true;
    }

    if let Some(filter) = &config.filter {
        builder = if let Some(threshold) = filter.spike_threshold {
            builder.filter_with_spike_rejection(filter.alpha, threshold)
        } else {
            builder.filter(filter.alpha)
        };
        has_nodes = true;
    }

    if !has_nodes {
        // Empty config → identity pipeline (deadzone 0.0 passes the signal through unchanged)
        builder = builder.deadzone(0.0);
    }

    builder
        .compile()
        .map_err(|e| anyhow::anyhow!("Pipeline compile error for axis '{axis_name}': {e:?}"))
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
    /// Bus publisher for telemetry distribution
    bus_publisher: Option<BusPublisher>,
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
    /// VKB STECS runtime
    stecs_runtime: Option<VkbStecsInputRuntime>,
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
            bus_publisher: None,
            auto_switch: None,
            curve_conflict: None,
            capability_service: None,
            watchdog: None,
            tflight_runtime: None,
            stecs_runtime: None,
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

        // Validate state transition
        {
            let mut state = self.state.write().await;
            if !state.can_transition_to(ServiceState::Starting) {
                return Err(anyhow::anyhow!(
                    "Cannot start service from state {:?}",
                    *state
                ));
            }
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
        if self.config.enable_stecs_runtime {
            self.health
                .register_component("input_hotas_vkb_stecs")
                .await;
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
        if self.config.enable_stecs_runtime {
            self.initialize_stecs_runtime().await?;
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

        let engine_config = EngineConfig {
            enable_rt_checks: self.config.axis_config.enable_rt_checks,
            max_frame_time_us: self.config.axis_config.max_frame_time_us,
            enable_counters: self.config.axis_config.enable_counters,
            enable_conflict_detection: self.config.axis_config.enable_conflict_detection,
            ..EngineConfig::default()
        };
        let engine = AxisEngine::with_config("primary".to_string(), engine_config);
        self.axis_engine = Some(Arc::new(engine));
        self.health
            .info("axis_engine", "Axis engine initialized")
            .await;
        Ok(())
    }

    /// Initialize auto-switch service
    async fn initialize_auto_switch(&mut self) -> Result<()> {
        info!("Initializing auto-switch service");

        let bus_rate_hz = self
            .config
            .auto_switch_config
            .bus_subscription
            .telemetry_rate;
        let mut bus_publisher = BusPublisher::new(bus_rate_hz);

        let auto_switch = AircraftAutoSwitchService::new(self.config.auto_switch_config.clone());
        auto_switch
            .start(&mut bus_publisher)
            .await
            .map_err(|e| anyhow::anyhow!("Auto-switch start failed: {e}"))?;

        self.bus_publisher = Some(bus_publisher);
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
            throttle_inversion: self.config.tflight_throttle_inversion,
            strip_report_id: self.config.tflight_strip_report_id,
        };

        // Use the real HID-backed source when the feature is enabled;
        // fall back to the deterministic simulated source otherwise.
        #[cfg(feature = "tflight-hidapi")]
        let (source, source_label): (
            Box<dyn crate::input_runtime::TFlightReportSource>,
            &str,
        ) = {
            match crate::hidapi_source::HidApiTFlightReportSource::new() {
                Ok(real) => (Box::new(real), "hidapi"),
                Err(e) => {
                    warn!(
                        "hidapi source unavailable ({}), falling back to simulated",
                        e
                    );
                    (
                        Box::new(SimulatedTFlightReportSource::default()),
                        "simulated (hidapi unavailable)",
                    )
                }
            }
        };

        #[cfg(not(feature = "tflight-hidapi"))]
        let (source, source_label): (
            Box<dyn crate::input_runtime::TFlightReportSource>,
            &str,
        ) = (
            Box::new(SimulatedTFlightReportSource::default()),
            "simulated",
        );

        let runtime = TFlightInputRuntime::start(source, Arc::clone(&self.health), config);

        self.tflight_runtime = Some(runtime);
        self.health
            .info(
                "input_hotas_tflight",
                &format!("T.Flight HOTAS runtime initialized ({source_label} report source)"),
            )
            .await;
        Ok(())
    }

    /// Initialize VKB STECS ingest runtime.
    async fn initialize_stecs_runtime(&mut self) -> Result<()> {
        info!("Initializing VKB STECS runtime");

        let config = VkbStecsRuntimeConfig {
            poll_hz: self.config.stecs_poll_hz,
            strip_report_id: self.config.stecs_strip_report_id,
            discovery_interval_ticks: self.config.stecs_poll_hz.max(1) as u32,
        };

        #[cfg(feature = "stecs-hidapi")]
        let (source, source_label): (
            Box<dyn crate::stecs_runtime::VkbStecsReportSource>,
            &str,
        ) = {
            match crate::stecs_hidapi_source::HidApiVkbStecsReportSource::new() {
                Ok(real) => (Box::new(real), "hidapi"),
                Err(e) => {
                    warn!(
                        "stecs hidapi source unavailable ({}), falling back to simulated",
                        e
                    );
                    (
                        Box::new(SimulatedVkbStecsReportSource::default()),
                        "simulated (hidapi unavailable)",
                    )
                }
            }
        };

        #[cfg(not(feature = "stecs-hidapi"))]
        let (source, source_label): (
            Box<dyn crate::stecs_runtime::VkbStecsReportSource>,
            &str,
        ) = (
            Box::new(SimulatedVkbStecsReportSource::default()),
            "simulated",
        );

        let runtime = VkbStecsInputRuntime::start(source, Arc::clone(&self.health), config);
        self.stecs_runtime = Some(runtime);

        self.health
            .info(
                "input_hotas_vkb_stecs",
                &format!("VKB STECS runtime initialized ({source_label} report source)"),
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

    /// Apply a profile by compiling pipelines off-thread and swapping
    /// atomically into the axis engine (ADR-001 RT-safe pattern).
    ///
    /// Each axis pipeline is compiled on a blocking thread pool so the RT
    /// spine is never blocked. Individual axis compilation failures are
    /// logged and skipped — the remaining axes are still applied.
    pub async fn apply_profile(&self, profile: &Profile) -> Result<()> {
        info!("Applying profile: {:?}", profile);

        // Validate profile structure first; surface any schema/bounds errors early
        profile
            .validate()
            .map_err(|e| anyhow::anyhow!("Profile validation failed: {}", e))?;

        let engine = match &self.axis_engine {
            Some(e) => Arc::clone(e),
            None => {
                let msg = "Cannot apply profile - axis engine not initialized";
                self.health.warning("service", msg).await;
                return Err(anyhow::anyhow!(msg));
            }
        };

        // Compile all pipelines off-thread (non-RT) then swap into the engine.
        let axes: Vec<(String, AxisConfig)> = profile
            .axes
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let compiled = tokio::task::spawn_blocking(move || {
            let mut results = Vec::with_capacity(axes.len());
            for (axis_name, axis_config) in &axes {
                let pipeline_result = build_pipeline_for_axis(axis_name, axis_config);
                results.push((axis_name.clone(), pipeline_result));
            }
            results
        })
        .await
        .map_err(|e| anyhow::anyhow!("Pipeline compilation task panicked: {e}"))?;

        let mut failed_count = 0u32;
        let total_count = compiled.len();

        for (axis_name, pipeline_result) in compiled {
            match pipeline_result {
                Ok(pipeline) => match engine.update_pipeline(pipeline) {
                    UpdateResult::Pending => {
                        info!("Axis pipeline update pending for axis '{axis_name}'");
                    }
                    UpdateResult::Success => {
                        info!("Axis pipeline updated for axis '{axis_name}'");
                    }
                    UpdateResult::Failed(msg) => {
                        warn!("Axis pipeline update failed for axis '{axis_name}': {msg}");
                        failed_count += 1;
                    }
                    UpdateResult::Rejected(msg) => {
                        warn!("Axis pipeline update rejected for axis '{axis_name}': {msg}");
                        failed_count += 1;
                    }
                },
                Err(e) => {
                    error!("Failed to compile pipeline for axis '{axis_name}': {e}");
                    failed_count += 1;
                }
            }
        }

        if failed_count > 0 && failed_count < total_count as u32 {
            let msg = format!(
                "Profile partially applied ({} of {} axes failed)",
                failed_count, total_count
            );
            self.health.warning("service", &msg).await;
            warn!("{msg}");
            // Partial failure → degrade the service so operators notice
            self.transition_to_degraded(&msg).await;
        } else if failed_count > 0 {
            let msg = format!("Profile apply failed: all {} axes failed", total_count);
            self.health.warning("service", &msg).await;
            warn!("{msg}");
            self.transition_to_degraded(&msg).await;
        } else {
            self.health
                .info("service", "Profile applied successfully")
                .await;
        }

        Ok(())
    }

    /// Transition the service to `Degraded` if the current state allows it.
    async fn transition_to_degraded(&self, reason: &str) {
        let mut state = self.state.write().await;
        if state.can_transition_to(ServiceState::Degraded) {
            info!("Service degraded: {reason}");
            *state = ServiceState::Degraded;
        }
    }

    /// Attempt to recover from `Degraded` back to `Running` by re-applying
    /// the given profile.  Returns `true` on a successful recovery.
    pub async fn try_recover(&self, profile: &Profile) -> Result<bool> {
        {
            let current = *self.state.read().await;
            if current != ServiceState::Degraded {
                return Ok(false);
            }
        }
        // Re-apply the profile; if it succeeds fully, restore Running.
        self.apply_profile(profile).await?;
        let mut state = self.state.write().await;
        if *state == ServiceState::Degraded {
            *state = ServiceState::Running;
            info!("Service recovered from Degraded → Running");
        }
        Ok(true)
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

    /// Get latest VKB STECS snapshots.
    pub async fn get_stecs_snapshots(&self) -> HashMap<String, VkbStecsSnapshot> {
        if let Some(runtime) = &self.stecs_runtime {
            runtime.snapshots().await
        } else {
            HashMap::new()
        }
    }

    /// Get latest snapshot for one VKB STECS physical device.
    pub async fn get_stecs_snapshot(&self, device_id: &str) -> Option<VkbStecsSnapshot> {
        if let Some(runtime) = &self.stecs_runtime {
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

        // Validate state transition
        {
            let mut state = self.state.write().await;
            if !state.can_transition_to(ServiceState::Stopping) {
                warn!("Shutdown requested from state {:?}, forcing stop", *state);
            }
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
        if let Some(mut runtime) = self.stecs_runtime.take() {
            runtime.shutdown().await;
            debug!("VKB STECS runtime stopped");
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

        if let Some(auto_switch) = self.auto_switch.take() {
            if let Err(e) = auto_switch.stop().await {
                warn!("Auto-switch service stop error: {}", e);
            }
            debug!("Auto-switch service stopped");
        }

        if let Some(_bus_publisher) = self.bus_publisher.take() {
            debug!("Bus publisher dropped");
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

    #[test]
    fn test_build_pipeline_for_axis_basic() {
        use flight_core::profile::AxisConfig;
        let config = AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.3),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        };
        let pipeline = build_pipeline_for_axis("pitch", &config);
        assert!(pipeline.is_ok(), "expected Ok, got {:?}", pipeline.err());
    }

    #[test]
    fn test_build_pipeline_for_axis_empty_config() {
        use flight_core::profile::AxisConfig;
        let config = AxisConfig {
            deadzone: None,
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        };
        // Should still compile (adds identity deadzone internally)
        let pipeline = build_pipeline_for_axis("roll", &config);
        assert!(
            pipeline.is_ok(),
            "expected Ok for empty config, got {:?}",
            pipeline.err()
        );
    }

    #[tokio::test]
    async fn test_apply_profile_wires_pipeline() {
        use flight_core::profile::{AxisConfig, Profile};
        use std::collections::HashMap;

        let mut config = FlightServiceConfig::default();
        config.safe_mode = false;
        let mut service = FlightService::new(config);
        let _ = service.initialize_axis_engine().await;

        let mut axes = HashMap::new();
        axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.03),
                expo: Some(0.2),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: None,
            aircraft: None,
            axes,
            pof_overrides: None,
        };

        let result = service.apply_profile(&profile).await;
        assert!(
            result.is_ok(),
            "apply_profile should succeed: {:?}",
            result.err()
        );

        // Engine should now have a pending pipeline
        let engine = service.axis_engine.as_ref().expect("axis engine present");
        // After update_pipeline() the engine marks a pending swap
        assert!(
            engine.active_version().is_some() || engine.swap_ack_count() == 0,
            "pipeline should be queued or active"
        );
    }

    #[tokio::test]
    async fn test_axis_engine_uses_service_config() {
        let mut config = FlightServiceConfig::default();
        config.axis_config.enable_rt_checks = true;
        config.axis_config.max_frame_time_us = 2_000;
        config.axis_config.enable_counters = false;
        config.axis_config.enable_conflict_detection = true;

        let mut service = FlightService::new(config);
        let result = service.initialize_axis_engine().await;
        assert!(result.is_ok(), "axis engine init should succeed");
        assert!(
            service.axis_engine.is_some(),
            "axis engine should be present after init"
        );
    }

    #[tokio::test]
    async fn test_auto_switch_creates_bus_publisher() {
        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        let result = service.initialize_auto_switch().await;
        assert!(result.is_ok(), "auto-switch init should succeed");
        assert!(
            service.bus_publisher.is_some(),
            "bus publisher should be created during auto-switch init"
        );
        assert!(
            service.auto_switch.is_some(),
            "auto-switch service should be present"
        );
    }

    #[tokio::test]
    async fn test_apply_profile_all_axes() {
        use flight_core::profile::{AxisConfig, Profile};
        use std::collections::HashMap;

        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        let _ = service.initialize_axis_engine().await;

        let mut axes = HashMap::new();
        axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.03),
                expo: Some(0.2),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        axes.insert(
            "roll".to_string(),
            AxisConfig {
                deadzone: Some(0.05),
                expo: Some(0.4),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        axes.insert(
            "yaw".to_string(),
            AxisConfig {
                deadzone: Some(0.08),
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );

        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: None,
            aircraft: None,
            axes,
            pof_overrides: None,
        };

        let result = service.apply_profile(&profile).await;
        assert!(
            result.is_ok(),
            "apply_profile with multiple axes should succeed: {:?}",
            result.err()
        );
    }

    // -----------------------------------------------------------------------
    // Config loading tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_config_load_from_str_valid() {
        let json = serde_json::to_string(&FlightServiceConfig::default()).unwrap();
        let loaded = FlightServiceConfig::load_from_str(&json);
        assert!(loaded.is_ok(), "valid JSON should load: {:?}", loaded.err());
    }

    #[test]
    fn test_config_load_from_str_invalid_json() {
        let result = FlightServiceConfig::load_from_str("not json at all");
        assert!(result.is_err(), "garbage input should fail");
    }

    #[test]
    fn test_config_load_from_str_validation_failure() {
        // Construct JSON with an invalid field value (poll_hz = 0).
        let mut cfg = FlightServiceConfig::default();
        cfg.tflight_poll_hz = 0;
        let json = serde_json::to_string(&cfg).unwrap();
        let result = FlightServiceConfig::load_from_str(&json);
        assert!(result.is_err(), "zero poll_hz should fail validation");
    }

    #[test]
    fn test_config_load_from_file_missing() {
        let result = FlightServiceConfig::load_from_file("nonexistent_config_42.json");
        assert!(result.is_err(), "missing file should fail");
    }

    #[test]
    fn test_config_load_from_file_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("service.json");
        let json = serde_json::to_string_pretty(&FlightServiceConfig::default()).unwrap();
        std::fs::write(&path, json).unwrap();

        let loaded = FlightServiceConfig::load_from_file(&path);
        assert!(
            loaded.is_ok(),
            "file load should succeed: {:?}",
            loaded.err()
        );
    }

    #[test]
    fn test_config_load_or_default_missing_file() {
        let cfg = FlightServiceConfig::load_or_default("does_not_exist.json");
        // Should silently fall back to default
        assert_eq!(cfg.tflight_poll_hz, 250);
    }

    #[test]
    fn test_config_validate_defaults() {
        let cfg = FlightServiceConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_config_validate_zero_frame_time() {
        let mut cfg = FlightServiceConfig::default();
        cfg.axis_config.max_frame_time_us = 0;
        assert!(cfg.validate().is_err());
    }

    // -----------------------------------------------------------------------
    // Profile application tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_apply_profile_no_engine_returns_error() {
        let config = FlightServiceConfig::default();
        let service = FlightService::new(config);
        // Engine is not initialized — apply should fail gracefully.
        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let result = service.apply_profile(&profile).await;
        assert!(result.is_err(), "should fail without axis engine");
    }

    #[tokio::test]
    async fn test_apply_profile_empty_axes_succeeds() {
        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        let _ = service.initialize_axis_engine().await;
        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let result = service.apply_profile(&profile).await;
        assert!(
            result.is_ok(),
            "empty axes profile should succeed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_apply_profile_invalid_profile_returns_error() {
        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        let _ = service.initialize_axis_engine().await;
        let profile = Profile {
            schema: "bad_schema".to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let result = service.apply_profile(&profile).await;
        assert!(result.is_err(), "invalid schema should fail validation");
    }

    // -----------------------------------------------------------------------
    // State transition tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_state_valid_transitions() {
        assert!(ServiceState::Stopped.can_transition_to(ServiceState::Starting));
        assert!(ServiceState::Starting.can_transition_to(ServiceState::Running));
        assert!(ServiceState::Starting.can_transition_to(ServiceState::SafeMode));
        assert!(ServiceState::Running.can_transition_to(ServiceState::Stopping));
        assert!(ServiceState::Running.can_transition_to(ServiceState::Degraded));
        assert!(ServiceState::Degraded.can_transition_to(ServiceState::Running));
        assert!(ServiceState::Degraded.can_transition_to(ServiceState::Stopping));
        assert!(ServiceState::SafeMode.can_transition_to(ServiceState::Stopping));
        assert!(ServiceState::Stopping.can_transition_to(ServiceState::Stopped));
    }

    #[test]
    fn test_state_invalid_transitions() {
        assert!(!ServiceState::Running.can_transition_to(ServiceState::Starting));
        assert!(!ServiceState::Stopped.can_transition_to(ServiceState::Running));
        assert!(!ServiceState::Stopping.can_transition_to(ServiceState::Running));
        assert!(!ServiceState::Failed.can_transition_to(ServiceState::Running));
    }

    #[tokio::test]
    async fn test_double_start_rejected() {
        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        let r1 = service.start().await;
        assert!(r1.is_ok());
        let r2 = service.start().await;
        assert!(r2.is_err(), "double start should be rejected");
        let _ = service.shutdown().await;
    }

    // -----------------------------------------------------------------------
    // Degraded state & recovery tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_transition_to_degraded_from_running() {
        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        service.start().await.unwrap();
        assert_eq!(service.get_state().await, ServiceState::Running);

        service.transition_to_degraded("test reason").await;
        assert_eq!(service.get_state().await, ServiceState::Degraded);

        let _ = service.shutdown().await;
    }

    #[tokio::test]
    async fn test_transition_to_degraded_ignored_from_stopped() {
        let config = FlightServiceConfig::default();
        let service = FlightService::new(config);
        // Stopped → Degraded is not a valid transition
        service.transition_to_degraded("should be ignored").await;
        assert_eq!(service.get_state().await, ServiceState::Stopped);
    }

    #[tokio::test]
    async fn test_try_recover_from_degraded() {
        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        service.start().await.unwrap();
        let _ = service.initialize_axis_engine().await;

        // Force into degraded state
        service.transition_to_degraded("partial failure").await;
        assert_eq!(service.get_state().await, ServiceState::Degraded);

        // Recover with a valid profile
        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let recovered = service.try_recover(&profile).await.unwrap();
        assert!(recovered, "should recover from Degraded");
        assert_eq!(service.get_state().await, ServiceState::Running);

        let _ = service.shutdown().await;
    }

    #[tokio::test]
    async fn test_try_recover_noop_when_running() {
        let config = FlightServiceConfig::default();
        let mut service = FlightService::new(config);
        service.start().await.unwrap();
        assert_eq!(service.get_state().await, ServiceState::Running);

        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let recovered = service.try_recover(&profile).await.unwrap();
        assert!(!recovered, "should not recover when already Running");

        let _ = service.shutdown().await;
    }
}
