//! Deb package validation tests.
//!
//! Validates the Linux Debian packaging against `installer/linux/files.toml`,
//! the actual `debian/` scripts, and the `flight_installer::manifest` module.

use std::collections::HashSet;
use std::path::Path;

use flight_installer::manifest::{self, InstallManifest};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn load_linux_spec() -> toml::Value {
    let path = workspace_root().join("installer/linux/files.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    text.parse::<toml::Value>().expect("invalid TOML")
}

fn read_debian_file(name: &str) -> String {
    let path = workspace_root().join("installer/debian").join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

fn linux_manifest() -> InstallManifest {
    manifest::linux_manifest(Path::new("/"))
}

// ═════════════════════════════════════════════════════════════════════════════
// Control file fields
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn deb_control_has_required_fields() {
    let control = read_debian_file("control");
    for field in ["Package:", "Version:", "Architecture:", "Depends:"] {
        assert!(
            control.contains(field),
            "control file missing field: {field}"
        );
    }
}

#[test]
fn deb_control_package_name_matches_spec() {
    let spec = load_linux_spec();
    let expected = spec["control"]["package"].as_str().unwrap();
    let control = read_debian_file("control");
    assert!(
        control.contains(&format!("Package: {expected}")),
        "control file package name does not match spec"
    );
}

#[test]
fn deb_control_architecture_is_amd64() {
    let control = read_debian_file("control");
    assert!(
        control.contains("Architecture: amd64"),
        "control file must specify amd64 architecture"
    );
}

#[test]
fn deb_control_depends_includes_libc() {
    let control = read_debian_file("control");
    let deps_line = control
        .lines()
        .find(|l| l.starts_with("Depends:"))
        .expect("no Depends line");
    assert!(
        deps_line.contains("libc6"),
        "Depends must include libc6"
    );
}

#[test]
fn deb_control_depends_includes_libudev() {
    let control = read_debian_file("control");
    let deps_line = control
        .lines()
        .find(|l| l.starts_with("Depends:"))
        .expect("no Depends line");
    assert!(
        deps_line.contains("libudev1"),
        "Depends must include libudev1"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// postinst script
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn deb_postinst_creates_udev_rules() {
    let script = read_debian_file("postinst");
    assert!(
        script.contains("99-flight-hub.rules"),
        "postinst must reference udev rules file"
    );
    assert!(
        script.contains("/etc/udev/rules.d/"),
        "postinst must copy rules to /etc/udev/rules.d/"
    );
}

#[test]
fn deb_postinst_adds_group_membership() {
    let script = read_debian_file("postinst");
    assert!(
        script.contains("usermod") && script.contains("input"),
        "postinst must add user to input group"
    );
}

#[test]
fn deb_postinst_reloads_udev() {
    let script = read_debian_file("postinst");
    assert!(
        script.contains("udevadm control --reload-rules"),
        "postinst must reload udev rules"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// postrm script
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn deb_postrm_removes_udev_rules() {
    let script = read_debian_file("postrm");
    assert!(
        script.contains("rm") && script.contains("99-flight-hub.rules"),
        "postrm must remove udev rules"
    );
}

#[test]
fn deb_postrm_removes_config_on_purge() {
    let script = read_debian_file("postrm");
    assert!(
        script.contains("purge"),
        "postrm must handle purge action"
    );
    assert!(
        script.contains("/etc/flight-hub") || script.contains("CONFIG_DIR"),
        "postrm purge must clean system config directory"
    );
}

#[test]
fn deb_postrm_reloads_udev() {
    let script = read_debian_file("postrm");
    assert!(
        script.contains("udevadm control --reload-rules"),
        "postrm must reload udev rules after removal"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// systemd unit file
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn deb_systemd_unit_has_service_section() {
    let unit = read_debian_file("flightd.service");
    assert!(
        unit.contains("[Service]"),
        "systemd unit must contain [Service] section"
    );
}

#[test]
fn deb_systemd_exec_start_points_to_flightd() {
    let unit = read_debian_file("flightd.service");
    assert!(
        unit.contains("ExecStart=/usr/bin/flightd"),
        "ExecStart must point to /usr/bin/flightd"
    );
}

#[test]
fn deb_systemd_is_user_mode() {
    let unit = read_debian_file("flightd.service");
    assert!(
        unit.contains("WantedBy=default.target"),
        "unit must be user-mode (WantedBy=default.target)"
    );
}

#[test]
fn deb_systemd_restart_on_failure() {
    let unit = read_debian_file("flightd.service");
    assert!(
        unit.contains("Restart=on-failure"),
        "unit must restart on failure"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// Binary permissions
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn deb_spec_binaries_have_755() {
    let spec = load_linux_spec();
    for f in spec["files"].as_array().unwrap() {
        let dest = f["destination"].as_str().unwrap();
        let perms = f["permissions"].as_str().unwrap();
        if dest.starts_with("/usr/bin/") {
            assert_eq!(perms, "755", "binary {dest} must have 755 permissions");
        }
    }
}

#[test]
fn deb_spec_configs_have_644() {
    let spec = load_linux_spec();
    for f in spec["files"].as_array().unwrap() {
        let dest = f["destination"].as_str().unwrap();
        let perms = f["permissions"].as_str().unwrap();
        if !dest.starts_with("/usr/bin/") {
            assert_eq!(perms, "644", "non-binary {dest} must have 644 permissions");
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Manifest consistency
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn deb_manifest_validation_passes() {
    let m = linux_manifest();
    m.verify().expect("Linux manifest validation failed");
}

#[test]
fn deb_manifest_file_count_matches_spec() {
    let spec = load_linux_spec();
    let spec_files = spec["files"].as_array().unwrap();
    let m = linux_manifest();
    assert_eq!(m.files.len(), spec_files.len());
}

#[test]
fn deb_manifest_sources_match_spec() {
    let spec = load_linux_spec();
    let spec_sources: HashSet<String> = spec["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["source"].as_str().unwrap().to_string())
        .collect();
    let manifest_sources: HashSet<String> = linux_manifest()
        .files
        .iter()
        .map(|f| f.source.to_string_lossy().replace('\\', "/"))
        .collect();
    assert_eq!(spec_sources, manifest_sources);
}

#[test]
fn deb_manifest_preserves_user_dirs() {
    let m = linux_manifest();
    let preserved: Vec<String> = m
        .preserved_paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    assert!(preserved.iter().any(|p| p.contains(".config/flight-hub")));
    assert!(preserved.iter().any(|p| p.contains(".local/share/flight-hub")));
}
