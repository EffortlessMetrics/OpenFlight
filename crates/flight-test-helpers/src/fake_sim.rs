// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! A fake simulator backend for testing without a running sim.

/// A single snapshot of simulated flight state.
#[derive(Debug, Clone)]
pub struct FakeSnapshot {
    pub altitude: f64,
    pub airspeed: f64,
    pub heading: f64,
    pub pitch: f64,
    pub roll: f64,
    pub yaw: f64,
    pub on_ground: bool,
}

/// A fake simulator backend for testing.
#[derive(Debug)]
pub struct FakeSim {
    pub name: String,
    pub connected: bool,
    pub aircraft: Option<String>,
    snapshots: Vec<FakeSnapshot>,
    snapshot_position: usize,
    received_commands: Vec<String>,
}

impl FakeSim {
    /// Create a new fake sim with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            connected: false,
            aircraft: None,
            snapshots: Vec::new(),
            snapshot_position: 0,
            received_commands: Vec::new(),
        }
    }

    /// Mark the sim as connected.
    pub fn connect(&mut self) {
        self.connected = true;
    }

    /// Mark the sim as disconnected.
    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    /// Set the active aircraft type.
    pub fn set_aircraft(&mut self, name: &str) {
        self.aircraft = Some(name.to_string());
    }

    /// Push a snapshot into the replay queue.
    pub fn push_snapshot(&mut self, snapshot: FakeSnapshot) {
        self.snapshots.push(snapshot);
    }

    /// Consume the next snapshot from the queue.
    pub fn next_snapshot(&mut self) -> Option<FakeSnapshot> {
        if self.snapshot_position < self.snapshots.len() {
            let snap = self.snapshots[self.snapshot_position].clone();
            self.snapshot_position += 1;
            Some(snap)
        } else {
            None
        }
    }

    /// Record a command sent to the sim.
    pub fn send_command(&mut self, cmd: &str) {
        self.received_commands.push(cmd.to_string());
    }

    /// Return all commands that have been recorded.
    pub fn received_commands(&self) -> &[String] {
        &self.received_commands
    }

    /// Clear all snapshots, reset playback position, and clear recorded commands.
    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.snapshot_position = 0;
        self.received_commands.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{FakeSim, FakeSnapshot};

    fn sample_snapshot(altitude: f64, on_ground: bool) -> FakeSnapshot {
        FakeSnapshot {
            altitude,
            airspeed: 120.0,
            heading: 270.0,
            pitch: 2.5,
            roll: 0.0,
            yaw: 0.0,
            on_ground,
        }
    }

    #[test]
    fn new_sim_defaults() {
        let sim = FakeSim::new("MSFS");
        assert_eq!(sim.name, "MSFS");
        assert!(!sim.connected);
        assert!(sim.aircraft.is_none());
        assert!(sim.received_commands().is_empty());
    }

    #[test]
    fn connect_disconnect() {
        let mut sim = FakeSim::new("X-Plane");
        sim.connect();
        assert!(sim.connected);
        sim.disconnect();
        assert!(!sim.connected);
    }

    #[test]
    fn set_aircraft() {
        let mut sim = FakeSim::new("DCS");
        sim.set_aircraft("F-16C");
        assert_eq!(sim.aircraft.as_deref(), Some("F-16C"));
    }

    #[test]
    fn snapshot_replay() {
        let mut sim = FakeSim::new("MSFS");
        sim.push_snapshot(sample_snapshot(0.0, true));
        sim.push_snapshot(sample_snapshot(5000.0, false));

        let s1 = sim.next_snapshot().unwrap();
        assert!(s1.on_ground);
        assert!((s1.altitude - 0.0).abs() < f64::EPSILON);

        let s2 = sim.next_snapshot().unwrap();
        assert!(!s2.on_ground);
        assert!((s2.altitude - 5000.0).abs() < f64::EPSILON);

        assert!(sim.next_snapshot().is_none());
    }

    #[test]
    fn command_recording() {
        let mut sim = FakeSim::new("DCS");
        sim.send_command("GEAR_TOGGLE");
        sim.send_command("FLAPS_UP");
        assert_eq!(sim.received_commands().len(), 2);
        assert_eq!(sim.received_commands()[0], "GEAR_TOGGLE");
        assert_eq!(sim.received_commands()[1], "FLAPS_UP");
    }

    #[test]
    fn clear_resets_everything() {
        let mut sim = FakeSim::new("MSFS");
        sim.push_snapshot(sample_snapshot(1000.0, false));
        sim.send_command("AUTOPILOT_ON");
        sim.next_snapshot();
        sim.clear();

        assert!(sim.next_snapshot().is_none());
        assert!(sim.received_commands().is_empty());
    }
}
