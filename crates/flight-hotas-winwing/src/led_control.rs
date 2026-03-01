// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Higher-level LED and display control for WinWing devices.
//!
//! This module provides a batch-oriented API on top of the low-level
//! feature-report builders in [`crate::protocol`].  Instead of building
//! individual feature reports, callers assemble a sequence of
//! [`LedCommand`] or [`DisplayCommand`] values and call
//! [`build_led_report`] / [`build_display_report`] to produce the raw
//! byte buffers that should be sent to the device.

use crate::protocol::{
    CommandCategory, DisplaySubCommand, FeatureReportFrame, ProtocolError,
    build_backlight_all_command, build_backlight_all_rgb_command, build_backlight_single_command,
    build_backlight_single_rgb_command,
};

// ── LED command ───────────────────────────────────────────────────────────────

/// An individual LED control instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedCommand {
    /// Set a single button's backlight to a monochrome intensity (0–255).
    SetIntensity {
        panel_id: u8,
        button_index: u8,
        intensity: u8,
    },
    /// Set a single button's backlight to an RGB colour.
    SetRgb {
        panel_id: u8,
        button_index: u8,
        r: u8,
        g: u8,
        b: u8,
    },
    /// Set all buttons on a panel to the same monochrome intensity.
    SetAllIntensity { panel_id: u8, intensity: u8 },
    /// Set all buttons on a panel to the same RGB colour.
    SetAllRgb { panel_id: u8, r: u8, g: u8, b: u8 },
}

/// Build one feature-report frame per [`LedCommand`].
///
/// Returns one `Vec<u8>` per command containing the raw bytes to send.
///
/// # Errors
///
/// Propagates [`ProtocolError`] if any command cannot be encoded.
pub fn build_led_report(commands: &[LedCommand]) -> Result<Vec<Vec<u8>>, ProtocolError> {
    let mut reports = Vec::with_capacity(commands.len());
    for cmd in commands {
        let frame = match cmd {
            LedCommand::SetIntensity {
                panel_id,
                button_index,
                intensity,
            } => build_backlight_single_command(*panel_id, *button_index, *intensity)?,
            LedCommand::SetRgb {
                panel_id,
                button_index,
                r,
                g,
                b,
            } => build_backlight_single_rgb_command(*panel_id, *button_index, *r, *g, *b)?,
            LedCommand::SetAllIntensity {
                panel_id,
                intensity,
            } => build_backlight_all_command(*panel_id, *intensity)?,
            LedCommand::SetAllRgb { panel_id, r, g, b } => {
                build_backlight_all_rgb_command(*panel_id, *r, *g, *b)?
            }
        };
        reports.push(frame.as_bytes().to_vec());
    }
    Ok(reports)
}

// ── Display command ───────────────────────────────────────────────────────────

/// An individual display control instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayCommand {
    /// Write ASCII text to a display field (truncated to 16 chars).
    WriteText {
        panel_id: u8,
        field_index: u8,
        text: String,
    },
    /// Write raw 7-segment bitmask data to a display field.
    WriteSegment {
        panel_id: u8,
        field_index: u8,
        segments: Vec<u8>,
    },
    /// Set display brightness (0–255).
    SetBrightness { panel_id: u8, brightness: u8 },
    /// Clear all fields on a panel display.
    ClearAll { panel_id: u8 },
}

/// Build one feature-report frame per [`DisplayCommand`].
///
/// Returns one `Vec<u8>` per command containing the raw bytes to send.
///
/// # Errors
///
/// Propagates [`ProtocolError`] if any command cannot be encoded.
pub fn build_display_report(commands: &[DisplayCommand]) -> Result<Vec<Vec<u8>>, ProtocolError> {
    let mut reports = Vec::with_capacity(commands.len());
    for cmd in commands {
        let frame = match cmd {
            DisplayCommand::WriteText {
                panel_id,
                field_index,
                text,
            } => {
                let bytes = text.as_bytes();
                let truncated = if bytes.len() > 16 {
                    &bytes[..16]
                } else {
                    bytes
                };
                let mut payload = Vec::with_capacity(2 + truncated.len());
                payload.push(*panel_id);
                payload.push(*field_index);
                payload.extend_from_slice(truncated);
                FeatureReportFrame::new(
                    CommandCategory::Display,
                    DisplaySubCommand::WriteText as u8,
                    &payload,
                )?
            }
            DisplayCommand::WriteSegment {
                panel_id,
                field_index,
                segments,
            } => {
                let mut payload = Vec::with_capacity(2 + segments.len());
                payload.push(*panel_id);
                payload.push(*field_index);
                payload.extend_from_slice(segments);
                FeatureReportFrame::new(
                    CommandCategory::Display,
                    DisplaySubCommand::WriteSegment as u8,
                    &payload,
                )?
            }
            DisplayCommand::SetBrightness {
                panel_id,
                brightness,
            } => FeatureReportFrame::new(
                CommandCategory::Display,
                DisplaySubCommand::SetBrightness as u8,
                &[*panel_id, *brightness],
            )?,
            DisplayCommand::ClearAll { panel_id } => FeatureReportFrame::new(
                CommandCategory::Display,
                DisplaySubCommand::ClearAll as u8,
                &[*panel_id],
            )?,
        };
        reports.push(frame.as_bytes().to_vec());
    }
    Ok(reports)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        BacklightSubCommand, FEATURE_REPORT_ID, MIN_FRAME_LEN, parse_feature_report,
    };

    // ── LED commands ──────────────────────────────────────────────────────

    #[test]
    fn test_build_led_report_empty() {
        let reports = build_led_report(&[]).unwrap();
        assert!(reports.is_empty());
    }

    #[test]
    fn test_build_led_report_single_intensity() {
        let cmds = [LedCommand::SetIntensity {
            panel_id: 0x01,
            button_index: 5,
            intensity: 200,
        }];
        let reports = build_led_report(&cmds).unwrap();
        assert_eq!(reports.len(), 1);

        let parsed = parse_feature_report(&reports[0]).unwrap();
        assert_eq!(parsed.category, CommandCategory::Backlight);
        assert_eq!(parsed.sub_command, BacklightSubCommand::SetSingle as u8);
        assert_eq!(parsed.payload, &[0x01, 5, 200]);
    }

    #[test]
    fn test_build_led_report_single_rgb() {
        let cmds = [LedCommand::SetRgb {
            panel_id: 0x02,
            button_index: 3,
            r: 255,
            g: 128,
            b: 0,
        }];
        let reports = build_led_report(&cmds).unwrap();
        assert_eq!(reports.len(), 1);

        let parsed = parse_feature_report(&reports[0]).unwrap();
        assert_eq!(parsed.sub_command, BacklightSubCommand::SetSingleRgb as u8);
        assert_eq!(parsed.payload, &[0x02, 3, 255, 128, 0]);
    }

    #[test]
    fn test_build_led_report_all_intensity() {
        let cmds = [LedCommand::SetAllIntensity {
            panel_id: 0x01,
            intensity: 128,
        }];
        let reports = build_led_report(&cmds).unwrap();
        let parsed = parse_feature_report(&reports[0]).unwrap();
        assert_eq!(parsed.sub_command, BacklightSubCommand::SetAll as u8);
        assert_eq!(parsed.payload, &[0x01, 128]);
    }

    #[test]
    fn test_build_led_report_all_rgb() {
        let cmds = [LedCommand::SetAllRgb {
            panel_id: 0x02,
            r: 0,
            g: 255,
            b: 0,
        }];
        let reports = build_led_report(&cmds).unwrap();
        let parsed = parse_feature_report(&reports[0]).unwrap();
        assert_eq!(parsed.sub_command, BacklightSubCommand::SetAllRgb as u8);
        assert_eq!(parsed.payload, &[0x02, 0, 255, 0]);
    }

    #[test]
    fn test_build_led_report_multiple_commands() {
        let cmds = [
            LedCommand::SetIntensity {
                panel_id: 0x01,
                button_index: 0,
                intensity: 0,
            },
            LedCommand::SetRgb {
                panel_id: 0x01,
                button_index: 1,
                r: 255,
                g: 0,
                b: 0,
            },
            LedCommand::SetAllIntensity {
                panel_id: 0x01,
                intensity: 255,
            },
        ];
        let reports = build_led_report(&cmds).unwrap();
        assert_eq!(reports.len(), 3);

        // Verify each report starts with the feature report ID
        for report in &reports {
            assert_eq!(report[0], FEATURE_REPORT_ID);
        }
    }

    #[test]
    fn test_led_off_command() {
        let cmds = [LedCommand::SetIntensity {
            panel_id: 0x01,
            button_index: 0,
            intensity: 0,
        }];
        let reports = build_led_report(&cmds).unwrap();
        let parsed = parse_feature_report(&reports[0]).unwrap();
        assert_eq!(parsed.payload[2], 0);
    }

    #[test]
    fn test_led_full_brightness() {
        let cmds = [LedCommand::SetIntensity {
            panel_id: 0x01,
            button_index: 0,
            intensity: 255,
        }];
        let reports = build_led_report(&cmds).unwrap();
        let parsed = parse_feature_report(&reports[0]).unwrap();
        assert_eq!(parsed.payload[2], 255);
    }

    #[test]
    fn test_led_report_frame_structure() {
        let cmds = [LedCommand::SetIntensity {
            panel_id: 0x01,
            button_index: 0,
            intensity: 128,
        }];
        let reports = build_led_report(&cmds).unwrap();
        let frame = &reports[0];
        // Min frame + 3 byte payload
        assert_eq!(frame.len(), MIN_FRAME_LEN + 3);
        assert_eq!(frame[0], FEATURE_REPORT_ID);
        assert_eq!(frame[1], CommandCategory::Backlight as u8);
    }

    // ── Display commands ──────────────────────────────────────────────────

    #[test]
    fn test_build_display_report_empty() {
        let reports = build_display_report(&[]).unwrap();
        assert!(reports.is_empty());
    }

    #[test]
    fn test_build_display_report_write_text() {
        let cmds = [DisplayCommand::WriteText {
            panel_id: 0x01,
            field_index: 0,
            text: "12345".to_string(),
        }];
        let reports = build_display_report(&cmds).unwrap();
        assert_eq!(reports.len(), 1);

        let parsed = parse_feature_report(&reports[0]).unwrap();
        assert_eq!(parsed.category, CommandCategory::Display);
        assert_eq!(parsed.sub_command, DisplaySubCommand::WriteText as u8);
        assert_eq!(parsed.payload[0], 0x01);
        assert_eq!(parsed.payload[1], 0x00);
        assert_eq!(&parsed.payload[2..], b"12345");
    }

    #[test]
    fn test_build_display_report_text_truncation() {
        let cmds = [DisplayCommand::WriteText {
            panel_id: 0x01,
            field_index: 0,
            text: "ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_string(),
        }];
        let reports = build_display_report(&cmds).unwrap();
        let parsed = parse_feature_report(&reports[0]).unwrap();
        assert_eq!(parsed.payload.len(), 2 + 16);
        assert_eq!(&parsed.payload[2..], b"ABCDEFGHIJKLMNOP");
    }

    #[test]
    fn test_build_display_report_segments() {
        let cmds = [DisplayCommand::WriteSegment {
            panel_id: 0x02,
            field_index: 0,
            segments: vec![0x7F, 0x06, 0x5B],
        }];
        let reports = build_display_report(&cmds).unwrap();
        let parsed = parse_feature_report(&reports[0]).unwrap();
        assert_eq!(parsed.sub_command, DisplaySubCommand::WriteSegment as u8);
        assert_eq!(parsed.payload[0], 0x02);
        assert_eq!(&parsed.payload[2..], &[0x7F, 0x06, 0x5B]);
    }

    #[test]
    fn test_build_display_report_brightness() {
        let cmds = [DisplayCommand::SetBrightness {
            panel_id: 0x01,
            brightness: 200,
        }];
        let reports = build_display_report(&cmds).unwrap();
        let parsed = parse_feature_report(&reports[0]).unwrap();
        assert_eq!(parsed.sub_command, DisplaySubCommand::SetBrightness as u8);
        assert_eq!(parsed.payload, &[0x01, 200]);
    }

    #[test]
    fn test_build_display_report_clear() {
        let cmds = [DisplayCommand::ClearAll { panel_id: 0x03 }];
        let reports = build_display_report(&cmds).unwrap();
        let parsed = parse_feature_report(&reports[0]).unwrap();
        assert_eq!(parsed.sub_command, DisplaySubCommand::ClearAll as u8);
        assert_eq!(parsed.payload, &[0x03]);
    }

    #[test]
    fn test_build_display_report_mixed_commands() {
        let cmds = [
            DisplayCommand::SetBrightness {
                panel_id: 0x01,
                brightness: 255,
            },
            DisplayCommand::WriteText {
                panel_id: 0x01,
                field_index: 0,
                text: "UFC".to_string(),
            },
            DisplayCommand::ClearAll { panel_id: 0x02 },
        ];
        let reports = build_display_report(&cmds).unwrap();
        assert_eq!(reports.len(), 3);
    }

    #[test]
    fn test_display_empty_text() {
        let cmds = [DisplayCommand::WriteText {
            panel_id: 0x01,
            field_index: 0,
            text: String::new(),
        }];
        let reports = build_display_report(&cmds).unwrap();
        let parsed = parse_feature_report(&reports[0]).unwrap();
        // payload = panel_id + field_index only
        assert_eq!(parsed.payload.len(), 2);
    }

    #[test]
    fn test_display_brightness_min_max() {
        for brightness in [0u8, 255u8] {
            let cmds = [DisplayCommand::SetBrightness {
                panel_id: 0x01,
                brightness,
            }];
            let reports = build_display_report(&cmds).unwrap();
            let parsed = parse_feature_report(&reports[0]).unwrap();
            assert_eq!(parsed.payload[1], brightness);
        }
    }
}
