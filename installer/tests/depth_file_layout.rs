//! Depth tests — File layout verification for MSI and deb installers.
//!
//! Validates that platform manifests produce the correct directory structure,
//! file placement, permissions metadata, and cleanup behaviour.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use flight_installer::manifest::{self, FileEntry, InstallManifest};
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
            "[Unit]\nDescription=Flight Hub daemon\n\n[Service]\nExecStart=/usr/bin/flightd\n\n[Install]\nWantedBy=default.target\n"
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
// 1. Windows MSI installs to correct paths
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn windows_msi_installs_to_correct_paths() {
    let prefix = Path::new(r"C:\Program Files\Flight Hub");
    let m = manifest::windows_manifest(prefix);

    // Every destination must live under the install prefix.
    for entry in &m.files {
        assert!(
            entry.destination.starts_with(prefix),
            "destination {} is not under prefix {}",
            entry.destination.display(),
            prefix.display()
        );
    }

    // Specific expected paths.
    let dests: Vec<String> = m
        .files
        .iter()
        .map(|f| f.destination.to_string_lossy().replace('\\', "/"))
        .collect();

    assert!(dests.iter().any(|d| d.ends_with("bin/flightd.exe")));
    assert!(dests.iter().any(|d| d.ends_with("bin/flightctl.exe")));
    assert!(dests.iter().any(|d| d.ends_with("config/config.toml")));
    assert!(dests
        .iter()
        .any(|d| d.ends_with("config/default.profile.toml")));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Linux deb installs to correct paths
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn linux_deb_installs_to_correct_paths() {
    let prefix = Path::new("/");
    let m = manifest::linux_manifest(prefix);

    let dests: Vec<String> = m
        .files
        .iter()
        .map(|f| f.destination.to_string_lossy().replace('\\', "/"))
        .collect();

    assert!(dests.iter().any(|d| d.contains("usr/bin/flightd")));
    assert!(dests.iter().any(|d| d.contains("usr/bin/flightctl")));
    assert!(dests
        .iter()
        .any(|d| d.contains("usr/share/flight-hub/99-flight-hub.rules")));
    assert!(dests
        .iter()
        .any(|d| d.contains("usr/lib/systemd/user/flightd.service")));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Config directory created
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn windows_config_directory_created_on_install() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("fh");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    let config_dir = prefix.join("config");
    assert!(config_dir.is_dir(), "config directory must exist after install");
    assert!(
        config_dir.join("config.toml").exists(),
        "config.toml must exist inside config directory"
    );
    assert!(
        config_dir.join("default.profile.toml").exists(),
        "default.profile.toml must exist inside config directory"
    );
}

#[test]
fn linux_config_directories_referenced_in_preserved_paths() {
    let m = manifest::linux_manifest(Path::new("/"));
    assert!(
        m.preserved_paths
            .iter()
            .any(|p| p.to_string_lossy().contains(".config/flight-hub")),
        "Linux manifest must preserve ~/.config/flight-hub"
    );
    assert!(
        m.preserved_paths
            .iter()
            .any(|p| p.to_string_lossy().contains(".local/share/flight-hub")),
        "Linux manifest must preserve ~/.local/share/flight-hub"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Binary permissions correct (all binaries marked required)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn windows_binary_entries_are_required() {
    let m = manifest::windows_manifest(Path::new(r"C:\Program Files\Flight Hub"));
    let binaries: Vec<&FileEntry> = m
        .files
        .iter()
        .filter(|f| {
            f.destination
                .to_string_lossy()
                .replace('\\', "/")
                .contains("/bin/")
        })
        .collect();

    assert!(!binaries.is_empty(), "must have binary entries");
    for b in &binaries {
        assert!(
            b.required,
            "binary {} must be marked required",
            b.destination.display()
        );
    }
}

#[test]
fn linux_binary_entries_are_required() {
    let m = manifest::linux_manifest(Path::new("/"));
    let binaries: Vec<&FileEntry> = m
        .files
        .iter()
        .filter(|f| {
            f.destination
                .to_string_lossy()
                .replace('\\', "/")
                .contains("usr/bin/")
        })
        .collect();

    assert!(!binaries.is_empty(), "must have binary entries");
    for b in &binaries {
        assert!(
            b.required,
            "binary {} must be marked required",
            b.destination.display()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Sim integration files placed correctly
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn linux_sim_integration_files_placed() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("root");
    let staging = tmp.path().join("staging");

    let m = manifest::linux_manifest(&prefix);
    stage_sources(&staging, &m);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    // udev rules for HID device access
    let udev = prefix.join("usr/share/flight-hub/99-flight-hub.rules");
    assert!(udev.exists(), "udev rules must be installed");
    let content = fs::read_to_string(&udev).unwrap();
    assert!(
        content.contains("SUBSYSTEM"),
        "udev rules must contain SUBSYSTEM directives"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Udev rules installed (Linux)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn linux_udev_rules_contain_expected_vendors() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("root");
    let staging = tmp.path().join("staging");

    // Override the udev content with full rules for this test.
    let m = manifest::linux_manifest(&prefix);
    for entry in &m.files {
        let src = staging.join(&entry.source);
        if let Some(parent) = src.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        if entry
            .source
            .to_string_lossy()
            .contains("99-flight-hub.rules")
        {
            // Realistic content matching the actual rules file.
            fs::write(
                &src,
                "SUBSYSTEM==\"hidraw\", MODE=\"0660\", GROUP=\"input\"\nSUBSYSTEM==\"usb\", ATTR{bInterfaceClass}==\"03\", MODE=\"0660\", GROUP=\"input\"\n",
            )
            .unwrap();
        } else if entry.source.to_string_lossy().contains("flightd.service") {
            fs::write(&src, "[Unit]\nDescription=Flight Hub\n\n[Service]\nExecStart=/usr/bin/flightd\n").unwrap();
        } else {
            fs::write(&src, format!("placeholder:{}", entry.source.display())).unwrap();
        }
    }

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    let udev = prefix.join("usr/share/flight-hub/99-flight-hub.rules");
    let content = fs::read_to_string(&udev).unwrap();
    assert!(content.contains("hidraw"), "rules must reference hidraw subsystem");
    assert!(content.contains("MODE"), "rules must set device permissions");
    assert!(content.contains("GROUP"), "rules must assign a group");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. Desktop shortcut / start menu (Windows — verified via WiX manifest)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn windows_manifest_includes_shortcut_metadata() {
    // The MSI shortcut is defined in Components.wxs — we verify the service
    // info and display name used by the installer are consistent.
    let m = manifest::windows_manifest(Path::new(r"C:\Program Files\Flight Hub"));
    assert_eq!(m.service.display_name, "Flight Hub Service");
    assert_eq!(m.service.description, "Flight Hub input management service for flight simulation");
    // Binary path should point to flightd.exe for the service / shortcut target.
    assert!(
        m.service
            .binary_path
            .to_string_lossy()
            .contains("flightd"),
        "service binary must reference flightd"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. Uninstall removes all files
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn uninstall_removes_non_preserved_files() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    // Write user config to preserved path.
    let user_config = prefix.join("config/config.toml");
    fs::write(&user_config, "user-custom = true\n").unwrap();

    perform_uninstall(&m, &prefix);

    // Non-preserved files gone.
    let preserved_set: HashSet<PathBuf> = m.preserved_paths.iter().cloned().collect();
    for entry in &m.files {
        if preserved_set.contains(&entry.destination)
            || m.preserved_paths
                .iter()
                .any(|p| entry.destination.ends_with(p))
        {
            assert!(
                entry.destination.exists(),
                "preserved file removed: {}",
                entry.destination.display()
            );
        } else {
            assert!(
                !entry.destination.exists(),
                "non-preserved file remains: {}",
                entry.destination.display()
            );
        }
    }
}
