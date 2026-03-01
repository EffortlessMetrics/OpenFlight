//! MSI manifest validation tests.
//!
//! Verifies the Windows installer packaging list, install paths, service
//! registration, uninstall cleanup, and file permissions against the
//! canonical `installer/windows/files.toml` spec and the
//! `flight_installer::manifest` module.

use std::collections::HashSet;
use std::path::Path;

use flight_installer::manifest::{self, FileEntry, InstallManifest};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Load and parse `installer/windows/files.toml` relative to the workspace root.
fn load_windows_spec() -> toml::Value {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("installer/windows/files.toml");
    let text = std::fs::read_to_string(&spec_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", spec_path.display()));
    text.parse::<toml::Value>().expect("invalid TOML")
}

fn windows_manifest() -> InstallManifest {
    manifest::windows_manifest(Path::new(r"C:\Program Files\Flight Hub"))
}

// ── Expected files ───────────────────────────────────────────────────────────

#[test]
fn msi_manifest_contains_all_expected_files() {
    let m = windows_manifest();
    let names: Vec<String> = m
        .files
        .iter()
        .map(|f| f.source.to_string_lossy().into_owned())
        .collect();

    assert!(names.iter().any(|n| n.contains("flightd.exe")));
    assert!(names.iter().any(|n| n.contains("flightctl.exe")));
    assert!(names.iter().any(|n| n.contains("config.toml")));
    assert!(names.iter().any(|n| n.contains("default.profile.toml")));
}

#[test]
fn msi_manifest_file_count_matches_spec() {
    let spec = load_windows_spec();
    let spec_files = spec["files"].as_array().expect("files array");
    let m = windows_manifest();
    assert_eq!(
        m.files.len(),
        spec_files.len(),
        "manifest file count must match spec"
    );
}

#[test]
fn msi_spec_file_sources_match_manifest() {
    let spec = load_windows_spec();
    let spec_sources: HashSet<String> = spec["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["source"].as_str().unwrap().to_string())
        .collect();
    let manifest_sources: HashSet<String> = windows_manifest()
        .files
        .iter()
        .map(|f| f.source.to_string_lossy().replace('\\', "/"))
        .collect();
    assert_eq!(spec_sources, manifest_sources);
}

// ── Install paths ────────────────────────────────────────────────────────────

#[test]
fn msi_install_paths_under_program_files() {
    let m = windows_manifest();
    for file in &m.files {
        let dest = file.destination.to_string_lossy();
        assert!(
            dest.starts_with(r"C:\Program Files\Flight Hub"),
            "destination not under Program Files: {dest}"
        );
    }
}

#[test]
fn msi_binaries_in_bin_subdirectory() {
    let m = windows_manifest();
    let bins: Vec<&FileEntry> = m
        .files
        .iter()
        .filter(|f| {
            f.source
                .extension()
                .is_some_and(|e| e == "exe")
        })
        .collect();
    assert!(!bins.is_empty(), "no binaries found");
    for b in &bins {
        assert!(
            b.destination.to_string_lossy().contains("bin"),
            "binary not in bin/: {}",
            b.destination.display()
        );
    }
}

#[test]
fn msi_configs_in_config_subdirectory() {
    let m = windows_manifest();
    let configs: Vec<&FileEntry> = m
        .files
        .iter()
        .filter(|f| {
            f.source
                .extension()
                .is_some_and(|e| e == "toml")
        })
        .collect();
    assert!(!configs.is_empty(), "no config files found");
    for c in &configs {
        assert!(
            c.destination.to_string_lossy().contains("config"),
            "config not in config/: {}",
            c.destination.display()
        );
    }
}

// ── Service registration ─────────────────────────────────────────────────────

#[test]
fn msi_service_name_is_flighthub() {
    let m = windows_manifest();
    assert_eq!(m.service.name, "FlightHub");
}

#[test]
fn msi_service_display_name_set() {
    let m = windows_manifest();
    assert!(
        !m.service.display_name.is_empty(),
        "service display name is empty"
    );
}

#[test]
fn msi_service_auto_start_enabled() {
    let m = windows_manifest();
    assert!(m.service.auto_start);
}

#[test]
fn msi_service_binary_path_points_to_daemon() {
    let m = windows_manifest();
    let svc_bin = m.service.binary_path.to_string_lossy();
    assert!(
        svc_bin.contains("flightd"),
        "service binary does not reference flightd: {svc_bin}"
    );
}

// ── Uninstall cleanup ────────────────────────────────────────────────────────

#[test]
fn msi_uninstall_cleanup_covers_install_dirs() {
    let m = windows_manifest();
    let cleanup_strs: Vec<String> = m
        .cleanup_paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    assert!(cleanup_strs.iter().any(|s| s.contains("bin")));
    assert!(cleanup_strs.iter().any(|s| s.contains("config")));
    assert!(cleanup_strs.iter().any(|s| s.contains("logs")));
}

#[test]
fn msi_uninstall_cleanup_reverses_all_install_actions() {
    let m = windows_manifest();
    // Every non-preserved file destination's parent directory should appear
    // in cleanup_paths.
    let preserved_set: HashSet<_> = m.preserved_paths.iter().collect();
    let cleanup_strs: HashSet<String> = m
        .cleanup_paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();

    for file in &m.files {
        if preserved_set.contains(&file.destination) {
            continue;
        }
        let parent = file
            .destination
            .parent()
            .expect("file has no parent dir")
            .to_string_lossy()
            .into_owned();
        assert!(
            cleanup_strs.iter().any(|c| parent.starts_with(c.as_str())),
            "no cleanup entry covers parent of {}",
            file.destination.display()
        );
    }
}

#[test]
fn msi_preserved_paths_not_in_cleanup() {
    let m = windows_manifest();
    assert!(m.verify().is_ok(), "manifest self-validation failed");
}

// ── File permissions spec ────────────────────────────────────────────────────

#[test]
fn msi_spec_binaries_have_755_permissions() {
    let spec = load_windows_spec();
    for f in spec["files"].as_array().unwrap() {
        let src = f["source"].as_str().unwrap();
        let perms = f["permissions"].as_str().unwrap();
        if src.ends_with(".exe") {
            assert_eq!(perms, "755", "binary {src} must have 755 permissions");
        }
    }
}

#[test]
fn msi_spec_configs_have_644_permissions() {
    let spec = load_windows_spec();
    for f in spec["files"].as_array().unwrap() {
        let src = f["source"].as_str().unwrap();
        let perms = f["permissions"].as_str().unwrap();
        if src.ends_with(".toml") {
            assert_eq!(perms, "644", "config {src} must have 644 permissions");
        }
    }
}

#[test]
fn msi_manifest_validation_passes() {
    let m = windows_manifest();
    m.verify().expect("Windows manifest validation failed");
}
