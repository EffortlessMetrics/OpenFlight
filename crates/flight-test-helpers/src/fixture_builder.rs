// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Fluent builders for constructing test fixtures (devices, profiles, telemetry).

/// A single axis in a device fixture.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisFixture {
    pub index: u8,
    pub value: f64,
    pub name: String,
}

/// A single button in a device fixture.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonFixture {
    pub index: u8,
    pub pressed: bool,
    pub name: String,
}

/// Synthetic device fixture for tests.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceFixture {
    pub id: String,
    pub name: String,
    pub axes: Vec<AxisFixture>,
    pub buttons: Vec<ButtonFixture>,
}

/// Fluent builder for [`DeviceFixture`].
#[derive(Debug, Clone)]
pub struct DeviceFixtureBuilder {
    device: DeviceFixture,
}

impl DeviceFixtureBuilder {
    /// Start building a device fixture with the given id.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            device: DeviceFixture {
                id: id.into(),
                name: String::new(),
                axes: Vec::new(),
                buttons: Vec::new(),
            },
        }
    }

    /// Set the device display name.
    pub fn name(mut self, n: impl Into<String>) -> Self {
        self.device.name = n.into();
        self
    }

    /// Add a named axis with a value.
    pub fn axis(mut self, index: u8, value: f64, name: impl Into<String>) -> Self {
        self.device.axes.push(AxisFixture {
            index,
            value,
            name: name.into(),
        });
        self
    }

    /// Add a named button.
    pub fn button(mut self, index: u8, pressed: bool, name: impl Into<String>) -> Self {
        self.device.buttons.push(ButtonFixture {
            index,
            pressed,
            name: name.into(),
        });
        self
    }

    /// Add standard flight axes: pitch (0), roll (1), yaw (2), throttle (3) at centre/idle.
    pub fn with_standard_axes(self) -> Self {
        self.axis(0, 0.0, "pitch")
            .axis(1, 0.0, "roll")
            .axis(2, 0.0, "yaw")
            .axis(3, 0.0, "throttle")
    }

    /// Add typical HOTAS buttons (trigger, thumb, pinkie, hat push).
    pub fn with_hotas_buttons(self) -> Self {
        self.button(0, false, "trigger")
            .button(1, false, "thumb")
            .button(2, false, "pinkie")
            .button(3, false, "hat_push")
    }

    /// Consume the builder and return the finished fixture.
    pub fn build(self) -> DeviceFixture {
        self.device
    }
}

// ---------------------------------------------------------------------------
// Profile fixture
// ---------------------------------------------------------------------------

/// Minimal profile fixture for tests.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileFixture {
    pub name: String,
    pub simulator: String,
    pub aircraft: Option<String>,
    pub curve_points: Vec<(f64, f64)>,
    pub deadzone: f64,
}

/// Fluent builder for [`ProfileFixture`].
#[derive(Debug, Clone)]
pub struct ProfileFixtureBuilder {
    fixture: ProfileFixture,
}

impl ProfileFixtureBuilder {
    /// Start building a profile fixture with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            fixture: ProfileFixture {
                name: name.into(),
                simulator: "MSFS".to_owned(),
                aircraft: None,
                curve_points: Vec::new(),
                deadzone: 0.0,
            },
        }
    }

    /// Set the target simulator.
    pub fn simulator(mut self, sim: impl Into<String>) -> Self {
        self.fixture.simulator = sim.into();
        self
    }

    /// Set the target aircraft.
    pub fn aircraft(mut self, ac: impl Into<String>) -> Self {
        self.fixture.aircraft = Some(ac.into());
        self
    }

    /// Add a curve control point.
    pub fn curve_point(mut self, input: f64, output: f64) -> Self {
        self.fixture.curve_points.push((input, output));
        self
    }

    /// Populate a linear 1:1 curve.
    pub fn with_linear_curve(self) -> Self {
        self.curve_point(0.0, 0.0).curve_point(1.0, 1.0)
    }

    /// Set the deadzone percentage (0.0–1.0).
    pub fn deadzone(mut self, dz: f64) -> Self {
        self.fixture.deadzone = dz;
        self
    }

    /// Consume the builder.
    pub fn build(self) -> ProfileFixture {
        self.fixture
    }
}

// ---------------------------------------------------------------------------
// Telemetry fixture
// ---------------------------------------------------------------------------

/// Minimal telemetry snapshot fixture.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryFixture {
    pub airspeed_kts: f64,
    pub altitude_ft: f64,
    pub heading_deg: f64,
    pub vertical_speed_fpm: f64,
    pub on_ground: bool,
}

/// Fluent builder for [`TelemetryFixture`].
#[derive(Debug, Clone)]
pub struct TelemetryFixtureBuilder {
    fixture: TelemetryFixture,
}

impl TelemetryFixtureBuilder {
    /// Start with zeroed telemetry on the ground.
    pub fn new() -> Self {
        Self {
            fixture: TelemetryFixture {
                airspeed_kts: 0.0,
                altitude_ft: 0.0,
                heading_deg: 0.0,
                vertical_speed_fpm: 0.0,
                on_ground: true,
            },
        }
    }

    pub fn airspeed(mut self, kts: f64) -> Self {
        self.fixture.airspeed_kts = kts;
        self
    }

    pub fn altitude(mut self, ft: f64) -> Self {
        self.fixture.altitude_ft = ft;
        self
    }

    pub fn heading(mut self, deg: f64) -> Self {
        self.fixture.heading_deg = deg;
        self
    }

    pub fn vertical_speed(mut self, fpm: f64) -> Self {
        self.fixture.vertical_speed_fpm = fpm;
        self
    }

    pub fn on_ground(mut self, on: bool) -> Self {
        self.fixture.on_ground = on;
        self
    }

    /// Pre-built "cruising" telemetry state.
    pub fn cruising(self) -> Self {
        self.airspeed(250.0)
            .altitude(35_000.0)
            .heading(90.0)
            .vertical_speed(0.0)
            .on_ground(false)
    }

    /// Pre-built "on ramp" telemetry state.
    pub fn on_ramp(self) -> Self {
        self.airspeed(0.0)
            .altitude(0.0)
            .heading(0.0)
            .vertical_speed(0.0)
            .on_ground(true)
    }

    pub fn build(self) -> TelemetryFixture {
        self.fixture
    }
}

impl Default for TelemetryFixtureBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_builder_minimal() {
        let dev = DeviceFixtureBuilder::new("dev-1").build();
        assert_eq!(dev.id, "dev-1");
        assert!(dev.axes.is_empty());
        assert!(dev.buttons.is_empty());
    }

    #[test]
    fn device_builder_with_name() {
        let dev = DeviceFixtureBuilder::new("x52")
            .name("Saitek X52 Pro")
            .build();
        assert_eq!(dev.name, "Saitek X52 Pro");
    }

    #[test]
    fn device_builder_standard_axes() {
        let dev = DeviceFixtureBuilder::new("stick")
            .with_standard_axes()
            .build();
        assert_eq!(dev.axes.len(), 4);
        assert_eq!(dev.axes[0].name, "pitch");
        assert_eq!(dev.axes[3].name, "throttle");
    }

    #[test]
    fn device_builder_hotas_buttons() {
        let dev = DeviceFixtureBuilder::new("hotas")
            .with_hotas_buttons()
            .build();
        assert_eq!(dev.buttons.len(), 4);
        assert_eq!(dev.buttons[0].name, "trigger");
        assert!(!dev.buttons[0].pressed);
    }

    #[test]
    fn device_builder_custom_axis_and_button() {
        let dev = DeviceFixtureBuilder::new("custom")
            .axis(5, 0.75, "slider")
            .button(10, true, "fire")
            .build();
        assert_eq!(dev.axes[0].index, 5);
        assert_eq!(dev.axes[0].value, 0.75);
        assert!(dev.buttons[0].pressed);
    }

    #[test]
    fn profile_builder_defaults() {
        let p = ProfileFixtureBuilder::new("default").build();
        assert_eq!(p.simulator, "MSFS");
        assert!(p.aircraft.is_none());
        assert!(p.curve_points.is_empty());
        assert_eq!(p.deadzone, 0.0);
    }

    #[test]
    fn profile_builder_full() {
        let p = ProfileFixtureBuilder::new("combat")
            .simulator("DCS")
            .aircraft("F-16C")
            .deadzone(0.05)
            .with_linear_curve()
            .build();
        assert_eq!(p.simulator, "DCS");
        assert_eq!(p.aircraft.as_deref(), Some("F-16C"));
        assert_eq!(p.curve_points.len(), 2);
        assert_eq!(p.deadzone, 0.05);
    }

    #[test]
    fn telemetry_builder_defaults() {
        let t = TelemetryFixtureBuilder::new().build();
        assert!(t.on_ground);
        assert_eq!(t.airspeed_kts, 0.0);
    }

    #[test]
    fn telemetry_builder_cruising() {
        let t = TelemetryFixtureBuilder::new().cruising().build();
        assert!(!t.on_ground);
        assert_eq!(t.altitude_ft, 35_000.0);
        assert_eq!(t.airspeed_kts, 250.0);
    }

    #[test]
    fn telemetry_builder_on_ramp() {
        let t = TelemetryFixtureBuilder::new().on_ramp().build();
        assert!(t.on_ground);
        assert_eq!(t.airspeed_kts, 0.0);
    }

    #[test]
    fn telemetry_builder_custom() {
        let t = TelemetryFixtureBuilder::new()
            .airspeed(180.0)
            .altitude(5000.0)
            .heading(270.0)
            .vertical_speed(-500.0)
            .on_ground(false)
            .build();
        assert_eq!(t.airspeed_kts, 180.0);
        assert_eq!(t.heading_deg, 270.0);
        assert_eq!(t.vertical_speed_fpm, -500.0);
    }

    #[test]
    fn device_builder_chaining_preserves_order() {
        let dev = DeviceFixtureBuilder::new("ordered")
            .axis(0, 0.1, "a0")
            .axis(1, 0.2, "a1")
            .button(0, false, "b0")
            .button(1, true, "b1")
            .build();
        assert_eq!(dev.axes[0].index, 0);
        assert_eq!(dev.axes[1].index, 1);
        assert!(!dev.buttons[0].pressed);
        assert!(dev.buttons[1].pressed);
    }
}
