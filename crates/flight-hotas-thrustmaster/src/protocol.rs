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
    DeviceEntry { pid: 0xB10A, device: ThrustmasterDevice::T16000mJoystick },
    DeviceEntry { pid: 0xB687, device: ThrustmasterDevice::TwcsThrottle },
    // Warthog
    DeviceEntry { pid: 0x0402, device: ThrustmasterDevice::WarthogJoystick },
    DeviceEntry { pid: 0x0404, device: ThrustmasterDevice::WarthogThrottle },
    // T.Flight HOTAS
    DeviceEntry { pid: 0xB108, device: ThrustmasterDevice::TFlightHotasX },
    DeviceEntry { pid: 0xB67B, device: ThrustmasterDevice::TFlightHotas4 },
    DeviceEntry { pid: 0xB67A, device: ThrustmasterDevice::TFlightHotas4Legacy },
    DeviceEntry { pid: 0xB67C, device: ThrustmasterDevice::TFlightHotas4V2 },
    DeviceEntry { pid: 0xB68D, device: ThrustmasterDevice::TFlightHotasOne },
    DeviceEntry { pid: 0xB68B, device: ThrustmasterDevice::TFlightHotasOneBulk },
    // T.Flight Stick X
    DeviceEntry { pid: 0xB106, device: ThrustmasterDevice::TFlightStickX },
    DeviceEntry { pid: 0xB107, device: ThrustmasterDevice::TFlightStickXV2 },
    // Pedals
    DeviceEntry { pid: 0xB678, device: ThrustmasterDevice::TfrpRudderPedals },
    DeviceEntry { pid: 0xB679, device: ThrustmasterDevice::TRudder },
    DeviceEntry { pid: 0xB68F, device: ThrustmasterDevice::TprPendular },
    DeviceEntry { pid: 0xB68E, device: ThrustmasterDevice::TprPendularBulk },
    // Legacy
    DeviceEntry { pid: 0x0400, device: ThrustmasterDevice::HotasCougar },
    // TCA Boeing
    DeviceEntry { pid: 0xB68C, device: ThrustmasterDevice::TcaYokeBoeing },
    DeviceEntry { pid: 0xB694, device: ThrustmasterDevice::TcaQuadrantBoeingEng12 },
    DeviceEntry { pid: 0xB695, device: ThrustmasterDevice::TcaQuadrantBoeingEng34 },
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
            assert!(!entry.device.name().is_empty(), "empty name for {:?}", entry.device);
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
}
