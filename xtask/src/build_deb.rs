// SPDX-License-Identifier: MIT OR Apache-2.0

//! Debian package builder for Flight Hub.

use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub fn run_build_deb(version: Option<&str>, output_dir: Option<&str>) -> Result<()> {
    let repo_root = env::current_dir().context("failed to determine workspace root")?;
    let script_dir = repo_root.join("installer/debian");
    let version = match version {
        Some(value) => value.to_owned(),
        None => read_workspace_version(&repo_root.join("Cargo.toml"))?,
    };
    let output_dir = output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| script_dir.join("output"));
    let skip_build = env::var("SKIP_BUILD").unwrap_or_else(|_| "0".to_string()) == "1";
    let configuration = env::var("CONFIGURATION").unwrap_or_else(|_| "release".to_string());
    let bin_dir = repo_root.join("target").join(&configuration);
    let pkg_name = format!("flight-hub_{version}_amd64");
    let pkg_dir = script_dir.join(&pkg_name);

    println!();
    println!("============================================================");
    println!("   Flight Hub Debian Package Builder");
    println!("============================================================");
    println!("  Version:    {version}");
    println!("  Config:     {configuration}");
    println!("  Output dir: {}", output_dir.display());
    println!("  Repo root:  {}", repo_root.display());
    println!();

    if skip_build {
        warn("Skipping Rust build (SKIP_BUILD=1)");
    } else {
        info(&format!("Building Rust binaries ({configuration})"));
        let mut args = vec!["build", "-p", "flight-service", "-p", "flight-cli"];
        if configuration == "release" {
            args.push("--release");
        }
        run(Command::new("cargo").args(args).current_dir(&repo_root))?;
        success("Rust binaries built");
    }

    for bin in ["flightd", "flightctl"] {
        let path = bin_dir.join(bin);
        if !path.is_file() {
            bail!(
                "Binary not found: {} — run without SKIP_BUILD=1 to build first",
                path.display()
            );
        }
    }

    info("Creating package directory structure");
    if pkg_dir.exists() {
        fs::remove_dir_all(&pkg_dir)
            .with_context(|| format!("failed to remove {}", pkg_dir.display()))?;
    }
    create_dir(pkg_dir.join("DEBIAN"))?;
    create_dir(pkg_dir.join("usr/bin"))?;
    create_dir(pkg_dir.join("usr/share/flight-hub"))?;
    create_dir(pkg_dir.join("usr/lib/systemd/user"))?;
    success(&format!("Package directory created: {}", pkg_dir.display()));

    info("Copying DEBIAN control files");
    copy(
        &script_dir.join("postinst"),
        &pkg_dir.join("DEBIAN/postinst"),
    )?;
    copy(&script_dir.join("postrm"), &pkg_dir.join("DEBIAN/postrm"))?;
    set_executable(&pkg_dir.join("DEBIAN/postinst"))?;
    set_executable(&pkg_dir.join("DEBIAN/postrm"))?;

    let control =
        fs::read_to_string(script_dir.join("control")).context("failed to read control")?;
    fs::write(
        pkg_dir.join("DEBIAN/control"),
        control.replace("{{VERSION}}", &version),
    )
    .context("failed to write rendered control file")?;
    success("Control files staged");

    info("Staging application files");
    install(
        &bin_dir.join("flightd"),
        &pkg_dir.join("usr/bin/flightd"),
        0o755,
    )?;
    install(
        &bin_dir.join("flightctl"),
        &pkg_dir.join("usr/bin/flightctl"),
        0o755,
    )?;
    success("Binaries staged");

    install(
        &script_dir.join("99-flight-hub.rules"),
        &pkg_dir.join("usr/share/flight-hub/99-flight-hub.rules"),
        0o644,
    )?;
    success("udev rules staged");

    install(
        &script_dir.join("flightd.service"),
        &pkg_dir.join("usr/lib/systemd/user/flightd.service"),
        0o644,
    )?;
    success("Systemd user service staged");

    let rt_script = repo_root.join("scripts/setup-linux-rt.sh");
    if rt_script.is_file() {
        install(
            &rt_script,
            &pkg_dir.join("usr/share/flight-hub/setup-linux-rt.sh"),
            0o755,
        )?;
        success("RT setup script staged");
    } else {
        warn(&format!(
            "RT setup script not found at {} — skipping (postinst step 4 will still reference it)",
            rt_script.display()
        ));
    }

    info("Building .deb package");
    create_dir(&output_dir)?;
    let deb_file = output_dir.join(format!("{pkg_name}.deb"));
    run(Command::new("dpkg-deb")
        .args(["--build", "--root-owner-group"])
        .arg(&pkg_dir)
        .arg(&deb_file))?;
    success(&format!(".deb package created: {}", deb_file.display()));

    let checksum = Command::new("sha256sum")
        .arg(&deb_file)
        .output()
        .context("failed to run sha256sum")?;
    if !checksum.status.success() {
        bail!("sha256sum failed for {}", deb_file.display());
    }
    fs::write(format!("{}.sha256", deb_file.display()), checksum.stdout)
        .context("failed to write checksum file")?;
    success(&format!("Checksum written: {}.sha256", deb_file.display()));

    fs::remove_dir_all(&pkg_dir)
        .with_context(|| format!("failed to clean staging directory {}", pkg_dir.display()))?;
    success("Staging directory cleaned up");

    let deb_size = String::from_utf8(
        Command::new("du")
            .args(["-sh"])
            .arg(&deb_file)
            .output()
            .context("failed to run du")?
            .stdout,
    )
    .unwrap_or_else(|_| "unknown".to_string())
    .split_whitespace()
    .next()
    .unwrap_or("unknown")
    .to_string();

    println!();
    println!("============================================================");
    println!("   Build Complete!");
    println!("============================================================");
    println!();
    println!("  Package:  {}", deb_file.display());
    println!("  Size:     {deb_size}");
    println!("  Version:  {version}");
    println!();
    println!("  To install:");
    println!("    sudo dpkg -i {}", deb_file.display());
    println!();
    println!("  To install and satisfy dependencies:");
    println!("    sudo apt install {}", deb_file.display());
    println!();

    Ok(())
}

fn read_workspace_version(cargo_toml: &Path) -> Result<String> {
    let content = fs::read_to_string(cargo_toml)
        .with_context(|| format!("failed to read {}", cargo_toml.display()))?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("version")
            && let Some((_, value)) = rest.split_once('=')
        {
            let version = value.trim().trim_matches('"');
            if !version.is_empty() {
                return Ok(version.to_string());
            }
        }
    }
    bail!("could not extract version from {}", cargo_toml.display())
}

fn create_dir(path: impl AsRef<Path>) -> Result<()> {
    fs::create_dir_all(path.as_ref())
        .with_context(|| format!("failed to create {}", path.as_ref().display()))
}

fn copy(source: &Path, destination: &Path) -> Result<()> {
    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn install(source: &Path, destination: &Path, mode: u32) -> Result<()> {
    copy(source, destination)?;
    set_mode(destination, mode)
}

fn set_executable(path: &Path) -> Result<()> {
    set_mode(path, 0o755)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("failed to set mode on {}", path.display()))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

fn run(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("failed to run command: {command:?}"))?;
    if !status.success() {
        bail!("command failed with status {status}: {command:?}");
    }
    Ok(())
}

fn info(message: &str) {
    println!("\x1b[36m=== {message} ===\x1b[0m");
}

fn success(message: &str) {
    println!("\x1b[32m[OK] {message}\x1b[0m");
}

fn warn(message: &str) {
    println!("\x1b[33m[WARN] {message}\x1b[0m");
}
