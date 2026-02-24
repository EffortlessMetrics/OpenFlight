// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! PC mode detection for Thrustmaster T.Flight HOTAS devices.
//!
//! The T.Flight HOTAS 4 and HOTAS One can operate in two top-level
//! operating modes:
//!
//! - **PC mode** (Green LED): Exposes a full 5-axis HID gamepad with all
//!   axes and buttons available. The device sends 8-byte (merged) or
//!   9-byte (separate) reports depending on the axis mode switch.
//!
//! - **Console mode** (Red LED): Intended for use with PlayStation
//!   consoles. The HID descriptor is simplified; most axes are absent or
//!   remapped. Reports are typically 5 bytes or carry a different layout.
//!
//! # Entering PC Mode
//!
//! The device must be placed in PC mode **before** OpenFlight can receive
//! full axis data. There are two methods:
//!
//! 1. **Base mode switch**: Use the physical PC/PS switch on the throttle
//!    unit before plugging in via USB.
//! 2. **"Secret handshake"**: Hold **Share + Options + PS** (HOTAS 4) or
//!    **View + Menu + Logo** (HOTAS One) while inserting the USB cable.
//!    The LED turns **green** confirming PC mode.
//!
//! # Detection
//!
//! [`PcModeDetector`] classifies each incoming HID report by its byte
//! count and assigns a [`PcModeStatus`]:
//!
//! | Report length | Interpretation              |
//! |---------------|-----------------------------|
//! | ≥ 8 bytes     | `PcMode` (full axis layout) |
//! | < 8 bytes     | `ConsoleMode`               |
//! | No reports    | `Unknown`                   |
//!
//! A configurable number of **consecutive** same-status reports must be
//! seen before the status is promoted from `Unknown`. This avoids
//! flip-flopping on the first few reports when a device is first plugged
//! in.

/// Minimum report length that indicates PC-mode HID layout.
pub const PC_MODE_MIN_REPORT_LEN: usize = 8;

/// Instructions to present to users when the device appears to be in
/// console mode.
pub const PC_MODE_HANDSHAKE_INSTRUCTIONS: &str = "\
The T.Flight HOTAS appears to be in Console (PS) mode (Red LED). \
To enable full PC axis access:\n\
\n\
Method 1 — Hardware switch:\n\
  Disconnect USB, set the PC/PS selector switch on the throttle unit to 'PC', \
then reconnect.\n\
\n\
Method 2 — Secret handshake:\n\
  While inserting the USB cable, hold Share + Options + PS (HOTAS 4) \
or View + Menu + Logo (HOTAS One). Release after the LED turns Green.\n\
\n\
Without PC mode, throttle, twist and rocker axes may be unavailable.";

/// Classification of the device's current operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PcModeStatus {
    /// No reports have been received yet or the status is indeterminate.
    #[default]
    Unknown,
    /// Device is in PC mode — full axis layout available.
    PcMode,
    /// Device appears to be in console (PS) mode — limited axis layout.
    ConsoleMode,
}

impl std::fmt::Display for PcModeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => f.write_str("Unknown"),
            Self::PcMode => f.write_str("PC mode (Green LED)"),
            Self::ConsoleMode => f.write_str("Console mode (Red LED)"),
        }
    }
}

/// Detects whether a T.Flight HOTAS is in PC or Console mode by
/// inspecting the length of incoming HID reports.
///
/// # Example
///
/// ```
/// use flight_hotas_thrustmaster::pc_mode::{PcModeDetector, PcModeStatus};
///
/// // Use confirm_count=1 for immediate classification in this example
/// let mut detector = PcModeDetector::with_confirm_count(1);
///
/// let status = detector.update(&[0u8; 8]);
/// assert_eq!(status, PcModeStatus::PcMode);
/// ```
#[derive(Debug, Clone)]
pub struct PcModeDetector {
    /// Current committed status (only updated after `confirm_count` consistent reports).
    committed: PcModeStatus,
    /// Candidate status being accumulated.
    candidate: PcModeStatus,
    /// Count of consecutive reports matching the current candidate.
    run_length: u32,
    /// How many consecutive matching reports are required to commit.
    confirm_count: u32,
}

impl Default for PcModeDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PcModeDetector {
    /// Default number of consecutive same-type reports required to
    /// confirm a mode transition.
    pub const DEFAULT_CONFIRM_COUNT: u32 = 3;

    /// Create a new detector with default confirmation threshold.
    pub fn new() -> Self {
        Self::with_confirm_count(Self::DEFAULT_CONFIRM_COUNT)
    }

    /// Create a detector that commits after `confirm_count` consecutive
    /// same-type reports.
    pub fn with_confirm_count(confirm_count: u32) -> Self {
        Self {
            committed: PcModeStatus::Unknown,
            candidate: PcModeStatus::Unknown,
            run_length: 0,
            confirm_count,
        }
    }

    /// Process a single HID report and return the current committed status.
    ///
    /// The returned value is the **committed** (stable) status, not the
    /// raw classification of this individual report.
    pub fn update(&mut self, report: &[u8]) -> PcModeStatus {
        let observed = classify_report(report);

        if observed == self.candidate {
            self.run_length += 1;
        } else {
            self.candidate = observed;
            self.run_length = 1;
        }

        if self.run_length >= self.confirm_count {
            self.committed = self.candidate;
        }

        self.committed
    }

    /// Return the current committed status without processing a report.
    pub fn status(&self) -> PcModeStatus {
        self.committed
    }

    /// Return `true` if the device is confirmed to be in PC mode.
    pub fn is_pc_mode(&self) -> bool {
        self.committed == PcModeStatus::PcMode
    }

    /// Return `true` if the device appears to be in console (PS) mode.
    pub fn is_console_mode(&self) -> bool {
        self.committed == PcModeStatus::ConsoleMode
    }

    /// Reset all accumulated state. Next `update()` starts fresh.
    pub fn reset(&mut self) {
        self.committed = PcModeStatus::Unknown;
        self.candidate = PcModeStatus::Unknown;
        self.run_length = 0;
    }

    /// Return user-facing guidance string when in console mode.
    ///
    /// Returns `Some(&str)` when the device is confirmed to be in console
    /// mode, `None` otherwise.
    pub fn console_mode_guidance(&self) -> Option<&'static str> {
        if self.is_console_mode() {
            Some(PC_MODE_HANDSHAKE_INSTRUCTIONS)
        } else {
            None
        }
    }
}

/// Classify a single report by length alone.
fn classify_report(report: &[u8]) -> PcModeStatus {
    if report.len() >= PC_MODE_MIN_REPORT_LEN {
        PcModeStatus::PcMode
    } else {
        PcModeStatus::ConsoleMode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_pc_mode_8_bytes() {
        assert_eq!(classify_report(&[0u8; 8]), PcModeStatus::PcMode);
    }

    #[test]
    fn test_classify_pc_mode_9_bytes() {
        assert_eq!(classify_report(&[0u8; 9]), PcModeStatus::PcMode);
    }

    #[test]
    fn test_classify_console_mode_5_bytes() {
        assert_eq!(classify_report(&[0u8; 5]), PcModeStatus::ConsoleMode);
    }

    #[test]
    fn test_classify_console_mode_empty() {
        assert_eq!(classify_report(&[]), PcModeStatus::ConsoleMode);
    }

    #[test]
    fn test_detector_default_unknown() {
        let detector = PcModeDetector::new();
        assert_eq!(detector.status(), PcModeStatus::Unknown);
    }

    #[test]
    fn test_detector_commits_after_confirm_count() {
        let mut d = PcModeDetector::with_confirm_count(3);
        // Two reports not enough
        assert_eq!(d.update(&[0u8; 8]), PcModeStatus::Unknown);
        assert_eq!(d.update(&[0u8; 8]), PcModeStatus::Unknown);
        // Third commits
        assert_eq!(d.update(&[0u8; 8]), PcModeStatus::PcMode);
        assert!(d.is_pc_mode());
    }

    #[test]
    fn test_detector_confirms_console_mode() {
        let mut d = PcModeDetector::with_confirm_count(2);
        d.update(&[0u8; 5]);
        d.update(&[0u8; 5]);
        assert_eq!(d.status(), PcModeStatus::ConsoleMode);
        assert!(d.is_console_mode());
    }

    #[test]
    fn test_detector_resets_run_on_type_change() {
        let mut d = PcModeDetector::with_confirm_count(3);
        d.update(&[0u8; 8]);
        d.update(&[0u8; 8]);
        // Console report interrupts run
        d.update(&[0u8; 5]);
        // Still Unknown — PC run was reset
        assert_eq!(d.status(), PcModeStatus::Unknown);
    }

    #[test]
    fn test_detector_reset_clears_state() {
        let mut d = PcModeDetector::with_confirm_count(1);
        d.update(&[0u8; 8]);
        assert_eq!(d.status(), PcModeStatus::PcMode);
        d.reset();
        assert_eq!(d.status(), PcModeStatus::Unknown);
    }

    #[test]
    fn test_console_mode_guidance_present_when_console() {
        let mut d = PcModeDetector::with_confirm_count(1);
        d.update(&[0u8; 5]);
        let guidance = d.console_mode_guidance();
        assert!(guidance.is_some());
        let text = guidance.unwrap();
        assert!(text.contains("Share"));
        assert!(text.contains("throttle"));
    }

    #[test]
    fn test_console_mode_guidance_absent_when_pc() {
        let mut d = PcModeDetector::with_confirm_count(1);
        d.update(&[0u8; 8]);
        assert!(d.console_mode_guidance().is_none());
    }

    #[test]
    fn test_display_formatting() {
        assert_eq!(PcModeStatus::PcMode.to_string(), "PC mode (Green LED)");
        assert_eq!(PcModeStatus::ConsoleMode.to_string(), "Console mode (Red LED)");
        assert_eq!(PcModeStatus::Unknown.to_string(), "Unknown");
    }
}
