// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fake simulator backend for integration testing without a running sim.
//!
//! [`FakeSimBackend`] returns canned telemetry snapshots and supports
//! disconnect/reconnect simulation.

/// A single snapshot of simulated telemetry.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetrySnapshot {
    pub altitude_ft: f64,
    pub airspeed_kts: f64,
    pub heading_deg: f64,
    pub pitch_deg: f64,
    pub roll_deg: f64,
    pub yaw_deg: f64,
    pub on_ground: bool,
    pub gear_down: bool,
    pub flaps_pct: f64,
    pub throttle_pct: f64,
}

impl TelemetrySnapshot {
    /// Create a snapshot with all fields zeroed and on the ground.
    #[must_use]
    pub fn on_ramp() -> Self {
        Self {
            altitude_ft: 0.0,
            airspeed_kts: 0.0,
            heading_deg: 0.0,
            pitch_deg: 0.0,
            roll_deg: 0.0,
            yaw_deg: 0.0,
            on_ground: true,
            gear_down: true,
            flaps_pct: 0.0,
            throttle_pct: 0.0,
        }
    }

    /// Create a typical cruise snapshot.
    #[must_use]
    pub fn cruising() -> Self {
        Self {
            altitude_ft: 35_000.0,
            airspeed_kts: 250.0,
            heading_deg: 90.0,
            pitch_deg: 2.0,
            roll_deg: 0.0,
            yaw_deg: 0.0,
            on_ground: false,
            gear_down: false,
            flaps_pct: 0.0,
            throttle_pct: 0.85,
        }
    }
}

/// Connection state of the fake simulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimConnectionState {
    Disconnected,
    Connected,
    Reconnecting,
}

/// A fake simulator backend that replays canned telemetry.
#[derive(Debug)]
pub struct FakeSimBackend {
    pub name: String,
    pub aircraft: Option<String>,
    state: SimConnectionState,
    snapshots: Vec<TelemetrySnapshot>,
    position: usize,
    commands: Vec<String>,
    /// Schedule a disconnect after this many polls.
    disconnect_after: Option<usize>,
    poll_count: usize,
}

impl FakeSimBackend {
    /// Create a backend with no pre-loaded telemetry.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            aircraft: None,
            state: SimConnectionState::Disconnected,
            snapshots: Vec::new(),
            position: 0,
            commands: Vec::new(),
            disconnect_after: None,
            poll_count: 0,
        }
    }

    /// Create a backend pre-loaded with telemetry snapshots.
    #[must_use]
    pub fn with_telemetry(name: &str, snapshots: Vec<TelemetrySnapshot>) -> Self {
        Self {
            name: name.to_owned(),
            aircraft: None,
            state: SimConnectionState::Connected,
            snapshots,
            position: 0,
            commands: Vec::new(),
            disconnect_after: None,
            poll_count: 0,
        }
    }

    /// Connect the fake sim.
    pub fn connect(&mut self) {
        self.state = SimConnectionState::Connected;
    }

    /// Disconnect the fake sim.
    pub fn disconnect(&mut self) {
        self.state = SimConnectionState::Disconnected;
    }

    /// Simulate a reconnection cycle.
    pub fn reconnect(&mut self) {
        self.state = SimConnectionState::Reconnecting;
        self.state = SimConnectionState::Connected;
        self.position = 0;
        self.poll_count = 0;
    }

    /// Schedule a disconnect after `n` polls.
    pub fn disconnect_after(&mut self, n: usize) {
        self.disconnect_after = Some(n);
    }

    /// Set the active aircraft.
    pub fn set_aircraft(&mut self, aircraft: &str) {
        self.aircraft = Some(aircraft.to_owned());
    }

    /// Current connection state.
    #[must_use]
    pub fn state(&self) -> SimConnectionState {
        self.state
    }

    /// Poll the next telemetry snapshot.
    ///
    /// Returns `None` if disconnected or all snapshots have been consumed.
    pub fn poll(&mut self) -> Option<TelemetrySnapshot> {
        if self.state == SimConnectionState::Disconnected {
            return None;
        }

        if let Some(limit) = self.disconnect_after
            && self.poll_count >= limit
        {
            self.state = SimConnectionState::Disconnected;
            return None;
        }

        self.poll_count += 1;

        if self.position < self.snapshots.len() {
            let snap = self.snapshots[self.position].clone();
            self.position += 1;
            Some(snap)
        } else {
            None
        }
    }

    /// Send a command to the sim (records it for later inspection).
    pub fn send_command(&mut self, cmd: &str) {
        self.commands.push(cmd.to_owned());
    }

    /// Return all recorded commands.
    #[must_use]
    pub fn commands(&self) -> &[String] {
        &self.commands
    }

    /// Return how many snapshots have been consumed.
    #[must_use]
    pub fn polls_completed(&self) -> usize {
        self.position
    }

    /// Push additional snapshots.
    pub fn push_snapshot(&mut self, snapshot: TelemetrySnapshot) {
        self.snapshots.push(snapshot);
    }

    /// Reset position and commands.
    pub fn reset(&mut self) {
        self.position = 0;
        self.poll_count = 0;
        self.commands.clear();
        self.disconnect_after = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_backend_is_disconnected() {
        let sim = FakeSimBackend::new("MSFS");
        assert_eq!(sim.state(), SimConnectionState::Disconnected);
        assert!(sim.aircraft.is_none());
    }

    #[test]
    fn with_telemetry_starts_connected() {
        let sim = FakeSimBackend::with_telemetry(
            "X-Plane",
            vec![TelemetrySnapshot::on_ramp()],
        );
        assert_eq!(sim.state(), SimConnectionState::Connected);
    }

    #[test]
    fn poll_returns_snapshots_in_order() {
        let mut sim = FakeSimBackend::with_telemetry(
            "MSFS",
            vec![TelemetrySnapshot::on_ramp(), TelemetrySnapshot::cruising()],
        );
        let s1 = sim.poll().unwrap();
        assert!(s1.on_ground);
        let s2 = sim.poll().unwrap();
        assert!(!s2.on_ground);
        assert!(sim.poll().is_none());
    }

    #[test]
    fn poll_returns_none_when_disconnected() {
        let mut sim = FakeSimBackend::with_telemetry(
            "MSFS",
            vec![TelemetrySnapshot::on_ramp()],
        );
        sim.disconnect();
        assert!(sim.poll().is_none());
    }

    #[test]
    fn disconnect_after_limits_polls() {
        let mut sim = FakeSimBackend::with_telemetry(
            "DCS",
            vec![
                TelemetrySnapshot::on_ramp(),
                TelemetrySnapshot::cruising(),
                TelemetrySnapshot::cruising(),
            ],
        );
        sim.disconnect_after(2);
        assert!(sim.poll().is_some());
        assert!(sim.poll().is_some());
        assert!(sim.poll().is_none());
        assert_eq!(sim.state(), SimConnectionState::Disconnected);
    }

    #[test]
    fn reconnect_resets_position() {
        let mut sim = FakeSimBackend::with_telemetry(
            "MSFS",
            vec![TelemetrySnapshot::on_ramp()],
        );
        sim.poll();
        assert!(sim.poll().is_none());
        sim.reconnect();
        assert!(sim.poll().is_some());
    }

    #[test]
    fn commands_recorded() {
        let mut sim = FakeSimBackend::new("DCS");
        sim.send_command("GEAR_TOGGLE");
        sim.send_command("FLAPS_DOWN");
        assert_eq!(sim.commands(), &["GEAR_TOGGLE", "FLAPS_DOWN"]);
    }

    #[test]
    fn set_aircraft() {
        let mut sim = FakeSimBackend::new("MSFS");
        sim.set_aircraft("A320neo");
        assert_eq!(sim.aircraft.as_deref(), Some("A320neo"));
    }

    #[test]
    fn push_snapshot_extends_sequence() {
        let mut sim = FakeSimBackend::with_telemetry("MSFS", vec![]);
        sim.push_snapshot(TelemetrySnapshot::cruising());
        assert!(sim.poll().is_some());
    }

    #[test]
    fn reset_clears_state() {
        let mut sim = FakeSimBackend::with_telemetry(
            "MSFS",
            vec![TelemetrySnapshot::on_ramp()],
        );
        sim.poll();
        sim.send_command("CMD");
        sim.reset();
        assert_eq!(sim.polls_completed(), 0);
        assert!(sim.commands().is_empty());
        assert!(sim.poll().is_some());
    }

    #[test]
    fn telemetry_snapshot_presets() {
        let ramp = TelemetrySnapshot::on_ramp();
        assert!(ramp.on_ground);
        assert!(ramp.gear_down);
        assert_eq!(ramp.altitude_ft, 0.0);

        let cruise = TelemetrySnapshot::cruising();
        assert!(!cruise.on_ground);
        assert!(!cruise.gear_down);
        assert_eq!(cruise.altitude_ft, 35_000.0);
    }

    #[test]
    fn polls_completed_tracks_consumption() {
        let mut sim = FakeSimBackend::with_telemetry(
            "X-Plane",
            vec![TelemetrySnapshot::on_ramp(), TelemetrySnapshot::cruising()],
        );
        assert_eq!(sim.polls_completed(), 0);
        sim.poll();
        assert_eq!(sim.polls_completed(), 1);
        sim.poll();
        assert_eq!(sim.polls_completed(), 2);
    }
}
