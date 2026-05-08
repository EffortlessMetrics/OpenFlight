// SPDX-License-Identifier: MIT OR Apache-2.0

//! Debian package builder.
//!
//! Replaces the historical shell wrapper with a cross-platform Rust xtask while
//! still invoking the Debian packaging tools that are intrinsically external.

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const PACKAGE_NAME: &str = "flight-hub";

pub fn run_debian_build(
    version: Option<String>,
    output_dir: Option<PathBuf>,
    skip_build: bool,
    configuration: &str,
) -> Result<()> {
    let repo_root = std::env::current_dir().context("failed to resolve workspace root")?;
    let script_dir = repo_root.join("installer/debian");
    let version = match version {
        Some(version) => version,
        None => workspace_version(&repo_root.join("Cargo.toml"))?,
    };
    let output_dir = output_dir.unwrap_or_else(|| script_dir.join("output"));
    let package_dir = script_dir.join(format!("{PACKAGE_NAME}_{version}_amd64"));
    let binary_dir = repo_root.join("target").join(configuration);

    print_header(&version, configuration, &output_dir, &repo_root);

    if skip_build {
        warn("Skipping Rust build (--skip-build)");
    } else {
        info(&format!("Building Rust binaries ({configuration})"));
        let mut cargo_args = vec!["build", "-p", "flight-service", "-p", "flight-cli"];
        if configuration == "release" {
            cargo_args.push("--release");
        }
        run(Command::new("cargo")
            .args(cargo_args)
            .current_dir(&repo_root))?;
        success("Rust binaries built");
    }

    for binary in ["flightd", "flightctl"] {
        let path = binary_dir.join(binary);
        if !path.is_file() {
            bail!(
                "binary not found: {} — run without --skip-build to build first",
                path.display()
            );
        }
    }

    info("Creating package directory structure");
    if package_dir.exists() {
        fs::remove_dir_all(&package_dir)
            .with_context(|| format!("failed to remove {}", package_dir.display()))?;
    }
    create_dirs(&package_dir)?;
    success(&format!(
        "Package directory created: {}",
        package_dir.display()
    ));

    info("Copying DEBIAN control files");
    copy_with_mode(
        script_dir.join("postinst"),
        package_dir.join("DEBIAN/postinst"),
        0o755,
    )?;
    copy_with_mode(
        script_dir.join("postrm"),
        package_dir.join("DEBIAN/postrm"),
        0o755,
    )?;
    let control = fs::read_to_string(script_dir.join("control"))
        .context("failed to read Debian control template")?
        .replace("{{VERSION}}", &version);
    fs::write(package_dir.join("DEBIAN/control"), control).context("failed to write control")?;
    success("Control files staged");

    info("Staging application files");
    copy_with_mode(
        binary_dir.join("flightd"),
        package_dir.join("usr/bin/flightd"),
        0o755,
    )?;
    copy_with_mode(
        binary_dir.join("flightctl"),
        package_dir.join("usr/bin/flightctl"),
        0o755,
    )?;
    success("Binaries staged");

    copy_with_mode(
        script_dir.join("99-flight-hub.rules"),
        package_dir.join("usr/share/flight-hub/99-flight-hub.rules"),
        0o644,
    )?;
    success("udev rules staged");

    copy_with_mode(
        script_dir.join("flightd.service"),
        package_dir.join("usr/lib/systemd/user/flightd.service"),
        0o644,
    )?;
    success("Systemd user service staged");

    let rt_script = repo_root.join("scripts/setup-linux-rt.sh");
    if rt_script.is_file() {
        copy_with_mode(
            rt_script,
            package_dir.join("usr/share/flight-hub/setup-linux-rt.sh"),
            0o755,
        )?;
        success("RT setup script staged");
    } else {
        warn("RT setup script not found — skipping");
    }

    info("Building .deb package");
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let deb_file = output_dir.join(format!("{PACKAGE_NAME}_{version}_amd64.deb"));
    run(Command::new("dpkg-deb")
        .arg("--build")
        .arg("--root-owner-group")
        .arg(&package_dir)
        .arg(&deb_file))?;
    success(&format!(".deb package created: {}", deb_file.display()));

    write_sha256(&deb_file)?;
    success(&format!("Checksum written: {}.sha256", deb_file.display()));

    fs::remove_dir_all(&package_dir)
        .with_context(|| format!("failed to remove {}", package_dir.display()))?;
    success("Staging directory cleaned up");

    print_summary(&deb_file, &version)?;
    Ok(())
}

fn workspace_version(cargo_toml: &Path) -> Result<String> {
    let content = fs::read_to_string(cargo_toml)
        .with_context(|| format!("failed to read {}", cargo_toml.display()))?;
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("version") {
            let Some((_, value)) = rest.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches('"');
            if !value.is_empty() {
                return Ok(value.to_string());
            }
        }
    }
    bail!("could not extract workspace package version from Cargo.toml")
}

fn create_dirs(package_dir: &Path) -> Result<()> {
    for dir in [
        "DEBIAN",
        "usr/bin",
        "usr/share/flight-hub",
        "usr/lib/systemd/user",
    ] {
        fs::create_dir_all(package_dir.join(dir))?;
    }
    Ok(())
}

fn copy_with_mode(src: impl AsRef<Path>, dst: impl AsRef<Path>, mode: u32) -> Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();
    fs::copy(src, dst)
        .with_context(|| format!("failed to copy {} to {}", src.display(), dst.display()))?;
    set_mode(dst, mode)?;
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = fs::Permissions::from_mode(mode);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("failed to set mode on {}", path.display()))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

fn write_sha256(path: &Path) -> Result<()> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let hash = Sha256::digest(&bytes);
    let mut file = fs::File::create(format!("{}.sha256", path.display()))?;
    writeln!(file, "{:x}  {}", hash, path.display())?;
    Ok(())
}

fn run(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("failed to run {:?}", command))?;
    if !status.success() {
        bail!("command {:?} failed with {status}", command);
    }
    Ok(())
}

fn print_header(version: &str, configuration: &str, output_dir: &Path, repo_root: &Path) {
    println!("\n============================================================");
    println!("   Flight Hub Debian Package Builder");
    println!("============================================================");
    println!("  Version:    {version}");
    println!("  Config:     {configuration}");
    println!("  Output dir: {}", output_dir.display());
    println!("  Repo root:  {}", repo_root.display());
    println!();
}

fn print_summary(deb_file: &Path, version: &str) -> Result<()> {
    let size = fs::metadata(deb_file)?.len();
    println!("\n============================================================");
    println!("   Build Complete!");
    println!("============================================================\n");
    println!("  Package:  {}", deb_file.display());
    println!("  Size:     {} bytes", size);
    println!("  Version:  {version}\n");
    println!("  To install:");
    println!("    sudo dpkg -i {}\n", deb_file.display());
    println!("  To install and satisfy dependencies:");
    println!("    sudo apt install {}\n", deb_file.display());
    Ok(())
}

fn info(message: &str) {
    println!("=== {message} ===");
}

fn success(message: &str) {
    println!("[OK] {message}");
}

fn warn(message: &str) {
    println!("[WARN] {message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_workspace_version() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cargo = temp_dir.path().join("Cargo.toml");
        fs::write(
            &cargo,
            "[workspace.package]\nversion = \"1.2.3\"\nedition = \"2024\"\n",
        )
        .unwrap();

        assert_eq!(workspace_version(&cargo).unwrap(), "1.2.3");
    }
}
