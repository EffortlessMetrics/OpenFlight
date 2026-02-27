// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Live axis value monitor with ASCII bar chart (REQ-705)

/// Configuration for the axis monitor display.
#[derive(Debug, Clone)]
pub struct AxisMonitorConfig {
    /// Refresh rate in Hz (default 10).
    pub refresh_rate_hz: u32,
    /// Whether to show processing stages.
    pub show_stages: bool,
}

impl Default for AxisMonitorConfig {
    fn default() -> Self {
        Self {
            refresh_rate_hz: 10,
            show_stages: false,
        }
    }
}

const BAR_WIDTH: usize = 40;

/// Format a single axis as an ASCII bar chart line.
///
/// The bar shows the position of `value` between `min` and `max`.
pub fn format_axis_line(name: &str, value: f32, min: f32, max: f32) -> String {
    let range = max - min;
    let ratio = if range.abs() < f32::EPSILON {
        0.5
    } else {
        ((value - min) / range).clamp(0.0, 1.0)
    };

    let filled = (ratio * BAR_WIDTH as f32).round() as usize;
    let empty = BAR_WIDTH - filled;

    format!(
        "{:<20} [{}{}>] {:>7.3}",
        truncate_name(name, 20),
        "=".repeat(filled),
        " ".repeat(empty),
        value,
    )
}

/// Format a complete monitor frame with all axes.
pub fn format_monitor_frame(axes: &[(String, f32)]) -> String {
    if axes.is_empty() {
        return "No axes to display.".to_string();
    }

    let mut lines = Vec::with_capacity(axes.len() + 1);
    lines.push(format!("--- Axis Monitor ({} axes) ---", axes.len()));

    for (name, value) in axes {
        lines.push(format_axis_line(name, *value, -1.0, 1.0));
    }

    lines.join("\n")
}

fn truncate_name(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_chart_min_value_shows_empty_bar() {
        let line = format_axis_line("Roll", -1.0, -1.0, 1.0);
        assert!(line.contains("Roll"));
        // At min, no filled segments
        assert!(line.contains("["));
        assert!(line.contains(">]"));
        assert!(line.contains("-1.000"));
    }

    #[test]
    fn bar_chart_max_value_shows_full_bar() {
        let line = format_axis_line("Pitch", 1.0, -1.0, 1.0);
        assert!(line.contains("Pitch"));
        assert!(line.contains("1.000"));
        // Should have BAR_WIDTH filled characters
        let bar_section: String = line.chars().skip_while(|c| *c != '[').collect();
        let eq_count = bar_section.chars().filter(|c| *c == '=').count();
        assert_eq!(eq_count, BAR_WIDTH);
    }

    #[test]
    fn bar_chart_midpoint_shows_half_bar() {
        let line = format_axis_line("Yaw", 0.0, -1.0, 1.0);
        assert!(line.contains("Yaw"));
        let bar_section: String = line.chars().skip_while(|c| *c != '[').collect();
        let eq_count = bar_section.chars().filter(|c| *c == '=').count();
        assert_eq!(eq_count, BAR_WIDTH / 2);
    }

    #[test]
    fn multiple_axes_formatted_in_frame() {
        let axes = vec![
            ("Roll".to_string(), 0.5),
            ("Pitch".to_string(), -0.3),
            ("Yaw".to_string(), 0.0),
        ];
        let frame = format_monitor_frame(&axes);
        assert!(frame.contains("3 axes"));
        assert!(frame.contains("Roll"));
        assert!(frame.contains("Pitch"));
        assert!(frame.contains("Yaw"));
        // Header + 3 axis lines
        assert_eq!(frame.lines().count(), 4);
    }

    #[test]
    fn empty_axes_shows_message() {
        let frame = format_monitor_frame(&[]);
        assert_eq!(frame, "No axes to display.");
    }

    #[test]
    fn value_beyond_max_is_clamped() {
        let line = format_axis_line("Throttle", 2.0, -1.0, 1.0);
        let bar_section: String = line.chars().skip_while(|c| *c != '[').collect();
        let eq_count = bar_section.chars().filter(|c| *c == '=').count();
        assert_eq!(eq_count, BAR_WIDTH);
    }

    #[test]
    fn value_below_min_is_clamped() {
        let line = format_axis_line("Throttle", -5.0, -1.0, 1.0);
        let bar_section: String = line.chars().skip_while(|c| *c != '[').collect();
        let eq_count = bar_section.chars().filter(|c| *c == '=').count();
        assert_eq!(eq_count, 0);
    }

    #[test]
    fn default_config_has_10hz_refresh() {
        let config = AxisMonitorConfig::default();
        assert_eq!(config.refresh_rate_hz, 10);
        assert!(!config.show_stages);
    }
}
