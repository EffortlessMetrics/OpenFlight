// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! # CH Products Report Dump Example
//!
//! Demonstrates parsing synthetic HID reports for every supported
//! CH Products device. In a real application the raw bytes would come
//! from the OS HID layer (e.g. `hidapi`).
//!
//! ```sh
//! cargo run -p flight-hotas-ch --example ch_dump
//! ```

use flight_hotas_ch::{
    identify_device, normalize_axis, normalize_pedal, normalize_throttle, parse_combatstick,
    parse_eclipse_yoke, parse_fighterstick, parse_flight_yoke, parse_pro_pedals,
    parse_pro_throttle, CH_VENDOR_ID,
};

fn main() {
    println!("=== CH Products HID Report Parser Demo ===\n");

    // --- Fighterstick (PID 0x00F3) ---
    let pid = 0x00F3u16;
    let device = identify_device(CH_VENDOR_ID, pid);
    println!("Device: {:?} (VID={CH_VENDOR_ID:#06x} PID={pid:#06x})", device);

    let report = [0x01, 0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x10];
    let state = parse_fighterstick(&report).unwrap();
    println!("  {state}");
    println!(
        "  Normalized: X={:.3} Y={:.3} Z={:.3}",
        normalize_axis(state.x),
        normalize_axis(state.y),
        normalize_axis(state.z),
    );

    // --- Pro Throttle (PID 0x00F1) ---
    let report = [0x01, 0xFF, 0xFF, 0x00, 0x80, 0x00, 0x80, 0x05, 0x30];
    let state = parse_pro_throttle(&report).unwrap();
    println!("\n  {state}");
    println!("  Throttle normalized: {:.3}", normalize_throttle(state.throttle_main));

    // --- Pro Pedals (PID 0x00F2) ---
    let report = [0x01, 0x00, 0x80, 0xFF, 0xFF, 0x00, 0x00];
    let state = parse_pro_pedals(&report).unwrap();
    println!("\n  {state}");
    println!(
        "  Rudder={:.3} Left={:.3} Right={:.3}",
        normalize_pedal(state.rudder),
        normalize_pedal(state.left_toe),
        normalize_pedal(state.right_toe),
    );

    // --- Combat Stick (PID 0x00F4) ---
    let report = [0x01, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0x00, 0x50];
    let state = parse_combatstick(&report).unwrap();
    println!("\n  {state}");

    // --- Eclipse Yoke (PID 0x0051) ---
    let report = [0x01, 0x00, 0x80, 0x00, 0x80, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00];
    let state = parse_eclipse_yoke(&report).unwrap();
    println!("\n  {state}");

    // --- Flight Sim Yoke (PID 0x00FF) ---
    let report = [0x01, 0x00, 0x80, 0x00, 0x80, 0x00, 0x40, 0xFF, 0xFF, 0x0F];
    let state = parse_flight_yoke(&report).unwrap();
    println!("\n  {state}");

    println!("\nDone — all parsers executed successfully.");
}
