// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Project Wingman adapter for OpenFlight.
//!
//! Project Wingman (Simmer Entertainment, 2021) is an Unreal Engine 4 combat
//! flight game. It exposes **no** in-process telemetry API. This adapter:
//!
//! 1. Integrates with the Flight Hub process-detection system so that profiles
//!    are activated automatically when `ProjectWingman.exe` is running.
//! 2. Publishes a presence [`BusSnapshot`] (all validity flags `false`) to
//!    signal that the game is active.
//! 3. Provides a [`VirtualController`] abstraction for routing processed HOTAS
//!    outputs into the game via a virtual XInput device.
//!
//! ## Virtual controller requirement
//!
//! The stub implementation ([`StubVirtualController`]) logs axis/button values
//! but does not create a real virtual device. To produce actual XInput output
//! on Windows, install **ViGEm Bus** (<https://github.com/nefarius/ViGEmBus>)
//! and replace the stub with a ViGEm-backed controller.
//!
//! ## Input binding
//!
//! Project Wingman reads inputs through SDL2. Bind axes and buttons in-game
//! after attaching the virtual controller. See `docs/how-to/wingman-setup.md`.

pub mod virtual_controller;

#[cfg(windows)]
pub mod vigem_controller;
#[cfg(windows)]
pub use vigem_controller::ViGEmXInputController;

use flight_adapter_common::{AdapterError, AdapterMetrics, AdapterState};
use flight_bus::{
    BusPublisher, BusSnapshot, PublisherError,
    types::{AircraftId, SimId},
};
use flight_metrics::{
    MetricsRegistry,
    common::{ADAPTER_ERRORS_TOTAL, ADAPTER_UPDATES_TOTAL},
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, info};
use virtual_controller::{StubVirtualController, VirtualController, VirtualControllerError};

/// Project Wingman adapter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WingmanConfig {
    /// Expected process name for detection validation.
    pub process_name: String,
    /// Presence-snapshot publish rate in Hz.
    pub poll_rate_hz: f32,
    /// Bus publish max rate in Hz.
    pub bus_max_rate_hz: f32,
}

impl Default for WingmanConfig {
    fn default() -> Self {
        Self {
            process_name: "ProjectWingman.exe".to_string(),
            poll_rate_hz: 10.0,
            bus_max_rate_hz: 10.0,
        }
    }
}

/// Errors produced by the Project Wingman adapter.
#[derive(Debug, Error)]
pub enum WingmanError {
    #[error("adapter is not started")]
    NotStarted,
    #[error("publisher error: {0}")]
    Publisher(#[from] PublisherError),
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error("virtual controller error: {0}")]
    VirtualController(#[from] VirtualControllerError),
}

/// Project Wingman process-detection adapter.
///
/// Call [`start`](Self::start) when the game process is detected, then poll
/// [`poll_once`](Self::poll_once) at `poll_rate_hz` to publish presence
/// snapshots. Use [`send_axis`](Self::send_axis) /
/// [`send_button`](Self::send_button) to forward processed HOTAS inputs to
/// the virtual controller.
pub struct WingmanAdapter {
    config: WingmanConfig,
    controller: Option<Box<dyn VirtualController>>,
    bus_publisher: BusPublisher,
    state: AdapterState,
    metrics: AdapterMetrics,
    metrics_registry: MetricsRegistry,
    started_at: Instant,
}

impl WingmanAdapter {
    /// Create a new adapter (not yet started).
    pub fn new(config: WingmanConfig) -> Self {
        Self {
            bus_publisher: BusPublisher::new(config.bus_max_rate_hz),
            config,
            controller: None,
            state: AdapterState::Disconnected,
            metrics: AdapterMetrics::new(),
            metrics_registry: MetricsRegistry::new(),
            started_at: Instant::now(),
        }
    }

    /// Start the adapter and initialise the virtual controller.
    pub fn start(&mut self) {
        self.controller = Some(Box::new(StubVirtualController::new()));
        self.state = AdapterState::Connected;
        info!(
            "Wingman adapter ready — monitoring {}",
            self.config.process_name
        );
    }

    /// Stop the adapter and release the virtual controller.
    pub fn stop(&mut self) {
        self.controller = None;
        self.state = AdapterState::Disconnected;
    }

    /// Publish a presence snapshot and return it.
    ///
    /// Returns `Err(WingmanError::NotStarted)` if [`start`](Self::start) has
    /// not been called. All validity flags in the snapshot are `false` because
    /// Project Wingman exposes no telemetry API.
    pub fn poll_once(&mut self) -> Result<Option<BusSnapshot>, WingmanError> {
        if self.controller.is_none() {
            return Err(WingmanError::NotStarted);
        }

        let mut snapshot = BusSnapshot::new(SimId::Wingman, AircraftId::new("WINGMAN"));
        snapshot.timestamp = Instant::now().duration_since(self.started_at).as_nanos() as u64;

        self.bus_publisher
            .publish(snapshot.clone())
            .inspect_err(|_| {
                self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            })?;

        self.metrics.record_update();
        self.metrics_registry.inc_counter(ADAPTER_UPDATES_TOTAL, 1);
        debug!("Wingman: presence snapshot published");
        Ok(Some(snapshot))
    }

    /// Forward a processed axis value to the virtual controller.
    ///
    /// `index` selects the axis (0–7); `value` is clamped to `[-1.0, 1.0]`.
    pub fn send_axis(&mut self, index: u8, value: f32) -> Result<(), WingmanError> {
        let controller = self.controller.as_mut().ok_or(WingmanError::NotStarted)?;
        controller.send_axis(index, value)?;
        Ok(())
    }

    /// Forward a processed button state to the virtual controller.
    ///
    /// `index` selects the button (0–31).
    pub fn send_button(&mut self, index: u8, pressed: bool) -> Result<(), WingmanError> {
        let controller = self.controller.as_mut().ok_or(WingmanError::NotStarted)?;
        controller.send_button(index, pressed)?;
        Ok(())
    }

    /// Return the current adapter state.
    pub fn state(&self) -> AdapterState {
        self.state
    }

    /// Return accumulated adapter metrics.
    pub fn metrics(&self) -> AdapterMetrics {
        self.metrics.clone()
    }

    /// Return the shared metrics registry.
    pub fn metrics_registry(&self) -> &MetricsRegistry {
        &self.metrics_registry
    }

    /// Return the poll interval implied by `poll_rate_hz`.
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs_f32(1.0 / self.config.poll_rate_hz.max(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_initial_state_is_disconnected() {
        let adapter = WingmanAdapter::new(WingmanConfig::default());
        assert_eq!(adapter.state(), AdapterState::Disconnected);
    }

    #[test]
    fn start_transitions_to_connected_stop_disconnects() {
        let mut adapter = WingmanAdapter::new(WingmanConfig::default());
        adapter.start();
        assert_eq!(adapter.state(), AdapterState::Connected);
        adapter.stop();
        assert_eq!(adapter.state(), AdapterState::Disconnected);
    }

    #[test]
    fn poll_returns_snapshot_with_correct_sim_id() {
        let mut adapter = WingmanAdapter::new(WingmanConfig::default());
        adapter.start();
        let snapshot = adapter.poll_once().unwrap().unwrap();
        assert_eq!(snapshot.sim, SimId::Wingman);
    }

    #[test]
    fn poll_snapshot_has_no_valid_telemetry() {
        let mut adapter = WingmanAdapter::new(WingmanConfig::default());
        adapter.start();
        let snapshot = adapter.poll_once().unwrap().unwrap();
        // Project Wingman exposes no telemetry API; all validity flags must be false.
        assert!(!snapshot.validity.attitude_valid);
        assert!(!snapshot.validity.velocities_valid);
        assert!(!snapshot.validity.position_valid);
        assert!(!snapshot.validity.safe_for_ffb);
    }

    #[test]
    fn poll_without_start_returns_not_started() {
        let mut adapter = WingmanAdapter::new(WingmanConfig::default());
        assert!(matches!(adapter.poll_once(), Err(WingmanError::NotStarted)));
    }

    #[test]
    fn send_axis_without_start_returns_not_started() {
        let mut adapter = WingmanAdapter::new(WingmanConfig::default());
        assert!(matches!(
            adapter.send_axis(0, 1.0),
            Err(WingmanError::NotStarted)
        ));
    }

    #[test]
    fn send_button_without_start_returns_not_started() {
        let mut adapter = WingmanAdapter::new(WingmanConfig::default());
        assert!(matches!(
            adapter.send_button(0, true),
            Err(WingmanError::NotStarted)
        ));
    }

    #[test]
    fn poll_interval_matches_config_rate() {
        let config = WingmanConfig {
            poll_rate_hz: 20.0,
            ..Default::default()
        };
        let adapter = WingmanAdapter::new(config);
        let interval = adapter.poll_interval();
        let expected_ms = 50u64; // 1000ms / 20hz
        let actual_ms = interval.as_millis() as u64;
        assert!(
            (actual_ms as i64 - expected_ms as i64).abs() <= 2,
            "expected ~50ms, got {}ms",
            actual_ms
        );
    }

    #[test]
    fn default_config_has_sensible_values() {
        let config = WingmanConfig::default();
        assert_eq!(config.process_name, "ProjectWingman.exe");
        assert!(config.poll_rate_hz > 0.0);
        assert!(config.bus_max_rate_hz > 0.0);
    }
}
