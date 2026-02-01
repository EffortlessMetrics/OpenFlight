// SPDX-License-Identifier: MIT OR Apache-2.0

//! HOTAS device verification tools.
//!
//! These commands help verify protocol claims for Saitek/Logitech HOTAS devices.

use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum HotasCommand {
    /// List connected HOTAS devices with VID/PID info
    Enumerate,

    /// Capture raw HID reports from a device
    Capture {
        /// Device path or VID:PID
        device: String,
        /// Output file for captured data
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Duration to capture in seconds
        #[arg(short, long, default_value = "10")]
        duration: u64,
    },

    /// Dump HID report descriptor for a device
    Descriptor {
        /// Device path or VID:PID
        device: String,
        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Replay captured control transfers (for protocol testing)
    Replay {
        /// Capture file to replay
        capture: PathBuf,
        /// Target device path or VID:PID
        #[arg(short, long)]
        device: Option<String>,
    },
}

pub fn run(cmd: HotasCommand) -> Result<()> {
    match cmd {
        HotasCommand::Enumerate => enumerate(),
        HotasCommand::Capture {
            device,
            output,
            duration,
        } => capture(&device, output, duration),
        HotasCommand::Descriptor { device, output } => descriptor(&device, output),
        HotasCommand::Replay { capture, device } => replay(&capture, device.as_deref()),
    }
}

fn enumerate() -> Result<()> {
    println!("HOTAS Device Enumeration");
    println!("========================");
    println!();

    // Check if hidapi is available
    #[cfg(feature = "hidapi")]
    {
        use hidapi::HidApi;

        let api = HidApi::new().context("Failed to initialize HID API")?;

        let known_vids = [
            (0x06A3, "Saitek"),
            (0x0738, "Mad Catz"),
            (0x046D, "Logitech"),
        ];

        let mut found = false;

        for device in api.device_list() {
            let vid = device.vendor_id();
            let pid = device.product_id();

            // Check if it's a known HOTAS vendor
            if let Some((_, vendor_name)) = known_vids.iter().find(|(v, _)| *v == vid) {
                found = true;
                println!("Found: {} device", vendor_name);
                println!("  VID:PID: {:04X}:{:04X}", vid, pid);
                println!("  Path: {}", device.path().to_string_lossy());
                if let Some(product) = device.product_string() {
                    println!("  Product: {}", product);
                }
                if let Some(serial) = device.serial_number() {
                    println!("  Serial: {}", serial);
                }
                println!("  Usage Page: 0x{:04X}", device.usage_page());
                println!("  Usage: 0x{:04X}", device.usage());
                println!();
            }
        }

        if !found {
            println!("No Saitek/Logitech HOTAS devices found.");
            println!();
            println!("Supported vendors:");
            for (vid, name) in &known_vids {
                println!("  {} (VID: 0x{:04X})", name, vid);
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "hidapi"))]
    {
        println!("HID enumeration requires the 'hidapi' feature.");
        println!();
        println!("To enable, add to xtask/Cargo.toml:");
        println!("  hidapi = \"2\"");
        println!();
        println!("Or use platform tools:");
        println!("  Windows: Device Manager or USBView");
        println!("  Linux: lsusb -v | grep -A 20 '06a3\\|0738\\|046d'");
        println!("  macOS: system_profiler SPUSBDataType");

        Ok(())
    }
}

fn capture(device: &str, output: Option<PathBuf>, duration: u64) -> Result<()> {
    println!("HID Report Capture");
    println!("==================");
    println!();
    println!("Device: {}", device);
    println!("Duration: {} seconds", duration);
    if let Some(ref path) = output {
        println!("Output: {}", path.display());
    }
    println!();

    // Placeholder - actual implementation would:
    // 1. Open the HID device
    // 2. Read reports in a loop for the specified duration
    // 3. Write to output file or stdout

    println!("NOTE: HID capture not yet implemented.");
    println!();
    println!("For now, use platform-specific tools:");
    println!("  Windows: Wireshark + USBPcap");
    println!("  Linux: Wireshark + usbmon, or: sudo cat /dev/hidraw0 | xxd");
    println!("  macOS: Wireshark + macOS USB capture");
    println!();
    println!("Save captures to: fixtures/hotas/<device>/");

    Ok(())
}

fn descriptor(device: &str, output: Option<PathBuf>) -> Result<()> {
    println!("HID Report Descriptor Dump");
    println!("==========================");
    println!();
    println!("Device: {}", device);
    if let Some(ref path) = output {
        println!("Output: {}", path.display());
    }
    println!();

    // Placeholder - actual implementation would:
    // 1. Open the HID device
    // 2. Request the report descriptor
    // 3. Parse and display or save to file

    println!("NOTE: Descriptor dump not yet implemented.");
    println!();
    println!("For now, use platform-specific tools:");
    println!(
        "  Linux: sudo usbhid-dump -d {:04x}:{:04x} -e descriptor",
        0x06A3, 0x0762
    ); // Example for X52 Pro
    println!("  Windows: USBView or HID descriptor tools");
    println!();
    println!("Save descriptors to: fixtures/hotas/<device>/descriptor.bin");

    Ok(())
}

fn replay(capture: &PathBuf, device: Option<&str>) -> Result<()> {
    println!("Control Transfer Replay");
    println!("=======================");
    println!();
    println!("Capture file: {}", capture.display());
    if let Some(dev) = device {
        println!("Target device: {}", dev);
    }
    println!();

    // Check if capture file exists
    if !capture.exists() {
        anyhow::bail!("Capture file not found: {}", capture.display());
    }

    // Placeholder - actual implementation would:
    // 1. Parse the capture file
    // 2. Open the target HID device
    // 3. Replay the control transfers
    // 4. Report results

    println!("NOTE: Replay not yet implemented.");
    println!();
    println!("This feature is for protocol verification:");
    println!("  1. Capture official software setting MFD text");
    println!("  2. Isolate the control transfer packets");
    println!("  3. Replay to confirm understanding");
    println!();
    println!("See docs/reference/hotas-claims.md for verification process.");

    Ok(())
}
