// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! # T.Flight HOTAS Report Dumper
//!
//! Lightweight "no-daemon" capture tool: enumerates T.Flight HOTAS devices,
//! reads raw HID reports, parses each one with `TFlightInputHandler`, and prints
//! the decoded state plus the raw hex on stdout.
//!
//! Redirect stdout to build hardware-receipt log files:
//!
//! ```sh
//! cargo run -p flight-hotas-thrustmaster --example tflight_dump \
//!   > receipts/hid/thrustmaster/tflight-hotas4/windows-driver/merged_reports.log
//! ```
//!
//! ## Flags
//!
//! | Flag | Description |
//! |------|-------------|
//! | `--strip-report-id` | Strip the leading Report ID byte before parsing |
//! | `--invert-throttle` | Invert throttle axis (0.0 ↔ 1.0) |
//! | `--yaw=auto\|twist\|aux` | Yaw source policy (default: auto) |
//! | `--duration=<secs>` | Run for N seconds then exit (default: run until Ctrl-C) |

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flight_hotas_thrustmaster::{AxisMode, TFlightInputHandler, TFlightModel, TFlightYawPolicy};
use hidapi::HidApi;

const THRUSTMASTER_VID: u16 = 0x044F;
const HOTAS4_PID: u16 = 0xB67B;
const HOTAS4_LEGACY_PID: u16 = 0xB67A;
const HOTAS_ONE_PID: u16 = 0xB68B;
const READ_TIMEOUT_MS: i32 = 50;
const MAX_REPORT_BYTES: usize = 64;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!(
            "{}",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/tflight_dump.rs"
            ))
            .lines()
            .take(30)
            .filter(|l| l.starts_with("//!"))
            .map(|l| l.trim_start_matches("//! ").trim_start_matches("//!"))
            .collect::<Vec<_>>()
            .join("\n")
        );
        return Ok(());
    }

    let strip_report_id = args.iter().any(|a| a == "--strip-report-id");
    let invert_throttle = args.iter().any(|a| a == "--invert-throttle");
    let yaw_policy = args
        .iter()
        .find(|a| a.starts_with("--yaw="))
        .map(|a| a.trim_start_matches("--yaw="))
        .unwrap_or("auto");
    let yaw_policy = match yaw_policy {
        "twist" => TFlightYawPolicy::Twist,
        "aux" => TFlightYawPolicy::Aux,
        _ => TFlightYawPolicy::Auto,
    };
    let run_for: Option<Duration> = args
        .iter()
        .find(|a| a.starts_with("--duration="))
        .and_then(|a| a.trim_start_matches("--duration=").parse::<u64>().ok())
        .map(Duration::from_secs);

    eprintln!(
        "tflight_dump: strip_report_id={strip_report_id} invert_throttle={invert_throttle} yaw_policy={yaw_policy:?}"
    );

    let api = HidApi::new()?;

    let candidates: Vec<_> = api
        .device_list()
        .filter(|d| {
            d.vendor_id() == THRUSTMASTER_VID
                && (d.product_id() == HOTAS4_PID
                    || d.product_id() == HOTAS4_LEGACY_PID
                    || d.product_id() == HOTAS_ONE_PID)
        })
        .collect();

    if candidates.is_empty() {
        eprintln!(
            "No T.Flight HOTAS devices found (VID=0x{THRUSTMASTER_VID:04X} PID=0x{HOTAS4_PID:04X}/0x{HOTAS4_LEGACY_PID:04X}/0x{HOTAS_ONE_PID:04X})."
        );
        eprintln!(
            "Tip: on Windows ensure the Thrustmaster driver is installed; on Linux try `sudo` or set up udev rules."
        );
        return Ok(());
    }

    eprintln!(
        "Found {} device(s). Reading — press Ctrl-C to stop.",
        candidates.len()
    );

    // Open the first candidate (add a loop over candidates for multi-device setups).
    let dev_info = &candidates[0];
    eprintln!(
        "  VID={:04X} PID={:04X} path={} product={:?}",
        dev_info.vendor_id(),
        dev_info.product_id(),
        dev_info.path().to_string_lossy(),
        dev_info.product_string().unwrap_or("<unknown>"),
    );

    let device = dev_info.open_device(&api)?;
    let model = match dev_info.product_id() {
        HOTAS_ONE_PID => TFlightModel::HotasOne,
        HOTAS4_PID | HOTAS4_LEGACY_PID => TFlightModel::Hotas4,
        _ => TFlightModel::Hotas4,
    };
    eprintln!("  detected_model={model:?}");

    let mut handler = TFlightInputHandler::with_axis_mode(model, AxisMode::Unknown)
        .with_yaw_policy(yaw_policy)
        .with_throttle_inversion(invert_throttle)
        .with_report_id(strip_report_id);

    let start = Instant::now();
    let mut buf = [0u8; MAX_REPORT_BYTES];
    let mut report_count: u64 = 0;

    // Print CSV-style header to stdout.
    println!("epoch_ms,len,raw_hex,mode,roll,pitch,throttle,twist,rocker,hat,buttons,yaw,yaw_src");

    loop {
        if let Some(max) = run_for
            && start.elapsed() >= max
        {
            eprintln!("Duration limit reached after {report_count} reports.");
            break;
        }

        let n = match device.read_timeout(&mut buf, READ_TIMEOUT_MS) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("read error: {e}");
                break;
            }
        };

        if n == 0 {
            continue; // timeout — no data
        }

        let report = &buf[..n];
        let epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let hex: String = report.iter().map(|b| format!("{b:02x}")).collect();

        match handler.try_parse_report(report) {
            Ok(state) => {
                let yaw = handler.resolve_yaw(&state);
                let rocker = state
                    .axes
                    .rocker
                    .map(|v| format!("{v:.4}"))
                    .unwrap_or_else(|| "n/a".to_string());
                println!(
                    "{epoch_ms},{n},{hex},{mode:?},{roll:.4},{pitch:.4},{throttle:.4},{twist:.4},{rocker},{hat},{buttons:#05x},{yaw:.4},{yaw_src:?}",
                    mode = state.axis_mode,
                    roll = state.axes.roll,
                    pitch = state.axes.pitch,
                    throttle = state.axes.throttle,
                    twist = state.axes.twist,
                    hat = state.buttons.hat,
                    buttons = state.buttons.buttons,
                    yaw = yaw.value,
                    yaw_src = yaw.source,
                );
                report_count += 1;
            }
            Err(e) => {
                eprintln!("parse error (len={n} raw={hex}): {e}");
            }
        }
    }

    eprintln!("Total reports captured: {report_count}");
    Ok(())
}
