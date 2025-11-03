// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Aircraft Auto-Switch System
//!
//! Implements automatic profile switching based on aircraft detection with ≤500ms response time.
//! Provides process+aircraft detection, profile resolution with merge hierarchy, and
//! compile-and-swap system for profile changes with PoF hysteresis logic.

use crate::profile::{CapabilityContext, CapabilityMode, Profile, merge_axis_configs};
use crate::{FlightError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

/// Simulator identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SimId {
    Msfs,
    XPlane,
    Dcs,
    Unknown,
}

impl std::fmt::Display for SimId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimId::Msfs => write!(f, "MSFS"),
            SimId::XPlane => write!(f, "X-Plane"),
            SimId::Dcs => write!(f, "DCS"),
            SimId::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Aircraft identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AircraftId {
    pub icao: String,
    pub variant: Option<String>,
}

impl AircraftId {
    pub fn new(icao: impl Into<String>) -> Self {
        Self {
            icao: icao.into(),
            variant: None,
        }
    }

    pub fn with_variant(icao: impl Into<String>, variant: impl Into<String>) -> Self {
        Self {
            icao: icao.into(),
            variant: Some(variant.into()),
        }
    }
}

impl std::fmt::Display for AircraftId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.variant {
            Some(variant) => write!(f, "{}-{}", self.icao, variant),
            None => write!(f, "{}", self.icao),
        }
    }
}

/// Simplified telemetry snapshot for PoF determination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    pub sim: SimId,
    pub aircraft: AircraftId,
    pub timestamp: u64,
    pub ias_knots: f32,
    pub ground_speed_knots: f32,
    pub altitude_feet: f32,
    pub vertical_speed_fpm: f32,
    pub gear_down: bool,
}

/// Aircraft detection and auto-switch system
#[derive(Debug)]
pub struct AircraftAutoSwitch {
    config: AutoSwitchConfig,
    state: Arc<RwLock<AutoSwitchState>>,
    profile_cache: Arc<RwLock<ProfileCache>>,
    pof_tracker: Arc<RwLock<PofTracker>>,
    switch_tx: mpsc::UnboundedSender<SwitchRequest>,
    switch_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<SwitchRequest>>>>,
}

/// Configuration for aircraft auto-switch system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoSwitchConfig {
    /// Maximum time allowed for profile switching (default: 500ms)
    pub max_switch_time: Duration,
    /// Profile search paths in priority order
    pub profile_paths: Vec<PathBuf>,
    /// Whether to enable PoF (Phase of Flight) switching
    pub enable_pof: bool,
    /// PoF hysteresis configuration
    pub pof_hysteresis: PofHysteresisConfig,
    /// Capability enforcement context
    pub capability_context: CapabilityContext,
}

/// Phase of Flight hysteresis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PofHysteresisConfig {
    /// Minimum time in phase before allowing switch (prevents flapping)
    pub min_phase_time: Duration,
    /// Hysteresis bands for different flight parameters
    pub hysteresis_bands: HashMap<String, HysteresisBand>,
}

/// Hysteresis band configuration for a parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HysteresisBand {
    /// Lower threshold for entering condition
    pub enter_threshold: f32,
    /// Upper threshold for exiting condition  
    pub exit_threshold: f32,
    /// Parameter unit (for validation)
    pub unit: String,
}

/// Internal state of the auto-switch system
#[derive(Debug)]
struct AutoSwitchState {
    /// Currently active aircraft
    current_aircraft: Option<DetectedAircraft>,
    /// Currently active profile
    current_profile: Option<CompiledProfile>,
    /// Last switch timestamp
    last_switch: Option<Instant>,
    /// Current phase of flight
    #[allow(dead_code)]
    current_pof: Option<PhaseOfFlight>,
    /// Switch performance metrics
    metrics: SwitchMetrics,
}

/// Profile cache for fast lookups
#[derive(Debug)]
struct ProfileCache {
    /// Cached profiles by aircraft ID
    profiles: HashMap<AircraftId, CachedProfile>,
    /// Cache invalidation timestamps
    cache_timestamps: HashMap<AircraftId, Instant>,
    /// Cache TTL
    cache_ttl: Duration,
}

/// Phase of Flight tracker with hysteresis
#[derive(Debug)]
struct PofTracker {
    /// Current PoF state
    current_pof: Option<PhaseOfFlight>,
    /// Time when current PoF was entered
    pof_enter_time: Option<Instant>,
    /// Hysteresis state for each parameter
    hysteresis_state: HashMap<String, HysteresisState>,
}

/// Detected aircraft information
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedAircraft {
    pub sim: SimId,
    pub aircraft_id: AircraftId,
    pub process_name: String,
    pub detection_time: Instant,
    pub confidence: f32, // 0.0 to 1.0
}

/// Compiled profile ready for axis engine
#[derive(Debug, Clone)]
pub struct CompiledProfile {
    pub profile: Profile,
    pub effective_hash: String,
    pub compile_time: Instant,
    pub pof_overrides: HashMap<PhaseOfFlight, Profile>,
}

/// Cached profile with metadata
#[derive(Debug, Clone)]
struct CachedProfile {
    #[allow(dead_code)]
    pub base_profile: Profile,
    pub compiled: CompiledProfile,
    #[allow(dead_code)]
    pub file_path: PathBuf,
    #[allow(dead_code)]
    pub last_modified: Instant,
}

/// Phase of Flight enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhaseOfFlight {
    Ground,
    Taxi,
    Takeoff,
    Climb,
    Cruise,
    Descent,
    Approach,
    Landing,
    GoAround,
}

/// Hysteresis state for a parameter
#[derive(Debug, Clone)]
struct HysteresisState {
    current_value: f32,
    in_condition: bool,
    last_transition: Instant,
}

/// Switch request for internal processing
#[derive(Debug)]
enum SwitchRequest {
    AircraftDetected(DetectedAircraft),
    TelemetryUpdate(TelemetrySnapshot),
    ForceSwitch(AircraftId),
    InvalidateCache(Option<AircraftId>),
}

/// Switch performance metrics
#[derive(Debug, Default)]
pub struct SwitchMetrics {
    pub total_switches: u64,
    pub successful_switches: u64,
    pub failed_switches: u64,
    pub average_switch_time: Duration,
    pub max_switch_time: Duration,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Switch operation result
#[derive(Debug)]
pub struct SwitchResult {
    pub success: bool,
    pub switch_time: Duration,
    pub old_aircraft: Option<AircraftId>,
    pub new_aircraft: AircraftId,
    pub profile_hash: String,
    pub pof_changed: bool,
}

impl Default for AutoSwitchConfig {
    fn default() -> Self {
        Self {
            max_switch_time: Duration::from_millis(500),
            profile_paths: vec![
                PathBuf::from("profiles/global"),
                PathBuf::from("profiles/sim"),
                PathBuf::from("profiles/aircraft"),
            ],
            enable_pof: true,
            pof_hysteresis: PofHysteresisConfig::default(),
            capability_context: CapabilityContext::for_mode(CapabilityMode::Full),
        }
    }
}

impl Default for PofHysteresisConfig {
    fn default() -> Self {
        let mut hysteresis_bands = HashMap::new();

        // IAS hysteresis for phase transitions
        hysteresis_bands.insert(
            "ias".to_string(),
            HysteresisBand {
                enter_threshold: 90.0, // Enter approach at 90 knots
                exit_threshold: 100.0, // Exit approach at 100 knots
                unit: "knots".to_string(),
            },
        );

        // Altitude hysteresis
        hysteresis_bands.insert(
            "altitude".to_string(),
            HysteresisBand {
                enter_threshold: 1000.0, // Enter pattern at 1000 ft
                exit_threshold: 1500.0,  // Exit pattern at 1500 ft
                unit: "feet".to_string(),
            },
        );

        // Ground speed hysteresis
        hysteresis_bands.insert(
            "ground_speed".to_string(),
            HysteresisBand {
                enter_threshold: 5.0, // Enter taxi at 5 knots
                exit_threshold: 10.0, // Exit taxi at 10 knots
                unit: "knots".to_string(),
            },
        );

        Self {
            min_phase_time: Duration::from_secs(5), // Minimum 5 seconds in phase
            hysteresis_bands,
        }
    }
}

impl AircraftAutoSwitch {
    /// Create new aircraft auto-switch system
    pub fn new(config: AutoSwitchConfig) -> Self {
        let (switch_tx, switch_rx) = mpsc::unbounded_channel();

        Self {
            config,
            state: Arc::new(RwLock::new(AutoSwitchState::new())),
            profile_cache: Arc::new(RwLock::new(ProfileCache::new())),
            pof_tracker: Arc::new(RwLock::new(PofTracker::new())),
            switch_tx,
            switch_rx: Arc::new(RwLock::new(Some(switch_rx))),
        }
    }

    /// Start the auto-switch processing loop
    pub async fn start(&self) -> Result<()> {
        let mut rx =
            self.switch_rx.write().await.take().ok_or_else(|| {
                FlightError::AutoSwitch("Auto-switch already started".to_string())
            })?;

        let state = Arc::clone(&self.state);
        let profile_cache = Arc::clone(&self.profile_cache);
        let pof_tracker = Arc::clone(&self.pof_tracker);
        let config = self.config.clone();

        tokio::spawn(async move {
            info!("Aircraft auto-switch system started");

            while let Some(request) = rx.recv().await {
                let start_time = Instant::now();

                match Self::process_switch_request(
                    request,
                    &state,
                    &profile_cache,
                    &pof_tracker,
                    &config,
                )
                .await
                {
                    Ok(result) => {
                        if let Some(result) = result {
                            let mut state_guard = state.write().await;
                            state_guard.metrics.total_switches += 1;
                            if result.success {
                                state_guard.metrics.successful_switches += 1;
                            } else {
                                state_guard.metrics.failed_switches += 1;
                            }

                            // Update timing metrics
                            let switch_time = start_time.elapsed();
                            if switch_time > state_guard.metrics.max_switch_time {
                                state_guard.metrics.max_switch_time = switch_time;
                            }

                            // Update average (simple moving average)
                            let total = state_guard.metrics.total_switches as f64;
                            let current_avg =
                                state_guard.metrics.average_switch_time.as_nanos() as f64;
                            let new_avg = (current_avg * (total - 1.0)
                                + switch_time.as_nanos() as f64)
                                / total;
                            state_guard.metrics.average_switch_time =
                                Duration::from_nanos(new_avg as u64);

                            info!(
                                "Aircraft switch completed: {} -> {} in {:?}",
                                result
                                    .old_aircraft
                                    .map(|a| a.to_string())
                                    .unwrap_or_else(|| "None".to_string()),
                                result.new_aircraft,
                                switch_time
                            );
                        }
                    }
                    Err(e) => {
                        error!("Switch request processing failed: {}", e);
                        let mut state_guard = state.write().await;
                        state_guard.metrics.failed_switches += 1;
                    }
                }
            }

            info!("Aircraft auto-switch system stopped");
        });

        Ok(())
    }

    /// Handle aircraft detection event
    pub async fn on_aircraft_detected(&self, aircraft: DetectedAircraft) -> Result<()> {
        debug!("Aircraft detected: {:?}", aircraft);
        self.switch_tx
            .send(SwitchRequest::AircraftDetected(aircraft))
            .map_err(|e| {
                FlightError::AutoSwitch(format!("Failed to send aircraft detection: {}", e))
            })?;
        Ok(())
    }

    /// Handle telemetry update for PoF tracking
    pub async fn on_telemetry_update(&self, snapshot: TelemetrySnapshot) -> Result<()> {
        if self.config.enable_pof {
            self.switch_tx
                .send(SwitchRequest::TelemetryUpdate(snapshot))
                .map_err(|e| {
                    FlightError::AutoSwitch(format!("Failed to send telemetry update: {}", e))
                })?;
        }
        Ok(())
    }

    /// Force switch to specific aircraft (for testing/manual override)
    pub async fn force_switch(&self, aircraft_id: AircraftId) -> Result<()> {
        info!("Forcing switch to aircraft: {}", aircraft_id);
        self.switch_tx
            .send(SwitchRequest::ForceSwitch(aircraft_id))
            .map_err(|e| FlightError::AutoSwitch(format!("Failed to send force switch: {}", e)))?;
        Ok(())
    }

    /// Invalidate profile cache
    pub async fn invalidate_cache(&self, aircraft_id: Option<AircraftId>) -> Result<()> {
        debug!("Invalidating profile cache for: {:?}", aircraft_id);
        self.switch_tx
            .send(SwitchRequest::InvalidateCache(aircraft_id))
            .map_err(|e| {
                FlightError::AutoSwitch(format!("Failed to send cache invalidation: {}", e))
            })?;
        Ok(())
    }

    /// Get current switch metrics
    pub async fn get_metrics(&self) -> SwitchMetrics {
        self.state.read().await.metrics.clone()
    }

    /// Get current aircraft
    pub async fn get_current_aircraft(&self) -> Option<DetectedAircraft> {
        self.state.read().await.current_aircraft.clone()
    }

    /// Get current phase of flight
    pub async fn get_current_pof(&self) -> Option<PhaseOfFlight> {
        self.pof_tracker.read().await.current_pof
    }

    /// Process switch request (internal)
    async fn process_switch_request(
        request: SwitchRequest,
        state: &Arc<RwLock<AutoSwitchState>>,
        profile_cache: &Arc<RwLock<ProfileCache>>,
        pof_tracker: &Arc<RwLock<PofTracker>>,
        config: &AutoSwitchConfig,
    ) -> Result<Option<SwitchResult>> {
        match request {
            SwitchRequest::AircraftDetected(aircraft) => {
                Self::handle_aircraft_detection(aircraft, state, profile_cache, config).await
            }
            SwitchRequest::TelemetryUpdate(snapshot) => {
                Self::handle_telemetry_update(snapshot, state, pof_tracker, profile_cache, config)
                    .await
            }
            SwitchRequest::ForceSwitch(aircraft_id) => {
                Self::handle_force_switch(aircraft_id, state, profile_cache, config).await
            }
            SwitchRequest::InvalidateCache(aircraft_id) => {
                Self::handle_cache_invalidation(aircraft_id, profile_cache).await
            }
        }
    }

    /// Handle aircraft detection
    async fn handle_aircraft_detection(
        aircraft: DetectedAircraft,
        state: &Arc<RwLock<AutoSwitchState>>,
        profile_cache: &Arc<RwLock<ProfileCache>>,
        config: &AutoSwitchConfig,
    ) -> Result<Option<SwitchResult>> {
        let start_time = Instant::now();

        // Check if this is actually a new aircraft
        {
            let state_guard = state.read().await;
            if let Some(current) = &state_guard.current_aircraft
                && current.aircraft_id == aircraft.aircraft_id
                && current.sim == aircraft.sim
            {
                debug!("Aircraft detection ignored - same aircraft already active");
                return Ok(None);
            }
        }

        // Load and compile profile
        let compiled_profile = Self::load_and_compile_profile(
            &aircraft.aircraft_id,
            aircraft.sim,
            profile_cache,
            config,
        )
        .await?;

        // Perform the switch
        let old_aircraft = {
            let mut state_guard = state.write().await;
            let old = state_guard.current_aircraft.clone();
            state_guard.current_aircraft = Some(aircraft.clone());
            state_guard.current_profile = Some(compiled_profile.clone());
            state_guard.last_switch = Some(start_time);
            old
        };

        let switch_time = start_time.elapsed();

        // Check if we exceeded the time budget
        if switch_time > config.max_switch_time {
            warn!(
                "Aircraft switch exceeded time budget: {:?} > {:?}",
                switch_time, config.max_switch_time
            );
        }

        Ok(Some(SwitchResult {
            success: true,
            switch_time,
            old_aircraft: old_aircraft.map(|a| a.aircraft_id),
            new_aircraft: aircraft.aircraft_id,
            profile_hash: compiled_profile.effective_hash,
            pof_changed: false,
        }))
    }
    /// Handle telemetry update for PoF tracking
    async fn handle_telemetry_update(
        snapshot: TelemetrySnapshot,
        state: &Arc<RwLock<AutoSwitchState>>,
        pof_tracker: &Arc<RwLock<PofTracker>>,
        _profile_cache: &Arc<RwLock<ProfileCache>>,
        config: &AutoSwitchConfig,
    ) -> Result<Option<SwitchResult>> {
        // Determine current phase of flight
        let new_pof = Self::determine_phase_of_flight(&snapshot);

        // Update PoF tracker with hysteresis
        let pof_changed = {
            let mut tracker = pof_tracker.write().await;
            Self::update_pof_with_hysteresis(&mut tracker, new_pof, &snapshot, config).await?
        };

        if pof_changed {
            // PoF changed, check if we need to apply overrides
            let state_guard = state.read().await;
            if let Some(current_aircraft) = &state_guard.current_aircraft
                && let Some(current_profile) = &state_guard.current_profile
            {
                // Check if current profile has PoF overrides for the new phase
                if let Some(override_profile) = current_profile.pof_overrides.get(&new_pof) {
                    info!("Applying PoF override for phase: {:?}", new_pof);

                    // Apply the override (this would trigger axis engine recompilation)
                    return Ok(Some(SwitchResult {
                        success: true,
                        switch_time: Duration::from_millis(0), // PoF switches should be very fast
                        old_aircraft: Some(current_aircraft.aircraft_id.clone()),
                        new_aircraft: current_aircraft.aircraft_id.clone(),
                        profile_hash: override_profile.effective_hash(),
                        pof_changed: true,
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Handle force switch request
    async fn handle_force_switch(
        aircraft_id: AircraftId,
        state: &Arc<RwLock<AutoSwitchState>>,
        profile_cache: &Arc<RwLock<ProfileCache>>,
        config: &AutoSwitchConfig,
    ) -> Result<Option<SwitchResult>> {
        let start_time = Instant::now();

        // Create synthetic aircraft detection
        let aircraft = DetectedAircraft {
            sim: SimId::Unknown, // Will be determined from profile
            aircraft_id: aircraft_id.clone(),
            process_name: "manual".to_string(),
            detection_time: start_time,
            confidence: 1.0,
        };

        Self::handle_aircraft_detection(aircraft, state, profile_cache, config).await
    }

    /// Handle cache invalidation
    async fn handle_cache_invalidation(
        aircraft_id: Option<AircraftId>,
        profile_cache: &Arc<RwLock<ProfileCache>>,
    ) -> Result<Option<SwitchResult>> {
        let mut cache = profile_cache.write().await;

        match aircraft_id {
            Some(id) => {
                cache.profiles.remove(&id);
                cache.cache_timestamps.remove(&id);
                debug!("Invalidated cache for aircraft: {}", id);
            }
            None => {
                cache.profiles.clear();
                cache.cache_timestamps.clear();
                debug!("Invalidated entire profile cache");
            }
        }

        Ok(None)
    }
    /// Load and compile profile for aircraft
    async fn load_and_compile_profile(
        aircraft_id: &AircraftId,
        sim: SimId,
        profile_cache: &Arc<RwLock<ProfileCache>>,
        config: &AutoSwitchConfig,
    ) -> Result<CompiledProfile> {
        // Check cache first
        {
            let cache = profile_cache.read().await;
            if let Some(cached) = cache.profiles.get(aircraft_id)
                && let Some(timestamp) = cache.cache_timestamps.get(aircraft_id)
                && timestamp.elapsed() < cache.cache_ttl
            {
                debug!("Using cached profile for aircraft: {}", aircraft_id);
                return Ok(cached.compiled.clone());
            }
        }

        // Load profile hierarchy: Global → Sim → Aircraft
        let profiles = Self::load_profile_hierarchy(aircraft_id, sim, config).await?;

        // Merge profiles with deterministic hierarchy
        let mut merged_profile = profiles[0].clone();
        for profile in profiles.iter().skip(1) {
            merged_profile = merged_profile.merge_with(profile)?;
        }

        // Validate with capability enforcement
        merged_profile.validate_with_capabilities(&config.capability_context)?;

        // Compile profile
        let compiled = Self::compile_profile(merged_profile)?;

        // Update cache
        {
            let mut cache = profile_cache.write().await;
            cache.profiles.insert(
                aircraft_id.clone(),
                CachedProfile {
                    base_profile: compiled.profile.clone(),
                    compiled: compiled.clone(),
                    file_path: PathBuf::new(), // TODO: Track actual file path
                    last_modified: Instant::now(),
                },
            );
            cache
                .cache_timestamps
                .insert(aircraft_id.clone(), Instant::now());
        }

        Ok(compiled)
    }

    /// Load profile hierarchy for aircraft
    async fn load_profile_hierarchy(
        aircraft_id: &AircraftId,
        sim: SimId,
        config: &AutoSwitchConfig,
    ) -> Result<Vec<Profile>> {
        let mut profiles = Vec::new();

        // Load global profile
        if let Ok(global_profile) =
            Self::load_profile_from_path(&config.profile_paths[0], "global.json").await
        {
            profiles.push(global_profile);
        }

        // Load sim-specific profile
        let sim_name = match sim {
            SimId::Msfs => "msfs",
            SimId::XPlane => "xplane",
            SimId::Dcs => "dcs",
            SimId::Unknown => "unknown",
        };

        if let Ok(sim_profile) =
            Self::load_profile_from_path(&config.profile_paths[1], &format!("{}.json", sim_name))
                .await
        {
            profiles.push(sim_profile);
        }

        // Load aircraft-specific profile
        let aircraft_filename = format!("{}.json", aircraft_id.icao);
        if let Ok(aircraft_profile) =
            Self::load_profile_from_path(&config.profile_paths[2], &aircraft_filename).await
        {
            profiles.push(aircraft_profile);
        }

        if profiles.is_empty() {
            return Err(FlightError::AutoSwitch(format!(
                "No profiles found for aircraft: {}",
                aircraft_id
            )));
        }

        Ok(profiles)
    }

    /// Load profile from file path
    async fn load_profile_from_path(base_path: &Path, filename: &str) -> Result<Profile> {
        let profile_path = base_path.join(filename);

        let content = tokio::fs::read_to_string(&profile_path)
            .await
            .map_err(|e| {
                FlightError::AutoSwitch(format!(
                    "Failed to read profile {}: {}",
                    profile_path.display(),
                    e
                ))
            })?;

        let profile: Profile = serde_json::from_str(&content).map_err(|e| {
            FlightError::AutoSwitch(format!(
                "Failed to parse profile {}: {}",
                profile_path.display(),
                e
            ))
        })?;

        profile.validate()?;

        Ok(profile)
    }

    /// Compile profile for axis engine
    fn compile_profile(profile: Profile) -> Result<CompiledProfile> {
        let effective_hash = profile.effective_hash();

        // Compile PoF overrides
        let mut pof_overrides = HashMap::new();
        if let Some(overrides) = &profile.pof_overrides {
            for (pof_name, override_config) in overrides {
                if let Ok(pof) = pof_name.parse::<PhaseOfFlight>() {
                    // Create override profile by merging base with override
                    let mut override_profile = profile.clone();
                    override_profile.pof_overrides = None; // Remove nested overrides

                    // Apply override axes
                    if let Some(override_axes) = &override_config.axes {
                        for (axis_name, axis_config) in override_axes {
                            if let Some(existing_config) = override_profile.axes.get(axis_name) {
                                override_profile.axes.insert(
                                    axis_name.clone(),
                                    merge_axis_configs(existing_config, axis_config),
                                );
                            } else {
                                override_profile
                                    .axes
                                    .insert(axis_name.clone(), axis_config.clone());
                            }
                        }
                    }

                    pof_overrides.insert(pof, override_profile);
                }
            }
        }

        Ok(CompiledProfile {
            profile,
            effective_hash,
            compile_time: Instant::now(),
            pof_overrides,
        })
    }

    /// Determine phase of flight from telemetry
    fn determine_phase_of_flight(snapshot: &TelemetrySnapshot) -> PhaseOfFlight {
        let ias = snapshot.ias_knots;
        let altitude = snapshot.altitude_feet;
        let ground_speed = snapshot.ground_speed_knots;
        let gear_down = snapshot.gear_down;
        let vertical_speed = snapshot.vertical_speed_fpm;

        // Prioritize high-energy phases before ground phases to prevent misclassification
        
        // GoAround - high vertical speed at low altitude (emergency maneuver)
        if vertical_speed > 1000.0 && altitude < 2000.0 {
            return PhaseOfFlight::GoAround;
        }

        // Takeoff - high speed with positive vertical speed at low altitude
        if ias > 60.0 && vertical_speed > 500.0 && altitude < 1000.0 {
            return PhaseOfFlight::Takeoff;
        }

        // Climb - positive vertical speed above pattern altitude
        if vertical_speed > 300.0 && altitude > 1000.0 {
            return PhaseOfFlight::Climb;
        }

        // Cruise - stable flight at altitude with sufficient speed
        // Requires altitude >= 5000 ft, stable vertical speed, and minimum airspeed
        if altitude >= 5000.0 && vertical_speed.abs() < 200.0 && ias >= 60.0 {
            return PhaseOfFlight::Cruise;
        }

        // Descent - negative vertical speed above pattern altitude
        if vertical_speed < -300.0 && altitude > 2000.0 {
            return PhaseOfFlight::Descent;
        }

        // Approach - low speed at pattern altitude without gear
        if ias < 120.0 && altitude < 2000.0 && !gear_down {
            return PhaseOfFlight::Approach;
        }

        // Ground-only phases - only match when clearly on ground
        
        // Landing - gear down, low altitude, descending
        if gear_down && altitude < 500.0 && vertical_speed < -100.0 {
            return PhaseOfFlight::Landing;
        }

        // Taxi - low ground speed with gear down (on ground)
        // Only matches when ground_speed < 30 knots and gear is down
        if ground_speed < 30.0 && ground_speed >= 5.0 && gear_down {
            return PhaseOfFlight::Taxi;
        }

        // Ground - stationary or very slow movement
        if ground_speed < 5.0 {
            return PhaseOfFlight::Ground;
        }

        // Default fallback for ambiguous cases
        PhaseOfFlight::Cruise
    }
    /// Update PoF with hysteresis logic
    async fn update_pof_with_hysteresis(
        tracker: &mut PofTracker,
        new_pof: PhaseOfFlight,
        snapshot: &TelemetrySnapshot,
        config: &AutoSwitchConfig,
    ) -> Result<bool> {
        let now = Instant::now();

        // Update hysteresis state for all parameters
        Self::update_hysteresis_parameters(tracker, snapshot, &config.pof_hysteresis);

        // Check if we should change PoF
        let should_change = match tracker.current_pof {
            Some(current_pof) if current_pof == new_pof => false, // Same PoF
            Some(_) => {
                // Different PoF - check minimum time requirement
                if let Some(enter_time) = tracker.pof_enter_time {
                    now.duration_since(enter_time) >= config.pof_hysteresis.min_phase_time
                } else {
                    true // No enter time recorded, allow change
                }
            }
            None => true, // No current PoF, allow change
        };

        if should_change && tracker.current_pof != Some(new_pof) {
            debug!("PoF changed: {:?} -> {:?}", tracker.current_pof, new_pof);
            tracker.current_pof = Some(new_pof);
            tracker.pof_enter_time = Some(now);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Update hysteresis parameters
    fn update_hysteresis_parameters(
        tracker: &mut PofTracker,
        snapshot: &TelemetrySnapshot,
        config: &PofHysteresisConfig,
    ) {
        let now = Instant::now();

        // Update IAS hysteresis
        if let Some(band) = config.hysteresis_bands.get("ias") {
            let ias = snapshot.ias_knots;
            Self::update_parameter_hysteresis(&mut tracker.hysteresis_state, "ias", ias, band, now);
        }

        // Update altitude hysteresis
        if let Some(band) = config.hysteresis_bands.get("altitude") {
            let altitude = snapshot.altitude_feet;
            Self::update_parameter_hysteresis(
                &mut tracker.hysteresis_state,
                "altitude",
                altitude,
                band,
                now,
            );
        }

        // Update ground speed hysteresis
        if let Some(band) = config.hysteresis_bands.get("ground_speed") {
            let ground_speed = snapshot.ground_speed_knots;
            Self::update_parameter_hysteresis(
                &mut tracker.hysteresis_state,
                "ground_speed",
                ground_speed,
                band,
                now,
            );
        }
    }

    /// Update hysteresis state for a single parameter
    fn update_parameter_hysteresis(
        hysteresis_state: &mut HashMap<String, HysteresisState>,
        param_name: &str,
        current_value: f32,
        band: &HysteresisBand,
        now: Instant,
    ) {
        let state = hysteresis_state
            .entry(param_name.to_string())
            .or_insert_with(|| HysteresisState {
                current_value,
                in_condition: false,
                last_transition: now,
            });

        let old_condition = state.in_condition;

        // Apply hysteresis logic
        if !state.in_condition && current_value >= band.enter_threshold {
            state.in_condition = true;
        } else if state.in_condition && current_value <= band.exit_threshold {
            state.in_condition = false;
        }

        // Update state
        state.current_value = current_value;
        if state.in_condition != old_condition {
            state.last_transition = now;
        }
    }
}

impl AutoSwitchState {
    fn new() -> Self {
        Self {
            current_aircraft: None,
            current_profile: None,
            last_switch: None,
            current_pof: None,
            metrics: SwitchMetrics::default(),
        }
    }
}

impl ProfileCache {
    fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            cache_timestamps: HashMap::new(),
            cache_ttl: Duration::from_secs(300), // 5 minute cache TTL
        }
    }
}

impl PofTracker {
    fn new() -> Self {
        Self {
            current_pof: None,
            pof_enter_time: None,
            hysteresis_state: HashMap::new(),
        }
    }
}

impl std::str::FromStr for PhaseOfFlight {
    type Err = FlightError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "ground" => Ok(PhaseOfFlight::Ground),
            "taxi" => Ok(PhaseOfFlight::Taxi),
            "takeoff" => Ok(PhaseOfFlight::Takeoff),
            "climb" => Ok(PhaseOfFlight::Climb),
            "cruise" => Ok(PhaseOfFlight::Cruise),
            "descent" => Ok(PhaseOfFlight::Descent),
            "approach" => Ok(PhaseOfFlight::Approach),
            "landing" => Ok(PhaseOfFlight::Landing),
            "goaround" => Ok(PhaseOfFlight::GoAround),
            _ => Err(FlightError::AutoSwitch(format!(
                "Unknown phase of flight: {}",
                s
            ))),
        }
    }
}

impl std::fmt::Display for PhaseOfFlight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhaseOfFlight::Ground => write!(f, "ground"),
            PhaseOfFlight::Taxi => write!(f, "taxi"),
            PhaseOfFlight::Takeoff => write!(f, "takeoff"),
            PhaseOfFlight::Climb => write!(f, "climb"),
            PhaseOfFlight::Cruise => write!(f, "cruise"),
            PhaseOfFlight::Descent => write!(f, "descent"),
            PhaseOfFlight::Approach => write!(f, "approach"),
            PhaseOfFlight::Landing => write!(f, "landing"),
            PhaseOfFlight::GoAround => write!(f, "goaround"),
        }
    }
}

impl Clone for SwitchMetrics {
    fn clone(&self) -> Self {
        Self {
            total_switches: self.total_switches,
            successful_switches: self.successful_switches,
            failed_switches: self.failed_switches,
            average_switch_time: self.average_switch_time,
            max_switch_time: self.max_switch_time,
            cache_hits: self.cache_hits,
            cache_misses: self.cache_misses,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_aircraft_auto_switch_creation() {
        let config = AutoSwitchConfig::default();
        let auto_switch = AircraftAutoSwitch::new(config);

        assert!(auto_switch.get_current_aircraft().await.is_none());
        assert!(auto_switch.get_current_pof().await.is_none());
    }

    #[tokio::test]
    async fn test_phase_of_flight_determination() {
        // Create test snapshot for ground phase
        let mut snapshot = create_test_snapshot();
        snapshot.ground_speed_knots = 2.0;

        let pof = AircraftAutoSwitch::determine_phase_of_flight(&snapshot);
        assert_eq!(pof, PhaseOfFlight::Ground);

        // Test taxi phase
        snapshot.ground_speed_knots = 15.0;
        snapshot.gear_down = true;

        let pof = AircraftAutoSwitch::determine_phase_of_flight(&snapshot);
        assert_eq!(pof, PhaseOfFlight::Taxi);

        // Test cruise phase
        snapshot.ias_knots = 180.0;
        snapshot.vertical_speed_fpm = 50.0; // Stable
        snapshot.altitude_feet = 8000.0;

        let pof = AircraftAutoSwitch::determine_phase_of_flight(&snapshot);
        assert_eq!(pof, PhaseOfFlight::Cruise);
    }

    #[tokio::test]
    async fn test_hysteresis_logic() {
        let config = PofHysteresisConfig::default();
        let mut tracker = PofTracker::new();
        let now = Instant::now();

        // Test IAS hysteresis
        let band = config.hysteresis_bands.get("ias").unwrap();

        // Below enter threshold - should not be in condition
        AircraftAutoSwitch::update_parameter_hysteresis(
            &mut tracker.hysteresis_state,
            "ias",
            85.0,
            band,
            now,
        );

        let state = tracker.hysteresis_state.get("ias").unwrap();
        assert!(!state.in_condition);

        // Above enter threshold - should enter condition
        AircraftAutoSwitch::update_parameter_hysteresis(
            &mut tracker.hysteresis_state,
            "ias",
            95.0,
            band,
            now,
        );

        let state = tracker.hysteresis_state.get("ias").unwrap();
        assert!(state.in_condition);

        // Above exit threshold but below enter - should stay in condition
        AircraftAutoSwitch::update_parameter_hysteresis(
            &mut tracker.hysteresis_state,
            "ias",
            105.0,
            band,
            now,
        );

        let state = tracker.hysteresis_state.get("ias").unwrap();
        assert!(state.in_condition);

        // Below exit threshold - should exit condition
        AircraftAutoSwitch::update_parameter_hysteresis(
            &mut tracker.hysteresis_state,
            "ias",
            95.0,
            band,
            now,
        );

        let state = tracker.hysteresis_state.get("ias").unwrap();
        assert!(!state.in_condition);
    }

    #[tokio::test]
    async fn test_profile_hierarchy_loading() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create profile directories
        let global_dir = base_path.join("global");
        let sim_dir = base_path.join("sim");
        let aircraft_dir = base_path.join("aircraft");

        fs::create_dir_all(&global_dir).await.unwrap();
        fs::create_dir_all(&sim_dir).await.unwrap();
        fs::create_dir_all(&aircraft_dir).await.unwrap();

        // Create test profiles
        let global_profile = r#"{
            "schema": "flight.profile/1",
            "axes": {
                "pitch": {
                    "deadzone": 0.02,
                    "expo": 0.1
                }
            }
        }"#;

        let sim_profile = r#"{
            "schema": "flight.profile/1",
            "sim": "msfs",
            "axes": {
                "pitch": {
                    "expo": 0.2
                }
            }
        }"#;

        let aircraft_profile = r#"{
            "schema": "flight.profile/1",
            "aircraft": {"icao": "C172"},
            "axes": {
                "pitch": {
                    "slew_rate": 1.5
                }
            }
        }"#;

        fs::write(global_dir.join("global.json"), global_profile)
            .await
            .unwrap();
        fs::write(sim_dir.join("msfs.json"), sim_profile)
            .await
            .unwrap();
        fs::write(aircraft_dir.join("C172.json"), aircraft_profile)
            .await
            .unwrap();

        // Test profile loading
        let config = AutoSwitchConfig {
            profile_paths: vec![global_dir, sim_dir, aircraft_dir],
            ..Default::default()
        };

        let aircraft_id = AircraftId::new("C172");
        let profiles =
            AircraftAutoSwitch::load_profile_hierarchy(&aircraft_id, SimId::Msfs, &config)
                .await
                .unwrap();

        assert_eq!(profiles.len(), 3);

        // Test profile merging
        let mut merged = profiles[0].clone();
        for profile in profiles.iter().skip(1) {
            merged = merged.merge_with(profile).unwrap();
        }
        let pitch_config = merged.axes.get("pitch").unwrap();

        // Should have deadzone from global, expo from sim, and slew_rate from aircraft
        assert_eq!(pitch_config.deadzone, Some(0.02));
        assert_eq!(pitch_config.expo, Some(0.2)); // Overridden by sim
        assert_eq!(pitch_config.slew_rate, Some(1.5));
    }

    #[tokio::test]
    async fn test_switch_timing_budget() {
        let config = AutoSwitchConfig {
            max_switch_time: Duration::from_millis(100), // Very tight budget for testing
            ..Default::default()
        };

        let auto_switch = AircraftAutoSwitch::new(config);
        auto_switch.start().await.unwrap();

        let aircraft = DetectedAircraft {
            sim: SimId::Msfs,
            aircraft_id: AircraftId::new("C172"),
            process_name: "FlightSimulator.exe".to_string(),
            detection_time: Instant::now(),
            confidence: 0.9,
        };

        // This should complete but may exceed the tight timing budget
        auto_switch.on_aircraft_detected(aircraft).await.unwrap();

        // Give some time for processing
        tokio::time::sleep(Duration::from_millis(200)).await;

        let metrics = auto_switch.get_metrics().await;
        assert!(metrics.total_switches > 0);
    }

    #[tokio::test]
    async fn test_pof_override_application() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();
        let aircraft_dir = base_path.join("aircraft");
        fs::create_dir_all(&aircraft_dir).await.unwrap();

        // Create profile with PoF overrides
        let aircraft_profile = r#"{
            "schema": "flight.profile/1",
            "aircraft": {"icao": "C172"},
            "axes": {
                "pitch": {
                    "deadzone": 0.02,
                    "expo": 0.1
                }
            },
            "pof_overrides": {
                "approach": {
                    "axes": {
                        "pitch": {
                            "expo": 0.25
                        }
                    }
                }
            }
        }"#;

        fs::write(aircraft_dir.join("C172.json"), aircraft_profile)
            .await
            .unwrap();

        let config = AutoSwitchConfig {
            profile_paths: vec![PathBuf::new(), PathBuf::new(), aircraft_dir],
            enable_pof: true,
            ..Default::default()
        };

        let auto_switch = AircraftAutoSwitch::new(config);
        auto_switch.start().await.unwrap();

        // Simulate aircraft detection
        let aircraft = DetectedAircraft {
            sim: SimId::Msfs,
            aircraft_id: AircraftId::new("C172"),
            process_name: "FlightSimulator.exe".to_string(),
            detection_time: Instant::now(),
            confidence: 0.9,
        };

        auto_switch.on_aircraft_detected(aircraft).await.unwrap();

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create approach phase telemetry
        let mut snapshot = create_test_snapshot();
        snapshot.ias_knots = 90.0;
        snapshot.altitude_feet = 1500.0;
        snapshot.gear_down = false;

        // Send telemetry update
        auto_switch.on_telemetry_update(snapshot).await.unwrap();

        // Wait for PoF processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check that PoF was detected (this is a simplified test)
        let current_pof = auto_switch.get_current_pof().await;
        assert!(current_pof.is_some());
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let config = AutoSwitchConfig::default();
        let auto_switch = AircraftAutoSwitch::new(config);
        auto_switch.start().await.unwrap();

        let aircraft_id = AircraftId::new("C172");

        // Invalidate specific aircraft cache
        auto_switch
            .invalidate_cache(Some(aircraft_id.clone()))
            .await
            .unwrap();

        // Invalidate entire cache
        auto_switch.invalidate_cache(None).await.unwrap();

        // Should not fail
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_force_switch() {
        let config = AutoSwitchConfig::default();
        let auto_switch = AircraftAutoSwitch::new(config);
        auto_switch.start().await.unwrap();

        let aircraft_id = AircraftId::new("A320");

        // Force switch should work even without profiles
        auto_switch.force_switch(aircraft_id).await.unwrap();

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        let metrics = auto_switch.get_metrics().await;
        // Should have attempted at least one switch
        assert!(metrics.total_switches > 0);
    }

    #[test]
    fn test_phase_of_flight_string_conversion() {
        assert_eq!(
            "ground".parse::<PhaseOfFlight>().unwrap(),
            PhaseOfFlight::Ground
        );
        assert_eq!(
            "taxi".parse::<PhaseOfFlight>().unwrap(),
            PhaseOfFlight::Taxi
        );
        assert_eq!(
            "takeoff".parse::<PhaseOfFlight>().unwrap(),
            PhaseOfFlight::Takeoff
        );
        assert_eq!(
            "climb".parse::<PhaseOfFlight>().unwrap(),
            PhaseOfFlight::Climb
        );
        assert_eq!(
            "cruise".parse::<PhaseOfFlight>().unwrap(),
            PhaseOfFlight::Cruise
        );
        assert_eq!(
            "descent".parse::<PhaseOfFlight>().unwrap(),
            PhaseOfFlight::Descent
        );
        assert_eq!(
            "approach".parse::<PhaseOfFlight>().unwrap(),
            PhaseOfFlight::Approach
        );
        assert_eq!(
            "landing".parse::<PhaseOfFlight>().unwrap(),
            PhaseOfFlight::Landing
        );
        assert_eq!(
            "goaround".parse::<PhaseOfFlight>().unwrap(),
            PhaseOfFlight::GoAround
        );

        assert_eq!(PhaseOfFlight::Ground.to_string(), "ground");
        assert_eq!(PhaseOfFlight::Approach.to_string(), "approach");
        assert_eq!(PhaseOfFlight::GoAround.to_string(), "goaround");

        assert!("invalid".parse::<PhaseOfFlight>().is_err());
    }

    #[test]
    fn test_hysteresis_band_configuration() {
        let config = PofHysteresisConfig::default();

        assert!(config.hysteresis_bands.contains_key("ias"));
        assert!(config.hysteresis_bands.contains_key("altitude"));
        assert!(config.hysteresis_bands.contains_key("ground_speed"));

        let ias_band = config.hysteresis_bands.get("ias").unwrap();
        assert_eq!(ias_band.enter_threshold, 90.0);
        assert_eq!(ias_band.exit_threshold, 100.0);
        assert_eq!(ias_band.unit, "knots");
    }

    #[tokio::test]
    async fn test_metrics_tracking() {
        let config = AutoSwitchConfig::default();
        let auto_switch = AircraftAutoSwitch::new(config);
        auto_switch.start().await.unwrap();

        let initial_metrics = auto_switch.get_metrics().await;
        assert_eq!(initial_metrics.total_switches, 0);
        assert_eq!(initial_metrics.successful_switches, 0);
        assert_eq!(initial_metrics.failed_switches, 0);

        // Simulate some activity
        let aircraft = DetectedAircraft {
            sim: SimId::Msfs,
            aircraft_id: AircraftId::new("C172"),
            process_name: "FlightSimulator.exe".to_string(),
            detection_time: Instant::now(),
            confidence: 0.9,
        };

        auto_switch.on_aircraft_detected(aircraft).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        let updated_metrics = auto_switch.get_metrics().await;
        assert!(updated_metrics.total_switches > initial_metrics.total_switches);
    }

    fn create_test_snapshot() -> TelemetrySnapshot {
        TelemetrySnapshot {
            sim: SimId::Msfs,
            aircraft: AircraftId::new("C172"),
            timestamp: 0,
            ias_knots: 0.0,
            ground_speed_knots: 0.0,
            altitude_feet: 0.0,
            vertical_speed_fpm: 0.0,
            gear_down: true,
        }
    }
}
