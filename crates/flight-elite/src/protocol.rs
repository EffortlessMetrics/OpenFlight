// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Protocol types for Elite: Dangerous `Status.json` and journal events.

use serde::{Deserialize, Serialize};

/// Elite Dangerous status flags (bit positions).
///
/// These are the flags written to `Status.json::Flags`.
/// Reference: <https://elite-journal.readthedocs.io/en/latest/Status%20File/>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EliteFlags(u64);

impl EliteFlags {
    pub const DOCKED: EliteFlags = EliteFlags(1 << 0);
    pub const LANDED: EliteFlags = EliteFlags(1 << 1);
    pub const GEAR_DOWN: EliteFlags = EliteFlags(1 << 2);
    pub const SHIELDS_UP: EliteFlags = EliteFlags(1 << 3);
    pub const SUPERCRUISE: EliteFlags = EliteFlags(1 << 4);
    pub const FLIGHT_ASSIST_OFF: EliteFlags = EliteFlags(1 << 5);
    pub const HARDPOINTS_DEPLOYED: EliteFlags = EliteFlags(1 << 6);
    pub const IN_WING: EliteFlags = EliteFlags(1 << 7);
    pub const LIGHTS_ON: EliteFlags = EliteFlags(1 << 8);
    pub const CARGO_SCOOP: EliteFlags = EliteFlags(1 << 9);
    pub const SILENT_RUNNING: EliteFlags = EliteFlags(1 << 10);
    pub const SCOOPING_FUEL: EliteFlags = EliteFlags(1 << 11);
    pub const IN_SRV: EliteFlags = EliteFlags(1 << 16);
    pub const FSD_JUMP: EliteFlags = EliteFlags(1 << 28);

    pub fn from_bits_truncate(bits: u64) -> Self {
        EliteFlags(bits)
    }

    pub fn bits(&self) -> u64 {
        self.0
    }

    pub fn contains(&self, other: EliteFlags) -> bool {
        self.0 & other.0 != 0
    }
}

/// Fuel quantities reported in `Status.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuelStatus {
    /// Main fuel tank level in tonnes.
    #[serde(rename = "FuelMain")]
    pub fuel_main: f32,
    /// Reserve fuel tank level in tonnes.
    #[serde(rename = "FuelReservoir")]
    pub fuel_reservoir: f32,
}

/// Parsed `Status.json` file written by Elite Dangerous every ~250 ms.
///
/// Only a subset of fields is read; unknown fields are ignored by serde.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StatusJson {
    /// Schema version (e.g. `"4"`).
    #[serde(rename = "SchemaVersion", default)]
    pub schema_version: Option<u32>,

    /// Event type (always `"Status"`).
    #[serde(rename = "event", default)]
    pub event: Option<String>,

    /// Status flags bitmask.
    #[serde(rename = "Flags", default)]
    pub flags: u64,

    /// SysPanel pips [System, Engine, Weapons].
    #[serde(rename = "Pips", default)]
    pub pips: Option<[u8; 3]>,

    /// Fire group index.
    #[serde(rename = "FireGroup", default)]
    pub fire_group: Option<u32>,

    /// GUI focus state (0 = no focus, others = various panels).
    #[serde(rename = "GuiFocus", default)]
    pub gui_focus: Option<u32>,

    /// Fuel quantities.
    #[serde(rename = "Fuel", default)]
    pub fuel: Option<FuelStatus>,

    /// Cargo quantity (tonnes).
    #[serde(rename = "Cargo", default)]
    pub cargo: Option<f32>,

    /// Legal state string (e.g. `"Clean"`, `"Wanted"`).
    #[serde(rename = "LegalState", default)]
    pub legal_state: Option<String>,
}

/// Journal event types relevant to the Flight Hub bus.
///
/// Journal files are JSONL (one JSON object per line). We only parse the
/// events we care about; unknown events are represented as `Unknown`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "PascalCase")]
pub enum JournalEvent {
    /// Commander loaded a game / switched ship.
    LoadGame {
        #[serde(rename = "Ship")]
        ship: String,
        #[serde(rename = "Commander")]
        commander: Option<String>,
    },
    /// Player is in a star system (spawned / initial location event).
    Location {
        #[serde(rename = "StarSystem")]
        star_system: String,
        /// 3D galactic coordinates [x, y, z] in light-years.
        #[serde(rename = "StarPos", default)]
        star_pos: Option<[f64; 3]>,
    },
    /// Docked at a station or surface port.
    Docked {
        #[serde(rename = "StationName")]
        station_name: String,
        #[serde(rename = "StarSystem")]
        star_system: String,
    },
    /// Undocked from a station.
    Undocked {
        #[serde(rename = "StationName")]
        station_name: String,
    },
    /// FSD jump completed (arrived in new system).
    FsdJump {
        #[serde(rename = "StarSystem")]
        star_system: String,
        /// 3D galactic coordinates [x, y, z] in light-years.
        #[serde(rename = "StarPos", default)]
        star_pos: Option<[f64; 3]>,
    },
    /// Landing on a planetary body.
    Touchdown {
        #[serde(rename = "Latitude")]
        latitude: Option<f64>,
        #[serde(rename = "Longitude")]
        longitude: Option<f64>,
    },
    /// Lifted off from a planetary body.
    Liftoff {
        #[serde(rename = "Latitude")]
        latitude: Option<f64>,
        #[serde(rename = "Longitude")]
        longitude: Option<f64>,
    },
    /// Refuelled at a station.
    RefuelAll {
        /// Fuel amount purchased (tonnes).
        #[serde(rename = "Amount")]
        amount: Option<f32>,
    },
}

/// Attempt to parse a single line from a journal file.
///
/// Returns `None` if the event type is unknown or the line cannot be parsed.
pub fn parse_journal_line(line: &str) -> Option<JournalEvent> {
    serde_json::from_str(line).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_json() {
        let raw = r#"{
            "timestamp":"2025-01-01T00:00:00Z",
            "event":"Status",
            "Flags":274877906948,
            "Pips":[4,4,4],
            "FireGroup":0,
            "GuiFocus":0,
            "Fuel":{"FuelMain":16.0,"FuelReservoir":0.57},
            "Cargo":0.0
        }"#;
        let s: StatusJson = serde_json::from_str(raw).expect("should parse");
        assert_eq!(s.flags, 274_877_906_948);
        assert!(s.fuel.is_some());
        assert!((s.fuel.unwrap().fuel_main - 16.0).abs() < 0.01);
    }

    #[test]
    fn elite_flags_gear_down() {
        let f = EliteFlags::from_bits_truncate(EliteFlags::GEAR_DOWN.bits());
        assert!(f.contains(EliteFlags::GEAR_DOWN));
        assert!(!f.contains(EliteFlags::DOCKED));
    }

    #[test]
    fn parses_load_game_event() {
        let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"LoadGame","Commander":"CMDR Test","Ship":"SideWinder"}"#;
        match parse_journal_line(line) {
            Some(JournalEvent::LoadGame { ship, .. }) => assert_eq!(ship, "SideWinder"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parses_location_event_with_star_pos() {
        let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Location","StarSystem":"Sol","StarPos":[0.0,0.0,0.0]}"#;
        match parse_journal_line(line) {
            Some(JournalEvent::Location { star_system, star_pos }) => {
                assert_eq!(star_system, "Sol");
                assert_eq!(star_pos, Some([0.0, 0.0, 0.0]));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parses_fsd_jump_event_with_star_pos() {
        let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"FsdJump","StarSystem":"HIP 78085","StarPos":[10.5,-20.3,5.0]}"#;
        match parse_journal_line(line) {
            Some(JournalEvent::FsdJump { star_system, star_pos }) => {
                assert_eq!(star_system, "HIP 78085");
                let pos = star_pos.unwrap();
                assert!((pos[0] - 10.5).abs() < 0.01);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parses_refuel_all_event() {
        let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"RefuelAll","Amount":12.5,"Cost":1250}"#;
        match parse_journal_line(line) {
            Some(JournalEvent::RefuelAll { amount }) => {
                assert!((amount.unwrap() - 12.5).abs() < 0.01);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn unknown_event_returns_none() {
        let line = r#"{"timestamp":"2025-01-01T00:00:00Z","event":"Music","MusicTrack":"MainMenu"}"#;
        assert!(parse_journal_line(line).is_none());
    }
}
