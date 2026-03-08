//! Depth tests — Upgrade behaviour for the installer.
//!
//! Validates config preservation, rollback on failure, version-downgrade
//! prevention, database migration, and sim-integration updates.

use std::fs;
use std::path::Path;

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

// ═══════════════════════════════════════════════════════════════════════════════
// 1. In-place upgrade preserves config
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn inplace_upgrade_preserves_user_config() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    // Install v1.
    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    // User edits config.
    let config_entry = m
        .files
        .iter()
        .find(|e| e.source.to_string_lossy().contains("config.toml"))
        .expect("config.toml not found in manifest");
    let user_config = config_entry.destination.clone();
    let custom = "# v1 user config\nmy_key = 42\n";
    fs::write(&user_config, custom).unwrap();

    // Upgrade: skip preserved paths.
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
    tx2.commit().unwrap();

    assert_eq!(
        fs::read_to_string(&user_config).unwrap(),
        custom,
        "user config must survive upgrade"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Rollback on upgrade failure
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn rollback_restores_original_on_upgrade_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    // Install v1.
    let mut tx1 = InstallTransaction::new();
    perform_install(&mut tx1, &staging, &m, &prefix);
    tx1.commit().unwrap();

    // Tag original binary content.
    let daemon_index = m
        .files
        .iter()
        .position(|entry| {
            let dest = entry.destination.to_string_lossy().replace('\\', "/");
            dest.ends_with("bin/flightd.exe") || dest.ends_with("usr/bin/flightd")
        })
        .expect("daemon binary not found in manifest");
    let daemon = &m.files[daemon_index].destination;
    fs::write(daemon, "v1-daemon-binary").unwrap();

    // Begin upgrade.
    let mut tx2 = InstallTransaction::new();
    let src = staging.join(&m.files[daemon_index].source);
    tx2.install_file(&src, daemon).unwrap();
    // Content is now v2.
    assert_ne!(fs::read_to_string(daemon).unwrap(), "v1-daemon-binary");

    // Simulate failure → rollback.
    tx2.rollback().unwrap();
    assert_eq!(
        fs::read_to_string(daemon).unwrap(),
        "v1-daemon-binary",
        "rollback must restore v1 binary"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Version check prevents downgrade
// ═══════════════════════════════════════════════════════════════════════════════

/// Simulates a version-check gate: the installer should refuse to install an
/// older version over a newer one.
#[test]
fn version_check_prevents_downgrade() {
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    struct SemVer(u32, u32, u32);

    impl SemVer {
        fn parse(s: &str) -> Self {
            let parts: Vec<u32> = s.split('.').map(|p| p.parse().unwrap()).collect();
            Self(parts[0], parts[1], parts[2])
        }
    }

    fn check_upgrade(installed: &str, candidate: &str) -> Result<(), String> {
        let i = SemVer::parse(installed);
        let c = SemVer::parse(candidate);
        if c < i {
            Err(format!(
                "downgrade from {installed} to {candidate} is not allowed"
            ))
        } else {
            Ok(())
        }
    }

    // Upgrade allowed.
    assert!(check_upgrade("1.0.0", "1.1.0").is_ok());
    assert!(check_upgrade("1.0.0", "2.0.0").is_ok());
    assert!(check_upgrade("1.0.0", "1.0.0").is_ok()); // same version ok

    // Downgrade rejected.
    assert!(check_upgrade("2.0.0", "1.9.9").is_err());
    assert!(check_upgrade("1.1.0", "1.0.9").is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Database migration on upgrade
// ═══════════════════════════════════════════════════════════════════════════════

/// Simulates a database migration step during upgrade: the installer must
/// create or update a schema-version marker.
#[test]
fn database_migration_on_upgrade() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    fs::create_dir_all(&data_dir).unwrap();

    // v1 schema.
    let schema_file = data_dir.join("schema_version");
    fs::write(&schema_file, "1").unwrap();

    // Simulate upgrade migration.
    fn migrate(data_dir: &Path, target_version: u32) -> Result<(), String> {
        let schema_file = data_dir.join("schema_version");
        let current: u32 = if schema_file.exists() {
            fs::read_to_string(&schema_file)
                .unwrap()
                .trim()
                .parse()
                .unwrap()
        } else {
            0
        };
        if target_version < current {
            return Err("cannot downgrade schema".into());
        }
        fs::write(&schema_file, target_version.to_string()).unwrap();
        Ok(())
    }

    assert!(migrate(&data_dir, 2).is_ok());
    assert_eq!(fs::read_to_string(&schema_file).unwrap(), "2");

    assert!(migrate(&data_dir, 3).is_ok());
    assert_eq!(fs::read_to_string(&schema_file).unwrap(), "3");

    // Downgrade rejected.
    assert!(migrate(&data_dir, 1).is_err());
    // Schema still at 3.
    assert_eq!(fs::read_to_string(&schema_file).unwrap(), "3");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Sim integration updated on upgrade
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn sim_integration_files_updated_on_upgrade() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("root");
    let staging = tmp.path().join("staging");

    let m = manifest::linux_manifest(&prefix);
    stage_sources(&staging, &m);

    // Install v1.
    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    // Mark udev rules with v1 content.
    let udev = prefix.join("usr/share/flight-hub/99-flight-hub.rules");
    fs::write(&udev, "v1-rules").unwrap();

    // Prepare v2 staging with updated rules.
    let v2_staging = tmp.path().join("staging_v2");
    for entry in &m.files {
        let src = v2_staging.join(&entry.source);
        if let Some(parent) = src.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        if entry
            .source
            .to_string_lossy()
            .contains("99-flight-hub.rules")
        {
            fs::write(&src, "SUBSYSTEM==\"hidraw\", MODE=\"0660\", GROUP=\"input\" # v2\n").unwrap();
        } else {
            fs::write(&src, format!("v2-{}", entry.source.display())).unwrap();
        }
    }

    // Install v2 — sim integration files are not preserved, so they get updated.
    let mut tx2 = InstallTransaction::new();
    for entry in &m.files {
        let is_preserved = m
            .preserved_paths
            .iter()
            .any(|p| entry.destination.ends_with(p) || *p == entry.destination);
        if is_preserved {
            continue;
        }
        let src = v2_staging.join(&entry.source);
        tx2.install_file(&src, &entry.destination).unwrap();
    }
    tx2.commit().unwrap();

    let content = fs::read_to_string(&udev).unwrap();
    assert!(
        content.contains("v2"),
        "udev rules must be updated to v2 content"
    );
}

#[test]
fn linux_systemd_unit_updated_on_upgrade() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("root");
    let staging = tmp.path().join("staging");

    let m = manifest::linux_manifest(&prefix);
    stage_sources(&staging, &m);

    // Install v1.
    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    let unit = prefix.join("usr/lib/systemd/user/flightd.service");
    fs::write(&unit, "[Service]\nExecStart=/usr/bin/flightd # v1\n").unwrap();

    // Prepare v2 staging.
    let v2_staging = tmp.path().join("staging_v2");
    for entry in &m.files {
        let src = v2_staging.join(&entry.source);
        if let Some(parent) = src.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        if entry.source.to_string_lossy().contains("flightd.service") {
            fs::write(
                &src,
                "[Unit]\nDescription=Flight Hub v2\n\n[Service]\nExecStart=/usr/bin/flightd\n",
            )
            .unwrap();
        } else {
            fs::write(&src, format!("v2-{}", entry.source.display())).unwrap();
        }
    }

    let mut tx2 = InstallTransaction::new();
    for entry in &m.files {
        let is_preserved = m
            .preserved_paths
            .iter()
            .any(|p| entry.destination.ends_with(p) || *p == entry.destination);
        if is_preserved {
            continue;
        }
        let src = v2_staging.join(&entry.source);
        tx2.install_file(&src, &entry.destination).unwrap();
    }
    tx2.commit().unwrap();

    let content = fs::read_to_string(&unit).unwrap();
    assert!(
        content.contains("v2"),
        "systemd unit must be updated to v2"
    );
}
