// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS aircraft database
//!
//! Maps DCS internal module names to aircraft metadata used for profile
//! selection, FFB tuning, and axis configuration.

/// Category of a DCS aircraft module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AircraftCategory {
    FixedWing,
    Helicopter,
    TrainerJet,
    WarBird,
    TransportCargo,
}

impl std::fmt::Display for AircraftCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AircraftCategory::FixedWing => write!(f, "Fixed Wing"),
            AircraftCategory::Helicopter => write!(f, "Helicopter"),
            AircraftCategory::TrainerJet => write!(f, "Trainer Jet"),
            AircraftCategory::WarBird => write!(f, "Warbird"),
            AircraftCategory::TransportCargo => write!(f, "Transport/Cargo"),
        }
    }
}

/// Axes configuration profile for a class of aircraft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AxesProfile {
    /// Standard stick + throttle jet layout (e.g. F-16, F/A-18).
    StandardJet,
    /// Helicopter collective + cyclic + anti-torque pedals.
    HelicopterCollective,
    /// Yoke + throttle quadrant (transports, some warbirds).
    YokeThrottle,
    /// Stick + throttle + prop/mixture for WWII aircraft.
    Warbird4Axis,
}

impl std::fmt::Display for AxesProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AxesProfile::StandardJet => write!(f, "Standard Jet"),
            AxesProfile::HelicopterCollective => write!(f, "Helicopter Collective"),
            AxesProfile::YokeThrottle => write!(f, "Yoke + Throttle"),
            AxesProfile::Warbird4Axis => write!(f, "Warbird 4-Axis"),
        }
    }
}

/// Metadata for a single DCS aircraft module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DcsAircraftInfo {
    /// DCS internal module name (e.g. `"F-16C_50"`).
    pub dcs_name: &'static str,
    /// Human-readable display name.
    pub display_name: &'static str,
    /// Aircraft category.
    pub category: AircraftCategory,
    /// Whether a curated FFB profile exists.
    pub has_ffb_profile: bool,
    /// Recommended axes configuration.
    pub axes_config: AxesProfile,
}

// ---------------------------------------------------------------------------
// Static database
// ---------------------------------------------------------------------------

/// Complete list of supported DCS aircraft modules.
static AIRCRAFT_DB: &[DcsAircraftInfo] = &[
    // --- Modern jets ---
    DcsAircraftInfo {
        dcs_name: "F-16C_50",
        display_name: "F-16C Viper",
        category: AircraftCategory::FixedWing,
        has_ffb_profile: true,
        axes_config: AxesProfile::StandardJet,
    },
    DcsAircraftInfo {
        dcs_name: "FA-18C_hornet",
        display_name: "F/A-18C Hornet",
        category: AircraftCategory::FixedWing,
        has_ffb_profile: true,
        axes_config: AxesProfile::StandardJet,
    },
    DcsAircraftInfo {
        dcs_name: "A-10C_2",
        display_name: "A-10C II Thunderbolt",
        category: AircraftCategory::FixedWing,
        has_ffb_profile: true,
        axes_config: AxesProfile::StandardJet,
    },
    DcsAircraftInfo {
        dcs_name: "F-14B",
        display_name: "F-14B Tomcat",
        category: AircraftCategory::FixedWing,
        has_ffb_profile: true,
        axes_config: AxesProfile::StandardJet,
    },
    DcsAircraftInfo {
        dcs_name: "F-15ESE",
        display_name: "F-15E Strike Eagle",
        category: AircraftCategory::FixedWing,
        has_ffb_profile: true,
        axes_config: AxesProfile::StandardJet,
    },
    DcsAircraftInfo {
        dcs_name: "JF-17",
        display_name: "JF-17 Thunder",
        category: AircraftCategory::FixedWing,
        has_ffb_profile: false,
        axes_config: AxesProfile::StandardJet,
    },
    DcsAircraftInfo {
        dcs_name: "AJS37",
        display_name: "AJS-37 Viggen",
        category: AircraftCategory::FixedWing,
        has_ffb_profile: false,
        axes_config: AxesProfile::StandardJet,
    },
    DcsAircraftInfo {
        dcs_name: "M-2000C",
        display_name: "Mirage 2000C",
        category: AircraftCategory::FixedWing,
        has_ffb_profile: false,
        axes_config: AxesProfile::StandardJet,
    },
    DcsAircraftInfo {
        dcs_name: "AV8BNA",
        display_name: "AV-8B Night Attack",
        category: AircraftCategory::FixedWing,
        has_ffb_profile: false,
        axes_config: AxesProfile::StandardJet,
    },
    // --- Flanker family (FC3-level) ---
    DcsAircraftInfo {
        dcs_name: "Su-25T",
        display_name: "Su-25T Frogfoot",
        category: AircraftCategory::FixedWing,
        has_ffb_profile: false,
        axes_config: AxesProfile::StandardJet,
    },
    DcsAircraftInfo {
        dcs_name: "Su-27",
        display_name: "Su-27 Flanker",
        category: AircraftCategory::FixedWing,
        has_ffb_profile: false,
        axes_config: AxesProfile::StandardJet,
    },
    DcsAircraftInfo {
        dcs_name: "MiG-29S",
        display_name: "MiG-29 Fulcrum",
        category: AircraftCategory::FixedWing,
        has_ffb_profile: false,
        axes_config: AxesProfile::StandardJet,
    },
    // --- Trainer ---
    DcsAircraftInfo {
        dcs_name: "L-39C",
        display_name: "L-39 Albatros",
        category: AircraftCategory::TrainerJet,
        has_ffb_profile: false,
        axes_config: AxesProfile::StandardJet,
    },
    // --- Helicopters ---
    DcsAircraftInfo {
        dcs_name: "AH-64D_BLK_II",
        display_name: "AH-64D Apache",
        category: AircraftCategory::Helicopter,
        has_ffb_profile: true,
        axes_config: AxesProfile::HelicopterCollective,
    },
    DcsAircraftInfo {
        dcs_name: "Mi-8MT",
        display_name: "Mi-8MTV2 Hip",
        category: AircraftCategory::Helicopter,
        has_ffb_profile: false,
        axes_config: AxesProfile::HelicopterCollective,
    },
    DcsAircraftInfo {
        dcs_name: "Mi-24P",
        display_name: "Mi-24P Hind",
        category: AircraftCategory::Helicopter,
        has_ffb_profile: false,
        axes_config: AxesProfile::HelicopterCollective,
    },
    DcsAircraftInfo {
        dcs_name: "UH-1H",
        display_name: "UH-1H Huey",
        category: AircraftCategory::Helicopter,
        has_ffb_profile: true,
        axes_config: AxesProfile::HelicopterCollective,
    },
    DcsAircraftInfo {
        dcs_name: "Ka-50_3",
        display_name: "Ka-50 Black Shark III",
        category: AircraftCategory::Helicopter,
        has_ffb_profile: true,
        axes_config: AxesProfile::HelicopterCollective,
    },
    // --- Warbirds ---
    DcsAircraftInfo {
        dcs_name: "TF-51D",
        display_name: "P-51D Mustang",
        category: AircraftCategory::WarBird,
        has_ffb_profile: true,
        axes_config: AxesProfile::Warbird4Axis,
    },
    DcsAircraftInfo {
        dcs_name: "SpitfireLFMkIX",
        display_name: "Spitfire LF Mk.IX",
        category: AircraftCategory::WarBird,
        has_ffb_profile: true,
        axes_config: AxesProfile::Warbird4Axis,
    },
    DcsAircraftInfo {
        dcs_name: "Bf-109K-4",
        display_name: "Bf 109 K-4",
        category: AircraftCategory::WarBird,
        has_ffb_profile: false,
        axes_config: AxesProfile::Warbird4Axis,
    },
    DcsAircraftInfo {
        dcs_name: "FW-190D9",
        display_name: "Fw 190 D-9",
        category: AircraftCategory::WarBird,
        has_ffb_profile: false,
        axes_config: AxesProfile::Warbird4Axis,
    },
    // --- Transport ---
    DcsAircraftInfo {
        dcs_name: "C-101CC",
        display_name: "C-101 Aviojet",
        category: AircraftCategory::TransportCargo,
        has_ffb_profile: false,
        axes_config: AxesProfile::YokeThrottle,
    },
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Look up aircraft info by DCS internal module name.
pub fn lookup(dcs_name: &str) -> Option<&'static DcsAircraftInfo> {
    AIRCRAFT_DB.iter().find(|a| a.dcs_name == dcs_name)
}

/// Look up aircraft info with a case-insensitive, partial match.
///
/// Returns the first entry whose `dcs_name` contains `query` (ignoring case).
pub fn lookup_fuzzy(query: &str) -> Option<&'static DcsAircraftInfo> {
    let lower = query.to_ascii_lowercase();
    AIRCRAFT_DB
        .iter()
        .find(|a| a.dcs_name.to_ascii_lowercase().contains(&lower))
}

/// Return all aircraft entries in the database.
pub fn all_aircraft() -> &'static [DcsAircraftInfo] {
    AIRCRAFT_DB
}

/// Return all aircraft of a given category.
pub fn by_category(category: AircraftCategory) -> Vec<&'static DcsAircraftInfo> {
    AIRCRAFT_DB
        .iter()
        .filter(|a| a.category == category)
        .collect()
}

/// Return all aircraft that have a curated FFB profile.
pub fn with_ffb_profiles() -> Vec<&'static DcsAircraftInfo> {
    AIRCRAFT_DB.iter().filter(|a| a.has_ffb_profile).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_has_at_least_20_entries() {
        assert!(all_aircraft().len() >= 20);
    }

    #[test]
    fn test_lookup_f16() {
        let info = lookup("F-16C_50").unwrap();
        assert_eq!(info.display_name, "F-16C Viper");
        assert_eq!(info.category, AircraftCategory::FixedWing);
        assert!(info.has_ffb_profile);
        assert_eq!(info.axes_config, AxesProfile::StandardJet);
    }

    #[test]
    fn test_lookup_fa18() {
        let info = lookup("FA-18C_hornet").unwrap();
        assert_eq!(info.display_name, "F/A-18C Hornet");
    }

    #[test]
    fn test_lookup_a10c() {
        let info = lookup("A-10C_2").unwrap();
        assert_eq!(info.display_name, "A-10C II Thunderbolt");
    }

    #[test]
    fn test_lookup_f14b() {
        let info = lookup("F-14B").unwrap();
        assert_eq!(info.category, AircraftCategory::FixedWing);
    }

    #[test]
    fn test_lookup_apache() {
        let info = lookup("AH-64D_BLK_II").unwrap();
        assert_eq!(info.category, AircraftCategory::Helicopter);
        assert_eq!(info.axes_config, AxesProfile::HelicopterCollective);
    }

    #[test]
    fn test_lookup_huey() {
        let info = lookup("UH-1H").unwrap();
        assert_eq!(info.category, AircraftCategory::Helicopter);
        assert!(info.has_ffb_profile);
    }

    #[test]
    fn test_lookup_p51d() {
        let info = lookup("TF-51D").unwrap();
        assert_eq!(info.category, AircraftCategory::WarBird);
        assert_eq!(info.axes_config, AxesProfile::Warbird4Axis);
    }

    #[test]
    fn test_lookup_spitfire() {
        let info = lookup("SpitfireLFMkIX").unwrap();
        assert_eq!(info.display_name, "Spitfire LF Mk.IX");
    }

    #[test]
    fn test_lookup_missing() {
        assert!(lookup("Boeing747").is_none());
    }

    #[test]
    fn test_fuzzy_lookup() {
        let info = lookup_fuzzy("f-16").unwrap();
        assert_eq!(info.dcs_name, "F-16C_50");
    }

    #[test]
    fn test_fuzzy_lookup_case_insensitive() {
        let info = lookup_fuzzy("ka-50").unwrap();
        assert_eq!(info.dcs_name, "Ka-50_3");
    }

    #[test]
    fn test_by_category_helicopter() {
        let helis = by_category(AircraftCategory::Helicopter);
        assert!(helis.len() >= 4);
        assert!(
            helis
                .iter()
                .all(|a| a.category == AircraftCategory::Helicopter)
        );
    }

    #[test]
    fn test_by_category_warbird() {
        let warbirds = by_category(AircraftCategory::WarBird);
        assert!(warbirds.len() >= 4);
    }

    #[test]
    fn test_with_ffb_profiles() {
        let ffb = with_ffb_profiles();
        assert!(ffb.len() >= 5);
        assert!(ffb.iter().all(|a| a.has_ffb_profile));
    }

    #[test]
    fn test_all_dcs_names_unique() {
        let names: Vec<_> = all_aircraft().iter().map(|a| a.dcs_name).collect();
        let mut uniq = names.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(names.len(), uniq.len(), "duplicate DCS names found");
    }

    #[test]
    fn test_display_category() {
        assert_eq!(AircraftCategory::FixedWing.to_string(), "Fixed Wing");
        assert_eq!(AircraftCategory::Helicopter.to_string(), "Helicopter");
    }

    #[test]
    fn test_display_axes_profile() {
        assert_eq!(AxesProfile::StandardJet.to_string(), "Standard Jet");
        assert_eq!(
            AxesProfile::HelicopterCollective.to_string(),
            "Helicopter Collective"
        );
    }
}
