// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Aircraft identification from DCS export telemetry.
//!
//! Parses the `aircraft` field from DCS telemetry packets, maps DCS module
//! names to the internal aircraft database, and detects fidelity level and
//! cockpit seat for multi-crew aircraft.

use crate::aircraft_db::{self, AircraftCategory, AxesProfile, DcsAircraftInfo};

// ---------------------------------------------------------------------------
// Fidelity classification
// ---------------------------------------------------------------------------

/// Fidelity level of a DCS module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleFidelity {
    /// Full-fidelity clickable cockpit (e.g. F/A-18C, A-10C II).
    FullFidelity,
    /// Simplified avionics — Flaming Cliffs 3 (FC3) level.
    Fc3,
    /// Community/third-party mod with unknown fidelity.
    Mod,
}

impl std::fmt::Display for ModuleFidelity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleFidelity::FullFidelity => write!(f, "Full Fidelity"),
            ModuleFidelity::Fc3 => write!(f, "FC3"),
            ModuleFidelity::Mod => write!(f, "Mod"),
        }
    }
}

/// FC3-level module names (simplified cockpits).
static FC3_MODULES: &[&str] = &[
    "Su-25T",
    "Su-27",
    "Su-33",
    "MiG-29A",
    "MiG-29S",
    "F-15C",
    "J-11A",
    "Su-25",
];

/// Determine fidelity level for a DCS module name.
pub fn classify_fidelity(dcs_name: &str) -> ModuleFidelity {
    if FC3_MODULES.contains(&dcs_name) {
        return ModuleFidelity::Fc3;
    }
    if aircraft_db::lookup(dcs_name).is_some() {
        return ModuleFidelity::FullFidelity;
    }
    ModuleFidelity::Mod
}

// ---------------------------------------------------------------------------
// Multi-crew / cockpit seat detection
// ---------------------------------------------------------------------------

/// Cockpit seat position in a multi-crew aircraft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CockpitSeat {
    /// Single-seat aircraft (default for most modules).
    Single,
    /// Front seat / pilot in a multi-crew aircraft.
    Front,
    /// Rear seat / weapon systems officer / co-pilot gunner.
    Rear,
}

impl std::fmt::Display for CockpitSeat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CockpitSeat::Single => write!(f, "Single Seat"),
            CockpitSeat::Front => write!(f, "Front Seat"),
            CockpitSeat::Rear => write!(f, "Rear Seat"),
        }
    }
}

/// Multi-crew aircraft module names mapped to their crew role names.
///
/// DCS appends a suffix or uses a separate module name for multi-crew
/// rear seats: e.g. the F-14 RIO slot may report `"F-14B_RIO"`.
static MULTI_CREW_MODULES: &[(&str, &str)] = &[
    ("F-14B", "F-14B"),
    ("F-14A-135-GR", "F-14A-135-GR"),
    ("F-15ESE", "F-15ESE"),
    ("AH-64D_BLK_II", "AH-64D_BLK_II"),
    ("Mi-24P", "Mi-24P"),
    ("L-39C", "L-39C"),
    ("MosquitoFBMkVI", "MosquitoFBMkVI"),
];

/// Returns `true` if the base module supports multi-crew.
pub fn is_multi_crew(dcs_name: &str) -> bool {
    let base = strip_seat_suffix(dcs_name);
    MULTI_CREW_MODULES.iter().any(|(m, _)| *m == base)
}

/// Detect cockpit seat from the raw DCS module name string.
///
/// DCS may report the rear-seat variant with suffixes like `_RIO`, `_WSO`,
/// `_CPG`, `_REAR`, or `_BACK`. If no suffix is found the aircraft is
/// treated as front-seat (or single-seat if it's not multi-crew).
pub fn detect_seat(dcs_name: &str) -> CockpitSeat {
    let upper = dcs_name.to_ascii_uppercase();
    let rear_suffixes = ["_RIO", "_WSO", "_CPG", "_REAR", "_BACK", "_GUNNER"];
    for suffix in &rear_suffixes {
        if upper.ends_with(suffix) {
            return CockpitSeat::Rear;
        }
    }
    let base = strip_seat_suffix(dcs_name);
    if is_multi_crew(base) {
        CockpitSeat::Front
    } else {
        CockpitSeat::Single
    }
}

/// Strip known seat suffixes to obtain the base module name.
fn strip_seat_suffix(name: &str) -> &str {
    let upper = name.to_ascii_uppercase();
    let suffixes = ["_RIO", "_WSO", "_CPG", "_REAR", "_BACK", "_GUNNER"];
    for suffix in &suffixes {
        if upper.ends_with(suffix) {
            return &name[..name.len() - suffix.len()];
        }
    }
    name
}

// ---------------------------------------------------------------------------
// Full detection result
// ---------------------------------------------------------------------------

/// Complete aircraft identification result.
#[derive(Debug, Clone, PartialEq)]
pub struct AircraftDetection {
    /// Raw DCS module name as reported in telemetry.
    pub raw_name: String,
    /// Base module name (seat suffix stripped).
    pub base_name: String,
    /// Matched database entry (if known).
    pub db_info: Option<&'static DcsAircraftInfo>,
    /// Module fidelity level.
    pub fidelity: ModuleFidelity,
    /// Cockpit seat.
    pub seat: CockpitSeat,
    /// Whether the module supports multiple crew positions.
    pub multi_crew: bool,
}

/// Identify an aircraft from a raw DCS module name string.
///
/// Performs database lookup (exact, then fuzzy), fidelity classification,
/// and multi-crew / seat detection in a single call.
pub fn detect_aircraft(dcs_name: &str) -> AircraftDetection {
    let base = strip_seat_suffix(dcs_name).to_string();
    let db_info = aircraft_db::lookup(&base).or_else(|| aircraft_db::lookup_fuzzy(&base));
    let fidelity = classify_fidelity(&base);
    let seat = detect_seat(dcs_name);
    let multi_crew = is_multi_crew(dcs_name);

    AircraftDetection {
        raw_name: dcs_name.to_string(),
        base_name: base,
        db_info,
        fidelity,
        seat,
        multi_crew,
    }
}

/// Convenience: detect aircraft and return its recommended axes profile,
/// falling back to [`AxesProfile::StandardJet`] for unknown modules.
pub fn detect_axes_profile(dcs_name: &str) -> AxesProfile {
    detect_aircraft(dcs_name)
        .db_info
        .map(|i| i.axes_config)
        .unwrap_or(AxesProfile::StandardJet)
}

/// Convenience: detect aircraft category, returning `None` for unknown modules.
pub fn detect_category(dcs_name: &str) -> Option<AircraftCategory> {
    detect_aircraft(dcs_name).db_info.map(|i| i.category)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Fidelity classification ---

    #[test]
    fn test_fc3_modules_classified() {
        assert_eq!(classify_fidelity("Su-25T"), ModuleFidelity::Fc3);
        assert_eq!(classify_fidelity("F-15C"), ModuleFidelity::Fc3);
        assert_eq!(classify_fidelity("Su-27"), ModuleFidelity::Fc3);
        assert_eq!(classify_fidelity("MiG-29A"), ModuleFidelity::Fc3);
        assert_eq!(classify_fidelity("Su-33"), ModuleFidelity::Fc3);
        assert_eq!(classify_fidelity("MiG-29S"), ModuleFidelity::Fc3);
    }

    #[test]
    fn test_full_fidelity_modules() {
        assert_eq!(classify_fidelity("FA-18C_hornet"), ModuleFidelity::FullFidelity);
        assert_eq!(classify_fidelity("F-16C_50"), ModuleFidelity::FullFidelity);
        assert_eq!(classify_fidelity("A-10C_2"), ModuleFidelity::FullFidelity);
        assert_eq!(classify_fidelity("AH-64D_BLK_II"), ModuleFidelity::FullFidelity);
    }

    #[test]
    fn test_unknown_module_is_mod() {
        assert_eq!(classify_fidelity("SomeCommunityMod"), ModuleFidelity::Mod);
        assert_eq!(classify_fidelity("Boeing747"), ModuleFidelity::Mod);
    }

    #[test]
    fn test_fidelity_display() {
        assert_eq!(ModuleFidelity::FullFidelity.to_string(), "Full Fidelity");
        assert_eq!(ModuleFidelity::Fc3.to_string(), "FC3");
        assert_eq!(ModuleFidelity::Mod.to_string(), "Mod");
    }

    // --- Multi-crew detection ---

    #[test]
    fn test_multi_crew_f14() {
        assert!(is_multi_crew("F-14B"));
        assert!(is_multi_crew("F-14B_RIO"));
    }

    #[test]
    fn test_multi_crew_apache() {
        assert!(is_multi_crew("AH-64D_BLK_II"));
        assert!(is_multi_crew("AH-64D_BLK_II_CPG"));
    }

    #[test]
    fn test_single_seat_not_multi_crew() {
        assert!(!is_multi_crew("F-16C_50"));
        assert!(!is_multi_crew("FA-18C_hornet"));
    }

    // --- Seat detection ---

    #[test]
    fn test_detect_seat_single() {
        assert_eq!(detect_seat("F-16C_50"), CockpitSeat::Single);
        assert_eq!(detect_seat("FA-18C_hornet"), CockpitSeat::Single);
    }

    #[test]
    fn test_detect_seat_front() {
        assert_eq!(detect_seat("F-14B"), CockpitSeat::Front);
        assert_eq!(detect_seat("AH-64D_BLK_II"), CockpitSeat::Front);
    }

    #[test]
    fn test_detect_seat_rear_rio() {
        assert_eq!(detect_seat("F-14B_RIO"), CockpitSeat::Rear);
    }

    #[test]
    fn test_detect_seat_rear_wso() {
        assert_eq!(detect_seat("F-15ESE_WSO"), CockpitSeat::Rear);
    }

    #[test]
    fn test_detect_seat_rear_cpg() {
        assert_eq!(detect_seat("AH-64D_BLK_II_CPG"), CockpitSeat::Rear);
    }

    #[test]
    fn test_detect_seat_rear_gunner() {
        assert_eq!(detect_seat("Mi-24P_GUNNER"), CockpitSeat::Rear);
    }

    #[test]
    fn test_detect_seat_case_insensitive_suffix() {
        assert_eq!(detect_seat("F-14B_rio"), CockpitSeat::Rear);
        assert_eq!(detect_seat("F-14B_Rear"), CockpitSeat::Rear);
    }

    #[test]
    fn test_seat_display() {
        assert_eq!(CockpitSeat::Single.to_string(), "Single Seat");
        assert_eq!(CockpitSeat::Front.to_string(), "Front Seat");
        assert_eq!(CockpitSeat::Rear.to_string(), "Rear Seat");
    }

    // --- Full detection ---

    #[test]
    fn test_detect_f16() {
        let det = detect_aircraft("F-16C_50");
        assert_eq!(det.raw_name, "F-16C_50");
        assert_eq!(det.base_name, "F-16C_50");
        assert!(det.db_info.is_some());
        assert_eq!(det.db_info.unwrap().display_name, "F-16C Viper");
        assert_eq!(det.fidelity, ModuleFidelity::FullFidelity);
        assert_eq!(det.seat, CockpitSeat::Single);
        assert!(!det.multi_crew);
    }

    #[test]
    fn test_detect_f14_pilot() {
        let det = detect_aircraft("F-14B");
        assert_eq!(det.seat, CockpitSeat::Front);
        assert!(det.multi_crew);
        assert_eq!(det.fidelity, ModuleFidelity::FullFidelity);
    }

    #[test]
    fn test_detect_f14_rio() {
        let det = detect_aircraft("F-14B_RIO");
        assert_eq!(det.base_name, "F-14B");
        assert_eq!(det.seat, CockpitSeat::Rear);
        assert!(det.multi_crew);
        assert!(det.db_info.is_some());
    }

    #[test]
    fn test_detect_apache_cpg() {
        let det = detect_aircraft("AH-64D_BLK_II_CPG");
        assert_eq!(det.base_name, "AH-64D_BLK_II");
        assert_eq!(det.seat, CockpitSeat::Rear);
        assert!(det.multi_crew);
        assert_eq!(
            det.db_info.unwrap().category,
            AircraftCategory::Helicopter
        );
    }

    #[test]
    fn test_detect_fc3_su25t() {
        let det = detect_aircraft("Su-25T");
        assert_eq!(det.fidelity, ModuleFidelity::Fc3);
        assert!(!det.multi_crew);
        assert_eq!(det.seat, CockpitSeat::Single);
    }

    #[test]
    fn test_detect_unknown_module() {
        let det = detect_aircraft("SuperDuperMod_v3");
        assert!(det.db_info.is_none());
        assert_eq!(det.fidelity, ModuleFidelity::Mod);
        assert_eq!(det.seat, CockpitSeat::Single);
        assert!(!det.multi_crew);
    }

    #[test]
    fn test_detect_axes_profile_helicopter() {
        assert_eq!(
            detect_axes_profile("AH-64D_BLK_II"),
            AxesProfile::HelicopterCollective
        );
    }

    #[test]
    fn test_detect_axes_profile_warbird() {
        assert_eq!(
            detect_axes_profile("TF-51D"),
            AxesProfile::Warbird4Axis
        );
    }

    #[test]
    fn test_detect_axes_profile_unknown_defaults_jet() {
        assert_eq!(
            detect_axes_profile("UnknownModule"),
            AxesProfile::StandardJet
        );
    }

    #[test]
    fn test_detect_category_fixed_wing() {
        assert_eq!(
            detect_category("FA-18C_hornet"),
            Some(AircraftCategory::FixedWing)
        );
    }

    #[test]
    fn test_detect_category_unknown() {
        assert_eq!(detect_category("UnknownModule"), None);
    }

    #[test]
    fn test_detect_aircraft_from_telemetry_header() {
        // Simulates parsing the aircraft name from a real telemetry packet
        let raw_aircraft_name = "FA-18C_hornet";
        let det = detect_aircraft(raw_aircraft_name);
        assert_eq!(det.db_info.unwrap().display_name, "F/A-18C Hornet");
        assert_eq!(det.fidelity, ModuleFidelity::FullFidelity);
        assert_eq!(det.seat, CockpitSeat::Single);
    }

    #[test]
    fn test_strip_seat_suffix_no_suffix() {
        assert_eq!(strip_seat_suffix("F-16C_50"), "F-16C_50");
    }

    #[test]
    fn test_strip_seat_suffix_with_rio() {
        assert_eq!(strip_seat_suffix("F-14B_RIO"), "F-14B");
    }

    #[test]
    fn test_strip_seat_suffix_with_cpg() {
        assert_eq!(strip_seat_suffix("AH-64D_BLK_II_CPG"), "AH-64D_BLK_II");
    }

    #[test]
    fn test_detect_hind_multi_crew() {
        assert!(is_multi_crew("Mi-24P"));
        let det = detect_aircraft("Mi-24P_GUNNER");
        assert_eq!(det.seat, CockpitSeat::Rear);
        assert!(det.multi_crew);
    }

    #[test]
    fn test_detect_f15e_wso() {
        let det = detect_aircraft("F-15ESE_WSO");
        assert_eq!(det.base_name, "F-15ESE");
        assert_eq!(det.seat, CockpitSeat::Rear);
        assert!(det.multi_crew);
        assert!(det.db_info.is_some());
    }

    #[test]
    fn test_detect_mosquito_rear() {
        let det = detect_aircraft("MosquitoFBMkVI_REAR");
        assert_eq!(det.base_name, "MosquitoFBMkVI");
        assert_eq!(det.seat, CockpitSeat::Rear);
        assert!(det.multi_crew);
    }
}
