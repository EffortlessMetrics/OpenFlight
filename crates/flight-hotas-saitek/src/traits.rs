// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Protocol traits for HOTAS output devices.
//!
//! These traits define the interface for MFD, LED, and RGB control.
//! Implementations are feature-gated and experimental until protocols are verified.

use thiserror::Error;

/// Result type for HOTAS operations.
pub type HotasResult<T> = Result<T, HotasError>;

/// Errors that can occur during HOTAS operations.
#[derive(Debug, Error)]
pub enum HotasError {
    /// Device not found or disconnected.
    #[error("device not found: {0}")]
    DeviceNotFound(String),

    /// USB communication error.
    #[error("USB error: {0}")]
    UsbError(String),

    /// Protocol is unverified and operation failed.
    #[error("unverified protocol '{0}' failed - see docs/reference/hotas-claims.md")]
    UnverifiedProtocol(&'static str),

    /// Feature not supported by this device.
    #[error("feature not supported: {0}")]
    NotSupported(String),

    /// Invalid parameter value.
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),
}

/// LED identifier for X52/X52 Pro devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LedId {
    /// Fire button LED (trigger)
    Fire,
    /// A button LED
    ButtonA,
    /// B button LED
    ButtonB,
    /// D button LED (if present)
    ButtonD,
    /// E button LED (if present)
    ButtonE,
    /// T1 toggle LED
    Toggle1,
    /// T2 toggle LED
    Toggle2,
    /// T3 toggle LED
    Toggle3,
    /// POV2 LED
    Pov2,
    /// Clutch LED (i button)
    Clutch,
    /// Throttle LED
    Throttle,
}

/// LED state for X52 devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedState {
    Off,
    Green,
    Amber,
    Red,
}

/// RGB color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const BLACK: Self = Self::new(0, 0, 0);
    pub const WHITE: Self = Self::new(255, 255, 255);
    pub const RED: Self = Self::new(255, 0, 0);
    pub const GREEN: Self = Self::new(0, 255, 0);
    pub const BLUE: Self = Self::new(0, 0, 255);
}

/// RGB zone identifier for X56 devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RgbZone {
    /// Stick base lighting
    StickBase,
    /// Stick grip lighting
    StickGrip,
    /// Throttle base lighting
    ThrottleBase,
    /// Throttle grip lighting
    ThrottleGrip,
}

/// MFD display protocol for X52 Pro.
///
/// # Protocol Status
///
/// **UNVERIFIED** - This protocol is based on community documentation and has not
/// been verified via USB capture. Enable `x52-mfd-experimental` feature to use.
pub trait MfdProtocol: Send + Sync {
    /// Set text on a display line (0-2).
    ///
    /// Text will be truncated to fit the display width (typically 16 characters).
    fn set_line(&mut self, line: u8, text: &str) -> HotasResult<()>;

    /// Set display brightness (0-127).
    fn set_brightness(&mut self, level: u8) -> HotasResult<()>;

    /// Clear all display lines.
    fn clear(&mut self) -> HotasResult<()>;
}

/// LED control protocol for X52/X52 Pro.
///
/// # Protocol Status
///
/// **UNVERIFIED** - Enable `x52-led-experimental` feature to use.
pub trait LedProtocol: Send + Sync {
    /// Set the state of an individual LED.
    fn set_led(&mut self, led: LedId, state: LedState) -> HotasResult<()>;

    /// Set global LED brightness (0-127).
    fn set_global_brightness(&mut self, level: u8) -> HotasResult<()>;
}

/// RGB lighting protocol for X56.
///
/// # Protocol Status
///
/// **UNVERIFIED** - Enable `x56-rgb-experimental` feature to use.
pub trait RgbProtocol: Send + Sync {
    /// Set color for a specific zone.
    fn set_color(&mut self, zone: RgbZone, color: RgbColor) -> HotasResult<()>;

    /// Set all zones to the same color.
    fn set_all(&mut self, color: RgbColor) -> HotasResult<()>;
}
