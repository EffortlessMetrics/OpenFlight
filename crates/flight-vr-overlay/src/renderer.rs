// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Renderer abstraction for the VR overlay.
//!
//! The [`OverlayRenderer`] trait decouples the panel logic from the underlying
//! VR runtime (OpenXR, SteamVR overlay, etc.).  A [`NullRenderer`] is provided
//! for tests and headless environments.

use crate::{OverlayError, OverlayState};

/// Trait implemented by VR runtime backends that can draw the overlay panel.
///
/// # Contract
/// - `render_frame` must be cheap to call at display frequency (90–120 Hz).
/// - Implementations must be `Send` so the overlay service can drive them from
///   a dedicated thread.
pub trait OverlayRenderer: Send + 'static {
    /// Render one frame with the given state snapshot.
    fn render_frame(&mut self, state: &OverlayState) -> Result<(), OverlayError>;

    /// Make the overlay panel visible in the headset.
    fn show(&mut self) -> Result<(), OverlayError>;

    /// Hide the overlay panel (retains state but stops drawing).
    fn hide(&mut self) -> Result<(), OverlayError>;

    /// Set the panel opacity (0.0 = transparent, 1.0 = opaque).
    fn set_opacity(&mut self, opacity: f32) -> Result<(), OverlayError>;

    /// Return a human-readable name for this renderer backend.
    fn backend_name(&self) -> &'static str;
}

/// A no-op renderer used in tests and headless service mode.
///
/// Records calls for later assertion.
#[derive(Debug, Default)]
pub struct NullRenderer {
    pub frames_rendered: u64,
    pub visible: bool,
    pub opacity: f32,
    pub last_profile: Option<String>,
}

impl NullRenderer {
    /// Create a new `NullRenderer` with opacity 1.0 and visibility `false`.
    pub fn new() -> Self {
        Self {
            frames_rendered: 0,
            visible: false,
            opacity: 1.0,
            last_profile: None,
        }
    }
}

impl OverlayRenderer for NullRenderer {
    fn render_frame(&mut self, state: &OverlayState) -> Result<(), OverlayError> {
        self.frames_rendered += 1;
        self.last_profile = Some(state.profile_name.clone());
        Ok(())
    }

    fn show(&mut self) -> Result<(), OverlayError> {
        self.visible = true;
        Ok(())
    }

    fn hide(&mut self) -> Result<(), OverlayError> {
        self.visible = false;
        Ok(())
    }

    fn set_opacity(&mut self, opacity: f32) -> Result<(), OverlayError> {
        self.opacity = opacity.clamp(0.0, 1.0);
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "null"
    }
}

/// Placeholder for future OpenXR backend selection.
///
/// An actual OpenXR renderer lives behind an optional crate feature and
/// requires an active VR runtime at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererBackend {
    /// No-op renderer (tests / headless).
    Null,
    /// OpenXR compositor layer (requires `openxr` feature).
    OpenXr,
    /// SteamVR IVROverlay API (Windows only; requires `steamvr` feature).
    SteamVr,
}

impl std::fmt::Display for RendererBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => f.write_str("Null (headless)"),
            Self::OpenXr => f.write_str("OpenXR"),
            Self::SteamVr => f.write_str("SteamVR Overlay"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OverlayState;

    #[test]
    fn test_null_renderer_counts_frames() {
        let mut r = NullRenderer::new();
        let state = OverlayState::default();
        r.render_frame(&state).unwrap();
        r.render_frame(&state).unwrap();
        assert_eq!(r.frames_rendered, 2);
    }

    #[test]
    fn test_null_renderer_show_hide() {
        let mut r = NullRenderer::new();
        assert!(!r.visible);
        r.show().unwrap();
        assert!(r.visible);
        r.hide().unwrap();
        assert!(!r.visible);
    }

    #[test]
    fn test_null_renderer_set_opacity_clamps() {
        let mut r = NullRenderer::new();
        r.set_opacity(1.5).unwrap();
        assert!((r.opacity - 1.0).abs() < 1e-6);
        r.set_opacity(-0.5).unwrap();
        assert!((r.opacity).abs() < 1e-6);
    }

    #[test]
    fn test_null_renderer_records_last_profile() {
        let mut r = NullRenderer::new();
        let mut state = OverlayState::default();
        state.profile_name = "MSFS-F18".to_string();
        r.render_frame(&state).unwrap();
        assert_eq!(r.last_profile.as_deref(), Some("MSFS-F18"));
    }

    #[test]
    fn test_backend_display() {
        assert_eq!(RendererBackend::Null.to_string(), "Null (headless)");
        assert_eq!(RendererBackend::OpenXr.to_string(), "OpenXR");
        assert_eq!(RendererBackend::SteamVr.to_string(), "SteamVR Overlay");
    }
}
