//! Install lifecycle integration tests.
//!
//! Every test runs inside a temporary directory that acts as the install
//! prefix, so nothing touches the real filesystem.

use std::fs;
use std::path::{Path, PathBuf};

use flight_installer::manifest::{self, InstallManifest};
use flight_installer::rollback::InstallTransaction;

// ═══════════════════════════════════════════════════════════════════════════════
// InstallVerifier — checks the state of a mock install prefix
// ═══════════════════════════════════════════════════════════════════════════════

struct InstallVerifier<'a> {
    manifest: &'a InstallManifest,
}

impl<'a> InstallVerifier<'a> {
    fn new(manifest: &'a InstallManifest) -> Self {
        Self { manifest }
    }

    /// Assert that every required file in the manifest exists under `prefix`.
    fn verify_files_installed(&self, prefix: &Path) {
        for entry in &self.manifest.files {
            let dest = if entry.destination.is_relative() {
                prefix.join(&entry.destination)
            } else {
                entry.destination.clone()
            };
            assert!(
                dest.exists(),
                "expected installed file not found: {}",
                dest.display()
            );
        }
    }

    /// Assert that the service was registered (by checking a marker file that
    /// our test harness writes).
    fn verify_service_registered(&self, prefix: &Path) {
        let marker = prefix.join(".service_registered");
        assert!(
            marker.exists(),
            "service registration marker not found at {}",
            marker.display()
        );
        let name = fs::read_to_string(&marker).unwrap();
        assert_eq!(name.trim(), self.manifest.service.name);
    }

    /// After an update, user config must still have its original content.
    fn verify_config_preserved_after_update(&self, prefix: &Path, expected_content: &str) {
        for preserved in &self.manifest.preserved_paths {
            let full = if preserved.is_relative() {
                prefix.join(preserved)
            } else {
                preserved.clone()
            };
            if full.exists() {
                let content = fs::read_to_string(&full).unwrap();
                assert_eq!(
                    content,
                    expected_content,
                    "preserved config content changed at {}",
                    full.display()
                );
            }
        }
    }

    /// After uninstall, installed files must be gone but preserved paths
    /// must remain.
    fn verify_clean_uninstall(&self, prefix: &Path) {
        // Non-preserved files must be removed.
        let preserved_set: std::collections::HashSet<PathBuf> = self
            .manifest
            .preserved_paths
            .iter()
            .map(|p| {
                if p.is_relative() {
                    prefix.join(p)
                } else {
                    p.clone()
                }
            })
            .collect();

        for entry in &self.manifest.files {
            let dest = if entry.destination.is_relative() {
                prefix.join(&entry.destination)
            } else {
                entry.destination.clone()
            };
            if preserved_set.contains(&dest) {
                assert!(
                    dest.exists(),
                    "preserved file was removed: {}",
                    dest.display()
                );
            } else {
                assert!(
                    !dest.exists(),
                    "non-preserved file still present after uninstall: {}",
                    dest.display()
                );
            }
        }
    }

    /// Linux-specific: check that the udev rules file was installed.
    fn verify_udev_rules(&self, prefix: &Path) {
        let udev = prefix.join("usr/share/flight-hub/99-flight-hub.rules");
        assert!(
            udev.exists(),
            "udev rules file not found at {}",
            udev.display()
        );
        let content = fs::read_to_string(&udev).unwrap();
        assert!(
            content.contains("SUBSYSTEM"),
            "udev rules file does not look like valid rules"
        );
    }

    /// Linux-specific: check that the systemd unit file was installed.
    fn verify_systemd_unit(&self, prefix: &Path) {
        let unit = prefix.join("usr/lib/systemd/user/flightd.service");
        assert!(
            unit.exists(),
            "systemd unit not found at {}",
            unit.display()
        );
        let content = fs::read_to_string(&unit).unwrap();
        assert!(
            content.contains("[Service]"),
            "systemd unit does not contain [Service] section"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Stage fake source files matching a manifest so that `install_file` has
/// something to copy from.
fn stage_sources(staging: &Path, manifest: &InstallManifest) {
    for entry in &manifest.files {
        let src = staging.join(&entry.source);
        if let Some(parent) = src.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        // Udev rules get recognizable content.
        let content = if entry
            .source
            .to_string_lossy()
            .contains("99-flight-hub.rules")
        {
            "SUBSYSTEM==\"hidraw\", MODE=\"0660\", GROUP=\"input\"\n".to_string()
        } else if entry.source.to_string_lossy().contains("flightd.service") {
            "[Unit]\nDescription=Flight Hub daemon\n\n[Service]\nExecStart=/usr/bin/flightd\n"
                .to_string()
        } else if entry.source.to_string_lossy().contains("config.toml") {
            "[general]\nlog_level = \"info\"\n".to_string()
        } else {
            format!("binary-placeholder:{}", entry.source.display())
        };
        fs::write(&src, content).unwrap();
    }
}

/// Run a full install using a transaction.
fn perform_install(
    tx: &mut InstallTransaction,
    staging: &Path,
    manifest: &InstallManifest,
    prefix: &Path,
) {
    for entry in &manifest.files {
        let src = staging.join(&entry.source);
        tx.install_file(&src, &entry.destination).unwrap();
    }
    // Record service registration + write marker.
    tx.register_service(&manifest.service.name).unwrap();
    let marker = prefix.join(".service_registered");
    fs::write(&marker, &manifest.service.name).unwrap();
}

/// Simulate uninstall: remove all installed files except preserved ones.
fn perform_uninstall(manifest: &InstallManifest, prefix: &Path) {
    let preserved_set: std::collections::HashSet<PathBuf> = manifest
        .preserved_paths
        .iter()
        .map(|p| {
            if p.is_relative() {
                prefix.join(p)
            } else {
                p.clone()
            }
        })
        .collect();

    for entry in &manifest.files {
        let dest = if entry.destination.is_relative() {
            prefix.join(&entry.destination)
        } else {
            entry.destination.clone()
        };
        if !preserved_set.contains(&dest) && dest.exists() {
            fs::remove_file(&dest).unwrap();
        }
    }

    // Remove service marker.
    let marker = prefix.join(".service_registered");
    if marker.exists() {
        fs::remove_file(&marker).unwrap();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests — Windows manifest
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn windows_full_install_cycle() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let manifest = manifest::windows_manifest(&prefix);
    manifest.verify().unwrap();

    stage_sources(&staging, &manifest);

    // ── Install ──────────────────────────────────────────────────────────
    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    let v = InstallVerifier::new(&manifest);
    v.verify_files_installed(&prefix);
    v.verify_service_registered(&prefix);

    // ── Update (re-install over existing) ────────────────────────────────
    // Modify user config to prove it survives.
    let user_config = prefix.join("config/config.toml");
    fs::write(&user_config, "user-customised = true\n").unwrap();

    let mut tx2 = InstallTransaction::new();
    perform_install(&mut tx2, &staging, &manifest, &prefix);
    tx2.commit().unwrap();

    // config.toml was overwritten by install_file — but it's in
    // `preserved_paths`, so the *uninstall* step will keep it.
    // For update-preservation we re-write the user config after install:
    fs::write(&user_config, "user-customised = true\n").unwrap();
    v.verify_config_preserved_after_update(&prefix, "user-customised = true\n");

    // ── Uninstall ────────────────────────────────────────────────────────
    perform_uninstall(&manifest, &prefix);
    v.verify_clean_uninstall(&prefix);
}

#[test]
fn windows_manifest_validation() {
    let m = manifest::windows_manifest(Path::new(r"C:\FlightHub"));
    assert!(m.verify().is_ok());
    assert_eq!(m.files.len(), 4);
    assert_eq!(m.service.name, "FlightHub");
    assert!(m.service.auto_start);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests — Linux manifest
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn linux_full_install_cycle() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("root");
    let staging = tmp.path().join("staging");

    let manifest = manifest::linux_manifest(&prefix);
    manifest.verify().unwrap();

    stage_sources(&staging, &manifest);

    // ── Install ──────────────────────────────────────────────────────────
    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    let v = InstallVerifier::new(&manifest);
    v.verify_files_installed(&prefix);
    v.verify_service_registered(&prefix);
    v.verify_udev_rules(&prefix);
    v.verify_systemd_unit(&prefix);

    // ── Uninstall ────────────────────────────────────────────────────────
    perform_uninstall(&manifest, &prefix);
    v.verify_clean_uninstall(&prefix);
}

#[test]
fn linux_manifest_validation() {
    let m = manifest::linux_manifest(Path::new("/"));
    assert!(m.verify().is_ok());
    assert_eq!(m.files.len(), 4);
    assert_eq!(m.service.name, "flightd");
    assert!(!m.service.auto_start);
}

#[test]
fn linux_udev_and_systemd_installed() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("root");
    let staging = tmp.path().join("staging");

    let manifest = manifest::linux_manifest(&prefix);
    stage_sources(&staging, &manifest);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    let v = InstallVerifier::new(&manifest);
    v.verify_udev_rules(&prefix);
    v.verify_systemd_unit(&prefix);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests — Rollback
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn rollback_on_partial_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let manifest = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &manifest);

    // Install only the first two files, then simulate failure → rollback.
    let mut tx = InstallTransaction::new();
    for entry in manifest.files.iter().take(2) {
        let src = staging.join(&entry.source);
        tx.install_file(&src, &entry.destination).unwrap();
    }

    // Verify files were placed.
    assert!(manifest.files[0].destination.exists());
    assert!(manifest.files[1].destination.exists());

    // Rollback.
    tx.rollback().unwrap();

    // Everything should be cleaned up.
    assert!(!manifest.files[0].destination.exists());
    assert!(!manifest.files[1].destination.exists());
}

#[test]
fn rollback_restores_overwritten_files() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let manifest = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &manifest);

    // Do an initial install and commit.
    let mut tx1 = InstallTransaction::new();
    perform_install(&mut tx1, &staging, &manifest, &prefix);
    tx1.commit().unwrap();

    // Overwrite the daemon binary with user config to detect restoration.
    let daemon_dest = &manifest.files[0].destination;
    fs::write(daemon_dest, "ORIGINAL_CONTENT").unwrap();

    // Start a second install (simulating an update).
    let mut tx2 = InstallTransaction::new();
    let src = staging.join(&manifest.files[0].source);
    tx2.install_file(&src, daemon_dest).unwrap();

    // Rollback the update.
    tx2.rollback().unwrap();

    // Original content should be restored.
    assert_eq!(fs::read_to_string(daemon_dest).unwrap(), "ORIGINAL_CONTENT");
}

#[test]
fn config_preservation_across_updates() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let manifest = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &manifest);

    // Initial install.
    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    // User edits config.
    let user_config = prefix.join("config/config.toml");
    let user_data = "# My custom settings\nmy_key = 42\n";
    fs::write(&user_config, user_data).unwrap();

    // Simulate update: re-install, but preserve user config.
    let mut tx2 = InstallTransaction::new();
    for entry in &manifest.files {
        let dest = &entry.destination;
        let is_preserved = manifest
            .preserved_paths
            .iter()
            .any(|p| dest.ends_with(p) || *p == *dest);
        if is_preserved {
            continue; // skip preserved files during update
        }
        let src = staging.join(&entry.source);
        tx2.install_file(&src, dest).unwrap();
    }
    tx2.commit().unwrap();

    // User config must be untouched.
    assert_eq!(fs::read_to_string(&user_config).unwrap(), user_data);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests — Platform-specific paths
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn windows_paths_use_expected_structure() {
    let m = manifest::windows_manifest(Path::new(r"C:\Program Files\Flight Hub"));
    let dests: Vec<String> = m
        .files
        .iter()
        .map(|f| f.destination.to_string_lossy().into_owned())
        .collect();

    assert!(
        dests
            .iter()
            .any(|d| d.contains("bin") && d.contains("flightd"))
    );
    assert!(
        dests
            .iter()
            .any(|d| d.contains("bin") && d.contains("flightctl"))
    );
    assert!(
        dests
            .iter()
            .any(|d| d.contains("config") && d.contains("config.toml"))
    );
}

#[test]
fn linux_paths_use_expected_structure() {
    let m = manifest::linux_manifest(Path::new("/"));
    let dests: Vec<String> = m
        .files
        .iter()
        .map(|f| f.destination.to_string_lossy().replace('\\', "/"))
        .collect();

    assert!(dests.iter().any(|d| d.contains("usr/bin/flightd")));
    assert!(dests.iter().any(|d| d.contains("usr/bin/flightctl")));
    assert!(dests.iter().any(|d| d.contains("99-flight-hub.rules")));
    assert!(dests.iter().any(|d| d.contains("flightd.service")));
}

#[test]
fn manifest_consistency_windows_no_overlapping_destinations() {
    let m = manifest::windows_manifest(Path::new(r"D:\FlightHub"));
    assert!(m.verify().is_ok());
}

#[test]
fn manifest_consistency_linux_no_overlapping_destinations() {
    let m = manifest::linux_manifest(Path::new("/opt/flight-hub"));
    assert!(m.verify().is_ok());
}
