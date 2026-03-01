// SPDX-License-Identifier: MIT OR Apache-2.0

//! Installer packaging commands for Windows (MSI) and Linux (deb).
//!
//! These commands orchestrate the platform-specific build scripts that live
//! under `installer/wix/` (Windows) and `installer/debian/` (Linux).
//! When the required tooling is not available they print what *would* happen
//! so the commands are still useful for CI dry-runs and documentation.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

// ── Windows MSI packaging ───────────────────────────────────────────────────

/// Build a Windows MSI installer package via the WiX build script.
pub fn run_package_windows(skip_build: bool) -> Result<()> {
    println!("📦 Packaging Windows MSI installer\n");

    let build_script = Path::new("installer/wix/build.ps1");
    if !build_script.exists() {
        anyhow::bail!(
            "WiX build script not found at {}. Run from workspace root.",
            build_script.display()
        );
    }

    // Check for PowerShell
    let ps = which_powershell();
    if ps.is_none() {
        println!("⚠  PowerShell not found — printing dry-run steps:\n");
        print_windows_dry_run(skip_build);
        return Ok(());
    }
    let ps = ps.unwrap();

    // Check for WiX toolset
    if !wix_available() {
        println!("⚠  WiX Toolset not detected (candle.exe not in PATH or WIX env var).");
        println!("   Install WiX 3.x from https://wixtoolset.org/releases/\n");
        println!("   Printing dry-run steps instead:\n");
        print_windows_dry_run(skip_build);
        return Ok(());
    }

    // Invoke the build script
    let mut args = vec![
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        "installer\\wix\\build.ps1",
    ];
    if skip_build {
        args.push("-SkipBuild");
    }

    println!("Running: {} {}\n", ps, args.join(" "));
    let status = Command::new(&ps)
        .args(&args)
        .status()
        .context("Failed to run WiX build script")?;

    if !status.success() {
        anyhow::bail!(
            "Windows MSI packaging failed (exit code {:?})",
            status.code()
        );
    }

    println!("\n✅ Windows MSI package created successfully");
    Ok(())
}

fn which_powershell() -> Option<String> {
    for name in &["pwsh", "powershell"] {
        if Command::new(name).arg("-Version").output().is_ok() {
            return Some((*name).to_string());
        }
    }
    None
}

fn wix_available() -> bool {
    // Check WIX env var
    if let Ok(wix_dir) = std::env::var("WIX") {
        let candle = Path::new(&wix_dir).join("bin").join("candle.exe");
        if candle.exists() {
            return true;
        }
    }
    // Check PATH
    Command::new("candle").arg("-?").output().is_ok()
}

fn print_windows_dry_run(skip_build: bool) {
    if !skip_build {
        println!("  1. cargo build --release -p flight-service -p flight-cli");
    }
    println!("  2. Stage binaries and config into installer/wix/staging/");
    println!("  3. candle.exe -dVersion=<ver> Product.wxs Components.wxs");
    println!("  4. light.exe -ext WixUIExtension -ext WixUtilExtension -out FlightHub-<ver>.msi");
    println!("  5. Generate SHA256 checksum");
    println!("\n  Install WiX Toolset 3.x to run these steps automatically.");
}

// ── Linux deb packaging ─────────────────────────────────────────────────────

/// Build a Linux .deb package via the Debian build script.
pub fn run_package_linux(skip_build: bool) -> Result<()> {
    println!("📦 Packaging Linux deb installer\n");

    let build_script = Path::new("installer/debian/build.sh");
    if !build_script.exists() {
        anyhow::bail!(
            "Debian build script not found at {}. Run from workspace root.",
            build_script.display()
        );
    }

    // Check for dpkg-deb
    if !dpkg_deb_available() {
        println!("⚠  dpkg-deb not found — printing dry-run steps:\n");
        print_linux_dry_run(skip_build);
        return Ok(());
    }

    // Invoke the build script
    let mut cmd = Command::new("bash");
    cmd.arg("installer/debian/build.sh");

    if skip_build {
        cmd.env("SKIP_BUILD", "1");
    }

    println!("Running: bash installer/debian/build.sh\n");
    let status = cmd.status().context("Failed to run Debian build script")?;

    if !status.success() {
        anyhow::bail!("Linux deb packaging failed (exit code {:?})", status.code());
    }

    println!("\n✅ Linux deb package created successfully");
    Ok(())
}

fn dpkg_deb_available() -> bool {
    Command::new("dpkg-deb").arg("--version").output().is_ok()
}

fn print_linux_dry_run(skip_build: bool) {
    if !skip_build {
        println!("  1. cargo build --release -p flight-service -p flight-cli");
    }
    println!("  2. Create package directory structure (DEBIAN/, usr/bin/, etc.)");
    println!("  3. Stage binaries, udev rules, systemd unit, control files");
    println!("  4. dpkg-deb --build --root-owner-group <pkg-dir> <output>.deb");
    println!("  5. Generate SHA256 checksum");
    println!("\n  Install dpkg-deb (part of dpkg) to run these steps automatically.");
}
