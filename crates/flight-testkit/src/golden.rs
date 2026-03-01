// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Golden-file testing utilities.
//!
//! Compares actual output against a stored "golden" file. When the
//! `FLIGHT_UPDATE_GOLDEN` environment variable is set, mismatched files are
//! overwritten so that the new output becomes the accepted baseline.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Return the path to the golden-files directory for this crate.
pub fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("golden")
}

/// Assert that `actual` matches the golden file named `name`.
///
/// If the golden file does not yet exist, it is created.
/// If `FLIGHT_UPDATE_GOLDEN=1` is set, mismatched golden files are updated.
///
/// # Panics
///
/// Panics with a diff message when the actual content does not match the
/// stored golden file and `FLIGHT_UPDATE_GOLDEN` is not set.
pub fn assert_golden(name: &str, actual: &str) {
    let path = golden_dir().join(name);

    // If the golden file doesn't exist, create it.
    if !path.exists() {
        fs::write(&path, actual).unwrap_or_else(|e| {
            panic!("failed to write new golden file {}: {e}", path.display());
        });
        return;
    }

    let expected = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("failed to read golden file {}: {e}", path.display());
    });

    if expected == actual {
        return;
    }

    // Mismatch — update or fail.
    if env::var("FLIGHT_UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        fs::write(&path, actual).unwrap_or_else(|e| {
            panic!("failed to update golden file {}: {e}", path.display());
        });
        return;
    }

    panic!(
        "golden file mismatch for '{name}':\n\
         --- expected ({})\n{expected}\n\
         +++ actual\n{actual}",
        path.display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn golden_dir_exists() {
        let dir = golden_dir();
        // The golden directory was created at crate setup time.
        assert!(dir.exists(), "golden dir should exist: {}", dir.display());
    }

    #[test]
    fn assert_golden_creates_new_file() {
        let name = "_test_new_golden.txt";
        let path = golden_dir().join(name);
        // Clean up from any prior run.
        let _ = fs::remove_file(&path);

        assert_golden(name, "hello golden");
        assert!(path.exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello golden");

        // Clean up.
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn assert_golden_matches_existing() {
        let name = "_test_match_golden.txt";
        let path = golden_dir().join(name);
        fs::write(&path, "exact match").unwrap();

        // Should not panic.
        assert_golden(name, "exact match");

        let _ = fs::remove_file(&path);
    }

    #[test]
    #[should_panic(expected = "golden file mismatch")]
    fn assert_golden_panics_on_mismatch() {
        let name = "_test_mismatch_golden.txt";
        let path = golden_dir().join(name);
        fs::write(&path, "old content").unwrap();

        // Ensure the update env var is NOT set for this test.
        // SAFETY: This test is single-threaded and no other code reads
        // this variable concurrently.
        unsafe { env::remove_var("FLIGHT_UPDATE_GOLDEN") };
        assert_golden(name, "new content");

        // cleanup (won't run due to panic, but the file is tiny)
    }
}
