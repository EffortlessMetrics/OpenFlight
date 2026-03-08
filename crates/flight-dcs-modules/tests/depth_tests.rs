// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for flight-dcs-modules.
//!
//! Covers module loading, lookup, serde round-trips, error handling,
//! edge cases, shipped database completeness, and property-based tests.

use std::fs;
use std::path::Path;

use flight_dcs_modules::{DcsModule, ModuleError, ModuleLoader};
use proptest::prelude::*;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn toml_for(aircraft: &str, axes: u8, throw: f32, quirks: &[&str]) -> String {
    let q: Vec<String> = quirks.iter().map(|q| format!("\"{q}\"")).collect();
    format!(
        "aircraft = \"{aircraft}\"\n\
         axis_count = {axes}\n\
         throttle_range = [0.0, 1.0]\n\
         stick_throw = {throw}\n\
         quirks = [{}]\n",
        q.join(", ")
    )
}

fn write_module(dir: &Path, filename: &str, content: &str) {
    fs::write(dir.join(filename), content).unwrap();
}

fn loader_with_shipped_modules() -> ModuleLoader {
    let modules_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("modules");
    let mut loader = ModuleLoader::new();
    loader.load_from_dir(&modules_dir).unwrap();
    loader
}

// ===================================================================
// 1. Module / aircraft lookup
// ===================================================================

#[test]
fn lookup_fa18c_by_name() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("F/A-18C").expect("F/A-18C must exist");
    assert_eq!(m.aircraft, "F/A-18C");
}

#[test]
fn lookup_f16c_by_name() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("F-16C").expect("F-16C must exist");
    assert_eq!(m.aircraft, "F-16C");
}

#[test]
fn lookup_a10c_by_name() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("A-10C").expect("A-10C must exist");
    assert_eq!(m.aircraft, "A-10C");
}

#[test]
fn lookup_nonexistent_returns_none() {
    let loader = loader_with_shipped_modules();
    assert!(loader.get("MiG-29S").is_none());
}

#[test]
fn lookup_empty_string_returns_none() {
    let loader = loader_with_shipped_modules();
    assert!(loader.get("").is_none());
}

#[test]
fn lookup_case_sensitive() {
    let loader = loader_with_shipped_modules();
    // Name is "F/A-18C", not "f/a-18c"
    assert!(loader.get("f/a-18c").is_none());
}

// ===================================================================
// 2. Aircraft field validation
// ===================================================================

#[test]
fn fa18c_axis_count() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("F/A-18C").unwrap();
    assert_eq!(m.axis_count, 6);
}

#[test]
fn fa18c_throttle_range_normalised() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("F/A-18C").unwrap();
    assert_eq!(m.throttle_range, [0.0, 1.0]);
}

#[test]
fn fa18c_stick_throw() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("F/A-18C").unwrap();
    assert!((m.stick_throw - 45.0).abs() < f32::EPSILON);
}

#[test]
fn fa18c_quirks_include_twin_throttle() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("F/A-18C").unwrap();
    assert!(m.quirks.contains(&"twin-throttle".to_owned()));
}

#[test]
fn fa18c_quirks_include_catapult_bar() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("F/A-18C").unwrap();
    assert!(m.quirks.contains(&"catapult-bar".to_owned()));
}

#[test]
fn f16c_side_stick_quirk() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("F-16C").unwrap();
    assert!(m.quirks.contains(&"side-stick".to_owned()));
}

#[test]
fn f16c_stick_throw_30() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("F-16C").unwrap();
    assert!((m.stick_throw - 30.0).abs() < f32::EPSILON);
}

#[test]
fn a10c_axis_count_7() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("A-10C").unwrap();
    assert_eq!(m.axis_count, 7);
}

#[test]
fn a10c_stick_throw_50() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("A-10C").unwrap();
    assert!((m.stick_throw - 50.0).abs() < f32::EPSILON);
}

#[test]
fn a10c_gun_trigger_quirk() {
    let loader = loader_with_shipped_modules();
    let m = loader.get("A-10C").unwrap();
    assert!(m.quirks.contains(&"gun-trigger".to_owned()));
}

// ===================================================================
// 3. Database completeness — shipped modules
// ===================================================================

#[test]
fn shipped_modules_directory_exists() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("modules");
    assert!(path.is_dir(), "modules/ directory must ship with the crate");
}

#[test]
fn shipped_database_has_three_aircraft() {
    let loader = loader_with_shipped_modules();
    assert_eq!(loader.len(), 3);
}

#[test]
fn shipped_database_is_not_empty() {
    let loader = loader_with_shipped_modules();
    assert!(!loader.is_empty());
}

#[test]
fn shipped_aircraft_names_sorted() {
    let loader = loader_with_shipped_modules();
    let names = loader.aircraft_names();
    assert_eq!(names, vec!["A-10C", "F-16C", "F/A-18C"]);
}

#[test]
fn all_shipped_modules_have_positive_axis_count() {
    let loader = loader_with_shipped_modules();
    for name in loader.aircraft_names() {
        let m = loader.get(name).unwrap();
        assert!(m.axis_count > 0, "{name} must have at least 1 axis");
    }
}

#[test]
fn all_shipped_modules_have_valid_throttle_range() {
    let loader = loader_with_shipped_modules();
    for name in loader.aircraft_names() {
        let m = loader.get(name).unwrap();
        assert!(
            m.throttle_range[0] < m.throttle_range[1],
            "{name}: throttle min must be less than max"
        );
    }
}

#[test]
fn all_shipped_modules_have_positive_stick_throw() {
    let loader = loader_with_shipped_modules();
    for name in loader.aircraft_names() {
        let m = loader.get(name).unwrap();
        assert!(
            m.stick_throw > 0.0,
            "{name}: stick throw must be positive"
        );
    }
}

#[test]
fn all_shipped_modules_have_nonempty_quirks() {
    let loader = loader_with_shipped_modules();
    for name in loader.aircraft_names() {
        let m = loader.get(name).unwrap();
        assert!(
            !m.quirks.is_empty(),
            "{name}: every shipped module should document at least one quirk"
        );
    }
}

// ===================================================================
// 4. ModuleLoader basics & edge cases
// ===================================================================

#[test]
fn default_trait_creates_empty_loader() {
    let loader = ModuleLoader::default();
    assert!(loader.is_empty());
    assert_eq!(loader.len(), 0);
}

#[test]
fn len_and_is_empty_agree() {
    let loader = ModuleLoader::new();
    assert_eq!(loader.is_empty(), loader.len() == 0);
}

#[test]
fn load_from_empty_directory_returns_zero() {
    let dir = TempDir::new().unwrap();
    let mut loader = ModuleLoader::new();
    let count = loader.load_from_dir(dir.path()).unwrap();
    assert_eq!(count, 0);
    assert!(loader.is_empty());
}

#[test]
fn non_toml_files_are_skipped() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("readme.md"), "# Not a module").unwrap();
    fs::write(dir.path().join("data.json"), "{}").unwrap();
    let mut loader = ModuleLoader::new();
    let count = loader.load_from_dir(dir.path()).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn mixed_toml_and_non_toml_loads_only_toml() {
    let dir = TempDir::new().unwrap();
    write_module(dir.path(), "readme.md", "# ignore");
    write_module(
        dir.path(),
        "su-27.toml",
        &toml_for("Su-27", 5, 40.0, &["twin-engine"]),
    );
    let mut loader = ModuleLoader::new();
    let count = loader.load_from_dir(dir.path()).unwrap();
    assert_eq!(count, 1);
    assert!(loader.get("Su-27").is_some());
}

#[test]
fn load_directory_nonexistent_returns_io_error() {
    let mut loader = ModuleLoader::new();
    let err = loader
        .load_from_dir(Path::new("totally/bogus/path"))
        .unwrap_err();
    assert!(matches!(err, ModuleError::Io(_)));
}

#[test]
fn loading_same_dir_twice_updates_count() {
    let dir = TempDir::new().unwrap();
    write_module(
        dir.path(),
        "ka-50.toml",
        &toml_for("Ka-50", 4, 35.0, &["coaxial-rotor"]),
    );
    let mut loader = ModuleLoader::new();
    loader.load_from_dir(dir.path()).unwrap();
    assert_eq!(loader.len(), 1);
    // Loading again with same data — key collision keeps count at 1
    let count = loader.load_from_dir(dir.path()).unwrap();
    assert_eq!(count, 1);
    assert_eq!(loader.len(), 1);
}

#[test]
fn loading_two_different_dirs_accumulates() {
    let dir1 = TempDir::new().unwrap();
    let dir2 = TempDir::new().unwrap();
    write_module(
        dir1.path(),
        "mi-8.toml",
        &toml_for("Mi-8", 4, 35.0, &["helicopter"]),
    );
    write_module(
        dir2.path(),
        "uh-1.toml",
        &toml_for("UH-1H", 4, 40.0, &["helicopter"]),
    );
    let mut loader = ModuleLoader::new();
    loader.load_from_dir(dir1.path()).unwrap();
    loader.load_from_dir(dir2.path()).unwrap();
    assert_eq!(loader.len(), 2);
}

// ===================================================================
// 5. Error handling
// ===================================================================

#[test]
fn malformed_toml_produces_parse_error() {
    let dir = TempDir::new().unwrap();
    write_module(dir.path(), "bad.toml", "not valid [[[");
    let mut loader = ModuleLoader::new();
    let err = loader.load_from_dir(dir.path()).unwrap_err();
    assert!(matches!(err, ModuleError::ParseError { .. }));
}

#[test]
fn parse_error_contains_file_name() {
    let dir = TempDir::new().unwrap();
    write_module(dir.path(), "oops.toml", "bad content = [[[");
    let mut loader = ModuleLoader::new();
    let err = loader.load_from_dir(dir.path()).unwrap_err();
    match err {
        ModuleError::ParseError { file, .. } => assert_eq!(file, "oops.toml"),
        other => panic!("expected ParseError, got: {other:?}"),
    }
}

#[test]
fn missing_required_field_is_parse_error() {
    let dir = TempDir::new().unwrap();
    // Missing axis_count, throttle_range, stick_throw, quirks
    write_module(dir.path(), "partial.toml", "aircraft = \"Incomplete\"");
    let mut loader = ModuleLoader::new();
    let err = loader.load_from_dir(dir.path()).unwrap_err();
    assert!(matches!(err, ModuleError::ParseError { .. }));
}

#[test]
fn wrong_type_for_axis_count_is_parse_error() {
    let dir = TempDir::new().unwrap();
    let content = "aircraft = \"X\"\n\
                   axis_count = \"six\"\n\
                   throttle_range = [0.0, 1.0]\n\
                   stick_throw = 45.0\n\
                   quirks = []";
    write_module(dir.path(), "wrong_type.toml", content);
    let mut loader = ModuleLoader::new();
    let err = loader.load_from_dir(dir.path()).unwrap_err();
    assert!(matches!(err, ModuleError::ParseError { .. }));
}

#[test]
fn module_error_io_display() {
    let err = ModuleError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "test",
    ));
    let msg = format!("{err}");
    assert!(msg.contains("I/O error"));
}

#[test]
fn module_error_parse_display() {
    let toml_err = toml::from_str::<DcsModule>("bad").unwrap_err();
    let err = ModuleError::ParseError {
        file: "test.toml".to_owned(),
        source: toml_err,
    };
    let msg = format!("{err}");
    assert!(msg.contains("test.toml"));
}

// ===================================================================
// 6. Serde round-trips
// ===================================================================

#[test]
fn serde_roundtrip_zero_quirks() {
    let m = DcsModule {
        aircraft: "Test-Zero".to_owned(),
        axis_count: 2,
        throttle_range: [0.0, 0.5],
        stick_throw: 10.0,
        quirks: vec![],
    };
    let s = toml::to_string(&m).unwrap();
    let d: DcsModule = toml::from_str(&s).unwrap();
    assert_eq!(d.aircraft, m.aircraft);
    assert_eq!(d.quirks.len(), 0);
}

#[test]
fn serde_roundtrip_many_quirks() {
    let m = DcsModule {
        aircraft: "Quirky".to_owned(),
        axis_count: 8,
        throttle_range: [0.1, 0.9],
        stick_throw: 60.0,
        quirks: vec![
            "a".to_owned(),
            "b".to_owned(),
            "c".to_owned(),
            "d".to_owned(),
        ],
    };
    let s = toml::to_string(&m).unwrap();
    let d: DcsModule = toml::from_str(&s).unwrap();
    assert_eq!(d.quirks.len(), 4);
}

#[test]
fn serde_roundtrip_preserves_throttle_range() {
    let m = DcsModule {
        aircraft: "Throttle".to_owned(),
        axis_count: 1,
        throttle_range: [0.2, 0.8],
        stick_throw: 15.0,
        quirks: vec![],
    };
    let s = toml::to_string(&m).unwrap();
    let d: DcsModule = toml::from_str(&s).unwrap();
    assert!((d.throttle_range[0] - 0.2).abs() < f32::EPSILON);
    assert!((d.throttle_range[1] - 0.8).abs() < f32::EPSILON);
}

#[test]
fn serde_roundtrip_max_axis_count() {
    let m = DcsModule {
        aircraft: "MaxAxes".to_owned(),
        axis_count: u8::MAX,
        throttle_range: [0.0, 1.0],
        stick_throw: 90.0,
        quirks: vec![],
    };
    let s = toml::to_string(&m).unwrap();
    let d: DcsModule = toml::from_str(&s).unwrap();
    assert_eq!(d.axis_count, u8::MAX);
}

#[test]
fn serde_roundtrip_unicode_aircraft_name() {
    let m = DcsModule {
        aircraft: "Миг-29".to_owned(),
        axis_count: 5,
        throttle_range: [0.0, 1.0],
        stick_throw: 40.0,
        quirks: vec!["twin-engine".to_owned()],
    };
    let s = toml::to_string(&m).unwrap();
    let d: DcsModule = toml::from_str(&s).unwrap();
    assert_eq!(d.aircraft, "Миг-29");
}

#[test]
fn serde_roundtrip_via_file() {
    let dir = TempDir::new().unwrap();
    let m = DcsModule {
        aircraft: "FileRT".to_owned(),
        axis_count: 3,
        throttle_range: [0.0, 1.0],
        stick_throw: 25.0,
        quirks: vec!["test".to_owned()],
    };
    let s = toml::to_string(&m).unwrap();
    let path = dir.path().join("filert.toml");
    fs::write(&path, &s).unwrap();

    let mut loader = ModuleLoader::new();
    loader.load_from_dir(dir.path()).unwrap();
    let loaded = loader.get("FileRT").unwrap();
    assert_eq!(loaded.aircraft, m.aircraft);
    assert_eq!(loaded.axis_count, m.axis_count);
}

// ===================================================================
// 7. DcsModule clone / debug
// ===================================================================

#[test]
fn dcs_module_clone_is_independent() {
    let original = DcsModule {
        aircraft: "Clone".to_owned(),
        axis_count: 4,
        throttle_range: [0.0, 1.0],
        stick_throw: 30.0,
        quirks: vec!["test".to_owned()],
    };
    let mut cloned = original.clone();
    cloned.aircraft = "Changed".to_owned();
    assert_eq!(original.aircraft, "Clone");
    assert_eq!(cloned.aircraft, "Changed");
}

#[test]
fn dcs_module_debug_contains_aircraft() {
    let m = DcsModule {
        aircraft: "DebugMe".to_owned(),
        axis_count: 1,
        throttle_range: [0.0, 1.0],
        stick_throw: 10.0,
        quirks: vec![],
    };
    let dbg = format!("{m:?}");
    assert!(dbg.contains("DebugMe"));
}

// ===================================================================
// 8. Duplicate key handling
// ===================================================================

#[test]
fn duplicate_aircraft_in_same_dir_last_wins() {
    let dir = TempDir::new().unwrap();
    write_module(
        dir.path(),
        "v1.toml",
        &toml_for("Dup", 3, 20.0, &["old"]),
    );
    write_module(
        dir.path(),
        "v2.toml",
        &toml_for("Dup", 5, 40.0, &["new"]),
    );
    let mut loader = ModuleLoader::new();
    loader.load_from_dir(dir.path()).unwrap();
    // Only one entry for "Dup"
    assert_eq!(loader.len(), 1);
    let m = loader.get("Dup").unwrap();
    // We accept whichever version the filesystem enumerates last
    assert!(m.axis_count == 3 || m.axis_count == 5);
}

// ===================================================================
// 9. Property-based tests (proptest)
// ===================================================================

prop_compose! {
    fn arb_dcs_module()
        (
            aircraft in "[A-Za-z0-9/\\-]{1,20}",
            axis_count in 1..=u8::MAX,
            thr_min in 0.0f32..0.5,
            thr_max in 0.5f32..1.0,
            stick_throw in 1.0f32..180.0,
            quirk_count in 0usize..5,
        )
        (
            aircraft in Just(aircraft),
            axis_count in Just(axis_count),
            thr_min in Just(thr_min),
            thr_max in Just(thr_max),
            stick_throw in Just(stick_throw),
            quirks in proptest::collection::vec("[a-z\\-]{1,12}", quirk_count..=quirk_count),
        )
    -> DcsModule {
        DcsModule {
            aircraft,
            axis_count,
            throttle_range: [thr_min, thr_max],
            stick_throw,
            quirks,
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn prop_serde_roundtrip(m in arb_dcs_module()) {
        let serialized = toml::to_string(&m).unwrap();
        let deserialized: DcsModule = toml::from_str(&serialized).unwrap();
        prop_assert_eq!(&deserialized.aircraft, &m.aircraft);
        prop_assert_eq!(deserialized.axis_count, m.axis_count);
        prop_assert!((deserialized.stick_throw - m.stick_throw).abs() < f32::EPSILON);
        prop_assert_eq!(&deserialized.quirks, &m.quirks);
        prop_assert!((deserialized.throttle_range[0] - m.throttle_range[0]).abs() < f32::EPSILON);
        prop_assert!((deserialized.throttle_range[1] - m.throttle_range[1]).abs() < f32::EPSILON);
    }

    #[test]
    fn prop_load_via_file_roundtrip(m in arb_dcs_module()) {
        let dir = TempDir::new().unwrap();
        let serialized = toml::to_string(&m).unwrap();
        fs::write(dir.path().join("test.toml"), &serialized).unwrap();

        let mut loader = ModuleLoader::new();
        loader.load_from_dir(dir.path()).unwrap();
        let loaded = loader.get(&m.aircraft).unwrap();
        prop_assert_eq!(&loaded.aircraft, &m.aircraft);
        prop_assert_eq!(loaded.axis_count, m.axis_count);
    }

    #[test]
    fn prop_aircraft_names_always_sorted(
        names in proptest::collection::vec("[A-Z][a-z0-9\\-]{0,8}", 1..10)
    ) {
        let dir = TempDir::new().unwrap();
        for (i, name) in names.iter().enumerate() {
            write_module(
                dir.path(),
                &format!("m{i}.toml"),
                &toml_for(name, 4, 30.0, &["q"]),
            );
        }
        let mut loader = ModuleLoader::new();
        loader.load_from_dir(dir.path()).unwrap();
        let result = loader.aircraft_names();
        let mut expected = result.clone();
        expected.sort_unstable();
        prop_assert_eq!(result, expected);
    }

    #[test]
    fn prop_axis_count_preserved(count in 1u8..=u8::MAX) {
        let m = DcsModule {
            aircraft: "PropAxis".to_owned(),
            axis_count: count,
            throttle_range: [0.0, 1.0],
            stick_throw: 30.0,
            quirks: vec![],
        };
        let s = toml::to_string(&m).unwrap();
        let d: DcsModule = toml::from_str(&s).unwrap();
        prop_assert_eq!(d.axis_count, count);
    }

    #[test]
    fn prop_stick_throw_preserved(throw in 0.1f32..360.0) {
        let m = DcsModule {
            aircraft: "PropThrow".to_owned(),
            axis_count: 4,
            throttle_range: [0.0, 1.0],
            stick_throw: throw,
            quirks: vec![],
        };
        let s = toml::to_string(&m).unwrap();
        let d: DcsModule = toml::from_str(&s).unwrap();
        prop_assert!((d.stick_throw - throw).abs() < f32::EPSILON);
    }

    #[test]
    fn prop_quirks_count_preserved(
        quirks in proptest::collection::vec("[a-z]{1,8}", 0..10)
    ) {
        let m = DcsModule {
            aircraft: "PropQuirks".to_owned(),
            axis_count: 4,
            throttle_range: [0.0, 1.0],
            stick_throw: 30.0,
            quirks,
        };
        let original_count = m.quirks.len();
        let s = toml::to_string(&m).unwrap();
        let d: DcsModule = toml::from_str(&s).unwrap();
        prop_assert_eq!(d.quirks.len(), original_count);
    }

    #[test]
    fn prop_loader_len_matches_unique_aircraft(
        names in proptest::collection::hash_set("[A-Z][a-z0-9]{1,6}", 1..8)
    ) {
        let dir = TempDir::new().unwrap();
        for (i, name) in names.iter().enumerate() {
            write_module(
                dir.path(),
                &format!("m{i}.toml"),
                &toml_for(name, 4, 30.0, &["q"]),
            );
        }
        let mut loader = ModuleLoader::new();
        loader.load_from_dir(dir.path()).unwrap();
        prop_assert_eq!(loader.len(), names.len());
    }
}

// ===================================================================
// 10. Additional edge-case tests
// ===================================================================

#[test]
fn zero_axis_count_roundtrips() {
    let m = DcsModule {
        aircraft: "ZeroAxes".to_owned(),
        axis_count: 0,
        throttle_range: [0.0, 1.0],
        stick_throw: 10.0,
        quirks: vec![],
    };
    let s = toml::to_string(&m).unwrap();
    let d: DcsModule = toml::from_str(&s).unwrap();
    assert_eq!(d.axis_count, 0);
}

#[test]
fn negative_stick_throw_roundtrips() {
    // Not semantically valid, but serde should handle it
    let m = DcsModule {
        aircraft: "NegThrow".to_owned(),
        axis_count: 2,
        throttle_range: [0.0, 1.0],
        stick_throw: -10.0,
        quirks: vec![],
    };
    let s = toml::to_string(&m).unwrap();
    let d: DcsModule = toml::from_str(&s).unwrap();
    assert!((d.stick_throw - (-10.0)).abs() < f32::EPSILON);
}

#[test]
fn empty_quirk_string_roundtrips() {
    let m = DcsModule {
        aircraft: "EmptyQuirk".to_owned(),
        axis_count: 2,
        throttle_range: [0.0, 1.0],
        stick_throw: 20.0,
        quirks: vec!["".to_owned()],
    };
    let s = toml::to_string(&m).unwrap();
    let d: DcsModule = toml::from_str(&s).unwrap();
    assert_eq!(d.quirks, vec![""]);
}

#[test]
fn special_chars_in_aircraft_name_roundtrip() {
    let m = DcsModule {
        aircraft: "F/A-18C «Hornet»".to_owned(),
        axis_count: 6,
        throttle_range: [0.0, 1.0],
        stick_throw: 45.0,
        quirks: vec![],
    };
    let s = toml::to_string(&m).unwrap();
    let d: DcsModule = toml::from_str(&s).unwrap();
    assert_eq!(d.aircraft, "F/A-18C «Hornet»");
}

#[test]
fn loader_get_after_loading_returns_reference() {
    let dir = TempDir::new().unwrap();
    write_module(
        dir.path(),
        "test.toml",
        &toml_for("RefTest", 3, 25.0, &["ref"]),
    );
    let mut loader = ModuleLoader::new();
    loader.load_from_dir(dir.path()).unwrap();
    let r1 = loader.get("RefTest").unwrap();
    let r2 = loader.get("RefTest").unwrap();
    assert_eq!(r1.aircraft, r2.aircraft);
    // Both should point to the same data
    assert!(std::ptr::eq(r1, r2));
}

#[test]
fn aircraft_names_empty_when_no_modules_loaded() {
    let loader = ModuleLoader::new();
    assert!(loader.aircraft_names().is_empty());
}

#[test]
fn toml_with_extra_fields_is_accepted() {
    // serde(deny_unknown_fields) is NOT set, so extras should parse fine
    let dir = TempDir::new().unwrap();
    let content = "aircraft = \"Extra\"\n\
                   axis_count = 4\n\
                   throttle_range = [0.0, 1.0]\n\
                   stick_throw = 30.0\n\
                   quirks = []\n\
                   unknown_field = 42\n";
    write_module(dir.path(), "extra.toml", content);
    let mut loader = ModuleLoader::new();
    let count = loader.load_from_dir(dir.path()).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn throttle_range_equal_values_roundtrip() {
    let m = DcsModule {
        aircraft: "EqualThrottle".to_owned(),
        axis_count: 1,
        throttle_range: [0.5, 0.5],
        stick_throw: 10.0,
        quirks: vec![],
    };
    let s = toml::to_string(&m).unwrap();
    let d: DcsModule = toml::from_str(&s).unwrap();
    assert!((d.throttle_range[0] - d.throttle_range[1]).abs() < f32::EPSILON);
}
