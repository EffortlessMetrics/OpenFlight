// SPDX-License-Identifier: MIT OR Apache-2.0

//! X56 RGB lighting implementation.
//!
//! **UNVERIFIED PROTOCOL** - See `docs/reference/hotas-claims.md`
//!
//! The X56 RGB protocol is largely unknown. This implementation provides
//! a placeholder for future community verification.

use crate::traits::{HotasError, HotasResult, RgbColor, RgbProtocol, RgbZone};

/// X56 RGB lighting controller.
///
/// # Protocol Status
///
/// **UNVERIFIED** - The X56 RGB protocol packet format is unknown.
/// This is a placeholder for community verification.
pub struct X56Rgb {
    #[allow(dead_code)]
    device_path: String,
}

impl X56Rgb {
    /// Create a new RGB controller.
    pub fn new(device_path: String) -> Self {
        tracing::warn!(
            target: "hotas::rgb",
            device = %device_path,
            "Creating X56 RGB controller with UNKNOWN protocol. \
             Protocol verification needed - see docs/reference/hotas-claims.md"
        );

        Self { device_path }
    }
}

impl RgbProtocol for X56Rgb {
    fn set_color(&mut self, zone: RgbZone, color: RgbColor) -> HotasResult<()> {
        tracing::info!(
            target: "hotas::rgb",
            zone = ?zone,
            r = color.r,
            g = color.g,
            b = color.b,
            "Attempting to set RGB color (UNKNOWN protocol)"
        );

        // X56 RGB protocol is completely unknown
        Err(HotasError::UnverifiedProtocol("x56_rgb"))
    }

    fn set_all(&mut self, color: RgbColor) -> HotasResult<()> {
        tracing::info!(
            target: "hotas::rgb",
            r = color.r,
            g = color.g,
            b = color.b,
            "Attempting to set all RGB zones (UNKNOWN protocol)"
        );

        Err(HotasError::UnverifiedProtocol("x56_rgb"))
    }
}
