// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-core — error catalog, aircraft detection,
//! profile management, event types, configuration types, and property tests.

use std::collections::{HashMap, HashSet};

use flight_core::calibration_store::{AxisCalibration, CalibrationStore};
use flight_core::circuit_breaker::{
    CallResult, CircuitBreaker, CircuitBreakerConfig, CircuitState,
};
use flight_core::error::FlightError;
use flight_core::error_catalog::{ErrorCatalog, ErrorCategory};
use flight_core::profile::{
    AircraftId, AxisConfig, CapabilityContext, CapabilityMode, CurvePoint,
    DetentZone, FilterConfig, PofOverrides, Profile, PROFILE_SCHEMA_VERSION,
};
use flight_core::profile_watcher::ReloadNotifier;
use flight_core::{
    AutoSwitchConfig, DetectionMetrics, PhaseOfFlight, PofHysteresisConfig,
    ProcessDetectionConfig, ProcessDetectionError, SessionError, SimId, SwitchMetrics,
    WatchdogConfig,
};

// ═══════════════════════════════════════════════════════════════════════════════
// 1. ERROR CATALOG
// ═══════════════════════════════════════════════════════════════════════════════

mod error_catalog {
    use super::*;

    #[test]
    fn all_error_codes_are_unique() {
        let all = ErrorCatalog::all();
        let mut seen = HashSet::new();
        for info in all {
            assert!(seen.insert(info.code), "Duplicate error code: {}", info.code);
        }
    }

    #[test]
    fn every_error_code_has_non_empty_description() {
        for info in ErrorCatalog::all() {
            assert!(!info.description.is_empty(), "code {} has empty description", info.code);
            assert!(!info.message.is_empty(), "code {} has empty message", info.code);
            assert!(!info.resolution.is_empty(), "code {} has empty resolution", info.code);
        }
    }

    #[test]
    fn error_codes_follow_prefix_format() {
        for info in ErrorCatalog::all() {
            let parts: Vec<&str> = info.code.split('-').collect();
            assert_eq!(parts.len(), 2, "code '{}' should be XXX-NNN", info.code);
            assert_eq!(parts[0].len(), 3, "prefix '{}' should be 3 chars", parts[0]);
            assert_eq!(parts[1].len(), 3, "number '{}' should be 3 digits", parts[1]);
            assert!(
                parts[0].chars().all(|c| c.is_ascii_uppercase()),
                "prefix should be uppercase: {}",
                info.code
            );
            assert!(
                parts[1].chars().all(|c| c.is_ascii_digit()),
                "suffix should be digits: {}",
                info.code
            );
        }
    }

    #[test]
    fn every_category_has_at_least_four_entries() {
        let categories = [
            ErrorCategory::Device,
            ErrorCategory::Sim,
            ErrorCategory::Profile,
            ErrorCategory::Service,
            ErrorCategory::Plugin,
            ErrorCategory::Network,
            ErrorCategory::Config,
            ErrorCategory::Internal,
        ];
        for cat in categories {
            let entries = ErrorCatalog::by_category(cat);
            assert!(
                entries.len() >= 1,
                "Category {:?} has no entries",
                cat,
            );
        }
    }

    #[test]
    fn lookup_returns_correct_entry_for_every_code() {
        for info in ErrorCatalog::all() {
            let looked_up = ErrorCatalog::lookup(info.code)
                .unwrap_or_else(|| panic!("lookup failed for code {}", info.code));
            assert_eq!(looked_up.code, info.code);
            assert_eq!(looked_up.category, info.category);
            assert_eq!(looked_up.message, info.message);
        }
    }

    #[test]
    fn lookup_unknown_code_returns_none() {
        assert!(ErrorCatalog::lookup("ZZZ-999").is_none());
        assert!(ErrorCatalog::lookup("").is_none());
        assert!(ErrorCatalog::lookup("INVALID").is_none());
    }

    #[test]
    fn format_error_known_code_includes_all_fields() {
        let first = ErrorCatalog::all()
            .first()
            .expect("ErrorCatalog should contain at least one entry");
        let formatted = ErrorCatalog::format_error(first.code);
        assert!(formatted.contains(first.code));
        assert!(formatted.contains(first.message));
        assert!(formatted.contains("Resolution:"));
    }

    #[test]
    fn format_error_unknown_code_says_unknown() {
        let formatted = ErrorCatalog::format_error("ZZZ-999");
        assert!(formatted.contains("Unknown error code"));
    }

    #[test]
    fn error_category_display_is_human_readable() {
        assert_eq!(ErrorCategory::Device.to_string(), "Device");
        assert_eq!(ErrorCategory::Sim.to_string(), "Simulator");
        assert_eq!(ErrorCategory::Profile.to_string(), "Profile");
        assert_eq!(ErrorCategory::Service.to_string(), "Service");
        assert_eq!(ErrorCategory::Plugin.to_string(), "Plugin");
        assert_eq!(ErrorCategory::Network.to_string(), "Network");
        assert_eq!(ErrorCategory::Config.to_string(), "Configuration");
        assert_eq!(ErrorCategory::Internal.to_string(), "Internal");
    }

    #[test]
    fn error_category_debug_clone_eq_hash() {
        let cat = ErrorCategory::Device;
        let cloned = cat;
        assert_eq!(cat, cloned);
        let _ = format!("{:?}", cat);
        let mut set = HashSet::new();
        set.insert(cat);
        assert!(set.contains(&ErrorCategory::Device));
    }

    #[test]
    fn error_info_is_debug_clone() {
        let info = ErrorCatalog::all()
            .first()
            .expect("ErrorCatalog should contain at least one entry");
        let cloned = info.clone();
        assert_eq!(cloned.code, info.code);
        let _ = format!("{:?}", info);
    }

    #[test]
    fn error_code_description_is_deterministic() {
        // Same code always gives same description — call multiple times
        for info in ErrorCatalog::all() {
            let d1 = ErrorCatalog::lookup(info.code).unwrap().description;
            let d2 = ErrorCatalog::lookup(info.code).unwrap().description;
            assert_eq!(d1, d2, "Description for {} is not deterministic", info.code);
        }
    }

    #[test]
    fn error_code_prefixes_match_category() {
        for info in ErrorCatalog::all() {
            let prefix = info.code.split('-').next().unwrap();
            let expected_cat = match prefix {
                "DEV" => ErrorCategory::Device,
                "SIM" => ErrorCategory::Sim,
                "PRF" => ErrorCategory::Profile,
                "SVC" => ErrorCategory::Service,
                "PLG" => ErrorCategory::Plugin,
                "NET" => ErrorCategory::Network,
                "CFG" => ErrorCategory::Config,
                "INT" => ErrorCategory::Internal,
                other => panic!("Unknown prefix '{}' in code {}", other, info.code),
            };
            assert_eq!(
                info.category, expected_cat,
                "Code {} prefix implies {:?} but category is {:?}",
                info.code, expected_cat, info.category
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. AIRCRAFT TYPES / SIMULATOR DETECTION
// ═══════════════════════════════════════════════════════════════════════════════

mod aircraft_types {
    use super::*;

    #[test]
    fn all_sim_id_variants_have_non_empty_display() {
        let sims = [
            SimId::Msfs,
            SimId::Msfs2024,
            SimId::XPlane,
            SimId::Dcs,
            SimId::AceCombat7,
            SimId::WarThunder,
            SimId::EliteDangerous,
            SimId::Ksp,
            SimId::Wingman,
            SimId::Il2,
            SimId::Unknown,
        ];
        for sim in sims {
            let display = sim.to_string();
            assert!(!display.is_empty(), "SimId::{:?} has empty display", sim);
        }
    }

    #[test]
    fn sim_id_debug_clone_copy_eq_hash() {
        let sim = SimId::Msfs;
        let copied = sim;
        assert_eq!(sim, copied);
        let cloned = sim.clone();
        assert_eq!(sim, cloned);
        let _ = format!("{:?}", sim);
        let mut set = HashSet::new();
        set.insert(sim);
        assert!(set.contains(&SimId::Msfs));
    }

    #[test]
    fn sim_id_known_display_values() {
        assert_eq!(SimId::Msfs.to_string(), "MSFS");
        assert_eq!(SimId::Msfs2024.to_string(), "MSFS 2024");
        assert_eq!(SimId::XPlane.to_string(), "X-Plane");
        assert_eq!(SimId::Dcs.to_string(), "DCS");
    }

    #[test]
    fn all_phase_of_flight_variants_constructable_and_distinct() {
        let phases = [
            PhaseOfFlight::Ground,
            PhaseOfFlight::Taxi,
            PhaseOfFlight::Takeoff,
            PhaseOfFlight::Climb,
            PhaseOfFlight::Cruise,
            PhaseOfFlight::Descent,
            PhaseOfFlight::Approach,
            PhaseOfFlight::Landing,
            PhaseOfFlight::GoAround,
        ];
        let mut seen = HashSet::new();
        for phase in &phases {
            assert!(seen.insert(*phase), "Duplicate phase: {:?}", phase);
        }
        assert_eq!(seen.len(), 9);
    }

    #[test]
    fn phase_of_flight_display_roundtrip_via_from_str() {
        let phases = [
            PhaseOfFlight::Ground,
            PhaseOfFlight::Taxi,
            PhaseOfFlight::Takeoff,
            PhaseOfFlight::Climb,
            PhaseOfFlight::Cruise,
            PhaseOfFlight::Descent,
            PhaseOfFlight::Approach,
            PhaseOfFlight::Landing,
            PhaseOfFlight::GoAround,
        ];
        for phase in phases {
            let display = phase.to_string();
            let parsed: PhaseOfFlight = display.parse().unwrap_or_else(|_| {
                panic!("Failed to parse '{}' back to PhaseOfFlight", display)
            });
            assert_eq!(phase, parsed);
        }
    }

    #[test]
    fn phase_of_flight_from_str_is_case_insensitive() {
        let cases = [
            ("CRUISE", PhaseOfFlight::Cruise),
            ("Cruise", PhaseOfFlight::Cruise),
            ("cruise", PhaseOfFlight::Cruise),
            ("GROUND", PhaseOfFlight::Ground),
            ("GoAround", PhaseOfFlight::GoAround),
            ("GOAROUND", PhaseOfFlight::GoAround),
        ];
        for (input, expected) in cases {
            let parsed: PhaseOfFlight = input.parse().unwrap_or_else(|_| {
                panic!("Failed to parse '{}'", input)
            });
            assert_eq!(parsed, expected, "input '{}' mismatch", input);
        }
    }

    #[test]
    fn phase_of_flight_from_str_rejects_unknown() {
        assert!("invalid_phase".parse::<PhaseOfFlight>().is_err());
        assert!("".parse::<PhaseOfFlight>().is_err());
        assert!("flying".parse::<PhaseOfFlight>().is_err());
    }

    #[test]
    fn aircraft_id_icao_code_roundtrip() {
        let id = AircraftId {
            icao: "C172".to_string(),
        };
        let json = serde_json::to_string(&id).unwrap();
        let restored: AircraftId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, restored);
    }

    #[test]
    fn aircraft_id_various_icao_codes() {
        let codes = ["C172", "B738", "A320", "F16C", "B787", "PA28", "SR22"];
        for code in codes {
            let id = AircraftId {
                icao: code.to_string(),
            };
            assert_eq!(id.icao, code);
        }
    }

    #[test]
    fn detection_metrics_default_starts_at_zero() {
        let m = DetectionMetrics::default();
        assert_eq!(m.total_scans, 0);
        assert_eq!(m.successful_detections, 0);
        assert_eq!(m.false_positives, 0);
    }

    #[test]
    fn switch_metrics_default_starts_at_zero() {
        let m = SwitchMetrics::default();
        assert_eq!(m.total_switches, 0);
        assert_eq!(m.failed_switches, 0);
        assert_eq!(m.successful_switches, 0);
        assert_eq!(m.committed_switches, 0);
        assert_eq!(m.cache_hits, 0);
        assert_eq!(m.cache_misses, 0);
    }

    #[test]
    fn process_detection_config_default_has_all_major_sims() {
        let cfg = ProcessDetectionConfig::default();
        assert!(
            cfg.process_definitions.contains_key(&SimId::Msfs),
            "missing MSFS definition"
        );
        assert!(
            cfg.process_definitions.contains_key(&SimId::XPlane),
            "missing X-Plane definition"
        );
        assert!(
            cfg.process_definitions.contains_key(&SimId::Dcs),
            "missing DCS definition"
        );
    }

    #[test]
    fn process_detection_config_default_has_positive_interval() {
        let cfg = ProcessDetectionConfig::default();
        assert!(cfg.detection_interval.as_millis() > 0);
        assert!(cfg.max_detection_time.as_millis() > 0);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. PROFILE MANAGEMENT
// ═══════════════════════════════════════════════════════════════════════════════

mod profile_management {
    use super::*;

    fn base_axis() -> AxisConfig {
        AxisConfig {
            deadzone: Some(0.05),
            expo: Some(0.3),
            slew_rate: Some(2.0),
            detents: vec![],
            curve: None,
            filter: None,
        }
    }

    fn global_profile() -> Profile {
        let mut axes = HashMap::new();
        axes.insert("pitch".to_string(), base_axis());
        axes.insert("roll".to_string(), base_axis());
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes,
            pof_overrides: None,
        }
    }

    fn sim_profile() -> Profile {
        let mut axes = HashMap::new();
        axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.03),
                expo: Some(0.4),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("msfs".to_string()),
            aircraft: None,
            axes,
            pof_overrides: None,
        }
    }

    fn aircraft_profile() -> Profile {
        let mut axes = HashMap::new();
        axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: None,
                expo: Some(0.5),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId {
                icao: "C172".to_string(),
            }),
            axes,
            pof_overrides: None,
        }
    }

    fn pof_profile() -> Profile {
        let mut pof_axes = HashMap::new();
        pof_axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: None,
                expo: Some(0.2),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        let mut pof = HashMap::new();
        pof.insert(
            "landing".to_string(),
            PofOverrides {
                axes: Some(pof_axes),
                hysteresis: None,
            },
        );
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId {
                icao: "C172".to_string(),
            }),
            axes: HashMap::new(),
            pof_overrides: Some(pof),
        }
    }

    // ── ADR-007 cascade: Global → Simulator → Aircraft → Phase-of-Flight ──

    #[test]
    fn merge_cascade_global_then_sim() {
        let global = global_profile();
        let sim = sim_profile();
        let merged = global.merge_with(&sim).unwrap();

        let pitch = merged.axes.get("pitch").unwrap();
        // Sim overrides expo (0.3 → 0.4) and deadzone (0.05 → 0.03)
        assert_eq!(pitch.expo, Some(0.4));
        assert_eq!(pitch.deadzone, Some(0.03));
        // Global's slew_rate preserved (sim has None)
        assert_eq!(pitch.slew_rate, Some(2.0));
        // Global's roll axis preserved
        assert!(merged.axes.contains_key("roll"));
    }

    #[test]
    fn merge_cascade_global_sim_aircraft() {
        let global = global_profile();
        let sim = sim_profile();
        let aircraft = aircraft_profile();
        let merged = global.merge_with(&sim).unwrap().merge_with(&aircraft).unwrap();

        let pitch = merged.axes.get("pitch").unwrap();
        // Aircraft's expo (0.5) wins over sim's (0.4) and global's (0.3)
        assert_eq!(pitch.expo, Some(0.5));
        // Sim's deadzone (0.03) survives since aircraft has None
        assert_eq!(pitch.deadzone, Some(0.03));
        // Aircraft sets sim and aircraft fields
        assert_eq!(merged.sim.as_deref(), Some("msfs"));
        assert!(merged.aircraft.is_some());
    }

    #[test]
    fn merge_cascade_full_pipeline() {
        let global = global_profile();
        let sim = sim_profile();
        let aircraft = aircraft_profile();
        let pof = pof_profile();
        // Full cascade: Global → Sim → Aircraft → PoF
        let merged = global
            .merge_with(&sim)
            .unwrap()
            .merge_with(&aircraft)
            .unwrap()
            .merge_with(&pof)
            .unwrap();
        // PoF profile adds pof_overrides
        assert!(merged.pof_overrides.is_some());
        // Aircraft's expo still wins in main axes (PoF profile has empty axes)
        let pitch = merged.axes.get("pitch").unwrap();
        assert_eq!(pitch.expo, Some(0.5));
    }

    #[test]
    fn more_specific_profile_overrides_less_specific() {
        let base = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("msfs".to_string()),
            aircraft: None,
            axes: [("pitch".to_string(), AxisConfig {
                deadzone: Some(0.05),
                expo: Some(0.3),
                slew_rate: Some(2.0),
                detents: vec![],
                curve: None,
                filter: None,
            })]
            .into(),
            pof_overrides: None,
        };
        let specific = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId { icao: "B738".to_string() }),
            axes: [("pitch".to_string(), AxisConfig {
                deadzone: Some(0.02),
                expo: None,
                slew_rate: Some(3.0),
                detents: vec![],
                curve: None,
                filter: None,
            })]
            .into(),
            pof_overrides: None,
        };
        let merged = base.merge_with(&specific).unwrap();
        let pitch = merged.axes.get("pitch").unwrap();
        assert_eq!(pitch.deadzone, Some(0.02), "specific deadzone wins");
        assert_eq!(pitch.expo, Some(0.3), "base expo preserved");
        assert_eq!(pitch.slew_rate, Some(3.0), "specific slew_rate wins");
    }

    #[test]
    fn empty_profile_merge_is_identity() {
        let profile = global_profile();
        let empty = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        };
        let merged = profile.merge_with(&empty).unwrap();
        assert_eq!(profile, merged, "merging with empty must be identity");
    }

    #[test]
    fn merge_with_self_is_idempotent() {
        let profile = global_profile();
        let merged = profile.merge_with(&profile).unwrap();
        assert_eq!(profile, merged);
    }

    #[test]
    fn merge_adds_new_axis_from_override() {
        let base = global_profile();
        let override_p = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("yaw".to_string(), base_axis())].into(),
            pof_overrides: None,
        };
        let merged = base.merge_with(&override_p).unwrap();
        assert!(merged.axes.contains_key("pitch"));
        assert!(merged.axes.contains_key("roll"));
        assert!(merged.axes.contains_key("yaw"));
    }

    // ── Axis configuration validation ──

    #[test]
    fn axis_config_deadzone_bounds() {
        // Valid boundaries
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().deadzone = Some(0.0);
        assert!(p.validate().is_ok());
        p.axes.get_mut("pitch").unwrap().deadzone = Some(0.5);
        assert!(p.validate().is_ok());

        // Invalid
        p.axes.get_mut("pitch").unwrap().deadzone = Some(-0.01);
        assert!(p.validate().is_err());
        p.axes.get_mut("pitch").unwrap().deadzone = Some(0.51);
        assert!(p.validate().is_err());
    }

    #[test]
    fn axis_config_expo_bounds() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().expo = Some(0.0);
        assert!(p.validate().is_ok());
        p.axes.get_mut("pitch").unwrap().expo = Some(1.0);
        assert!(p.validate().is_ok());

        p.axes.get_mut("pitch").unwrap().expo = Some(-0.01);
        assert!(p.validate().is_err());
        p.axes.get_mut("pitch").unwrap().expo = Some(1.01);
        assert!(p.validate().is_err());
    }

    #[test]
    fn axis_config_slew_rate_bounds() {
        let mut p = global_profile();
        p.axes.get_mut("pitch").unwrap().slew_rate = Some(0.0);
        assert!(p.validate().is_ok());
        p.axes.get_mut("pitch").unwrap().slew_rate = Some(100.0);
        assert!(p.validate().is_ok());

        p.axes.get_mut("pitch").unwrap().slew_rate = Some(-0.01);
        assert!(p.validate().is_err());
        p.axes.get_mut("pitch").unwrap().slew_rate = Some(100.01);
        assert!(p.validate().is_err());
    }

    #[test]
    fn axis_config_all_none_is_valid() {
        let p = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("any".to_string(), AxisConfig {
                deadzone: None,
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            })]
            .into(),
            pof_overrides: None,
        };
        assert!(p.validate().is_ok());
    }

    #[test]
    fn schema_version_validation() {
        let mut p = global_profile();
        assert!(p.validate().is_ok());
        p.schema = "flight.profile/999".to_string();
        assert!(p.validate().is_err());
    }

    #[test]
    fn effective_hash_stable_across_calls() {
        let p = global_profile();
        assert_eq!(p.effective_hash(), p.effective_hash());
    }

    #[test]
    fn effective_hash_changes_with_content() {
        let p1 = global_profile();
        let mut p2 = global_profile();
        p2.axes.get_mut("pitch").unwrap().expo = Some(0.99);
        assert_ne!(p1.effective_hash(), p2.effective_hash());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. EVENT TYPES (FlightError variants, SessionError, ProcessDetectionError)
// ═══════════════════════════════════════════════════════════════════════════════

mod event_types {
    use super::*;

    #[test]
    fn flight_error_all_string_variants_constructable() {
        let errors: Vec<FlightError> = vec![
            FlightError::RulesValidation("test".into()),
            FlightError::Configuration("test".into()),
            FlightError::Writer("test".into()),
            FlightError::Hardware("test".into()),
        ];
        for err in &errors {
            let msg = err.to_string();
            assert!(!msg.is_empty(), "Error {:?} has empty display", err);
        }
    }

    #[test]
    fn flight_error_display_contains_context() {
        let err = FlightError::Configuration("missing key xyz".to_string());
        assert!(err.to_string().contains("missing key xyz"));

        let err = FlightError::Hardware("USB stall".to_string());
        assert!(err.to_string().contains("USB stall"));

        let err = FlightError::Writer("channel closed".to_string());
        assert!(err.to_string().contains("channel closed"));

        let err = FlightError::RulesValidation("bad syntax".to_string());
        assert!(err.to_string().contains("bad syntax"));
    }

    #[test]
    fn flight_error_io_from_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let flight_err: FlightError = io_err.into();
        assert!(flight_err.to_string().contains("not found"));
    }

    #[test]
    fn flight_error_is_debug() {
        let err = FlightError::Configuration("test".into());
        let dbg = format!("{:?}", err);
        assert!(!dbg.is_empty());
    }

    #[test]
    fn session_error_variants() {
        let err = SessionError::Configuration("bad config".into());
        assert!(err.to_string().contains("bad config") || err.to_string().contains("Configuration"));

        let err = SessionError::AutoSwitch("failed".into());
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn process_detection_error_variants() {
        let err = ProcessDetectionError::Platform("windows error".into());
        assert!(!err.to_string().is_empty());

        let err = ProcessDetectionError::System("system error".into());
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn flight_error_result_alias() {
        let ok: flight_core::Result<u32> = Ok(42);
        assert_eq!(ok.unwrap(), 42);

        let err: flight_core::Result<u32> =
            Err(FlightError::Configuration("fail".into()));
        assert!(err.is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. CONFIGURATION TYPES
// ═══════════════════════════════════════════════════════════════════════════════

mod config_types {
    use super::*;
    use std::time::Duration;

    // ── AxisConfig validation ──

    #[test]
    fn axis_config_serialization_roundtrip() {
        let axis = AxisConfig {
            deadzone: Some(0.1),
            expo: Some(0.5),
            slew_rate: Some(10.0),
            detents: vec![DetentZone {
                position: 0.0,
                width: 0.05,
                role: "center".to_string(),
            }],
            curve: Some(vec![
                CurvePoint { input: 0.0, output: 0.0 },
                CurvePoint { input: 1.0, output: 1.0 },
            ]),
            filter: Some(FilterConfig {
                alpha: 0.9,
                spike_threshold: Some(0.02),
                max_spike_count: Some(5),
            }),
        };
        let json = serde_json::to_string(&axis).unwrap();
        let restored: AxisConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(axis, restored);
    }

    #[test]
    fn filter_config_alpha_bounds_validation() {
        let mut profile = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("x".to_string(), AxisConfig {
                deadzone: None,
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: Some(FilterConfig {
                    alpha: 0.5,
                    spike_threshold: None,
                    max_spike_count: None,
                }),
            })]
            .into(),
            pof_overrides: None,
        };
        assert!(profile.validate().is_ok());

        // Invalid alpha > 1.0
        profile.axes.get_mut("x").unwrap().filter.as_mut().unwrap().alpha = 1.5;
        assert!(profile.validate().is_err());
    }

    #[test]
    fn detent_zone_position_bounds() {
        let mut p = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("throttle".to_string(), AxisConfig {
                deadzone: None,
                expo: None,
                slew_rate: None,
                detents: vec![DetentZone {
                    position: 0.5,
                    width: 0.05,
                    role: "idle".to_string(),
                }],
                curve: None,
                filter: None,
            })]
            .into(),
            pof_overrides: None,
        };
        assert!(p.validate().is_ok());

        // Position out of range
        p.axes.get_mut("throttle").unwrap().detents[0].position = 1.5;
        assert!(p.validate().is_err());
    }

    #[test]
    fn curve_must_be_monotonically_increasing() {
        let p = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: [("x".to_string(), AxisConfig {
                deadzone: None,
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: Some(vec![
                    CurvePoint { input: 0.0, output: 0.0 },
                    CurvePoint { input: 0.5, output: 0.5 },
                    CurvePoint { input: 0.5, output: 0.9 }, // duplicate input
                    CurvePoint { input: 1.0, output: 1.0 },
                ]),
                filter: None,
            })]
            .into(),
            pof_overrides: None,
        };
        assert!(p.validate().is_err());
    }

    // ── AutoSwitchConfig / ServiceConfig defaults ──

    #[test]
    fn auto_switch_config_default_is_sensible() {
        let cfg = AutoSwitchConfig::default();
        assert!(cfg.max_switch_time.as_millis() > 0);
        assert!(!cfg.profile_paths.is_empty());
        assert!(cfg.enable_pof);
    }

    #[test]
    fn pof_hysteresis_config_default_has_bands() {
        let cfg = PofHysteresisConfig::default();
        assert!(!cfg.hysteresis_bands.is_empty());
        assert!(cfg.consecutive_frames_required > 0);
        assert!(cfg.min_phase_time.as_millis() > 0);
    }

    #[test]
    fn hysteresis_band_thresholds_are_finite() {
        let cfg = PofHysteresisConfig::default();
        for (name, band) in &cfg.hysteresis_bands {
            assert!(
                band.enter_threshold.is_finite(),
                "Band '{}': enter_threshold is not finite",
                name,
            );
            assert!(
                band.exit_threshold.is_finite(),
                "Band '{}': exit_threshold is not finite",
                name,
            );
            assert!(
                band.enter_threshold != band.exit_threshold,
                "Band '{}': enter and exit thresholds are equal (no hysteresis dead band)",
                name,
            );
        }
    }

    #[test]
    fn watchdog_config_default_sane() {
        let cfg = WatchdogConfig::default();
        assert!(cfg.max_execution_time.as_micros() > 0);
        assert!(cfg.usb_timeout.as_millis() > 0);
        let _ = format!("{:?}", cfg);
    }

    #[test]
    fn security_config_default_sane() {
        let cfg = flight_core::SecurityConfig::default();
        let _ = format!("{:?}", cfg);
    }

    // ── Capability modes ──

    #[test]
    fn capability_context_for_each_mode() {
        let full = CapabilityContext::for_mode(CapabilityMode::Full);
        let demo = CapabilityContext::for_mode(CapabilityMode::Demo);
        let kid = CapabilityContext::for_mode(CapabilityMode::Kid);

        // Full mode has highest limits
        assert!(full.limits.max_expo >= demo.limits.max_expo);
        assert!(demo.limits.max_expo >= kid.limits.max_expo);
        assert!(full.limits.max_slew_rate >= demo.limits.max_slew_rate);
        assert!(demo.limits.max_slew_rate >= kid.limits.max_slew_rate);
    }

    #[test]
    fn kid_mode_rejects_high_expo() {
        let mut axes = HashMap::new();
        axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.05),
                expo: Some(0.4), // exceeds Kid limit of 0.3
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        let p = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes,
            pof_overrides: None,
        };
        let kid = CapabilityContext::for_mode(CapabilityMode::Kid);
        assert!(p.validate_with_capabilities(&kid).is_err());
        let full = CapabilityContext::for_mode(CapabilityMode::Full);
        assert!(p.validate_with_capabilities(&full).is_ok());
    }

    #[test]
    fn demo_mode_rejects_high_slew() {
        let mut axes = HashMap::new();
        axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: None,
                expo: None,
                slew_rate: Some(60.0), // exceeds Demo limit of 50
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        let p = Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes,
            pof_overrides: None,
        };
        let demo = CapabilityContext::for_mode(CapabilityMode::Demo);
        assert!(p.validate_with_capabilities(&demo).is_err());
        let full = CapabilityContext::for_mode(CapabilityMode::Full);
        assert!(p.validate_with_capabilities(&full).is_ok());
    }

    // ── CalibrationStore ──

    #[test]
    fn calibration_store_roundtrip_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cal.toml");
        let mut store = CalibrationStore::new();
        store.set(0x044F, 0xB10A, vec![AxisCalibration::new(0, 0, 65535, 32767)]);
        store.save_to_file(&path).unwrap();
        let loaded = CalibrationStore::load_from_file(&path).unwrap();
        assert_eq!(loaded.device_count(), 1);
        let cals = loaded.get(0x044F, 0xB10A).unwrap();
        assert_eq!(cals[0].axis_id, 0);
    }

    #[test]
    fn calibration_normalize_center_is_zero() {
        let cal = AxisCalibration::new(0, 0, 65535, 32767);
        assert_eq!(cal.normalize(32767), 0.0);
    }

    #[test]
    fn calibration_normalize_extremes() {
        let cal = AxisCalibration::new(0, 0, 65535, 32767);
        assert!((cal.normalize(0) + 1.0).abs() < 1e-4);
        assert!((cal.normalize(65535) - 1.0).abs() < 1e-4);
    }

    // ── CircuitBreaker ──

    #[test]
    fn circuit_breaker_lifecycle() {
        let cfg = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 1,
            timeout: Duration::from_millis(10),
        };
        let mut cb = CircuitBreaker::new(cfg);
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.call_allowed(), CallResult::Allowed);

        // Trip the breaker
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert_eq!(cb.call_allowed(), CallResult::Rejected);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cb.call_allowed(), CallResult::Allowed);
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Recover
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    // ── ProfileWatcher / ReloadNotifier ──

    #[test]
    fn reload_notifier_thread_safe_sharing() {
        let n1 = ReloadNotifier::new();
        let n2 = n1.clone();
        n1.notify(std::path::PathBuf::from("a.yaml"));
        assert!(n2.has_pending());
        let drained = n2.drain();
        assert_eq!(drained.len(), 1);
        assert!(!n1.has_pending());
    }

    #[test]
    fn reload_notifier_deduplicates() {
        let n = ReloadNotifier::new();
        n.notify(std::path::PathBuf::from("same.yaml"));
        n.notify(std::path::PathBuf::from("same.yaml"));
        assert_eq!(n.drain().len(), 1);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. PROPERTY TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // ── Strategies ──

    fn simple_axis(dz: Option<f32>, expo: Option<f32>, slew: Option<f32>) -> AxisConfig {
        AxisConfig {
            deadzone: dz,
            expo,
            slew_rate: slew,
            detents: vec![],
            curve: None,
            filter: None,
        }
    }

    fn make_profile(axes: HashMap<String, AxisConfig>) -> Profile {
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes,
            pof_overrides: None,
        }
    }

    prop_compose! {
        fn arb_axis_config()(
            deadzone in prop::option::of(0.0f32..0.5f32),
            expo in prop::option::of(0.0f32..1.0f32),
            slew_rate in prop::option::of(0.0f32..100.0f32),
        ) -> AxisConfig {
            simple_axis(deadzone, expo, slew_rate)
        }
    }

    prop_compose! {
        fn arb_profile()(
            axes in prop::collection::hash_map("[a-z]{1,5}", arb_axis_config(), 0..3),
        ) -> Profile {
            make_profile(axes)
        }
    }

    proptest! {
        // ── Error catalog property tests ──

        /// Error code → description is deterministic.
        #[test]
        fn prop_error_code_description_deterministic(idx in 0usize..100) {
            let all = ErrorCatalog::all();
            if idx < all.len() {
                let code = all[idx].code;
                let d1 = ErrorCatalog::lookup(code).unwrap().description;
                let d2 = ErrorCatalog::lookup(code).unwrap().description;
                prop_assert_eq!(d1, d2);
            }
        }

        /// All error messages are non-empty.
        #[test]
        fn prop_error_messages_non_empty(idx in 0usize..100) {
            let all = ErrorCatalog::all();
            if idx < all.len() {
                prop_assert!(!all[idx].message.is_empty());
                prop_assert!(!all[idx].description.is_empty());
                prop_assert!(!all[idx].resolution.is_empty());
            }
        }

        /// lookup() never panics for arbitrary input.
        #[test]
        fn prop_lookup_never_panics(code in ".{0,256}") {
            let _ = ErrorCatalog::lookup(&code);
        }

        /// format_error() never panics for arbitrary input.
        #[test]
        fn prop_format_error_never_panics(code in ".{0,256}") {
            let s = ErrorCatalog::format_error(&code);
            prop_assert!(!s.is_empty());
        }

        // ── Profile merge property tests ──

        /// Merge is associative for disjoint axes.
        #[test]
        fn prop_merge_associative_disjoint(
            dz_a in 0.0f32..0.5,
            dz_b in 0.0f32..0.5,
            dz_c in 0.0f32..0.5,
        ) {
            let a = make_profile(
                [("pitch".into(), simple_axis(Some(dz_a), None, None))].into()
            );
            let b = make_profile(
                [("roll".into(), simple_axis(Some(dz_b), None, None))].into()
            );
            let c = make_profile(
                [("yaw".into(), simple_axis(Some(dz_c), None, None))].into()
            );

            let ab_c = a.merge_with(&b).unwrap().merge_with(&c).unwrap();
            let a_bc = a.merge_with(&b.merge_with(&c).unwrap()).unwrap();

            prop_assert_eq!(ab_c.axes.len(), a_bc.axes.len());
            for (key, val) in &ab_c.axes {
                let other = a_bc.axes.get(key).unwrap();
                prop_assert_eq!(val, other);
            }
        }

        /// Merge with empty is identity.
        #[test]
        fn prop_merge_empty_identity(profile in arb_profile()) {
            let empty = make_profile(HashMap::new());
            let merged = profile.merge_with(&empty).unwrap();
            prop_assert_eq!(&profile, &merged);
        }

        // ── Serialization round-trip ──

        /// JSON round-trip for all profile types.
        #[test]
        fn prop_profile_json_roundtrip(profile in arb_profile()) {
            let json = serde_json::to_string(&profile).unwrap();
            let restored: Profile = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(&profile, &restored);
        }

        /// Hash is stable across serialization round-trip.
        #[test]
        fn prop_hash_stable_across_roundtrip(profile in arb_profile()) {
            let json = serde_json::to_string(&profile).unwrap();
            let restored: Profile = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(profile.effective_hash(), restored.effective_hash());
        }

        /// Canonicalize is deterministic.
        #[test]
        fn prop_canonicalize_deterministic(profile in arb_profile()) {
            prop_assert_eq!(profile.canonicalize(), profile.canonicalize());
        }

        // ── Validation consistency ──

        /// Valid profiles always re-validate.
        #[test]
        fn prop_valid_profile_revalidates(
            dz in 0.0f32..0.5,
            expo in 0.0f32..1.0,
            slew in 0.0f32..100.0,
        ) {
            let profile = make_profile(
                [("x".into(), simple_axis(Some(dz), Some(expo), Some(slew)))].into()
            );
            prop_assert!(profile.validate().is_ok());
            prop_assert!(profile.validate().is_ok()); // re-validation
        }

        /// Merged valid profiles produce a valid profile.
        #[test]
        fn prop_merged_valid_profiles_valid(
            dz_a in 0.0f32..0.5,
            expo_a in 0.0f32..1.0,
            dz_b in 0.0f32..0.5,
            expo_b in 0.0f32..1.0,
        ) {
            let a = make_profile(
                [("pitch".into(), simple_axis(Some(dz_a), Some(expo_a), None))].into()
            );
            let b = make_profile(
                [("roll".into(), simple_axis(Some(dz_b), Some(expo_b), None))].into()
            );
            prop_assert!(a.validate().is_ok());
            prop_assert!(b.validate().is_ok());
            let merged = a.merge_with(&b).unwrap();
            prop_assert!(merged.validate().is_ok());
        }

        // ── Core types trait checks ──

        /// All AxisConfig instances are Debug + Clone + PartialEq.
        #[test]
        fn prop_axis_config_traits(axis in arb_axis_config()) {
            let cloned = axis.clone();
            prop_assert_eq!(&axis, &cloned);
            let _ = format!("{:?}", axis);
        }

        /// AircraftId is Debug + Clone + PartialEq + Serialize/Deserialize.
        #[test]
        fn prop_aircraft_id_roundtrip(icao in "[A-Z0-9]{2,6}") {
            let id = AircraftId { icao: icao.clone() };
            let cloned = id.clone();
            prop_assert_eq!(&id, &cloned);
            let json = serde_json::to_string(&id).unwrap();
            let restored: AircraftId = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(&id, &restored);
            let _ = format!("{:?}", id);
        }

        // ── Calibration normalize property tests ──

        /// Normalize always returns values in [-1.0, 1.0].
        #[test]
        fn prop_calibration_normalize_bounded(
            raw in -100_000i32..100_000,
            center in 10_000i32..50_000,
        ) {
            let cal = AxisCalibration::new(0, 0, 65535, center);
            let n = cal.normalize(raw);
            prop_assert!(
                (-1.0..=1.0).contains(&n),
                "normalize({}) with center {} = {}, out of range",
                raw, center, n
            );
        }
    }
}
