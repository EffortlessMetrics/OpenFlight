// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Enhanced Thrustmaster HID protocol layer.
//!
//! Provides a unified device identification table, Warthog button-matrix
//! helpers (including shifted states via the pinkie switch), LED control
//! report generation for the Warthog throttle, and TARGET scripting
//! compatibility notes.
//!
//! # Device identification
//!
//! All Thrustmaster flight peripherals share VID `0x044F`. Use
//! [`identify_device`] to map a `(vid, pid)` pair to a [`ThrustmasterDevice`]
//! variant, or iterate [`DEVICE_TABLE`] for the complete catalogue.
//!
//! # Warthog button matrix
//!
//! The Warthog throttle's S3 pinkie switch acts as a hardware shift key.
//! Physical buttons 1–19 on the stick map to logical buttons 1–19 when
//! unshifted, and 20–38 when shifted (pinkie held). Use
//! [`resolve_shifted_button`] to translate a `(physical_button, pinkie_held)`
//! pair into the logical button number used by DCS / TARGET.
//!
//! # LED control
//!
//! The Warthog throttle backlight can be set via a 2-byte HID output report
//! (report ID `0x01`). [`build_led_report`] constructs this report from a
//! [`LedState`] value.
//!
//! # TARGET scripting compatibility
//!
//! TARGET (Thrustmaster Advanced pRogramming Graphical EdiTor) uses its own
//! logical button numbering. The constants in this module align with the
//! TARGET SDK's `H:` device declarations so that OpenFlight profiles can
//! reference the same button indices as community TARGET scripts.

use crate::warthog::{WarthogStickButtons, WarthogThrottleButtons};

// ─── Vendor / Product IDs ────────────────────────────────────────────────────

/// Thrustmaster USB Vendor ID (all products).
pub const VENDOR_ID: u16 = 0x044F;

/// Complete catalogue of known Thrustmaster flight-sim USB Product IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThrustmasterDevice {
    // ── T.16000M family ──────────────────────────────────────────────────
    /// T.16000M FCS joystick (HALL sensors, 14-bit axes, twist).
    T16000mJoystick,
    /// TWCS Throttle (mini-stick, rocker, slider).
    TwcsThrottle,

    // ── Warthog family ───────────────────────────────────────────────────
    /// HOTAS Warthog Joystick (metal, X/Y, 19 buttons, hat).
    WarthogJoystick,
    /// HOTAS Warthog Throttle (dual levers, slew, LEDs, many switches).
    WarthogThrottle,

    // ── T.Flight family ──────────────────────────────────────────────────
    /// T.Flight HOTAS X (combined PS3/PC).
    TFlightHotasX,
    /// T.Flight HOTAS 4 (primary PID).
    TFlightHotas4,
    /// T.Flight HOTAS 4 (legacy PID, older firmware).
    TFlightHotas4Legacy,
    /// T.Flight HOTAS 4 v2 (newer firmware).
    TFlightHotas4V2,
    /// T.Flight HOTAS One (Xbox/PC, interrupt mode).
    TFlightHotasOne,
    /// T.Flight HOTAS One (bulk endpoint variant).
    TFlightHotasOneBulk,
    /// T.Flight Stick X (standalone joystick).
    TFlightStickX,
    /// T.Flight Stick X v2 (alternate firmware).
    TFlightStickXV2,

    // ── Pedals ───────────────────────────────────────────────────────────
    /// T.Flight Rudder Pedals (TFRP).
    TfrpRudderPedals,
    /// T-Rudder pedals.
    TRudder,
    /// T-Pendular Rudder (TPR) standard.
    TprPendular,
    /// T-Pendular Rudder (TPR) bulk variant.
    TprPendularBulk,

    // ── Legacy ───────────────────────────────────────────────────────────
    /// HOTAS Cougar (F-16 replica, combined stick + throttle).
    HotasCougar,

    // ── TCA Boeing ───────────────────────────────────────────────────────
    /// TCA Yoke Boeing Edition.
    TcaYokeBoeing,
    /// TCA Quadrant Boeing Edition (Engines 1 & 2).
    TcaQuadrantBoeingEng12,
    /// TCA Quadrant Boeing Edition (Engines 3 & 4).
    TcaQuadrantBoeingEng34,
}

impl ThrustmasterDevice {
    /// Human-readable product name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::T16000mJoystick => "T.16000M FCS Joystick",
            Self::TwcsThrottle => "TWCS Throttle",
            Self::WarthogJoystick => "HOTAS Warthog Joystick",
            Self::WarthogThrottle => "HOTAS Warthog Throttle",
            Self::TFlightHotasX => "T.Flight HOTAS X",
            Self::TFlightHotas4 => "T.Flight HOTAS 4",
            Self::TFlightHotas4Legacy => "T.Flight HOTAS 4 (legacy)",
            Self::TFlightHotas4V2 => "T.Flight HOTAS 4 v2",
            Self::TFlightHotasOne => "T.Flight HOTAS One",
            Self::TFlightHotasOneBulk => "T.Flight HOTAS One (bulk)",
            Self::TFlightStickX => "T.Flight Stick X",
            Self::TFlightStickXV2 => "T.Flight Stick X v2",
            Self::TfrpRudderPedals => "T.Flight Rudder Pedals (TFRP)",
            Self::TRudder => "T-Rudder",
            Self::TprPendular => "T-Pendular Rudder (TPR)",
            Self::TprPendularBulk => "T-Pendular Rudder (TPR Bulk)",
            Self::HotasCougar => "HOTAS Cougar",
            Self::TcaYokeBoeing => "TCA Yoke Boeing",
            Self::TcaQuadrantBoeingEng12 => "TCA Quadrant Boeing Eng 1&2",
            Self::TcaQuadrantBoeingEng34 => "TCA Quadrant Boeing Eng 3&4",
        }
    }
}

/// A row in the device identification table.
#[derive(Debug, Clone, Copy)]
pub struct DeviceEntry {
    /// USB Product ID.
    pub pid: u16,
    /// Enumerated device variant.
    pub device: ThrustmasterDevice,
}

/// Complete VID/PID lookup table for all known Thrustmaster flight devices.
///
/// All entries share VID [`VENDOR_ID`] (`0x044F`).
pub const DEVICE_TABLE: &[DeviceEntry] = &[
    // T.16000M family
    DeviceEntry {
        pid: 0xB10A,
        device: ThrustmasterDevice::T16000mJoystick,
    },
    DeviceEntry {
        pid: 0xB687,
        device: ThrustmasterDevice::TwcsThrottle,
    },
    // Warthog
    DeviceEntry {
        pid: 0x0402,
        device: ThrustmasterDevice::WarthogJoystick,
    },
    DeviceEntry {
        pid: 0x0404,
        device: ThrustmasterDevice::WarthogThrottle,
    },
    // T.Flight HOTAS
    DeviceEntry {
        pid: 0xB108,
        device: ThrustmasterDevice::TFlightHotasX,
    },
    DeviceEntry {
        pid: 0xB67B,
        device: ThrustmasterDevice::TFlightHotas4,
    },
    DeviceEntry {
        pid: 0xB67A,
        device: ThrustmasterDevice::TFlightHotas4Legacy,
    },
    DeviceEntry {
        pid: 0xB67C,
        device: ThrustmasterDevice::TFlightHotas4V2,
    },
    DeviceEntry {
        pid: 0xB68D,
        device: ThrustmasterDevice::TFlightHotasOne,
    },
    DeviceEntry {
        pid: 0xB68B,
        device: ThrustmasterDevice::TFlightHotasOneBulk,
    },
    // T.Flight Stick X
    DeviceEntry {
        pid: 0xB106,
        device: ThrustmasterDevice::TFlightStickX,
    },
    DeviceEntry {
        pid: 0xB107,
        device: ThrustmasterDevice::TFlightStickXV2,
    },
    // Pedals
    DeviceEntry {
        pid: 0xB678,
        device: ThrustmasterDevice::TfrpRudderPedals,
    },
    DeviceEntry {
        pid: 0xB679,
        device: ThrustmasterDevice::TRudder,
    },
    DeviceEntry {
        pid: 0xB68F,
        device: ThrustmasterDevice::TprPendular,
    },
    DeviceEntry {
        pid: 0xB68E,
        device: ThrustmasterDevice::TprPendularBulk,
    },
    // Legacy
    DeviceEntry {
        pid: 0x0400,
        device: ThrustmasterDevice::HotasCougar,
    },
    // TCA Boeing
    DeviceEntry {
        pid: 0xB68C,
        device: ThrustmasterDevice::TcaYokeBoeing,
    },
    DeviceEntry {
        pid: 0xB694,
        device: ThrustmasterDevice::TcaQuadrantBoeingEng12,
    },
    DeviceEntry {
        pid: 0xB695,
        device: ThrustmasterDevice::TcaQuadrantBoeingEng34,
    },
];

/// Identify a Thrustmaster device by USB VID/PID.
///
/// Returns `None` if the VID is not `0x044F` or the PID is unknown.
pub fn identify_device(vendor_id: u16, product_id: u16) -> Option<ThrustmasterDevice> {
    if vendor_id != VENDOR_ID {
        return None;
    }
    DEVICE_TABLE
        .iter()
        .find(|e| e.pid == product_id)
        .map(|e| e.device)
}

// ─── Warthog button matrix ──────────────────────────────────────────────────

/// Number of physical buttons on the Warthog joystick (1-indexed: 1–19).
pub const WARTHOG_STICK_PHYSICAL_BUTTONS: u8 = 19;

/// Number of physical buttons on the Warthog throttle (1-indexed: 1–40).
pub const WARTHOG_THROTTLE_PHYSICAL_BUTTONS: u8 = 40;

/// The pinkie switch on the Warthog stick is physical button 2 (S3).
///
/// In the TARGET SDK, holding S3 shifts all other buttons into a second
/// layer (logical buttons 20–38 for stick buttons 1, 3–19).
pub const WARTHOG_PINKIE_BUTTON: u8 = 2;

/// Resolve a physical stick button to a logical button index, accounting for
/// the pinkie shift layer.
///
/// When `pinkie_held` is `false`, physical buttons 1–19 map to logical 1–19.
/// When `pinkie_held` is `true`:
///   - The pinkie button itself (2) stays at logical 2.
///   - Physical button 1 maps to logical 20.
///   - Physical buttons 3–19 map to logical 21–37.
///
/// Returns `None` for physical buttons outside 1–19.
pub fn resolve_shifted_button(physical: u8, pinkie_held: bool) -> Option<u8> {
    if physical == 0 || physical > WARTHOG_STICK_PHYSICAL_BUTTONS {
        return None;
    }
    if !pinkie_held {
        return Some(physical);
    }
    // Pinkie button itself is not shifted.
    if physical == WARTHOG_PINKIE_BUTTON {
        return Some(physical);
    }
    // Shift layer: physical 1 → logical 20, physical 3 → 21, …, 19 → 37.
    if physical < WARTHOG_PINKIE_BUTTON {
        Some(physical + 19) // 1 → 20
    } else {
        Some(physical + 18) // 3→21, 4→22, …, 19→37
    }
}

/// Query the pinkie-shift state from a parsed Warthog stick report.
///
/// Returns `true` if the pinkie switch (S3, button 2) is currently held.
pub fn is_pinkie_held(buttons: &WarthogStickButtons) -> bool {
    buttons.button(WARTHOG_PINKIE_BUTTON)
}

// ─── Warthog throttle split / merge detection ───────────────────────────────

/// Tolerance for detecting split vs. merged throttle (normalised units).
const SPLIT_THRESHOLD: f32 = 0.02;

/// Returns `true` if the Warthog throttle levers are in "split" mode
/// (significant difference between left and right positions).
///
/// The combined axis tracks the average when merged; when the physical
/// interlock is disengaged the left and right axes diverge.
pub fn is_throttle_split(left: f32, right: f32) -> bool {
    (left - right).abs() > SPLIT_THRESHOLD
}

// ─── Warthog Throttle LED control ───────────────────────────────────────────

/// LED backlight brightness / state for the Warthog throttle.
///
/// The Warthog throttle has a backlight LED that can be controlled via a
/// 2-byte HID output report (Report ID `0x01`).
///
/// The second byte is the brightness level:
/// - `0x00` — off
/// - `0x01`–`0x05` — increasing brightness levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedState {
    Off,
    /// Brightness level 1–5 (clamped).
    Brightness(u8),
}

impl LedState {
    /// Maximum brightness level.
    pub const MAX_BRIGHTNESS: u8 = 5;

    fn to_byte(self) -> u8 {
        match self {
            Self::Off => 0x00,
            Self::Brightness(v) => v.min(Self::MAX_BRIGHTNESS),
        }
    }
}

/// HID output report ID used for Warthog throttle LED control.
pub const WARTHOG_LED_REPORT_ID: u8 = 0x01;

/// Build a 2-byte HID output report to set the Warthog throttle backlight.
///
/// The returned array can be sent directly via `hid_write()`.
///
/// ```
/// use flight_hotas_thrustmaster::protocol::{LedState, build_led_report, WARTHOG_LED_REPORT_ID};
///
/// let report = build_led_report(LedState::Brightness(3));
/// assert_eq!(report[0], WARTHOG_LED_REPORT_ID);
/// assert_eq!(report[1], 3);
/// ```
pub fn build_led_report(state: LedState) -> [u8; 2] {
    [WARTHOG_LED_REPORT_ID, state.to_byte()]
}

// ─── Warthog Throttle toggle-switch helpers ─────────────────────────────────

/// Bitmask positions for Warthog throttle toggle switches (byte 15).
///
/// These are physical two- or three-position toggle switches on the
/// throttle base. Their state is reported as a bitmask in the toggles
/// field of [`WarthogThrottleButtons`].
pub mod toggles {
    /// Engine-fuel flow left — NORM position (bit 0).
    pub const EFL_NORM: u8 = 0;
    /// Engine-fuel flow right — NORM position (bit 1).
    pub const EFR_NORM: u8 = 1;
    /// Engine oper left — NORM position (bit 2).
    pub const EOL_NORM: u8 = 2;
    /// Engine oper right — NORM position (bit 3).
    pub const EOR_NORM: u8 = 3;
    /// APU start switch (bit 4).
    pub const APU_START: u8 = 4;
    /// Landing-gear horn silence (bit 5).
    pub const LGSIL: u8 = 5;
    /// Speed brake — forward position (bit 6).
    pub const SPDF: u8 = 6;
    /// Speed brake — back position (bit 7).
    pub const SPDB: u8 = 7;

    /// Returns `true` if the given toggle bit is set in the raw bitmask.
    pub fn is_set(toggles: u8, bit: u8) -> bool {
        bit < 8 && (toggles >> bit) & 1 != 0
    }
}

/// Determine whether a given toggle switch is active in the throttle state.
pub fn is_toggle_active(buttons: &WarthogThrottleButtons, bit: u8) -> bool {
    toggles::is_set(buttons.toggles, bit)
}

// ─── TARGET compatibility notes ─────────────────────────────────────────────

/// Notes on TARGET scripting compatibility.
///
/// TARGET (Thrustmaster Advanced pRogramming Graphical EdiTor) is the
/// official programming tool for Thrustmaster devices. Key compatibility
/// points:
///
/// 1. **Button numbering**: TARGET uses 1-indexed button numbers matching
///    the HID report bitmask order. OpenFlight's `button(n)` method uses
///    the same 1-indexed convention.
///
/// 2. **Shift layers**: TARGET's `EXEC` / `SEQ` / `TEMPO` macros create
///    virtual shifted layers. OpenFlight's [`resolve_shifted_button`]
///    implements the most common shift pattern (pinkie S3 shift) for the
///    Warthog stick, producing logical button numbers 20–37 that match
///    TARGET's default shift-map output.
///
/// 3. **Axis mapping**: TARGET axis IDs (JOYX, JOYY, RUDDER, THR_LEFT,
///    THR_RIGHT, SCX, SCY, etc.) correspond directly to the axis fields
///    in OpenFlight's parsed state structs.
///
/// 4. **LED control**: TARGET's `led(&Throttle, LED_ONOFF, LED_CURRENT)`
///    sends the same 2-byte output report that [`build_led_report`]
///    generates.
///
/// 5. **Device handle naming**: TARGET uses `&Joystick` for the Warthog
///    stick and `&Throttle` for the throttle, matching the
///    [`ThrustmasterDevice::WarthogJoystick`] / `WarthogThrottle` variants.
pub const TARGET_COMPAT_NOTES: &str = "\
TARGET button numbers are 1-indexed and match OpenFlight's button(n) convention. \
The Warthog pinkie-shift layer produces logical buttons 20-37, matching TARGET defaults. \
Axis IDs (JOYX, JOYY, THR_LEFT, etc.) map directly to parsed struct fields.";

// ─── Unified stick / throttle state ─────────────────────────────────────────

/// Unified stick input state across all Thrustmaster stick devices.
///
/// Axes are normalized to f64:
///   - Centered axes: −1.0 (full left / forward) to 1.0 (full right / back)
///   - Unipolar axes: 0.0 (idle) to 1.0 (full)
#[derive(Debug, Clone)]
pub struct StickState {
    /// Roll axis (−1.0 = left, 1.0 = right).
    pub roll: f64,
    /// Pitch axis (−1.0 = forward/up, 1.0 = back/down).
    pub pitch: f64,
    /// Twist / rudder axis (−1.0 = left, 1.0 = right). `0.0` if the stick has no twist.
    pub twist: f64,
    /// Throttle slider (0.0 = idle, 1.0 = full). `0.0` if no slider present.
    pub throttle_slider: f64,
    /// Button states, indexed 0..N (button 1 = index 0).
    pub buttons: Vec<bool>,
    /// Hat switch position: 0 = center, 1 = N, 2 = NE, 3 = E, … 8 = NW.
    pub hat: u8,
}

/// Unified throttle input state across all Thrustmaster throttle devices.
#[derive(Debug, Clone)]
pub struct ThrottleState {
    /// Primary / left throttle lever (0.0 = idle, 1.0 = full).
    pub throttle_left: f64,
    /// Right throttle lever (0.0 = idle, 1.0 = full). Same as `throttle_left` for single-lever devices.
    pub throttle_right: f64,
    /// Combined / interlock throttle (0.0 = idle, 1.0 = full).
    pub throttle_combined: f64,
    /// Mini-stick / slew X (−1.0 … 1.0). `0.0` if not present.
    pub slew_x: f64,
    /// Mini-stick / slew Y (−1.0 … 1.0). `0.0` if not present.
    pub slew_y: f64,
    /// Rocker axis (−1.0 … 1.0). `0.0` if not present.
    pub rocker: f64,
    /// Button states, indexed 0..N (button 1 = index 0).
    pub buttons: Vec<bool>,
    /// Engine detent indicators (idle, afterburner, reverse), if detectable.
    pub detent_positions: Vec<f64>,
}

/// Errors from the unified protocol parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThrustmasterProtocolError {
    /// The device has not been identified yet.
    UnknownDevice,
    /// Report too short for the expected device.
    ReportTooShort { expected: usize, actual: usize },
    /// Parsing the underlying device report failed.
    ParseError(String),
}

impl std::fmt::Display for ThrustmasterProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownDevice => write!(f, "unknown Thrustmaster device"),
            Self::ReportTooShort { expected, actual } => {
                write!(f, "report too short: expected {expected}, got {actual}")
            }
            Self::ParseError(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for ThrustmasterProtocolError {}

/// Device-aware Thrustmaster HID protocol handler.
///
/// Wraps device identification and dispatches raw HID reports to the
/// appropriate device-specific parser, returning a unified [`StickState`]
/// or [`ThrottleState`].
///
/// # Example
///
/// ```
/// use flight_hotas_thrustmaster::protocol::ThrustmasterProtocol;
///
/// let proto = ThrustmasterProtocol::new(0x044F, 0x0402); // Warthog stick
/// assert!(proto.device().is_some());
/// ```
pub struct ThrustmasterProtocol {
    device: Option<ThrustmasterDevice>,
}

impl ThrustmasterProtocol {
    /// Create a protocol handler for the given VID/PID.
    pub fn new(vendor_id: u16, product_id: u16) -> Self {
        Self {
            device: identify_device(vendor_id, product_id),
        }
    }

    /// The identified device, if any.
    pub fn device(&self) -> Option<ThrustmasterDevice> {
        self.device
    }

    /// Parse a raw HID report as a stick report.
    ///
    /// Dispatches to the correct parser based on the identified device.
    pub fn parse_stick_report(&self, data: &[u8]) -> Result<StickState, ThrustmasterProtocolError> {
        let device = self
            .device
            .ok_or(ThrustmasterProtocolError::UnknownDevice)?;
        parse_stick_report(data, device)
    }

    /// Parse a raw HID report as a throttle report.
    ///
    /// Dispatches to the correct parser based on the identified device.
    pub fn parse_throttle_report(
        &self,
        data: &[u8],
    ) -> Result<ThrottleState, ThrustmasterProtocolError> {
        let device = self
            .device
            .ok_or(ThrustmasterProtocolError::UnknownDevice)?;
        parse_throttle_report(data, device)
    }
}

/// Parse a stick HID report for a known device into a unified [`StickState`].
pub fn parse_stick_report(
    data: &[u8],
    device: ThrustmasterDevice,
) -> Result<StickState, ThrustmasterProtocolError> {
    match device {
        ThrustmasterDevice::WarthogJoystick => {
            let payload = strip_report_id(data, crate::warthog::WARTHOG_STICK_MIN_REPORT_BYTES);
            let state = crate::warthog::parse_warthog_stick(payload)
                .map_err(|e| ThrustmasterProtocolError::ParseError(e.to_string()))?;
            let mut buttons = Vec::with_capacity(19);
            for i in 1..=19 {
                buttons.push(state.buttons.button(i));
            }
            Ok(StickState {
                roll: state.axes.x as f64,
                pitch: state.axes.y as f64,
                twist: state.axes.rz as f64,
                throttle_slider: 0.0,
                buttons,
                hat: warthog_hat_to_u8(state.buttons.hat),
            })
        }
        ThrustmasterDevice::T16000mJoystick => {
            let state = crate::t16000m::parse_t16000m_report(data)
                .map_err(|e| ThrustmasterProtocolError::ParseError(e.to_string()))?;
            let mut buttons = Vec::with_capacity(16);
            for i in 0..16 {
                buttons.push((state.buttons.buttons >> i) & 1 != 0);
            }
            Ok(StickState {
                roll: state.axes.x as f64,
                pitch: state.axes.y as f64,
                twist: state.axes.twist as f64,
                throttle_slider: state.axes.throttle as f64,
                buttons,
                hat: state.buttons.hat,
            })
        }
        ThrustmasterDevice::HotasCougar => {
            let payload = strip_report_id(data, crate::cougar::COUGAR_MIN_REPORT_BYTES);
            let state = crate::cougar::parse_cougar(payload)
                .map_err(|e| ThrustmasterProtocolError::ParseError(e.to_string()))?;
            let mut buttons = Vec::with_capacity(16);
            for i in 0..16 {
                buttons.push(state.buttons.button(i + 1));
            }
            Ok(StickState {
                roll: state.axes.x as f64,
                pitch: state.axes.y as f64,
                twist: 0.0,
                throttle_slider: state.axes.throttle as f64,
                buttons,
                hat: cougar_hat_to_u8(state.buttons.tms_hat),
            })
        }
        _ => Err(ThrustmasterProtocolError::ParseError(format!(
            "{} is not a stick device",
            device.name()
        ))),
    }
}

/// Parse a throttle HID report for a known device into a unified [`ThrottleState`].
pub fn parse_throttle_report(
    data: &[u8],
    device: ThrustmasterDevice,
) -> Result<ThrottleState, ThrustmasterProtocolError> {
    match device {
        ThrustmasterDevice::WarthogThrottle => {
            let payload = strip_report_id(data, crate::warthog::WARTHOG_THROTTLE_MIN_REPORT_BYTES);
            let state = crate::warthog::parse_warthog_throttle(payload)
                .map_err(|e| ThrustmasterProtocolError::ParseError(e.to_string()))?;
            let mut buttons = Vec::with_capacity(40);
            for i in 1..=40 {
                buttons.push(state.buttons.button(i));
            }
            Ok(ThrottleState {
                throttle_left: state.axes.throttle_left as f64,
                throttle_right: state.axes.throttle_right as f64,
                throttle_combined: state.axes.throttle_combined as f64,
                slew_x: state.axes.slew_x as f64,
                slew_y: state.axes.slew_y as f64,
                rocker: 0.0,
                buttons,
                detent_positions: Vec::new(),
            })
        }
        ThrustmasterDevice::TwcsThrottle => {
            let state = crate::t16000m::parse_twcs_report(data)
                .map_err(|e| ThrustmasterProtocolError::ParseError(e.to_string()))?;
            let mut buttons = Vec::with_capacity(14);
            for i in 0..14 {
                buttons.push((state.buttons.buttons >> i) & 1 != 0);
            }
            Ok(ThrottleState {
                throttle_left: state.axes.throttle as f64,
                throttle_right: state.axes.throttle as f64,
                throttle_combined: state.axes.throttle as f64,
                slew_x: state.axes.mini_stick_x as f64,
                slew_y: state.axes.mini_stick_y as f64,
                rocker: state.axes.rocker as f64,
                buttons,
                detent_positions: Vec::new(),
            })
        }
        _ => Err(ThrustmasterProtocolError::ParseError(format!(
            "{} is not a throttle device",
            device.name()
        ))),
    }
}

/// Strip a leading report-ID byte when the buffer is exactly one byte
/// longer than the expected payload. Returns the payload slice unchanged
/// when its length already matches or exceeds `min_payload_len`.
fn strip_report_id(data: &[u8], min_payload_len: usize) -> &[u8] {
    if data.len() == min_payload_len + 1 {
        &data[1..]
    } else {
        data
    }
}

/// Convert a [`WarthogHat`] to the standard 0–8 encoding.
fn warthog_hat_to_u8(hat: crate::warthog::WarthogHat) -> u8 {
    use crate::warthog::WarthogHat;
    match hat {
        WarthogHat::Center => 0,
        WarthogHat::North => 1,
        WarthogHat::NorthEast => 2,
        WarthogHat::East => 3,
        WarthogHat::SouthEast => 4,
        WarthogHat::South => 5,
        WarthogHat::SouthWest => 6,
        WarthogHat::West => 7,
        WarthogHat::NorthWest => 8,
    }
}

/// Convert a [`CougarHat`] to the standard 0–8 encoding.
fn cougar_hat_to_u8(hat: crate::cougar::CougarHat) -> u8 {
    use crate::cougar::CougarHat;
    match hat {
        CougarHat::Center => 0,
        CougarHat::North => 1,
        CougarHat::NorthEast => 2,
        CougarHat::East => 3,
        CougarHat::SouthEast => 4,
        CougarHat::South => 5,
        CougarHat::SouthWest => 6,
        CougarHat::West => 7,
        CougarHat::NorthWest => 8,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::warthog::{WarthogHat, WarthogStickButtons, WarthogThrottleButtons};

    // ── Device identification ────────────────────────────────────────────

    #[test]
    fn identify_t16000m_joystick() {
        assert_eq!(
            identify_device(0x044F, 0xB10A),
            Some(ThrustmasterDevice::T16000mJoystick)
        );
    }

    #[test]
    fn identify_twcs_throttle() {
        assert_eq!(
            identify_device(0x044F, 0xB687),
            Some(ThrustmasterDevice::TwcsThrottle)
        );
    }

    #[test]
    fn identify_warthog_joystick() {
        assert_eq!(
            identify_device(0x044F, 0x0402),
            Some(ThrustmasterDevice::WarthogJoystick)
        );
    }

    #[test]
    fn identify_warthog_throttle() {
        assert_eq!(
            identify_device(0x044F, 0x0404),
            Some(ThrustmasterDevice::WarthogThrottle)
        );
    }

    #[test]
    fn identify_tflight_hotas_x() {
        assert_eq!(
            identify_device(0x044F, 0xB108),
            Some(ThrustmasterDevice::TFlightHotasX)
        );
    }

    #[test]
    fn identify_tflight_hotas4_all_variants() {
        assert_eq!(
            identify_device(0x044F, 0xB67B),
            Some(ThrustmasterDevice::TFlightHotas4)
        );
        assert_eq!(
            identify_device(0x044F, 0xB67A),
            Some(ThrustmasterDevice::TFlightHotas4Legacy)
        );
        assert_eq!(
            identify_device(0x044F, 0xB67C),
            Some(ThrustmasterDevice::TFlightHotas4V2)
        );
    }

    #[test]
    fn identify_tflight_hotas_one() {
        assert_eq!(
            identify_device(0x044F, 0xB68D),
            Some(ThrustmasterDevice::TFlightHotasOne)
        );
        assert_eq!(
            identify_device(0x044F, 0xB68B),
            Some(ThrustmasterDevice::TFlightHotasOneBulk)
        );
    }

    #[test]
    fn identify_tflight_stick_x() {
        assert_eq!(
            identify_device(0x044F, 0xB106),
            Some(ThrustmasterDevice::TFlightStickX)
        );
        assert_eq!(
            identify_device(0x044F, 0xB107),
            Some(ThrustmasterDevice::TFlightStickXV2)
        );
    }

    #[test]
    fn identify_tfrp_pedals() {
        assert_eq!(
            identify_device(0x044F, 0xB678),
            Some(ThrustmasterDevice::TfrpRudderPedals)
        );
    }

    #[test]
    fn identify_t_rudder() {
        assert_eq!(
            identify_device(0x044F, 0xB679),
            Some(ThrustmasterDevice::TRudder)
        );
    }

    #[test]
    fn identify_tpr_pendular() {
        assert_eq!(
            identify_device(0x044F, 0xB68F),
            Some(ThrustmasterDevice::TprPendular)
        );
        assert_eq!(
            identify_device(0x044F, 0xB68E),
            Some(ThrustmasterDevice::TprPendularBulk)
        );
    }

    #[test]
    fn identify_cougar() {
        assert_eq!(
            identify_device(0x044F, 0x0400),
            Some(ThrustmasterDevice::HotasCougar)
        );
    }

    #[test]
    fn identify_tca_boeing_family() {
        assert_eq!(
            identify_device(0x044F, 0xB68C),
            Some(ThrustmasterDevice::TcaYokeBoeing)
        );
        assert_eq!(
            identify_device(0x044F, 0xB694),
            Some(ThrustmasterDevice::TcaQuadrantBoeingEng12)
        );
        assert_eq!(
            identify_device(0x044F, 0xB695),
            Some(ThrustmasterDevice::TcaQuadrantBoeingEng34)
        );
    }

    #[test]
    fn identify_wrong_vendor() {
        assert_eq!(identify_device(0x1234, 0xB10A), None);
    }

    #[test]
    fn identify_unknown_pid() {
        assert_eq!(identify_device(0x044F, 0xFFFF), None);
    }

    #[test]
    fn device_table_has_no_duplicate_pids() {
        let mut pids: Vec<u16> = DEVICE_TABLE.iter().map(|e| e.pid).collect();
        pids.sort();
        pids.dedup();
        assert_eq!(
            pids.len(),
            DEVICE_TABLE.len(),
            "duplicate PID in DEVICE_TABLE"
        );
    }

    #[test]
    fn device_name_is_nonempty() {
        for entry in DEVICE_TABLE {
            assert!(
                !entry.device.name().is_empty(),
                "empty name for {:?}",
                entry.device
            );
        }
    }

    // ── Shifted button resolution ────────────────────────────────────────

    #[test]
    fn unshifted_buttons_are_identity() {
        for n in 1..=19u8 {
            assert_eq!(resolve_shifted_button(n, false), Some(n));
        }
    }

    #[test]
    fn shifted_pinkie_stays_at_2() {
        assert_eq!(
            resolve_shifted_button(WARTHOG_PINKIE_BUTTON, true),
            Some(WARTHOG_PINKIE_BUTTON)
        );
    }

    #[test]
    fn shifted_button_1_maps_to_20() {
        assert_eq!(resolve_shifted_button(1, true), Some(20));
    }

    #[test]
    fn shifted_button_3_maps_to_21() {
        assert_eq!(resolve_shifted_button(3, true), Some(21));
    }

    #[test]
    fn shifted_button_19_maps_to_37() {
        assert_eq!(resolve_shifted_button(19, true), Some(37));
    }

    #[test]
    fn shifted_out_of_range_returns_none() {
        assert_eq!(resolve_shifted_button(0, true), None);
        assert_eq!(resolve_shifted_button(20, true), None);
        assert_eq!(resolve_shifted_button(0, false), None);
        assert_eq!(resolve_shifted_button(20, false), None);
    }

    #[test]
    fn shifted_mapping_has_no_collisions() {
        let mut logical: Vec<u8> = Vec::new();
        for n in 1..=19u8 {
            if let Some(l) = resolve_shifted_button(n, true) {
                logical.push(l);
            }
        }
        let count = logical.len();
        logical.sort();
        logical.dedup();
        assert_eq!(logical.len(), count, "collision in shifted button mapping");
    }

    // ── Pinkie-held detection ────────────────────────────────────────────

    #[test]
    fn pinkie_held_when_button2_set() {
        let b = WarthogStickButtons {
            buttons_low: 0x0002, // bit 1 = button 2
            buttons_high: 0,
            hat: WarthogHat::Center,
        };
        assert!(is_pinkie_held(&b));
    }

    #[test]
    fn pinkie_not_held_when_button2_clear() {
        let b = WarthogStickButtons {
            buttons_low: 0x0001, // only button 1
            buttons_high: 0,
            hat: WarthogHat::Center,
        };
        assert!(!is_pinkie_held(&b));
    }

    // ── Throttle split / merge detection ─────────────────────────────────

    #[test]
    fn merged_throttle_detected() {
        assert!(!is_throttle_split(0.50, 0.50));
        assert!(!is_throttle_split(0.50, 0.51));
    }

    #[test]
    fn split_throttle_detected() {
        assert!(is_throttle_split(0.20, 0.80));
        assert!(is_throttle_split(0.0, 0.05));
    }

    // ── LED control reports ──────────────────────────────────────────────

    #[test]
    fn led_off_report() {
        let r = build_led_report(LedState::Off);
        assert_eq!(r, [WARTHOG_LED_REPORT_ID, 0x00]);
    }

    #[test]
    fn led_brightness_report() {
        for level in 1..=5u8 {
            let r = build_led_report(LedState::Brightness(level));
            assert_eq!(r[0], WARTHOG_LED_REPORT_ID);
            assert_eq!(r[1], level);
        }
    }

    #[test]
    fn led_brightness_clamped_to_max() {
        let r = build_led_report(LedState::Brightness(255));
        assert_eq!(r[1], LedState::MAX_BRIGHTNESS);
    }

    // ── Toggle switch helpers ────────────────────────────────────────────

    #[test]
    fn toggle_is_set_checks_correct_bit() {
        assert!(toggles::is_set(0b0000_0001, toggles::EFL_NORM));
        assert!(!toggles::is_set(0b0000_0001, toggles::EFR_NORM));
        assert!(toggles::is_set(0b1000_0000, toggles::SPDB));
    }

    #[test]
    fn toggle_bit_out_of_range_returns_false() {
        assert!(!toggles::is_set(0xFF, 8));
        assert!(!toggles::is_set(0xFF, 255));
    }

    #[test]
    fn is_toggle_active_delegates_correctly() {
        let b = WarthogThrottleButtons {
            buttons_low: 0,
            buttons_mid: 0,
            buttons_high: 0,
            toggles: 0b0001_0000, // APU_START
            hat_dms: WarthogHat::Center,
            hat_csl: WarthogHat::Center,
        };
        assert!(is_toggle_active(&b, toggles::APU_START));
        assert!(!is_toggle_active(&b, toggles::EFL_NORM));
    }

    // ── TARGET compat ────────────────────────────────────────────────────

    #[test]
    fn target_compat_notes_not_empty() {
        assert!(!TARGET_COMPAT_NOTES.is_empty());
    }

    // ── ThrustmasterProtocol ─────────────────────────────────────────────

    #[test]
    fn protocol_identifies_warthog_stick() {
        let p = ThrustmasterProtocol::new(0x044F, 0x0402);
        assert_eq!(p.device(), Some(ThrustmasterDevice::WarthogJoystick));
    }

    #[test]
    fn protocol_identifies_warthog_throttle() {
        let p = ThrustmasterProtocol::new(0x044F, 0x0404);
        assert_eq!(p.device(), Some(ThrustmasterDevice::WarthogThrottle));
    }

    #[test]
    fn protocol_unknown_device() {
        let p = ThrustmasterProtocol::new(0x1234, 0x5678);
        assert_eq!(p.device(), None);
    }

    #[test]
    fn protocol_unknown_device_stick_parse_fails() {
        let p = ThrustmasterProtocol::new(0x1234, 0x5678);
        assert!(matches!(
            p.parse_stick_report(&[0; 32]),
            Err(ThrustmasterProtocolError::UnknownDevice)
        ));
    }

    #[test]
    fn protocol_unknown_device_throttle_parse_fails() {
        let p = ThrustmasterProtocol::new(0x1234, 0x5678);
        assert!(matches!(
            p.parse_throttle_report(&[0; 32]),
            Err(ThrustmasterProtocolError::UnknownDevice)
        ));
    }

    // ── Unified StickState parsing ───────────────────────────────────────

    #[test]
    fn parse_warthog_stick_centered() {
        fn stick_report(x: u16, y: u16, rz: u16) -> Vec<u8> {
            let mut buf = vec![0u8; 10];
            buf[0..2].copy_from_slice(&x.to_le_bytes());
            buf[2..4].copy_from_slice(&y.to_le_bytes());
            buf[4..6].copy_from_slice(&rz.to_le_bytes());
            buf[9] = 0xFF; // center hat
            buf
        }
        let data = stick_report(32768, 32768, 32768);
        let state = parse_stick_report(&data, ThrustmasterDevice::WarthogJoystick).unwrap();
        assert!(state.roll.abs() < 0.01);
        assert!(state.pitch.abs() < 0.01);
        assert!(state.twist.abs() < 0.01);
        assert_eq!(state.buttons.len(), 19);
        assert_eq!(state.hat, 0); // center
    }

    #[test]
    fn parse_t16000m_stick_centered() {
        fn joystick_report(x: u16, y: u16, rz: u16) -> Vec<u8> {
            let mut buf = vec![0u8; 11];
            buf[0..2].copy_from_slice(&x.to_le_bytes());
            buf[2..4].copy_from_slice(&y.to_le_bytes());
            buf[4..6].copy_from_slice(&rz.to_le_bytes());
            buf[10] = 0x0F; // center hat
            buf
        }
        let data = joystick_report(8192, 8192, 8192);
        let state = parse_stick_report(&data, ThrustmasterDevice::T16000mJoystick).unwrap();
        assert!(state.roll.abs() < 0.01);
        assert!(state.pitch.abs() < 0.01);
        assert!(state.twist.abs() < 0.01);
        assert_eq!(state.buttons.len(), 16);
        assert_eq!(state.hat, 0); // center
    }

    #[test]
    fn parse_warthog_stick_buttons_decoded() {
        let mut buf = vec![0u8; 10];
        buf[0..2].copy_from_slice(&32768u16.to_le_bytes());
        buf[2..4].copy_from_slice(&32768u16.to_le_bytes());
        buf[4..6].copy_from_slice(&32768u16.to_le_bytes());
        buf[6..8].copy_from_slice(&0x0003u16.to_le_bytes()); // buttons 1 & 2
        buf[9] = 0xFF;
        let state = parse_stick_report(&buf, ThrustmasterDevice::WarthogJoystick).unwrap();
        assert!(state.buttons[0], "button 1");
        assert!(state.buttons[1], "button 2");
        assert!(!state.buttons[2], "button 3 not pressed");
    }

    // ── Unified ThrottleState parsing ────────────────────────────────────

    #[test]
    fn parse_warthog_throttle_idle() {
        let mut buf = vec![0u8; 20];
        // Slew centered
        buf[0..2].copy_from_slice(&32768u16.to_le_bytes());
        buf[2..4].copy_from_slice(&32768u16.to_le_bytes());
        // Throttles at 0
        buf[16] = 0xFF; // hat center
        buf[17] = 0xFF;
        let state = parse_throttle_report(&buf, ThrustmasterDevice::WarthogThrottle).unwrap();
        assert!(state.throttle_left < 0.001);
        assert!(state.throttle_right < 0.001);
        assert_eq!(state.buttons.len(), 40);
    }

    #[test]
    fn parse_twcs_throttle_idle() {
        let mut buf = vec![0u8; 10];
        buf[2..4].copy_from_slice(&32768u16.to_le_bytes()); // mini-stick X centered
        buf[4..6].copy_from_slice(&32768u16.to_le_bytes()); // mini-stick Y centered
        buf[6..8].copy_from_slice(&32768u16.to_le_bytes()); // rocker centered
        let state = parse_throttle_report(&buf, ThrustmasterDevice::TwcsThrottle).unwrap();
        assert!(state.throttle_left < 0.001);
        assert!(state.slew_x.abs() < 0.01);
        assert_eq!(state.buttons.len(), 14);
    }

    #[test]
    fn parse_stick_report_wrong_device_type() {
        let buf = vec![0u8; 32];
        assert!(parse_stick_report(&buf, ThrustmasterDevice::WarthogThrottle).is_err());
    }

    #[test]
    fn parse_throttle_report_wrong_device_type() {
        let buf = vec![0u8; 32];
        assert!(parse_throttle_report(&buf, ThrustmasterDevice::WarthogJoystick).is_err());
    }

    #[test]
    fn warthog_stick_report_id_stripped() {
        // 11 bytes = 1 report-ID + 10 payload; report ID should be stripped
        let mut buf = vec![0u8; 11];
        buf[0] = 0x01; // report ID
        buf[1..3].copy_from_slice(&32768u16.to_le_bytes()); // x
        buf[3..5].copy_from_slice(&32768u16.to_le_bytes()); // y
        buf[5..7].copy_from_slice(&32768u16.to_le_bytes()); // rz
        buf[10] = 0xFF; // center hat
        let state = parse_stick_report(&buf, ThrustmasterDevice::WarthogJoystick).unwrap();
        assert_eq!(state.buttons.len(), 19);
    }

    #[test]
    fn cougar_report_id_stripped() {
        // 11 bytes = 1 report-ID + 10 payload
        let mut buf = vec![0u8; 11];
        buf[0] = 0x01;
        buf[1..3].copy_from_slice(&32768u16.to_le_bytes());
        buf[3..5].copy_from_slice(&32768u16.to_le_bytes());
        let state = parse_stick_report(&buf, ThrustmasterDevice::HotasCougar).unwrap();
        assert_eq!(state.buttons.len(), 16);
    }

    #[test]
    fn warthog_throttle_report_id_stripped() {
        // 19 bytes = 1 report-ID + 18 payload
        let mut buf = vec![0u8; 19];
        buf[0] = 0x01;
        buf[1..3].copy_from_slice(&32768u16.to_le_bytes());
        buf[3..5].copy_from_slice(&32768u16.to_le_bytes());
        let state = parse_throttle_report(&buf, ThrustmasterDevice::WarthogThrottle).unwrap();
        assert_eq!(state.buttons.len(), 40);
    }

    #[test]
    fn stick_state_hat_directions() {
        // Warthog hat: upper nibble at byte 9
        let mut buf = vec![0u8; 10];
        buf[0..2].copy_from_slice(&32768u16.to_le_bytes());
        buf[2..4].copy_from_slice(&32768u16.to_le_bytes());
        buf[4..6].copy_from_slice(&32768u16.to_le_bytes());

        // North: upper nibble = 0
        buf[9] = 0x00;
        let state = parse_stick_report(&buf, ThrustmasterDevice::WarthogJoystick).unwrap();
        assert_eq!(state.hat, 1, "North");

        // East: upper nibble = 2
        buf[9] = 0x20;
        let state = parse_stick_report(&buf, ThrustmasterDevice::WarthogJoystick).unwrap();
        assert_eq!(state.hat, 3, "East");
    }

    #[test]
    fn protocol_error_display() {
        let e = ThrustmasterProtocolError::UnknownDevice;
        assert_eq!(format!("{e}"), "unknown Thrustmaster device");

        let e = ThrustmasterProtocolError::ReportTooShort {
            expected: 10,
            actual: 5,
        };
        assert!(format!("{e}").contains("too short"));
    }
}
