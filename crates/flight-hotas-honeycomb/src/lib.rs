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
pub mod bravo;
pub mod bravo_leds;
pub mod charlie;
pub mod health;
pub mod presets;
pub mod profiles;
pub mod protocol;

/// USB Vendor ID for all Honeycomb Aeronautical products.
pub const HONEYCOMB_VENDOR_ID: u16 = 0x294B;

/// USB Product ID for the Alpha Flight Controls XPC (Yoke).
///
/// **Caution:** This PID (0x0102) is a community-reported value. It has not been
/// confirmed with hardware. Use with care; verify with `lsusb` / USBView before
/// relying on it for matching.
pub const HONEYCOMB_ALPHA_YOKE_PID: u16 = 0x0102;

/// USB Product ID for the Bravo Throttle Quadrant.
///
/// Confirmed from BetterBravoLights (RoystonS), FwlDynamicJoystickMapper Lua
/// scripts, SPAD.neXt profiles, and linux-hardware.org probe data.
pub const HONEYCOMB_BRAVO_PID: u16 = 0x1901;

/// USB Product ID for the Charlie Rudder Pedals.
///
/// **Caution:** This PID (0x1902) is community-inferred from the sequential
/// Honeycomb numbering scheme (Alpha=0x1900, Bravo=0x1901, Charlie=0x1902).
/// Not hardware-confirmed. Verify with `lsusb` / USBView before relying on
/// it for matching.
pub const HONEYCOMB_CHARLIE_PID: u16 = 0x1902;

/// Returns `true` if this VID/PID combination belongs to a known Honeycomb device.
pub fn is_honeycomb_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == HONEYCOMB_VENDOR_ID
        && matches!(
            product_id,
            HONEYCOMB_ALPHA_YOKE_PID | HONEYCOMB_BRAVO_PID | HONEYCOMB_CHARLIE_PID
        )
}

/// Identify which Honeycomb model a VID/PID refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoneycombModel {
    AlphaYoke,
    BravoThrottle,
    CharliePedals,
}

/// Returns the model for a known Honeycomb PID, or `None` for unknown PIDs.
pub fn honeycomb_model(product_id: u16) -> Option<HoneycombModel> {
    match product_id {
        HONEYCOMB_ALPHA_YOKE_PID => Some(HoneycombModel::AlphaYoke),
        HONEYCOMB_BRAVO_PID => Some(HoneycombModel::BravoThrottle),
        HONEYCOMB_CHARLIE_PID => Some(HoneycombModel::CharliePedals),
        _ => None,
    }
}

pub use alpha::{AlphaInputState, AlphaParseError, parse_alpha_report};
pub use bravo::{BravoInputState, BravoParseError, parse_bravo_report};
pub use bravo_leds::{BravoLedState, deserialize_led_report, serialize_led_report};
pub use charlie::{CharlieInputState, CharlieParseError, parse_charlie_report};
pub use protocol::{
    AlphaRockerSwitch, EncoderTracker, FlapPosition, FlapSwitchTracker,
    GearIndicatorState, MagnetoPosition, ToggleSwitchState, TrimWheelTracker,
    WrappingEncoder, decode_all_alpha_rockers, decode_alpha_rocker, decode_magneto,
    decode_toggle_switch,
};
