// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! War Thunder HTTP telemetry adapter for OpenFlight.
//!
//! War Thunder exposes a local HTTP API at `http://localhost:8111` when the
//! game is running. This adapter polls the `/indicators` endpoint at a
//! configurable rate and publishes normalized [`BusSnapshot`] values to the
//! Flight Hub bus.
//!
//! ## Enabling the API in War Thunder
//!
//! The HTTP API is enabled by default when the game is running; no extra
//! configuration is required. Ensure the game is not run in a restricted
//! sandbox that blocks loopback connections.
//!
//! ## JSON field names
//!
//! The `/indicators` endpoint uses the following key fields (community-documented):
//!
//! | JSON field | Unit | Description |
//! |---|---|---|
//! | `"IAS km/h"` | km/h | Indicated airspeed |
//! | `"TAS km/h"` | km/h | True airspeed |
//! | `"altitude"` | m | Geometric altitude |
//! | `"heading"` | deg | Magnetic heading 0–360 |
//! | `"pitch"` | deg | Pitch angle (+up) |
//! | `"roll"` | deg | Bank/roll angle |
//! | `"gLoad"` | g | Normal (vertical) G-force |
//! | `"vertSpeed"` | m/s | Vertical speed |
//! | `"airframe"` | string | Aircraft display name |
//! | `"flaps"` | 0–1 | Flap deployment ratio |
//! | `"gear"` | 0–1 | Landing gear deployment |
//! | `"valid"` | bool | Whether telemetry is live |

pub mod protocol;

use flight_adapter_common::{AdapterConfig, AdapterError, AdapterMetrics, AdapterState};
use flight_bus::{
    BusPublisher, BusSnapshot, PublisherError,
    types::{
        AircraftId, GForce, GearPosition, GearState, Percentage, SimId, ValidatedAngle,
        ValidatedSpeed,
    },
};
use flight_core::units::{angles, conversions};
use flight_metrics::{
    MetricsRegistry,
    common::{
        ADAPTER_ERRORS_TOTAL, ADAPTER_TIME_SINCE_LAST_PACKET_MS, ADAPTER_UPDATE_LATENCY_MS,
        ADAPTER_UPDATES_TOTAL,
    },
};
use protocol::WtIndicators;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, info, warn};

/// War Thunder telemetry adapter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarThunderConfig {
    /// Base URL of the War Thunder HTTP API (e.g. `http://localhost:8111`).
    pub base_url: String,
    /// Polling rate in Hz.
    pub poll_rate_hz: f32,
    /// HTTP request timeout.
    pub request_timeout: Duration,
    /// Bus publish max rate.
    pub bus_max_rate_hz: f32,
}

impl Default for WarThunderConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8111".to_string(),
            poll_rate_hz: 30.0,
            request_timeout: Duration::from_millis(500),
            bus_max_rate_hz: 30.0,
        }
    }
}

impl AdapterConfig for WarThunderConfig {
    fn publish_rate_hz(&self) -> f32 {
        self.poll_rate_hz
    }

    fn connection_timeout(&self) -> Duration {
        self.request_timeout
    }

    fn max_reconnect_attempts(&self) -> u32 {
        0
    }

    fn enable_auto_reconnect(&self) -> bool {
        true
    }
}

/// Errors produced by the War Thunder adapter.
#[derive(Debug, Error)]
pub enum WarThunderError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("adapter is not started")]
    NotStarted,
    #[error("publisher error: {0}")]
    Publisher(#[from] PublisherError),
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error("invalid telemetry field: {field}")]
    InvalidField { field: &'static str },
}

/// War Thunder telemetry adapter.
///
/// Poll [`poll_once`](Self::poll_once) periodically (at `poll_rate_hz`) to
/// receive the latest telemetry from the game and publish a [`BusSnapshot`].
pub struct WarThunderAdapter {
    config: WarThunderConfig,
    client: Option<reqwest::Client>,
    bus_publisher: BusPublisher,
    state: AdapterState,
    metrics: AdapterMetrics,
    metrics_registry: MetricsRegistry,
    started_at: Instant,
    last_packet: Option<Instant>,
}

impl WarThunderAdapter {
    /// Create a new adapter (not yet started).
    pub fn new(config: WarThunderConfig) -> Self {
        Self {
            bus_publisher: BusPublisher::new(config.bus_max_rate_hz),
            config,
            client: None,
            state: AdapterState::Disconnected,
            metrics: AdapterMetrics::new(),
            metrics_registry: MetricsRegistry::new(),
            started_at: Instant::now(),
            last_packet: None,
        }
    }

    /// Start the adapter (initialises the HTTP client).
    pub fn start(&mut self) -> Result<(), WarThunderError> {
        let client = reqwest::Client::builder()
            .timeout(self.config.request_timeout)
            .build()
            .map_err(WarThunderError::Http)?;
        self.client = Some(client);
        self.state = AdapterState::Connected;
        info!(
            "War Thunder adapter ready — polling {}",
            self.config.base_url
        );
        Ok(())
    }

    /// Stop the adapter.
    pub fn stop(&mut self) {
        self.client = None;
        self.state = AdapterState::Disconnected;
    }

    /// Poll the `/indicators` endpoint once and return a [`BusSnapshot`].
    ///
    /// Returns `Ok(None)` when War Thunder reports `"valid": false`
    /// (i.e. the aircraft is not in-flight).
    pub async fn poll_once(&mut self) -> Result<Option<BusSnapshot>, WarThunderError> {
        let client = self.client.as_ref().ok_or(WarThunderError::NotStarted)?;

        let url = format!("{}/indicators", self.config.base_url);
        let update_start = Instant::now();

        let indicators: WtIndicators = client
            .get(&url)
            .send()
            .await
            .inspect_err(|_| {
                self.state = AdapterState::Disconnected;
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
                warn!("War Thunder: HTTP request to {} failed", url);
            })?
            .json()
            .await
            .inspect_err(|_| {
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            })?;

        self.state = AdapterState::Active;

        // Track time since last packet
        if let Some(last) = self.last_packet {
            self.metrics_registry.set_gauge(
                ADAPTER_TIME_SINCE_LAST_PACKET_MS,
                Instant::now().duration_since(last).as_secs_f64() * 1000.0,
            );
        }
        self.last_packet = Some(Instant::now());

        if !indicators.valid.unwrap_or(true) {
            debug!("War Thunder: /indicators reports not valid — skipping snapshot");
            return Ok(None);
        }

        let snapshot = self.convert_indicators(&indicators)?;
        self.bus_publisher
            .publish(snapshot.clone())
            .inspect_err(|_| {
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            })?;

        self.metrics.record_update();
        if let Some(ref name) = indicators.airframe {
            self.metrics.record_aircraft_change(name.clone());
        }
        self.metrics_registry.inc_counter(ADAPTER_UPDATES_TOTAL, 1);
        self.metrics_registry.observe(
            ADAPTER_UPDATE_LATENCY_MS,
            update_start.elapsed().as_secs_f64() * 1000.0,
        );

        debug!("War Thunder: snapshot published");
        Ok(Some(snapshot))
    }

    /// Convert a [`WtIndicators`] struct into a [`BusSnapshot`].
    pub fn convert_indicators(&self, ind: &WtIndicators) -> Result<BusSnapshot, WarThunderError> {
        let aircraft_name = ind.airframe.clone().unwrap_or_default();
        let mut snapshot = BusSnapshot::new(SimId::WarThunder, AircraftId::new(aircraft_name));
        snapshot.timestamp = Instant::now().duration_since(self.started_at).as_nanos() as u64;

        // IAS — km/h → m/s
        if let Some(ias_kmh) = ind.ias_kmh {
            snapshot.kinematics.ias = ValidatedSpeed::new_mps(conversions::kph_to_mps(ias_kmh))
                .map_err(|_| WarThunderError::InvalidField { field: "IAS km/h" })?;
        }

        // TAS — km/h → m/s
        if let Some(tas_kmh) = ind.tas_kmh {
            snapshot.kinematics.tas = ValidatedSpeed::new_mps(conversions::kph_to_mps(tas_kmh))
                .map_err(|_| WarThunderError::InvalidField { field: "TAS km/h" })?;
        }

        // Altitude m → feet
        if let Some(alt_m) = ind.altitude {
            snapshot.environment.altitude = conversions::meters_to_feet(alt_m);
        }

        // Heading
        if let Some(hdg) = ind.heading {
            snapshot.kinematics.heading =
                ValidatedAngle::new_degrees(angles::normalize_degrees_signed(hdg))
                    .map_err(|_| WarThunderError::InvalidField { field: "heading" })?;
        }

        // Pitch
        if let Some(pitch) = ind.pitch {
            snapshot.kinematics.pitch = ValidatedAngle::new_degrees(pitch)
                .map_err(|_| WarThunderError::InvalidField { field: "pitch" })?;
        }

        // Roll / bank
        if let Some(roll) = ind.roll {
            snapshot.kinematics.bank = ValidatedAngle::new_degrees(roll)
                .map_err(|_| WarThunderError::InvalidField { field: "roll" })?;
        }

        // G-force
        if let Some(g) = ind.g_load {
            snapshot.kinematics.g_force =
                GForce::new(g).map_err(|_| WarThunderError::InvalidField { field: "gLoad" })?;
        }

        // Vertical speed m/s → ft/min
        if let Some(vs) = ind.vert_speed {
            snapshot.kinematics.vertical_speed = conversions::mps_to_fpm(vs);
        }

        // Gear (0.0 = retracted, ≥0.5 = deployed/down)
        if let Some(gear) = ind.gear {
            let pos = if gear >= 0.5 {
                GearPosition::Down
            } else {
                GearPosition::Up
            };
            snapshot.config.gear = GearState {
                nose: pos,
                left: pos,
                right: pos,
            };
        }
        // Flaps (0..=1 ratio → Percentage 0..=100)
        if let Some(flaps) = ind.flaps {
            let pct = (flaps * 100.0).clamp(0.0, 100.0);
            if let Ok(p) = Percentage::new(pct) {
                snapshot.config.flaps = p;
            }
        }

        // Validity flags
        snapshot.validity.attitude_valid = ind.pitch.is_some() && ind.roll.is_some();
        snapshot.validity.velocities_valid = ind.ias_kmh.is_some() || ind.tas_kmh.is_some();
        snapshot.validity.position_valid = ind.altitude.is_some();
        snapshot.validity.kinematics_valid = ind.g_load.is_some();
        snapshot.validity.aero_valid = snapshot.validity.attitude_valid;
        snapshot.validity.safe_for_ffb = snapshot.validity.attitude_valid
            && snapshot.validity.velocities_valid
            && snapshot.validity.position_valid;

        Ok(snapshot)
    }

    /// Return current adapter state.
    pub fn state(&self) -> AdapterState {
        self.state
    }

    /// Return adapter metrics.
    pub fn metrics(&self) -> AdapterMetrics {
        self.metrics.clone()
    }

    /// Return shared metrics registry.
    pub fn metrics_registry(&self) -> &MetricsRegistry {
        &self.metrics_registry
    }

    /// Return time since the last successful poll.
    pub fn time_since_last_packet(&self) -> Option<Duration> {
        self.last_packet
            .map(|last| Instant::now().duration_since(last))
    }

    /// Return true if last-packet age exceeds `request_timeout`.
    pub fn is_connection_timeout(&self) -> bool {
        self.last_packet
            .map(|last| Instant::now().duration_since(last) > self.config.request_timeout)
            .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::WtIndicators;

    fn full_indicators() -> WtIndicators {
        WtIndicators {
            valid: Some(true),
            airframe: Some("Spitfire Mk.Vc".to_string()),
            ias_kmh: Some(400.0),
            tas_kmh: Some(420.0),
            altitude: Some(2000.0),
            heading: Some(180.0),
            pitch: Some(5.0),
            roll: Some(-10.0),
            g_load: Some(1.5),
            vert_speed: Some(2.0),
            gear: Some(0.0),
            flaps: Some(0.3),
        }
    }

    #[test]
    fn converts_indicators_to_snapshot() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        let snapshot = adapter.convert_indicators(&full_indicators()).unwrap();

        assert_eq!(snapshot.sim, SimId::WarThunder);
        assert_eq!(snapshot.aircraft.icao, "Spitfire Mk.Vc");
        assert!(snapshot.validity.safe_for_ffb);
        // IAS: 400 km/h → ~111.1 m/s
        let ias_mps: f32 = snapshot.kinematics.ias.to_mps();
        assert!((ias_mps - 111.11).abs() < 0.1, "IAS was {ias_mps}");
        // Altitude: 2000 m → ~6561.7 ft
        let alt_ft = snapshot.environment.altitude;
        assert!((alt_ft - 6561.7).abs() < 1.0, "altitude was {alt_ft}");
    }

    #[test]
    fn missing_fields_produce_invalid_flags() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        let ind = WtIndicators {
            valid: Some(true),
            altitude: Some(500.0),
            ..Default::default()
        };
        let snapshot = adapter.convert_indicators(&ind).unwrap();
        assert!(snapshot.validity.position_valid);
        assert!(!snapshot.validity.attitude_valid);
        assert!(!snapshot.validity.safe_for_ffb);
    }

    #[test]
    fn adapter_initial_state_is_disconnected() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        assert_eq!(adapter.state(), AdapterState::Disconnected);
        assert!(adapter.is_connection_timeout());
    }

    #[test]
    fn start_transitions_to_connected() {
        let mut adapter = WarThunderAdapter::new(WarThunderConfig::default());
        adapter.start().unwrap();
        assert_eq!(adapter.state(), AdapterState::Connected);
        adapter.stop();
        assert_eq!(adapter.state(), AdapterState::Disconnected);
    }

    #[test]
    fn gear_below_half_is_up() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        let ind = WtIndicators {
            valid: Some(true),
            gear: Some(0.3),
            ..Default::default()
        };
        let snapshot = adapter.convert_indicators(&ind).unwrap();
        assert_eq!(snapshot.config.gear.nose, GearPosition::Up);
    }

    #[test]
    fn gear_at_half_is_down() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        let ind = WtIndicators {
            valid: Some(true),
            gear: Some(0.5),
            ..Default::default()
        };
        let snapshot = adapter.convert_indicators(&ind).unwrap();
        assert_eq!(snapshot.config.gear.nose, GearPosition::Down);
    }

    #[test]
    fn flaps_clamped_to_percentage() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        let ind_max = WtIndicators {
            valid: Some(true),
            flaps: Some(1.0),
            ..Default::default()
        };
        let snapshot = adapter.convert_indicators(&ind_max).unwrap();
        assert!((snapshot.config.flaps.value() - 100.0).abs() < 0.01);

        let ind_zero = WtIndicators {
            valid: Some(true),
            flaps: Some(0.0),
            ..Default::default()
        };
        let snap2 = adapter.convert_indicators(&ind_zero).unwrap();
        assert!(snap2.config.flaps.value() < 0.01);
    }

    #[test]
    fn no_valid_field_treats_as_valid() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        // When `valid` is absent (None), the adapter should default to treating
        // the snapshot as valid (unwrap_or(true) behaviour).
        let ind = WtIndicators {
            valid: None,
            altitude: Some(1000.0),
            pitch: Some(0.0),
            roll: Some(0.0),
            ias_kmh: Some(200.0),
            ..Default::default()
        };
        let snapshot = adapter.convert_indicators(&ind).unwrap();
        assert!(snapshot.validity.position_valid);
    }

    #[test]
    fn default_config_has_sensible_defaults() {
        let cfg = WarThunderConfig::default();
        assert_eq!(cfg.base_url, "http://localhost:8111");
        assert!(cfg.poll_rate_hz > 0.0);
        assert!(cfg.request_timeout.as_millis() > 0);
    }

    #[test]
    fn time_since_last_packet_is_none_before_poll() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        assert!(adapter.time_since_last_packet().is_none());
    }

    #[test]
    fn all_none_indicators_all_validity_false() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        let snapshot = adapter
            .convert_indicators(&WtIndicators::default())
            .unwrap();
        assert!(!snapshot.validity.attitude_valid);
        assert!(!snapshot.validity.velocities_valid);
        assert!(!snapshot.validity.position_valid);
        assert!(!snapshot.validity.kinematics_valid);
        assert!(!snapshot.validity.safe_for_ffb);
    }

    #[test]
    fn various_airframe_names_reflected_in_snapshot_aircraft_id() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        for name in &["F-86F Sabre", "Me 262 A-1a", "Su-27"] {
            let ind = WtIndicators {
                airframe: Some(name.to_string()),
                ..Default::default()
            };
            let snap = adapter.convert_indicators(&ind).unwrap();
            assert_eq!(
                snap.aircraft.icao, *name,
                "airframe name should appear in snapshot"
            );
        }
    }

    #[test]
    fn vertical_speed_mps_to_fpm_conversion() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        // 1 m/s = 196.85 ft/min
        let ind = WtIndicators {
            vert_speed: Some(1.0),
            ..Default::default()
        };
        let snap = adapter.convert_indicators(&ind).unwrap();
        assert!(
            (snap.kinematics.vertical_speed - 196.85).abs() < 0.5,
            "1 m/s → ~196.85 ft/min, got {}",
            snap.kinematics.vertical_speed
        );
    }

    #[test]
    fn large_altitude_above_fl400_handled_correctly() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        // FL400 = 40,000 ft; 15,000 m ≈ 49,212.6 ft
        let ind = WtIndicators {
            altitude: Some(15_000.0),
            ..Default::default()
        };
        let snap = adapter.convert_indicators(&ind).unwrap();
        assert!(
            (snap.environment.altitude - 49_212.6).abs() < 5.0,
            "15000 m should be ~49212.6 ft, got {}",
            snap.environment.altitude
        );
    }

    #[test]
    fn heading_360_normalises_to_zero() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        let ind = WtIndicators {
            heading: Some(360.0),
            ..Default::default()
        };
        let snap = adapter.convert_indicators(&ind).unwrap();
        // normalize_degrees_signed(360.0) → 0.0
        assert!(
            snap.kinematics.heading.to_degrees().abs() < 0.01,
            "heading 360° should normalise to 0°, got {}",
            snap.kinematics.heading.to_degrees()
        );
    }

    #[test]
    fn negative_vertical_speed_produces_negative_fpm() {
        let adapter = WarThunderAdapter::new(WarThunderConfig::default());
        // -5 m/s means descending
        let ind = WtIndicators {
            vert_speed: Some(-5.0),
            ..Default::default()
        };
        let snap = adapter.convert_indicators(&ind).unwrap();
        assert!(
            snap.kinematics.vertical_speed < 0.0,
            "descending should give negative ft/min, got {}",
            snap.kinematics.vertical_speed
        );
    }
}
