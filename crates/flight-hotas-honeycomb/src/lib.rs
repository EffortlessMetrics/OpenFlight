// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Honeycomb Aeronautical Alpha Yoke, Bravo Throttle Quadrant, and Charlie
//! Rudder Pedals driver.
//!
//! # Devices
//!
//! - **Alpha Flight Controls XPC** — 2-axis yoke, 36 buttons, 1 hat, magneto switch
//! - **Bravo Throttle Quadrant** — 7-axis throttle, 64 buttons, 21 LEDs, rotary encoders
//! - **Charlie Rudder Pedals** — 3-axis pedals (rudder, left/right brake);
//!   VID 0x294B confirmed; PID 0x1902 community-inferred.
//!   See `compat/devices/honeycomb/charlie-rudder-pedals.yaml`.
//!
//! # Protocol details
//!
//! The [`protocol`] module provides higher-level decoding:
//! - Magneto switch position decoding (Alpha)
//! - Encoder delta tracking (Bravo autopilot knobs)
//! - Gear indicator state management (Bravo)
//! - Toggle switch decoding (Bravo)
//!
//! # Device profiles
//!
//! The [`profiles`] module provides default axis/button configurations for
//! each device, suitable for the profile pipeline.
//!
//! # Bravo LED control
//!
//! LED state is set via [`bravo_leds::BravoLedState`] and serialised with
//! [`bravo_leds::serialize_led_report`], producing a 5-byte feature report
//! that must be sent with `hid_send_feature_report`.
//!
//! # Report layout
//!
//! Input report layouts are estimated from the HID joystick specification.
//! Hardware validation is recommended before deploying in production.
//! The LED output protocol is confirmed from BetterBravoLights (RoystonS).

pub mod alpha;
pub mod alpha_buttons;
pub mod bravo;
pub mod bravo_buttons;
pub mod bravo_leds;
pub mod button_delta;
pub mod charlie;
pub mod health;
pub mod presets;
pub mod profiles;
pub mod protocol;

/// USB Vendor ID for all Honeycomb Aeronautical products (current production).
pub const HONEYCOMB_VENDOR_ID: u16 = 0x294B;

/// USB Vendor ID used by early Honeycomb units (Microchip default VID).
///
/// Some first-generation Alpha Yokes and Bravo Throttle Quadrants shipped
/// with the Microchip default VID (0x04D8) before Honeycomb obtained their
/// own vendor ID. Both VIDs should be checked when enumerating devices.
pub const HONEYCOMB_VENDOR_ID_LEGACY: u16 = 0x04D8;

/// USB Product ID for the Alpha Flight Controls XPC (Yoke).
///
/// Confirmed: VID 0x294B, PID 0x1900 — linux-hardware.org hardware probe data
/// (8 probes reporting "Alpha Flight Controls"). See also
/// `compat/devices/honeycomb/alpha-yoke.yaml`.
pub const HONEYCOMB_ALPHA_YOKE_PID: u16 = 0x1900;

/// Legacy PID alias (0x0102) — community-reported, never hardware-confirmed.
/// Kept for backward compatibility; prefer [`HONEYCOMB_ALPHA_YOKE_PID`].
pub const HONEYCOMB_ALPHA_YOKE_PID_LEGACY_COMMUNITY: u16 = 0x0102;

/// Legacy PID for the Alpha Flight Controls under the Microchip VID.
///
/// Early production Alpha Yokes using VID 0x04D8 report PID 0xE6D6.
pub const HONEYCOMB_ALPHA_YOKE_PID_LEGACY: u16 = 0xE6D6;

/// USB Product ID for the Bravo Throttle Quadrant.
///
/// Confirmed from BetterBravoLights (RoystonS), FwlDynamicJoystickMapper Lua
/// scripts, SPAD.neXt profiles, and linux-hardware.org probe data.
pub const HONEYCOMB_BRAVO_PID: u16 = 0x1901;

/// Legacy PID for the Bravo Throttle Quadrant under the Microchip VID.
///
/// Early production Bravo units using VID 0x04D8 report PID 0xE6D5.
pub const HONEYCOMB_BRAVO_PID_LEGACY: u16 = 0xE6D5;

/// USB Product ID for the Charlie Rudder Pedals.
///
/// **Caution:** This PID (0x1902) is community-inferred from the sequential
/// Honeycomb numbering scheme (Alpha=0x1900, Bravo=0x1901, Charlie=0x1902).
/// Not hardware-confirmed. Verify with `lsusb` / USBView before relying on
/// it for matching.
pub const HONEYCOMB_CHARLIE_PID: u16 = 0x1902;

/// Returns `true` if this VID/PID combination belongs to a known Honeycomb device.
///
/// Recognises both the current Honeycomb VID (0x294B) and the legacy
/// Microchip VID (0x04D8) used by early production units.
pub fn is_honeycomb_device(vendor_id: u16, product_id: u16) -> bool {
    honeycomb_model_from_vid_pid(vendor_id, product_id).is_some()
}

/// Identify which Honeycomb model a VID/PID refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HoneycombModel {
    AlphaYoke,
    BravoThrottle,
    CharliePedals,
}

/// Returns the model for a known Honeycomb VID/PID pair, or `None`.
///
/// Handles both the current VID (0x294B) and legacy Microchip VID (0x04D8).
pub fn honeycomb_model_from_vid_pid(vendor_id: u16, product_id: u16) -> Option<HoneycombModel> {
    match (vendor_id, product_id) {
        (HONEYCOMB_VENDOR_ID, HONEYCOMB_ALPHA_YOKE_PID) => Some(HoneycombModel::AlphaYoke),
        (HONEYCOMB_VENDOR_ID, HONEYCOMB_ALPHA_YOKE_PID_LEGACY_COMMUNITY) => {
            Some(HoneycombModel::AlphaYoke)
        }
        (HONEYCOMB_VENDOR_ID, HONEYCOMB_BRAVO_PID) => Some(HoneycombModel::BravoThrottle),
        (HONEYCOMB_VENDOR_ID, HONEYCOMB_CHARLIE_PID) => Some(HoneycombModel::CharliePedals),
        (HONEYCOMB_VENDOR_ID_LEGACY, HONEYCOMB_ALPHA_YOKE_PID_LEGACY) => {
            Some(HoneycombModel::AlphaYoke)
        }
        (HONEYCOMB_VENDOR_ID_LEGACY, HONEYCOMB_BRAVO_PID_LEGACY) => {
            Some(HoneycombModel::BravoThrottle)
        }
        _ => None,
    }
}

/// Returns the model for a known Honeycomb PID (current VID only), or `None`.
pub fn honeycomb_model(product_id: u16) -> Option<HoneycombModel> {
    match product_id {
        HONEYCOMB_ALPHA_YOKE_PID | HONEYCOMB_ALPHA_YOKE_PID_LEGACY_COMMUNITY => {
            Some(HoneycombModel::AlphaYoke)
        }
        HONEYCOMB_BRAVO_PID => Some(HoneycombModel::BravoThrottle),
        HONEYCOMB_CHARLIE_PID => Some(HoneycombModel::CharliePedals),
        _ => None,
    }
}

pub use alpha::{AlphaInputState, AlphaParseError, parse_alpha_report};
pub use alpha_buttons::AlphaButton;
pub use bravo::{BravoInputState, BravoParseError, parse_bravo_report};
pub use bravo_buttons::BravoButton;
pub use bravo_leds::{BravoLedState, serialize_led_report};
pub use button_delta::ButtonDelta;
pub use charlie::{CharlieInputState, CharlieParseError, parse_charlie_report};
pub use protocol::{
    EncoderTracker, GearIndicatorState, MagnetoPosition, ToggleSwitchState, WrappingEncoder,
    decode_magneto, decode_toggle_switch,
};
