// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Enhanced aircraft detection with fuzzy matching and confidence scoring.
//!
//! Supplements the core `AircraftDetector` (which talks to SimConnect) with
//! an offline matching engine that can identify aircraft from partial or
//! non-standard TITLE / ATC_TYPE / ATC_MODEL strings — including community
//! mods that use custom naming conventions.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Database entry
// ---------------------------------------------------------------------------

/// An entry in the aircraft recognition database.
#[derive(Debug, Clone, PartialEq)]
pub struct AircraftEntry {
    /// Canonical ICAO type designator (e.g. `"C172"`).
    pub icao: String,
    /// Human-readable name (e.g. `"Cessna 172 Skyhawk"`).
    pub display_name: String,
    /// Known title strings that may appear in SimConnect TITLE.
    pub known_titles: Vec<String>,
    /// Known ATC_TYPE values.
    pub known_atc_types: Vec<String>,
    /// Known ATC_MODEL values.
    pub known_atc_models: Vec<String>,
    /// Tags for broad classification (e.g. `"ga"`, `"airliner"`, `"helicopter"`).
    pub tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// Match result
// ---------------------------------------------------------------------------

/// Confidence level of a detection match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchConfidence {
    /// No usable match found.
    None,
    /// Weak heuristic match (e.g. substring in title).
    Low,
    /// Partial indicator match (one of ATC_TYPE or ATC_MODEL).
    Medium,
    /// Strong match on multiple indicators.
    High,
    /// Exact match on ATC_MODEL or full title.
    Exact,
}

/// Result of an aircraft detection attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectionResult {
    /// Matched ICAO code, if any.
    pub icao: Option<String>,
    /// Display name of the matched aircraft.
    pub display_name: Option<String>,
    /// Overall confidence.
    pub confidence: MatchConfidence,
    /// Individual confidence scores from each indicator.
    pub indicator_scores: IndicatorScores,
    /// Whether this looks like a community mod (non-standard title).
    pub is_community_mod: bool,
}

/// Per-indicator confidence scores (0.0 … 1.0).
#[derive(Debug, Clone, PartialEq)]
pub struct IndicatorScores {
    pub title_score: f32,
    pub atc_type_score: f32,
    pub atc_model_score: f32,
}

impl Default for IndicatorScores {
    fn default() -> Self {
        Self {
            title_score: 0.0,
            atc_type_score: 0.0,
            atc_model_score: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Sim data input
// ---------------------------------------------------------------------------

/// Raw sim data fields used for detection.
#[derive(Debug, Clone, Default)]
pub struct SimAircraftData {
    pub title: String,
    pub atc_type: String,
    pub atc_model: String,
}

// ---------------------------------------------------------------------------
// Detection engine
// ---------------------------------------------------------------------------

/// Fuzzy aircraft detection engine.
pub struct AircraftDetectionEngine {
    entries: Vec<AircraftEntry>,
    /// Optional index: lowercase ATC_MODEL → entry index for fast exact lookup.
    model_index: HashMap<String, usize>,
}

impl AircraftDetectionEngine {
    /// Build an engine from a list of database entries.
    pub fn new(entries: Vec<AircraftEntry>) -> Self {
        let mut model_index = HashMap::new();
        for (idx, entry) in entries.iter().enumerate() {
            for model in &entry.known_atc_models {
                model_index.insert(model.to_lowercase(), idx);
            }
        }
        Self {
            entries,
            model_index,
        }
    }

    /// Build the default engine pre-populated with common MSFS aircraft.
    pub fn default_msfs() -> Self {
        Self::new(default_aircraft_entries())
    }

    /// Detect aircraft from sim data fields.
    pub fn detect(&self, data: &SimAircraftData) -> DetectionResult {
        // 1. Fast path: exact ATC_MODEL match.
        if !data.atc_model.is_empty()
            && let Some(&idx) = self.model_index.get(&data.atc_model.to_lowercase())
        {
            let entry = &self.entries[idx];
            return DetectionResult {
                icao: Some(entry.icao.clone()),
                display_name: Some(entry.display_name.clone()),
                confidence: MatchConfidence::Exact,
                indicator_scores: IndicatorScores {
                    title_score: 0.0,
                    atc_type_score: 0.0,
                    atc_model_score: 1.0,
                },
                is_community_mod: self.looks_like_community_mod(&data.title),
            };
        }

        // 2. Score every entry and keep the best.
        let mut best: Option<(usize, f32, IndicatorScores)> = None;

        for (idx, entry) in self.entries.iter().enumerate() {
            let scores = self.score_entry(entry, data);
            let combined = scores.title_score * 0.4
                + scores.atc_type_score * 0.3
                + scores.atc_model_score * 0.3;

            if let Some((_, best_score, _)) = &best {
                if combined > *best_score {
                    best = Some((idx, combined, scores));
                }
            } else if combined > 0.0 {
                best = Some((idx, combined, scores));
            }
        }

        match best {
            Some((idx, score, scores)) => {
                let entry = &self.entries[idx];
                let confidence = if score >= 0.8 {
                    MatchConfidence::High
                } else if score >= 0.5 {
                    MatchConfidence::Medium
                } else {
                    MatchConfidence::Low
                };
                let is_community_mod = self.looks_like_community_mod(&data.title);
                DetectionResult {
                    icao: Some(entry.icao.clone()),
                    display_name: Some(entry.display_name.clone()),
                    confidence,
                    indicator_scores: scores,
                    is_community_mod,
                }
            }
            None => DetectionResult {
                icao: None,
                display_name: None,
                confidence: MatchConfidence::None,
                indicator_scores: IndicatorScores::default(),
                is_community_mod: self.looks_like_community_mod(&data.title),
            },
        }
    }

    /// Number of entries in the database.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    // -- scoring helpers --

    fn score_entry(&self, entry: &AircraftEntry, data: &SimAircraftData) -> IndicatorScores {
        let title_score = self.best_fuzzy_score(&entry.known_titles, &data.title);
        let atc_type_score = self.best_fuzzy_score(&entry.known_atc_types, &data.atc_type);
        let atc_model_score = self.best_fuzzy_score(&entry.known_atc_models, &data.atc_model);
        IndicatorScores {
            title_score,
            atc_type_score,
            atc_model_score,
        }
    }

    fn best_fuzzy_score(&self, patterns: &[String], input: &str) -> f32 {
        if input.is_empty() {
            return 0.0;
        }
        let input_lower = input.to_lowercase();
        patterns
            .iter()
            .map(|p| fuzzy_score(&p.to_lowercase(), &input_lower))
            .fold(0.0_f32, f32::max)
    }

    fn looks_like_community_mod(&self, title: &str) -> bool {
        let lower = title.to_lowercase();
        // Community mods often contain these patterns.
        let mod_indicators = [
            "livery",
            "mod",
            "addon",
            "custom",
            "repaint",
            "package",
            "workingtitle",
            "flybywire",
            "pmdg",
            "fenix",
            "inibuilds",
            "justflight",
            "aerosoft",
            "blacksquare",
            "milviz",
            "carenado",
        ];
        mod_indicators.iter().any(|kw| lower.contains(kw))
    }
}

impl Default for AircraftDetectionEngine {
    fn default() -> Self {
        Self::default_msfs()
    }
}

// ---------------------------------------------------------------------------
// Fuzzy scoring
// ---------------------------------------------------------------------------

/// Compute a 0.0–1.0 similarity score between `pattern` and `input`.
/// Both should already be lowercased by the caller.
fn fuzzy_score(pattern: &str, input: &str) -> f32 {
    if pattern == input {
        return 1.0;
    }
    if pattern.is_empty() || input.is_empty() {
        return 0.0;
    }
    // Substring containment.
    if input.contains(pattern) {
        let ratio = pattern.len() as f32 / input.len() as f32;
        return 0.6 + 0.3 * ratio; // 0.6 … 0.9
    }
    if pattern.contains(input) {
        let ratio = input.len() as f32 / pattern.len() as f32;
        return 0.4 + 0.3 * ratio;
    }
    // Token overlap (Jaccard-ish).
    let p_tokens: Vec<&str> = pattern.split_whitespace().collect();
    let i_tokens: Vec<&str> = input.split_whitespace().collect();
    if p_tokens.is_empty() || i_tokens.is_empty() {
        return 0.0;
    }
    let common = p_tokens.iter().filter(|t| i_tokens.contains(t)).count();
    let union = p_tokens.len().max(i_tokens.len());
    common as f32 / union as f32 * 0.6
}

// ---------------------------------------------------------------------------
// Default database
// ---------------------------------------------------------------------------

fn default_aircraft_entries() -> Vec<AircraftEntry> {
    vec![
        // General Aviation
        AircraftEntry {
            icao: "C172".into(),
            display_name: "Cessna 172 Skyhawk".into(),
            known_titles: vec![
                "Cessna Skyhawk".into(),
                "Cessna 172 Skyhawk".into(),
                "Cessna 172".into(),
            ],
            known_atc_types: vec!["CESSNA".into(), "Cessna".into()],
            known_atc_models: vec!["C172".into(), "C172S".into()],
            tags: vec!["ga".into(), "single-engine".into(), "piston".into()],
        },
        AircraftEntry {
            icao: "C152".into(),
            display_name: "Cessna 152".into(),
            known_titles: vec!["Cessna 152".into()],
            known_atc_types: vec!["CESSNA".into()],
            known_atc_models: vec!["C152".into()],
            tags: vec!["ga".into(), "single-engine".into(), "trainer".into()],
        },
        AircraftEntry {
            icao: "DA40".into(),
            display_name: "Diamond DA40 NG".into(),
            known_titles: vec!["Diamond DA40".into(), "DA40 NG".into()],
            known_atc_types: vec!["DIAMOND".into()],
            known_atc_models: vec!["DA40".into()],
            tags: vec!["ga".into(), "single-engine".into()],
        },
        AircraftEntry {
            icao: "DA62".into(),
            display_name: "Diamond DA62".into(),
            known_titles: vec!["Diamond DA62".into()],
            known_atc_types: vec!["DIAMOND".into()],
            known_atc_models: vec!["DA62".into()],
            tags: vec!["ga".into(), "twin-engine".into()],
        },
        AircraftEntry {
            icao: "PA28".into(),
            display_name: "Piper PA-28 Cherokee".into(),
            known_titles: vec![
                "Piper Cherokee".into(),
                "Piper PA-28".into(),
                "PA-28 Cherokee".into(),
            ],
            known_atc_types: vec!["PIPER".into()],
            known_atc_models: vec!["PA28".into(), "P28A".into()],
            tags: vec!["ga".into(), "single-engine".into()],
        },
        // Turboprops
        AircraftEntry {
            icao: "TBM9".into(),
            display_name: "Daher TBM 930".into(),
            known_titles: vec!["TBM 930".into(), "Daher TBM".into()],
            known_atc_types: vec!["DAHER".into(), "TBM".into()],
            known_atc_models: vec!["TBM9".into(), "TBM930".into()],
            tags: vec!["ga".into(), "turboprop".into()],
        },
        AircraftEntry {
            icao: "BE58".into(),
            display_name: "Beechcraft Baron G58".into(),
            known_titles: vec!["Beechcraft Baron".into(), "Baron G58".into()],
            known_atc_types: vec!["BEECHCRAFT".into(), "BEECH".into()],
            known_atc_models: vec!["BE58".into()],
            tags: vec!["ga".into(), "twin-engine".into(), "piston".into()],
        },
        // Commercial
        AircraftEntry {
            icao: "A320".into(),
            display_name: "Airbus A320neo".into(),
            known_titles: vec![
                "Airbus A320neo".into(),
                "A320neo".into(),
                "Airbus A320".into(),
            ],
            known_atc_types: vec!["AIRBUS".into()],
            known_atc_models: vec!["A320".into(), "A20N".into()],
            tags: vec!["airliner".into(), "narrowbody".into(), "jet".into()],
        },
        AircraftEntry {
            icao: "B738".into(),
            display_name: "Boeing 737-800".into(),
            known_titles: vec!["Boeing 737-800".into(), "737-800".into()],
            known_atc_types: vec!["BOEING".into()],
            known_atc_models: vec!["B738".into(), "B737".into()],
            tags: vec!["airliner".into(), "narrowbody".into(), "jet".into()],
        },
        AircraftEntry {
            icao: "B748".into(),
            display_name: "Boeing 747-8".into(),
            known_titles: vec!["Boeing 747-8".into(), "747-8 Intercontinental".into()],
            known_atc_types: vec!["BOEING".into()],
            known_atc_models: vec!["B748".into(), "B747".into()],
            tags: vec!["airliner".into(), "widebody".into(), "jet".into()],
        },
        // Helicopters
        AircraftEntry {
            icao: "R22".into(),
            display_name: "Robinson R22".into(),
            known_titles: vec!["Robinson R22".into()],
            known_atc_types: vec!["ROBINSON".into()],
            known_atc_models: vec!["R22".into()],
            tags: vec!["helicopter".into(), "training".into()],
        },
        AircraftEntry {
            icao: "H135".into(),
            display_name: "Airbus H135".into(),
            known_titles: vec!["Airbus H135".into(), "H135".into()],
            known_atc_types: vec!["AIRBUS HELICOPTERS".into(), "EUROCOPTER".into()],
            known_atc_models: vec!["H135".into(), "EC35".into()],
            tags: vec!["helicopter".into()],
        },
    ]
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> AircraftDetectionEngine {
        AircraftDetectionEngine::default_msfs()
    }

    // -- exact ATC_MODEL match --

    #[test]
    fn exact_atc_model_match() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData {
            title: String::new(),
            atc_type: String::new(),
            atc_model: "C172".into(),
        });
        assert_eq!(result.confidence, MatchConfidence::Exact);
        assert_eq!(result.icao, Some("C172".into()));
        assert!(!result.is_community_mod);
    }

    #[test]
    fn exact_atc_model_case_insensitive() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData {
            title: String::new(),
            atc_type: String::new(),
            atc_model: "c172".into(),
        });
        assert_eq!(result.confidence, MatchConfidence::Exact);
        assert_eq!(result.icao, Some("C172".into()));
    }

    #[test]
    fn exact_alternate_model_code() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData {
            title: String::new(),
            atc_type: String::new(),
            atc_model: "A20N".into(),
        });
        assert_eq!(result.confidence, MatchConfidence::Exact);
        assert_eq!(result.icao, Some("A320".into()));
    }

    // -- fuzzy title matching --

    #[test]
    fn fuzzy_title_match_cessna() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData {
            title: "Cessna 172 Skyhawk G1000 NXi".into(),
            atc_type: String::new(),
            atc_model: String::new(),
        });
        assert!(result.confidence >= MatchConfidence::Low);
        assert_eq!(result.icao, Some("C172".into()));
    }

    #[test]
    fn fuzzy_title_match_boeing() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData {
            title: "Boeing 747-8 Intercontinental".into(),
            atc_type: String::new(),
            atc_model: String::new(),
        });
        assert!(result.confidence >= MatchConfidence::Low);
        assert_eq!(result.icao, Some("B748".into()));
    }

    #[test]
    fn fuzzy_atc_type_match() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData {
            title: String::new(),
            atc_type: "CESSNA".into(),
            atc_model: String::new(),
        });
        // Multiple Cessna aircraft exist; any match is fine.
        assert!(result.confidence >= MatchConfidence::Low);
        assert!(result.icao.is_some());
    }

    // -- multi-indicator confidence --

    #[test]
    fn multi_indicator_high_confidence() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData {
            title: "Airbus A320neo".into(),
            atc_type: "AIRBUS".into(),
            atc_model: String::new(),
        });
        assert!(result.confidence >= MatchConfidence::Medium);
        assert_eq!(result.icao, Some("A320".into()));
    }

    // -- no match --

    #[test]
    fn no_match_returns_none() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData {
            title: "Completely Unknown Aircraft XYZ-9999".into(),
            atc_type: "UNKNOWN_MFR".into(),
            atc_model: "ZZZZ".into(),
        });
        assert_eq!(result.confidence, MatchConfidence::None);
        assert!(result.icao.is_none());
    }

    #[test]
    fn empty_data_returns_none() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData::default());
        assert_eq!(result.confidence, MatchConfidence::None);
    }

    // -- community mod detection --

    #[test]
    fn community_mod_detected_flybywire() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData {
            title: "FlyByWire A320neo (LEAP)".into(),
            atc_type: "AIRBUS".into(),
            atc_model: "A320".into(),
        });
        assert_eq!(result.confidence, MatchConfidence::Exact);
        assert!(result.is_community_mod);
    }

    #[test]
    fn community_mod_detected_pmdg() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData {
            title: "PMDG 737-800 NGXu".into(),
            atc_type: "BOEING".into(),
            atc_model: "B738".into(),
        });
        assert!(result.is_community_mod);
    }

    #[test]
    fn standard_aircraft_not_community_mod() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData {
            title: "Cessna 172 Skyhawk".into(),
            atc_type: "CESSNA".into(),
            atc_model: "C172".into(),
        });
        assert!(!result.is_community_mod);
    }

    // -- fuzzy score unit tests --

    #[test]
    fn fuzzy_score_exact() {
        assert!((fuzzy_score("c172", "c172") - 1.0).abs() < f64::EPSILON as f32);
    }

    #[test]
    fn fuzzy_score_substring() {
        let s = fuzzy_score("cessna", "cessna 172 skyhawk");
        assert!(s >= 0.6 && s <= 0.9, "got {s}");
    }

    #[test]
    fn fuzzy_score_empty() {
        assert_eq!(fuzzy_score("", "hello"), 0.0);
        assert_eq!(fuzzy_score("hello", ""), 0.0);
    }

    #[test]
    fn fuzzy_score_no_overlap() {
        let s = fuzzy_score("xyz", "abc");
        assert!(s < 0.01, "got {s}");
    }

    // -- engine metadata --

    #[test]
    fn default_engine_has_entries() {
        let eng = engine();
        assert!(
            eng.entry_count() >= 10,
            "expected ≥10 default entries, got {}",
            eng.entry_count()
        );
    }

    // -- indicator scores --

    #[test]
    fn indicator_scores_default() {
        let scores = IndicatorScores::default();
        assert_eq!(scores.title_score, 0.0);
        assert_eq!(scores.atc_type_score, 0.0);
        assert_eq!(scores.atc_model_score, 0.0);
    }

    // -- MatchConfidence ordering --

    #[test]
    fn confidence_ordering() {
        assert!(MatchConfidence::Exact > MatchConfidence::High);
        assert!(MatchConfidence::High > MatchConfidence::Medium);
        assert!(MatchConfidence::Medium > MatchConfidence::Low);
        assert!(MatchConfidence::Low > MatchConfidence::None);
    }

    // -- helicopter detection --

    #[test]
    fn helicopter_detection() {
        let eng = engine();
        let result = eng.detect(&SimAircraftData {
            title: "Robinson R22".into(),
            atc_type: "ROBINSON".into(),
            atc_model: "R22".into(),
        });
        assert_eq!(result.confidence, MatchConfidence::Exact);
        assert_eq!(result.icao, Some("R22".into()));
    }
}
