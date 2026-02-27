// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Translates [`RacingTelemetry`] into force-feedback effect parameters.

use crate::telemetry::RacingTelemetry;

/// Force-feedback output for a single processing tick.
#[derive(Debug, Clone, Default)]
pub struct FfbOutput {
    /// Lateral centering / cornering force, normalised to `−1.0` – `1.0`.
    pub lateral_force: f32,
    /// Engine vibration frequency in Hz (RPM ÷ 60).
    pub vibration_hz: f32,
    /// Engine vibration amplitude, normalised to `0.0` – `1.0`.
    pub vibration_amp: f32,
    /// Kerb / road-texture rumble amplitude, normalised to `0.0` – `1.0`.
    pub rumble_amp: f32,
}

/// Translates [`RacingTelemetry`] into [`FfbOutput`] parameters.
#[derive(Debug, Clone)]
pub struct RacingFfbTranslator {
    /// Maximum force in Newtons used to normalise lateral G loads. Default: `10.0`.
    pub max_force_n: f32,
    /// Scaling factor applied to vibration and rumble amplitudes. Default: `1.0`.
    pub rumble_scale: f32,
}

impl Default for RacingFfbTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl RacingFfbTranslator {
    /// Create a translator with default parameters (`max_force_n = 10.0`, `rumble_scale = 1.0`).
    pub fn new() -> Self {
        Self {
            max_force_n: 10.0,
            rumble_scale: 1.0,
        }
    }

    /// Translate a telemetry sample into FFB effect parameters for one tick.
    pub fn translate(&self, telemetry: &RacingTelemetry) -> FfbOutput {
        let lateral_force = (telemetry.lateral_g / (self.max_force_n * 0.1)).clamp(-1.0, 1.0);
        let vibration_hz = telemetry.rpm / 60.0;
        let vibration_amp = (telemetry.rpm_normalized() * self.rumble_scale).clamp(0.0, 1.0);
        let rumble_amp = (telemetry.vertical_g.abs().min(1.0) * self.rumble_scale).clamp(0.0, 1.0);

        tracing::trace!(
            lateral_force,
            vibration_hz,
            vibration_amp,
            rumble_amp,
            "FFB translation complete"
        );

        FfbOutput {
            lateral_force,
            vibration_hz,
            vibration_amp,
            rumble_amp,
        }
    }
}
