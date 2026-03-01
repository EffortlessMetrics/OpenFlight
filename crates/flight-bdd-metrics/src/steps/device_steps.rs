// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Device/HID step definitions.
//!
//! Connects Gherkin steps to `flight_hid_types` for mock device creation
//! and HID report parsing simulation.

use crate::step_registry::{StepOutcome, StepRegistry};
use flight_hid_types::HidDeviceInfo;

/// Parsed result from a simulated HID report.
#[derive(Debug, Clone)]
pub struct ParsedReport {
    pub axes: Vec<(String, f32)>,
    pub buttons: Vec<(u32, bool)>,
}

/// Build a mock [`HidDeviceInfo`] for the given vendor/model.
fn mock_device(vendor: &str, model: &str) -> HidDeviceInfo {
    let (vid, pid) = match (vendor, model) {
        ("Thrustmaster", "T.Flight HOTAS 4") => (0x044F, 0xB679),
        ("VKB", "Gladiator NXT EVO") => (0x231D, 0x0200),
        ("Logitech", "X56 Rhino") => (0x046D, 0x0A38),
        _ => (0xFFFF, 0xFFFF),
    };
    HidDeviceInfo {
        vendor_id: vid,
        product_id: pid,
        serial_number: Some("MOCK-001".to_string()),
        manufacturer: Some(vendor.to_string()),
        product_name: Some(model.to_string()),
        device_path: format!("\\\\?\\HID#VID_{vid:04X}&PID_{pid:04X}"),
        usage_page: 0x01,
        usage: 0x04,
        report_descriptor: None,
    }
}

/// Simulate parsing a HID report hex string into axis/button data.
///
/// Format: each byte pair encodes a value. The first 6 bytes are treated as
/// 3 little-endian 16-bit axis values (normalized to [-1, 1]). Remaining
/// bytes are button bitmasks.
fn parse_hid_report(hex: &str) -> Result<ParsedReport, String> {
    let bytes: Result<Vec<u8>, _> = hex
        .split_whitespace()
        .map(|b| u8::from_str_radix(b, 16))
        .collect();
    let bytes = bytes.map_err(|e| format!("bad hex: {e}"))?;

    let mut axes = Vec::new();
    let axis_names = ["X", "Y", "Z", "Rx", "Ry", "Rz"];
    let axis_count = (bytes.len().min(12)) / 2;
    for i in 0..axis_count {
        let lo = bytes[i * 2] as u16;
        let hi = bytes[i * 2 + 1] as u16;
        let raw = lo | (hi << 8);
        let norm = (raw as f32 / 65535.0) * 2.0 - 1.0;
        let name = if i < axis_names.len() {
            axis_names[i].to_string()
        } else {
            format!("axis_{i}")
        };
        axes.push((name, norm));
    }

    let mut buttons = Vec::new();
    let button_start = axis_count * 2;
    for (byte_idx, &b) in bytes[button_start..].iter().enumerate() {
        for bit in 0..8u32 {
            let btn_id = (byte_idx as u32) * 8 + bit + 1;
            buttons.push((btn_id, (b >> bit) & 1 == 1));
        }
    }

    Ok(ParsedReport { axes, buttons })
}

/// Register all device-related step definitions.
pub fn register(registry: &mut StepRegistry) {
    // -- Given ----------------------------------------------------------

    registry.given(
        r#"^a "([^"]+)" "([^"]+)" is connected$"#,
        |ctx, caps| {
            let vendor = &caps[1];
            let model = &caps[2];
            let device = mock_device(vendor, model);
            ctx.set("device_vendor_id", device.vendor_id);
            ctx.set("device_product_id", device.product_id);
            ctx.set("device_info", device);
            StepOutcome::Passed
        },
    );

    // -- When -----------------------------------------------------------

    registry.when(
        r"^the device sends HID report ([0-9A-Fa-f ]+)$",
        |ctx, caps| {
            let hex = &caps[1];
            match parse_hid_report(hex) {
                Ok(report) => {
                    ctx.set("parsed_report", report);
                    StepOutcome::Passed
                }
                Err(e) => StepOutcome::Failed(e),
            }
        },
    );

    // -- Then -----------------------------------------------------------

    registry.then(
        r#"^axis "([^"]+)" should read (-?\d+\.?\d*) ±(\d+\.?\d*)$"#,
        |ctx, caps| {
            let axis_name = &caps[1];
            let expected: f32 = match caps[2].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad float: {e}")),
            };
            let tolerance: f32 = match caps[3].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad tolerance: {e}")),
            };
            let report = match ctx.get::<ParsedReport>("parsed_report") {
                Some(r) => r,
                None => return StepOutcome::Failed("no parsed_report in context".to_string()),
            };
            match report.axes.iter().find(|(n, _)| n == axis_name) {
                Some((_, actual)) => {
                    if (actual - expected).abs() <= tolerance {
                        StepOutcome::Passed
                    } else {
                        StepOutcome::Failed(format!(
                            "axis '{axis_name}': expected {expected} ±{tolerance}, got {actual}"
                        ))
                    }
                }
                None => StepOutcome::Failed(format!("axis '{axis_name}' not found in report")),
            }
        },
    );

    registry.then(
        r"^button (\d+) should be (pressed|released)$",
        |ctx, caps| {
            let btn_id: u32 = match caps[1].parse() {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad button id: {e}")),
            };
            let expected_pressed = &caps[2] == "pressed";
            let report = match ctx.get::<ParsedReport>("parsed_report") {
                Some(r) => r,
                None => return StepOutcome::Failed("no parsed_report in context".to_string()),
            };
            match report.buttons.iter().find(|(id, _)| *id == btn_id) {
                Some((_, actual)) => {
                    if *actual == expected_pressed {
                        StepOutcome::Passed
                    } else {
                        let state = if *actual { "pressed" } else { "released" };
                        StepOutcome::Failed(format!("button {btn_id} is {state}"))
                    }
                }
                None => StepOutcome::Failed(format!("button {btn_id} not in report")),
            }
        },
    );

    registry.then(
        r"^the device vendor ID should be 0x([0-9A-Fa-f]+)$",
        |ctx, caps| {
            let expected = match u16::from_str_radix(&caps[1], 16) {
                Ok(v) => v,
                Err(e) => return StepOutcome::Failed(format!("bad hex: {e}")),
            };
            match ctx.get::<u16>("device_vendor_id") {
                Some(vid) if *vid == expected => StepOutcome::Passed,
                Some(vid) => StepOutcome::Failed(format!(
                    "expected vendor 0x{expected:04X}, got 0x{:04X}", *vid
                )),
                None => StepOutcome::Failed("no device in context".to_string()),
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{parse_scenario, run_scenario};

    fn registry() -> StepRegistry {
        let mut r = StepRegistry::new();
        register(&mut r);
        r
    }

    #[test]
    fn connect_known_device() {
        let reg = registry();
        let s = parse_scenario(
            "connect",
            r#"Given a "Thrustmaster" "T.Flight HOTAS 4" is connected
Then the device vendor ID should be 0x044F"#,
        );
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "{:?}", result.step_results);
    }

    #[test]
    fn parse_hid_report_axes() {
        let reg = registry();
        // FF 7F = 0x7FFF = 32767 → (32767/65535)*2 - 1 ≈ 0.0
        let s = parse_scenario(
            "hid_axes",
            r#"Given a "VKB" "Gladiator NXT EVO" is connected
When the device sends HID report FF 7F 00 00 FF FF
Then axis "X" should read 0.0 ±0.01"#,
        );
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "{:?}", result.step_results);
    }

    #[test]
    fn parse_hid_report_buttons() {
        let reg = registry();
        // Axes: 6 bytes of zero, then button byte 0x01 = bit 0 pressed
        let s = parse_scenario(
            "hid_buttons",
            r#"Given a "VKB" "Gladiator NXT EVO" is connected
When the device sends HID report 00 00 00 00 00 00 01
Then button 1 should be pressed
And button 2 should be released"#,
        );
        let result = run_scenario(&s, &reg);
        assert!(result.is_passed(), "{:?}", result.step_results);
    }
}
