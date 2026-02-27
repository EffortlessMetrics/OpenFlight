// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! AeroWinx PSX Boeing 744 simulator adapter for OpenFlight.
//!
//! PSX exposes a proprietary text protocol over TCP (default port
//! [`PSX_DEFAULT_PORT`]).  Clients send variable-query or subscription
//! commands; PSX replies with lines of the form:
//!
//! ```text
//! <id>=<value>
//! ```
//!
//! where `<id>` is a variable identifier (e.g. `Qi0415`) and `<value>` is a
//! decimal number.  This crate parses those response lines into typed
//! [`PsxVariable`] / `f64` pairs.
//!
//! ## Known variable IDs (sampled subset)
//!
//! | ID       | [`PsxVariable`] variant | Description                 | Unit   |
//! |----------|-------------------------|-----------------------------|--------|
//! | `Qi0001` | `FcuSpd`                | FCU selected speed          | knots  |
//! | `Qi0002` | `FcuHdg`                | FCU selected heading        | degrees|
//! | `Qi0010` | `N1Left`                | Left engine N1              | %      |
//! | `Qi0011` | `N1Right`               | Right engine N1             | %      |
//! | `Qi0100` | `FuelLeft`              | Left wing fuel quantity     | kg     |
//! | `Qi0101` | `FuelRight`             | Right wing fuel quantity    | kg     |
//! | `Qi0200` | `GearDown`              | Gear position (0=up, 1=down)| –      |
//!
//! Unknown IDs are wrapped in [`PsxVariable::Unknown`].

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Reserved for future real implementation; silences the unused-extern-crate lint.
#[allow(unused_extern_crates)]
extern crate flight_core;

/// Default TCP port PSX listens on.
pub const PSX_DEFAULT_PORT: u16 = 10747;

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors produced by the PSX adapter.
#[derive(Debug, Error, PartialEq)]
pub enum PsxAdapterError {
    /// The line did not contain the `=` separator.
    #[error("missing '=' separator in PSX line: {line:?}")]
    MissingSeparator { line: String },

    /// The numeric value after `=` could not be parsed.
    #[error("invalid numeric value in PSX line: {raw:?}")]
    InvalidValue { raw: String },

    /// The variable ID was empty.
    #[error("empty variable ID in PSX line")]
    EmptyId,
}

// ── Domain types ──────────────────────────────────────────────────────────────

/// Known PSX variable identifiers (Boeing 744 subset).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PsxVariable {
    /// FCU selected speed in knots (`Qi0001`).
    FcuSpd,
    /// FCU selected heading in degrees (`Qi0002`).
    FcuHdg,
    /// Left engine N1 in percent (`Qi0010`).
    N1Left,
    /// Right engine N1 in percent (`Qi0011`).
    N1Right,
    /// Left wing fuel quantity in kg (`Qi0100`).
    FuelLeft,
    /// Right wing fuel quantity in kg (`Qi0101`).
    FuelRight,
    /// Gear position: `0.0` = up, `1.0` = down (`Qi0200`).
    GearDown,
    /// Variable ID not recognised by this adapter.
    Unknown(String),
}

impl PsxVariable {
    /// Map a raw PSX variable ID string to a [`PsxVariable`].
    pub fn from_id(id: &str) -> Self {
        match id {
            "Qi0001" => PsxVariable::FcuSpd,
            "Qi0002" => PsxVariable::FcuHdg,
            "Qi0010" => PsxVariable::N1Left,
            "Qi0011" => PsxVariable::N1Right,
            "Qi0100" => PsxVariable::FuelLeft,
            "Qi0101" => PsxVariable::FuelRight,
            "Qi0200" => PsxVariable::GearDown,
            other => PsxVariable::Unknown(other.to_owned()),
        }
    }

    /// Return the raw PSX variable ID string for known variables.
    ///
    /// Returns `None` for [`PsxVariable::Unknown`].
    pub fn id(&self) -> Option<&str> {
        match self {
            PsxVariable::FcuSpd => Some("Qi0001"),
            PsxVariable::FcuHdg => Some("Qi0002"),
            PsxVariable::N1Left => Some("Qi0010"),
            PsxVariable::N1Right => Some("Qi0011"),
            PsxVariable::FuelLeft => Some("Qi0100"),
            PsxVariable::FuelRight => Some("Qi0101"),
            PsxVariable::GearDown => Some("Qi0200"),
            PsxVariable::Unknown(_) => None,
        }
    }
}

/// Snapshot of PSX Boeing 744 flight-deck state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PsxTelemetry {
    /// FCU selected speed in knots.
    pub fcu_spd: f64,
    /// FCU selected heading in degrees.
    pub fcu_hdg: f64,
    /// Left engine N1 in percent.
    pub n1_left: f64,
    /// Right engine N1 in percent.
    pub n1_right: f64,
    /// Left wing fuel quantity in kg.
    pub fuel_left: f64,
    /// Right wing fuel quantity in kg.
    pub fuel_right: f64,
    /// `true` when the gear is down.
    pub gear_down: bool,
}

impl Default for PsxTelemetry {
    fn default() -> Self {
        Self {
            fcu_spd: 0.0,
            fcu_hdg: 0.0,
            n1_left: 0.0,
            n1_right: 0.0,
            fuel_left: 0.0,
            fuel_right: 0.0,
            gear_down: false,
        }
    }
}

impl PsxTelemetry {
    /// Apply a single parsed `(variable, value)` pair to this telemetry snapshot.
    pub fn apply(&mut self, variable: &PsxVariable, value: f64) {
        match variable {
            PsxVariable::FcuSpd => self.fcu_spd = value,
            PsxVariable::FcuHdg => self.fcu_hdg = value,
            PsxVariable::N1Left => self.n1_left = value,
            PsxVariable::N1Right => self.n1_right = value,
            PsxVariable::FuelLeft => self.fuel_left = value,
            PsxVariable::FuelRight => self.fuel_right = value,
            PsxVariable::GearDown => self.gear_down = value >= 0.5,
            PsxVariable::Unknown(_) => {}
        }
    }
}

// ── Adapter ───────────────────────────────────────────────────────────────────

/// AeroWinx PSX TCP adapter.
///
/// In a real deployment, open a TCP connection to
/// `127.0.0.1:`[`PSX_DEFAULT_PORT`] and feed each received line to
/// [`process_line`](Self::process_line).
pub struct PsxAdapter {
    /// TCP port the adapter connects to.
    pub port: u16,
    telemetry: PsxTelemetry,
}

impl PsxAdapter {
    /// Create a new adapter targeting the default PSX port
    /// ([`PSX_DEFAULT_PORT`]).
    pub fn new() -> Self {
        tracing::info!(port = PSX_DEFAULT_PORT, "PSX adapter created");
        Self {
            port: PSX_DEFAULT_PORT,
            telemetry: PsxTelemetry::default(),
        }
    }

    /// Create a new adapter targeting a custom TCP `port`.
    pub fn with_port(port: u16) -> Self {
        tracing::info!(port, "PSX adapter created with custom port");
        Self {
            port,
            telemetry: PsxTelemetry::default(),
        }
    }

    /// Parse a single PSX response line and update the internal telemetry
    /// snapshot.
    ///
    /// Returns the `(variable, value)` pair on success.
    pub fn process_line(&mut self, line: &str) -> Result<(PsxVariable, f64), PsxAdapterError> {
        tracing::debug!(line, "processing PSX line");
        let (var, val) = parse_psx_line(line)?;
        self.telemetry.apply(&var, val);
        Ok((var, val))
    }

    /// Return a reference to the current accumulated telemetry snapshot.
    pub fn telemetry(&self) -> &PsxTelemetry {
        &self.telemetry
    }
}

impl Default for PsxAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

/// Parse a single PSX protocol line into a `(variable, value)` pair.
///
/// Lines have the form `<id>=<value>`, e.g. `Qi0415=123.45`.
/// Leading and trailing whitespace is ignored.
///
/// # Errors
///
/// - [`PsxAdapterError::MissingSeparator`] — no `=` found.
/// - [`PsxAdapterError::EmptyId`] — the ID part is empty.
/// - [`PsxAdapterError::InvalidValue`] — the value part is not a valid `f64`.
pub fn parse_psx_line(line: &str) -> Result<(PsxVariable, f64), PsxAdapterError> {
    let line = line.trim();
    let Some(sep) = line.find('=') else {
        return Err(PsxAdapterError::MissingSeparator {
            line: line.to_owned(),
        });
    };

    let id = &line[..sep];
    if id.is_empty() {
        return Err(PsxAdapterError::EmptyId);
    }

    let raw_value = &line[sep + 1..];
    let value: f64 = raw_value
        .parse()
        .map_err(|_| PsxAdapterError::InvalidValue {
            raw: raw_value.to_owned(),
        })?;

    let variable = PsxVariable::from_id(id);
    tracing::trace!(id, value, "parsed PSX variable");
    Ok((variable, value))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_psx_line ───────────────────────────────────────────────────────

    #[test]
    fn parse_known_variable_fcu_spd() {
        let (var, val) = parse_psx_line("Qi0001=280.0").unwrap();
        assert_eq!(var, PsxVariable::FcuSpd);
        assert!((val - 280.0).abs() < 0.01);
    }

    #[test]
    fn parse_known_variable_n1_left() {
        let (var, val) = parse_psx_line("Qi0010=88.5").unwrap();
        assert_eq!(var, PsxVariable::N1Left);
        assert!((val - 88.5).abs() < 0.01);
    }

    #[test]
    fn parse_known_variable_gear_down() {
        let (var, val) = parse_psx_line("Qi0200=1").unwrap();
        assert_eq!(var, PsxVariable::GearDown);
        assert!((val - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_unknown_variable_preserved() {
        let (var, val) = parse_psx_line("Qi9999=42.0").unwrap();
        assert_eq!(var, PsxVariable::Unknown("Qi9999".to_owned()));
        assert!((val - 42.0).abs() < 0.01);
    }

    #[test]
    fn missing_separator_returns_error() {
        let err = parse_psx_line("Qi0001 280.0").unwrap_err();
        assert!(matches!(err, PsxAdapterError::MissingSeparator { .. }));
    }

    #[test]
    fn empty_id_returns_error() {
        let err = parse_psx_line("=123.0").unwrap_err();
        assert!(matches!(err, PsxAdapterError::EmptyId));
    }

    #[test]
    fn invalid_value_returns_error() {
        let err = parse_psx_line("Qi0001=not_a_number").unwrap_err();
        assert!(matches!(err, PsxAdapterError::InvalidValue { .. }));
    }

    #[test]
    fn whitespace_trimmed_from_line() {
        let (var, val) = parse_psx_line("  Qi0002=360.0  ").unwrap();
        assert_eq!(var, PsxVariable::FcuHdg);
        assert!((val - 360.0).abs() < 0.01);
    }

    // ── PsxVariable ─────────────────────────────────────────────────────────

    #[test]
    fn variable_round_trip_id() {
        let vars = [
            PsxVariable::FcuSpd,
            PsxVariable::FcuHdg,
            PsxVariable::N1Left,
            PsxVariable::N1Right,
            PsxVariable::FuelLeft,
            PsxVariable::FuelRight,
            PsxVariable::GearDown,
        ];
        for v in vars {
            let id = v.id().expect("known variable must have an ID");
            assert_eq!(PsxVariable::from_id(id), v);
        }
    }

    #[test]
    fn unknown_variable_has_no_id() {
        let v = PsxVariable::Unknown("Qi9999".to_owned());
        assert_eq!(v.id(), None);
    }

    // ── PsxTelemetry ────────────────────────────────────────────────────────

    #[test]
    fn telemetry_default_values() {
        let t = PsxTelemetry::default();
        assert_eq!(t.fcu_spd, 0.0);
        assert_eq!(t.fcu_hdg, 0.0);
        assert_eq!(t.n1_left, 0.0);
        assert_eq!(t.n1_right, 0.0);
        assert_eq!(t.fuel_left, 0.0);
        assert_eq!(t.fuel_right, 0.0);
        assert!(!t.gear_down);
    }

    #[test]
    fn telemetry_apply_known_vars() {
        let mut t = PsxTelemetry::default();
        t.apply(&PsxVariable::FcuSpd, 250.0);
        t.apply(&PsxVariable::N1Left, 90.0);
        t.apply(&PsxVariable::GearDown, 1.0);
        assert!((t.fcu_spd - 250.0).abs() < 0.01);
        assert!((t.n1_left - 90.0).abs() < 0.01);
        assert!(t.gear_down);
    }

    #[test]
    fn telemetry_gear_down_threshold() {
        let mut t = PsxTelemetry::default();
        t.apply(&PsxVariable::GearDown, 0.4);
        assert!(!t.gear_down);
        t.apply(&PsxVariable::GearDown, 0.5);
        assert!(t.gear_down);
    }

    #[test]
    fn telemetry_apply_unknown_var_no_op() {
        let mut t = PsxTelemetry::default();
        t.apply(&PsxVariable::Unknown("Qi9999".to_owned()), 999.0);
        // Default state must be unchanged
        assert_eq!(t, PsxTelemetry::default());
    }

    // ── PsxAdapter ───────────────────────────────────────────────────────────

    #[test]
    fn adapter_default_port() {
        let adapter = PsxAdapter::default();
        assert_eq!(adapter.port, PSX_DEFAULT_PORT);
    }

    #[test]
    fn adapter_custom_port() {
        let adapter = PsxAdapter::with_port(9000);
        assert_eq!(adapter.port, 9000);
    }

    #[test]
    fn adapter_process_line_updates_telemetry() {
        let mut adapter = PsxAdapter::new();
        adapter.process_line("Qi0001=320.0").unwrap();
        assert!((adapter.telemetry().fcu_spd - 320.0).abs() < 0.01);
    }

    #[test]
    fn adapter_process_invalid_line_returns_error() {
        let mut adapter = PsxAdapter::new();
        let result = adapter.process_line("no-separator-here");
        assert!(result.is_err());
    }

    #[test]
    fn adapter_accumulates_multiple_lines() {
        let mut adapter = PsxAdapter::new();
        adapter.process_line("Qi0010=85.0").unwrap();
        adapter.process_line("Qi0011=84.5").unwrap();
        adapter.process_line("Qi0100=45000.0").unwrap();
        let t = adapter.telemetry();
        assert!((t.n1_left - 85.0).abs() < 0.01);
        assert!((t.n1_right - 84.5).abs() < 0.01);
        assert!((t.fuel_left - 45_000.0).abs() < 0.01);
    }
}
