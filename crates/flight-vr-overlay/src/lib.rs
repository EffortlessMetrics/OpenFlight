// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! VR Overlay for Flight Hub
//!
//! Provides an in-cockpit VR overlay for real-time notifications, profile
//! status, and axis monitoring.
//!
//! # Architecture
//!
//! ```text
//! Flight Hub Bus ──► OverlayService ──► OverlayRenderer (OpenXR / SteamVR)
//!                         │
//!                   NotificationQueue
//!                   OverlayState
//! ```
//!
//! The service is driven by [`OverlayService::spawn`] and controlled via the
//! returned [`OverlayHandle`].
//!
//! # Example
//!
//! ```no_run
//! use flight_vr_overlay::{OverlayConfig, OverlayService};
//! use flight_vr_overlay::renderer::NullRenderer;
//! use flight_vr_overlay::notification::Severity;
//!
//! # tokio_test::block_on(async {
//! let handle = OverlayService::spawn(OverlayConfig::default(), NullRenderer::new());
//! handle.notify("Profile loaded: MSFS-A320", Severity::Info, 4).await?;
//! handle.toggle().await?;
//! handle.shutdown().await?;
//! # Ok::<(), flight_vr_overlay::OverlayError>(())
//! # });
//! ```

pub mod config;
pub mod notification;
pub mod renderer;
pub mod service;
pub mod state;

pub use config::{AnchorPoint, OverlayConfig};
pub use notification::{NotificationQueue, OverlayNotification, Severity};
pub use renderer::{NullRenderer, OverlayRenderer, RendererBackend};
pub use service::{OverlayCommand, OverlayHandle, OverlayService};
pub use state::{AxisStatus, FfbStatus, OverlayState, SimConnectionStatus};

/// Overlay-specific error type.
#[derive(Debug, thiserror::Error)]
pub enum OverlayError {
    /// The overlay service has been shut down.
    #[error("overlay service has been shut down")]
    ServiceShutdown,

    /// The VR runtime is not available or failed to initialise.
    #[error("VR runtime unavailable: {0}")]
    RuntimeUnavailable(String),

    /// Renderer-specific error.
    #[error("renderer error: {0}")]
    Renderer(String),

    /// Configuration validation error.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
}
