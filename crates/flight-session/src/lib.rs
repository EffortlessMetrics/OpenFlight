// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Aircraft Auto-Switch System
//!
//! Implements automatic profile switching based on aircraft detection with ≤500ms response time.
//! Provides process+aircraft detection, profile resolution with merge hierarchy, and
//! compile-and-swap system for profile changes with PoF hysteresis logic.

use flight_profile::{CapabilityContext, CapabilityMode, Profile, merge_axis_configs};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Profile validation error: {0}")]
    ProfileValidation(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Profile error: {0}")]
    Profile(#[from] flight_profile::ProfileError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Auto-switch error: {0}")]
    AutoSwitch(String),
}

pub type Result<T> = std::result::Result<T, SessionError>;

pub use flight_process_detection::SimId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

/// Simulator identifier
// Removed SimId definition here, now using crate::process_detection::SimId

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
    /// Number of consecutive frames required to confirm phase transition (at 250Hz, 3-5 frames = 12-20ms)
    pub consecutive_frames_required: u32,
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
    /// Consecutive frames counter for phase stability
    consecutive_frames: HashMap<PhaseOfFlight, u32>,
    /// Candidate phase being evaluated
    candidate_pof: Option<PhaseOfFlight>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aircraft_id_basics() {
        let id = AircraftId::new("C172");
        assert_eq!(id.to_string(), "C172");

        let id_var = AircraftId::with_variant("A320", "NEO");
        assert_eq!(id_var.to_string(), "A320-NEO");
    }

    use proptest::prelude::*;

    proptest! {
        // Test AircraftId Display implementation
        #[test]
        fn prop_aircraft_id_display(icao in "[A-Z0-9]{4}", variant in proptest::option::of("[A-Z0-9-]{1,10}")) {
            let id = if let Some(ref v) = variant {
                AircraftId::with_variant(icao.clone(), v.clone())
            } else {
                AircraftId::new(icao.clone())
            };

            let display = id.to_string();

            if let Some(v) = variant {
                prop_assert_eq!(display, format!("{}-{}", icao, v));
            } else {
                prop_assert_eq!(display, icao);
            }
        }

        // Test PofHysteresisConfig validation or structure (basic integrity)
        #[test]
        fn prop_hysteresis_band_integrity(
            enter in -1000.0f32..1000.0,
            exit in -1000.0f32..1000.0,
            unit in "[a-z]+"
        ) {
            let band = HysteresisBand {
                enter_threshold: enter,
                exit_threshold: exit,
                unit: unit.clone(),
            };

            prop_assert_eq!(band.enter_threshold, enter);
            prop_assert_eq!(band.exit_threshold, exit);
            prop_assert_eq!(band.unit, unit);
        }

        // Test state transitions or logic if we can access enough internals
        // For now, let's verify serialization roundtrips for key structs
        #[test]
        fn prop_telemetry_snapshot_roundtrip(
            timestamp in any::<u64>(),
            ias in 0.0f32..1000.0,
            gs in 0.0f32..1000.0,
            alt in -1000.0f32..60000.0,
            vs in -10000.0f32..10000.0,
            gear in any::<bool>()
        ) {
            let snap = TelemetrySnapshot {
                sim: SimId::Msfs,
                aircraft: AircraftId::new("C172"),
                timestamp,
                ias_knots: ias,
                ground_speed_knots: gs,
                altitude_feet: alt,
                vertical_speed_fpm: vs,
                gear_down: gear,
            };

            let serialized = serde_json::to_string(&snap).unwrap();
            let deserialized: TelemetrySnapshot = serde_json::from_str(&serialized).unwrap();

            prop_assert_eq!(snap.timestamp, deserialized.timestamp);
            prop_assert!((snap.ias_knots - deserialized.ias_knots).abs() < 0.001);
            prop_assert_eq!(snap.gear_down, deserialized.gear_down);
        }
    }
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
    pub committed_switches: u64,
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
            consecutive_frames_required: 4, // At 250Hz, 4 frames = 16ms stability requirement
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
                SessionError::AutoSwitch("Auto-switch already started".to_string())
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
                SessionError::AutoSwitch(format!("Failed to send aircraft detection: {}", e))
            })?;
        Ok(())
    }

    /// Handle telemetry update for PoF tracking
    pub async fn on_telemetry_update(&self, snapshot: TelemetrySnapshot) -> Result<()> {
        if self.config.enable_pof {
            self.switch_tx
                .send(SwitchRequest::TelemetryUpdate(snapshot))
                .map_err(|e| {
                    SessionError::AutoSwitch(format!("Failed to send telemetry update: {}", e))
                })?;
        }
        Ok(())
    }

    /// Force switch to specific aircraft (for testing/manual override)
    pub async fn force_switch(&self, aircraft_id: AircraftId) -> Result<()> {
        info!("Forcing switch to aircraft: {}", aircraft_id);
        self.switch_tx
            .send(SwitchRequest::ForceSwitch(aircraft_id))
            .map_err(|e| SessionError::AutoSwitch(format!("Failed to send force switch: {}", e)))?;
        Ok(())
    }

    /// Invalidate profile cache
    pub async fn invalidate_cache(&self, aircraft_id: Option<AircraftId>) -> Result<()> {
        debug!("Invalidating profile cache for: {:?}", aircraft_id);
        self.switch_tx
            .send(SwitchRequest::InvalidateCache(aircraft_id))
            .map_err(|e| {
                SessionError::AutoSwitch(format!("Failed to send cache invalidation: {}", e))
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

            // Check if aircraft ID changed (profile switch semantics: count only on ID change)
            let aircraft_id_changed = state_guard
                .current_aircraft
                .as_ref()
                .map(|a| &a.aircraft_id)
                != Some(&aircraft.aircraft_id);

            state_guard.current_aircraft = Some(aircraft.clone());
            state_guard.current_profile = Some(compiled_profile.clone());
            state_guard.last_switch = Some(start_time);

            // Increment committed_switches counter if aircraft ID changed
            if aircraft_id_changed {
                state_guard.metrics.committed_switches = state_guard
                    .metrics
                    .committed_switches
                    .checked_add(1)
                    .unwrap_or_else(|| {
                        warn!("committed_switches counter overflow, resetting to 0");
                        0
                    });
            }

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
            return Err(SessionError::AutoSwitch(format!(
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
                SessionError::AutoSwitch(format!(
                    "Failed to read profile {}: {}",
                    profile_path.display(),
                    e
                ))
            })?;

        let profile: Profile = serde_json::from_str(&content).map_err(|e| {
            SessionError::AutoSwitch(format!(
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
        if (5.0..30.0).contains(&ground_speed) && gear_down {
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

        // Check if this is the same as current PoF
        if tracker.current_pof == Some(new_pof) {
            // Reset candidate and consecutive frames since we're stable
            tracker.candidate_pof = None;
            tracker.consecutive_frames.clear();

            #[cfg(test)]
            debug!("PoF stable: {:?}", new_pof);

            return Ok(false);
        }

        // Check if this is a new candidate or continuation of existing candidate
        let frames_required = config.pof_hysteresis.consecutive_frames_required;

        if tracker.candidate_pof == Some(new_pof) {
            // Same candidate - increment counter
            let count = tracker.consecutive_frames.entry(new_pof).or_insert(0);
            *count += 1;

            #[cfg(test)]
            debug!(
                "PoF candidate {:?} frame count: {}/{}",
                new_pof, count, frames_required
            );

            // Check if we've met the consecutive frames requirement
            if *count >= frames_required {
                // Also check minimum time requirement if we have a current PoF
                let time_ok = match tracker.current_pof {
                    Some(_) => {
                        if let Some(enter_time) = tracker.pof_enter_time {
                            now.duration_since(enter_time) >= config.pof_hysteresis.min_phase_time
                        } else {
                            true
                        }
                    }
                    None => true, // No current PoF, allow change
                };

                if time_ok {
                    // Transition confirmed
                    #[cfg(test)]
                    debug!(
                        "PoF transition confirmed: {:?} -> {:?} (after {} consecutive frames)",
                        tracker.current_pof, new_pof, count
                    );

                    tracker.current_pof = Some(new_pof);
                    tracker.pof_enter_time = Some(now);
                    tracker.candidate_pof = None;
                    tracker.consecutive_frames.clear();
                    return Ok(true);
                } else {
                    #[cfg(test)]
                    debug!("PoF transition delayed: minimum time requirement not met");
                }
            }
        } else {
            // New candidate - reset counters and start tracking
            #[cfg(test)]
            debug!(
                "New PoF candidate: {:?} (previous candidate: {:?})",
                new_pof, tracker.candidate_pof
            );

            tracker.candidate_pof = Some(new_pof);
            tracker.consecutive_frames.clear();
            tracker.consecutive_frames.insert(new_pof, 1);
        }

        Ok(false)
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
            consecutive_frames: HashMap::new(),
            candidate_pof: None,
        }
    }
}

impl std::str::FromStr for PhaseOfFlight {
    type Err = SessionError;

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
            _ => Err(SessionError::AutoSwitch(format!(
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
            committed_switches: self.committed_switches,
            average_switch_time: self.average_switch_time,
            max_switch_time: self.max_switch_time,
            cache_hits: self.cache_hits,
            cache_misses: self.cache_misses,
        }
    }
}
#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // Test AircraftId Display implementation
        #[test]
        fn prop_aircraft_id_display(icao in "[A-Z0-9]{4}", variant in proptest::option::of("[A-Z0-9-]{1,10}")) {
            let id = if let Some(ref v) = variant {
                AircraftId::with_variant(icao.clone(), v.clone())
            } else {
                AircraftId::new(icao.clone())
            };

            let display = id.to_string();

            if let Some(v) = variant {
                prop_assert_eq!(display, format!("{}-{}", icao, v));
            } else {
                prop_assert_eq!(display, icao);
            }
        }
    }
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
    // Use test fixture instead of creating temporary profiles
    let config = test_profile_repo();
    let aircraft_id = AircraftId::new("C172");

    // Load the C172 profile from fixtures
    let profiles = AircraftAutoSwitch::load_profile_hierarchy(&aircraft_id, SimId::Msfs, &config)
        .await
        .unwrap();

    // Should load at least the aircraft profile
    assert!(!profiles.is_empty());

    // Test that the profile has expected axes
    let aircraft_profile = &profiles[0];
    assert!(aircraft_profile.axes.contains_key("pitch"));

    // Verify the profile has pof_overrides
    assert!(aircraft_profile.pof_overrides.is_some());
    let pof_overrides = aircraft_profile.pof_overrides.as_ref().unwrap();
    assert!(pof_overrides.contains_key("cruise"));
    assert!(pof_overrides.contains_key("approach"));
}

#[tokio::test]
async fn test_switch_timing_budget() {
    let mut config = test_profile_repo();
    config.max_switch_time = Duration::from_millis(100); // Very tight budget for testing

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
    // Use test fixture with C172 profile that has PoF overrides
    let config = test_profile_repo();
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

    // Send multiple telemetry updates to satisfy consecutive frames requirement
    // Default config requires 4 consecutive frames
    for _ in 0..5 {
        auto_switch
            .on_telemetry_update(snapshot.clone())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await; // Simulate frame timing at ~200Hz
    }

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
    let config = test_profile_repo();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    let aircraft_id = AircraftId::new("C172");

    // Force switch to C172 which has a profile in fixtures
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

    // Verify consecutive frames requirement
    assert_eq!(config.consecutive_frames_required, 4);
}

#[tokio::test]
async fn test_consecutive_frames_hysteresis() {
    let mut config = AutoSwitchConfig::default();
    config.pof_hysteresis.consecutive_frames_required = 3;
    config.pof_hysteresis.min_phase_time = Duration::from_millis(100);

    let mut tracker = PofTracker::new();

    // Start in Ground phase
    let mut snapshot = create_test_snapshot();
    snapshot.ground_speed_knots = 2.0;
    snapshot.altitude_feet = 0.0;

    let pof = AircraftAutoSwitch::determine_phase_of_flight(&snapshot);
    assert_eq!(pof, PhaseOfFlight::Ground);

    // Initialize current PoF
    tracker.current_pof = Some(PhaseOfFlight::Ground);
    tracker.pof_enter_time = Some(Instant::now());

    // Wait to satisfy min_phase_time
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Change to Cruise conditions
    snapshot.ias_knots = 180.0;
    snapshot.vertical_speed_fpm = 50.0;
    snapshot.altitude_feet = 8000.0;

    let new_pof = AircraftAutoSwitch::determine_phase_of_flight(&snapshot);
    assert_eq!(new_pof, PhaseOfFlight::Cruise);

    // Frame 1 - should not transition yet
    let changed =
        AircraftAutoSwitch::update_pof_with_hysteresis(&mut tracker, new_pof, &snapshot, &config)
            .await
            .unwrap();
    assert!(!changed, "Should not transition on first frame");
    assert_eq!(tracker.current_pof, Some(PhaseOfFlight::Ground));
    assert_eq!(tracker.candidate_pof, Some(PhaseOfFlight::Cruise));
    assert_eq!(
        *tracker
            .consecutive_frames
            .get(&PhaseOfFlight::Cruise)
            .unwrap(),
        1
    );

    // Frame 2 - still not enough
    let changed =
        AircraftAutoSwitch::update_pof_with_hysteresis(&mut tracker, new_pof, &snapshot, &config)
            .await
            .unwrap();
    assert!(!changed, "Should not transition on second frame");
    assert_eq!(tracker.current_pof, Some(PhaseOfFlight::Ground));
    assert_eq!(
        *tracker
            .consecutive_frames
            .get(&PhaseOfFlight::Cruise)
            .unwrap(),
        2
    );

    // Frame 3 - should transition now (3 consecutive frames)
    let changed =
        AircraftAutoSwitch::update_pof_with_hysteresis(&mut tracker, new_pof, &snapshot, &config)
            .await
            .unwrap();
    assert!(changed, "Should transition on third consecutive frame");
    assert_eq!(tracker.current_pof, Some(PhaseOfFlight::Cruise));
    assert_eq!(tracker.candidate_pof, None);
    assert!(tracker.consecutive_frames.is_empty());
}

#[tokio::test]
async fn test_phase_flip_flop_prevention() {
    let mut config = AutoSwitchConfig::default();
    config.pof_hysteresis.consecutive_frames_required = 4;
    config.pof_hysteresis.min_phase_time = Duration::from_millis(100);

    let mut tracker = PofTracker::new();
    tracker.current_pof = Some(PhaseOfFlight::Cruise);
    tracker.pof_enter_time = Some(Instant::now());

    // Wait to satisfy min_phase_time
    tokio::time::sleep(Duration::from_millis(150)).await;

    let mut snapshot = create_test_snapshot();

    // Simulate flip-flopping between Cruise and Descent
    for i in 0..10 {
        if i % 2 == 0 {
            // Cruise conditions
            snapshot.ias_knots = 180.0;
            snapshot.vertical_speed_fpm = 50.0;
            snapshot.altitude_feet = 8000.0;
        } else {
            // Descent conditions (but only for 1 frame)
            snapshot.ias_knots = 180.0;
            snapshot.vertical_speed_fpm = -400.0;
            snapshot.altitude_feet = 7900.0;
        }

        let pof = AircraftAutoSwitch::determine_phase_of_flight(&snapshot);
        let changed =
            AircraftAutoSwitch::update_pof_with_hysteresis(&mut tracker, pof, &snapshot, &config)
                .await
                .unwrap();

        // Should never transition because we never get 4 consecutive frames
        assert!(
            !changed,
            "Should not transition during flip-flop at iteration {}",
            i
        );
        assert_eq!(
            tracker.current_pof,
            Some(PhaseOfFlight::Cruise),
            "Should remain in Cruise during flip-flop at iteration {}",
            i
        );
    }
}

#[tokio::test]
async fn test_consecutive_frames_reset_on_different_candidate() {
    let mut config = AutoSwitchConfig::default();
    config.pof_hysteresis.consecutive_frames_required = 3;
    config.pof_hysteresis.min_phase_time = Duration::from_millis(100);

    let mut tracker = PofTracker::new();
    tracker.current_pof = Some(PhaseOfFlight::Ground);
    tracker.pof_enter_time = Some(Instant::now());

    // Wait to satisfy min_phase_time
    tokio::time::sleep(Duration::from_millis(150)).await;

    let mut snapshot = create_test_snapshot();

    // Build up frames for Cruise
    snapshot.ias_knots = 180.0;
    snapshot.vertical_speed_fpm = 50.0;
    snapshot.altitude_feet = 8000.0;

    let cruise_pof = AircraftAutoSwitch::determine_phase_of_flight(&snapshot);
    assert_eq!(cruise_pof, PhaseOfFlight::Cruise);

    // Frame 1 for Cruise
    AircraftAutoSwitch::update_pof_with_hysteresis(&mut tracker, cruise_pof, &snapshot, &config)
        .await
        .unwrap();
    assert_eq!(
        *tracker
            .consecutive_frames
            .get(&PhaseOfFlight::Cruise)
            .unwrap(),
        1
    );

    // Frame 2 for Cruise
    AircraftAutoSwitch::update_pof_with_hysteresis(&mut tracker, cruise_pof, &snapshot, &config)
        .await
        .unwrap();
    assert_eq!(
        *tracker
            .consecutive_frames
            .get(&PhaseOfFlight::Cruise)
            .unwrap(),
        2
    );

    // Suddenly change to Descent (interrupts Cruise candidate)
    snapshot.vertical_speed_fpm = -400.0;
    snapshot.altitude_feet = 7500.0;

    let descent_pof = AircraftAutoSwitch::determine_phase_of_flight(&snapshot);
    assert_eq!(descent_pof, PhaseOfFlight::Descent);

    // Frame 1 for Descent - should reset Cruise counter
    AircraftAutoSwitch::update_pof_with_hysteresis(&mut tracker, descent_pof, &snapshot, &config)
        .await
        .unwrap();

    assert_eq!(tracker.candidate_pof, Some(PhaseOfFlight::Descent));
    assert_eq!(
        *tracker
            .consecutive_frames
            .get(&PhaseOfFlight::Descent)
            .unwrap(),
        1
    );
    assert!(
        !tracker
            .consecutive_frames
            .contains_key(&PhaseOfFlight::Cruise),
        "Cruise counter should be cleared"
    );
}

#[tokio::test]
async fn test_min_phase_time_with_consecutive_frames() {
    let mut config = AutoSwitchConfig::default();
    config.pof_hysteresis.consecutive_frames_required = 2;
    config.pof_hysteresis.min_phase_time = Duration::from_millis(200);

    let mut tracker = PofTracker::new();
    tracker.current_pof = Some(PhaseOfFlight::Ground);
    tracker.pof_enter_time = Some(Instant::now());

    let mut snapshot = create_test_snapshot();
    snapshot.ias_knots = 180.0;
    snapshot.vertical_speed_fpm = 50.0;
    snapshot.altitude_feet = 8000.0;

    let cruise_pof = AircraftAutoSwitch::determine_phase_of_flight(&snapshot);

    // Get 2 consecutive frames immediately (before min_phase_time)
    AircraftAutoSwitch::update_pof_with_hysteresis(&mut tracker, cruise_pof, &snapshot, &config)
        .await
        .unwrap();

    let changed = AircraftAutoSwitch::update_pof_with_hysteresis(
        &mut tracker,
        cruise_pof,
        &snapshot,
        &config,
    )
    .await
    .unwrap();

    // Should not transition yet because min_phase_time not satisfied
    assert!(!changed, "Should not transition before min_phase_time");
    assert_eq!(tracker.current_pof, Some(PhaseOfFlight::Ground));

    // Wait for min_phase_time
    tokio::time::sleep(Duration::from_millis(250)).await;

    // Now try again with 2 consecutive frames
    tracker.candidate_pof = None;
    tracker.consecutive_frames.clear();

    AircraftAutoSwitch::update_pof_with_hysteresis(&mut tracker, cruise_pof, &snapshot, &config)
        .await
        .unwrap();

    let changed = AircraftAutoSwitch::update_pof_with_hysteresis(
        &mut tracker,
        cruise_pof,
        &snapshot,
        &config,
    )
    .await
    .unwrap();

    // Should transition now
    assert!(changed, "Should transition after min_phase_time");
    assert_eq!(tracker.current_pof, Some(PhaseOfFlight::Cruise));
}

#[tokio::test]
async fn test_stable_phase_resets_counters() {
    let mut config = AutoSwitchConfig::default();
    config.pof_hysteresis.consecutive_frames_required = 3;
    config.pof_hysteresis.min_phase_time = Duration::from_millis(100);

    let mut tracker = PofTracker::new();
    tracker.current_pof = Some(PhaseOfFlight::Cruise);
    tracker.pof_enter_time = Some(Instant::now());

    let mut snapshot = create_test_snapshot();
    snapshot.ias_knots = 180.0;
    snapshot.vertical_speed_fpm = 50.0;
    snapshot.altitude_feet = 8000.0;

    let cruise_pof = AircraftAutoSwitch::determine_phase_of_flight(&snapshot);
    assert_eq!(cruise_pof, PhaseOfFlight::Cruise);

    // Send same phase multiple times
    for _ in 0..5 {
        let changed = AircraftAutoSwitch::update_pof_with_hysteresis(
            &mut tracker,
            cruise_pof,
            &snapshot,
            &config,
        )
        .await
        .unwrap();

        assert!(!changed, "Should not change when stable");
        assert_eq!(tracker.current_pof, Some(PhaseOfFlight::Cruise));
        assert_eq!(
            tracker.candidate_pof, None,
            "Should have no candidate when stable"
        );
        assert!(
            tracker.consecutive_frames.is_empty(),
            "Should have no frame counters when stable"
        );
    }
}

#[tokio::test]
async fn test_metrics_tracking() {
    let config = test_profile_repo();
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

#[tokio::test]
async fn test_committed_switches_counter() {
    let config = test_profile_repo();
    let auto_switch = AircraftAutoSwitch::new(config);
    auto_switch.start().await.unwrap();

    let initial_metrics = auto_switch.get_metrics().await;
    assert_eq!(initial_metrics.committed_switches, 0);

    // First switch to C172
    let aircraft1 = DetectedAircraft {
        sim: SimId::Msfs,
        aircraft_id: AircraftId::new("C172"),
        process_name: "FlightSimulator.exe".to_string(),
        detection_time: Instant::now(),
        confidence: 0.9,
    };

    auto_switch
        .on_aircraft_detected(aircraft1.clone())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let metrics_after_first = auto_switch.get_metrics().await;
    assert_eq!(
        metrics_after_first.committed_switches, 1,
        "First switch should increment counter"
    );

    // Switch to same aircraft (C172) - should NOT increment
    auto_switch.on_aircraft_detected(aircraft1).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let metrics_after_same = auto_switch.get_metrics().await;
    assert_eq!(
        metrics_after_same.committed_switches, 1,
        "Same aircraft should not increment counter"
    );

    // Force switch to same aircraft - should NOT increment (Option 1 semantics)
    auto_switch
        .force_switch(AircraftId::new("C172"))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let metrics_after_force_same = auto_switch.get_metrics().await;
    assert_eq!(
        metrics_after_force_same.committed_switches, 1,
        "Force switch to same aircraft should not increment counter"
    );
}

#[cfg(test)]
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

/// Test helper function that returns AutoSwitchConfig pointing to fixtures directory
#[cfg(test)]
fn test_profile_repo() -> AutoSwitchConfig {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/profiles");

    AutoSwitchConfig {
        profile_paths: vec![
            fixture_dir.clone(), // global
            fixture_dir.clone(), // sim
            fixture_dir,         // aircraft
        ],
        ..Default::default()
    }
}
