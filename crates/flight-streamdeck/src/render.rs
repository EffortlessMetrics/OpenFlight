// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Button image rendering for StreamDeck keys
//!
//! Generates icon descriptors for flight sim functions. The actual pixel
//! rasterisation is deferred to the StreamDeck SDK / plugin layer; this module
//! produces the logical icon specification (colors, text, layout) that the
//! renderer consumes.

use crate::device::StreamDeckModel;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors during icon rendering.
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("Icon size must be > 0")]
    InvalidSize,
    #[error("Color format invalid: {0}")]
    InvalidColor(String),
}

/// Horizontal text alignment on a key face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Visual style for a rendered button icon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconStyle {
    pub background_color: String,
    pub text_color: String,
    pub accent_color: String,
    pub font_size: u32,
    pub text_align: TextAlign,
    pub bold: bool,
}

impl Default for IconStyle {
    fn default() -> Self {
        Self {
            background_color: "#1A1A2E".to_string(),
            text_color: "#FFFFFF".to_string(),
            accent_color: "#00D4FF".to_string(),
            font_size: 14,
            text_align: TextAlign::Center,
            bold: false,
        }
    }
}

/// Predefined color themes for different function categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconTheme {
    Autopilot,
    Communication,
    Navigation,
    Lights,
    Systems,
    Warning,
    Custom,
}

impl IconTheme {
    /// Returns (background, text, accent) hex colors for each theme.
    pub fn colors(&self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Autopilot => ("#0A1628", "#00FF88", "#00D4FF"),
            Self::Communication => ("#1A0A28", "#FFD700", "#FF8C00"),
            Self::Navigation => ("#0A1A28", "#00BFFF", "#1E90FF"),
            Self::Lights => ("#28280A", "#FFFF00", "#FFA500"),
            Self::Systems => ("#0A2818", "#00FF00", "#32CD32"),
            Self::Warning => ("#280A0A", "#FF0000", "#FF4500"),
            Self::Custom => ("#1A1A2E", "#FFFFFF", "#00D4FF"),
        }
    }

    /// Build an [`IconStyle`] from the theme defaults.
    pub fn to_style(&self) -> IconStyle {
        let (bg, text, accent) = self.colors();
        IconStyle {
            background_color: bg.to_string(),
            text_color: text.to_string(),
            accent_color: accent.to_string(),
            ..IconStyle::default()
        }
    }
}

/// Logical specification of a rendered key icon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyIcon {
    /// Pixel size (square).
    pub size: u32,
    /// Primary label shown on the key.
    pub label: String,
    /// Optional secondary value line (e.g. "350" for altitude).
    pub value: Option<String>,
    /// Visual style.
    pub style: IconStyle,
    /// Whether the associated function is currently active/on.
    pub active: bool,
}

impl KeyIcon {
    /// Create an icon sized for the given model.
    pub fn for_model(model: StreamDeckModel, label: &str, theme: IconTheme) -> Option<Self> {
        let size = model.icon_size()?;
        let style = theme.to_style();
        Some(Self {
            size,
            label: label.to_string(),
            value: None,
            style,
            active: false,
        })
    }

    /// Set the value sub-label.
    pub fn with_value(mut self, value: &str) -> Self {
        self.value = Some(value.to_string());
        self
    }

    /// Mark the icon as active (on state).
    pub fn with_active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Apply a custom style override.
    pub fn with_style(mut self, style: IconStyle) -> Self {
        self.style = style;
        self
    }

    /// Produce the "active" variant: swap text color to accent color.
    pub fn active_variant(&self) -> Self {
        let mut icon = self.clone();
        icon.active = true;
        icon.style.text_color = icon.style.accent_color.clone();
        icon
    }

    /// Produce the "inactive" (off) variant.
    pub fn inactive_variant(&self) -> Self {
        let mut icon = self.clone();
        icon.active = false;
        let (_bg, text, _accent) = IconTheme::Custom.colors();
        icon.style.text_color = text.to_string();
        icon
    }
}

/// Convenience builder for producing a full set of key icons for a device.
pub struct IconRenderer {
    model: StreamDeckModel,
}

impl IconRenderer {
    pub fn new(model: StreamDeckModel) -> Self {
        Self { model }
    }

    /// Render a toggle-style icon pair (off / on).
    pub fn toggle_icon(&self, label: &str, theme: IconTheme) -> Option<(KeyIcon, KeyIcon)> {
        let base = KeyIcon::for_model(self.model, label, theme)?;
        Some((base.inactive_variant(), base.active_variant()))
    }

    /// Render a value-display icon (e.g. HDG 270).
    pub fn value_icon(&self, label: &str, value: &str, theme: IconTheme) -> Option<KeyIcon> {
        KeyIcon::for_model(self.model, label, theme).map(|i| i.with_value(value))
    }

    /// Render a momentary-action icon (single state).
    pub fn momentary_icon(&self, label: &str, theme: IconTheme) -> Option<KeyIcon> {
        KeyIcon::for_model(self.model, label, theme)
    }

    /// Icon size for the current model (convenience).
    pub fn icon_size(&self) -> Option<u32> {
        self.model.icon_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── IconTheme ──────────────────────────────────────────────────────

    #[test]
    fn test_theme_colors_non_empty() {
        let themes = [
            IconTheme::Autopilot,
            IconTheme::Communication,
            IconTheme::Navigation,
            IconTheme::Lights,
            IconTheme::Systems,
            IconTheme::Warning,
            IconTheme::Custom,
        ];
        for theme in themes {
            let (bg, text, accent) = theme.colors();
            assert!(bg.starts_with('#'), "{theme:?} bg");
            assert!(text.starts_with('#'), "{theme:?} text");
            assert!(accent.starts_with('#'), "{theme:?} accent");
        }
    }

    #[test]
    fn test_theme_to_style() {
        let style = IconTheme::Autopilot.to_style();
        assert_eq!(style.background_color, "#0A1628");
        assert_eq!(style.text_color, "#00FF88");
    }

    // ── KeyIcon ────────────────────────────────────────────────────────

    #[test]
    fn test_key_icon_for_model_original() {
        let icon =
            KeyIcon::for_model(StreamDeckModel::Original, "HDG", IconTheme::Autopilot).unwrap();
        assert_eq!(icon.size, 72);
        assert_eq!(icon.label, "HDG");
        assert!(!icon.active);
    }

    #[test]
    fn test_key_icon_for_mk2() {
        let icon =
            KeyIcon::for_model(StreamDeckModel::Mk2, "AP", IconTheme::Autopilot).unwrap();
        assert_eq!(icon.size, 72);
        assert_eq!(icon.label, "AP");
    }

    #[test]
    fn test_key_icon_for_all_displayable_models() {
        for model in StreamDeckModel::all() {
            let result = KeyIcon::for_model(*model, "TST", IconTheme::Custom);
            if model.has_display() {
                assert!(result.is_some(), "{:?} should produce an icon", model);
                let icon = result.unwrap();
                assert_eq!(icon.size, model.icon_size().unwrap());
            } else {
                assert!(result.is_none(), "{:?} should not produce an icon", model);
            }
        }
    }

    #[test]
    fn test_key_icon_for_pedal_returns_none() {
        assert!(KeyIcon::for_model(StreamDeckModel::Pedal, "X", IconTheme::Custom).is_none());
    }

    #[test]
    fn test_active_variant_swaps_color() {
        let icon = KeyIcon::for_model(StreamDeckModel::Xl, "ALT", IconTheme::Navigation).unwrap();
        let active = icon.active_variant();
        assert!(active.active);
        assert_eq!(active.style.text_color, active.style.accent_color);
    }

    #[test]
    fn test_inactive_variant() {
        let icon = KeyIcon::for_model(StreamDeckModel::Xl, "ALT", IconTheme::Navigation).unwrap();
        let active = icon.active_variant();
        let inactive = active.inactive_variant();
        assert!(!inactive.active);
    }

    #[test]
    fn test_with_value() {
        let icon = KeyIcon::for_model(StreamDeckModel::Plus, "SPD", IconTheme::Autopilot)
            .unwrap()
            .with_value("250");
        assert_eq!(icon.value.as_deref(), Some("250"));
    }

    // ── IconRenderer ───────────────────────────────────────────────────

    #[test]
    fn test_toggle_icon_pair() {
        let renderer = IconRenderer::new(StreamDeckModel::Original);
        let (off, on) = renderer.toggle_icon("NAV", IconTheme::Lights).unwrap();
        assert!(!off.active);
        assert!(on.active);
    }

    #[test]
    fn test_value_icon() {
        let renderer = IconRenderer::new(StreamDeckModel::Xl);
        let icon = renderer
            .value_icon("HDG", "270", IconTheme::Autopilot)
            .unwrap();
        assert_eq!(icon.value.as_deref(), Some("270"));
    }

    #[test]
    fn test_momentary_icon() {
        let renderer = IconRenderer::new(StreamDeckModel::Mini);
        let icon = renderer
            .momentary_icon("COM SWAP", IconTheme::Communication)
            .unwrap();
        assert!(!icon.active);
    }

    #[test]
    fn test_renderer_pedal_returns_none() {
        let renderer = IconRenderer::new(StreamDeckModel::Pedal);
        assert!(renderer.toggle_icon("X", IconTheme::Custom).is_none());
        assert!(renderer.icon_size().is_none());
    }

    #[test]
    fn test_renderer_icon_size() {
        let renderer = IconRenderer::new(StreamDeckModel::Plus);
        assert_eq!(renderer.icon_size(), Some(120));
    }

    #[test]
    fn test_renderer_mk2() {
        let renderer = IconRenderer::new(StreamDeckModel::Mk2);
        assert_eq!(renderer.icon_size(), Some(72));
        let icon = renderer.momentary_icon("BAT", IconTheme::Systems).unwrap();
        assert_eq!(icon.size, 72);
    }

    #[test]
    fn test_icon_style_default() {
        let style = IconStyle::default();
        assert_eq!(style.font_size, 14);
        assert_eq!(style.text_align, TextAlign::Center);
        assert!(!style.bold);
    }

    #[test]
    fn test_with_style_override() {
        let icon = KeyIcon::for_model(StreamDeckModel::Original, "X", IconTheme::Custom).unwrap();
        let custom_style = IconStyle {
            bold: true,
            font_size: 24,
            ..IconStyle::default()
        };
        let styled = icon.with_style(custom_style);
        assert!(styled.style.bold);
        assert_eq!(styled.style.font_size, 24);
    }

    #[test]
    fn test_icon_sizes_match_model_spec() {
        let cases: &[(StreamDeckModel, u32)] = &[
            (StreamDeckModel::Original, 72),
            (StreamDeckModel::Mk2, 72),
            (StreamDeckModel::Mini, 80),
            (StreamDeckModel::Xl, 96),
            (StreamDeckModel::Plus, 120),
            (StreamDeckModel::Neo, 96),
        ];
        for (model, expected) in cases {
            let renderer = IconRenderer::new(*model);
            assert_eq!(renderer.icon_size(), Some(*expected), "{:?}", model);
        }
    }
}
