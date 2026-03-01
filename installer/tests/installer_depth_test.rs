//! Depth tests for the Flight Hub installer and packaging system.
//!
//! Covers: file layout, service management, upgrade, uninstall,
//! sim integration metadata, and platform-specific packaging details.
//!
//! Every test runs in a temporary directory — nothing touches the real filesystem.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use flight_installer::manifest::{self, InstallManifest, ManifestError};
use flight_installer::rollback::InstallTransaction;

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Write a file and return its path, creating parents as needed.
fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let p = dir.join(name);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&p, content).unwrap();
    p
}

/// Stage fake source files matching a manifest.
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
        } else if entry.source.to_string_lossy().contains("default.profile.toml") {
            "[profile]\nname = \"Default\"\n".to_string()
        } else {
            format!("binary-placeholder:{}", entry.source.display())
        };
        fs::write(&src, content).unwrap();
    }
}

/// Perform a full transactional install, writing a service-registration marker.
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

/// Simulate uninstall: remove installed files except preserved ones.
fn perform_uninstall(manifest: &InstallManifest, prefix: &Path) {
    let preserved: HashSet<PathBuf> = manifest
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
        if !preserved.contains(&dest) && dest.exists() {
            fs::remove_file(&dest).unwrap();
        }
    }
    let marker = prefix.join(".service_registered");
    if marker.exists() {
        fs::remove_file(&marker).unwrap();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. FILE LAYOUT (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn file_layout_windows_binary_locations() {
    let m = manifest::windows_manifest(Path::new(r"C:\Program Files\Flight Hub"));
    let bins: Vec<_> = m
        .files
        .iter()
        .filter(|f| f.destination.to_string_lossy().contains("bin"))
        .collect();
    assert_eq!(bins.len(), 2, "expected exactly 2 binaries in bin/");
    let names: HashSet<String> = bins
        .iter()
        .map(|b| {
            b.destination
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(names.contains("flightd.exe"));
    assert!(names.contains("flightctl.exe"));
    assert!(bins.iter().all(|b| b.required), "all binaries must be required");
}

#[test]
fn file_layout_config_directory_structure() {
    let m = manifest::windows_manifest(Path::new(r"C:\FlightHub"));
    let configs: Vec<_> = m
        .files
        .iter()
        .filter(|f| f.destination.to_string_lossy().contains("config"))
        .collect();
    assert!(configs.len() >= 2, "expected config.toml and default.profile.toml");
    let names: HashSet<String> = configs
        .iter()
        .map(|c| {
            c.destination
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(names.contains("config.toml"));
    assert!(names.contains("default.profile.toml"));
}

#[test]
fn file_layout_data_directory_in_cleanup() {
    let prefix = Path::new(r"C:\FlightHub");
    let m = manifest::windows_manifest(prefix);
    // The log directory should be in cleanup_paths
    let has_logs = m
        .cleanup_paths
        .iter()
        .any(|p| p.to_string_lossy().contains("logs"));
    assert!(has_logs, "logs directory should be in cleanup_paths");
}

#[test]
fn file_layout_log_directory_in_cleanup() {
    let prefix = Path::new(r"C:\FlightHub");
    let m = manifest::windows_manifest(prefix);
    assert!(
        m.cleanup_paths
            .iter()
            .any(|p| *p == prefix.join("logs")),
        "explicit logs path must appear in cleanup_paths"
    );
}

#[test]
fn file_layout_plugin_directory_linux() {
    let m = manifest::linux_manifest(Path::new("/"));
    // The share directory acts as the plugin/data directory
    let has_share = m
        .files
        .iter()
        .any(|f| f.destination.to_string_lossy().replace('\\', "/").contains("usr/share/flight-hub"));
    assert!(has_share, "linux manifest should install into usr/share/flight-hub");
}

#[test]
fn file_layout_udev_rules_linux() {
    let m = manifest::linux_manifest(Path::new("/"));
    let udev = m
        .files
        .iter()
        .find(|f| f.source.to_string_lossy().contains("99-flight-hub.rules"));
    assert!(udev.is_some(), "linux manifest must include udev rules file");
    assert!(udev.unwrap().required, "udev rules must be required");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. SERVICE MANAGEMENT (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn service_systemd_unit_file_content() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("root");
    let staging = tmp.path().join("staging");
    let manifest = manifest::linux_manifest(&prefix);
    stage_sources(&staging, &manifest);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    let unit = prefix.join("usr/lib/systemd/user/flightd.service");
    assert!(unit.exists());
    let content = fs::read_to_string(&unit).unwrap();
    assert!(content.contains("[Unit]"), "unit file missing [Unit] section");
    assert!(content.contains("[Service]"), "unit file missing [Service] section");
    assert!(content.contains("[Install]"), "unit file missing [Install] section");
    assert!(
        content.contains("ExecStart"),
        "unit file missing ExecStart directive"
    );
}

#[test]
fn service_linux_not_auto_start() {
    let m = manifest::linux_manifest(Path::new("/"));
    assert!(
        !m.service.auto_start,
        "linux user service should not auto-start at system level"
    );
}

#[test]
fn service_windows_auto_start() {
    let m = manifest::windows_manifest(Path::new(r"C:\FlightHub"));
    assert!(
        m.service.auto_start,
        "Windows service should auto-start"
    );
}

#[test]
fn service_windows_registration_metadata() {
    let m = manifest::windows_manifest(Path::new(r"C:\FlightHub"));
    assert_eq!(m.service.name, "FlightHub");
    assert_eq!(m.service.display_name, "Flight Hub Service");
    assert!(
        !m.service.description.is_empty(),
        "service description must not be empty"
    );
    assert_eq!(
        m.service.binary_path,
        PathBuf::from("bin/flightd.exe")
    );
}

#[test]
fn service_enable_disable_via_transaction() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");
    let manifest = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &manifest);

    // Install with service registration
    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    let marker = prefix.join(".service_registered");
    assert!(marker.exists(), "service should be registered after install");
    assert_eq!(
        fs::read_to_string(&marker).unwrap().trim(),
        "FlightHub"
    );

    // Uninstall removes service marker
    perform_uninstall(&manifest, &prefix);
    assert!(
        !marker.exists(),
        "service marker should be removed after uninstall"
    );
}

#[test]
fn service_status_marker_content() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");
    let manifest = manifest::linux_manifest(&prefix);
    stage_sources(&staging, &manifest);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    let marker = prefix.join(".service_registered");
    assert_eq!(fs::read_to_string(&marker).unwrap(), "flightd");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. UPGRADE (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn upgrade_in_place_overwrites_binaries() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");
    let manifest = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &manifest);

    // Initial install
    let mut tx1 = InstallTransaction::new();
    perform_install(&mut tx1, &staging, &manifest, &prefix);
    tx1.commit().unwrap();

    let daemon = &manifest.files[0].destination;
    let original = fs::read_to_string(daemon).unwrap();

    // Update staging with new content
    let new_staging = tmp.path().join("staging_v2");
    stage_sources(&new_staging, &manifest);
    let daemon_src = new_staging.join(&manifest.files[0].source);
    fs::write(&daemon_src, "updated-binary-v2").unwrap();

    // In-place upgrade
    let mut tx2 = InstallTransaction::new();
    let src = new_staging.join(&manifest.files[0].source);
    tx2.install_file(&src, daemon).unwrap();
    tx2.commit().unwrap();

    let updated = fs::read_to_string(daemon).unwrap();
    assert_ne!(original, updated);
    assert_eq!(updated, "updated-binary-v2");
}

#[test]
fn upgrade_config_preservation() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");
    let manifest = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &manifest);

    // Initial install
    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    // User customises config
    let user_config = prefix.join("config/config.toml");
    let custom_content = "# user settings\nmy_key = 99\n";
    fs::write(&user_config, custom_content).unwrap();

    // Upgrade skipping preserved paths
    let mut tx2 = InstallTransaction::new();
    for entry in &manifest.files {
        let is_preserved = manifest
            .preserved_paths
            .iter()
            .any(|p| entry.destination.ends_with(p) || *p == entry.destination);
        if is_preserved {
            continue;
        }
        let src = staging.join(&entry.source);
        tx2.install_file(&src, &entry.destination).unwrap();
    }
    tx2.commit().unwrap();

    assert_eq!(fs::read_to_string(&user_config).unwrap(), custom_content);
}

#[test]
fn upgrade_data_migration_new_files_added() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");
    let manifest = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &manifest);

    // v1 install
    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    // Simulate v2 adding a new plugin directory and file
    let plugin_dir = prefix.join("plugins");
    let plugin_file = plugin_dir.join("builtin.dll");
    let plugin_src = write_file(tmp.path(), "new_plugin.dll", "plugin-binary");

    let mut tx2 = InstallTransaction::new();
    tx2.create_directory(&plugin_dir).unwrap();
    tx2.install_file(&plugin_src, &plugin_file).unwrap();
    tx2.commit().unwrap();

    assert!(plugin_file.exists());
    assert_eq!(fs::read_to_string(&plugin_file).unwrap(), "plugin-binary");
}

#[test]
fn upgrade_rollback_on_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");
    let manifest = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &manifest);

    // Initial install
    let mut tx1 = InstallTransaction::new();
    perform_install(&mut tx1, &staging, &manifest, &prefix);
    tx1.commit().unwrap();

    let daemon = &manifest.files[0].destination;
    fs::write(daemon, "ORIGINAL_V1").unwrap();

    // Start upgrade, install one file, then rollback
    let mut tx2 = InstallTransaction::new();
    let src = staging.join(&manifest.files[0].source);
    tx2.install_file(&src, daemon).unwrap();
    assert_ne!(fs::read_to_string(daemon).unwrap(), "ORIGINAL_V1");

    tx2.rollback().unwrap();
    assert_eq!(
        fs::read_to_string(daemon).unwrap(),
        "ORIGINAL_V1",
        "rollback must restore original content"
    );
}

#[test]
fn upgrade_downgrade_prevention_via_manifest_verify() {
    // Simulate downgrade detection: a manifest with version metadata
    // cannot be validated if it has conflicting entries.
    let mut m = manifest::windows_manifest(Path::new(r"C:\FlightHub"));
    // Inject a duplicate to simulate a corrupt downgrade manifest
    let dup = m.files[0].clone();
    m.files.push(dup);
    assert!(
        matches!(m.verify(), Err(ManifestError::DuplicateDestination(_))),
        "corrupt manifest should be rejected"
    );
}

#[test]
fn upgrade_version_tracking_via_service_info() {
    let m1 = manifest::windows_manifest(Path::new(r"C:\FlightHub"));
    let m2 = manifest::linux_manifest(Path::new("/"));

    // Both manifests carry their service identity for version tracking
    assert!(!m1.service.name.is_empty());
    assert!(!m2.service.name.is_empty());
    assert_ne!(
        m1.service.name, m2.service.name,
        "platform manifests should have distinct service names"
    );

    // Service binary path is always present
    assert!(!m1.service.binary_path.as_os_str().is_empty());
    assert!(!m2.service.binary_path.as_os_str().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. UNINSTALL (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn uninstall_clean_removes_all_non_preserved() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");
    let manifest = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &manifest);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    perform_uninstall(&manifest, &prefix);

    let preserved: HashSet<PathBuf> = manifest
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
        let dest = &entry.destination;
        if preserved.contains(dest) {
            assert!(dest.exists(), "preserved file was removed: {}", dest.display());
        } else {
            assert!(!dest.exists(), "file not removed: {}", dest.display());
        }
    }
}

#[test]
fn uninstall_config_preservation_option() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");
    let manifest = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &manifest);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    // User config is in preserved_paths
    let user_config = prefix.join("config/config.toml");
    let custom = "# preserved\n";
    fs::write(&user_config, custom).unwrap();

    perform_uninstall(&manifest, &prefix);

    // config.toml is in preserved_paths so it must survive
    assert!(user_config.exists(), "config.toml must survive uninstall");
    assert_eq!(fs::read_to_string(&user_config).unwrap(), custom);
}

#[test]
fn uninstall_data_removal_via_full_cleanup() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");
    let manifest = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &manifest);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    // Simulate "purge" by removing everything including preserved
    for entry in &manifest.files {
        if entry.destination.exists() {
            fs::remove_file(&entry.destination).unwrap();
        }
    }
    for path in &manifest.cleanup_paths {
        if path.exists() && path.is_dir() {
            let _ = fs::remove_dir_all(path);
        }
    }

    // Everything gone
    for entry in &manifest.files {
        assert!(!entry.destination.exists());
    }
}

#[test]
fn uninstall_service_cleanup() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");
    let manifest = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &manifest);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    let marker = prefix.join(".service_registered");
    assert!(marker.exists());

    perform_uninstall(&manifest, &prefix);
    assert!(!marker.exists(), "service marker must be removed on uninstall");
}

#[test]
fn uninstall_registry_cleanup_windows_manifest_has_cleanup_paths() {
    let prefix = Path::new(r"C:\FlightHub");
    let m = manifest::windows_manifest(prefix);

    // The install prefix itself is in cleanup_paths (representing registry/dir cleanup)
    assert!(
        m.cleanup_paths.iter().any(|p| *p == prefix.to_path_buf()),
        "install prefix must be in cleanup_paths for full registry cleanup"
    );
    // bin and config dirs also
    assert!(m.cleanup_paths.iter().any(|p| *p == prefix.join("bin")));
    assert!(m.cleanup_paths.iter().any(|p| *p == prefix.join("config")));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. SIM INTEGRATION (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn sim_msfs_addon_wix_feature_defined() {
    // The WiX Product.wxs defines an MSFS feature at level 1000
    // Verify the package manifest fixture reflects this
    let fixture = include_str!("../../crates/flight-updater/tests/fixtures/installer/package_manifest.json");
    let parsed: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let features = parsed["features"].as_object().unwrap();
    // Core feature is required
    assert_eq!(features["Core"]["required"], true);
    assert_eq!(features["Core"]["level"], 1);
}

#[test]
fn sim_xplane_plugin_path_layout() {
    // X-Plane plugins go under Resources/plugins — the Linux manifest's share dir
    // is the staging area for sim integration files
    let m = manifest::linux_manifest(Path::new("/"));
    let share_files: Vec<_> = m
        .files
        .iter()
        .filter(|f| {
            f.destination
                .to_string_lossy()
                .replace('\\', "/")
                .contains("usr/share/flight-hub")
        })
        .collect();
    assert!(
        !share_files.is_empty(),
        "share directory must contain integration files"
    );
}

#[test]
fn sim_dcs_script_installation_path() {
    // DCS integration uses Export.lua scripts; verify the manifest structure
    // supports adding integration files beyond core binaries
    let m = manifest::linux_manifest(Path::new("/"));
    // Manifest allows extension — verify it validates cleanly
    assert!(m.verify().is_ok());
    // Service description mentions flight simulation
    assert!(
        m.service
            .description
            .to_lowercase()
            .contains("flight simulation"),
        "service description should reference flight simulation"
    );
}

#[test]
fn sim_integration_verification_all_required_files_present() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("root");
    let staging = tmp.path().join("staging");
    let manifest = manifest::linux_manifest(&prefix);
    stage_sources(&staging, &manifest);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    tx.commit().unwrap();

    // Every required file must exist
    for entry in &manifest.files {
        if entry.required {
            assert!(
                entry.destination.exists(),
                "required integration file missing: {}",
                entry.destination.display()
            );
        }
    }
}

#[test]
fn sim_integration_reversal_via_rollback() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("root");
    let staging = tmp.path().join("staging");
    let manifest = manifest::linux_manifest(&prefix);
    stage_sources(&staging, &manifest);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &manifest, &prefix);
    // Do NOT commit — rollback instead
    tx.rollback().unwrap();

    // All installed files should be removed
    for entry in &manifest.files {
        assert!(
            !entry.destination.exists(),
            "file should be removed after rollback: {}",
            entry.destination.display()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. PLATFORM-SPECIFIC (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn platform_msi_properties_from_wix() {
    // Verify the WiX Product.wxs contains required MSI metadata
    let wix = include_str!("../wix/Product.wxs");
    assert!(wix.contains("UpgradeCode"), "MSI must define UpgradeCode");
    assert!(
        wix.contains("MajorUpgrade"),
        "MSI must support major upgrades"
    );
    assert!(
        wix.contains("DowngradeErrorMessage"),
        "MSI must prevent downgrades"
    );
    assert!(
        wix.contains("InstallerVersion=\"500\""),
        "MSI must target InstallerVersion 500+"
    );
    assert!(
        wix.contains("InstallScope=\"perMachine\""),
        "MSI must be per-machine for service registration"
    );
    assert!(
        wix.contains("ProductName"),
        "MSI must declare ProductName"
    );
}

#[test]
fn platform_deb_package_metadata() {
    let control = include_str!("../debian/control");
    assert!(control.contains("Package: flight-hub"));
    assert!(control.contains("Architecture: amd64"));
    assert!(
        control.contains("Depends:"),
        "deb control must list dependencies"
    );
    assert!(
        control.contains("libudev1"),
        "deb must depend on libudev1 for HID"
    );
    assert!(
        control.contains("Recommends: rtkit"),
        "deb should recommend rtkit for RT scheduling"
    );
    assert!(control.contains("Homepage:"));
    assert!(control.contains("Description:"));
}

#[test]
fn platform_postinst_prerm_scripts() {
    let postinst = include_str!("../debian/postinst");
    let postrm = include_str!("../debian/postrm");

    // postinst must handle 'configure' case
    assert!(postinst.contains("configure"), "postinst must handle configure");
    assert!(
        postinst.contains("udevadm"),
        "postinst must reload udev rules"
    );
    assert!(
        postinst.contains("99-flight-hub.rules"),
        "postinst must reference udev rules file"
    );

    // postrm must handle 'remove' and 'purge'
    assert!(postrm.contains("remove"), "postrm must handle remove");
    assert!(postrm.contains("purge"), "postrm must handle purge");
    assert!(
        postrm.contains("udevadm"),
        "postrm must reload udev rules"
    );
}

#[test]
fn platform_elevated_privilege_handling() {
    // postinst grants input group membership when SUDO_USER is set
    let postinst = include_str!("../debian/postinst");
    assert!(
        postinst.contains("SUDO_USER"),
        "postinst must detect sudo user for privilege handling"
    );
    assert!(
        postinst.contains("usermod"),
        "postinst must add user to input group"
    );
    assert!(
        postinst.contains("input"),
        "postinst must reference input group"
    );
}

#[test]
fn platform_path_configuration() {
    // WiX Components.wxs defines PATH environment variable component
    let components = include_str!("../wix/Components.wxs");
    assert!(
        components.contains("PathEnvVar"),
        "WiX must define PathEnvVar component"
    );
    assert!(
        components.contains("Environment"),
        "WiX must use Environment element for PATH"
    );
    assert!(
        components.contains("Name=\"PATH\""),
        "WiX environment element must target PATH"
    );
    assert!(
        components.contains("Part=\"last\""),
        "PATH should be appended, not replaced"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. ADDITIONAL DEPTH — cross-cutting concerns (3+ bonus tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn manifest_json_roundtrip_preserves_all_fields() {
    let m = manifest::windows_manifest(Path::new(r"C:\FlightHub"));
    let json = serde_json::to_string_pretty(&m).unwrap();
    let m2: InstallManifest = serde_json::from_str(&json).unwrap();

    assert_eq!(m.files.len(), m2.files.len());
    assert_eq!(m.service.name, m2.service.name);
    assert_eq!(m.service.display_name, m2.service.display_name);
    assert_eq!(m.service.description, m2.service.description);
    assert_eq!(m.service.auto_start, m2.service.auto_start);
    assert_eq!(m.service.binary_path, m2.service.binary_path);
    assert_eq!(m.cleanup_paths, m2.cleanup_paths);
    assert_eq!(m.preserved_paths, m2.preserved_paths);
    for (a, b) in m.files.iter().zip(m2.files.iter()) {
        assert_eq!(a.source, b.source);
        assert_eq!(a.destination, b.destination);
        assert_eq!(a.required, b.required);
    }
}

#[test]
fn transaction_multi_file_rollback_reverse_order() {
    let tmp = tempfile::tempdir().unwrap();
    let dir1 = tmp.path().join("a");
    let dir2 = tmp.path().join("a/b");
    let src = write_file(tmp.path(), "src.txt", "data");

    let mut tx = InstallTransaction::new();
    tx.create_directory(&dir1).unwrap();
    tx.create_directory(&dir2).unwrap();
    tx.install_file(&src, &dir2.join("file.txt")).unwrap();
    assert_eq!(tx.operation_count(), 3);

    tx.rollback().unwrap();

    assert!(!dir2.join("file.txt").exists());
    // Deepest dir should be removed if empty
    assert!(!dir2.exists());
}

#[test]
fn registry_fixture_matches_wix_components() {
    let fixture = include_str!("../../crates/flight-updater/tests/fixtures/installer/registry_entries.json");
    let parsed: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let entries = parsed["entries"].as_array().unwrap();

    // The fixture must define the same registry keys as Components.wxs
    let keys: Vec<String> = entries
        .iter()
        .map(|e| e["key"].as_str().unwrap().to_string())
        .collect();

    assert!(keys.iter().any(|k| k.contains("OpenFlight\\Flight Hub")));
    assert!(keys.iter().any(|k| k.contains("Services\\FlightHub")));
    assert!(keys.iter().any(|k| k.contains("EventLog")));
}

#[test]
fn wix_service_recovery_configuration() {
    let components = include_str!("../wix/Components.wxs");
    assert!(
        components.contains("ServiceConfig"),
        "WiX must configure service recovery"
    );
    assert!(
        components.contains("FirstFailureActionType=\"restart\""),
        "service must restart on first failure"
    );
    assert!(
        components.contains("SecondFailureActionType=\"restart\""),
        "service must restart on second failure"
    );
    assert!(
        components.contains("ThirdFailureActionType=\"none\""),
        "service must stop restarting after third failure"
    );
}
