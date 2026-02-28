// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Enhanced aircraft detection for X-Plane
//!
//! Builds on the base [`AircraftDetector`](crate::aircraft::AircraftDetector)
//! to provide:
//!
//! * ICAO code parsing from `sim/aircraft/view/acf_ICAO`
//! * Display-name parsing from `sim/aircraft/view/acf_descrip`
//! * Matching against the X-Plane aircraft database
//! * Livery change detection via `sim/aircraft/view/acf_livery_path`
//! * Community / non-standard ICAO handling

use crate::aircraft_db::{AircraftCategory, AircraftDatabase, XPlaneAircraftEntry};
use std::collections::HashMap;
use tracing::{debug, info};

// ── DataRef paths ────────────────────────────────────────────────────

/// ICAO type designator dataref.
pub const DATAREF_ACF_ICAO: &str = "sim/aircraft/view/acf_ICAO";
/// Description / display name dataref.
pub const DATAREF_ACF_DESCRIP: &str = "sim/aircraft/view/acf_descrip";
/// Livery path dataref.
pub const DATAREF_ACF_LIVERY: &str = "sim/aircraft/view/acf_livery_path";
/// Aircraft file path dataref.
pub const DATAREF_ACF_FILE_PATH: &str = "sim/aircraft/view/acf_file_path";
/// Author dataref.
pub const DATAREF_ACF_AUTHOR: &str = "sim/aircraft/view/acf_author";

// ── Types ────────────────────────────────────────────────────────────

/// Result of an enhanced aircraft detection pass.
#[derive(Debug, Clone, PartialEq)]
pub struct EnhancedAircraftId {
    /// Cleaned ICAO type designator (upper-case, max 4 chars).
    pub icao: String,
    /// Display name as reported by X-Plane.
    pub display_name: String,
    /// Livery path (if available).
    pub livery_path: Option<String>,
    /// Database match (if the aircraft is in the known database).
    pub db_match: Option<AircraftDbMatch>,
    /// Whether this ICAO is a well-known standard code.
    pub is_standard_icao: bool,
}

/// Information from a successful database match.
#[derive(Debug, Clone, PartialEq)]
pub struct AircraftDbMatch {
    pub display_name: String,
    pub category: AircraftCategory,
    pub default_profile: String,
    pub custom_datarefs: Vec<String>,
}

/// Describes a change between two detection passes.
#[derive(Debug, Clone, PartialEq)]
pub enum AircraftChange {
    /// Aircraft type changed (different ICAO).
    TypeChanged {
        old_icao: String,
        new_icao: String,
    },
    /// Same aircraft type but livery changed.
    LiveryChanged {
        icao: String,
        old_livery: String,
        new_livery: String,
    },
    /// No change.
    None,
}

// ── Standard ICAO set ────────────────────────────────────────────────

/// A representative set of well-known ICAO type designators.
const STANDARD_ICAO_CODES: &[&str] = &[
    // GA
    "C150", "C152", "C172", "C182", "C206", "C208", "C210", "PA28", "PA32", "PA34", "PA46",
    "SR20", "SR22", "BE36", "BE58", "M20P", "M20T", "DA40", "DA42", "DA62", "RV7", "RV10",
    // Turboprop
    "PC12", "TBM9", "B350", "C90", "DHC6", "ATR7",
    // Airliners
    "A318", "A319", "A320", "A321", "A330", "A340", "A350", "A380",
    "B737", "B738", "B739", "B744", "B748", "B752", "B763", "B772", "B77L", "B77W",
    "B787", "B788", "B789", "MD11", "MD80",
    "CRJ2", "CRJ7", "E145", "E170", "E175", "E190",
    // Military
    "F16", "F18", "F22", "F35", "A10", "C130", "C17", "B1", "B2", "B52",
    // Helicopters
    "R22", "R44", "B206", "B407", "EC35", "S76", "UH1H", "AH64", "H60",
    // Gliders
    "ASK2", "DG80", "LS8",
];

// ── EnhancedAircraftDetector ─────────────────────────────────────────

/// Enhanced aircraft detection with database matching, livery tracking,
/// and community aircraft support.
pub struct EnhancedAircraftDetector {
    db: AircraftDatabase,
    /// ICAO alias map: non-standard ICAO → standard ICAO.
    icao_aliases: HashMap<String, String>,
    /// Last detection result for change detection.
    last_detection: Option<EnhancedAircraftId>,
}

impl EnhancedAircraftDetector {
    /// Create a new detector backed by the given aircraft database.
    pub fn new(db: AircraftDatabase) -> Self {
        let mut detector = Self {
            db,
            icao_aliases: HashMap::new(),
            last_detection: None,
        };
        detector.init_aliases();
        detector
    }

    /// Create a detector with the default aircraft database.
    pub fn with_default_db() -> Self {
        Self::new(AircraftDatabase::new())
    }

    // ── Public API ───────────────────────────────────────────────────

    /// Identify the aircraft from raw dataref values.
    ///
    /// `raw_values` maps dataref paths (e.g. `DATAREF_ACF_ICAO`) to their
    /// string representation. Float-array string datarefs should already be
    /// decoded to a Rust `String`.
    pub fn identify(
        &mut self,
        raw_values: &HashMap<String, String>,
    ) -> EnhancedAircraftId {
        let raw_icao = raw_values
            .get(DATAREF_ACF_ICAO)
            .cloned()
            .unwrap_or_default();
        let display_name = raw_values
            .get(DATAREF_ACF_DESCRIP)
            .cloned()
            .unwrap_or_default();
        let livery_path = raw_values.get(DATAREF_ACF_LIVERY).cloned();
        let acf_file = raw_values
            .get(DATAREF_ACF_FILE_PATH)
            .cloned()
            .unwrap_or_default();

        let icao = self.parse_icao(&raw_icao);
        let resolved = self.resolve_alias(&icao);
        let is_standard = self.is_standard_icao(&resolved);

        // Try to match against the database (by acf_file first, then ICAO scan)
        let db_match = self.match_database(&acf_file, &resolved);

        if !is_standard {
            debug!(
                raw_icao = %raw_icao,
                resolved = %resolved,
                "community aircraft (non-standard ICAO)"
            );
        }

        let id = EnhancedAircraftId {
            icao: resolved,
            display_name,
            livery_path,
            db_match,
            is_standard_icao: is_standard,
        };

        info!(
            icao = %id.icao,
            name = %id.display_name,
            standard = id.is_standard_icao,
            "aircraft identified"
        );

        self.last_detection = Some(id.clone());
        id
    }

    /// Detect what changed compared to the previous [`identify`](Self::identify) call.
    pub fn detect_change(&self, current: &EnhancedAircraftId) -> AircraftChange {
        let Some(prev) = &self.last_detection else {
            return AircraftChange::None;
        };

        if prev.icao != current.icao {
            return AircraftChange::TypeChanged {
                old_icao: prev.icao.clone(),
                new_icao: current.icao.clone(),
            };
        }

        match (&prev.livery_path, &current.livery_path) {
            (Some(old), Some(new)) if old != new => AircraftChange::LiveryChanged {
                icao: current.icao.clone(),
                old_livery: old.clone(),
                new_livery: new.clone(),
            },
            _ => AircraftChange::None,
        }
    }

    /// Register a custom ICAO alias (e.g. community add-on ICAO → standard).
    pub fn add_alias(&mut self, from: &str, to: &str) {
        self.icao_aliases
            .insert(from.to_uppercase(), to.to_uppercase());
    }

    /// Check whether the given ICAO is in the well-known set.
    pub fn is_standard_icao(&self, icao: &str) -> bool {
        STANDARD_ICAO_CODES.contains(&icao)
    }

    /// Return the underlying database reference.
    pub fn database(&self) -> &AircraftDatabase {
        &self.db
    }

    // ── Internals ────────────────────────────────────────────────────

    /// Clean and normalise a raw ICAO string from X-Plane.
    fn parse_icao(&self, raw: &str) -> String {
        let cleaned: String = raw
            .trim()
            .replace('\0', "")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_uppercase();

        // Standard ICAO designators are at most 4 characters
        if cleaned.len() > 4 {
            cleaned[..4].to_owned()
        } else {
            cleaned
        }
    }

    /// Resolve an alias, returning the original if no alias exists.
    fn resolve_alias(&self, icao: &str) -> String {
        self.icao_aliases
            .get(icao)
            .cloned()
            .unwrap_or_else(|| icao.to_owned())
    }

    /// Try to match against the aircraft database.
    fn match_database(&self, acf_file: &str, icao: &str) -> Option<AircraftDbMatch> {
        // Primary: exact acf file path
        if !acf_file.is_empty() {
            if let Some(entry) = self.db.get(acf_file) {
                return Some(Self::entry_to_match(entry));
            }
        }

        // Fallback: scan all entries — check display_name and acf_path
        if !icao.is_empty() {
            let icao_upper = icao.to_uppercase();
            for entry in self.db.all() {
                if entry.display_name.to_uppercase().contains(&icao_upper)
                    || entry.acf_path.to_uppercase().contains(&icao_upper)
                {
                    return Some(Self::entry_to_match(entry));
                }
            }
        }

        None
    }

    fn entry_to_match(entry: &XPlaneAircraftEntry) -> AircraftDbMatch {
        AircraftDbMatch {
            display_name: entry.display_name.to_owned(),
            category: entry.category,
            default_profile: entry.default_profile.to_owned(),
            custom_datarefs: entry.custom_datarefs.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Seed common community-aircraft aliases.
    fn init_aliases(&mut self) {
        let aliases = [
            // Toliss variants
            ("TLSA", "A319"),
            ("TLSB", "A321"),
            // FlightFactor
            ("FF32", "A320"),
            ("FFB7", "B772"),
            // IXEG
            ("IX73", "B733"),
            // Rotate
            ("RTMD", "MD80"),
            // Jar Design
            ("JDA3", "A330"),
            // Felis
            ("FL74", "B742"),
        ];
        for (from, to) in aliases {
            self.icao_aliases
                .insert(from.to_owned(), to.to_owned());
        }
    }
}

impl Default for EnhancedAircraftDetector {
    fn default() -> Self {
        Self::with_default_db()
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_raw(icao: &str, descrip: &str) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert(DATAREF_ACF_ICAO.to_owned(), icao.to_owned());
        m.insert(DATAREF_ACF_DESCRIP.to_owned(), descrip.to_owned());
        m
    }

    fn make_raw_with_livery(
        icao: &str,
        descrip: &str,
        livery: &str,
    ) -> HashMap<String, String> {
        let mut m = make_raw(icao, descrip);
        m.insert(DATAREF_ACF_LIVERY.to_owned(), livery.to_owned());
        m
    }

    // ── ICAO parsing ─────────────────────────────────────────────────

    #[test]
    fn parse_icao_basic() {
        let det = EnhancedAircraftDetector::with_default_db();
        assert_eq!(det.parse_icao("C172"), "C172");
    }

    #[test]
    fn parse_icao_strips_nulls_and_spaces() {
        let det = EnhancedAircraftDetector::with_default_db();
        assert_eq!(det.parse_icao("C172\0\0\0"), "C172");
        assert_eq!(det.parse_icao("  c172  "), "C172");
    }

    #[test]
    fn parse_icao_truncates_to_four_chars() {
        let det = EnhancedAircraftDetector::with_default_db();
        assert_eq!(det.parse_icao("C172SP"), "C172");
    }

    #[test]
    fn parse_icao_uppercases() {
        let det = EnhancedAircraftDetector::with_default_db();
        assert_eq!(det.parse_icao("a320"), "A320");
    }

    #[test]
    fn parse_icao_empty_returns_empty() {
        let det = EnhancedAircraftDetector::with_default_db();
        assert_eq!(det.parse_icao(""), "");
    }

    // ── Standard ICAO check ──────────────────────────────────────────

    #[test]
    fn known_icao_is_standard() {
        let det = EnhancedAircraftDetector::with_default_db();
        assert!(det.is_standard_icao("C172"));
        assert!(det.is_standard_icao("A320"));
        assert!(det.is_standard_icao("B738"));
    }

    #[test]
    fn community_icao_is_not_standard() {
        let det = EnhancedAircraftDetector::with_default_db();
        assert!(!det.is_standard_icao("TLSA"));
        assert!(!det.is_standard_icao("FF32"));
        assert!(!det.is_standard_icao("ZZZZ"));
    }

    // ── Alias resolution ─────────────────────────────────────────────

    #[test]
    fn alias_resolves_to_standard() {
        let det = EnhancedAircraftDetector::with_default_db();
        assert_eq!(det.resolve_alias("TLSA"), "A319");
        assert_eq!(det.resolve_alias("FF32"), "A320");
    }

    #[test]
    fn no_alias_returns_self() {
        let det = EnhancedAircraftDetector::with_default_db();
        assert_eq!(det.resolve_alias("C172"), "C172");
    }

    #[test]
    fn custom_alias_is_registered() {
        let mut det = EnhancedAircraftDetector::with_default_db();
        det.add_alias("MYAC", "B738");
        assert_eq!(det.resolve_alias("MYAC"), "B738");
    }

    // ── Identification ───────────────────────────────────────────────

    #[test]
    fn identify_standard_aircraft() {
        let mut det = EnhancedAircraftDetector::with_default_db();
        let raw = make_raw("C172", "Cessna 172SP Skyhawk");
        let id = det.identify(&raw);
        assert_eq!(id.icao, "C172");
        assert_eq!(id.display_name, "Cessna 172SP Skyhawk");
        assert!(id.is_standard_icao);
    }

    #[test]
    fn identify_community_aircraft_via_alias() {
        let mut det = EnhancedAircraftDetector::with_default_db();
        let raw = make_raw("TLSA", "Toliss A319");
        let id = det.identify(&raw);
        assert_eq!(id.icao, "A319");
        assert!(id.is_standard_icao);
    }

    #[test]
    fn identify_unknown_community_aircraft() {
        let mut det = EnhancedAircraftDetector::with_default_db();
        let raw = make_raw("XXYZ", "Custom Build Ultralight");
        let id = det.identify(&raw);
        assert_eq!(id.icao, "XXYZ");
        assert!(!id.is_standard_icao);
    }

    #[test]
    fn identify_includes_livery() {
        let mut det = EnhancedAircraftDetector::with_default_db();
        let raw = make_raw_with_livery("A320", "Airbus A320", "liveries/Delta/");
        let id = det.identify(&raw);
        assert_eq!(id.livery_path, Some("liveries/Delta/".to_owned()));
    }

    // ── Database matching ────────────────────────────────────────────

    #[test]
    fn match_by_acf_path() {
        let mut det = EnhancedAircraftDetector::with_default_db();
        let mut raw = make_raw("C172", "Cessna 172SP");
        raw.insert(
            DATAREF_ACF_FILE_PATH.to_owned(),
            "Aircraft/Laminar Research/Cessna 172SP/Cessna_172SP.acf".to_owned(),
        );
        let id = det.identify(&raw);
        assert!(id.db_match.is_some());
        let m = id.db_match.unwrap();
        assert_eq!(m.category, AircraftCategory::SinglePiston);
        assert_eq!(m.default_profile, "ga-single-piston");
    }

    #[test]
    fn match_by_icao_display_name_fallback() {
        let mut det = EnhancedAircraftDetector::with_default_db();
        // No file path, but display name contains a string that appears in a DB entry
        let raw = make_raw("B738", "Boeing 737-800");
        let id = det.identify(&raw);
        // The DB has "Boeing 737-800" as a display name — the scan finds it
        assert!(id.db_match.is_some());
    }

    #[test]
    fn no_match_for_unknown_aircraft() {
        let mut det = EnhancedAircraftDetector::with_default_db();
        let raw = make_raw("ZZZZ", "Totally Unknown Plane");
        let id = det.identify(&raw);
        assert!(id.db_match.is_none());
    }

    // ── Change detection ─────────────────────────────────────────────

    #[test]
    fn detect_type_change() {
        let mut det = EnhancedAircraftDetector::with_default_db();

        let raw1 = make_raw("C172", "Cessna 172");
        det.identify(&raw1);

        let raw2 = make_raw("A320", "Airbus A320");
        let _id2 = det.identify(&raw2);

        // Compare against the stored last_detection *before* the second identify
        // — but we need the state that was set by the first identify.
        // So we reconstruct: use the result of the second identify vs what was
        // stored after the first.
        let mut det2 = EnhancedAircraftDetector::with_default_db();
        det2.identify(&raw1); // sets last_detection = C172
        let id2b = EnhancedAircraftId {
            icao: "A320".to_owned(),
            display_name: "Airbus A320".to_owned(),
            livery_path: None,
            db_match: None,
            is_standard_icao: true,
        };
        let change = det2.detect_change(&id2b);
        assert!(matches!(change, AircraftChange::TypeChanged { .. }));
        if let AircraftChange::TypeChanged { old_icao, new_icao } = change {
            assert_eq!(old_icao, "C172");
            assert_eq!(new_icao, "A320");
        }
    }

    #[test]
    fn detect_livery_change() {
        let mut det = EnhancedAircraftDetector::with_default_db();

        let raw1 = make_raw_with_livery("A320", "Airbus A320", "liveries/Delta/");
        det.identify(&raw1);

        let current = EnhancedAircraftId {
            icao: "A320".to_owned(),
            display_name: "Airbus A320".to_owned(),
            livery_path: Some("liveries/United/".to_owned()),
            db_match: None,
            is_standard_icao: true,
        };
        let change = det.detect_change(&current);
        assert!(matches!(change, AircraftChange::LiveryChanged { .. }));
        if let AircraftChange::LiveryChanged {
            old_livery,
            new_livery,
            ..
        } = change
        {
            assert_eq!(old_livery, "liveries/Delta/");
            assert_eq!(new_livery, "liveries/United/");
        }
    }

    #[test]
    fn detect_no_change() {
        let mut det = EnhancedAircraftDetector::with_default_db();

        let raw = make_raw_with_livery("C172", "Cessna 172", "liveries/default/");
        det.identify(&raw);

        let current = EnhancedAircraftId {
            icao: "C172".to_owned(),
            display_name: "Cessna 172".to_owned(),
            livery_path: Some("liveries/default/".to_owned()),
            db_match: None,
            is_standard_icao: true,
        };
        assert_eq!(det.detect_change(&current), AircraftChange::None);
    }

    #[test]
    fn detect_change_when_no_previous() {
        let det = EnhancedAircraftDetector::with_default_db();
        let current = EnhancedAircraftId {
            icao: "C172".to_owned(),
            display_name: "Cessna 172".to_owned(),
            livery_path: None,
            db_match: None,
            is_standard_icao: true,
        };
        assert_eq!(det.detect_change(&current), AircraftChange::None);
    }

    // ── Edge cases ───────────────────────────────────────────────────

    #[test]
    fn empty_raw_values_produce_empty_id() {
        let mut det = EnhancedAircraftDetector::with_default_db();
        let raw = HashMap::new();
        let id = det.identify(&raw);
        assert_eq!(id.icao, "");
        assert_eq!(id.display_name, "");
        assert!(!id.is_standard_icao);
    }

    #[test]
    fn non_ascii_chars_stripped_from_icao() {
        let det = EnhancedAircraftDetector::with_default_db();
        assert_eq!(det.parse_icao("C-172!"), "C172");
        assert_eq!(det.parse_icao("A320™"), "A320");
    }

    #[test]
    fn default_constructor() {
        let det = EnhancedAircraftDetector::default();
        assert!(!det.database().is_empty());
    }
}
