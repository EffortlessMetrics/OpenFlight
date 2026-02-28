// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Device listing and formatting (REQ-684)

use serde::Serialize;

/// Information about a detected device.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct DeviceInfo {
    /// USB Vendor ID
    pub vid: u16,
    /// USB Product ID
    pub pid: u16,
    /// Human-readable device name
    pub name: String,
    /// Number of axes
    pub axes: u8,
    /// Number of buttons
    pub buttons: u8,
    /// Whether the device supports force feedback
    pub ffb_capable: bool,
    /// Whether the device is currently connected
    pub connected: bool,
}

impl DeviceInfo {
    fn vid_pid(&self) -> String {
        format!("{:04X}:{:04X}", self.vid, self.pid)
    }

    fn status_str(&self) -> &'static str {
        if self.connected {
            "Connected"
        } else {
            "Disconnected"
        }
    }

    fn ffb_str(&self) -> &'static str {
        if self.ffb_capable { "Yes" } else { "No" }
    }
}

/// Format a list of devices as a human-readable table or JSON.
pub fn format_device_list(devices: &[DeviceInfo], json: bool) -> String {
    if json {
        return serde_json::to_string_pretty(devices).unwrap_or_else(|_| "[]".to_string());
    }

    if devices.is_empty() {
        return "No devices detected.".to_string();
    }

    // Column headers
    let header = format!(
        "| {:<9} | {:<30} | {:<4} | {:<7} | {:<3} | {:<12} |",
        "VID:PID", "Name", "Axes", "Buttons", "FFB", "Status"
    );
    let separator = format!(
        "|{:-<11}|{:-<32}|{:-<6}|{:-<9}|{:-<5}|{:-<14}|",
        "", "", "", "", "", ""
    );

    let mut lines = Vec::with_capacity(devices.len() + 2);
    lines.push(header);
    lines.push(separator);

    for d in devices {
        lines.push(format!(
            "| {:<9} | {:<30} | {:<4} | {:<7} | {:<3} | {:<12} |",
            d.vid_pid(),
            truncate(&d.name, 30),
            d.axes,
            d.buttons,
            d.ffb_str(),
            d.status_str(),
        ));
    }

    lines.join("\n")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_device(name: &str, connected: bool) -> DeviceInfo {
        DeviceInfo {
            vid: 0x044F,
            pid: 0xB10A,
            name: name.to_string(),
            axes: 6,
            buttons: 32,
            ffb_capable: true,
            connected,
        }
    }

    #[test]
    fn table_format_contains_header_and_rows() {
        let devices = vec![sample_device("TM Warthog Stick", true)];
        let output = format_device_list(&devices, false);
        assert!(output.contains("VID:PID"));
        assert!(output.contains("Name"));
        assert!(output.contains("044F:B10A"));
        assert!(output.contains("TM Warthog Stick"));
    }

    #[test]
    fn json_output_is_valid() {
        let devices = vec![sample_device("Test Device", true)];
        let output = format_device_list(&devices, true);
        let parsed: Vec<DeviceInfo> = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "Test Device");
    }

    #[test]
    fn empty_list_shows_message() {
        let output = format_device_list(&[], false);
        assert_eq!(output, "No devices detected.");
    }

    #[test]
    fn empty_list_json_is_empty_array() {
        let output = format_device_list(&[], true);
        let parsed: Vec<DeviceInfo> = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn disconnected_device_shown_with_status() {
        let devices = vec![sample_device("Disconnected Stick", false)];
        let output = format_device_list(&devices, false);
        assert!(output.contains("Disconnected"));
    }

    #[test]
    fn multiple_devices_each_on_own_row() {
        let devices = vec![
            sample_device("Device A", true),
            sample_device("Device B", false),
        ];
        let output = format_device_list(&devices, false);
        assert!(output.contains("Device A"));
        assert!(output.contains("Device B"));
        // Header + separator + 2 data rows = 4 lines
        assert_eq!(output.lines().count(), 4);
    }
}
