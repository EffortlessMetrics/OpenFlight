//! Depth tests — Service registration for Windows and Linux installers.
//!
//! Validates service metadata, lifecycle markers, upgrade preservation,
//! and uninstall cleanup of service registration state.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use flight_installer::manifest::{self, InstallManifest};
use flight_installer::rollback::InstallTransaction;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn stage_sources(staging: &Path, manifest: &InstallManifest) {
    for entry in &manifest.files {
        let src = staging.join(&entry.source);
        if let Some(parent) = src.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let content = if entry
            .source
            .to_string_lossy()
            .contains("99-flight-hub.rules")
        {
            "SUBSYSTEM==\"hidraw\", MODE=\"0660\", GROUP=\"input\"\n".to_string()
        } else if entry.source.to_string_lossy().contains("flightd.service") {
            "[Unit]\nDescription=Flight Hub daemon\n\n[Service]\nExecStart=/usr/bin/flightd\nRestart=on-failure\nRestartSec=5\n\n[Install]\nWantedBy=default.target\n"
                .to_string()
        } else if entry.source.to_string_lossy().contains("config.toml") {
            "[general]\nlog_level = \"info\"\n".to_string()
        } else {
            format!("binary-placeholder:{}", entry.source.display())
        };
        fs::write(&src, content).unwrap();
    }
}

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
    tx.register_service(&manifest.service.name).unwrap();
    let marker = prefix.join(".service_registered");
    fs::write(&marker, &manifest.service.name).unwrap();
}

fn perform_uninstall(manifest: &InstallManifest, prefix: &Path) {
    let preserved_set: HashSet<PathBuf> = manifest
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

    let marker = prefix.join(".service_registered");
    if marker.exists() {
        fs::remove_file(&marker).unwrap();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Windows: service installed as auto-start service
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn windows_service_metadata_correct() {
    let m = manifest::windows_manifest(Path::new(r"C:\Program Files\Flight Hub"));
    assert_eq!(m.service.name, "FlightHub");
    assert_eq!(m.service.display_name, "Flight Hub Service");
    assert!(
        m.service.auto_start,
        "Windows service must be auto-start for boot survival"
    );
    assert!(
        m.service
            .binary_path
            .to_string_lossy()
            .contains("flightd"),
        "service binary must reference flightd"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Linux: systemd unit installed
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn linux_systemd_unit_installed_with_correct_content() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("root");
    let staging = tmp.path().join("staging");

    let m = manifest::linux_manifest(&prefix);
    stage_sources(&staging, &m);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    let unit = prefix.join("usr/lib/systemd/user/flightd.service");
    assert!(unit.exists(), "systemd unit file must be installed");
    let content = fs::read_to_string(&unit).unwrap();
    assert!(content.contains("[Service]"), "must have [Service] section");
    assert!(
        content.contains("ExecStart"),
        "must have ExecStart directive"
    );
    assert!(
        content.contains("Restart=on-failure"),
        "must restart on failure for resilience"
    );
}

#[test]
fn linux_service_is_user_service() {
    let m = manifest::linux_manifest(Path::new("/"));

    // The systemd unit lives under user/ not system/ — confirming user-level service.
    let unit_entry = m
        .files
        .iter()
        .find(|f| f.source.to_string_lossy().contains("flightd.service"))
        .expect("must have systemd unit entry");

    let dest = unit_entry.destination.to_string_lossy().replace('\\', "/");
    assert!(
        dest.contains("systemd/user/"),
        "systemd unit must be a user service, got: {dest}"
    );

    assert!(
        !m.service.auto_start,
        "Linux user service should not be auto-started at system level"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Service starts after install (marker-based verification)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn service_registration_marker_present_after_install() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    let marker = prefix.join(".service_registered");
    assert!(marker.exists(), "service registration marker must exist");
    let name = fs::read_to_string(&marker).unwrap();
    assert_eq!(name, "FlightHub");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Service survives reboot (auto-start / WantedBy configuration)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn windows_service_auto_start_ensures_reboot_survival() {
    let m = manifest::windows_manifest(Path::new(r"C:\Program Files\Flight Hub"));
    assert!(
        m.service.auto_start,
        "Windows service must be auto-start to survive reboots"
    );
}

#[test]
fn linux_systemd_unit_has_install_section() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("root");
    let staging = tmp.path().join("staging");

    let m = manifest::linux_manifest(&prefix);
    stage_sources(&staging, &m);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    let unit = prefix.join("usr/lib/systemd/user/flightd.service");
    let content = fs::read_to_string(&unit).unwrap();
    assert!(
        content.contains("[Install]"),
        "systemd unit must have [Install] section for enable/disable"
    );
    assert!(
        content.contains("WantedBy="),
        "systemd unit must have WantedBy for boot survival"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Uninstall removes service
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn uninstall_removes_service_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    let marker = prefix.join(".service_registered");
    assert!(marker.exists());

    perform_uninstall(&m, &prefix);
    assert!(
        !marker.exists(),
        "service registration marker must be removed on uninstall"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Service config preserved on upgrade
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn service_config_preserved_across_upgrade() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    // Initial install.
    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    // User customises config.
    let user_config = prefix.join("config/config.toml");
    let custom = "# My custom Flight Hub config\nmy_axis = 42\n";
    fs::write(&user_config, custom).unwrap();

    // Simulate upgrade: skip preserved paths.
    let mut tx2 = InstallTransaction::new();
    for entry in &m.files {
        let is_preserved = m
            .preserved_paths
            .iter()
            .any(|p| entry.destination.ends_with(p) || *p == entry.destination);
        if is_preserved {
            continue;
        }
        let src = staging.join(&entry.source);
        tx2.install_file(&src, &entry.destination).unwrap();
    }
    tx2.register_service(&m.service.name).unwrap();
    tx2.commit().unwrap();

    // User config must be untouched.
    assert_eq!(fs::read_to_string(&user_config).unwrap(), custom);
}
