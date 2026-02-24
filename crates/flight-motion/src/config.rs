// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Motion platform configuration types.

use serde::{Deserialize, Serialize};

/// Configuration for a single degree-of-freedom channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoFConfig {
    /// Enable this channel (default: true).
    pub enabled: bool,
    /// Output gain multiplier applied after washout (0.0–2.0, default: 1.0).
    pub gain: f32,
    /// Invert the channel direction (default: false).
    pub invert: bool,
}

impl Default for DoFConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            gain: 1.0,
            invert: false,
        }
    }
}

/// Washout filter configuration.
///
/// The classic washout filter uses a high-pass filter on translational channels
/// (surge, sway, heave) to produce transient motion cues for acceleration onset
/// that fade back to neutral — avoiding sustained platform displacement.
///
/// Angular channels (roll, pitch, yaw) use the raw attitude angles for sustained
/// tilt cues, with an optional low-pass filter to smooth rapid changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WashoutConfig {
    /// High-pass corner frequency for translational channels in Hz (default: 0.5 Hz).
    ///
    /// Lower values allow longer motion cues before washing back to center.
    /// Higher values washout quickly. Typical range: 0.2–2.0 Hz.
    pub hp_frequency_hz: f32,

    /// Low-pass corner frequency for angular channels in Hz (default: 5.0 Hz).
    ///
    /// Smooths rapid angular changes. Typical range: 2.0–20.0 Hz.
    pub lp_frequency_hz: f32,
}

impl Default for WashoutConfig {
    fn default() -> Self {
        Self {
            hp_frequency_hz: 0.5,
            lp_frequency_hz: 5.0,
        }
    }
}

/// Top-level motion platform configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionConfig {
    /// Global intensity scale applied to all channels (0.0–1.0, default: 0.8).
    ///
    /// Set to 0 to disable all output without changing individual channel gains.
    pub intensity: f32,

    /// G-force value that maps to full platform excursion on translational channels
    /// (default: 3.0 G). Values above this are clamped.
    pub max_g: f32,

    /// Maximum angular displacement that maps to full platform excursion on tilt
    /// channels, in degrees (default: 30°).
    pub max_angle_deg: f32,

    /// Maximum angular rate that maps to full yaw excursion, in deg/s (default: 60°/s).
    pub max_yaw_rate_deg_s: f32,

    /// Washout filter parameters.
    pub washout: WashoutConfig,

    /// Per-channel configuration.
    pub surge: DoFConfig,
    pub sway: DoFConfig,
    pub heave: DoFConfig,
    pub roll: DoFConfig,
    pub pitch: DoFConfig,
    pub yaw: DoFConfig,
}

impl Default for MotionConfig {
    fn default() -> Self {
        Self {
            intensity: 0.8,
            max_g: 3.0,
            max_angle_deg: 30.0,
            max_yaw_rate_deg_s: 60.0,
            washout: WashoutConfig::default(),
            surge: DoFConfig::default(),
            sway: DoFConfig::default(),
            heave: DoFConfig::default(),
            roll: DoFConfig::default(),
            pitch: DoFConfig::default(),
            yaw: DoFConfig::default(),
        }
    }
}
