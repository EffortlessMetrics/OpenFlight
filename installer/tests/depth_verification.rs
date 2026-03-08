//! Depth tests — Install verification, health checks, and validation.
//!
//! Validates that installation state can be audited: manifest verification,
//! health checks, permission checks, dependency checks, and signature stubs.

use std::fs;
use std::path::{Path, PathBuf};

use flight_installer::manifest::{self, InstallManifest, ManifestError};
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
// 1. Install verification command — manifest verify()
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn manifest_verify_catches_all_error_classes() {
    // Valid manifests pass.
    let w = manifest::windows_manifest(Path::new(r"C:\FH"));
    assert!(w.verify().is_ok());
    let l = manifest::linux_manifest(Path::new("/"));
    assert!(l.verify().is_ok());

    // Duplicate destination.
    let mut m = manifest::windows_manifest(Path::new("/tmp/t"));
    let dup = m.files[0].clone();
    m.files.push(dup);
    assert!(matches!(
        m.verify(),
        Err(ManifestError::DuplicateDestination(_))
    ));

    // Empty source.
    let mut m2 = manifest::windows_manifest(Path::new("/tmp/t"));
    m2.files[0].source = PathBuf::new();
    assert!(matches!(m2.verify(), Err(ManifestError::EmptySource(_))));

    // Empty destination.
    let mut m3 = manifest::windows_manifest(Path::new("/tmp/t"));
    m3.files[0].destination = PathBuf::new();
    assert!(matches!(
        m3.verify(),
        Err(ManifestError::EmptyDestination(_))
    ));

    // Preserved path in cleanup list.
    let mut m4 = manifest::windows_manifest(Path::new("/tmp/t"));
    let preserved = m4.preserved_paths[0].clone();
    m4.cleanup_paths.push(preserved);
    assert!(matches!(
        m4.verify(),
        Err(ManifestError::PreservedPathInCleanup(_))
    ));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Health check after install
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn health_check_passes_after_successful_install() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    // Health check: all required files exist and are non-empty.
    for entry in &m.files {
        assert!(
            entry.destination.exists(),
            "health check: {} missing",
            entry.destination.display()
        );
        let meta = fs::metadata(&entry.destination).unwrap();
        assert!(
            meta.len() > 0,
            "health check: {} is empty",
            entry.destination.display()
        );
    }

    // Service marker exists.
    let marker = prefix.join(".service_registered");
    assert!(marker.exists(), "health check: service marker missing");
}

#[test]
fn health_check_detects_missing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    // Sabotage: remove a required file.
    fs::remove_file(&m.files[0].destination).unwrap();

    // Health check should detect the missing file.
    let missing: Vec<&PathBuf> = m
        .files
        .iter()
        .filter(|e| e.required && !e.destination.exists())
        .map(|e| &e.destination)
        .collect();

    assert_eq!(missing.len(), 1, "health check must detect exactly one missing file");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Permission verification
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn installed_files_are_readable() {
    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    for entry in &m.files {
        // Verify content can be read without error.
        let _ = fs::read(&entry.destination).expect("must be able to read installed file");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Dependency check (validated via manifest metadata)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn dependency_check_validates_required_files() {
    // The manifest's `required` flag acts as the dependency check:
    // all entries needed for a functional install are marked required.
    let w = manifest::windows_manifest(Path::new(r"C:\FH"));
    let required_count = w.files.iter().filter(|f| f.required).count();
    assert!(
        required_count >= 2,
        "must have at least daemon + config as required"
    );

    let l = manifest::linux_manifest(Path::new("/"));
    let required_count = l.files.iter().filter(|f| f.required).count();
    assert!(
        required_count >= 2,
        "must have at least daemon + udev rules as required"
    );
}

#[test]
fn dependency_check_binary_path_consistent_with_files() {
    // The service binary_path must correspond to a file in the manifest.
    let w = manifest::windows_manifest(Path::new(r"C:\FH"));
    let bin_name = w
        .service
        .binary_path
        .file_name()
        .unwrap()
        .to_string_lossy();
    assert!(
        w.files
            .iter()
            .any(|f| f.source.to_string_lossy().contains(bin_name.as_ref())),
        "service binary must be listed in manifest files"
    );

    let l = manifest::linux_manifest(Path::new("/"));
    let bin_name = l
        .service
        .binary_path
        .file_name()
        .unwrap()
        .to_string_lossy();
    assert!(
        l.files
            .iter()
            .any(|f| f.source.to_string_lossy().contains(bin_name.as_ref())),
        "service binary must be listed in manifest files"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Signature verification on install (stub / hash-based)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn signature_verification_detects_tampering() {
    use std::collections::HashMap;

    let tmp = tempfile::tempdir().unwrap();
    let prefix = tmp.path().join("install");
    let staging = tmp.path().join("staging");

    let m = manifest::windows_manifest(&prefix);
    stage_sources(&staging, &m);

    let mut tx = InstallTransaction::new();
    perform_install(&mut tx, &staging, &m, &prefix);
    tx.commit().unwrap();

    // Compute hash-based checksums.
    use std::hash::{DefaultHasher, Hasher};
    fn hash_file(path: &Path) -> u64 {
        let mut h = DefaultHasher::new();
        h.write(&fs::read(path).unwrap());
        h.finish()
    }

    let mut checksums: HashMap<PathBuf, u64> = HashMap::new();
    for entry in &m.files {
        checksums.insert(entry.destination.clone(), hash_file(&entry.destination));
    }

    // Verify all files match.
    for (path, expected_hash) in &checksums {
        assert_eq!(
            hash_file(path),
            *expected_hash,
            "checksum mismatch for {}",
            path.display()
        );
    }

    // Tamper with a file.
    let daemon_index = m
        .files
        .iter()
        .position(|entry| {
            let dest = entry.destination.to_string_lossy().replace('\\', "/");
            dest.ends_with("bin/flightd.exe") || dest.ends_with("usr/bin/flightd")
        })
        .expect("daemon binary not found in manifest");
    let daemon_dest = &m.files[daemon_index].destination;
    fs::write(daemon_dest, "TAMPERED CONTENT").unwrap();
    let tampered_hash = hash_file(daemon_dest);
    let original_hash = checksums[daemon_dest];
    assert_ne!(
        tampered_hash, original_hash,
        "tampered file must differ from original checksum"
    );
}

#[test]
fn manifest_roundtrip_integrity() {
    // Verify that manifest serialization is stable — a form of signature verification.
    let m = manifest::windows_manifest(Path::new(r"C:\FH"));
    let json1 = serde_json::to_string_pretty(&m).unwrap();
    let m2: InstallManifest = serde_json::from_str(&json1).unwrap();
    let json2 = serde_json::to_string_pretty(&m2).unwrap();
    let v1: serde_json::Value = serde_json::from_str(&json1).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&json2).unwrap();
    assert_eq!(v1, v2, "manifest must roundtrip identically through JSON (semantic equality)");
}
