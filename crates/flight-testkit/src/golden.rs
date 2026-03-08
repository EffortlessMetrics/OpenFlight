// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! File-based golden/snapshot testing framework.
//!
//! Compares serializable output against stored `.golden` files.
//! Set `UPDATE_GOLDEN=1` to auto-update snapshots.

use serde::Serialize;
use std::fs;
use std::path::PathBuf;

/// Default directory for golden files, relative to manifest dir.
const GOLDEN_DIR: &str = "testdata";

/// Run a golden-file test.
///
/// Compares `actual` (serialized to pretty JSON) against the file
/// `testdata/<test_name>.golden` relative to the crate's manifest directory.
///
/// - On first run (file missing): creates the golden file.
/// - When `UPDATE_GOLDEN=1`: overwrites the golden file with `actual`.
/// - Otherwise: asserts that `actual` matches the stored snapshot.
///
/// # Panics
///
/// Panics if the actual output doesn't match the stored golden file.
pub fn golden_test<T: Serialize>(test_name: &str, actual: &T) {
    let actual_json = serde_json::to_string_pretty(actual).expect("failed to serialize to JSON");
    golden_test_raw(test_name, &actual_json);
}

/// Run a golden-file test with a raw string (no serialization).
///
/// # Panics
///
/// Panics if the actual output doesn't match the stored golden file.
pub fn golden_test_raw(test_name: &str, actual: &str) {
    let golden_path = golden_file_path(test_name);

    let update = std::env::var("UPDATE_GOLDEN")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if update || !golden_path.exists() {
        if let Some(parent) = golden_path.parent() {
            fs::create_dir_all(parent).expect("failed to create golden directory");
        }
        fs::write(&golden_path, actual).expect("failed to write golden file");
        if !update {
            eprintln!(
                "Golden file created: {}. Re-run to verify.",
                golden_path.display()
            );
        }
        return;
    }

    let expected = fs::read_to_string(&golden_path).expect("failed to read golden file");

    if actual != expected {
        // Write actual output for easier diffing.
        let actual_path = golden_path.with_extension("golden.actual");
        let _ = fs::write(&actual_path, actual);

        panic!(
            "Golden file mismatch for '{test_name}'.\n\
             Expected (golden): {}\n\
             Actual written to: {}\n\
             Set UPDATE_GOLDEN=1 to update.",
            golden_path.display(),
            actual_path.display()
        );
    }
}

/// Resolve the path to a golden file.
fn golden_file_path(test_name: &str) -> PathBuf {
    assert!(
        !test_name.contains("..") && !test_name.contains('/') && !test_name.contains('\\'),
        "test_name must not contain path separators or '..'"
    );
    // Use CARGO_MANIFEST_DIR if available (i.e. running under `cargo test`),
    // otherwise fall back to current directory.
    let base = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join(GOLDEN_DIR).join(format!("{test_name}.golden"))
}

/// Check whether a golden file exists for the given test name.
#[must_use]
pub fn golden_file_exists(test_name: &str) -> bool {
    golden_file_path(test_name).exists()
}

/// Load an existing golden file as a string, or `None` if it doesn't exist.
#[must_use]
pub fn load_golden(test_name: &str) -> Option<String> {
    let path = golden_file_path(test_name);
    fs::read_to_string(path).ok()
}

/// Deserialize a golden file into `T`, or `None` if missing.
pub fn load_golden_as<T: serde::de::DeserializeOwned>(test_name: &str) -> Option<T> {
    let content = load_golden(test_name)?;
    serde_json::from_str(&content).ok()
}

/// Remove a golden file (useful for cleanup in meta-tests).
///
/// Returns `true` if the file existed and was removed.
pub fn remove_golden(test_name: &str) -> bool {
    let path = golden_file_path(test_name);
    fs::remove_file(path).is_ok()
}

/// List all `.golden` files in the `testdata/` directory.
#[must_use]
pub fn list_golden_files() -> Vec<PathBuf> {
    let base = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let dir = base.join(GOLDEN_DIR);
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("golden") {
                result.push(path);
            }
        }
    }
    result.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn golden_test_creates_and_verifies() {
        let name = "_test_golden_create_verify";
        // Clean up from any previous run.
        remove_golden(name);

        let data: BTreeMap<&str, i32> = [("a", 1), ("b", 2)].into_iter().collect();

        // First run: creates the file.
        golden_test(name, &data);
        assert!(golden_file_exists(name));

        // Second run: verifies the file matches.
        golden_test(name, &data);

        // Clean up.
        remove_golden(name);
    }

    #[test]
    fn golden_test_raw_creates_and_verifies() {
        let name = "_test_golden_raw";
        remove_golden(name);

        golden_test_raw(name, "hello world");
        golden_test_raw(name, "hello world");

        remove_golden(name);
    }

    #[test]
    #[should_panic(expected = "Golden file mismatch")]
    fn golden_test_detects_mismatch() {
        let name = "_test_golden_mismatch";
        remove_golden(name);

        golden_test_raw(name, "original");
        golden_test_raw(name, "modified"); // Should panic.
    }

    #[test]
    fn load_golden_returns_none_for_missing() {
        assert!(load_golden("_nonexistent_golden_file").is_none());
    }

    #[test]
    fn load_golden_as_deserializes() {
        let name = "_test_golden_deser";
        remove_golden(name);

        let data = vec![1, 2, 3];
        golden_test(name, &data);

        let loaded: Option<Vec<i32>> = load_golden_as(name);
        assert_eq!(loaded, Some(vec![1, 2, 3]));

        remove_golden(name);
    }

    #[test]
    fn list_golden_files_returns_sorted() {
        let files = list_golden_files();
        if files.len() >= 2 {
            for w in files.windows(2) {
                assert!(w[0] <= w[1]);
            }
        }
    }

    #[test]
    fn remove_golden_returns_false_for_missing() {
        assert!(!remove_golden("_definitely_not_here"));
    }

    #[test]
    fn golden_file_exists_false_for_missing() {
        assert!(!golden_file_exists("_no_such_golden"));
    }
}
