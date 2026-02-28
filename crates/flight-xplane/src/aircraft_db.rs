// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X-Plane aircraft database
//!
//! Static registry of well-known X-Plane aircraft with metadata used for
//! profile selection, dataref mapping, and categorisation.

use std::collections::HashMap;

/// Category of an X-Plane aircraft (mirrors `aircraft::AircraftType` but
/// decoupled so the database can be used independently).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AircraftCategory {
    SinglePiston,
    TwinPiston,
    Turboprop,
    LightJet,
    AirlinerNarrowBody,
    AirlinerWideBody,
    RegionalJet,
    MilitaryJet,
    Helicopter,
    Glider,
}

/// Static metadata for a known X-Plane aircraft.
#[derive(Debug, Clone, PartialEq)]
pub struct XPlaneAircraftEntry {
    /// Relative `.acf` file path (e.g. `"Aircraft/Laminar Research/Cessna 172SP/…"`).
    pub acf_path: &'static str,
    /// Human-readable name shown in UI.
    pub display_name: &'static str,
    /// Category for automatic profile selection.
    pub category: AircraftCategory,
    /// Name of the default profile to apply.
    pub default_profile: &'static str,
    /// Optional add-on datarefs specific to this aircraft.
    pub custom_datarefs: &'static [&'static str],
}

/// In-memory lookup of known X-Plane aircraft keyed by `.acf` path.
#[derive(Debug, Clone)]
pub struct AircraftDatabase {
    entries: HashMap<&'static str, XPlaneAircraftEntry>,
}

impl AircraftDatabase {
    /// Create the database pre-populated with 20+ well-known aircraft.
    pub fn new() -> Self {
        let aircraft: Vec<XPlaneAircraftEntry> = vec![
            // ── Single-engine piston ────────────────────────────────
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Laminar Research/Cessna 172SP/Cessna_172SP.acf",
                display_name: "Cessna 172SP Skyhawk",
                category: AircraftCategory::SinglePiston,
                default_profile: "ga-single-piston",
                custom_datarefs: &[],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Laminar Research/Cessna 172SP/Cessna_172SP_G1000.acf",
                display_name: "Cessna 172SP G1000",
                category: AircraftCategory::SinglePiston,
                default_profile: "ga-single-piston",
                custom_datarefs: &[],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Cirrus/SF50/SF50.acf",
                display_name: "Cirrus SF50 Vision Jet",
                category: AircraftCategory::LightJet,
                default_profile: "light-jet",
                custom_datarefs: &["cirrus/sf50/aoa_indicator"],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Cirrus/SR22/SR22.acf",
                display_name: "Cirrus SR22",
                category: AircraftCategory::SinglePiston,
                default_profile: "ga-single-piston",
                custom_datarefs: &["cirrus/sr22/percent_power"],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Piper/PA-28/PA28.acf",
                display_name: "Piper PA-28 Cherokee",
                category: AircraftCategory::SinglePiston,
                default_profile: "ga-single-piston",
                custom_datarefs: &[],
            },
            // ── Twin piston ─────────────────────────────────────────
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Beechcraft/Baron B58/Baron_58.acf",
                display_name: "Beechcraft Baron B58",
                category: AircraftCategory::TwinPiston,
                default_profile: "ga-twin-piston",
                custom_datarefs: &[],
            },
            // ── Turboprop ───────────────────────────────────────────
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Laminar Research/King Air C90B/KingAir_C90B.acf",
                display_name: "Beechcraft King Air C90B",
                category: AircraftCategory::Turboprop,
                default_profile: "turboprop-twin",
                custom_datarefs: &[],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Pilatus/PC-12/PC12.acf",
                display_name: "Pilatus PC-12",
                category: AircraftCategory::Turboprop,
                default_profile: "turboprop-single",
                custom_datarefs: &[],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/DAHER/TBM 900/TBM900.acf",
                display_name: "DAHER TBM 900",
                category: AircraftCategory::Turboprop,
                default_profile: "turboprop-single",
                custom_datarefs: &["tbm900/ecs/bleed_air_psi"],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/ATR/ATR 72-600/ATR72.acf",
                display_name: "ATR 72-600",
                category: AircraftCategory::Turboprop,
                default_profile: "turboprop-twin",
                custom_datarefs: &[],
            },
            // ── Narrow-body airliners ────────────────────────────────
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Laminar Research/Boeing 737-800/b738.acf",
                display_name: "Boeing 737-800",
                category: AircraftCategory::AirlinerNarrowBody,
                default_profile: "airliner-narrow",
                custom_datarefs: &[],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/FlightFactor/A320/A320.acf",
                display_name: "Airbus A320",
                category: AircraftCategory::AirlinerNarrowBody,
                default_profile: "airliner-narrow",
                custom_datarefs: &["a320/fcu/altitude", "a320/fcu/heading"],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/FlightFactor/A321/A321.acf",
                display_name: "Airbus A321",
                category: AircraftCategory::AirlinerNarrowBody,
                default_profile: "airliner-narrow",
                custom_datarefs: &[],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Boeing/757-200/B752.acf",
                display_name: "Boeing 757-200",
                category: AircraftCategory::AirlinerNarrowBody,
                default_profile: "airliner-narrow",
                custom_datarefs: &[],
            },
            // ── Wide-body airliners ─────────────────────────────────
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Airbus/A330-300/A333.acf",
                display_name: "Airbus A330-300",
                category: AircraftCategory::AirlinerWideBody,
                default_profile: "airliner-wide",
                custom_datarefs: &[],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Boeing/747-400/B744.acf",
                display_name: "Boeing 747-400",
                category: AircraftCategory::AirlinerWideBody,
                default_profile: "airliner-wide",
                custom_datarefs: &[],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Boeing/777-200LR/B77L.acf",
                display_name: "Boeing 777-200LR",
                category: AircraftCategory::AirlinerWideBody,
                default_profile: "airliner-wide",
                custom_datarefs: &[],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Boeing/787-9/B789.acf",
                display_name: "Boeing 787-9 Dreamliner",
                category: AircraftCategory::AirlinerWideBody,
                default_profile: "airliner-wide",
                custom_datarefs: &[],
            },
            // ── Regional jets ───────────────────────────────────────
            XPlaneAircraftEntry {
                acf_path: "Aircraft/CRJ/CRJ-200/CRJ2.acf",
                display_name: "Bombardier CRJ-200",
                category: AircraftCategory::RegionalJet,
                default_profile: "regional-jet",
                custom_datarefs: &[],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Embraer/E175/E175.acf",
                display_name: "Embraer E175",
                category: AircraftCategory::RegionalJet,
                default_profile: "regional-jet",
                custom_datarefs: &[],
            },
            // ── Military jets ───────────────────────────────────────
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Military/F-14/F14.acf",
                display_name: "Grumman F-14 Tomcat",
                category: AircraftCategory::MilitaryJet,
                default_profile: "military-fighter",
                custom_datarefs: &[],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Military/FA-18C/FA18C.acf",
                display_name: "Boeing F/A-18C Hornet",
                category: AircraftCategory::MilitaryJet,
                default_profile: "military-fighter",
                custom_datarefs: &[],
            },
            // ── Helicopters ─────────────────────────────────────────
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Laminar Research/Bell 206/Bell206.acf",
                display_name: "Bell 206 JetRanger",
                category: AircraftCategory::Helicopter,
                default_profile: "helicopter-light",
                custom_datarefs: &[],
            },
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Sikorsky/S-76/S76C.acf",
                display_name: "Sikorsky S-76C",
                category: AircraftCategory::Helicopter,
                default_profile: "helicopter-medium",
                custom_datarefs: &[],
            },
            // ── Gliders ─────────────────────────────────────────────
            XPlaneAircraftEntry {
                acf_path: "Aircraft/Gliders/ASK-21/ASK21.acf",
                display_name: "Schleicher ASK 21",
                category: AircraftCategory::Glider,
                default_profile: "glider",
                custom_datarefs: &[],
            },
        ];

        let mut entries = HashMap::with_capacity(aircraft.len());
        for a in aircraft {
            entries.insert(a.acf_path, a);
        }

        Self { entries }
    }

    /// Look up an aircraft by its `.acf` file path.
    pub fn get(&self, acf_path: &str) -> Option<&XPlaneAircraftEntry> {
        self.entries.get(acf_path)
    }

    /// Return all aircraft that match a given category.
    pub fn by_category(&self, category: AircraftCategory) -> Vec<&XPlaneAircraftEntry> {
        self.entries
            .values()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Total number of aircraft in the database.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the database is empty (always false after construction).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return every entry in the database.
    pub fn all(&self) -> Vec<&XPlaneAircraftEntry> {
        self.entries.values().collect()
    }
}

impl Default for AircraftDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_has_at_least_20_aircraft() {
        let db = AircraftDatabase::new();
        assert!(db.len() >= 20, "expected >=20 aircraft, got {}", db.len());
    }

    #[test]
    fn test_lookup_cessna_172sp() {
        let db = AircraftDatabase::new();
        let entry = db
            .get("Aircraft/Laminar Research/Cessna 172SP/Cessna_172SP.acf")
            .unwrap();
        assert_eq!(entry.display_name, "Cessna 172SP Skyhawk");
        assert_eq!(entry.category, AircraftCategory::SinglePiston);
    }

    #[test]
    fn test_lookup_missing_returns_none() {
        let db = AircraftDatabase::new();
        assert!(db.get("Aircraft/DoesNotExist.acf").is_none());
    }

    #[test]
    fn test_by_category_turboprop() {
        let db = AircraftDatabase::new();
        let tp = db.by_category(AircraftCategory::Turboprop);
        assert!(tp.len() >= 3, "expected >=3 turboprops, got {}", tp.len());
    }

    #[test]
    fn test_by_category_helicopter() {
        let db = AircraftDatabase::new();
        let h = db.by_category(AircraftCategory::Helicopter);
        assert!(h.len() >= 2, "expected >=2 helicopters, got {}", h.len());
    }

    #[test]
    fn test_is_not_empty() {
        let db = AircraftDatabase::new();
        assert!(!db.is_empty());
    }

    #[test]
    fn test_all_returns_correct_count() {
        let db = AircraftDatabase::new();
        assert_eq!(db.all().len(), db.len());
    }

    #[test]
    fn test_default_matches_new() {
        let a = AircraftDatabase::new();
        let b = AircraftDatabase::default();
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn test_custom_datarefs_populated() {
        let db = AircraftDatabase::new();
        let sf50 = db.get("Aircraft/Cirrus/SF50/SF50.acf").unwrap();
        assert!(!sf50.custom_datarefs.is_empty());
        assert!(sf50.custom_datarefs.contains(&"cirrus/sf50/aoa_indicator"));
    }

    #[test]
    fn test_every_entry_has_display_name() {
        let db = AircraftDatabase::new();
        for entry in db.all() {
            assert!(
                !entry.display_name.is_empty(),
                "missing display name for {}",
                entry.acf_path
            );
        }
    }

    #[test]
    fn test_every_entry_has_default_profile() {
        let db = AircraftDatabase::new();
        for entry in db.all() {
            assert!(
                !entry.default_profile.is_empty(),
                "missing default_profile for {}",
                entry.acf_path,
            );
        }
    }
}
