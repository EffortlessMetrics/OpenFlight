// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! MSFS 2024 aircraft database
//!
//! Provides a curated catalog of default aircraft shipped with MSFS 2024,
//! including ICAO codes, aircraft types, and suggested default profiles.
//! Used by the adapter layer to select appropriate axis/panel configurations
//! when an aircraft is detected via SimConnect.

use std::collections::HashMap;

/// Classification of aircraft by airframe and propulsion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AircraftType {
    SingleProp,
    TwinProp,
    Turboprop,
    SingleJet,
    TwinJet,
    Helicopter,
    Glider,
}

/// Information about a specific MSFS default aircraft.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MsfsAircraftInfo {
    /// ICAO type designator (e.g. `"C172"`).
    pub icao_code: &'static str,
    /// Human-readable display name.
    pub display_name: &'static str,
    /// Airframe / propulsion class.
    pub category: AircraftType,
    /// Suggested default profile key.
    pub default_profile: &'static str,
    /// SimVar names that are unique or particularly important for this type.
    pub special_vars: Vec<&'static str>,
}

/// Database of MSFS 2024 default aircraft.
pub struct MsfsAircraftDb {
    aircraft: HashMap<&'static str, MsfsAircraftInfo>,
}

impl MsfsAircraftDb {
    /// Create a new database pre-populated with MSFS 2024 default aircraft.
    pub fn new() -> Self {
        let entries: Vec<MsfsAircraftInfo> = vec![
            // ── Single-engine piston ─────────────────────────────────
            MsfsAircraftInfo {
                icao_code: "C172",
                display_name: "Cessna 172 Skyhawk",
                category: AircraftType::SingleProp,
                default_profile: "ga-single-piston",
                special_vars: vec!["GENERAL ENG MIXTURE LEVER POSITION:1"],
            },
            MsfsAircraftInfo {
                icao_code: "C152",
                display_name: "Cessna 152",
                category: AircraftType::SingleProp,
                default_profile: "ga-single-piston",
                special_vars: vec!["GENERAL ENG MIXTURE LEVER POSITION:1"],
            },
            MsfsAircraftInfo {
                icao_code: "DA40",
                display_name: "Diamond DA40 NG",
                category: AircraftType::SingleProp,
                default_profile: "ga-single-piston",
                special_vars: vec!["GENERAL ENG MIXTURE LEVER POSITION:1"],
            },
            MsfsAircraftInfo {
                icao_code: "DA62",
                display_name: "Diamond DA62",
                category: AircraftType::TwinProp,
                default_profile: "ga-twin-piston",
                special_vars: vec![
                    "GENERAL ENG MIXTURE LEVER POSITION:1",
                    "GENERAL ENG MIXTURE LEVER POSITION:2",
                ],
            },
            MsfsAircraftInfo {
                icao_code: "SR22",
                display_name: "Cirrus SR22",
                category: AircraftType::SingleProp,
                default_profile: "ga-single-piston",
                special_vars: vec!["GENERAL ENG MIXTURE LEVER POSITION:1"],
            },
            MsfsAircraftInfo {
                icao_code: "PA28",
                display_name: "Piper PA-28 Cherokee",
                category: AircraftType::SingleProp,
                default_profile: "ga-single-piston",
                special_vars: vec!["GENERAL ENG MIXTURE LEVER POSITION:1"],
            },
            MsfsAircraftInfo {
                icao_code: "C208",
                display_name: "Cessna 208B Grand Caravan EX",
                category: AircraftType::Turboprop,
                default_profile: "ga-turboprop",
                special_vars: vec!["PROP RPM:1", "ENG TORQUE PERCENT:1"],
            },
            // ── Twin-engine piston ───────────────────────────────────
            MsfsAircraftInfo {
                icao_code: "BE58",
                display_name: "Beechcraft Baron G58",
                category: AircraftType::TwinProp,
                default_profile: "ga-twin-piston",
                special_vars: vec![
                    "GENERAL ENG MIXTURE LEVER POSITION:1",
                    "GENERAL ENG MIXTURE LEVER POSITION:2",
                ],
            },
            // ── Turboprops ───────────────────────────────────────────
            MsfsAircraftInfo {
                icao_code: "TBM9",
                display_name: "Daher TBM 930",
                category: AircraftType::Turboprop,
                default_profile: "ga-turboprop",
                special_vars: vec!["PROP RPM:1", "ENG TORQUE PERCENT:1", "ENG ITT:1"],
            },
            MsfsAircraftInfo {
                icao_code: "PC12",
                display_name: "Pilatus PC-12 NGX",
                category: AircraftType::Turboprop,
                default_profile: "ga-turboprop",
                special_vars: vec!["PROP RPM:1", "ENG TORQUE PERCENT:1"],
            },
            MsfsAircraftInfo {
                icao_code: "BE20",
                display_name: "Beechcraft King Air 350i",
                category: AircraftType::Turboprop,
                default_profile: "ga-turboprop-twin",
                special_vars: vec![
                    "PROP RPM:1",
                    "PROP RPM:2",
                    "ENG TORQUE PERCENT:1",
                    "ENG TORQUE PERCENT:2",
                ],
            },
            MsfsAircraftInfo {
                icao_code: "DHC6",
                display_name: "de Havilland DHC-6 Twin Otter",
                category: AircraftType::Turboprop,
                default_profile: "ga-turboprop-twin",
                special_vars: vec!["PROP RPM:1", "PROP RPM:2"],
            },
            // ── Single-engine jet (light jets) ───────────────────────
            MsfsAircraftInfo {
                icao_code: "E50P",
                display_name: "Eclipse 550",
                category: AircraftType::SingleJet,
                default_profile: "jet-light",
                special_vars: vec!["ENG N1 RPM:1"],
            },
            // ── Twin-engine jets ─────────────────────────────────────
            MsfsAircraftInfo {
                icao_code: "C525",
                display_name: "Cessna Citation CJ4",
                category: AircraftType::TwinJet,
                default_profile: "jet-light",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "C700",
                display_name: "Cessna Citation Longitude",
                category: AircraftType::TwinJet,
                default_profile: "jet-light",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "A320",
                display_name: "Airbus A320neo",
                category: AircraftType::TwinJet,
                default_profile: "airliner-narrowbody",
                special_vars: vec![
                    "ENG N1 RPM:1",
                    "ENG N1 RPM:2",
                    "FLY BY WIRE ALPHA PROTECTION",
                ],
            },
            MsfsAircraftInfo {
                icao_code: "A310",
                display_name: "Airbus A310-300",
                category: AircraftType::TwinJet,
                default_profile: "airliner-widebody",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "B748",
                display_name: "Boeing 747-8 Intercontinental",
                category: AircraftType::TwinJet,
                default_profile: "airliner-widebody",
                special_vars: vec![
                    "ENG N1 RPM:1",
                    "ENG N1 RPM:2",
                    "ENG N1 RPM:3",
                    "ENG N1 RPM:4",
                ],
            },
            MsfsAircraftInfo {
                icao_code: "B78X",
                display_name: "Boeing 787-10 Dreamliner",
                category: AircraftType::TwinJet,
                default_profile: "airliner-widebody",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "B350",
                display_name: "Beechcraft King Air 350i",
                category: AircraftType::Turboprop,
                default_profile: "ga-turboprop-twin",
                special_vars: vec!["PROP RPM:1", "PROP RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "F18S",
                display_name: "Boeing F/A-18E Super Hornet",
                category: AircraftType::TwinJet,
                default_profile: "military-jet",
                special_vars: vec![
                    "ENG N1 RPM:1",
                    "ENG N1 RPM:2",
                    "FOLDING WING HANDLE POSITION",
                ],
            },
            // ── Helicopters ──────────────────────────────────────────
            MsfsAircraftInfo {
                icao_code: "B06",
                display_name: "Bell 206B JetRanger",
                category: AircraftType::Helicopter,
                default_profile: "helicopter-light",
                special_vars: vec!["ROTOR RPM:1", "ENG TORQUE PERCENT:1"],
            },
            MsfsAircraftInfo {
                icao_code: "H135",
                display_name: "Airbus H135",
                category: AircraftType::Helicopter,
                default_profile: "helicopter-medium",
                special_vars: vec![
                    "ROTOR RPM:1",
                    "ENG TORQUE PERCENT:1",
                    "ENG TORQUE PERCENT:2",
                ],
            },
            MsfsAircraftInfo {
                icao_code: "R44",
                display_name: "Robinson R44 Raven II",
                category: AircraftType::Helicopter,
                default_profile: "helicopter-light",
                special_vars: vec!["ROTOR RPM:1"],
            },
            MsfsAircraftInfo {
                icao_code: "S76",
                display_name: "Sikorsky S-76C",
                category: AircraftType::Helicopter,
                default_profile: "helicopter-medium",
                special_vars: vec![
                    "ROTOR RPM:1",
                    "ENG TORQUE PERCENT:1",
                    "ENG TORQUE PERCENT:2",
                ],
            },
            MsfsAircraftInfo {
                icao_code: "EC45",
                display_name: "Airbus H145",
                category: AircraftType::Helicopter,
                default_profile: "helicopter-medium",
                special_vars: vec![
                    "ROTOR RPM:1",
                    "ENG TORQUE PERCENT:1",
                    "ENG TORQUE PERCENT:2",
                ],
            },
            // ── Gliders ──────────────────────────────────────────────
            MsfsAircraftInfo {
                icao_code: "DG1T",
                display_name: "DG Flugzeugbau DG-1001e neo",
                category: AircraftType::Glider,
                default_profile: "glider",
                special_vars: vec!["TOTAL WEIGHT", "VARIOMETER RATE"],
            },
            // ── Popular third-party add-ons ──────────────────────────
            MsfsAircraftInfo {
                icao_code: "B738",
                display_name: "PMDG 737-800",
                category: AircraftType::TwinJet,
                default_profile: "airliner-narrowbody",
                special_vars: vec![
                    "ENG N1 RPM:1",
                    "ENG N1 RPM:2",
                    "TURB ENG CORRECTED N1:1",
                    "TURB ENG CORRECTED N1:2",
                ],
            },
            MsfsAircraftInfo {
                icao_code: "B739",
                display_name: "PMDG 737-900",
                category: AircraftType::TwinJet,
                default_profile: "airliner-narrowbody",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "B737",
                display_name: "PMDG 737-700",
                category: AircraftType::TwinJet,
                default_profile: "airliner-narrowbody",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "B77W",
                display_name: "PMDG 777-300ER",
                category: AircraftType::TwinJet,
                default_profile: "airliner-widebody",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "B77L",
                display_name: "PMDG 777-200LR",
                category: AircraftType::TwinJet,
                default_profile: "airliner-widebody",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "A20N",
                display_name: "FlyByWire A320neo",
                category: AircraftType::TwinJet,
                default_profile: "airliner-narrowbody",
                special_vars: vec![
                    "ENG N1 RPM:1",
                    "ENG N1 RPM:2",
                    "FLY BY WIRE ALPHA PROTECTION",
                ],
            },
            MsfsAircraftInfo {
                icao_code: "A319",
                display_name: "Fenix A319",
                category: AircraftType::TwinJet,
                default_profile: "airliner-narrowbody",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "A321",
                display_name: "Fenix A320/A321",
                category: AircraftType::TwinJet,
                default_profile: "airliner-narrowbody",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "A3ST",
                display_name: "iniBuilds A310-300",
                category: AircraftType::TwinJet,
                default_profile: "airliner-widebody",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "A306",
                display_name: "iniBuilds A300-600R",
                category: AircraftType::TwinJet,
                default_profile: "airliner-widebody",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "CRJ7",
                display_name: "Aerosoft CRJ 700",
                category: AircraftType::TwinJet,
                default_profile: "airliner-regional",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "CRJ9",
                display_name: "Aerosoft CRJ 900",
                category: AircraftType::TwinJet,
                default_profile: "airliner-regional",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "CRJX",
                display_name: "Aerosoft CRJ 1000",
                category: AircraftType::TwinJet,
                default_profile: "airliner-regional",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            MsfsAircraftInfo {
                icao_code: "C25C",
                display_name: "Working Title CJ4",
                category: AircraftType::TwinJet,
                default_profile: "jet-light",
                special_vars: vec!["ENG N1 RPM:1", "ENG N1 RPM:2"],
            },
            // ── Additional MSFS default aircraft ─────────────────────
            MsfsAircraftInfo {
                icao_code: "C310",
                display_name: "Cessna 310R",
                category: AircraftType::TwinProp,
                default_profile: "ga-twin-piston",
                special_vars: vec![
                    "GENERAL ENG MIXTURE LEVER POSITION:1",
                    "GENERAL ENG MIXTURE LEVER POSITION:2",
                ],
            },
            MsfsAircraftInfo {
                icao_code: "C140",
                display_name: "Cessna 140",
                category: AircraftType::SingleProp,
                default_profile: "ga-single-piston",
                special_vars: vec!["GENERAL ENG MIXTURE LEVER POSITION:1"],
            },
            MsfsAircraftInfo {
                icao_code: "RV7",
                display_name: "Van's RV-7",
                category: AircraftType::SingleProp,
                default_profile: "ga-single-piston",
                special_vars: vec!["GENERAL ENG MIXTURE LEVER POSITION:1"],
            },
            MsfsAircraftInfo {
                icao_code: "P28A",
                display_name: "Piper PA-28R Arrow",
                category: AircraftType::SingleProp,
                default_profile: "ga-single-piston",
                special_vars: vec!["GENERAL ENG MIXTURE LEVER POSITION:1"],
            },
            MsfsAircraftInfo {
                icao_code: "P38",
                display_name: "P-38 Lightning",
                category: AircraftType::TwinProp,
                default_profile: "ga-twin-piston",
                special_vars: vec![
                    "GENERAL ENG MIXTURE LEVER POSITION:1",
                    "GENERAL ENG MIXTURE LEVER POSITION:2",
                ],
            },
            MsfsAircraftInfo {
                icao_code: "DC3",
                display_name: "Douglas DC-3",
                category: AircraftType::TwinProp,
                default_profile: "ga-twin-piston",
                special_vars: vec![
                    "GENERAL ENG MIXTURE LEVER POSITION:1",
                    "GENERAL ENG MIXTURE LEVER POSITION:2",
                ],
            },
            MsfsAircraftInfo {
                icao_code: "SF50",
                display_name: "Cirrus Vision Jet SF50",
                category: AircraftType::SingleJet,
                default_profile: "jet-light",
                special_vars: vec!["ENG N1 RPM:1"],
            },
        ];

        let mut aircraft = HashMap::with_capacity(entries.len());
        for info in entries {
            aircraft.insert(info.icao_code, info);
        }

        Self { aircraft }
    }

    /// Look up an aircraft by ICAO code.
    pub fn get(&self, icao_code: &str) -> Option<&MsfsAircraftInfo> {
        self.aircraft.get(icao_code)
    }

    /// Return all aircraft of a given type.
    pub fn by_type(&self, aircraft_type: AircraftType) -> Vec<&MsfsAircraftInfo> {
        self.aircraft
            .values()
            .filter(|a| a.category == aircraft_type)
            .collect()
    }

    /// Return all ICAO codes in the database.
    pub fn all_icao_codes(&self) -> Vec<&'static str> {
        self.aircraft.keys().copied().collect()
    }

    /// Check whether an ICAO code exists in the database.
    pub fn contains(&self, icao_code: &str) -> bool {
        self.aircraft.contains_key(icao_code)
    }

    /// Return every aircraft entry.
    pub fn all(&self) -> Vec<&MsfsAircraftInfo> {
        self.aircraft.values().collect()
    }

    /// Total number of aircraft in the database.
    pub fn len(&self) -> usize {
        self.aircraft.len()
    }

    /// Returns `true` when the database is empty.
    pub fn is_empty(&self) -> bool {
        self.aircraft.is_empty()
    }
}

impl Default for MsfsAircraftDb {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_has_at_least_40_aircraft() {
        let db = MsfsAircraftDb::new();
        assert!(db.len() >= 40, "need ≥40 aircraft, got {}", db.len());
    }

    #[test]
    fn lookup_c172() {
        let db = MsfsAircraftDb::new();
        let c172 = db.get("C172").expect("C172 must be in the database");
        assert_eq!(c172.display_name, "Cessna 172 Skyhawk");
        assert_eq!(c172.category, AircraftType::SingleProp);
    }

    #[test]
    fn lookup_a320() {
        let db = MsfsAircraftDb::new();
        let a320 = db.get("A320").expect("A320 must be in the database");
        assert_eq!(a320.display_name, "Airbus A320neo");
        assert_eq!(a320.category, AircraftType::TwinJet);
        assert_eq!(a320.default_profile, "airliner-narrowbody");
    }

    #[test]
    fn lookup_missing_returns_none() {
        let db = MsfsAircraftDb::new();
        assert!(db.get("XXXX").is_none());
    }

    #[test]
    fn by_type_helicopter() {
        let db = MsfsAircraftDb::new();
        let helis = db.by_type(AircraftType::Helicopter);
        assert!(helis.len() >= 3, "need ≥3 helicopters");
        for h in &helis {
            assert_eq!(h.category, AircraftType::Helicopter);
        }
    }

    #[test]
    fn by_type_glider() {
        let db = MsfsAircraftDb::new();
        let gliders = db.by_type(AircraftType::Glider);
        assert!(!gliders.is_empty(), "must have at least one glider");
    }

    #[test]
    fn contains_checks() {
        let db = MsfsAircraftDb::new();
        assert!(db.contains("C172"));
        assert!(db.contains("B748"));
        assert!(!db.contains("ZZZZ"));
    }

    #[test]
    fn all_icao_codes_complete() {
        let db = MsfsAircraftDb::new();
        let codes = db.all_icao_codes();
        assert_eq!(codes.len(), db.len());
        assert!(codes.contains(&"C172"));
        assert!(codes.contains(&"A320"));
    }

    #[test]
    fn special_vars_populated() {
        let db = MsfsAircraftDb::new();
        for ac in db.all() {
            assert!(
                !ac.special_vars.is_empty(),
                "{} must have at least one special var",
                ac.icao_code
            );
        }
    }

    #[test]
    fn default_matches_new() {
        let a = MsfsAircraftDb::new();
        let b = MsfsAircraftDb::default();
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn every_type_represented() {
        let db = MsfsAircraftDb::new();
        let types = [
            AircraftType::SingleProp,
            AircraftType::TwinProp,
            AircraftType::Turboprop,
            AircraftType::TwinJet,
            AircraftType::Helicopter,
            AircraftType::Glider,
        ];
        for t in types {
            assert!(
                !db.by_type(t).is_empty(),
                "type {t:?} must have at least one aircraft"
            );
        }
    }

    #[test]
    fn lookup_pmdg_737() {
        let db = MsfsAircraftDb::new();
        let b738 = db
            .get("B738")
            .expect("B738 (PMDG 737-800) must be in the database");
        assert_eq!(b738.category, AircraftType::TwinJet);
        assert_eq!(b738.default_profile, "airliner-narrowbody");
    }

    #[test]
    fn lookup_pmdg_777() {
        let db = MsfsAircraftDb::new();
        let b77w = db
            .get("B77W")
            .expect("B77W (PMDG 777-300ER) must be in the database");
        assert_eq!(b77w.category, AircraftType::TwinJet);
        assert_eq!(b77w.default_profile, "airliner-widebody");
    }

    #[test]
    fn lookup_fbw_a320neo() {
        let db = MsfsAircraftDb::new();
        let a20n = db
            .get("A20N")
            .expect("A20N (FlyByWire A320neo) must be in the database");
        assert_eq!(a20n.category, AircraftType::TwinJet);
        assert!(a20n.special_vars.contains(&"FLY BY WIRE ALPHA PROTECTION"));
    }

    #[test]
    fn lookup_aerosoft_crj() {
        let db = MsfsAircraftDb::new();
        assert!(db.contains("CRJ7"), "CRJ7 (Aerosoft CRJ 700) must exist");
        assert!(db.contains("CRJ9"), "CRJ9 (Aerosoft CRJ 900) must exist");
    }

    #[test]
    fn lookup_working_title_cj4() {
        let db = MsfsAircraftDb::new();
        let cj4 = db
            .get("C25C")
            .expect("C25C (Working Title CJ4) must be in the database");
        assert_eq!(cj4.category, AircraftType::TwinJet);
        assert_eq!(cj4.default_profile, "jet-light");
    }

    #[test]
    fn third_party_aircraft_have_special_vars() {
        let db = MsfsAircraftDb::new();
        let third_party_codes = ["B738", "B77W", "A20N", "CRJ7", "C25C", "A319"];
        for code in third_party_codes {
            let ac = db.get(code).unwrap_or_else(|| panic!("{code} must exist"));
            assert!(!ac.special_vars.is_empty(), "{code} must have special vars");
        }
    }

    #[test]
    fn single_jet_type_has_entries() {
        let db = MsfsAircraftDb::new();
        let single_jets = db.by_type(AircraftType::SingleJet);
        assert!(single_jets.len() >= 2, "need ≥2 single-jet aircraft");
    }
}
