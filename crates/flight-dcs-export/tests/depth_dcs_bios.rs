// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS-BIOS cockpit builder integration — depth tests
//!
//! Covers:
//! 1. Protocol parsing (frame headers, address extraction, data types, corruption)
//! 2. Aircraft modules (A-10C CDU, F/A-18C UFC, F-16C DED, Ka-50, AH-64D, loading)
//! 3. Input commands (button, rotary, switch, axis, queue, debounce)
//! 4. Display sync (LED mapping, string display, refresh, staleness, priority)
//! 5. Property tests (round-trip, address uniqueness, mask/shift consistency)
//!
//! DCS-BIOS protocol reference (DCS-Skunkworks/dcs-bios):
//! - Binary export: sync `0x55 0x55 0x55 0x55`, then `<addr_u16_le> <len_u16_le> <data>`
//! - Import: plain text `<control_id> <argument>\n` over UDP port 7778
//! - Integer controls: 16-bit word address + mask + shift
//! - String controls: start address + fixed max length

use flight_dcs_export::control_injection::{
    a10c, ah64d, f14b, f16c, fa18c, lookup_aircraft_axis, parse_wire_command, parse_wire_payload,
    AircraftAxisMapping, Clickable, DcsActionType, DcsControlCommand, DcsControlInjector,
    WireParseError, A10C_AXES, F16C_AXES, FA18C_AXES,
};
use flight_dcs_export::protocol::{
    parse_device_arg_block, parse_instrument_block, parse_telemetry_batch,
};
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};

// ============================================================================
// Helper: DCS-BIOS binary frame builder
// ============================================================================

/// DCS-BIOS sync header (4 bytes).
const SYNC: [u8; 4] = [0x55, 0x55, 0x55, 0x55];

/// Build a DCS-BIOS export frame: sync + one or more write-access segments.
/// Each segment is `(address_u16_le, data_bytes)`.
fn build_bios_frame(segments: &[(u16, &[u8])]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&SYNC);
    for &(addr, data) in segments {
        let len = data.len() as u16;
        buf.extend_from_slice(&addr.to_le_bytes());
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(data);
    }
    buf
}

/// Extract a 16-bit unsigned integer value from a DCS-BIOS state buffer
/// given address, mask, and shift — mirrors the C reference implementation.
fn extract_integer(state: &[u8], address: u16, mask: u16, shift: u16) -> u16 {
    if (address as usize + 1) >= state.len() {
        return 0;
    }
    let word = u16::from_le_bytes([state[address as usize], state[address as usize + 1]]);
    (word & mask) >> shift
}

/// Extract a string value from a DCS-BIOS state buffer.
fn extract_string(state: &[u8], address: u16, max_len: u16) -> String {
    let start = address as usize;
    let end = (start + max_len as usize).min(state.len());
    if start >= state.len() {
        return String::new();
    }
    let bytes = &state[start..end];
    // Strings are NUL-padded
    let nul_pos = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..nul_pos]).to_string()
}

/// Apply a DCS-BIOS write-access to a state buffer.
fn apply_write(state: &mut Vec<u8>, address: u16, data: &[u8]) {
    let start = address as usize;
    let end = start + data.len();
    if end > state.len() {
        state.resize(end, 0);
    }
    state[start..end].copy_from_slice(data);
}

/// Parse a raw DCS-BIOS frame, applying writes to the state buffer.
/// Returns `true` on valid sync, `false` on invalid/corrupt data.
fn parse_bios_frame(raw: &[u8], state: &mut Vec<u8>) -> bool {
    if raw.len() < 4 || raw[..4] != SYNC {
        return false;
    }
    let mut pos = 4;
    while pos + 4 <= raw.len() {
        let addr = u16::from_le_bytes([raw[pos], raw[pos + 1]]);
        let len = u16::from_le_bytes([raw[pos + 2], raw[pos + 3]]) as usize;
        pos += 4;
        if pos + len > raw.len() {
            return false; // truncated
        }
        apply_write(state, addr, &raw[pos..pos + len]);
        pos += len;
    }
    true
}

// ============================================================================
// 1. Protocol parser tests (8)
// ============================================================================

/// Frame parsing with correct 0x55 0x55 0x55 0x55 header.
#[test]
fn bios_frame_with_correct_sync_header() {
    let frame = build_bios_frame(&[(0x0000, &[0x41, 0x00])]);
    let mut state = vec![0u8; 256];
    assert!(parse_bios_frame(&frame, &mut state));
    assert_eq!(state[0], 0x41);
    assert_eq!(state[1], 0x00);
}

/// Multi-frame assembly: two consecutive frames updating different addresses.
#[test]
fn bios_multi_frame_assembly() {
    let mut state = vec![0u8; 256];

    // Frame 1: write 0x1234 at address 0x0010
    let frame1 = build_bios_frame(&[(0x0010, &[0x34, 0x12])]);
    assert!(parse_bios_frame(&frame1, &mut state));

    // Frame 2: write 0xABCD at address 0x0020
    let frame2 = build_bios_frame(&[(0x0020, &[0xCD, 0xAB])]);
    assert!(parse_bios_frame(&frame2, &mut state));

    // Both writes should be in state
    assert_eq!(
        u16::from_le_bytes([state[0x10], state[0x11]]),
        0x1234
    );
    assert_eq!(
        u16::from_le_bytes([state[0x20], state[0x21]]),
        0xABCD
    );
}

/// Address extraction and masking per DCS-BIOS spec.
#[test]
fn bios_address_extraction_and_masking() {
    let mut state = vec![0u8; 256];

    // Write 0xFF0F at address 0x0004
    apply_write(&mut state, 0x0004, &[0x0F, 0xFF]);

    // Extract with mask 0x00FF, shift 0 → lower byte = 0x0F
    assert_eq!(extract_integer(&state, 0x0004, 0x00FF, 0), 0x0F);

    // Extract with mask 0xFF00, shift 8 → upper byte = 0xFF
    assert_eq!(extract_integer(&state, 0x0004, 0xFF00, 8), 0xFF);

    // Single-bit extraction: bit 3 of lower byte (mask 0x0008, shift 3)
    assert_eq!(extract_integer(&state, 0x0004, 0x0008, 3), 1);

    // Bit 4 of lower byte (mask 0x0010, shift 4) = 0
    assert_eq!(extract_integer(&state, 0x0004, 0x0010, 4), 0);
}

/// Integer and string data types from DCS-BIOS state buffer.
#[test]
fn bios_integer_and_string_data_types() {
    let mut state = vec![0u8; 256];

    // Integer: write a gear-down indicator (value 1) at address 0x0040
    apply_write(&mut state, 0x0040, &[0x01, 0x00]);
    assert_eq!(extract_integer(&state, 0x0040, 0xFFFF, 0), 1);

    // String: write "A-10C" at address 0x0080, max_len 16
    let name = b"A-10C\0\0\0\0\0\0\0\0\0\0\0";
    apply_write(&mut state, 0x0080, &name[..16]);
    assert_eq!(extract_string(&state, 0x0080, 16), "A-10C");
}

/// Corrupted frame: wrong sync header is rejected.
#[test]
fn bios_corrupted_frame_handling() {
    let mut state = vec![0u8; 64];

    // Wrong sync
    let bad = vec![0xAA, 0xBB, 0xCC, 0xDD, 0x00, 0x00, 0x02, 0x00, 0xFF, 0xFF];
    assert!(!parse_bios_frame(&bad, &mut state));

    // Partial sync (only 3 bytes)
    let partial = vec![0x55, 0x55, 0x55];
    assert!(!parse_bios_frame(&partial, &mut state));

    // Empty input
    assert!(!parse_bios_frame(&[], &mut state));
}

/// Truncated frame: declared data length larger than available bytes is rejected.
#[test]
fn bios_truncated_frame_rejection() {
    let mut state = vec![0u8; 64];

    // Frame claims 100 bytes of data but only has 2
    let mut bad_frame = Vec::new();
    bad_frame.extend_from_slice(&SYNC);
    bad_frame.extend_from_slice(&0x0000u16.to_le_bytes()); // addr
    bad_frame.extend_from_slice(&0x0064u16.to_le_bytes()); // len=100
    bad_frame.extend_from_slice(&[0xFF, 0xFF]); // only 2 bytes

    assert!(!parse_bios_frame(&bad_frame, &mut state));
    // State should not be corrupted
    assert!(state.iter().all(|&b| b == 0));
}

/// Frame reconstruction: concatenating chunks reconstructs the full frame.
#[test]
fn bios_frame_reconstructs_after_concatenation() {
    // Simulate receiving a frame in two chunks
    let full_frame = build_bios_frame(&[(0x0008, &[0xBE, 0xEF])]);
    let split = full_frame.len() / 2;

    let chunk1 = &full_frame[..split];
    let chunk2 = &full_frame[split..];

    // Neither chunk alone forms a valid frame
    let mut state = vec![0u8; 64];
    // Chunk 1 alone: may or may not parse depending on split point
    let mut accumulated = chunk1.to_vec();
    // After appending chunk 2, the full frame should parse
    accumulated.extend_from_slice(chunk2);
    assert!(parse_bios_frame(&accumulated, &mut state));
    assert_eq!(state[0x08], 0xBE);
    assert_eq!(state[0x09], 0xEF);
}

/// Multiple write-accesses in a single frame.
#[test]
fn bios_multiple_writes_single_frame() {
    let frame = build_bios_frame(&[
        (0x0000, &[0x01, 0x00]),
        (0x0010, &[0x02, 0x00]),
        (0x0020, &[0x03, 0x00]),
    ]);
    let mut state = vec![0u8; 256];
    assert!(parse_bios_frame(&frame, &mut state));

    assert_eq!(extract_integer(&state, 0x0000, 0xFFFF, 0), 1);
    assert_eq!(extract_integer(&state, 0x0010, 0xFFFF, 0), 2);
    assert_eq!(extract_integer(&state, 0x0020, 0xFFFF, 0), 3);
}

// ============================================================================
// 2. Aircraft module tests (10)
// ============================================================================

/// A-10C CDU display data: command lookup and device ID verification.
#[test]
fn module_a10c_cdu_display_data() {
    assert_eq!(a10c::CDU.id, 24);
    assert_eq!(a10c::CDU.name, "CDU");

    // CDU numeric buttons 1–9 should map to device 24, command IDs 3001–3009
    for i in 1..=9u32 {
        let name = format!("CDU_{i}");
        let cmd = a10c::lookup_command(&name)
            .unwrap_or_else(|| panic!("CDU_{i} should exist"));
        assert_eq!(cmd.device_id, 24, "CDU_{i} device");
        assert_eq!(cmd.command_id, 3000 + i, "CDU_{i} command_id");
    }
    // CDU_0 uses command_id 3010
    let cmd0 = a10c::lookup_command("CDU_0").expect("CDU_0 should exist");
    assert_eq!(cmd0.device_id, 24);
    assert_eq!(cmd0.command_id, 3010);
}

/// F/A-18C UFC (Up Front Controller) commands and display integration.
#[test]
fn module_fa18c_ufc_commands() {
    assert_eq!(fa18c::UFC.id, 25);

    // UFC numeric pad: UFC_1 through UFC_0
    let ufc_names = [
        "UFC_1", "UFC_2", "UFC_3", "UFC_4", "UFC_5", "UFC_6", "UFC_7", "UFC_8", "UFC_9",
        "UFC_0",
    ];
    let mut ids: HashSet<u32> = HashSet::new();
    for name in &ufc_names {
        let cmd = fa18c::lookup_command(name).expect(name);
        assert_eq!(cmd.device_id, 25);
        ids.insert(cmd.command_id);
    }
    // All command IDs should be unique
    assert_eq!(ids.len(), ufc_names.len());

    // ENT and CLR
    let ent = fa18c::lookup_command("UFC_ENT").unwrap();
    let clr = fa18c::lookup_command("UFC_CLR").unwrap();
    assert_ne!(ent.command_id, clr.command_id);
}

/// F-16C DED (Data Entry Display) via ICP panel.
#[test]
fn module_f16c_ded_icp_panel() {
    assert_eq!(f16c::ICP.id, 17);

    // ICP numeric pad
    for i in 0..=9 {
        let name = format!("ICP_{i}");
        let cmd = f16c::lookup_command(&name)
            .unwrap_or_else(|| panic!("ICP_{i} should exist"));
        assert_eq!(cmd.device_id, 17);
    }

    // DCS rocker (up/down)
    let up = f16c::lookup_command("ICP_DCS_UP").unwrap();
    let down = f16c::lookup_command("ICP_DCS_DOWN").unwrap();
    assert_eq!(up.device_id, 17);
    assert_eq!(down.device_id, 17);
    assert_ne!(up.command_id, down.command_id);
}

/// Ka-50 PVI-800 navigation: verify we can build correct commands.
#[test]
fn module_ka50_pvi800_navigation() {
    // Ka-50 doesn't have a dedicated module in control_injection yet,
    // so test via generic axis/command lookup and telemetry parsing.
    let batch = [
        "HEADER:timestamp=100.0,model_time=50.0,aircraft=Ka-50",
        "heading_deg=45.0",
        "altitude_m=200.0",
        "airspeed_ms=30.0",
    ]
    .join("\n");

    let pkt = parse_telemetry_batch(&batch).unwrap();
    assert_eq!(pkt.aircraft_name, "Ka-50");
    assert!((pkt.flight_data.heading_deg - 45.0).abs() < f64::EPSILON);
    assert!((pkt.flight_data.altitude_m - 200.0).abs() < f64::EPSILON);
}

/// AH-64D MPD (Multi-Purpose Display) — pilot and CPG keyboard units.
#[test]
fn module_ah64d_mpd_keyboard_units() {
    // Pilot and CPG KU are distinct devices
    assert_eq!(ah64d::PILOT_KU.id, 29);
    assert_eq!(ah64d::CPG_KU.id, 30);
    assert_ne!(ah64d::PILOT_KU.id, ah64d::CPG_KU.id);

    // ENT and CLR exist for both seats
    let plt_ent = ah64d::lookup_command("PLT_KU_ENT").unwrap();
    let cpg_ent = ah64d::lookup_command("CPG_KU_ENT").unwrap();
    assert_eq!(plt_ent.device_id, 29);
    assert_eq!(cpg_ent.device_id, 30);
    // Same command_id for equivalent function, different device
    assert_eq!(plt_ent.command_id, cpg_ent.command_id);

    let plt_clr = ah64d::lookup_command("PLT_KU_CLR").unwrap();
    let cpg_clr = ah64d::lookup_command("CPG_KU_CLR").unwrap();
    assert_ne!(plt_clr.command_id, plt_ent.command_id);
    assert_eq!(plt_clr.command_id, cpg_clr.command_id);
}

/// Module loading and registration via the TOML-based loader.
#[test]
fn module_loading_and_registration() {
    use flight_dcs_modules::ModuleLoader;
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("a-10c.toml"),
        r#"
aircraft = "A-10C"
axis_count = 7
throttle_range = [0.0, 1.0]
stick_throw = 50.0
quirks = ["twin-throttle", "gun-trigger"]
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("fa-18c.toml"),
        r#"
aircraft = "F/A-18C"
axis_count = 6
throttle_range = [0.0, 1.0]
stick_throw = 45.0
quirks = ["twin-throttle", "catapult-bar"]
"#,
    )
    .unwrap();

    let mut loader = ModuleLoader::new();
    let count = loader.load_from_dir(dir.path()).unwrap();
    assert_eq!(count, 2);

    let a10 = loader.get("A-10C").unwrap();
    assert_eq!(a10.axis_count, 7);
    assert!(a10.quirks.contains(&"gun-trigger".to_owned()));

    let fa18 = loader.get("F/A-18C").unwrap();
    assert_eq!(fa18.stick_throw, 45.0);
}

/// Missing module: graceful None return.
#[test]
fn module_missing_graceful_handling() {
    use flight_dcs_modules::ModuleLoader;

    let loader = ModuleLoader::new();
    assert!(loader.get("MiG-29").is_none());
    assert!(loader.get("").is_none());
    assert!(loader.get("nonexistent-aircraft-xyz").is_none());
}

/// Module version compatibility: reloading the same module file overwrites the previous version.
#[test]
fn module_version_compatibility() {
    use flight_dcs_modules::ModuleLoader;
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();

    // Simulate two "versions" of same aircraft with different params
    fs::write(
        dir.path().join("f-16c.toml"),
        r#"
aircraft = "F-16C"
axis_count = 5
throttle_range = [0.0, 1.0]
stick_throw = 30.0
quirks = ["side-stick", "fbw"]
"#,
    )
    .unwrap();

    let mut loader = ModuleLoader::new();
    loader.load_from_dir(dir.path()).unwrap();
    let f16 = loader.get("F-16C").unwrap();
    assert_eq!(f16.axis_count, 5);
    assert_eq!(f16.stick_throw, 30.0);

    // Re-loading with updated params overwrites
    fs::write(
        dir.path().join("f-16c.toml"),
        r#"
aircraft = "F-16C"
axis_count = 5
throttle_range = [0.0, 1.0]
stick_throw = 32.0
quirks = ["side-stick", "fbw", "hmcs"]
"#,
    )
    .unwrap();

    loader.load_from_dir(dir.path()).unwrap();
    let f16_v2 = loader.get("F-16C").unwrap();
    assert_eq!(f16_v2.stick_throw, 32.0);
    assert!(f16_v2.quirks.contains(&"hmcs".to_owned()));
}

/// F-14B Tomcat module: pilot stick and RIO CAP devices.
#[test]
fn module_f14b_tomcat_devices() {
    assert_eq!(f14b::PILOT_STICK.id, 0);
    assert_eq!(f14b::RIO_CAP.id, 42);

    let wing_auto = f14b::lookup_command("WING_SWEEP_AUTO").unwrap();
    let wing_manual = f14b::lookup_command("WING_SWEEP_MANUAL").unwrap();
    assert_eq!(wing_auto.device_id, 0);
    assert_ne!(wing_auto.command_id, wing_manual.command_id);

    let rio_launch = f14b::lookup_command("RIO_CAP_LAUNCH").unwrap();
    assert_eq!(rio_launch.device_id, 42);
}

// ============================================================================
// 3. Input command tests (6)
// ============================================================================

/// Button press/release: correct wire format and action types.
#[test]
fn input_button_press_release() {
    let press = DcsControlCommand::button_press(25, 3001);
    assert_eq!(press.action_type, DcsActionType::ButtonPress);
    assert!((press.value - 1.0).abs() < f64::EPSILON);

    let release = DcsControlCommand::button_release(25, 3001);
    assert_eq!(release.action_type, DcsActionType::ButtonRelease);
    assert!(release.value.abs() < f64::EPSILON);

    // Wire format round-trip
    let parsed_press = parse_wire_command(&press.to_wire()).unwrap();
    assert_eq!(parsed_press.device_id, 25);
    assert_eq!(parsed_press.command_id, 3001);
    assert!((parsed_press.value - 1.0).abs() < f64::EPSILON);

    let parsed_release = parse_wire_command(&release.to_wire()).unwrap();
    assert!(parsed_release.value.abs() < f64::EPSILON);
}

/// Rotary encoder increment/decrement via DCS-BIOS ICP DCS rocker.
#[test]
fn input_rotary_encoder_increment_decrement() {
    let mut inj = DcsControlInjector::new(16);

    // Simulate rotary encoder: ICP DCS UP = increment, DOWN = decrement
    let up = f16c::lookup_command("ICP_DCS_UP").unwrap();
    let down = f16c::lookup_command("ICP_DCS_DOWN").unwrap();

    // Press-release sequence for each detent
    inj.press_button(up.device_id, up.command_id);
    inj.release_button(up.device_id, up.command_id);
    inj.press_button(down.device_id, down.command_id);
    inj.release_button(down.device_id, down.command_id);

    assert_eq!(inj.pending_count(), 4);

    let payload = String::from_utf8(inj.flush()).unwrap();
    let lines: Vec<&str> = payload.lines().collect();
    assert_eq!(lines.len(), 4);

    // Verify up press, up release, down press, down release
    assert!(lines[0].contains(&format!("{}", up.command_id)));
    assert!(lines[0].contains("1.000000"));
    assert!(lines[1].contains("0.000000"));
    assert!(lines[2].contains(&format!("{}", down.command_id)));
}

/// Multi-position switch via Clickable with min/max range.
#[test]
fn input_multi_position_switch() {
    let switch = Clickable {
        label: "Master Mode Switch",
        device_id: 12,
        button: 3200,
        min_value: 0.0,
        max_value: 2.0,
    };

    // Position 0 (OFF)
    let cmd0 = switch.command(0.0);
    assert_eq!(cmd0.device_id, 12);
    assert!((cmd0.value - 0.0).abs() < f64::EPSILON);
    assert_eq!(cmd0.action_type, DcsActionType::Axis); // range > 0.01

    // Position 1 (ARM)
    let cmd1 = switch.command(1.0);
    assert!((cmd1.value - 1.0).abs() < f64::EPSILON);

    // Position 2 (SAFE)
    let cmd2 = switch.command(2.0);
    assert!((cmd2.value - 2.0).abs() < f64::EPSILON);

    // Clamping: beyond max
    let cmd_over = switch.command(5.0);
    assert!((cmd_over.value - 2.0).abs() < f64::EPSILON);

    // Press/release convenience
    let press = switch.press();
    assert!((press.value - 2.0).abs() < f64::EPSILON);
    let release = switch.release();
    assert!(release.value.abs() < f64::EPSILON);
}

/// Analog axis input with clamping and precision.
#[test]
fn input_analog_axis() {
    let mut inj = DcsControlInjector::new(16);

    // Named axis
    assert!(inj.set_axis("pitch", 0.0));
    assert!(inj.set_axis("roll", -1.0));
    assert!(inj.set_axis("throttle", 1.0));
    assert!(inj.set_axis("yaw", 0.5));

    assert_eq!(inj.pending_count(), 4);

    let payload = String::from_utf8(inj.flush()).unwrap();
    let cmds: Vec<_> = parse_wire_payload(&payload)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(cmds.len(), 4);
    assert_eq!(cmds[0].action_type, DcsActionType::Axis);

    // Verify clamping on out-of-range
    let extreme = DcsControlCommand::axis(0, 2001, 999.0);
    assert!((extreme.value - 1.0).abs() < f64::EPSILON);

    let extreme_neg = DcsControlCommand::axis(0, 2001, -999.0);
    assert!((extreme_neg.value - (-1.0)).abs() < f64::EPSILON);
}

/// Command queue management: capacity, drain, refill.
#[test]
fn input_command_queue_management() {
    let mut inj = DcsControlInjector::new(4);

    // Fill to capacity
    for i in 0..4 {
        assert!(inj.queue_command(DcsControlCommand::axis(0, i, 0.0)));
    }
    assert_eq!(inj.pending_count(), 4);

    // Over capacity
    assert!(!inj.queue_command(DcsControlCommand::axis(0, 99, 0.0)));
    assert_eq!(inj.pending_count(), 4);

    // Flush drains
    let p1 = inj.flush();
    assert!(!p1.is_empty());
    assert_eq!(inj.pending_count(), 0);

    // Refill after drain
    assert!(inj.queue_command(DcsControlCommand::button_press(1, 100)));
    assert_eq!(inj.pending_count(), 1);

    // Clear
    inj.clear();
    assert_eq!(inj.pending_count(), 0);
    assert!(inj.flush().is_empty());
}

/// Debouncing: rapid press-release pairs produce correct ordered output.
#[test]
fn input_debouncing_rapid_commands() {
    let mut inj = DcsControlInjector::new(64);

    // Simulate rapid button mashing: 10 press-release cycles
    for _ in 0..10 {
        inj.press_button(25, 3001);
        inj.release_button(25, 3001);
    }

    assert_eq!(inj.pending_count(), 20);

    let payload = String::from_utf8(inj.flush()).unwrap();
    let lines: Vec<&str> = payload.lines().collect();
    assert_eq!(lines.len(), 20);

    // Verify alternating press/release
    for (i, line) in lines.iter().enumerate() {
        if i % 2 == 0 {
            assert!(line.contains("1.000000"), "line {i} should be press");
        } else {
            assert!(line.contains("0.000000"), "line {i} should be release");
        }
    }
}

// ============================================================================
// 4. Display sync tests (6)
// ============================================================================

/// Panel LED → DCS-BIOS integer: map instrument reading to LED state.
#[test]
fn display_led_integer_mapping() {
    let block = [
        "INSTRUMENTS_BEGIN",
        "MasterCaution=1.0",
        "GearWarning=0.0",
        "EngineFireLeft=1.0",
        "EngineFireRight=0.0",
        "INSTRUMENTS_END",
    ]
    .join("\n");

    let readings = parse_instrument_block(&block).unwrap();
    assert_eq!(readings.len(), 4);

    // Map readings to LED states (on = value >= 0.5)
    let leds: HashMap<&str, bool> = readings
        .iter()
        .map(|r| (r.name.as_str(), r.value >= 0.5))
        .collect();

    assert!(leds["MasterCaution"]);
    assert!(!leds["GearWarning"]);
    assert!(leds["EngineFireLeft"]);
    assert!(!leds["EngineFireRight"]);
}

/// Panel display → DCS-BIOS string: extract display text from state buffer.
#[test]
fn display_string_from_state_buffer() {
    let mut state = vec![0u8; 512];

    // Simulate DCS-BIOS UFC scratchpad: "123.45" at address 0x0100, max 8 chars
    let text = b"123.45\0\0";
    apply_write(&mut state, 0x0100, text);
    assert_eq!(extract_string(&state, 0x0100, 8), "123.45");

    // Simulate CDU line: "STEERPT  1" at address 0x0200, max 24 chars
    let cdu_line = b"STEERPT  1\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
    apply_write(&mut state, 0x0200, &cdu_line[..24]);
    assert_eq!(extract_string(&state, 0x0200, 24), "STEERPT  1");

    // Empty string
    apply_write(&mut state, 0x0300, &[0u8; 8]);
    assert_eq!(extract_string(&state, 0x0300, 8), "");
}

/// Refresh rate management: device-arg block updates per frame.
#[test]
fn display_refresh_rate_management() {
    // Simulate 3 consecutive device-arg snapshots (30 Hz, ~33ms apart)
    let frames = [
        "ARGS_BEGIN\n0:71:0.000000\n0:85:0.000000\nARGS_END\n",
        "ARGS_BEGIN\n0:71:0.250000\n0:85:0.500000\nARGS_END\n",
        "ARGS_BEGIN\n0:71:0.500000\n0:85:1.000000\nARGS_END\n",
    ];

    let mut prev_values: HashMap<u32, f64> = HashMap::new();
    let mut update_count = 0u32;

    for frame_data in &frames {
        let entries = parse_device_arg_block(frame_data).unwrap();
        for entry in &entries {
            let changed = prev_values
                .get(&entry.arg_number)
                .map_or(true, |&prev| (prev - entry.value).abs() > 1e-6);
            if changed {
                update_count += 1;
                prev_values.insert(entry.arg_number, entry.value);
            }
        }
    }

    // First frame: both args are new (2), second frame: both changed (2),
    // third frame: both changed (2) = 6 total updates
    assert_eq!(update_count, 6);
}

/// Stale data detection: timestamps allow detecting stale cockpit data.
#[test]
fn display_stale_data_detection() {
    // Parse two telemetry batches with timestamps
    let batch1 = "HEADER:timestamp=100.0,model_time=50.0,aircraft=F-16C\naltitude_m=5000.0";
    let batch2 = "HEADER:timestamp=100.033,model_time=50.033,aircraft=F-16C\naltitude_m=5001.0";

    let pkt1 = parse_telemetry_batch(batch1).unwrap();
    let pkt2 = parse_telemetry_batch(batch2).unwrap();

    let dt = pkt2.timestamp - pkt1.timestamp;
    assert!(dt > 0.0, "timestamps must be monotonically increasing");
    assert!((dt - 0.033).abs() < 0.001, "~33ms between frames");

    // Detect staleness: if dt > threshold (e.g. 200ms), data is stale
    let stale_threshold = 0.200;
    assert!(dt < stale_threshold, "data should NOT be stale");

    // Simulate stale data
    let batch3 =
        "HEADER:timestamp=101.0,model_time=51.0,aircraft=F-16C\naltitude_m=5010.0";
    let pkt3 = parse_telemetry_batch(batch3).unwrap();
    let dt_stale = pkt3.timestamp - pkt2.timestamp;
    assert!(
        dt_stale > stale_threshold,
        "gap >200ms indicates stale data"
    );
}

/// Display priority: active data overrides stale data.
#[test]
fn display_priority_active_over_stale() {
    let mut state = vec![0u8; 256];

    // Initial stale value
    apply_write(&mut state, 0x0040, &[0x00, 0x00]);
    let stale_val = extract_integer(&state, 0x0040, 0xFFFF, 0);
    assert_eq!(stale_val, 0);

    // Active update arrives
    let frame = build_bios_frame(&[(0x0040, &[0x01, 0x00])]);
    assert!(parse_bios_frame(&frame, &mut state));

    let active_val = extract_integer(&state, 0x0040, 0xFFFF, 0);
    assert_eq!(active_val, 1, "active value should override stale");

    // Second active update
    let frame2 = build_bios_frame(&[(0x0040, &[0x05, 0x00])]);
    assert!(parse_bios_frame(&frame2, &mut state));
    assert_eq!(extract_integer(&state, 0x0040, 0xFFFF, 0), 5);
}

/// Display string partial update: DCS-BIOS can update strings partially.
#[test]
fn display_string_partial_update() {
    let mut state = vec![0u8; 256];

    // Full initial string "HELLO   " at 0x0050
    apply_write(&mut state, 0x0050, b"HELLO\0\0\0");
    assert_eq!(extract_string(&state, 0x0050, 8), "HELLO");

    // Partial update: overwrite bytes 0–1 with "HI"
    apply_write(&mut state, 0x0050, b"HI");
    // Result: "HILLO" (first 2 bytes replaced, rest unchanged)
    assert_eq!(extract_string(&state, 0x0050, 8), "HILLO");
}

// ============================================================================
// 5. Property tests (5)
// ============================================================================

proptest! {
    /// Protocol round-trip: any valid write can be read back correctly.
    #[test]
    fn prop_protocol_round_trip(
        addr in (0u16..=0xFE).prop_map(|a| a & 0xFFFE),  // even addresses
        value in any::<u16>(),
    ) {
        let data = value.to_le_bytes();
        let frame = build_bios_frame(&[(addr, &data)]);
        let mut state = vec![0u8; 256];
        prop_assert!(parse_bios_frame(&frame, &mut state));
        prop_assert_eq!(extract_integer(&state, addr, 0xFFFF, 0), value);
    }

    /// Address uniqueness per module: no duplicate command IDs within a module.
    #[test]
    fn prop_fa18c_address_uniqueness(_dummy in 0..1u8) {
        let mut seen: HashSet<(u32, u32)> = HashSet::new();
        for cmd in fa18c::COMMANDS {
            let key = (cmd.device_id, cmd.command_id);
            prop_assert!(
                seen.insert(key),
                "duplicate (device_id={}, command_id={}) in F/A-18C",
                cmd.device_id,
                cmd.command_id
            );
        }
    }

    /// Mask/shift consistency: extracted value always fits in mask.
    #[test]
    fn prop_mask_shift_consistency(
        raw_value in any::<u16>(),
        mask in prop_oneof![
            Just(0x0001u16), Just(0x0003u16), Just(0x000Fu16),
            Just(0x00FFu16), Just(0xFF00u16), Just(0xFFFFu16),
            Just(0x0100u16), Just(0x8000u16),
        ],
    ) {
        let shift = mask.trailing_zeros() as u16;
        let max_val = mask >> shift;

        let mut state = vec![0u8; 4];
        state[0..2].copy_from_slice(&raw_value.to_le_bytes());

        let extracted = extract_integer(&state, 0, mask, shift);
        prop_assert!(
            extracted <= max_val,
            "extracted {} > max {} for mask 0x{:04X}",
            extracted,
            max_val,
            mask
        );
    }

    /// Wire command round-trip: serialized commands parse back correctly.
    #[test]
    fn prop_wire_command_round_trip(
        device_id in 0u32..100,
        command_id in 1000u32..5000,
        value in -1.0f64..=1.0f64,
    ) {
        let cmd = DcsControlCommand::axis(device_id, command_id, value);
        let wire = cmd.to_wire();
        let parsed = parse_wire_command(&wire).unwrap();
        prop_assert_eq!(parsed.device_id, device_id);
        prop_assert_eq!(parsed.command_id, command_id);
        prop_assert!((parsed.value - cmd.value).abs() < 1e-5);
    }

    /// Per-aircraft axis mappings have no duplicate bus_axis names.
    #[test]
    fn prop_axis_mappings_unique(_dummy in 0..1u8) {
        let tables: &[(&str, &[AircraftAxisMapping])] = &[
            ("FA-18C", FA18C_AXES),
            ("F-16C", F16C_AXES),
            ("A-10C", A10C_AXES),
        ];
        for (name, table) in tables {
            let mut seen = HashSet::<&str>::new();
            for mapping in *table {
                prop_assert!(
                    seen.insert(mapping.bus_axis),
                    "duplicate bus_axis '{}' in {} axis table",
                    mapping.bus_axis,
                    name
                );
            }
        }
    }
}

// ============================================================================
// Additional integration tests
// ============================================================================

/// Per-aircraft axis lookup resolves correctly for known modules.
#[test]
fn aircraft_axis_lookup_all_modules() {
    // F/A-18C twin throttle
    let tl = lookup_aircraft_axis("FA-18C", "throttle_left").unwrap();
    assert_eq!(tl.device_id, 0);
    assert_eq!(tl.command_id, 2005);

    let tr = lookup_aircraft_axis("FA-18C", "throttle_right").unwrap();
    assert_eq!(tr.command_id, 2006);

    // F-16C single throttle
    let t = lookup_aircraft_axis("F-16C", "throttle").unwrap();
    assert_eq!(t.command_id, 2004);

    // A-10C twin throttle
    let atl = lookup_aircraft_axis("A-10C", "throttle_left").unwrap();
    assert_eq!(atl.command_id, 2005);

    // Unknown module
    assert!(lookup_aircraft_axis("Su-27", "pitch").is_none());

    // Unknown axis on known module
    assert!(lookup_aircraft_axis("F-16C", "nonexistent").is_none());
}

/// Wire payload parsing: multi-line payload round-trip.
#[test]
fn wire_payload_multiline_parsing() {
    let mut inj = DcsControlInjector::new(16);
    inj.queue_command(DcsControlCommand::axis(0, 2001, 0.5));
    inj.queue_command(DcsControlCommand::button_press(25, 3001));
    inj.queue_command(DcsControlCommand::toggle(2, 500));
    inj.queue_command(DcsControlCommand::button_release(25, 3001));

    let payload = String::from_utf8(inj.flush()).unwrap();
    let results = parse_wire_payload(&payload);
    assert_eq!(results.len(), 4);
    assert!(results.iter().all(|r| r.is_ok()));

    let cmds: Vec<_> = results.into_iter().map(|r| r.unwrap()).collect();
    assert_eq!(cmds[0].action_type, DcsActionType::Axis);
    assert_eq!(cmds[1].action_type, DcsActionType::ButtonPress);
    assert_eq!(cmds[2].action_type, DcsActionType::Toggle);
    assert_eq!(cmds[3].action_type, DcsActionType::ButtonRelease);
}

/// Wire parse error cases: malformed input produces correct errors.
#[test]
fn wire_parse_error_cases() {
    // Missing prefix
    assert!(matches!(
        parse_wire_command("no_colon"),
        Err(WireParseError::UnknownPrefix(_))
    ));

    // Unknown prefix
    assert!(matches!(
        parse_wire_command("XYZ:1,2,3.0"),
        Err(WireParseError::UnknownPrefix(_))
    ));

    // Wrong field count
    assert!(matches!(
        parse_wire_command("CMD:1,2"),
        Err(WireParseError::BadFieldCount { expected: 3, .. })
    ));

    // Invalid number
    assert!(matches!(
        parse_wire_command("CMD:abc,2,3.0"),
        Err(WireParseError::InvalidNumber(_))
    ));
}
