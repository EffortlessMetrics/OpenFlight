// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Lockheed Martin Prepar3D adapter for OpenFlight.
//!
//! P3D ships its own SimConnect.dll which shares the API surface with
//! Microsoft Flight Simulator's SimConnect. This adapter provides a thin
//! integration layer reusing [`flight_adapter_common`] patterns; a real
//! SimConnect binding will be wired in a future release.

// Reserved for future real implementation; silences the unused-extern-crate lint.
#[allow(unused_extern_crates)]
extern crate flight_adapter_common;
#[allow(unused_extern_crates)]
extern crate flight_core;

use thiserror::Error;

/// Connection state of the Prepar3D adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P3DState {
    /// No active connection to P3D.
    Disconnected,
    /// Attempting to open the SimConnect channel.
    Connecting,
    /// SimConnect channel is open; receiving data.
    Connected,
    /// A fatal error occurred; the adapter must be recreated.
    Error,
}

/// A snapshot of P3D flight data delivered through the SimConnect data callback.
#[derive(Debug, Clone)]
pub struct P3DFlightData {
    /// Aircraft pitch in radians (positive = nose up).
    pub pitch_rad: f32,
    /// Aircraft roll in radians (positive = right wing down).
    pub roll_rad: f32,
    /// Aircraft yaw (heading) in radians.
    pub yaw_rad: f32,
    /// Throttle lever position, normalised to `0.0` – `1.0`.
    pub throttle: f32,
    /// Pressure altitude in feet.
    pub altitude_ft: f32,
    /// Indicated airspeed in knots.
    pub airspeed_kts: f32,
}

/// Errors emitted by the Prepar3D adapter.
#[derive(Debug, Error)]
pub enum P3DError {
    /// P3D is not running or SimConnect.dll is not on the PATH.
    #[error("P3D not running or SimConnect not available")]
    NotAvailable,
    /// The SimConnect DLL version does not match the expected API version.
    #[error("Version mismatch: expected v{expected}, found v{found}")]
    VersionMismatch { expected: String, found: String },
}

/// Thin adapter for Lockheed Martin Prepar3D.
///
/// Uses the P3D-bundled SimConnect API (same interface as MSFS SimConnect).
/// Connect via [`simulate_connect`](Self::simulate_connect), then drive the
/// adapter with [`process_data`](Self::process_data) as SimConnect callbacks
/// deliver data.
pub struct Prepar3DAdapter {
    state: P3DState,
    connected: bool,
    version: Option<String>,
    last_data: Option<P3DFlightData>,
    packet_count: u64,
    error_count: u64,
}

impl Prepar3DAdapter {
    /// Create a new adapter in the [`P3DState::Disconnected`] state.
    pub fn new() -> Self {
        tracing::debug!("Prepar3DAdapter created");
        Self {
            state: P3DState::Disconnected,
            connected: false,
            version: None,
            last_data: None,
            packet_count: 0,
            error_count: 0,
        }
    }

    /// Return the current connection state.
    pub fn state(&self) -> P3DState {
        self.state
    }

    /// Simulate opening a SimConnect channel to the given P3D `version` string.
    ///
    /// Returns `true` on success (state → [`P3DState::Connected`]).
    /// In a real implementation this would call `SimConnect_Open`.
    pub fn simulate_connect(&mut self, version: &str) -> bool {
        tracing::info!(version, "Connecting to Prepar3D");
        self.state = P3DState::Connecting;
        self.version = Some(version.to_owned());
        self.connected = true;
        self.state = P3DState::Connected;
        true
    }

    /// Simulate closing the SimConnect channel.
    ///
    /// Resets state to [`P3DState::Disconnected`].
    pub fn simulate_disconnect(&mut self) {
        tracing::info!("Disconnecting from Prepar3D");
        self.connected = false;
        self.state = P3DState::Disconnected;
        self.version = None;
    }

    /// Process a flight-data snapshot received from a SimConnect callback.
    ///
    /// Increments [`packet_count`](Self::packet_count) and stores the snapshot.
    /// Returns `true` when the data was accepted (i.e., the adapter is connected).
    pub fn process_data(&mut self, data: P3DFlightData) -> bool {
        if self.state != P3DState::Connected {
            tracing::warn!("process_data called while not connected");
            self.error_count += 1;
            return false;
        }
        tracing::trace!(
            pitch = data.pitch_rad,
            roll = data.roll_rad,
            alt = data.altitude_ft,
            "P3D flight data received"
        );
        self.packet_count += 1;
        self.last_data = Some(data);
        true
    }

    /// Return the most recently received flight-data snapshot, if any.
    pub fn last_data(&self) -> Option<&P3DFlightData> {
        self.last_data.as_ref()
    }

    /// Total number of successfully processed data packets.
    pub fn packet_count(&self) -> u64 {
        self.packet_count
    }

    /// Number of processing errors (e.g., packets rejected when disconnected).
    pub fn error_count(&self) -> u64 {
        self.error_count
    }
}

impl Default for Prepar3DAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> P3DFlightData {
        P3DFlightData {
            pitch_rad: 0.05,
            roll_rad: -0.02,
            yaw_rad: 1.57,
            throttle: 0.75,
            altitude_ft: 5_000.0,
            airspeed_kts: 120.0,
        }
    }

    #[test]
    fn new_adapter_starts_disconnected() {
        let adapter = Prepar3DAdapter::new();
        assert_eq!(adapter.state(), P3DState::Disconnected);
    }

    #[test]
    fn simulate_connect_transitions_to_connected() {
        let mut adapter = Prepar3DAdapter::new();
        let ok = adapter.simulate_connect("5.3");
        assert!(ok);
        assert_eq!(adapter.state(), P3DState::Connected);
    }

    #[test]
    fn process_data_increments_packet_count() {
        let mut adapter = Prepar3DAdapter::new();
        adapter.simulate_connect("5.3");
        adapter.process_data(sample_data());
        adapter.process_data(sample_data());
        assert_eq!(adapter.packet_count(), 2);
    }

    #[test]
    fn simulate_disconnect_resets_state() {
        let mut adapter = Prepar3DAdapter::new();
        adapter.simulate_connect("5.3");
        adapter.simulate_disconnect();
        assert_eq!(adapter.state(), P3DState::Disconnected);
    }

    #[test]
    fn last_data_returns_none_before_any_data() {
        let adapter = Prepar3DAdapter::new();
        assert!(adapter.last_data().is_none());
    }

    #[test]
    fn last_data_returns_some_after_process_data() {
        let mut adapter = Prepar3DAdapter::new();
        adapter.simulate_connect("5.3");
        adapter.process_data(sample_data());
        assert!(adapter.last_data().is_some());
    }

    #[test]
    fn error_count_starts_at_zero() {
        let adapter = Prepar3DAdapter::new();
        assert_eq!(adapter.error_count(), 0);
    }

    #[test]
    fn flight_data_contains_expected_fields() {
        let data = P3DFlightData {
            pitch_rad: 0.1,
            roll_rad: 0.2,
            yaw_rad: 0.3,
            throttle: 0.5,
            altitude_ft: 10_000.0,
            airspeed_kts: 250.0,
        };
        assert!((data.pitch_rad - 0.1).abs() < f32::EPSILON);
        assert!((data.roll_rad - 0.2).abs() < f32::EPSILON);
        assert!((data.yaw_rad - 0.3).abs() < f32::EPSILON);
        assert!((data.throttle - 0.5).abs() < f32::EPSILON);
        assert!((data.altitude_ft - 10_000.0).abs() < 0.01);
        assert!((data.airspeed_kts - 250.0).abs() < f32::EPSILON);
    }
}
