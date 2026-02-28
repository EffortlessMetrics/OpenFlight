//! Install manifest definitions for Flight Hub.
//!
//! Provides platform-specific file lists, service registration info,
//! and config-preservation rules used by the installer and its tests.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ── Error type ───────────────────────────────────────────────────────────────

/// Errors that can occur during manifest validation.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("duplicate destination path: {0}")]
    DuplicateDestination(PathBuf),

    #[error("file entry has empty source: destination {0}")]
    EmptySource(PathBuf),

    #[error("file entry has empty destination: source {0}")]
    EmptyDestination(PathBuf),

    #[error("config preservation path is also in uninstall cleanup list: {0}")]
    PreservedPathInCleanup(PathBuf),
}

// ── Manifest types ───────────────────────────────────────────────────────────

/// A single file to install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Relative source path inside the build/staging area.
    pub source: PathBuf,
    /// Absolute destination path on the target system.
    pub destination: PathBuf,
    /// If `true`, the file is required for the installation to succeed.
    pub required: bool,
}

/// Service registration metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
    /// Path to the service binary (relative to install prefix).
    pub binary_path: PathBuf,
    pub auto_start: bool,
}

/// Full install manifest for a single platform target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallManifest {
    /// Files to install (source → destination).
    pub files: Vec<FileEntry>,
    /// Service registration information.
    pub service: ServiceInfo,
    /// Paths to remove on uninstall.
    pub cleanup_paths: Vec<PathBuf>,
    /// Paths that must survive uninstall / update (user config).
    pub preserved_paths: Vec<PathBuf>,
}

impl InstallManifest {
    /// Validate internal consistency of the manifest.
    ///
    /// Checks:
    /// - No empty source or destination paths.
    /// - No duplicate destination paths.
    /// - Preserved paths do not appear in the cleanup list.
    pub fn verify(&self) -> Result<(), ManifestError> {
        let mut seen_destinations = HashSet::new();

        for entry in &self.files {
            if entry.source.as_os_str().is_empty() {
                return Err(ManifestError::EmptySource(entry.destination.clone()));
            }
            if entry.destination.as_os_str().is_empty() {
                return Err(ManifestError::EmptyDestination(entry.source.clone()));
            }
            if !seen_destinations.insert(&entry.destination) {
                return Err(ManifestError::DuplicateDestination(
                    entry.destination.clone(),
                ));
            }
        }

        let cleanup_set: HashSet<&PathBuf> = self.cleanup_paths.iter().collect();
        for preserved in &self.preserved_paths {
            if cleanup_set.contains(preserved) {
                return Err(ManifestError::PreservedPathInCleanup(preserved.clone()));
            }
        }

        Ok(())
    }
}

// ── Platform-specific manifest builders ──────────────────────────────────────

/// Build the Windows (MSI) install manifest rooted at `prefix`.
///
/// Layout mirrors the WiX `Components.wxs` definitions:
/// ```text
/// <prefix>/bin/flightd.exe
/// <prefix>/bin/flightctl.exe
/// <prefix>/config/config.toml
/// <prefix>/config/default.profile.toml
/// <prefix>/logs/
/// ```
pub fn windows_manifest(prefix: &Path) -> InstallManifest {
    let bin = prefix.join("bin");
    let config = prefix.join("config");
    let logs = prefix.join("logs");

    InstallManifest {
        files: vec![
            FileEntry {
                source: PathBuf::from("bin/flightd.exe"),
                destination: bin.join("flightd.exe"),
                required: true,
            },
            FileEntry {
                source: PathBuf::from("bin/flightctl.exe"),
                destination: bin.join("flightctl.exe"),
                required: true,
            },
            FileEntry {
                source: PathBuf::from("config/config.toml"),
                destination: config.join("config.toml"),
                required: true,
            },
            FileEntry {
                source: PathBuf::from("config/default.profile.toml"),
                destination: config.join("default.profile.toml"),
                required: true,
            },
        ],
        service: ServiceInfo {
            name: "FlightHub".into(),
            display_name: "Flight Hub Service".into(),
            description: "Flight Hub input management service for flight simulation".into(),
            binary_path: PathBuf::from("bin/flightd.exe"),
            auto_start: true,
        },
        cleanup_paths: vec![bin, config.clone(), logs, prefix.to_path_buf()],
        preserved_paths: vec![config.join("config.toml")],
    }
}

/// Build the Linux (deb) install manifest rooted at `prefix`.
///
/// Layout mirrors the `debian/build.sh` staging structure:
/// ```text
/// <prefix>/usr/bin/flightd
/// <prefix>/usr/bin/flightctl
/// <prefix>/usr/share/flight-hub/99-flight-hub.rules
/// <prefix>/usr/lib/systemd/user/flightd.service
/// ```
pub fn linux_manifest(prefix: &Path) -> InstallManifest {
    let usr_bin = prefix.join("usr/bin");
    let share = prefix.join("usr/share/flight-hub");
    let systemd = prefix.join("usr/lib/systemd/user");

    InstallManifest {
        files: vec![
            FileEntry {
                source: PathBuf::from("usr/bin/flightd"),
                destination: usr_bin.join("flightd"),
                required: true,
            },
            FileEntry {
                source: PathBuf::from("usr/bin/flightctl"),
                destination: usr_bin.join("flightctl"),
                required: true,
            },
            FileEntry {
                source: PathBuf::from("usr/share/flight-hub/99-flight-hub.rules"),
                destination: share.join("99-flight-hub.rules"),
                required: true,
            },
            FileEntry {
                source: PathBuf::from("usr/lib/systemd/user/flightd.service"),
                destination: systemd.join("flightd.service"),
                required: true,
            },
        ],
        service: ServiceInfo {
            name: "flightd".into(),
            display_name: "Flight Hub daemon".into(),
            description: "Flight Hub daemon for flight simulation input management".into(),
            binary_path: PathBuf::from("usr/bin/flightd"),
            auto_start: false, // user service — not auto-started at system level
        },
        cleanup_paths: vec![usr_bin, share, systemd],
        preserved_paths: vec![
            PathBuf::from("~/.config/flight-hub"),
            PathBuf::from("~/.local/share/flight-hub"),
        ],
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_manifest_is_valid() {
        let m = windows_manifest(Path::new(r"C:\Program Files\Flight Hub"));
        assert!(m.verify().is_ok());
        assert!(m.files.iter().all(|f| f.required));
        assert_eq!(m.service.name, "FlightHub");
    }

    #[test]
    fn linux_manifest_is_valid() {
        let m = linux_manifest(Path::new("/"));
        assert!(m.verify().is_ok());
        assert_eq!(m.service.name, "flightd");
    }

    #[test]
    fn duplicate_destination_rejected() {
        let mut m = windows_manifest(Path::new("/tmp/test"));
        let dup = m.files[0].clone();
        m.files.push(dup);
        assert!(matches!(
            m.verify(),
            Err(ManifestError::DuplicateDestination(_))
        ));
    }

    #[test]
    fn empty_source_rejected() {
        let mut m = windows_manifest(Path::new("/tmp/test"));
        m.files[0].source = PathBuf::new();
        assert!(matches!(m.verify(), Err(ManifestError::EmptySource(_))));
    }

    #[test]
    fn empty_destination_rejected() {
        let mut m = windows_manifest(Path::new("/tmp/test"));
        m.files[0].destination = PathBuf::new();
        assert!(matches!(
            m.verify(),
            Err(ManifestError::EmptyDestination(_))
        ));
    }

    #[test]
    fn preserved_path_in_cleanup_rejected() {
        let mut m = windows_manifest(Path::new("/tmp/test"));
        // Put a preserved path into the cleanup list.
        let p = m.preserved_paths[0].clone();
        m.cleanup_paths.push(p);
        assert!(matches!(
            m.verify(),
            Err(ManifestError::PreservedPathInCleanup(_))
        ));
    }

    #[test]
    fn windows_manifest_preserves_user_config() {
        let m = windows_manifest(Path::new(r"C:\Program Files\Flight Hub"));
        assert!(
            m.preserved_paths
                .iter()
                .any(|p| p.to_string_lossy().contains("config.toml"))
        );
    }

    #[test]
    fn linux_manifest_preserves_user_dirs() {
        let m = linux_manifest(Path::new("/"));
        assert!(
            m.preserved_paths
                .iter()
                .any(|p| p.to_string_lossy().contains(".config/flight-hub"))
        );
    }

    #[test]
    fn manifest_roundtrips_through_json() {
        let m = windows_manifest(Path::new("/tmp/test"));
        let json = serde_json::to_string(&m).expect("serialize");
        let m2: InstallManifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(m.files.len(), m2.files.len());
        assert_eq!(m.service.name, m2.service.name);
    }
}
