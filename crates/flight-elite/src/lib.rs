// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Elite: Dangerous journal and `Status.json` adapter for OpenFlight.
//!
//! Elite Dangerous writes live game-state data to two file-based APIs in the
//! player's saved-games folder:
//!
//! | File | Update frequency | Content |
//! |------|-----------------|---------|
//! | `Status.json` | ~4 Hz | Current flags, pips, fuel, cargo, GUI focus |
//! | `Journal.<date>.log` | Event-driven | Typed events (FSD jumps, docking, etc.) |
//!
//! This adapter polls `Status.json` periodically and parses the supported
//! journal events that are relevant to the Flight Hub bus.
//!
//! ## Default journal directory
//!
//! ```text
//! %USERPROFILE%\Saved Games\Frontier Developments\Elite Dangerous
//! ```
//!
//! On Linux (via Proton/Wine), the equivalent Proton prefix path is used.
//!
//! ## Status flags (bit positions)
//!
//! | Bit | Meaning |
//! |-----|---------|
//! | 0   | Docked |
//! | 1   | Landed on planet surface |
//! | 2   | Landing gear deployed |
//! | 3   | Shields up |
//! | 4   | Supercruise |
//! | 5   | FlightAssist off |
//! | 6   | Hardpoints deployed |
//! | 7   | In wing |
//! | 8   | Lights on |
//! | 9   | Cargo scoop deployed |
//! | 16  | SRV (surface vehicle) |
//! | 28  | FSD jump |

pub mod journal;
pub mod protocol;

use flight_adapter_common::{AdapterConfig, AdapterError, AdapterMetrics, AdapterState};
use flight_bus::{
    BusPublisher, BusSnapshot, LightsConfig, PublisherError,
    types::{AircraftId, GearPosition, GearState, Percentage, SimId},
};
use flight_metrics::{
    MetricsRegistry,
    common::{ADAPTER_ERRORS_TOTAL, ADAPTER_UPDATE_LATENCY_MS, ADAPTER_UPDATES_TOTAL},
};
use journal::JournalReader;
use protocol::{EliteFlags, JournalEvent, StatusJson};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Elite Dangerous adapter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EliteConfig {
    /// Directory that contains `Status.json` and journal files.
    ///
    /// Defaults to `%USERPROFILE%\Saved Games\Frontier Developments\Elite Dangerous`
    /// on Windows.
    pub journal_dir: PathBuf,

    /// How often to re-read `Status.json` (should be ≥ 250 ms to avoid thrashing).
    pub poll_interval: Duration,

    /// Bus publish max rate.
    pub bus_max_rate_hz: f32,
}

impl EliteConfig {
    /// Return the default Elite Dangerous journal directory for the current OS.
    pub fn default_journal_dir() -> PathBuf {
        // Try the Windows / Wine Saved Games path
        if let Some(home) = dirs::home_dir() {
            let candidate = home
                .join("Saved Games")
                .join("Frontier Developments")
                .join("Elite Dangerous");
            if candidate.exists() {
                return candidate;
            }
        }
        // Fallback: current directory (useful in tests)
        PathBuf::from(".")
    }
}

impl Default for EliteConfig {
    fn default() -> Self {
        Self {
            journal_dir: Self::default_journal_dir(),
            poll_interval: Duration::from_millis(250),
            bus_max_rate_hz: 4.0,
        }
    }
}

impl AdapterConfig for EliteConfig {
    fn publish_rate_hz(&self) -> f32 {
        self.bus_max_rate_hz
    }

    fn connection_timeout(&self) -> Duration {
        Duration::from_secs(5)
    }

    fn max_reconnect_attempts(&self) -> u32 {
        0
    }

    fn enable_auto_reconnect(&self) -> bool {
        true
    }
}

/// Errors produced by the Elite adapter.
#[derive(Debug, Error)]
pub enum EliteError {
    #[error("Status.json not found in {path}")]
    StatusNotFound { path: PathBuf },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("publisher error: {0}")]
    Publisher(#[from] PublisherError),
    #[error(transparent)]
    Adapter(#[from] AdapterError),
}

/// Elite: Dangerous file-watcher adapter.
///
/// Call [`poll_once`](Self::poll_once) in a loop (at `poll_interval`) to
/// receive the latest game state from `Status.json`.
pub struct EliteAdapter {
    config: EliteConfig,
    started: bool,
    bus_publisher: BusPublisher,
    state: AdapterState,
    metrics: AdapterMetrics,
    metrics_registry: MetricsRegistry,
    started_at: Instant,
    last_poll: Option<Instant>,
    /// Aircraft / commander name from the last journal LoadGame event.
    current_ship: String,
    /// Current star system name (from Location or FsdJump journal events).
    current_system: String,
    /// Station the commander is currently docked at, if any.
    docked_station: Option<String>,
    /// Cached last-known status flags for change detection.
    last_flags: Option<u64>,
    /// Journal file reader.
    pub journal_reader: JournalReader,
}

impl EliteAdapter {
    /// Create a new adapter (not yet started).
    pub fn new(config: EliteConfig) -> Self {
        let journal_reader = JournalReader::new(config.journal_dir.clone());
        Self {
            bus_publisher: BusPublisher::new(config.bus_max_rate_hz),
            config,
            started: false,
            state: AdapterState::Disconnected,
            metrics: AdapterMetrics::new(),
            metrics_registry: MetricsRegistry::new(),
            started_at: Instant::now(),
            last_poll: None,
            current_ship: "Unknown".to_string(),
            current_system: String::new(),
            docked_station: None,
            last_flags: None,
            journal_reader,
        }
    }

    /// Start the adapter (verify the journal directory exists).
    pub fn start(&mut self) -> Result<(), EliteError> {
        let status_path = self.status_path();
        if !self.config.journal_dir.exists() {
            warn!(
                "Elite journal dir does not exist yet: {}",
                self.config.journal_dir.display()
            );
        }
        self.started = true;
        self.state = AdapterState::Connected;
        info!("Elite adapter ready — watching {}", status_path.display());
        Ok(())
    }

    /// Stop the adapter.
    pub fn stop(&mut self) {
        self.started = false;
        self.state = AdapterState::Disconnected;
    }

    /// Read `Status.json` once and return a [`BusSnapshot`] if the file
    /// exists and has changed since the last poll.
    ///
    /// Returns `Ok(None)` when `Status.json` is absent (game not running)
    /// or unchanged since the last call.
    pub async fn poll_once(&mut self) -> Result<Option<BusSnapshot>, EliteError> {
        if !self.started {
            return Err(EliteError::Adapter(AdapterError::NotConnected));
        }

        let path = self.status_path();
        let update_start = Instant::now();

        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("Elite: Status.json not found at {}", path.display());
                return Ok(None);
            }
            Err(e) => {
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                return Err(EliteError::Io(e));
            }
        };

        // Parse
        let status: StatusJson = serde_json::from_str(&content).map_err(|e| {
            self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            EliteError::Json(e)
        })?;

        self.state = AdapterState::Active;
        self.last_poll = Some(Instant::now());

        // Skip if flags unchanged (basic change detection)
        if self.last_flags == Some(status.flags) {
            return Ok(None);
        }
        self.last_flags = Some(status.flags);

        let snapshot = self.convert_status(&status);
        self.bus_publisher.publish(snapshot.clone()).map_err(|e| {
            self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            EliteError::Publisher(e)
        })?;

        self.metrics.record_update();
        self.metrics_registry.inc_counter(ADAPTER_UPDATES_TOTAL, 1);
        self.metrics_registry.observe(
            ADAPTER_UPDATE_LATENCY_MS,
            update_start.elapsed().as_secs_f64() * 1000.0,
        );

        debug!("Elite: snapshot published (flags={:#010x})", status.flags);
        Ok(Some(snapshot))
    }

    /// Convert a [`StatusJson`] record into a [`BusSnapshot`].
    pub fn convert_status(&self, status: &StatusJson) -> BusSnapshot {
        let mut snapshot = BusSnapshot::new(
            SimId::EliteDangerous,
            AircraftId::new(self.current_ship.clone()),
        );
        snapshot.timestamp = Instant::now().duration_since(self.started_at).as_nanos() as u64;

        let flags = EliteFlags::from_bits_truncate(status.flags);

        // Landing gear
        let gear_pos = if flags.contains(EliteFlags::GEAR_DOWN) {
            GearPosition::Down
        } else {
            GearPosition::Up
        };
        snapshot.config.gear = GearState {
            nose: gear_pos,
            left: gear_pos,
            right: gear_pos,
        };

        // Lights
        snapshot.config.lights = LightsConfig {
            nav: flags.contains(EliteFlags::LIGHTS_ON),
            beacon: false,
            strobe: false,
            landing: flags.contains(EliteFlags::LIGHTS_ON),
            ..LightsConfig::default()
        };

        // Fuel (percentage of main tank; 0-100)
        if let Some(fuel) = &status.fuel {
            // FuelMain is in tonnes; without max capacity we express it as a
            // "present vs reserve" ratio, or just store the raw value.
            // We store the reservoir fraction (FuelReservoir / FuelMain) if both known.
            let main_pct = if fuel.fuel_main > 0.0 {
                ((fuel.fuel_main / (fuel.fuel_main + fuel.fuel_reservoir)) * 100.0)
                    .clamp(0.0, 100.0)
            } else {
                0.0
            };
            if let Ok(p) = Percentage::new(main_pct) {
                snapshot.config.fuel.insert("main".to_string(), p);
            }
        }

        // Supercruise / in-flight state → validity
        let in_flight = !flags.contains(EliteFlags::DOCKED)
            && !flags.contains(EliteFlags::LANDED)
            && !flags.contains(EliteFlags::IN_SRV);
        snapshot.validity.position_valid = in_flight;
        snapshot.validity.safe_for_ffb = false; // ED provides no attitude data in Status.json

        // Current star system → navigation active waypoint
        if !self.current_system.is_empty() {
            snapshot.navigation.active_waypoint = Some(self.current_system.clone());
        }

        snapshot
    }

    /// Apply a parsed journal event to update adapter state.
    ///
    /// Call this after reading new events from [`JournalReader`] to keep the
    /// adapter's understanding of the game state up-to-date.
    pub fn apply_journal_event(&mut self, event: &JournalEvent) {
        match event {
            JournalEvent::LoadGame { ship, .. } => {
                self.set_ship(ship.clone());
                self.docked_station = None;
            }
            JournalEvent::Location { star_system, .. }
            | JournalEvent::FsdJump { star_system, .. } => {
                if self.current_system != *star_system {
                    info!("Elite: entered system {star_system}");
                    self.current_system = star_system.clone();
                }
                self.docked_station = None;
            }
            JournalEvent::Docked {
                station_name,
                star_system,
            } => {
                self.docked_station = Some(station_name.clone());
                if self.current_system != *star_system {
                    self.current_system = star_system.clone();
                }
            }
            JournalEvent::Undocked { .. } => {
                self.docked_station = None;
            }
            JournalEvent::RefuelAll { .. }
            | JournalEvent::Touchdown { .. }
            | JournalEvent::Liftoff { .. } => {}
        }
    }

    /// Update the current ship name (called when journal LoadGame event is seen).
    pub fn set_ship(&mut self, ship: impl Into<String>) {
        self.current_ship = ship.into();
        self.metrics
            .record_aircraft_change(self.current_ship.clone());
    }

    /// Return the current star system name (from the most recent Location/FsdJump event).
    pub fn current_system(&self) -> &str {
        &self.current_system
    }

    /// Return the station the commander is currently docked at, if any.
    pub fn docked_station(&self) -> Option<&str> {
        self.docked_station.as_deref()
    }

    /// Return current adapter state.
    pub fn state(&self) -> AdapterState {
        self.state
    }

    /// Return adapter metrics.
    pub fn metrics(&self) -> AdapterMetrics {
        self.metrics.clone()
    }

    fn status_path(&self) -> PathBuf {
        self.config.journal_dir.join("Status.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{FuelStatus, StatusJson};
    use tempfile::TempDir;

    fn config_with_dir(dir: &TempDir) -> EliteConfig {
        EliteConfig {
            journal_dir: dir.path().to_path_buf(),
            ..Default::default()
        }
    }

    fn write_status(dir: &TempDir, status: &StatusJson) {
        let path = dir.path().join("Status.json");
        std::fs::write(path, serde_json::to_string(status).unwrap()).unwrap();
    }

    #[test]
    fn converts_gear_down_flag() {
        let adapter = EliteAdapter::new(EliteConfig::default());
        let status = StatusJson {
            flags: EliteFlags::GEAR_DOWN.bits(),
            ..Default::default()
        };
        let snap = adapter.convert_status(&status);
        assert!(snap.config.gear.all_down());
    }

    #[test]
    fn converts_gear_up_flag() {
        let adapter = EliteAdapter::new(EliteConfig::default());
        let status = StatusJson {
            flags: 0,
            ..Default::default()
        };
        let snap = adapter.convert_status(&status);
        assert!(snap.config.gear.all_up());
    }

    #[test]
    fn converts_lights_on() {
        let adapter = EliteAdapter::new(EliteConfig::default());
        let status = StatusJson {
            flags: EliteFlags::LIGHTS_ON.bits(),
            ..Default::default()
        };
        let snap = adapter.convert_status(&status);
        assert!(snap.config.lights.nav);
        assert!(snap.config.lights.landing);
    }

    #[test]
    fn converts_fuel() {
        let adapter = EliteAdapter::new(EliteConfig::default());
        let status = StatusJson {
            fuel: Some(FuelStatus {
                fuel_main: 16.0,
                fuel_reservoir: 4.0,
            }),
            ..Default::default()
        };
        let snap = adapter.convert_status(&status);
        assert!(snap.config.fuel.contains_key("main"));
        let pct = snap.config.fuel["main"].value();
        // 16 / (16 + 4) = 80 %
        assert!((pct - 80.0).abs() < 0.01, "fuel pct was {pct}");
    }

    #[test]
    fn docked_ship_not_valid_for_ffb() {
        let adapter = EliteAdapter::new(EliteConfig::default());
        let status = StatusJson {
            flags: EliteFlags::DOCKED.bits(),
            ..Default::default()
        };
        let snap = adapter.convert_status(&status);
        assert!(!snap.validity.safe_for_ffb);
    }

    #[test]
    fn sim_id_is_elite_dangerous() {
        let adapter = EliteAdapter::new(EliteConfig::default());
        let status = StatusJson::default();
        let snap = adapter.convert_status(&status);
        assert_eq!(snap.sim, SimId::EliteDangerous);
    }

    #[tokio::test]
    async fn poll_reads_status_file() {
        let dir = TempDir::new().unwrap();
        let status = StatusJson {
            flags: EliteFlags::GEAR_DOWN.bits(),
            ..Default::default()
        };
        write_status(&dir, &status);

        let mut adapter = EliteAdapter::new(config_with_dir(&dir));
        adapter.start().unwrap();
        let snap = adapter.poll_once().await.unwrap().unwrap();
        assert!(snap.config.gear.all_down());
    }

    #[tokio::test]
    async fn poll_returns_none_when_no_change() {
        let dir = TempDir::new().unwrap();
        let status = StatusJson {
            flags: 0,
            ..Default::default()
        };
        write_status(&dir, &status);

        let mut adapter = EliteAdapter::new(config_with_dir(&dir));
        adapter.start().unwrap();

        // First poll → snapshot
        let first = adapter.poll_once().await.unwrap();
        assert!(first.is_some());
        // Second poll (same content) → None
        let second = adapter.poll_once().await.unwrap();
        assert!(second.is_none());
    }

    #[tokio::test]
    async fn poll_returns_none_when_file_absent() {
        let dir = TempDir::new().unwrap();
        let mut adapter = EliteAdapter::new(config_with_dir(&dir));
        adapter.start().unwrap();
        let result = adapter.poll_once().await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn apply_fsd_jump_updates_current_system() {
        let mut adapter = EliteAdapter::new(EliteConfig::default());
        let event = protocol::JournalEvent::FsdJump {
            star_system: "Colonia".to_string(),
            star_pos: Some([0.0, 0.0, 0.0]),
        };
        adapter.apply_journal_event(&event);
        assert_eq!(adapter.current_system(), "Colonia");
        assert!(adapter.docked_station().is_none());
    }

    #[test]
    fn apply_location_updates_current_system() {
        let mut adapter = EliteAdapter::new(EliteConfig::default());
        let event = protocol::JournalEvent::Location {
            star_system: "Sol".to_string(),
            star_pos: None,
        };
        adapter.apply_journal_event(&event);
        assert_eq!(adapter.current_system(), "Sol");
    }

    #[test]
    fn apply_docked_sets_station_and_system() {
        let mut adapter = EliteAdapter::new(EliteConfig::default());
        let event = protocol::JournalEvent::Docked {
            station_name: "Jameson Memorial".to_string(),
            star_system: "Shinrarta Dezhra".to_string(),
        };
        adapter.apply_journal_event(&event);
        assert_eq!(adapter.current_system(), "Shinrarta Dezhra");
        assert_eq!(adapter.docked_station(), Some("Jameson Memorial"));
    }

    #[test]
    fn apply_undocked_clears_station() {
        let mut adapter = EliteAdapter::new(EliteConfig::default());
        // First dock…
        adapter.apply_journal_event(&protocol::JournalEvent::Docked {
            station_name: "Jameson Memorial".to_string(),
            star_system: "Shinrarta Dezhra".to_string(),
        });
        assert!(adapter.docked_station().is_some());
        // …then undock.
        adapter.apply_journal_event(&protocol::JournalEvent::Undocked {
            station_name: "Jameson Memorial".to_string(),
        });
        assert!(adapter.docked_station().is_none());
    }

    #[test]
    fn current_system_appears_in_snapshot_waypoint() {
        let mut adapter = EliteAdapter::new(EliteConfig::default());
        adapter.apply_journal_event(&protocol::JournalEvent::FsdJump {
            star_system: "Sagittarius A*".to_string(),
            star_pos: None,
        });
        let snap = adapter.convert_status(&StatusJson::default());
        assert_eq!(
            snap.navigation.active_waypoint.as_deref(),
            Some("Sagittarius A*")
        );
    }
}
