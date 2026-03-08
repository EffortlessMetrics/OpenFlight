//! Depth tests — Edge cases for the installer.
//!
//! Validates per-user installs, pre-existing config, interrupted installs,
//! and multi-version coexistence.

use std::fs;
use std::path::Path;

use flight_installer::manifest::{self, InstallManifest};
use flight_installer::rollback::{InstallTransaction, TransactionError};

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
// 1. Install without admin (per-user install)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn per_user_install_uses_local_prefix() {
    let tmp = tempfile::tempdir().unwrap();
    // Simulate per-user install into LOCALAPPDATA equivalent.
    let local_prefix = tmp.path().join("AppData/Local/FlightHub");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&local_prefix);
    stage_sources(&staging, &m);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &local_prefix);
    tx.commit().unwrap();

    // All files under user-writable prefix.
    for entry in &m.files {
        assert!(
            entry.destination.starts_with(&local_prefix),
            "per-user install: {} not under {}",
            entry.destination.display(),
            local_prefix.display()
        );
        assert!(
            entry.destination.exists(),
            "per-user install: {} missing",
            entry.destination.display()
        );
    }
}

#[test]
fn per_user_manifest_is_valid() {
    // A per-user prefix must produce a valid manifest (no path collisions).
    let m = manifest::windows_manifest(Path::new(r"C:\Users\pilot\AppData\Local\FlightHub"));
    assert!(m.verify().is_ok());
    assert!(m.files.iter().all(|f| f.required));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Install with existing config
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn install_over_existing_config_creates_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    // Pre-create a config file to simulate existing config.
    let config_dest = &m
        .files
        .iter()
        .find(|f| f.destination.to_string_lossy().contains("config.toml"))
        .unwrap()
        .destination;
    if let Some(parent) = config_dest.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(config_dest, "pre-existing-user-config\n").unwrap();

    // Install: the transaction should back up the existing file.
    let mut tx = InstallTransaction::new();
    let src = staging.join("config/config.toml");
    tx.install_file(&src, config_dest).unwrap();

    // The .bak file should exist before commit.
    let bak = config_dest.with_extension("bak");
    assert!(
        bak.exists(),
        "backup of existing config must be created during install"
    );
    assert_eq!(
        fs::read_to_string(&bak).unwrap(),
        "pre-existing-user-config\n"
    );

    // Rollback: original config should be restored.
    tx.rollback().unwrap();
    assert_eq!(
        fs::read_to_string(config_dest).unwrap(),
        "pre-existing-user-config\n",
        "rollback must restore pre-existing config"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Interrupted install → clean state
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn interrupted_install_leaves_clean_state_after_rollback() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    // Partially install (only first 2 of 4 files).
    let mut tx = InstallTransaction::new();
    for entry in m.files.iter().take(2) {
        let src = staging.join(&entry.source);
        tx.install_file(&src, &entry.destination).unwrap();
    }

    // Verify partial state.
    assert!(m.files[0].destination.exists());
    assert!(m.files[1].destination.exists());
    assert!(!m.files[2].destination.exists());

    // Rollback simulates interrupted install cleanup.
    tx.rollback().unwrap();

    // All files must be gone.
    for entry in &m.files {
        assert!(
            !entry.destination.exists(),
            "after rollback, {} must not exist",
            entry.destination.display()
        );
    }
}

#[test]
fn cannot_operate_after_rollback() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src.txt");
    fs::write(&src, "data").unwrap();

    let mut tx = InstallTransaction::new();
    tx.rollback().unwrap();

    // All operations must fail with AlreadyRolledBack.
    assert!(matches!(
        tx.install_file(&src, &tmp.path().join("dst.txt")),
        Err(TransactionError::AlreadyRolledBack)
    ));
    assert!(matches!(
        tx.create_directory(&tmp.path().join("newdir")),
        Err(TransactionError::AlreadyRolledBack)
    ));
    assert!(matches!(
        tx.register_service("svc"),
        Err(TransactionError::AlreadyRolledBack)
    ));
    assert!(matches!(tx.commit(), Err(TransactionError::AlreadyRolledBack)));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Multiple versions coexist
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn multiple_versions_coexist_in_separate_prefixes() {
    let tmp = tempfile::tempdir().unwrap();
    let staging = tmp.path().join("staging");

    // Install v1 and v2 into separate prefixes.
    let prefix_v1 = tmp.path().join("v1");
    let prefix_v2 = tmp.path().join("v2");

    let m1 = manifest::windows_manifest(&prefix_v1);
    let m2 = manifest::windows_manifest(&prefix_v2);

    stage_sources(&staging, &m1);

    // Install v1.
    let mut tx1 = InstallTransaction::new();
    perform_install(&mut tx1, &staging, &m1, &prefix_v1);
    tx1.commit().unwrap();

    // Tag v1 binary.
    fs::write(&m1.files[0].destination, "v1-binary").unwrap();

    // Install v2 (re-use staging, different prefix).
    let mut tx2 = InstallTransaction::new();
    perform_install(&mut tx2, &staging, &m2, &prefix_v2);
    tx2.commit().unwrap();

    // Tag v2 binary.
    fs::write(&m2.files[0].destination, "v2-binary").unwrap();

    // Both versions exist independently.
    assert_eq!(
        fs::read_to_string(&m1.files[0].destination).unwrap(),
        "v1-binary"
    );
    assert_eq!(
        fs::read_to_string(&m2.files[0].destination).unwrap(),
        "v2-binary"
    );

    // Both manifests are independently valid.
    assert!(m1.verify().is_ok());
    assert!(m2.verify().is_ok());

    // No path overlap between v1 and v2.
    for e1 in &m1.files {
        for e2 in &m2.files {
            assert_ne!(
                e1.destination, e2.destination,
                "v1 and v2 must not share file paths"
            );
        }
    }
}
