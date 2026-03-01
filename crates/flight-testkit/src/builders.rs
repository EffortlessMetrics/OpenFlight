// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fluent builders for constructing test fixtures.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// AxisPipelineBuilder
// ---------------------------------------------------------------------------

/// Describes one stage in a test axis pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PipelineStageSpec {
    Deadzone { inner: f64, outer: f64 },
    Curve { expo: f64 },
    Clamp { min: f64, max: f64 },
    Sensitivity { multiplier: f64 },
    Smoothing { alpha: f64 },
}

/// Accumulated pipeline specification built via fluent API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AxisPipelineSpec {
    pub stages: Vec<PipelineStageSpec>,
}

/// Fluent builder for axis pipeline test specifications.
#[derive(Debug, Clone)]
pub struct AxisPipelineBuilder {
    stages: Vec<PipelineStageSpec>,
}

impl AxisPipelineBuilder {
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
        }
    }

    pub fn deadzone(mut self, inner: f64, outer: f64) -> Self {
        self.stages
            .push(PipelineStageSpec::Deadzone { inner, outer });
        self
    }

    pub fn curve(mut self, expo: f64) -> Self {
        self.stages.push(PipelineStageSpec::Curve { expo });
        self
    }

    pub fn clamp(mut self, min: f64, max: f64) -> Self {
        self.stages.push(PipelineStageSpec::Clamp { min, max });
        self
    }

    pub fn sensitivity(mut self, multiplier: f64) -> Self {
        self.stages
            .push(PipelineStageSpec::Sensitivity { multiplier });
        self
    }

    pub fn smoothing(mut self, alpha: f64) -> Self {
        self.stages.push(PipelineStageSpec::Smoothing { alpha });
        self
    }

    /// Pre-built "standard" pipeline: 5 % deadzone → expo 0.3 curve → clamp ±1.
    pub fn standard(self) -> Self {
        self.deadzone(0.05, 1.0).curve(0.3).clamp(-1.0, 1.0)
    }

    pub fn build(self) -> AxisPipelineSpec {
        AxisPipelineSpec {
            stages: self.stages,
        }
    }
}

impl Default for AxisPipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ProfileBuilder
// ---------------------------------------------------------------------------

/// Minimal profile fixture for tests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileSpec {
    pub name: String,
    pub simulator: String,
    pub aircraft: Option<String>,
    pub deadzone: f64,
    pub curve_expo: f64,
    pub sensitivity: f64,
}

/// Fluent builder for test profile specifications.
#[derive(Debug, Clone)]
pub struct ProfileBuilder {
    spec: ProfileSpec,
}

impl ProfileBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            spec: ProfileSpec {
                name: name.into(),
                simulator: "MSFS".to_owned(),
                aircraft: None,
                deadzone: 0.05,
                curve_expo: 0.0,
                sensitivity: 1.0,
            },
        }
    }

    pub fn simulator(mut self, sim: impl Into<String>) -> Self {
        self.spec.simulator = sim.into();
        self
    }

    pub fn aircraft(mut self, ac: impl Into<String>) -> Self {
        self.spec.aircraft = Some(ac.into());
        self
    }

    pub fn deadzone(mut self, dz: f64) -> Self {
        self.spec.deadzone = dz;
        self
    }

    pub fn curve_expo(mut self, expo: f64) -> Self {
        self.spec.curve_expo = expo;
        self
    }

    pub fn sensitivity(mut self, sens: f64) -> Self {
        self.spec.sensitivity = sens;
        self
    }

    /// Pre-built "combat" profile: DCS, 3 % deadzone, expo 0.3.
    pub fn combat(self) -> Self {
        self.simulator("DCS").deadzone(0.03).curve_expo(0.3)
    }

    /// Pre-built "airliner" profile: MSFS, 8 % deadzone, linear, low sensitivity.
    pub fn airliner(self) -> Self {
        self.simulator("MSFS")
            .deadzone(0.08)
            .curve_expo(0.0)
            .sensitivity(0.7)
    }

    pub fn build(self) -> ProfileSpec {
        self.spec
    }
}

// ---------------------------------------------------------------------------
// DeviceBuilder
// ---------------------------------------------------------------------------

/// Minimal device fixture specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceSpec {
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    pub axis_count: usize,
    pub button_count: usize,
    pub hat_count: usize,
}

/// Fluent builder for fake device specifications.
#[derive(Debug, Clone)]
pub struct DeviceBuilder {
    spec: DeviceSpec,
}

impl DeviceBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            spec: DeviceSpec {
                name: name.into(),
                vid: 0x1234,
                pid: 0x5678,
                axis_count: 3,
                button_count: 12,
                hat_count: 1,
            },
        }
    }

    pub fn vid_pid(mut self, vid: u16, pid: u16) -> Self {
        self.spec.vid = vid;
        self.spec.pid = pid;
        self
    }

    pub fn axes(mut self, count: usize) -> Self {
        self.spec.axis_count = count;
        self
    }

    pub fn buttons(mut self, count: usize) -> Self {
        self.spec.button_count = count;
        self
    }

    pub fn hats(mut self, count: usize) -> Self {
        self.spec.hat_count = count;
        self
    }

    /// Saitek X52 Pro preset.
    pub fn x52_pro(self) -> Self {
        self.vid_pid(0x06a3, 0x0762)
            .axes(7)
            .buttons(39)
            .hats(1)
    }

    /// Thrustmaster Warthog stick preset.
    pub fn warthog(self) -> Self {
        self.vid_pid(0x044f, 0x0402)
            .axes(4)
            .buttons(19)
            .hats(1)
    }

    pub fn build(self) -> DeviceSpec {
        self.spec
    }
}

// ---------------------------------------------------------------------------
// SnapshotBuilder
// ---------------------------------------------------------------------------

/// Minimal bus snapshot fixture for tests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotSpec {
    pub timestamp_us: u64,
    pub axes: Vec<f64>,
    pub buttons: Vec<bool>,
    pub connected: bool,
    pub simulator: String,
}

/// Fluent builder for bus snapshot test specifications.
#[derive(Debug, Clone)]
pub struct SnapshotBuilder {
    spec: SnapshotSpec,
}

impl SnapshotBuilder {
    pub fn new() -> Self {
        Self {
            spec: SnapshotSpec {
                timestamp_us: 0,
                axes: Vec::new(),
                buttons: Vec::new(),
                connected: true,
                simulator: "MSFS".to_owned(),
            },
        }
    }

    pub fn timestamp(mut self, us: u64) -> Self {
        self.spec.timestamp_us = us;
        self
    }

    pub fn axes(mut self, values: Vec<f64>) -> Self {
        self.spec.axes = values;
        self
    }

    pub fn buttons(mut self, values: Vec<bool>) -> Self {
        self.spec.buttons = values;
        self
    }

    pub fn connected(mut self, c: bool) -> Self {
        self.spec.connected = c;
        self
    }

    pub fn simulator(mut self, sim: impl Into<String>) -> Self {
        self.spec.simulator = sim.into();
        self
    }

    /// Idle state: 4 centred axes, no buttons pressed.
    pub fn idle_4axis(self) -> Self {
        self.axes(vec![0.0; 4]).buttons(vec![false; 12])
    }

    pub fn build(self) -> SnapshotSpec {
        self.spec
    }
}

impl Default for SnapshotBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- AxisPipelineBuilder -------------------------------------------------

    #[test]
    fn pipeline_builder_empty() {
        let spec = AxisPipelineBuilder::new().build();
        assert!(spec.stages.is_empty());
    }

    #[test]
    fn pipeline_builder_standard() {
        let spec = AxisPipelineBuilder::new().standard().build();
        assert_eq!(spec.stages.len(), 3);
        assert!(matches!(
            spec.stages[0],
            PipelineStageSpec::Deadzone {
                inner: _,
                outer: _
            }
        ));
        assert!(matches!(spec.stages[1], PipelineStageSpec::Curve { .. }));
        assert!(matches!(spec.stages[2], PipelineStageSpec::Clamp { .. }));
    }

    #[test]
    fn pipeline_builder_custom() {
        let spec = AxisPipelineBuilder::new()
            .sensitivity(2.0)
            .smoothing(0.3)
            .build();
        assert_eq!(spec.stages.len(), 2);
    }

    #[test]
    fn pipeline_spec_serializes() {
        let spec = AxisPipelineBuilder::new().standard().build();
        let json = serde_json::to_string(&spec).unwrap();
        let deser: AxisPipelineSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, deser);
    }

    // -- ProfileBuilder ------------------------------------------------------

    #[test]
    fn profile_builder_defaults() {
        let p = ProfileBuilder::new("default").build();
        assert_eq!(p.simulator, "MSFS");
        assert!(p.aircraft.is_none());
        assert!((p.deadzone - 0.05).abs() < f64::EPSILON);
        assert!((p.sensitivity - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn profile_builder_combat() {
        let p = ProfileBuilder::new("viper").combat().build();
        assert_eq!(p.simulator, "DCS");
        assert!((p.deadzone - 0.03).abs() < f64::EPSILON);
        assert!((p.curve_expo - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn profile_builder_airliner() {
        let p = ProfileBuilder::new("a320").airliner().build();
        assert_eq!(p.simulator, "MSFS");
        assert!((p.deadzone - 0.08).abs() < f64::EPSILON);
        assert!((p.sensitivity - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn profile_builder_full() {
        let p = ProfileBuilder::new("custom")
            .simulator("X-Plane")
            .aircraft("C172")
            .deadzone(0.02)
            .curve_expo(0.5)
            .sensitivity(1.2)
            .build();
        assert_eq!(p.simulator, "X-Plane");
        assert_eq!(p.aircraft.as_deref(), Some("C172"));
    }

    // -- DeviceBuilder -------------------------------------------------------

    #[test]
    fn device_builder_defaults() {
        let d = DeviceBuilder::new("Stick").build();
        assert_eq!(d.vid, 0x1234);
        assert_eq!(d.axis_count, 3);
        assert_eq!(d.button_count, 12);
        assert_eq!(d.hat_count, 1);
    }

    #[test]
    fn device_builder_x52_pro() {
        let d = DeviceBuilder::new("X52 Pro").x52_pro().build();
        assert_eq!(d.vid, 0x06a3);
        assert_eq!(d.pid, 0x0762);
        assert_eq!(d.axis_count, 7);
    }

    #[test]
    fn device_builder_warthog() {
        let d = DeviceBuilder::new("Warthog").warthog().build();
        assert_eq!(d.vid, 0x044f);
        assert_eq!(d.pid, 0x0402);
    }

    #[test]
    fn device_builder_custom() {
        let d = DeviceBuilder::new("Custom")
            .vid_pid(0xBEEF, 0xCAFE)
            .axes(6)
            .buttons(32)
            .hats(2)
            .build();
        assert_eq!(d.vid, 0xBEEF);
        assert_eq!(d.axis_count, 6);
        assert_eq!(d.hat_count, 2);
    }

    // -- SnapshotBuilder -----------------------------------------------------

    #[test]
    fn snapshot_builder_defaults() {
        let s = SnapshotBuilder::new().build();
        assert_eq!(s.timestamp_us, 0);
        assert!(s.axes.is_empty());
        assert!(s.connected);
    }

    #[test]
    fn snapshot_builder_idle() {
        let s = SnapshotBuilder::new().idle_4axis().build();
        assert_eq!(s.axes.len(), 4);
        assert_eq!(s.buttons.len(), 12);
        assert!(s.axes.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn snapshot_builder_custom() {
        let s = SnapshotBuilder::new()
            .timestamp(5000)
            .axes(vec![0.5, -0.5])
            .buttons(vec![true, false])
            .connected(false)
            .simulator("DCS")
            .build();
        assert_eq!(s.timestamp_us, 5000);
        assert!(!s.connected);
        assert_eq!(s.simulator, "DCS");
    }

    #[test]
    fn snapshot_spec_serializes() {
        let s = SnapshotBuilder::new()
            .idle_4axis()
            .timestamp(1000)
            .build();
        let json = serde_json::to_string(&s).unwrap();
        let deser: SnapshotSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(s, deser);
    }
}
