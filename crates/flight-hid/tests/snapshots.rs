// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for device quirks database and device fingerprint format.
//!
//! Run `cargo insta review` to accept new or changed snapshots.

use flight_hid::quirks::QuirksDatabase;
use flight_hid::stable_id::{DeviceFingerprint, DeviceRegistry};

// ── Device quirks database snapshots ─────────────────────────────────────────

#[test]
fn snapshot_quirks_database_defaults_summary() {
    let db = QuirksDatabase::with_defaults();
    let devices: Vec<(u16, u16)> = vec![
        (0x044F, 0xB10A), // Thrustmaster T16000M
        (0x044F, 0xB687), // Thrustmaster TWCS Throttle
        (0x068E, 0x00F4), // CH Products Pro Throttle
        (0x06A3, 0x0762), // Saitek X52 Pro
        (0x231D, 0x0200), // VKB Gladiator NXT
        (0x046D, 0xC215), // Logitech Extreme 3D Pro
    ];
    let mut summary = String::new();
    for (vid, pid) in &devices {
        let quirks = db.get_quirks(*vid, *pid);
        summary.push_str(&format!(
            "[{:04X}:{:04X}] {} quirk(s)\n",
            vid,
            pid,
            quirks.len()
        ));
        for q in &quirks {
            summary.push_str(&format!("  {:?}\n", q));
        }
    }
    insta::assert_snapshot!("quirks_database_defaults_summary", summary);
}

#[test]
fn snapshot_quirks_t16000m() {
    let db = QuirksDatabase::with_defaults();
    let quirks = db.get_quirks(0x044F, 0xB10A);
    insta::assert_debug_snapshot!("quirks_t16000m", quirks);
}

#[test]
fn snapshot_quirks_twcs_throttle() {
    let db = QuirksDatabase::with_defaults();
    let quirks = db.get_quirks(0x044F, 0xB687);
    insta::assert_debug_snapshot!("quirks_twcs_throttle", quirks);
}

// ── Device fingerprint format snapshots ──────────────────────────────────────

#[test]
fn snapshot_device_fingerprint_json() {
    let fp = DeviceFingerprint {
        vid: 0x044F,
        pid: 0x0402,
        serial: Some("WH001".into()),
        manufacturer: Some("Thrustmaster".into()),
        product: Some("HOTAS Warthog Joystick".into()),
        interface_number: Some(0),
        usage_page: 0x01,
        usage: 0x04,
        usb_path: Some("1-2.3".into()),
    };
    insta::assert_json_snapshot!("device_fingerprint_warthog", fp);
}

#[test]
fn snapshot_device_registry_json() {
    let mut reg = DeviceRegistry::new();
    reg.register(DeviceFingerprint {
        vid: 0x044F,
        pid: 0x0402,
        serial: Some("WH001".into()),
        manufacturer: Some("Thrustmaster".into()),
        product: Some("HOTAS Warthog Joystick".into()),
        interface_number: Some(0),
        usage_page: 0x01,
        usage: 0x04,
        usb_path: Some("1-2.3".into()),
    });
    reg.register(DeviceFingerprint {
        vid: 0x231D,
        pid: 0x0136,
        serial: Some("VKB001".into()),
        manufacturer: Some("VKB".into()),
        product: Some("Gladiator NXT EVO".into()),
        interface_number: None,
        usage_page: 0x01,
        usage: 0x04,
        usb_path: Some("1-4.1".into()),
    });
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("device_registry_two_devices", reg);
    });
}
