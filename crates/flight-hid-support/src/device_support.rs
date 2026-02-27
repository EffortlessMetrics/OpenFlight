// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Device support registry and quirk detection for common HID devices.

use crate::HidDeviceInfo;
use crate::hid_descriptor::{HidUsage, extract_usages};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;

pub const THRUSTMASTER_VENDOR_ID: u16 = 0x044F;
pub const VKB_VENDOR_ID: u16 = 0x231D;
pub const SAITEK_VENDOR_ID: u16 = 0x06A3;
pub const MAD_CATZ_VENDOR_ID: u16 = 0x0738;
pub const LOGITECH_VENDOR_ID: u16 = 0x046D;

/// USB Product ID for the Logitech Extreme 3D Pro joystick.
///
/// Confirmed: VID 0x046D (Logitech), PID 0xC215 — from linux-hardware.org (221 probes).
pub const EXTREME_3D_PRO_PID: u16 = 0xC215;

/// USB Product ID for the Logitech Force 3D Pro joystick (force-feedback).
///
/// Confirmed: VID 0x046D (Logitech), PID 0xC286 — linux-hardware.org (18 probes, "Force 3D Pro").
/// One of the most popular FFB joysticks ever made; DirectInput FFB on X/Y/Rz axes.
pub const FORCE_3D_PRO_PID: u16 = 0xC286;

/// USB Product ID for the Logitech Flight System G940 (FFB HOTAS set).
///
/// Confirmed: VID 0x046D (Logitech), PID 0xC287 — linux-hardware.org (1 probe, "Flight System G940").
/// One of the few commercial FFB HOTAS sets ever made; discontinued ~2013.
pub const G940_FLIGHT_SYSTEM_PID: u16 = 0xC287;

/// USB Product ID for the G940 throttle interface.
///
/// **Unconfirmed:** Inferred from the sequential Logitech flight-controller
/// numbering (Force 3D Pro = 0xC286, G940 joystick = 0xC287 → throttle = 0xC288).
/// No independent hardware probe has confirmed this PID.
pub const G940_THROTTLE_PID: u16 = 0xC288;

/// Returns `true` if this VID/PID combination is a Logitech Extreme 3D Pro.
pub fn is_extreme_3d_pro(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == LOGITECH_VENDOR_ID && product_id == EXTREME_3D_PRO_PID
}

/// Returns `true` if this VID/PID combination is a Logitech G940 joystick.
pub fn is_g940_joystick(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == LOGITECH_VENDOR_ID && product_id == G940_FLIGHT_SYSTEM_PID
}

/// Returns `true` if this VID/PID combination is a Logitech G940 throttle.
///
/// **Note:** Uses the unconfirmed [`G940_THROTTLE_PID`] constant.
pub fn is_g940_throttle(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == LOGITECH_VENDOR_ID && product_id == G940_THROTTLE_PID
}

/// USB Product ID for the Logitech G Flight Yoke System.
///
/// Confirmed: VID 0x046D (Logitech), PID 0xC259 — linux-hardware.org.
/// The yoke and bundled throttle quadrant (PID 0xC25A) enumerate as
/// separate USB HID devices.
pub const G_FLIGHT_YOKE_PID: u16 = 0xC259;

/// USB Product ID for the Logitech G Flight Throttle Quadrant.
///
/// Confirmed: VID 0x046D (Logitech), PID 0xC25A — linux-hardware.org.
/// Standalone throttle quadrant; three levers (Z/Rz/Slider) + 6 buttons.
pub const G_FLIGHT_THROTTLE_QUADRANT_PID: u16 = 0xC25A;

/// Returns `true` if this VID/PID is a Logitech G Flight Yoke.
pub fn is_g_flight_yoke(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == LOGITECH_VENDOR_ID && product_id == G_FLIGHT_YOKE_PID
}

/// Returns `true` if this VID/PID is a Logitech G Flight Throttle Quadrant.
pub fn is_g_flight_throttle_quadrant(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == LOGITECH_VENDOR_ID && product_id == G_FLIGHT_THROTTLE_QUADRANT_PID
}

/// USB Vendor ID for all Honeycomb Aeronautical products.
pub const HONEYCOMB_VENDOR_ID: u16 = 0x294B;

/// USB Product ID for the Honeycomb Alpha Flight Controls XPC (Yoke).
///
/// Confirmed: VID 0x294B, PID 0x1900 — linux-hardware.org hardware probe data
/// (8 independent system probes, device string "Alpha Flight Controls").
pub const HONEYCOMB_ALPHA_YOKE_PID: u16 = 0x1900;

/// USB Product ID for the Honeycomb Bravo Throttle Quadrant.
///
/// Confirmed from multiple independent sources: BetterBravoLights (RoystonS,
/// `BravoLights.Common/UsbLogic.cs`), SPAD.neXt profiles, and
/// linux-hardware.org probe data (8+ probes).
pub const HONEYCOMB_BRAVO_PID: u16 = 0x1901;

/// USB Product ID for the Honeycomb Charlie Rudder Pedals.
///
/// **Caution:** This PID (0x1902) is community-inferred from the sequential
/// Honeycomb numbering scheme (Alpha=0x1900, Bravo=0x1901 → Charlie=0x1902).
/// No independent hardware probe or open-source project has been found that
/// confirms this value. Verify with `lsusb` / USBView on real hardware before
/// using for production device matching.
pub const HONEYCOMB_CHARLIE_RUDDER_PID: u16 = 0x1902;

/// USB Product ID for the T.Flight Rudder Pedals (TFRP).
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB678 — from the-sz.com USB ID DB.
pub const TFRP_RUDDER_PEDALS_PID: u16 = 0xB678;

/// USB Product ID for the T-Rudder pedals.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB679 — from the-sz.com USB ID DB.
pub const T_RUDDER_PID: u16 = 0xB679;

/// USB Product ID for the T-Pendular Rudder (TPR) standard variant.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB68F — linux-hardware.org (7 probes,
/// USB device string "T-Pendular-Rudder").
pub const TPR_PENDULAR_RUDDER_PID: u16 = 0xB68F;

/// USB Product ID for the T-Pendular Rudder (TPR) Bulk-channel variant.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB68E — linux-hardware.org ("TPR Rudder Bulk").
/// The Bulk variant uses a different USB endpoint but reports the same axes as the standard TPR.
pub const TPR_PENDULAR_RUDDER_BULK_PID: u16 = 0xB68E;

/// USB Product ID for the T.Flight HOTAS One (primary HID interrupt mode).
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB68D — linux-hardware.org (17 probes,
/// USB string "T.Flight Hotas One"). This is the most common PID on PC.
pub const TFLIGHT_HOTAS_ONE_PID: u16 = 0xB68D;

/// USB Product ID for the T.Flight HOTAS One Bulk-endpoint variant.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB68B — linux-hardware.org ("T.Flight Hotas One Bulk").
/// Identical capabilities; uses bulk transfer instead of interrupt.
pub const TFLIGHT_HOTAS_ONE_BULK_PID: u16 = 0xB68B;

/// Primary PID for T.Flight HOTAS 4 (PS4 / "HID 4" variant).
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB67C — linux-hardware.org (11 probes,
/// USB string "T.Flight Hotas 4"). Newer firmware; see TFLIGHT_HOTAS_4_PID for older variant.
pub const TFLIGHT_HOTAS_4_PID_V2: u16 = 0xB67C;

/// Older PID for T.Flight HOTAS 4 - verified via USBView artifact.
pub const TFLIGHT_HOTAS_4_PID: u16 = 0xB67B;
/// Legacy PID for T.Flight HOTAS 4 - may appear on older firmware versions.
pub const TFLIGHT_HOTAS_4_PID_LEGACY: u16 = 0xB67A;
/// USB Product ID for the T.Flight HOTAS X (PS4/PC combined unit).
///
/// Confirmed: VID 0x044F (Thrustmaster), PID 0xB108 — from open-siege/siege-studio device info.
pub const TFLIGHT_HOTAS_X_PID: u16 = 0xB108;

/// USB Product ID for the T.Flight Stick X (standalone joystick, primary firmware variant).
///
/// Confirmed: VID 0x044F (Thrustmaster), PID 0xB106 — linux-hardware.org (28 probes, "T.Flight Stick X").
/// See also TFLIGHT_STICK_X_PID_V2 for the alternate firmware variant 0xB107.
pub const TFLIGHT_STICK_X_PID: u16 = 0xB106;
/// USB Product ID for the T.Flight Stick X alternate firmware variant.
///
/// Confirmed: VID 0x044F (Thrustmaster), PID 0xB107 — linux-hardware.org ("T.Flight Stick X").
pub const TFLIGHT_STICK_X_PID_V2: u16 = 0xB107;

/// USB Product ID for the T.16000M FCS joystick.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB10A — from linux-hardware.org probe data.
pub const T16000M_JOYSTICK_PID: u16 = 0xB10A;

/// USB Product ID for the TWCS Throttle (sold standalone and as part of the
/// T.16000M FCS HOTAS combo).
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB687 — from linux-hardware.org probe data.
pub const TWCS_THROTTLE_PID: u16 = 0xB687;

/// USB Product ID for the Thrustmaster TMX Force Feedback Racing Wheel.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB67F — linux-hardware.org ("TMX Racing Wheel",
/// 2 probes) + libsdl-org/SDL SDL2 initial_wheel_devices[].
/// Primarily a racing wheel but commonly used in flight simulators for rudder input.
pub const TMX_RACING_WHEEL_PID: u16 = 0xB67F;

/// USB Product ID for the HOTAS Warthog Joystick.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0x0402 — from linux-hardware.org probe data.
pub const WARTHOG_JOYSTICK_PID: u16 = 0x0402;

/// USB Product ID for the HOTAS Warthog Throttle.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0x0404 — from linux-hardware.org probe data.
pub const WARTHOG_THROTTLE_PID: u16 = 0x0404;

/// USB Product ID for the HOTAS Cougar stick.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0x0400 — from linux-hardware.org probe data
/// (ID usb:044f:0400, "ThrustMaster HOTAS Cougar").
pub const COUGAR_HOTAS_STICK_PID: u16 = 0x0400;

/// USB Product ID for the Thrustmaster F-16 MFD Cougar Panel 1.
/// Part of the "MFD Cougar Pack" — a pair of LCD-framed button panels for DCS World.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB351 — linux-hardware.org (3 probes,
/// USB name "F16 MFD 1", MSI MPG Z390 GAMING EDGE).
pub const F16_MFD_1_PID: u16 = 0xB351;

/// USB Product ID for the Thrustmaster F-16 MFD Cougar Panel 2.
/// Identical capabilities to Panel 1; only PID differs.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB352 — linux-hardware.org (3 probes,
/// USB name "F16 MFD 2", MSI MPG Z390 GAMING EDGE).
pub const F16_MFD_2_PID: u16 = 0xB352;

/// USB Product ID for the TCA Sidestick Airbus Edition (Pilot / left-hand captain's seat).
///
/// Confirmed: VID 0x044F (Thrustmaster), PID 0x0405 — linux-hardware.org (24 probes,
/// USB string "T.A320 Pilot").
pub const TCA_SIDESTICK_AIRBUS_PILOT_PID: u16 = 0x0405;

/// USB Product ID for the TCA Sidestick Airbus Edition (Copilot / right-hand first-officer's seat).
///
/// Confirmed: VID 0x044F (Thrustmaster), PID 0x0406 — linux-hardware.org (12 probes,
/// USB string "T.A320 Copilot").
pub const TCA_SIDESTICK_AIRBUS_COPILOT_PID: u16 = 0x0406;

/// USB Product ID for the TCA Quadrant Airbus Edition (engines 1 & 2).
///
/// Confirmed: VID 0x044F (Thrustmaster), PID 0x0407 — linux-hardware.org ("TCA Q-Eng 1&2").
pub const TCA_QUADRANT_AIRBUS_ENG12_PID: u16 = 0x0407;

/// USB Product ID for the TCA Quadrant Airbus Add-On (engines 3 & 4).
///
/// Confirmed: VID 0x044F (Thrustmaster), PID 0x0408 — linux-hardware.org ("TCA Q-Eng 3&4").
pub const TCA_QUADRANT_AIRBUS_ENG34_PID: u16 = 0x0408;

/// USB Product ID for the TCA Yoke Boeing Edition.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0x0409 — linux-hardware.org (probe, "TCA YOKE BOEING").
pub const TCA_YOKE_BOEING_PID: u16 = 0x0409;

/// USB Product ID for the TCA Quadrant Boeing Edition (engines 1 & 2).
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0x040A — linux-hardware.org ("TCA Quadrant Boeing 1&2").
pub const TCA_QUADRANT_BOEING_ENG12_PID: u16 = 0x040A;

/// USB Product ID for the TCA Quadrant Boeing Add-On (engines 3 & 4).
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0x040B — linux-hardware.org ("TCA Quadrant Boeing 3&4").
pub const TCA_QUADRANT_BOEING_ENG34_PID: u16 = 0x040B;

/// USB Product ID for the Thrustmaster Sim Pedals.
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB371 — linux-hardware.org (5 probes, "Sim Pedals").
pub const THRUSTMASTER_SIM_PEDALS_PID: u16 = 0xB371;

/// USB Product ID for the Thrustmaster USB Joystick (legacy budget joystick, circa 2003–2010).
///
/// Confirmed: VID 0x044F (ThrustMaster), PID 0xB304 — linux-hardware.org (multiple probes,
/// USB string "Thrustmaster USB Joystick"). Basic 3-axis + throttle + twist joystick.
pub const THRUSTMASTER_USB_JOYSTICK_PID: u16 = 0xB304;

// Saitek/Logitech HOTAS PIDs
// See docs/reference/hotas-claims.md for verification status
//
// X52 family (unified USB) - confidence: KNOWN
pub const X52_PID: u16 = 0x075C;
pub const X52_PRO_PID: u16 = 0x0762;

// X65F (unified USB, Saitek VID 0x06A3) - confidence: LIKELY
// Source: Linux kernel hid-ids.h (USB_DEVICE_ID_SAITEK_X65)
pub const X65F_PID: u16 = 0x0B6A;

// X55 family (split USB, Saitek VID 0x06A3) - confidence: LIKELY
// Note: Some X55 units may use Mad Catz VID (0x0738) with same PIDs
pub const X55_STICK_PID: u16 = 0x2215;
pub const X55_THROTTLE_PID: u16 = 0xA215;

// X56 family - Mad Catz era (split USB, VID 0x0738) - confidence: CONFIRMED
// Confirmed: linux-hardware.org (stick 26 probes, throttle 30 probes) + SDL2 source
// (libsdl-org/SDL SDL2 initial_flightstick_devices / initial_throttle_devices)
pub const X56_MADCATZ_STICK_PID: u16 = 0x2221;
pub const X56_MADCATZ_THROTTLE_PID: u16 = 0xA221;

// X56 family - Logitech branded (split USB, VID 0x046D) - confidence: SUSPECT/WRONG
// WARNING: PID 0xC229 is confirmed as "G19 Gaming Keyboard Macro Interface" on
// linux-hardware.org (80 probes). PID 0xC22A is "Gaming Keyboard G110" (116 probes).
// Neither is the Logitech-era X56. See docs/reference/hotas-claims.md.
// Do NOT use these PIDs for X56 matching until correct Logitech X56 PIDs are confirmed.
pub const X56_LOGITECH_STICK_PID: u16 = 0xC229; // WRONG: this is a G19 keyboard macro IF
// WRONG: pub const X56_LOGITECH_THROTTLE_PID: u16 = 0xC22A; // This is a G110 keyboard

// Saitek standalone devices - confidence: CONFIRMED (linux-hardware.org)
// 0x0764: Flight Pro Combat Rudder (2 probes) - pre-Pro Flight era gaming rudder pedals
// 0x0C2D: Pro Flight Quadrant (22 probes) - NOTE: also claimed by x56.yaml for X56
//         Mad Catz stick variant; linux-hardware.org evidence favours Quadrant
pub const SAITEK_FLIGHT_PRO_COMBAT_RUDDER_PID: u16 = 0x0764;
pub const SAITEK_PRO_FLIGHT_QUADRANT_PID: u16 = 0x0C2D;
/// USB Product ID for the Saitek Pro Flight Yoke System.
///
/// Confirmed: VID 0x06A3 (Saitek), PID 0x0BAC — linux-hardware.org (23 probes, "Pro Flight Yoke").
pub const SAITEK_PRO_FLIGHT_YOKE_PID: u16 = 0x0BAC;
/// USB Product ID for the Saitek Pro Flight Rudder Pedals (standard variant).
///
/// Confirmed: VID 0x06A3 (Saitek), PID 0x0763 — linux-hardware.org ("Pro Flight Rudder Pedals").
/// The Cessna-branded variant uses PID 0x0765.
pub const SAITEK_PRO_FLIGHT_RUDDER_PEDALS_PID: u16 = 0x0763;
/// USB Product ID for the Saitek Pro Flight Multi Panel.
///
/// Confirmed: VID 0x06A3 (Saitek), PID 0x0D06 — linux-hardware.org (11 probes, "Flight Pro Multi Panel").
pub const SAITEK_PRO_FLIGHT_MULTI_PANEL_PID: u16 = 0x0D06;
/// USB Product ID for the Saitek Pro Flight Radio Panel.
///
/// Confirmed: VID 0x06A3 (Saitek), PID 0x0D05 — linux-hardware.org ("Pro Flight Radio Panel").
pub const SAITEK_PRO_FLIGHT_RADIO_PANEL_PID: u16 = 0x0D05;
/// USB Product ID for the Saitek Pro Flight Switch Panel.
///
/// Confirmed: VID 0x06A3 (Saitek), PID 0x0D67 — linux-hardware.org ("Pro Flight Switch Panel").
pub const SAITEK_PRO_FLIGHT_SWITCH_PANEL_PID: u16 = 0x0D67;

// Saitek Cessna simulation line (VID 0x06A3) - confidence: CONFIRMED (linux-hardware.org)
/// USB Product ID for the Saitek Pro Flight Cessna Yoke.
///
/// Confirmed: VID 0x06A3 (Saitek), PID 0x0BD3 — linux-hardware.org (3 probes,
/// "Pro Flight Cessna Yoke").
pub const SAITEK_CESSNA_YOKE_PID: u16 = 0x0BD3;
/// USB Product ID for the Saitek Pro Flight Cessna Rudder Pedals.
///
/// Confirmed: VID 0x06A3 (Saitek), PID 0x0765 — linux-hardware.org (4 probes,
/// "Pro Flight Cessna Rudder Pedals"). Cessna-branded variant of PID 0x0763.
pub const SAITEK_CESSNA_RUDDER_PEDALS_PID: u16 = 0x0765;
/// USB Product ID for the Saitek Pro Flight Cessna Trim Wheel.
///
/// Confirmed: VID 0x06A3 (Saitek), PID 0x0BD4 — linux-hardware.org (3 probes,
/// "Pro Flight Cessna Trim Wheel").
pub const SAITEK_CESSNA_TRIM_WHEEL_PID: u16 = 0x0BD4;

/// USB Vendor ID for VIRPIL Controls UAB.
///
/// Confirmed: [the-sz.com USB ID DB](https://www.the-sz.com/products/usbid/index.php?v=0x3344)
pub const VIRPIL_VENDOR_ID: u16 = 0x3344;

/// USB Product ID for the VIRPIL VPC MongoosT-50CM2 Throttle.
///
/// Confirmed: charliefoxtwo/Virpil-Communicator (`ThrottleCM2Pids = new() { 0x8193 }`)
/// and muchimi/JoystickGremlinEx (virpil_device.py, `CM2 = [0x8193]`).
pub const VIRPIL_CM2_THROTTLE_PID: u16 = 0x8193;

/// USB Product ID for the VIRPIL VPC MongoosT-50CM2 Stick (left-hand).
///
/// Confirmed: RavenX8/open-vpc Linux kernel driver
/// (driver/vpcdevice.h, `USB_DEVICE_ID_VIRPIL_STICK_MT_50CM2 0x4138`).
pub const VIRPIL_CM2_STICK_PID: u16 = 0x4138;

/// USB Product ID for the VIRPIL VPC Throttle CM3.
///
/// Source: Buzzec/virpil open-source Rust LED control library.
pub const VIRPIL_CM3_THROTTLE_PID: u16 = 0x0194;

/// USB Product ID for the VIRPIL VPC MongoosT-50CM3 (right stick).
///
/// Source: Buzzec/virpil open-source Rust LED control library.
pub const VIRPIL_MONGOOST_STICK_PID: u16 = 0x4130;

/// USB Product ID for the VIRPIL VPC Control Panel 1.
///
/// Source: Buzzec/virpil open-source Rust LED control library.
pub const VIRPIL_PANEL1_PID: u16 = 0x025B;

/// USB Product ID for the VIRPIL VPC Control Panel 2 (Right Panel).
///
/// Confirmed: VID 0x3344, PID 0x0259 — from Buzzec/virpil open-source Rust LED control library
/// (src/right_panel.rs, `const PID: u16 = 0x0259`).
pub const VIRPIL_PANEL2_PID: u16 = 0x0259;

/// USB Product ID for the VIRPIL VPC Shark Panel.
///
/// Confirmed: VID 0x3344, PID 0x825D — from Buzzec/virpil open-source Rust LED control library
/// (src/shark_panel.rs, `const PID: u16 = 0x825D`).
pub const VIRPIL_SHARK_PANEL_PID: u16 = 0x825D;

/// USB Product ID for the VIRPIL VPC Constellation Alpha (left grip on CM3 base).
///
/// Confirmed: VID 0x3344, PID 0x838F — linux-hardware.org probe data
/// ("L-VPC Stick MT-50CM3", 1 probe). Also confirmed via community mapping script
/// FwlDynamicJoystickMapper/virpil_cm3_aprime_l.lua.
pub const VIRPIL_CONSTELLATION_ALPHA_LEFT_PID: u16 = 0x838F;

/// USB Product ID for the VIRPIL VPC Constellation Alpha Prime (left grip, standalone HOSAS).
///
/// Source: VPCLedHandle open-source Python library (VPC_LEDs_server.py,
/// `vendor_id=0x3344, product_id=0x138`).
pub const VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID: u16 = 0x0138;

/// USB Product ID for the VIRPIL VPC Constellation Alpha Prime (right grip, standalone HOSAS).
///
/// Source: VPCLedHandle open-source Python library (VPC_LEDs_server.py,
/// `vendor_id=0x3344, product_id=0x4139`).
pub const VIRPIL_CONSTELLATION_ALPHA_PRIME_RIGHT_PID: u16 = 0x4139;

/// USB Product ID for the VIRPIL VPC WarBRD (original, right-hand joystick base).
///
/// Source: fredemmott/cpp-remapper project `devicedb.h` — device listed as
/// "RIGHT VPC Stick WarBRD" with `HardwareID: HID\VID_3344&PID_40CC&Col01`,
/// VID 0x3344, PID 0x40CC.
pub const VIRPIL_WARBRD_PID: u16 = 0x40CC;

/// USB Product ID for the VIRPIL VPC WarBRD-D (revised "D" variant, right-hand joystick base).
///
/// Source: LunaBaloona/Virpil_devices_on_Linux — `lsusb` output from real hardware:
/// `ID 3344:43f5 Leaguer Microelectronics (LME) R-VPC Stick WarBRD-D`.
pub const VIRPIL_WARBRD_D_PID: u16 = 0x43F5;

/// USB Vendor ID for CH Products.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h`
pub const CH_VENDOR_ID: u16 = 0x068E;

/// USB Product ID for the CH Pro Throttle.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h` (USB_DEVICE_ID_CH_PRO_THROTTLE)
pub const CH_PRO_THROTTLE_PID: u16 = 0x00F1;

/// USB Product ID for the CH Pro Pedals.
///
/// Confirmed: Linux kernel `drivers/hid/hid-ids.h` (USB_DEVICE_ID_CH_PRO_PEDALS);
/// linux-hardware.org (26 probes, "Flight Sim Pedals / CH PRO PEDALS USB").
pub const CH_PRO_PEDALS_PID: u16 = 0x00F2;

/// USB Product ID for the CH Fighterstick.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h` (USB_DEVICE_ID_CH_FIGHTERSTICK)
pub const CH_FIGHTERSTICK_PID: u16 = 0x00F3;

/// USB Product ID for the CH Combat Stick.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h` (USB_DEVICE_ID_CH_COMBATSTICK)
pub const CH_COMBAT_STICK_PID: u16 = 0x00F4;

/// USB Product ID for the CH Flight Sim Eclipse Yoke.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h` (USB_DEVICE_ID_CH_ECLIPSE_YOKE)
pub const CH_ECLIPSE_YOKE_PID: u16 = 0x0051;

/// USB Product ID for the CH Flight Sim Yoke.
///
/// Source: Linux kernel `drivers/hid/hid-ids.h` (USB_DEVICE_ID_CH_YOKE_USB)
pub const CH_FLIGHT_YOKE_PID: u16 = 0x00FF;

pub const VKB_STECS_LEFT_SPACE_MINI_PID: u16 = 0x0136;
pub const VKB_STECS_RIGHT_SPACE_MINI_PID: u16 = 0x013A;
pub const VKB_STECS_LEFT_SPACE_MINI_PLUS_PID: u16 = 0x0137;
pub const VKB_STECS_RIGHT_SPACE_MINI_PLUS_PID: u16 = 0x013B;
pub const VKB_STECS_LEFT_SPACE_STANDARD_PID: u16 = 0x0138;
pub const VKB_STECS_RIGHT_SPACE_STANDARD_PID: u16 = 0x013C;
pub const VKB_GLADIATOR_NXT_EVO_RIGHT_PID: u16 = 0x0200;
pub const VKB_GLADIATOR_NXT_EVO_LEFT_PID: u16 = 0x0201;

/// USB Product ID for the VKB S-TECS Modern Throttle Mini (2023 generation).
///
/// Confirmed: VID 0x231D (VKB), PID 0x012B — linux-hardware.org (1 probe,
/// USB string "S-TECS MODERN THROTTLE MINI", vendor "VKB-Sim (C) Alex Oz 2023").
pub const VKB_STECS_MODERN_THROTTLE_MINI_PID: u16 = 0x012B;

/// USB Product ID for the VKB S-TECS Modern Throttle Max (2023 generation).
///
/// Confirmed: VID 0x231D (VKB), PID 0x012E — linux-hardware.org (1 probe,
/// USB string "S-TECS MODERN THROTTLE MAX", vendor "VKB-Sim (C) Alex Oz 2023").
pub const VKB_STECS_MODERN_THROTTLE_MAX_PID: u16 = 0x012E;

/// USB Product ID for the VKB Gladiator NXT EVO Right SEM (Space/Enhanced/Modern variant).
///
/// Confirmed: VID 0x231D (VKB), PID 0x0204 — linux-hardware.org (1 probe,
/// USB string "VKBsim Gladiator NXT R SEM", vendor "VKB-Sim Alex Oz 2021").
pub const VKB_GLADIATOR_NXT_EVO_RIGHT_SEM_PID: u16 = 0x0204;

/// USB Product ID for the VKBsim Gladiator EVO OT L (Omni-Throttle / left-hand EVO variant).
///
/// Confirmed: VID 0x231D (VKB), PID 0x3201 — linux-hardware.org (2 probes,
/// USB string "VKBsim Gladiator EVO OT L").
/// "OT" denotes the left-hand Omni-Throttle grip variant released in 2023/2024.
pub const VKB_GLADIATOR_EVO_OT_LEFT_PID: u16 = 0x3201;

/// USB Product ID for the VKBSim NXT SEM THQ (Sensor-Encoder-Module + Throttle Hat Quadrant).
///
/// Confirmed: VID 0x231D (VKB), PID 0x2214 — linux-hardware.org (1 probe,
/// USB string "VKBSim NXT SEM THQ", vendor "VKB-Sim Alex Oz 2021").
/// The THQ is a button-panel accessory that attaches to the NXT SEM module.
pub const VKB_NXT_SEM_THQ_PID: u16 = 0x2214;

/// USB Product ID for the VKB Gladiator Modern Combat Pro (2021 edition).
///
/// Confirmed: VID 0x231D (VKB), PID 0x0131 — linux-hardware.org (2 probes,
/// USB string "VKBsim Gladiator Modern Combat Pro", vendor "VKB-Sim Alex Oz 2021").
pub const VKB_GLADIATOR_MODERN_COMBAT_PRO_PID: u16 = 0x0131;

/// USB Product ID for the VKB Gunfighter Modern Combat Pro (heavy desktop base + MCG Pro grip).
///
/// Confirmed: VID 0x231D (VKB), PID 0x0125 — linux-hardware.org (3 probes,
/// USB string "VKBsim Gunfighter Modern Combat Pro", vendor "Alex Oz 7238-D030").
/// Note: distinct from `VKB_GLADIATOR_MODERN_COMBAT_PRO_PID` (0x0131), which uses the lighter
/// Gladiator base. This PID corresponds to the heavier Gunfighter pedestal base.
pub const VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID: u16 = 0x0125;

/// USB Product ID for the VKB Space Gunfighter (Gunfighter pedestal base, right-hand or
/// unspecified chirality).
///
/// Confirmed: VID 0x231D (VKB), PID 0x0126 — linux-hardware.org (2 probes,
/// USB string "VKBsim Space Gunfighter", vendor "VKB-Sim (C) Alex Oz 2023").
pub const VKB_SPACE_GUNFIGHTER_PID: u16 = 0x0126;

/// USB Product ID for the VKB Space Gunfighter L (left-hand variant).
///
/// Confirmed: VID 0x231D (VKB), PID 0x0127 — linux-hardware.org (3 probes,
/// USB string "VKBSim Space Gunfighter L", vendor "VKB-Sim (C) Alex Oz 2023").
pub const VKB_SPACE_GUNFIGHTER_LEFT_PID: u16 = 0x0127;

/// USB Product ID for the VKB S-TECS Modern Throttle Mini Plus (2023 generation).
///
/// Confirmed: VID 0x231D (VKB), PID 0x012C — linux-hardware.org (1 probe,
/// USB string "S-TECS MODERN THROTTLE MINI PLUS", vendor "VKB-Sim (C) Alex Oz 2023").
/// Expanded-button variant of the Modern Throttle Mini (`VKB_STECS_MODERN_THROTTLE_MINI_PID`
/// 0x012B).
pub const VKB_STECS_MODERN_THROTTLE_MINI_PLUS_PID: u16 = 0x012C;

/// USB Product ID for the VKB S-TECS Modern Throttle Standard Stem (2023 generation).
///
/// Confirmed: VID 0x231D (VKB), PID 0x012D — linux-hardware.org (4 probes,
/// USB string "S-TECS MODERN THROTTLE STANDARD STEM", vendor "VKB-Sim (C) Alex Oz 2023").
pub const VKB_STECS_MODERN_THROTTLE_STANDARD_STEM_PID: u16 = 0x012D;

/// USB Vendor ID for all WinWing products.
///
/// Confirmed: VID 0x4098 — all WinWing USB HID devices (Orion series, UFC, SKYWALKER).
pub const WINWING_VENDOR_ID: u16 = 0x4098;

/// USB Product ID for the WinWing Orion 2 F/A-18C Throttle.
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBE62 — linux-hardware.org (2 probes,
/// USB string "Orion Throttle Base II + F18 HANDLE").
pub const WINWING_ORION2_F18_THROTTLE_PID: u16 = 0xBE62;

/// USB Product ID for the WinWing Orion 2 F/A-18C Stick.
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBE63 — community-reported, matching
/// F/A-18C JGRIP grip paired with Orion 2 base.
pub const WINWING_ORION2_F18_STICK_PID: u16 = 0xBE63;

/// USB Product ID for the WinWing TFRP Rudder Pedals.
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBE64 — community-reported.
pub const WINWING_TFRP_RUDDER_PID: u16 = 0xBE64;

/// USB Product ID for the WinWing Orion 2 F-16EX Stick (JGRIP-F16 grip).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBEA8 — linux-hardware.org (5 probes,
/// USB string "Orion Joystick Base 2 + JGRIP-F16").
pub const WINWING_ORION2_F16EX_STICK_PID: u16 = 0xBEA8;

/// USB Product ID for the WinWing Orion 2 F-16EX Stick with Z-axis (JGRIP-F16 + ZAXIS).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBEA9 — linux-hardware.org (USB string
/// "Orion Joystick Base 2 + ZAXIS + JGRIP-F16"). Reports an additional Z-rotation axis.
pub const WINWING_ORION2_F16EX_STICK_ZAXIS_PID: u16 = 0xBEA9;

/// USB Product ID for the WinWing SuperTaurus / F-15EX Throttle (dual-handle).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBD64 — linux-hardware.org (2 probes,
/// USB string "Orion Throttle Base II + F15EX HANDLE L + F15EX HANDLE R").
pub const WINWING_SUPER_TAURUS_THROTTLE_PID: u16 = 0xBD64;

/// USB Product ID for the WinWing UFC1 + HUD1 panel combo.
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBEDE — linux-hardware.org (1 probe,
/// USB string "UFC1 + HUD1"). Universal Flight Controller + HUD display panel.
pub const WINWING_UFC1_HUD1_PID: u16 = 0xBEDE;

/// USB Product ID for the WinWing SKYWALKER Metal Rudder Pedals.
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBEF0 — linux-hardware.org (5 probes,
/// USB string "SKYWALKER Metal Rudder Pedals").
pub const WINWING_SKYWALKER_RUDDER_PID: u16 = 0xBEF0;

/// USB Product ID for the WinWing URSA MINOR Fighter Flight Stick (Left-hand).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBC29 — linux-hardware.org (2 probes,
/// USB string "WINWING URSA MINOR FIGHTER FLIGHT STICK L"). Left-hand stick
/// for dual-stick configurations; right-hand variant is `WINWING_URSA_MINOR_STICK_R_PID`.
pub const WINWING_URSA_MINOR_STICK_L_PID: u16 = 0xBC29;

/// USB Product ID for the WinWing URSA MINOR Fighter Flight Stick (Right-hand).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBC2A — linux-hardware.org (vendor
/// page listing, USB string "WINWING URSA MINOR FIGHTER FLIGHT STICK R").
/// Right-hand counterpart to `WINWING_URSA_MINOR_STICK_L_PID`.
pub const WINWING_URSA_MINOR_STICK_R_PID: u16 = 0xBC2A;

/// USB Product ID for the WinWing F/A-18 Takeoff Panel.
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBE04 — linux-hardware.org (1 probe,
/// USB string "F18 TAKEOFF PANEL"). First-generation; updated variant is
/// `WINWING_F18_TAKEOFF_PANEL_2_PID` (0xBF05).
pub const WINWING_F18_TAKEOFF_PANEL_PID: u16 = 0xBE04;

/// USB Product ID for the WinWing F/A-18 Combat Ready Panel.
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBE05 — linux-hardware.org (1 probe,
/// USB string "F18 COMBAT READY PANEL"). Observed alongside the Takeoff Panel
/// (0xBE04) and ICP (0xBF06) on the same system.
pub const WINWING_F18_COMBAT_READY_PANEL_PID: u16 = 0xBE05;

/// USB Product ID for the WinWing Orion Base 1 + F/A-18 Grip.
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBE11 — linux-hardware.org (2 probes,
/// USB string "JOYSTICK BASE1 + F18 GRIP"). First-generation Orion base; the
/// second-generation equivalent is `WINWING_ORION2_F18_STICK_PID` (0xBE63).
pub const WINWING_ORION1_F18_STICK_PID: u16 = 0xBE11;

/// USB Product ID for the WinWing Orion 2 Base + F-16 Grip (first-generation PID).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBE48 — linux-hardware.org (2 probes,
/// USB string "JOYSTICK BASE2 + JGRIP-F16"). Earlier PID variant of the Orion 2
/// F-16EX stick; the updated variant is `WINWING_ORION2_F16EX_STICK_PID` (0xBEA8).
pub const WINWING_ORION2_F16_STICK_GEN1_PID: u16 = 0xBE48;

/// USB Product ID for the WinWing Orion Throttle Base II + F-16 Throttle Grip.
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBE68 — linux-hardware.org (3 probes,
/// USB string "Orion Throttle Base II + TGRIP-F16"). Single-throttle F-16 variant;
/// F/A-18C dual-throttle is `WINWING_ORION2_F18_THROTTLE_PID` (0xBE62).
pub const WINWING_ORION2_F16_THROTTLE_PID: u16 = 0xBE68;

/// USB Product ID for the WinWing F/A-18 Takeoff Panel 2 (second-generation).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBF05 — linux-hardware.org (1 probe,
/// USB string "F18 TAKEOFF PANEL 2"). Updated variant of
/// `WINWING_F18_TAKEOFF_PANEL_PID` (0xBE04).
pub const WINWING_F18_TAKEOFF_PANEL_2_PID: u16 = 0xBF05;

/// USB Product ID for the WinWing ICP (F-16 Integrated Control Panel).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBF06 — linux-hardware.org (1 probe,
/// USB string "ICP"). The F-16 Integrated Control Panel (ICP) serves the same
/// UFC role as the UFC1+HUD1 (`WINWING_UFC1_HUD1_PID` 0xBEDE) for F/A-18.
pub const WINWING_ICP_PANEL_PID: u16 = 0xBF06;

/// USB Product ID for the WinWing MFD1 Centre Panel (Multi-Function Display).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBEE0 — linux-hardware.org (1 probe,
/// USB string "MFD1-C"). Centre unit of the three-piece MFD set; the left and
/// right panels use `WINWING_MFD1_L_PID` (0xBEE1) and
/// `WINWING_MFD1_R_PID` (0xBEE2) respectively.
pub const WINWING_MFD1_C_PID: u16 = 0xBEE0;

/// USB Product ID for the WinWing MFD1 Left Panel (Multi-Function Display).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBEE1 — linux-hardware.org (1 probe,
/// USB string "MFD1-L"). Left unit of the three-piece MFD set.
pub const WINWING_MFD1_L_PID: u16 = 0xBEE1;

/// USB Product ID for the WinWing MFD1 Right Panel (Multi-Function Display).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBEE2 — linux-hardware.org (1 probe,
/// USB string "MFD1-R"). Right unit of the three-piece MFD set.
pub const WINWING_MFD1_R_PID: u16 = 0xBEE2;

/// USB Product ID for the WinWing SimApp Pro FCU (Airbus Flight Control Unit, standalone).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBB10 — schenlap/winwing_fcu and
/// schenlap/XSchenFly (two independent open-source projects listing this as the
/// standalone FCU variant). The FCU replicates the Airbus A320/A330 autopilot
/// panel with 7-segment LCD display. Combined variants: 0xBC1D (FCU+EFIS-L),
/// 0xBC1E (FCU+EFIS-R), 0xBA01 (FCU+EFIS-L+R).
pub const WINWING_SIMAPP_PRO_FCU_PID: u16 = 0xBB10;

/// USB Product ID for the WinWing SimApp Pro FCU + EFIS-L (FCU with captain-side EFIS).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBC1D — schenlap/winwing_fcu and
/// schenlap/XSchenFly; also referenced as "PFP 4" in schenlap/winwing_mcdu.py.
pub const WINWING_SIMAPP_PRO_FCU_EFIS_L_PID: u16 = 0xBC1D;

/// USB Product ID for the WinWing SimApp Pro FCU + EFIS-R (FCU with first-officer-side EFIS).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBC1E — schenlap/winwing_fcu and
/// schenlap/XSchenFly.
pub const WINWING_SIMAPP_PRO_FCU_EFIS_R_PID: u16 = 0xBC1E;

/// USB Product ID for the WinWing SimApp Pro FCU + EFIS-L + EFIS-R (full three-panel combo).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBA01 — schenlap/winwing_fcu and
/// schenlap/XSchenFly; also referenced as "PFP 7" in schenlap/winwing_mcdu.py.
pub const WINWING_SIMAPP_PRO_FCU_EFIS_COMBO_PID: u16 = 0xBA01;

/// USB Product ID for the WinWing SimApp Pro MCDU (Airbus MCDU, Captain position).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBB36 — two independent projects:
/// schenlap/winwing_mcdu.py ("MCDU - Captain") and Flixhummel/ioBroker.mcdu
/// (PRODUCT_ID = 0xbb36). FO variant is 0xBB3E; Observer variant is 0xBB3A.
pub const WINWING_MCDU_CAPT_PID: u16 = 0xBB36;

/// USB Product ID for the WinWing SimApp Pro MCDU (Airbus MCDU, First Officer position).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBB3E — schenlap/winwing_mcdu.py
/// ("MCDU - First Officer"). Identical hardware to Captain MCDU (0xBB36).
pub const WINWING_MCDU_FO_PID: u16 = 0xBB3E;

/// USB Product ID for the WinWing SimApp Pro MCDU (Airbus MCDU, Observer / third seat).
///
/// Confirmed: VID 0x4098 (WinWing), PID 0xBB3A — schenlap/winwing_mcdu.py
/// ("MCDU - Observer"). Identical hardware to Captain MCDU (0xBB36).
pub const WINWING_MCDU_OBS_PID: u16 = 0xBB3A;

/// USB Vendor ID for all Moza (Gudsen Technology) products.
///
/// Confirmed: VID 0x346E — from the-sz.com USB ID database and
/// open-source Moza integration projects (moza-ffb, python-mozaffb).
pub const MOZA_VENDOR_ID: u16 = 0x346E;

/// USB Product ID for the Moza AB9 Force Feedback Base (joystick / flight config).
///
/// Confirmed: VID 0x346E, PID 0x0005 — community-tested, validated in
/// `compat/devices/moza/ab9.yaml` and `flight-ffb-moza` crate property tests.
/// Note: R3 FFB Base uses PID 0x0002.
pub const MOZA_AB9_PID: u16 = 0x0005;

/// USB Product ID for the Moza R3 Force Feedback Base.
///
/// Confirmed: VID 0x346E, PID 0x0002 — community-reported alongside AB9.
pub const MOZA_R3_PID: u16 = 0x0002;

/// USB Vendor ID for MFG (Motion Fantasy Games).
///
/// Confirmed: VID 0x1551 — registered USB vendor ID for MFG Crosswind pedals.
pub const MFG_VENDOR_ID: u16 = 0x1551;

/// USB Product ID for the MFG Crosswind V1 Rudder Pedals (original generation, ~2012–2015).
///
/// Community-reported: VID 0x1551, PID 0x0001. Not found in linux-hardware.org.
pub const MFG_CROSSWIND_V1_PID: u16 = 0x0001;

/// USB Product ID for the MFG Crosswind V2 Rudder Pedals (~2015–2020).
///
/// Community-reported: VID 0x1551, PID 0x0002. Not found in linux-hardware.org.
pub const MFG_CROSSWIND_V2_PID: u16 = 0x0002;

/// USB Product ID for the MFG Crosswind V3 Rudder Pedals (current generation, ~2020+).
///
/// Community-reported: VID 0x1551, PID 0x0004. Not found in linux-hardware.org.
pub const MFG_CROSSWIND_V3_PID: u16 = 0x0004;

/// USB Vendor ID for RealSimulator.
///
/// Community-confirmed: VID 0x20FF — used by FSSB R3 Force Sensing Stick Base.
pub const REALSIMULATOR_VENDOR_ID: u16 = 0x20FF;

/// USB Product ID for the RealSimulator FSSB R3 (Force Sensing Stick Base).
///
/// Community-reported: VID 0x20FF, PID 0x0001. Covers FSSB R3, R3 Lite, and
/// R3 Lightning variants (all share the same USB descriptor). Not found in
/// linux-hardware.org probe database.
pub const REALSIMULATOR_FSSB_R3_PID: u16 = 0x0001;

/// USB Product ID for the RealSimulator FSSB-R3 Lighting (2022+ generation).
///
/// Community-reported: VID 0x20FF, PID 0x0002. The FSSB-R3 Lighting is the
/// 2022+ generation product with RGB LED indicators, acoustic feedback, asymmetric
/// force sensing, and RS grip support. Distinct from the older FSSB R3/Lite/Lightning
/// (PID 0x0001). Not found in linux-hardware.org probe database; verify with
/// lsusb on real hardware.
pub const REALSIMULATOR_FSSB_R3_LIGHTING_PID: u16 = 0x0002;

/// USB Product ID for the VKB Gladiator Mk.II (original Gladiator, ~2014–2017).
///
/// Confirmed: VID 0x231D (VKB), PID 0x0121 — linux-hardware.org (2 probes,
/// USB string "VKBsim Gladiator", vendor "www.vkb-sim.pro Alex Oz 2012-2017").
pub const VKB_GLADIATOR_MK2_PID: u16 = 0x0121;

/// USB Vendor ID for Brunner Elektronik AG.
///
/// Confirmed: VID 0x25BB — from the-sz.com USB ID database
/// (source: linux-usb.org, vendor registered as "Brunner Elektronik AG").
pub const BRUNNER_VENDOR_ID: u16 = 0x25BB;

/// USB Product ID for the Brunner CLS-E FFB Yoke (PRT.5105).
///
/// Confirmed: VID 0x25BB, PID 0x0063 — from the-sz.com USB ID database
/// (listed as "PRT.5105 [Yoke]", the part number for the CLS-E direct USB connection).
pub const BRUNNER_CLS_E_YOKE_PID: u16 = 0x0063;

/// USB Product ID for the Brunner CLS-E MK II Force Feedback Joystick (PRT.5094).
///
/// Confirmed: VID 0x25BB, PID 0x0067 — from the-sz.com USB ID database
/// (listed as "PRT.5094"). A reserved variant at PID 0x0068 also maps to PRT.5094.
/// Product name inferred from Brunner shop catalogue; part number confirmed from USB registry.
pub const BRUNNER_CLS_E_JOYSTICK_PID: u16 = 0x0067;

/// USB Product ID for the Brunner CLS-E NG Force Feedback Yoke (PRT.5127).
///
/// Confirmed: VID 0x25BB, PID 0x006D — from the-sz.com USB ID database
/// (listed as "PRT.5127"). The NG (Next Generation) Yoke is the entry-level successor
/// to the CLS-E MK II Yoke. Product name inferred from Brunner shop catalogue.
pub const BRUNNER_CLS_E_NG_YOKE_PID: u16 = 0x006D;

/// USB Product ID for the Brunner CLS-E MK II Force Feedback Rudder Pedals (PRT.5123).
///
/// Confirmed: VID 0x25BB, PID 0x006B — from the-sz.com USB ID database
/// (listed as "PRT.5123"). A reserved variant at PID 0x006C also maps to PRT.5123.
/// Product name inferred from Brunner shop catalogue; part number confirmed from USB registry.
pub const BRUNNER_CLS_E_RUDDER_PID: u16 = 0x006B;

/// USB Vendor ID for Microsoft Corporation.
///
/// Confirmed: VID 0x045E — USB Implementers Forum vendor registry;
/// used by the SideWinder joystick family and many other Microsoft peripherals.
pub const MICROSOFT_VENDOR_ID: u16 = 0x045E;

/// USB Product ID for the Microsoft SideWinder Force Feedback Pro.
///
/// Confirmed: VID 0x045E (Microsoft), PID 0x001B — Linux kernel `hid-microsoft.c`
/// (USB_DEVICE_ID_MICROSOFT_SIDEWINDER_FFB) and linux-hardware.org hardware probes.
/// Three-axis FFB joystick (~1998); 10-bit X/Y, 8-bit Rz/Throttle, 9 buttons, hat.
pub const SIDEWINDER_FFB_PRO_PID: u16 = 0x001B;

/// USB Product ID for the Microsoft SideWinder Force Feedback 2.
///
/// Confirmed: VID 0x045E (Microsoft), PID 0x001C — Linux kernel `hid-microsoft.c`
/// (USB_DEVICE_ID_MICROSOFT_SIDEWINDER_FFB2). Revised FFB joystick (~2000);
/// shares the identical 7-byte HID report layout with the FFB Pro (0x001B).
pub const SIDEWINDER_FFB2_PID: u16 = 0x001C;

/// USB Product ID for the Microsoft SideWinder Precision 2.
///
/// Confirmed: VID 0x045E (Microsoft), PID 0x002B — linux-hardware.org (multiple
/// probes, USB string "Microsoft SideWinder Precision 2"). Non-FFB budget joystick
/// (~2000); same 7-byte HID report layout as the FFB variants.
pub const SIDEWINDER_PRECISION_2_PID: u16 = 0x002B;

/// Microsoft SideWinder product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidewinderModel {
    /// SideWinder Force Feedback Pro. VID 0x045E, PID 0x001B.
    FfbPro,
    /// SideWinder Force Feedback 2. VID 0x045E, PID 0x001C.
    Ffb2,
    /// SideWinder Precision 2 (no force feedback). VID 0x045E, PID 0x002B.
    Precision2,
}

impl SidewinderModel {
    /// Human-readable product name for this model.
    pub fn name(self) -> &'static str {
        match self {
            Self::FfbPro => "Microsoft SideWinder Force Feedback Pro",
            Self::Ffb2 => "Microsoft SideWinder Force Feedback 2",
            Self::Precision2 => "Microsoft SideWinder Precision 2",
        }
    }

    /// Returns `true` if this model has a force feedback motor.
    pub fn has_ffb(self) -> bool {
        matches!(self, Self::FfbPro | Self::Ffb2)
    }
}

/// Returns `true` if this VID/PID combination belongs to a known SideWinder joystick.
pub fn is_sidewinder_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == MICROSOFT_VENDOR_ID
        && matches!(
            product_id,
            SIDEWINDER_FFB_PRO_PID | SIDEWINDER_FFB2_PID | SIDEWINDER_PRECISION_2_PID
        )
}

/// Returns the [`SidewinderModel`] for a known PID, or `None` for unknown PIDs.
pub fn sidewinder_model(product_id: u16) -> Option<SidewinderModel> {
    match product_id {
        SIDEWINDER_FFB_PRO_PID => Some(SidewinderModel::FfbPro),
        SIDEWINDER_FFB2_PID => Some(SidewinderModel::Ffb2),
        SIDEWINDER_PRECISION_2_PID => Some(SidewinderModel::Precision2),
        _ => None,
    }
}

pub const USAGE_PAGE_GENERIC_DESKTOP: u16 = 0x01;
pub const USAGE_PAGE_BUTTON: u16 = 0x09;

pub const USAGE_JOYSTICK: u16 = 0x04;
pub const USAGE_X: u16 = 0x30;
pub const USAGE_Y: u16 = 0x31;
pub const USAGE_Z: u16 = 0x32;
pub const USAGE_RX: u16 = 0x33;
pub const USAGE_RY: u16 = 0x34;
pub const USAGE_RZ: u16 = 0x35;
pub const USAGE_SLIDER: u16 = 0x36;
pub const USAGE_DIAL: u16 = 0x37;
pub const USAGE_WHEEL: u16 = 0x38;
pub const USAGE_HAT_SWITCH: u16 = 0x39;

pub const AXIS_MODE_WARNING: &str =
    "Rudder sources are merged. Switch to full-axis mode for separate yaw inputs.";
pub const DRIVER_NOTE: &str = "Missing axes or buttons? Install the Thrustmaster driver, confirm PC full-axis mode, and on Linux use a corrected HID descriptor setup (for example hid-tflight4) when generic HID exposes limited axes.";
pub const PC_MODE_NOTE_HOTAS_4: &str = "If full-axis inputs are missing, switch HOTAS 4 to PC HID mode (hardware switch or hold Share+Option+PS while plugging in).";
pub const PC_MODE_NOTE_HOTAS_ONE: &str = "If full-axis inputs are missing, switch HOTAS One to PC mode (Xbox/PC selector and Guide button procedure) before plugging in.";
pub const DEFAULT_MAPPING_NOTE_UNKNOWN: &str =
    "Default mapping assumes full-axis mode; verify axis mode before applying.";

/// T.16000M FCS product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T16000mModel {
    /// T.16000M FCS Joystick (standalone). VID 0x044F, PID 0xB10A.
    Joystick,
    /// TWCS Throttle (standalone or part of T.16000M HOTAS combo).
    /// VID 0x044F, PID 0xB687.
    TwcsThrottle,
}

impl T16000mModel {
    pub fn name(&self) -> &'static str {
        match self {
            T16000mModel::Joystick => "T.16000M FCS Joystick",
            T16000mModel::TwcsThrottle => "T.16000M FCS TWCS Throttle",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a known T.16000M device.
pub fn is_t16000m_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == THRUSTMASTER_VENDOR_ID
        && matches!(product_id, T16000M_JOYSTICK_PID | TWCS_THROTTLE_PID)
}

/// Returns the T.16000M model for a known PID, or `None` for unknown PIDs.
pub fn t16000m_model(product_id: u16) -> Option<T16000mModel> {
    match product_id {
        T16000M_JOYSTICK_PID => Some(T16000mModel::Joystick),
        TWCS_THROTTLE_PID => Some(T16000mModel::TwcsThrottle),
        _ => None,
    }
}

/// HOTAS Warthog product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarthogModel {
    /// HOTAS Warthog Joystick. VID 0x044F, PID 0x0402.
    Joystick,
    /// HOTAS Warthog Throttle. VID 0x044F, PID 0x0404.
    Throttle,
}

impl WarthogModel {
    pub fn name(&self) -> &'static str {
        match self {
            WarthogModel::Joystick => "HOTAS Warthog Joystick",
            WarthogModel::Throttle => "HOTAS Warthog Throttle",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a HOTAS Warthog device.
pub fn is_warthog_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == THRUSTMASTER_VENDOR_ID
        && matches!(product_id, WARTHOG_JOYSTICK_PID | WARTHOG_THROTTLE_PID)
}

/// Returns the Warthog model for a known PID, or `None` for unknown PIDs.
pub fn warthog_model(product_id: u16) -> Option<WarthogModel> {
    match product_id {
        WARTHOG_JOYSTICK_PID => Some(WarthogModel::Joystick),
        WARTHOG_THROTTLE_PID => Some(WarthogModel::Throttle),
        _ => None,
    }
}

/// HOTAS Cougar product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CougarHotasModel {
    /// HOTAS Cougar Stick. VID 0x044F, PID 0x0400.
    Stick,
}

impl CougarHotasModel {
    pub fn name(&self) -> &'static str {
        match self {
            CougarHotasModel::Stick => "HOTAS Cougar Stick",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a HOTAS Cougar device.
pub fn is_cougar_hotas_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == THRUSTMASTER_VENDOR_ID && product_id == COUGAR_HOTAS_STICK_PID
}

/// Returns the Cougar HOTAS model for a known PID, or `None` for unknown PIDs.
pub fn cougar_hotas_model(product_id: u16) -> Option<CougarHotasModel> {
    match product_id {
        COUGAR_HOTAS_STICK_PID => Some(CougarHotasModel::Stick),
        _ => None,
    }
}

/// Returns `true` if this VID/PID is the Thrustmaster USB Joystick (legacy).
pub fn is_thrustmaster_usb_joystick(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == THRUSTMASTER_VENDOR_ID && product_id == THRUSTMASTER_USB_JOYSTICK_PID
}

/// Thrustmaster TCA Airbus Edition product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcaAirbusModel {
    /// TCA Sidestick Airbus Edition (Pilot / left-hand captain's seat). VID 0x044F, PID 0x0405.
    SidestickPilot,
    /// TCA Sidestick Airbus Edition (Copilot / right-hand first-officer's seat). VID 0x044F, PID 0x0406.
    SidestickCopilot,
    /// TCA Quadrant Airbus Edition (engines 1 & 2). VID 0x044F, PID 0x0407.
    QuadrantEng12,
    /// TCA Quadrant Airbus Add-On (engines 3 & 4). VID 0x044F, PID 0x0408.
    QuadrantEng34,
}

impl TcaAirbusModel {
    pub fn name(&self) -> &'static str {
        match self {
            TcaAirbusModel::SidestickPilot => "TCA Sidestick Airbus Edition (Pilot)",
            TcaAirbusModel::SidestickCopilot => "TCA Sidestick Airbus Edition (Copilot)",
            TcaAirbusModel::QuadrantEng12 => "TCA Quadrant Airbus Edition (Eng 1&2)",
            TcaAirbusModel::QuadrantEng34 => "TCA Quadrant Airbus Add-On (Eng 3&4)",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a known TCA Airbus device.
pub fn is_tca_airbus_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == THRUSTMASTER_VENDOR_ID
        && matches!(
            product_id,
            TCA_SIDESTICK_AIRBUS_PILOT_PID
                | TCA_SIDESTICK_AIRBUS_COPILOT_PID
                | TCA_QUADRANT_AIRBUS_ENG12_PID
                | TCA_QUADRANT_AIRBUS_ENG34_PID
        )
}

/// Returns the TCA Airbus model for a known PID, or `None` for unknown PIDs.
pub fn tca_airbus_model(product_id: u16) -> Option<TcaAirbusModel> {
    match product_id {
        TCA_SIDESTICK_AIRBUS_PILOT_PID => Some(TcaAirbusModel::SidestickPilot),
        TCA_SIDESTICK_AIRBUS_COPILOT_PID => Some(TcaAirbusModel::SidestickCopilot),
        TCA_QUADRANT_AIRBUS_ENG12_PID => Some(TcaAirbusModel::QuadrantEng12),
        TCA_QUADRANT_AIRBUS_ENG34_PID => Some(TcaAirbusModel::QuadrantEng34),
        _ => None,
    }
}

/// Thrustmaster TCA Boeing Edition product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcaBoeingModel {
    /// TCA Yoke Boeing Edition. VID 0x044F, PID 0x0409.
    YokeBoeing,
    /// TCA Quadrant Boeing Edition (engines 1 & 2). VID 0x044F, PID 0x040A.
    QuadrantBoeing12,
    /// TCA Quadrant Boeing Add-On (engines 3 & 4). VID 0x044F, PID 0x040B.
    QuadrantBoeing34,
}

impl TcaBoeingModel {
    pub fn name(&self) -> &'static str {
        match self {
            TcaBoeingModel::YokeBoeing => "TCA Yoke Boeing Edition",
            TcaBoeingModel::QuadrantBoeing12 => "TCA Quadrant Boeing Edition (Eng 1&2)",
            TcaBoeingModel::QuadrantBoeing34 => "TCA Quadrant Boeing Add-On (Eng 3&4)",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a known TCA Boeing device.
pub fn is_tca_boeing_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == THRUSTMASTER_VENDOR_ID
        && matches!(
            product_id,
            TCA_YOKE_BOEING_PID | TCA_QUADRANT_BOEING_ENG12_PID | TCA_QUADRANT_BOEING_ENG34_PID
        )
}

/// Returns the TCA Boeing model for a known PID, or `None` for unknown PIDs.
pub fn tca_boeing_model(product_id: u16) -> Option<TcaBoeingModel> {
    match product_id {
        TCA_YOKE_BOEING_PID => Some(TcaBoeingModel::YokeBoeing),
        TCA_QUADRANT_BOEING_ENG12_PID => Some(TcaBoeingModel::QuadrantBoeing12),
        TCA_QUADRANT_BOEING_ENG34_PID => Some(TcaBoeingModel::QuadrantBoeing34),
        _ => None,
    }
}

/// VIRPIL Controls VPC product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirpilModel {
    /// VPC MongoosT-50CM2 Throttle. VID 0x3344, PID 0x8193.
    Cm2Throttle,
    /// VPC MongoosT-50CM2 Stick (left-hand). VID 0x3344, PID 0x4138.
    Cm2Stick,
    /// VPC Throttle CM3. VID 0x3344, PID 0x0194.
    Cm3Throttle,
    /// VPC MongoosT-50CM3 (right stick). VID 0x3344, PID 0x4130.
    MongoostStick,
    /// VPC Control Panel 1 (left panel). VID 0x3344, PID 0x025B.
    ControlPanel1,
    /// VPC Control Panel 2 (right panel). VID 0x3344, PID 0x0259.
    ControlPanel2,
    /// VPC Shark Panel. VID 0x3344, PID 0x825D.
    SharkPanel,
    /// VPC Constellation Alpha (left grip on CM3 base). VID 0x3344, PID 0x838F.
    ConstellationAlphaLeft,
    /// VPC Constellation Alpha Prime (left grip, standalone HOSAS). VID 0x3344, PID 0x0138.
    ConstellationAlphaPrimeLeft,
    /// VPC Constellation Alpha Prime (right grip, standalone HOSAS). VID 0x3344, PID 0x4139.
    ConstellationAlphaPrimeRight,
    /// VPC WarBRD (original right-hand base). VID 0x3344, PID 0x40CC.
    WarBrd,
    /// VPC WarBRD-D (revised right-hand base). VID 0x3344, PID 0x43F5.
    WarBrdD,
}

impl VirpilModel {
    pub fn name(&self) -> &'static str {
        match self {
            VirpilModel::Cm2Throttle => "VPC MongoosT-50CM2 Throttle",
            VirpilModel::Cm2Stick => "VPC MongoosT-50CM2 Stick",
            VirpilModel::Cm3Throttle => "VPC Throttle CM3",
            VirpilModel::MongoostStick => "VPC MongoosT-50CM3 Stick",
            VirpilModel::ControlPanel1 => "VPC Control Panel 1",
            VirpilModel::ControlPanel2 => "VPC Control Panel 2",
            VirpilModel::SharkPanel => "VPC Shark Panel",
            VirpilModel::ConstellationAlphaLeft => "VPC Constellation Alpha Left (CM3)",
            VirpilModel::ConstellationAlphaPrimeLeft => "VPC Constellation Alpha Prime Left",
            VirpilModel::ConstellationAlphaPrimeRight => "VPC Constellation Alpha Prime Right",
            VirpilModel::WarBrd => "VPC WarBRD Stick",
            VirpilModel::WarBrdD => "VPC WarBRD-D Stick",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a known VIRPIL device.
pub fn is_virpil_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == VIRPIL_VENDOR_ID
        && matches!(
            product_id,
            VIRPIL_CM2_THROTTLE_PID
                | VIRPIL_CM2_STICK_PID
                | VIRPIL_CM3_THROTTLE_PID
                | VIRPIL_MONGOOST_STICK_PID
                | VIRPIL_PANEL1_PID
                | VIRPIL_PANEL2_PID
                | VIRPIL_SHARK_PANEL_PID
                | VIRPIL_CONSTELLATION_ALPHA_LEFT_PID
                | VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID
                | VIRPIL_CONSTELLATION_ALPHA_PRIME_RIGHT_PID
                | VIRPIL_WARBRD_PID
                | VIRPIL_WARBRD_D_PID
        )
}

/// Returns the VIRPIL model for a known PID, or `None` for unknown PIDs.
pub fn virpil_model(product_id: u16) -> Option<VirpilModel> {
    match product_id {
        VIRPIL_CM2_THROTTLE_PID => Some(VirpilModel::Cm2Throttle),
        VIRPIL_CM2_STICK_PID => Some(VirpilModel::Cm2Stick),
        VIRPIL_CM3_THROTTLE_PID => Some(VirpilModel::Cm3Throttle),
        VIRPIL_MONGOOST_STICK_PID => Some(VirpilModel::MongoostStick),
        VIRPIL_PANEL1_PID => Some(VirpilModel::ControlPanel1),
        VIRPIL_PANEL2_PID => Some(VirpilModel::ControlPanel2),
        VIRPIL_SHARK_PANEL_PID => Some(VirpilModel::SharkPanel),
        VIRPIL_CONSTELLATION_ALPHA_LEFT_PID => Some(VirpilModel::ConstellationAlphaLeft),
        VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID => Some(VirpilModel::ConstellationAlphaPrimeLeft),
        VIRPIL_CONSTELLATION_ALPHA_PRIME_RIGHT_PID => {
            Some(VirpilModel::ConstellationAlphaPrimeRight)
        }
        VIRPIL_WARBRD_PID => Some(VirpilModel::WarBrd),
        VIRPIL_WARBRD_D_PID => Some(VirpilModel::WarBrdD),
        _ => None,
    }
}

/// CH Products device family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChModel {
    /// CH Pro Throttle. VID 0x068E, PID 0x00F1.
    ProThrottle,
    /// CH Pro Pedals. VID 0x068E, PID 0x00F2.
    ProPedals,
    /// CH Fighterstick. VID 0x068E, PID 0x00F3.
    Fighterstick,
    /// CH Combat Stick. VID 0x068E, PID 0x00F4.
    CombatStick,
    /// CH Flight Sim Eclipse Yoke. VID 0x068E, PID 0x0051.
    EclipseYoke,
    /// CH Flight Sim Yoke. VID 0x068E, PID 0x00FF.
    FlightYoke,
}

impl ChModel {
    pub fn name(&self) -> &'static str {
        match self {
            ChModel::ProThrottle => "CH Pro Throttle",
            ChModel::ProPedals => "CH Pro Pedals",
            ChModel::Fighterstick => "CH Fighterstick",
            ChModel::CombatStick => "CH Combat Stick",
            ChModel::EclipseYoke => "CH Flight Sim Eclipse Yoke",
            ChModel::FlightYoke => "CH Flight Sim Yoke",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a known CH Products device.
pub fn is_ch_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == CH_VENDOR_ID
        && matches!(
            product_id,
            CH_PRO_THROTTLE_PID
                | CH_PRO_PEDALS_PID
                | CH_FIGHTERSTICK_PID
                | CH_COMBAT_STICK_PID
                | CH_ECLIPSE_YOKE_PID
                | CH_FLIGHT_YOKE_PID
        )
}

/// Returns the CH Products model for a known PID, or `None` for unknown PIDs.
pub fn ch_model(product_id: u16) -> Option<ChModel> {
    match product_id {
        CH_PRO_THROTTLE_PID => Some(ChModel::ProThrottle),
        CH_PRO_PEDALS_PID => Some(ChModel::ProPedals),
        CH_FIGHTERSTICK_PID => Some(ChModel::Fighterstick),
        CH_COMBAT_STICK_PID => Some(ChModel::CombatStick),
        CH_ECLIPSE_YOKE_PID => Some(ChModel::EclipseYoke),
        CH_FLIGHT_YOKE_PID => Some(ChModel::FlightYoke),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TFlightModel {
    HotasOne,
    Hotas4,
    /// T.Flight HOTAS X (PS4/PC). VID 0x044F, PID 0xB108.
    HotasX,
}

impl TFlightModel {
    pub fn name(&self) -> &'static str {
        match self {
            TFlightModel::HotasOne => "T.Flight HOTAS One",
            TFlightModel::Hotas4 => "T.Flight HOTAS 4",
            TFlightModel::HotasX => "T.Flight HOTAS X",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VkbStecsVariant {
    RightSpaceThrottleGripMini,
    LeftSpaceThrottleGripMini,
    RightSpaceThrottleGripMiniPlus,
    LeftSpaceThrottleGripMiniPlus,
    RightSpaceThrottleGripStandard,
    LeftSpaceThrottleGripStandard,
}

impl VkbStecsVariant {
    pub fn name(&self) -> &'static str {
        match self {
            VkbStecsVariant::RightSpaceThrottleGripMini => {
                "VKB STECS Right Space Throttle Grip Mini"
            }
            VkbStecsVariant::LeftSpaceThrottleGripMini => "VKB STECS Left Space Throttle Grip Mini",
            VkbStecsVariant::RightSpaceThrottleGripMiniPlus => {
                "VKB STECS Right Space Throttle Grip Mini+"
            }
            VkbStecsVariant::LeftSpaceThrottleGripMiniPlus => {
                "VKB STECS Left Space Throttle Grip Mini+"
            }
            VkbStecsVariant::RightSpaceThrottleGripStandard => {
                "VKB STECS Right Space Throttle Grip Standard"
            }
            VkbStecsVariant::LeftSpaceThrottleGripStandard => {
                "VKB STECS Left Space Throttle Grip Standard"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VkbGladiatorVariant {
    NxtEvoRight,
    NxtEvoLeft,
}

impl VkbGladiatorVariant {
    pub fn name(&self) -> &'static str {
        match self {
            VkbGladiatorVariant::NxtEvoRight => "VKB Gladiator NXT EVO Right",
            VkbGladiatorVariant::NxtEvoLeft => "VKB Gladiator NXT EVO Left",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisMode {
    Merged,
    Separate,
    Unknown,
}

impl AxisMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            AxisMode::Merged => "merged",
            AxisMode::Separate => "separate",
            AxisMode::Unknown => "unknown",
        }
    }
}

/// USB Vendor ID used by VPforce Rhino devices.
///
/// Note: 0x0483 belongs to STMicroelectronics (the MCU manufacturer).
/// VPforce does not hold a dedicated VID; this is documented in
/// `compat/devices/vpforce/rhino.yaml` (quirk: STM_VID).
pub const VPFORCE_VENDOR_ID: u16 = 0x0483;

/// USB Product ID for the VPforce Rhino FFB joystick base (revision 2).
pub const VPFORCE_RHINO_PID_V2: u16 = 0xA1C0;

/// USB Product ID for the VPforce Rhino FFB joystick base (revision 3 / Mk II).
pub const VPFORCE_RHINO_PID_V3: u16 = 0xA1C1;

/// VPforce device model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpforceModel {
    /// VPforce Rhino revision 2. VID 0x0483, PID 0xA1C0.
    RhinoV2,
    /// VPforce Rhino revision 3 (Mk II). VID 0x0483, PID 0xA1C1.
    RhinoV3,
}

impl VpforceModel {
    pub fn name(&self) -> &'static str {
        match self {
            VpforceModel::RhinoV2 => "VPforce Rhino (v2)",
            VpforceModel::RhinoV3 => "VPforce Rhino (v3 / Mk II)",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a known VPforce device.
pub fn is_vpforce_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == VPFORCE_VENDOR_ID
        && matches!(product_id, VPFORCE_RHINO_PID_V2 | VPFORCE_RHINO_PID_V3)
}

/// Returns the VPforce model for a known PID, or `None` for unknown PIDs.
pub fn vpforce_model(product_id: u16) -> Option<VpforceModel> {
    match product_id {
        VPFORCE_RHINO_PID_V2 => Some(VpforceModel::RhinoV2),
        VPFORCE_RHINO_PID_V3 => Some(VpforceModel::RhinoV3),
        _ => None,
    }
}

/// Moza (Gudsen Technology) FFB base product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MozaModel {
    /// Moza AB9 Force Feedback Base (joystick / flight config). VID 0x346E, PID 0x0005.
    Ab9,
    /// Moza R3 Force Feedback Base. VID 0x346E, PID 0x0002.
    R3,
}

impl MozaModel {
    pub fn name(&self) -> &'static str {
        match self {
            MozaModel::Ab9 => "Moza AB9 FFB Base",
            MozaModel::R3 => "Moza R3 FFB Base",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a known Moza device.
pub fn is_moza_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == MOZA_VENDOR_ID && matches!(product_id, MOZA_AB9_PID | MOZA_R3_PID)
}

/// Returns the Moza model for a known PID, or `None` for unknown PIDs.
pub fn moza_model(product_id: u16) -> Option<MozaModel> {
    match product_id {
        MOZA_AB9_PID => Some(MozaModel::Ab9),
        MOZA_R3_PID => Some(MozaModel::R3),
        _ => None,
    }
}

/// Brunner Elektronik AG FFB yoke product family models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrunnerModel {
    /// Brunner CLS-E Force Feedback Yoke (PRT.5105). VID 0x25BB, PID 0x0063.
    ClsE,
    /// Brunner CLS-E MK II Force Feedback Joystick (PRT.5094). VID 0x25BB, PID 0x0067.
    ClsEJoystick,
    /// Brunner CLS-E NG Force Feedback Yoke (PRT.5127). VID 0x25BB, PID 0x006D.
    ClsENgYoke,
    /// Brunner CLS-E MK II Force Feedback Rudder Pedals (PRT.5123). VID 0x25BB, PID 0x006B.
    ClsERudder,
}

impl BrunnerModel {
    pub fn name(&self) -> &'static str {
        match self {
            BrunnerModel::ClsE => "Brunner CLS-E FFB Yoke",
            BrunnerModel::ClsEJoystick => "Brunner CLS-E MK II FFB Joystick",
            BrunnerModel::ClsENgYoke => "Brunner CLS-E NG FFB Yoke",
            BrunnerModel::ClsERudder => "Brunner CLS-E MK II FFB Rudder Pedals",
        }
    }
}

/// Returns `true` if this VID/PID combination belongs to a known Brunner device.
pub fn is_brunner_device(vendor_id: u16, product_id: u16) -> bool {
    vendor_id == BRUNNER_VENDOR_ID
        && matches!(
            product_id,
            BRUNNER_CLS_E_YOKE_PID
                | BRUNNER_CLS_E_JOYSTICK_PID
                | BRUNNER_CLS_E_NG_YOKE_PID
                | BRUNNER_CLS_E_RUDDER_PID
        )
}

/// Returns the Brunner model for a known PID, or `None` for unknown PIDs.
pub fn brunner_model(product_id: u16) -> Option<BrunnerModel> {
    match product_id {
        BRUNNER_CLS_E_YOKE_PID => Some(BrunnerModel::ClsE),
        BRUNNER_CLS_E_JOYSTICK_PID => Some(BrunnerModel::ClsEJoystick),
        BRUNNER_CLS_E_NG_YOKE_PID => Some(BrunnerModel::ClsENgYoke),
        BRUNNER_CLS_E_RUDDER_PID => Some(BrunnerModel::ClsERudder),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisUsageSummary {
    pub has_x: bool,
    pub has_y: bool,
    pub has_rz: bool,
    pub slider_like_count: u8,
}

impl AxisUsageSummary {
    pub fn from_usages(usages: &[HidUsage]) -> Self {
        let mut summary = AxisUsageSummary {
            has_x: false,
            has_y: false,
            has_rz: false,
            slider_like_count: 0,
        };

        for usage in usages {
            if usage.usage_page != USAGE_PAGE_GENERIC_DESKTOP {
                continue;
            }

            match usage.usage {
                USAGE_X => summary.has_x = true,
                USAGE_Y => summary.has_y = true,
                USAGE_RZ => summary.has_rz = true,
                USAGE_SLIDER | USAGE_DIAL => {
                    summary.slider_like_count = summary.slider_like_count.saturating_add(1);
                }
                _ => {}
            }
        }

        summary
    }
}

pub fn axis_mode_from_summary(summary: &AxisUsageSummary) -> AxisMode {
    if !(summary.has_x && summary.has_y && summary.has_rz) {
        return AxisMode::Unknown;
    }

    if summary.slider_like_count >= 2 {
        AxisMode::Separate
    } else if summary.slider_like_count == 0 {
        AxisMode::Merged
    } else {
        AxisMode::Unknown
    }
}

pub fn axis_mode_from_usages(usages: &[HidUsage]) -> AxisMode {
    let summary = AxisUsageSummary::from_usages(usages);
    axis_mode_from_summary(&summary)
}

pub fn axis_mode_from_descriptor(descriptor: &[u8]) -> AxisMode {
    let usages = extract_usages(descriptor);
    axis_mode_from_usages(&usages)
}

pub fn axis_mode_from_device_info(device_info: &HidDeviceInfo) -> AxisMode {
    match device_info.report_descriptor.as_deref() {
        Some(descriptor) => axis_mode_from_descriptor(descriptor),
        None => AxisMode::Unknown,
    }
}

fn tflight_model_from_product_name(product_name: Option<&str>) -> Option<TFlightModel> {
    let name = product_name?.to_ascii_lowercase();
    if name.contains("hotas one") || name.contains("hotasone") {
        Some(TFlightModel::HotasOne)
    } else if name.contains("hotas 4") || name.contains("hotas4") {
        Some(TFlightModel::Hotas4)
    } else if name.contains("hotas x") || name.contains("hotasx") {
        Some(TFlightModel::HotasX)
    } else {
        None
    }
}

pub fn tflight_model(device_info: &HidDeviceInfo) -> Option<TFlightModel> {
    if device_info.vendor_id != THRUSTMASTER_VENDOR_ID {
        return None;
    }

    match device_info.product_id {
        TFLIGHT_HOTAS_ONE_PID => Some(TFlightModel::HotasOne),
        TFLIGHT_HOTAS_4_PID | TFLIGHT_HOTAS_4_PID_LEGACY => Some(TFlightModel::Hotas4),
        TFLIGHT_HOTAS_X_PID => Some(TFlightModel::HotasX),
        _ => tflight_model_from_product_name(device_info.product_name.as_deref()),
    }
}

/// Returns true if the HOTAS 4 was detected via the legacy PID.
///
/// This allows diagnostics/UI to note that the device may be running
/// older firmware. The legacy PID is still fully supported.
pub fn is_hotas4_legacy_pid(device_info: &HidDeviceInfo) -> bool {
    device_info.vendor_id == THRUSTMASTER_VENDOR_ID
        && device_info.product_id == TFLIGHT_HOTAS_4_PID_LEGACY
}

pub fn is_tflight_device(device_info: &HidDeviceInfo) -> bool {
    tflight_model(device_info).is_some()
}

pub fn axis_mode_warning(axis_mode: AxisMode) -> Option<&'static str> {
    if axis_mode == AxisMode::Merged {
        Some(AXIS_MODE_WARNING)
    } else {
        None
    }
}

pub fn driver_note() -> &'static str {
    DRIVER_NOTE
}

pub fn pc_mode_note(model: TFlightModel) -> &'static str {
    match model {
        TFlightModel::Hotas4 => PC_MODE_NOTE_HOTAS_4,
        TFlightModel::HotasOne => PC_MODE_NOTE_HOTAS_ONE,
        TFlightModel::HotasX => DEFAULT_MAPPING_NOTE_UNKNOWN,
    }
}

pub fn default_mapping_note(axis_mode: AxisMode) -> Option<&'static str> {
    if axis_mode == AxisMode::Unknown {
        Some(DEFAULT_MAPPING_NOTE_UNKNOWN)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AxisUsage {
    X,
    Y,
    Z,
    Rx,
    Ry,
    Rz,
    Slider0,
    Slider1,
    RzCombined,
}

impl fmt::Display for AxisUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AxisUsage::X => write!(f, "X"),
            AxisUsage::Y => write!(f, "Y"),
            AxisUsage::Z => write!(f, "Z"),
            AxisUsage::Rx => write!(f, "RX"),
            AxisUsage::Ry => write!(f, "RY"),
            AxisUsage::Rz => write!(f, "RZ"),
            AxisUsage::Slider0 => write!(f, "Slider0"),
            AxisUsage::Slider1 => write!(f, "Slider1"),
            AxisUsage::RzCombined => write!(f, "RZ (combined)"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalControl {
    Axis(AxisUsage),
    Hat,
}

impl fmt::Display for PhysicalControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PhysicalControl::Axis(axis) => write!(f, "{}", axis),
            PhysicalControl::Hat => write!(f, "Hat"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalControl {
    Roll,
    Pitch,
    Yaw,
    Throttle,
    Pov,
}

impl fmt::Display for LogicalControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogicalControl::Roll => write!(f, "Roll"),
            LogicalControl::Pitch => write!(f, "Pitch"),
            LogicalControl::Yaw => write!(f, "Yaw"),
            LogicalControl::Throttle => write!(f, "Throttle"),
            LogicalControl::Pov => write!(f, "POV"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlBinding {
    pub physical: PhysicalControl,
    pub logical: LogicalControl,
    pub note: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultMapping {
    pub bindings: &'static [ControlBinding],
}

impl DefaultMapping {
    pub fn as_hint_string(&self) -> String {
        let mut out = String::new();
        for (idx, binding) in self.bindings.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!("{}->{}", binding.physical, binding.logical));
            if let Some(note) = binding.note {
                out.push_str(" (");
                out.push_str(note);
                out.push(')');
            }
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AxisControl {
    pub usage: AxisUsage,
    pub name: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ButtonControl {
    pub index: u8,
    pub name: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EncoderControl {
    pub name: &'static str,
    pub cw_button: u8,
    pub ccw_button: u8,
    pub press_button: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DeviceControlMap {
    pub schema: &'static str,
    pub axes: &'static [AxisControl],
    pub buttons: &'static [ButtonControl],
    pub encoders: &'static [EncoderControl],
    pub notes: &'static [&'static str],
}

const DESCRIPTOR_DISCOVERY_SCHEMA: &str = "flight.hid-discovery/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DescriptorCounts {
    pub axes: usize,
    pub hats: usize,
    pub buttons: usize,
    pub other: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoveredAxis {
    pub usage_page: u16,
    pub usage: u16,
    pub index: u8,
    pub label: String,
    pub suggested_logical: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoveredHat {
    pub usage_page: u16,
    pub usage: u16,
    pub index: u8,
    pub label: String,
    pub suggested_logical: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoveredButton {
    pub usage_page: u16,
    pub usage: u16,
    pub index: u16,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DescriptorDiscovery {
    pub schema: &'static str,
    pub counts: DescriptorCounts,
    pub usages: Vec<HidUsage>,
    pub axes: Vec<DiscoveredAxis>,
    pub hats: Vec<DiscoveredHat>,
    pub buttons: Vec<DiscoveredButton>,
    pub notes: Vec<String>,
}

const DESCRIPTOR_DISCOVERY_NOTES: [&str; 2] = [
    "Derived from HID report descriptor usage tags; treat as best-effort.",
    "Prefer logical min/max and report sizes when building authoritative maps.",
];

const VKB_DISCOVERY_NOTES: [&str; 2] = [
    "VKBDevCfg can remap hats, ministicks, and axes; do not hardcode order.",
    "GNX modules may expose multiple HID devices or collections; group by serial or arrival time.",
];

const VKB_GLADIATOR_DISCOVERY_NOTES: [&str; 2] = [
    "Omni Throttle uses the same USB PID as Gladiator NXT EVO variants.",
    "Treat default mappings as hints; prefer descriptor-first discovery.",
];

fn axis_label_for_usage(usage: u16) -> Option<&'static str> {
    match usage {
        USAGE_X => Some("X"),
        USAGE_Y => Some("Y"),
        USAGE_Z => Some("Z"),
        USAGE_RX => Some("Rx"),
        USAGE_RY => Some("Ry"),
        USAGE_RZ => Some("Rz"),
        USAGE_SLIDER => Some("Slider"),
        USAGE_DIAL => Some("Dial"),
        USAGE_WHEEL => Some("Wheel"),
        _ => None,
    }
}

fn suggested_logical_for_axis(usage: u16) -> Option<&'static str> {
    match usage {
        USAGE_X => Some("roll"),
        USAGE_Y => Some("pitch"),
        USAGE_RZ => Some("yaw_candidate"),
        USAGE_SLIDER | USAGE_DIAL | USAGE_WHEEL => Some("throttle_candidate"),
        _ => None,
    }
}

fn suggested_logical_for_hat(usage: u16) -> Option<&'static str> {
    if usage == USAGE_HAT_SWITCH {
        Some("pov")
    } else {
        None
    }
}

fn push_note_lines(target: &mut Vec<String>, notes: &[&str]) {
    for note in notes {
        target.push((*note).to_string());
    }
}

pub fn descriptor_discovery_from_usages(usages: &[HidUsage]) -> DescriptorDiscovery {
    let mut axes = Vec::new();
    let mut hats = Vec::new();
    let mut buttons = Vec::new();
    let mut axis_index: u8 = 0;
    let mut hat_index: u8 = 0;

    for usage in usages {
        if usage.usage_page == USAGE_PAGE_GENERIC_DESKTOP {
            if usage.usage == USAGE_HAT_SWITCH {
                hats.push(DiscoveredHat {
                    usage_page: usage.usage_page,
                    usage: usage.usage,
                    index: hat_index,
                    label: "Hat switch".to_string(),
                    suggested_logical: suggested_logical_for_hat(usage.usage).map(str::to_string),
                });
                hat_index = hat_index.saturating_add(1);
                continue;
            }

            if let Some(label) = axis_label_for_usage(usage.usage) {
                axes.push(DiscoveredAxis {
                    usage_page: usage.usage_page,
                    usage: usage.usage,
                    index: axis_index,
                    label: label.to_string(),
                    suggested_logical: suggested_logical_for_axis(usage.usage).map(str::to_string),
                });
                axis_index = axis_index.saturating_add(1);
                continue;
            }
        }

        if usage.usage_page == USAGE_PAGE_BUTTON {
            let index = usage.usage;
            buttons.push(DiscoveredButton {
                usage_page: usage.usage_page,
                usage: usage.usage,
                index,
                label: format!("Button {}", index),
            });
        }
    }

    let counts = DescriptorCounts {
        axes: axes.len(),
        hats: hats.len(),
        buttons: buttons.len(),
        other: usages
            .len()
            .saturating_sub(axes.len() + hats.len() + buttons.len()),
    };

    let mut notes = Vec::new();
    push_note_lines(&mut notes, &DESCRIPTOR_DISCOVERY_NOTES);

    DescriptorDiscovery {
        schema: DESCRIPTOR_DISCOVERY_SCHEMA,
        counts,
        usages: usages.to_vec(),
        axes,
        hats,
        buttons,
        notes,
    }
}

pub fn descriptor_discovery_from_descriptor(descriptor: &[u8]) -> DescriptorDiscovery {
    let usages = extract_usages(descriptor);
    descriptor_discovery_from_usages(&usages)
}

pub fn descriptor_discovery_from_device_info(
    device_info: &HidDeviceInfo,
) -> Option<DescriptorDiscovery> {
    let descriptor = device_info.report_descriptor.as_deref()?;
    let mut discovery = descriptor_discovery_from_descriptor(descriptor);

    if device_info.vendor_id == VKB_VENDOR_ID {
        push_note_lines(&mut discovery.notes, &VKB_DISCOVERY_NOTES);
    }

    if is_vkb_gladiator_device(device_info) {
        push_note_lines(&mut discovery.notes, &VKB_GLADIATOR_DISCOVERY_NOTES);
    }

    Some(discovery)
}

const TFLIGHT_MAPPING_SEPARATE: [ControlBinding; 6] = [
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::X),
        logical: LogicalControl::Roll,
        note: None,
    },
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::Y),
        logical: LogicalControl::Pitch,
        note: Some("invert optional"),
    },
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::Slider0),
        logical: LogicalControl::Throttle,
        note: None,
    },
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::Rz),
        logical: LogicalControl::Yaw,
        note: Some("primary"),
    },
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::Slider1),
        logical: LogicalControl::Yaw,
        note: Some("alternate"),
    },
    ControlBinding {
        physical: PhysicalControl::Hat,
        logical: LogicalControl::Pov,
        note: None,
    },
];

const TFLIGHT_MAPPING_MERGED: [ControlBinding; 4] = [
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::X),
        logical: LogicalControl::Roll,
        note: None,
    },
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::Y),
        logical: LogicalControl::Pitch,
        note: Some("invert optional"),
    },
    ControlBinding {
        physical: PhysicalControl::Axis(AxisUsage::RzCombined),
        logical: LogicalControl::Yaw,
        note: Some("combined"),
    },
    ControlBinding {
        physical: PhysicalControl::Hat,
        logical: LogicalControl::Pov,
        note: None,
    },
];

const VKB_GLADIATOR_CONTROL_MAP_SCHEMA: &str = "flight.device-map/1";
const VKB_STECS_CONTROL_MAP_SCHEMA: &str = "flight.device-map/1";
const VKB_GLADIATOR_NOTES: [&str; 5] = [
    "SCG map is descriptor-first; axis labels are semantic hints, not fixed firmware contracts.",
    "The mini-stick can toggle between POV mode and analog X/Y axes via center push.",
    "A1 hat mode behavior (8-way vs alternate 4-way) is profile-dependent in VKBDevCfg.",
    "Firmware may expose extra axes through additional HID interfaces/devices to stay within legacy DirectInput limits.",
    "Gladiator NXT EVO hardware has no force-feedback motor output channel.",
];
const VKB_STECS_NOTES: [&str; 3] = [
    "Button/axis labels are derived from Elite Dangerous buttonMap files.",
    "VKBDevCfg profiles can remap buttons, encoders, and virtual buttons.",
    "Virtual controller interfaces are exposed separately by firmware (VC0..VC2); host software should group by serial/physical path.",
];

const VKB_STECS_RIGHT_MINI_AXES: [AxisControl; 5] = [
    AxisControl {
        usage: AxisUsage::Rx,
        name: "STECS SpaceBrake",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "STECS Laser Power",
    },
    AxisControl {
        usage: AxisUsage::X,
        name: "STECS [x52prox]",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "STECS [x52proy]",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "STECS [x52z]",
    },
];

const VKB_STECS_RIGHT_MINI_BUTTONS: [ButtonControl; 29] = [
    ButtonControl {
        index: 1,
        name: "STECS Sys",
    },
    ButtonControl {
        index: 2,
        name: "STECS Start",
    },
    ButtonControl {
        index: 3,
        name: "STECS Mode 1",
    },
    ButtonControl {
        index: 4,
        name: "STECS Mode 2",
    },
    ButtonControl {
        index: 5,
        name: "STECS Mode 3",
    },
    ButtonControl {
        index: 6,
        name: "STECS Mode 4",
    },
    ButtonControl {
        index: 7,
        name: "STECS Mode 5",
    },
    ButtonControl {
        index: 8,
        name: "STECS B1",
    },
    ButtonControl {
        index: 9,
        name: "STECS Trigger",
    },
    ButtonControl {
        index: 10,
        name: "STECS B2",
    },
    ButtonControl {
        index: 11,
        name: "STECS Speed Lim [x360LThumb]",
    },
    ButtonControl {
        index: 12,
        name: "STECS Speed Lim [ps4PadU]",
    },
    ButtonControl {
        index: 13,
        name: "STECS Speed Lim [ps4PadD]",
    },
    ButtonControl {
        index: 14,
        name: "STECS PHat [x360LThumb]",
    },
    ButtonControl {
        index: 15,
        name: "STECS Hat1 [x360LThumb]",
    },
    ButtonControl {
        index: 16,
        name: "STECS PHat [ps4PadU]",
    },
    ButtonControl {
        index: 17,
        name: "STECS PHat [ps4PadD]",
    },
    ButtonControl {
        index: 18,
        name: "STECS PHat [ps4PadR]",
    },
    ButtonControl {
        index: 19,
        name: "STECS PHat [ps4PadL]",
    },
    ButtonControl {
        index: 20,
        name: "STECS Hat1 [ps4PadR]",
    },
    ButtonControl {
        index: 21,
        name: "STECS Hat1 [ps4PadL]",
    },
    ButtonControl {
        index: 22,
        name: "STECS Hat1 [ps4PadD]",
    },
    ButtonControl {
        index: 23,
        name: "STECS Hat1 [ps4PadU]",
    },
    ButtonControl {
        index: 24,
        name: "STECS H1 [ps4PadD]",
    },
    ButtonControl {
        index: 25,
        name: "STECS H1 [ps4PadU]",
    },
    ButtonControl {
        index: 26,
        name: "STECS H1 [x360LThumb]",
    },
    ButtonControl {
        index: 27,
        name: "STECS H2 [ps4PadL]",
    },
    ButtonControl {
        index: 28,
        name: "STECS H2 [ps4PadR]",
    },
    ButtonControl {
        index: 29,
        name: "STECS H2 [x360LThumb]",
    },
];

const VKB_STECS_LEFT_MINI_AXES: [AxisControl; 5] = [
    AxisControl {
        usage: AxisUsage::Rx,
        name: "STECS SpaceBrake",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "STECS Laser Power",
    },
    AxisControl {
        usage: AxisUsage::X,
        name: "STECS [x52prox]",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "STECS [x52proy]",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "STECS [x52z]",
    },
];

const VKB_STECS_LEFT_MINI_BUTTONS: [ButtonControl; 29] = [
    ButtonControl {
        index: 1,
        name: "STECS Sys",
    },
    ButtonControl {
        index: 2,
        name: "STECS Start",
    },
    ButtonControl {
        index: 3,
        name: "STECS Mode 1",
    },
    ButtonControl {
        index: 4,
        name: "STECS Mode 2",
    },
    ButtonControl {
        index: 5,
        name: "STECS Mode 3",
    },
    ButtonControl {
        index: 6,
        name: "STECS Mode 4",
    },
    ButtonControl {
        index: 7,
        name: "STECS Mode 5",
    },
    ButtonControl {
        index: 8,
        name: "STECS B1",
    },
    ButtonControl {
        index: 9,
        name: "STECS Trigger",
    },
    ButtonControl {
        index: 10,
        name: "STECS B2",
    },
    ButtonControl {
        index: 11,
        name: "STECS Speed Lim [x360LThumb]",
    },
    ButtonControl {
        index: 12,
        name: "STECS Speed Lim [ps4PadU]",
    },
    ButtonControl {
        index: 13,
        name: "STECS Speed Lim [ps4PadD]",
    },
    ButtonControl {
        index: 14,
        name: "STECS PHat [x360LThumb]",
    },
    ButtonControl {
        index: 15,
        name: "STECS Hat1 [x360LThumb]",
    },
    ButtonControl {
        index: 16,
        name: "STECS PHat [ps4PadU]",
    },
    ButtonControl {
        index: 17,
        name: "STECS PHat [ps4PadD]",
    },
    ButtonControl {
        index: 18,
        name: "STECS PHat [ps4PadL]",
    },
    ButtonControl {
        index: 19,
        name: "STECS PHat [ps4PadR]",
    },
    ButtonControl {
        index: 20,
        name: "STECS Hat1 [ps4PadL]",
    },
    ButtonControl {
        index: 21,
        name: "STECS Hat1 [ps4PadR]",
    },
    ButtonControl {
        index: 22,
        name: "STECS Hat1 [ps4PadD]",
    },
    ButtonControl {
        index: 23,
        name: "STECS Hat1 [ps4PadU]",
    },
    ButtonControl {
        index: 24,
        name: "STECS H1 [ps4PadD]",
    },
    ButtonControl {
        index: 25,
        name: "STECS H1 [ps4PadU]",
    },
    ButtonControl {
        index: 26,
        name: "STECS H1 [x360LThumb]",
    },
    ButtonControl {
        index: 27,
        name: "STECS H2 [ps4PadL]",
    },
    ButtonControl {
        index: 28,
        name: "STECS H2 [ps4PadR]",
    },
    ButtonControl {
        index: 29,
        name: "STECS H2 [x360LThumb]",
    },
];

const VKB_STECS_LEFT_MINI_PLUS_AXES: [AxisControl; 5] = [
    AxisControl {
        usage: AxisUsage::Rx,
        name: "LSTECS SpaceBrake",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "LSTECS Laser Power",
    },
    AxisControl {
        usage: AxisUsage::X,
        name: "LSTECS [x52prox]",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "LSTECS [x52proy]",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "LSTECS Throttle",
    },
];

const VKB_STECS_LEFT_MINI_PLUS_BUTTONS: [ButtonControl; 42] = [
    ButtonControl {
        index: 1,
        name: "LSTECS Sys",
    },
    ButtonControl {
        index: 2,
        name: "LSTECS Start",
    },
    ButtonControl {
        index: 3,
        name: "LSTECS Mode 1",
    },
    ButtonControl {
        index: 4,
        name: "LSTECS Mode 2",
    },
    ButtonControl {
        index: 5,
        name: "LSTECS Mode 3",
    },
    ButtonControl {
        index: 6,
        name: "LSTECS Mode 4",
    },
    ButtonControl {
        index: 7,
        name: "LSTECS Mode 5",
    },
    ButtonControl {
        index: 8,
        name: "LSTECS Rot CCW",
    },
    ButtonControl {
        index: 9,
        name: "LSTECS Rot CW",
    },
    ButtonControl {
        index: 10,
        name: "LSTECS Safe",
    },
    ButtonControl {
        index: 11,
        name: "LSTECS #1",
    },
    ButtonControl {
        index: 12,
        name: "LSTECS #2",
    },
    ButtonControl {
        index: 13,
        name: "LSTECS #3",
    },
    ButtonControl {
        index: 14,
        name: "LSTECS #4",
    },
    ButtonControl {
        index: 15,
        name: "LSTECS Armed",
    },
    ButtonControl {
        index: 16,
        name: "LSTECS Rot [ps4PadU]",
    },
    ButtonControl {
        index: 17,
        name: "LSTECS Rot [ps4PadD]",
    },
    ButtonControl {
        index: 18,
        name: "LSTECS Rot [ps4PadR]",
    },
    ButtonControl {
        index: 19,
        name: "LSTECS Rot [ps4PadL]",
    },
    ButtonControl {
        index: 20,
        name: "LSTECS Rot Click",
    },
    ButtonControl {
        index: 21,
        name: "LSTECS B1",
    },
    ButtonControl {
        index: 22,
        name: "LSTECS Trigger",
    },
    ButtonControl {
        index: 23,
        name: "LSTECS B2",
    },
    ButtonControl {
        index: 24,
        name: "LSTECS Speed Lim [x360LThumb]",
    },
    ButtonControl {
        index: 25,
        name: "LSTECS Speed Lim [ps4PadU]",
    },
    ButtonControl {
        index: 26,
        name: "LSTECS Speed Lim [ps4PadD]",
    },
    ButtonControl {
        index: 27,
        name: "LSTECS PHat [x360LThumb]",
    },
    ButtonControl {
        index: 28,
        name: "LSTECS Hat1 [x360LThumb]",
    },
    ButtonControl {
        index: 29,
        name: "LSTECS PHat [ps4PadU]",
    },
    ButtonControl {
        index: 30,
        name: "LSTECS PHat [ps4PadD]",
    },
    ButtonControl {
        index: 31,
        name: "LSTECS PHat [ps4PadL]",
    },
    ButtonControl {
        index: 32,
        name: "LSTECS PHat [ps4PadR]",
    },
    ButtonControl {
        index: 33,
        name: "LSTECS Hat1 [ps4PadL]",
    },
    ButtonControl {
        index: 34,
        name: "LSTECS Hat1 [ps4PadR]",
    },
    ButtonControl {
        index: 35,
        name: "LSTECS Hat1 [ps4PadD]",
    },
    ButtonControl {
        index: 36,
        name: "LSTECS Hat1 [ps4PadU]",
    },
    ButtonControl {
        index: 37,
        name: "LSTECS H1 [ps4PadD]",
    },
    ButtonControl {
        index: 38,
        name: "LSTECS H1 [ps4PadU]",
    },
    ButtonControl {
        index: 39,
        name: "LSTECS H1 [x360LThumb]",
    },
    ButtonControl {
        index: 40,
        name: "LSTECS H2 [ps4PadL]",
    },
    ButtonControl {
        index: 41,
        name: "LSTECS H2 [ps4PadR]",
    },
    ButtonControl {
        index: 42,
        name: "LSTECS H2 [x360LThumb]",
    },
];

const VKB_STECS_RIGHT_MINI_PLUS_AXES: [AxisControl; 5] = [
    AxisControl {
        usage: AxisUsage::Rx,
        name: "RSTECS SpaceBrake",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "RSTECS Laser Power",
    },
    AxisControl {
        usage: AxisUsage::X,
        name: "RSTECS [x52prox]",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "RSTECS [x52proy]",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "RSTECS Throttle",
    },
];

const VKB_STECS_RIGHT_MINI_PLUS_BUTTONS: [ButtonControl; 42] = [
    ButtonControl {
        index: 1,
        name: "RSTECS Sys",
    },
    ButtonControl {
        index: 2,
        name: "RSTECS Start",
    },
    ButtonControl {
        index: 3,
        name: "RSTECS Mode 1",
    },
    ButtonControl {
        index: 4,
        name: "RSTECS Mode 2",
    },
    ButtonControl {
        index: 5,
        name: "RSTECS Mode 3",
    },
    ButtonControl {
        index: 6,
        name: "RSTECS Mode 4",
    },
    ButtonControl {
        index: 7,
        name: "RSTECS Mode 5",
    },
    ButtonControl {
        index: 8,
        name: "RSTECS Rot CCW",
    },
    ButtonControl {
        index: 9,
        name: "RSTECS Rot CW",
    },
    ButtonControl {
        index: 10,
        name: "RSTECS Safe",
    },
    ButtonControl {
        index: 11,
        name: "RSTECS #1",
    },
    ButtonControl {
        index: 12,
        name: "RSTECS #2",
    },
    ButtonControl {
        index: 13,
        name: "RSTECS #3",
    },
    ButtonControl {
        index: 14,
        name: "RSTECS #4",
    },
    ButtonControl {
        index: 15,
        name: "RSTECS Armed",
    },
    ButtonControl {
        index: 16,
        name: "RSTECS Rot [ps4PadU]",
    },
    ButtonControl {
        index: 17,
        name: "RSTECS Rot [ps4PadD]",
    },
    ButtonControl {
        index: 18,
        name: "RSTECS Rot [ps4PadR]",
    },
    ButtonControl {
        index: 19,
        name: "RSTECS Rot [ps4PadL]",
    },
    ButtonControl {
        index: 20,
        name: "RSTECS Rot Click",
    },
    ButtonControl {
        index: 21,
        name: "RSTECS B1",
    },
    ButtonControl {
        index: 22,
        name: "RSTECS Trigger",
    },
    ButtonControl {
        index: 23,
        name: "RSTECS B2",
    },
    ButtonControl {
        index: 24,
        name: "RSTECS Speed Lim [x360LThumb]",
    },
    ButtonControl {
        index: 25,
        name: "RSTECS Speed Lim [ps4PadU]",
    },
    ButtonControl {
        index: 26,
        name: "RSTECS Speed Lim [ps4PadD]",
    },
    ButtonControl {
        index: 27,
        name: "RSTECS PHat [x360LThumb]",
    },
    ButtonControl {
        index: 28,
        name: "RSTECS Hat1 [x360LThumb]",
    },
    ButtonControl {
        index: 29,
        name: "RSTECS PHat [ps4PadU]",
    },
    ButtonControl {
        index: 30,
        name: "RSTECS PHat [ps4PadD]",
    },
    ButtonControl {
        index: 31,
        name: "RSTECS PHat [ps4PadR]",
    },
    ButtonControl {
        index: 32,
        name: "RSTECS PHat [ps4PadL]",
    },
    ButtonControl {
        index: 33,
        name: "RSTECS Hat1 [ps4PadR]",
    },
    ButtonControl {
        index: 34,
        name: "RSTECS Hat1 [ps4PadL]",
    },
    ButtonControl {
        index: 35,
        name: "RSTECS Hat1 [ps4PadD]",
    },
    ButtonControl {
        index: 36,
        name: "RSTECS Hat1 [ps4PadU]",
    },
    ButtonControl {
        index: 37,
        name: "RSTECS H1 [ps4PadD]",
    },
    ButtonControl {
        index: 38,
        name: "RSTECS H1 [ps4PadU]",
    },
    ButtonControl {
        index: 39,
        name: "RSTECS H1 [x360LThumb]",
    },
    ButtonControl {
        index: 40,
        name: "RSTECS H2 [ps4PadL]",
    },
    ButtonControl {
        index: 41,
        name: "RSTECS H2 [ps4PadR]",
    },
    ButtonControl {
        index: 42,
        name: "RSTECS H2 [x360LThumb]",
    },
];

const VKB_STECS_LEFT_STANDARD_AXES: [AxisControl; 5] = [
    AxisControl {
        usage: AxisUsage::Rx,
        name: "STECS - Space Brake",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "STECS - Laser Power",
    },
    AxisControl {
        usage: AxisUsage::X,
        name: "STECS - [x52prox]",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "STECS - [x52proy]",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "STECS - [x52z]",
    },
];

const VKB_STECS_LEFT_STANDARD_BUTTONS: [ButtonControl; 53] = [
    ButtonControl {
        index: 1,
        name: "STECS - Base Sys",
    },
    ButtonControl {
        index: 2,
        name: "STECS - Base Start",
    },
    ButtonControl {
        index: 3,
        name: "STECS - Base Mode 1",
    },
    ButtonControl {
        index: 4,
        name: "STECS - Base Mode 2",
    },
    ButtonControl {
        index: 5,
        name: "STECS - Base Mode 3",
    },
    ButtonControl {
        index: 6,
        name: "STECS - Base Mode 4",
    },
    ButtonControl {
        index: 7,
        name: "STECS - Base Mode 5",
    },
    ButtonControl {
        index: 8,
        name: "STECS - B1",
    },
    ButtonControl {
        index: 9,
        name: "STECS - Trigger",
    },
    ButtonControl {
        index: 10,
        name: "STECS - B2",
    },
    ButtonControl {
        index: 11,
        name: "STECS - Speed Push",
    },
    ButtonControl {
        index: 12,
        name: "STECS - Speed Up",
    },
    ButtonControl {
        index: 13,
        name: "STECS - Speed Down",
    },
    ButtonControl {
        index: 14,
        name: "STECS - Index Push",
    },
    ButtonControl {
        index: 15,
        name: "STECS - HAT1 Push",
    },
    ButtonControl {
        index: 16,
        name: "STECS - Index Fore",
    },
    ButtonControl {
        index: 17,
        name: "STECS - Index Back",
    },
    ButtonControl {
        index: 18,
        name: "STECS - Index Left",
    },
    ButtonControl {
        index: 19,
        name: "STECS - Index Right",
    },
    ButtonControl {
        index: 20,
        name: "STECS - HAT1 Back",
    },
    ButtonControl {
        index: 21,
        name: "STECS - HAT1 Fore",
    },
    ButtonControl {
        index: 22,
        name: "STECS - HAT1 Down",
    },
    ButtonControl {
        index: 23,
        name: "STECS - HAT1 Up",
    },
    ButtonControl {
        index: 24,
        name: "STECS - H1 Down",
    },
    ButtonControl {
        index: 25,
        name: "STECS - H1 Up",
    },
    ButtonControl {
        index: 26,
        name: "STECS - H1 Push",
    },
    ButtonControl {
        index: 27,
        name: "STECS - H2 Back",
    },
    ButtonControl {
        index: 28,
        name: "STECS - H2 Fore",
    },
    ButtonControl {
        index: 29,
        name: "STECS - H2 Push",
    },
    ButtonControl {
        index: 30,
        name: "STECS - STEM A1",
    },
    ButtonControl {
        index: 31,
        name: "STECS - STEM A2",
    },
    ButtonControl {
        index: 32,
        name: "STECS - STEM C1",
    },
    ButtonControl {
        index: 33,
        name: "STECS - STEM B1",
    },
    ButtonControl {
        index: 34,
        name: "STECS - STEM B2",
    },
    ButtonControl {
        index: 35,
        name: "STECS - STEM B3",
    },
    ButtonControl {
        index: 36,
        name: "STECS - STEM B4",
    },
    ButtonControl {
        index: 37,
        name: "STECS - STEM B5",
    },
    ButtonControl {
        index: 38,
        name: "STECS - STEM Sw1 Up",
    },
    ButtonControl {
        index: 39,
        name: "STECS - STEM Sw1 Mid",
    },
    ButtonControl {
        index: 40,
        name: "STECS - STEM Sw1 Down",
    },
    ButtonControl {
        index: 41,
        name: "STECS - STEM Sw2 Up",
    },
    ButtonControl {
        index: 42,
        name: "STECS - STEM Sw2 Mid",
    },
    ButtonControl {
        index: 43,
        name: "STECS - STEM Sw2 Down",
    },
    ButtonControl {
        index: 44,
        name: "STECS - STEM Tgl Up",
    },
    ButtonControl {
        index: 45,
        name: "STECS - STEM Tgl Down",
    },
    ButtonControl {
        index: 46,
        name: "STECS - STEM Enc1 CCW",
    },
    ButtonControl {
        index: 47,
        name: "STECS - STEM Enc1 CW",
    },
    ButtonControl {
        index: 48,
        name: "STECS - STEM Enc2 CCW",
    },
    ButtonControl {
        index: 49,
        name: "STECS - STEM Enc2 CW",
    },
    ButtonControl {
        index: 50,
        name: "STECS - STEM Enc1 Push",
    },
    ButtonControl {
        index: 51,
        name: "STECS - STEM Enc2 Push",
    },
    ButtonControl {
        index: 52,
        name: "STECS - STEM Flap Up",
    },
    ButtonControl {
        index: 53,
        name: "STECS - STEM Flap Down",
    },
];

const VKB_STECS_RIGHT_STANDARD_AXES: [AxisControl; 5] = [
    AxisControl {
        usage: AxisUsage::Rx,
        name: "STECS - Space Brake",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "STECS - Laser Power",
    },
    AxisControl {
        usage: AxisUsage::X,
        name: "STECS - [x52prox]",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "STECS - [x52proy]",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "STECS - [x52z]",
    },
];

const VKB_STECS_RIGHT_STANDARD_BUTTONS: [ButtonControl; 53] = [
    ButtonControl {
        index: 1,
        name: "STECS - Base Sys",
    },
    ButtonControl {
        index: 2,
        name: "STECS - Base Start",
    },
    ButtonControl {
        index: 3,
        name: "STECS - Base Mode 1",
    },
    ButtonControl {
        index: 4,
        name: "STECS - Base Mode 2",
    },
    ButtonControl {
        index: 5,
        name: "STECS - Base Mode 3",
    },
    ButtonControl {
        index: 6,
        name: "STECS - Base Mode 4",
    },
    ButtonControl {
        index: 7,
        name: "STECS - Base Mode 5",
    },
    ButtonControl {
        index: 8,
        name: "STECS - B1",
    },
    ButtonControl {
        index: 9,
        name: "STECS - Trigger",
    },
    ButtonControl {
        index: 10,
        name: "STECS - B2",
    },
    ButtonControl {
        index: 11,
        name: "STECS - Speed Push",
    },
    ButtonControl {
        index: 12,
        name: "STECS - Speed Up",
    },
    ButtonControl {
        index: 13,
        name: "STECS - Speed Down",
    },
    ButtonControl {
        index: 14,
        name: "STECS - Index Push",
    },
    ButtonControl {
        index: 15,
        name: "STECS - HAT1 Push",
    },
    ButtonControl {
        index: 16,
        name: "STECS - Index Fore",
    },
    ButtonControl {
        index: 17,
        name: "STECS - Index Back",
    },
    ButtonControl {
        index: 18,
        name: "STECS - Index Right",
    },
    ButtonControl {
        index: 19,
        name: "STECS - Index Left",
    },
    ButtonControl {
        index: 20,
        name: "STECS - HAT1 Back",
    },
    ButtonControl {
        index: 21,
        name: "STECS - HAT1 Fore",
    },
    ButtonControl {
        index: 22,
        name: "STECS - HAT1 Down",
    },
    ButtonControl {
        index: 23,
        name: "STECS - HAT1 Up",
    },
    ButtonControl {
        index: 24,
        name: "STECS - H1 Down",
    },
    ButtonControl {
        index: 25,
        name: "STECS - H1 Up",
    },
    ButtonControl {
        index: 26,
        name: "STECS - H1 Push",
    },
    ButtonControl {
        index: 27,
        name: "STECS - H2 Back",
    },
    ButtonControl {
        index: 28,
        name: "STECS - H2 Fore",
    },
    ButtonControl {
        index: 29,
        name: "STECS - H2 Push",
    },
    ButtonControl {
        index: 30,
        name: "STECS - STEM A1",
    },
    ButtonControl {
        index: 31,
        name: "STECS - STEM A2",
    },
    ButtonControl {
        index: 32,
        name: "STECS - STEM C1",
    },
    ButtonControl {
        index: 33,
        name: "STECS - STEM B1",
    },
    ButtonControl {
        index: 34,
        name: "STECS - STEM B2",
    },
    ButtonControl {
        index: 35,
        name: "STECS - STEM B3",
    },
    ButtonControl {
        index: 36,
        name: "STECS - STEM B4",
    },
    ButtonControl {
        index: 37,
        name: "STECS - STEM B5",
    },
    ButtonControl {
        index: 38,
        name: "STECS - STEM Sw1 Up",
    },
    ButtonControl {
        index: 39,
        name: "STECS - STEM Sw1 Mid",
    },
    ButtonControl {
        index: 40,
        name: "STECS - STEM Sw1 Down",
    },
    ButtonControl {
        index: 41,
        name: "STECS - STEM Sw2 Up",
    },
    ButtonControl {
        index: 42,
        name: "STECS - STEM Sw2 Mid",
    },
    ButtonControl {
        index: 43,
        name: "STECS - STEM Sw2 Down",
    },
    ButtonControl {
        index: 44,
        name: "STECS - STEM Tgl Up",
    },
    ButtonControl {
        index: 45,
        name: "STECS - STEM Tgl Down",
    },
    ButtonControl {
        index: 46,
        name: "STECS - STEM Enc1 CCW",
    },
    ButtonControl {
        index: 47,
        name: "STECS - STEM Enc1 CW",
    },
    ButtonControl {
        index: 48,
        name: "STECS - STEM Enc2 CCW",
    },
    ButtonControl {
        index: 49,
        name: "STECS - STEM Enc2 CW",
    },
    ButtonControl {
        index: 50,
        name: "STECS - STEM Enc1 Push",
    },
    ButtonControl {
        index: 51,
        name: "STECS - STEM Enc2 Push",
    },
    ButtonControl {
        index: 52,
        name: "STECS - STEM Flap Up",
    },
    ButtonControl {
        index: 53,
        name: "STECS - STEM Flap Down",
    },
];

const VKB_STECS_RIGHT_MINI_ENCODERS: [EncoderControl; 0] = [];
const VKB_STECS_LEFT_MINI_ENCODERS: [EncoderControl; 0] = [];
const VKB_STECS_LEFT_MINI_PLUS_ENCODERS: [EncoderControl; 1] = [EncoderControl {
    name: "LSTECS Rot",
    cw_button: 9,
    ccw_button: 8,
    press_button: Some(20),
}];
const VKB_STECS_RIGHT_MINI_PLUS_ENCODERS: [EncoderControl; 1] = [EncoderControl {
    name: "RSTECS Rot",
    cw_button: 9,
    ccw_button: 8,
    press_button: Some(20),
}];
const VKB_STECS_STANDARD_ENCODERS: [EncoderControl; 2] = [
    EncoderControl {
        name: "STECS - STEM Enc1",
        cw_button: 47,
        ccw_button: 46,
        press_button: Some(50),
    },
    EncoderControl {
        name: "STECS - STEM Enc2",
        cw_button: 49,
        ccw_button: 48,
        press_button: Some(51),
    },
];

const VKB_STECS_RIGHT_MINI_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_STECS_CONTROL_MAP_SCHEMA,
    axes: &VKB_STECS_RIGHT_MINI_AXES,
    buttons: &VKB_STECS_RIGHT_MINI_BUTTONS,
    encoders: &VKB_STECS_RIGHT_MINI_ENCODERS,
    notes: &VKB_STECS_NOTES,
};

const VKB_STECS_LEFT_MINI_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_STECS_CONTROL_MAP_SCHEMA,
    axes: &VKB_STECS_LEFT_MINI_AXES,
    buttons: &VKB_STECS_LEFT_MINI_BUTTONS,
    encoders: &VKB_STECS_LEFT_MINI_ENCODERS,
    notes: &VKB_STECS_NOTES,
};

const VKB_STECS_LEFT_MINI_PLUS_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_STECS_CONTROL_MAP_SCHEMA,
    axes: &VKB_STECS_LEFT_MINI_PLUS_AXES,
    buttons: &VKB_STECS_LEFT_MINI_PLUS_BUTTONS,
    encoders: &VKB_STECS_LEFT_MINI_PLUS_ENCODERS,
    notes: &VKB_STECS_NOTES,
};

const VKB_STECS_RIGHT_MINI_PLUS_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_STECS_CONTROL_MAP_SCHEMA,
    axes: &VKB_STECS_RIGHT_MINI_PLUS_AXES,
    buttons: &VKB_STECS_RIGHT_MINI_PLUS_BUTTONS,
    encoders: &VKB_STECS_RIGHT_MINI_PLUS_ENCODERS,
    notes: &VKB_STECS_NOTES,
};

const VKB_STECS_LEFT_STANDARD_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_STECS_CONTROL_MAP_SCHEMA,
    axes: &VKB_STECS_LEFT_STANDARD_AXES,
    buttons: &VKB_STECS_LEFT_STANDARD_BUTTONS,
    encoders: &VKB_STECS_STANDARD_ENCODERS,
    notes: &VKB_STECS_NOTES,
};

const VKB_STECS_RIGHT_STANDARD_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_STECS_CONTROL_MAP_SCHEMA,
    axes: &VKB_STECS_RIGHT_STANDARD_AXES,
    buttons: &VKB_STECS_RIGHT_STANDARD_BUTTONS,
    encoders: &VKB_STECS_STANDARD_ENCODERS,
    notes: &VKB_STECS_NOTES,
};

const VKB_GLADIATOR_RIGHT_SCG_AXES: [AxisControl; 8] = [
    AxisControl {
        usage: AxisUsage::X,
        name: "RSCG Stick X (Roll)",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "RSCG Stick Y (Pitch)",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "RSCG Twist (Yaw)",
    },
    AxisControl {
        usage: AxisUsage::Slider0,
        name: "RSCG Base Throttle Wheel",
    },
    AxisControl {
        usage: AxisUsage::Rx,
        name: "RSCG Mini-stick X (Analog)",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "RSCG Mini-stick Y (Analog)",
    },
    AxisControl {
        usage: AxisUsage::Rz,
        name: "RSCG Analog Trigger 1 (Profile)",
    },
    AxisControl {
        usage: AxisUsage::Slider1,
        name: "RSCG Analog Trigger 2 (Profile)",
    },
];

const VKB_GLADIATOR_LEFT_SCG_AXES: [AxisControl; 8] = [
    AxisControl {
        usage: AxisUsage::X,
        name: "LSCG Stick X (Roll)",
    },
    AxisControl {
        usage: AxisUsage::Y,
        name: "LSCG Stick Y (Pitch)",
    },
    AxisControl {
        usage: AxisUsage::Z,
        name: "LSCG Twist (Yaw)",
    },
    AxisControl {
        usage: AxisUsage::Slider0,
        name: "LSCG Base Throttle Wheel",
    },
    AxisControl {
        usage: AxisUsage::Rx,
        name: "LSCG Mini-stick X (Analog)",
    },
    AxisControl {
        usage: AxisUsage::Ry,
        name: "LSCG Mini-stick Y (Analog)",
    },
    AxisControl {
        usage: AxisUsage::Rz,
        name: "LSCG Analog Trigger 1 (Profile)",
    },
    AxisControl {
        usage: AxisUsage::Slider1,
        name: "LSCG Analog Trigger 2 (Profile)",
    },
];

const VKB_GLADIATOR_BUTTONS: [ButtonControl; 0] = [];
const VKB_GLADIATOR_ENCODERS: [EncoderControl; 0] = [];

const VKB_GLADIATOR_RIGHT_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_GLADIATOR_CONTROL_MAP_SCHEMA,
    axes: &VKB_GLADIATOR_RIGHT_SCG_AXES,
    buttons: &VKB_GLADIATOR_BUTTONS,
    encoders: &VKB_GLADIATOR_ENCODERS,
    notes: &VKB_GLADIATOR_NOTES,
};

const VKB_GLADIATOR_LEFT_CONTROL_MAP: DeviceControlMap = DeviceControlMap {
    schema: VKB_GLADIATOR_CONTROL_MAP_SCHEMA,
    axes: &VKB_GLADIATOR_LEFT_SCG_AXES,
    buttons: &VKB_GLADIATOR_BUTTONS,
    encoders: &VKB_GLADIATOR_ENCODERS,
    notes: &VKB_GLADIATOR_NOTES,
};

pub fn tflight_default_mapping(axis_mode: AxisMode) -> DefaultMapping {
    match axis_mode {
        AxisMode::Merged => DefaultMapping {
            bindings: &TFLIGHT_MAPPING_MERGED,
        },
        AxisMode::Separate | AxisMode::Unknown => DefaultMapping {
            bindings: &TFLIGHT_MAPPING_SEPARATE,
        },
    }
}

pub fn vkb_gladiator_variant(device_info: &HidDeviceInfo) -> Option<VkbGladiatorVariant> {
    if device_info.vendor_id != VKB_VENDOR_ID {
        return None;
    }

    match device_info.product_id {
        VKB_GLADIATOR_NXT_EVO_RIGHT_PID => Some(VkbGladiatorVariant::NxtEvoRight),
        VKB_GLADIATOR_NXT_EVO_LEFT_PID => Some(VkbGladiatorVariant::NxtEvoLeft),
        _ => None,
    }
}

pub fn is_vkb_gladiator_device(device_info: &HidDeviceInfo) -> bool {
    vkb_gladiator_variant(device_info).is_some()
}

pub fn vkb_gladiator_control_map(variant: VkbGladiatorVariant) -> &'static DeviceControlMap {
    match variant {
        VkbGladiatorVariant::NxtEvoRight => &VKB_GLADIATOR_RIGHT_CONTROL_MAP,
        VkbGladiatorVariant::NxtEvoLeft => &VKB_GLADIATOR_LEFT_CONTROL_MAP,
    }
}

pub fn vkb_stecs_variant(device_info: &HidDeviceInfo) -> Option<VkbStecsVariant> {
    if device_info.vendor_id != VKB_VENDOR_ID {
        return None;
    }

    match device_info.product_id {
        VKB_STECS_RIGHT_SPACE_MINI_PID => Some(VkbStecsVariant::RightSpaceThrottleGripMini),
        VKB_STECS_LEFT_SPACE_MINI_PID => Some(VkbStecsVariant::LeftSpaceThrottleGripMini),
        VKB_STECS_RIGHT_SPACE_MINI_PLUS_PID => {
            Some(VkbStecsVariant::RightSpaceThrottleGripMiniPlus)
        }
        VKB_STECS_LEFT_SPACE_MINI_PLUS_PID => Some(VkbStecsVariant::LeftSpaceThrottleGripMiniPlus),
        VKB_STECS_RIGHT_SPACE_STANDARD_PID => Some(VkbStecsVariant::RightSpaceThrottleGripStandard),
        VKB_STECS_LEFT_SPACE_STANDARD_PID => Some(VkbStecsVariant::LeftSpaceThrottleGripStandard),
        _ => None,
    }
}

pub fn is_vkb_stecs_device(device_info: &HidDeviceInfo) -> bool {
    vkb_stecs_variant(device_info).is_some()
}

/// Per-interface metadata for VKB Gladiator multi-interface layouts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VkbGladiatorInterfaceMetadata {
    /// HID path for this interface.
    pub device_path: String,
    /// Stable physical device identifier (serial when available).
    pub physical_id: String,
    /// Zero-based interface index in sorted path order.
    pub interface_index: u8,
    /// Number of HID interfaces discovered for the physical device.
    pub interface_count: u8,
}

/// Per-interface metadata for VKB STECS virtual-controller layouts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VkbStecsInterfaceMetadata {
    /// HID path for this interface.
    pub device_path: String,
    /// Stable physical device identifier (serial when available).
    pub physical_id: String,
    /// Zero-based virtual-controller index inside the physical group.
    pub virtual_controller_index: u8,
    /// Number of HID interfaces discovered for the physical device.
    pub interface_count: u8,
}

fn vkb_path_group_key(device_path: &str) -> String {
    let mut normalized = if let Some((base, _)) = device_path.split_once("#if") {
        base.to_ascii_lowercase()
    } else {
        device_path.to_ascii_lowercase()
    };

    if let Some(mi_pos) = normalized.find("&mi_")
        && normalized
            .get(mi_pos + 4..mi_pos + 6)
            .is_some_and(|suffix| suffix.chars().all(|ch| ch.is_ascii_hexdigit()))
    {
        normalized.replace_range(mi_pos..mi_pos + 6, "");
    }

    normalized
}

/// Build a stable physical-device id for Gladiator interfaces.
///
/// Serial number is preferred because it survives re-enumeration across ports.
/// If serial is unavailable, a normalized HID path stem is used.
pub fn vkb_gladiator_physical_id(device_info: &HidDeviceInfo) -> Option<String> {
    if !is_vkb_gladiator_device(device_info) {
        return None;
    }

    if let Some(serial) = device_info
        .serial_number
        .as_deref()
        .map(str::trim)
        .filter(|serial| !serial.is_empty())
    {
        return Some(format!(
            "vkb-gladiator:{:04x}:{:04x}:{}",
            device_info.vendor_id,
            device_info.product_id,
            serial.to_ascii_lowercase()
        ));
    }

    Some(format!(
        "vkb-gladiator:path:{}",
        vkb_path_group_key(&device_info.device_path)
    ))
}

/// Compute Gladiator interface ordering metadata for a device set.
///
/// Interfaces are grouped by physical id and sorted by HID path to provide
/// deterministic indexing (`IF0`, `IF1`, ...).
pub fn vkb_gladiator_interface_metadata<'a, I>(devices: I) -> Vec<VkbGladiatorInterfaceMetadata>
where
    I: IntoIterator<Item = &'a HidDeviceInfo>,
{
    let mut groups: BTreeMap<String, Vec<&HidDeviceInfo>> = BTreeMap::new();

    for device in devices {
        let Some(physical_id) = vkb_gladiator_physical_id(device) else {
            continue;
        };
        groups.entry(physical_id).or_default().push(device);
    }

    let mut metadata = Vec::new();
    for (physical_id, mut interfaces) in groups {
        interfaces.sort_by(|lhs, rhs| lhs.device_path.cmp(&rhs.device_path));
        let interface_count = u8::try_from(interfaces.len()).unwrap_or(u8::MAX);

        for (index, interface) in interfaces.iter().enumerate() {
            metadata.push(VkbGladiatorInterfaceMetadata {
                device_path: interface.device_path.clone(),
                physical_id: physical_id.clone(),
                interface_index: u8::try_from(index).unwrap_or(u8::MAX),
                interface_count,
            });
        }
    }

    metadata.sort_by(|lhs, rhs| lhs.device_path.cmp(&rhs.device_path));
    metadata
}

/// Build a stable physical-device id for STECS interfaces.
///
/// Serial number is preferred because it survives re-enumeration across ports.
/// If serial is unavailable, a normalized HID path stem is used.
pub fn vkb_stecs_physical_id(device_info: &HidDeviceInfo) -> Option<String> {
    if !is_vkb_stecs_device(device_info) {
        return None;
    }

    if let Some(serial) = device_info
        .serial_number
        .as_deref()
        .map(str::trim)
        .filter(|serial| !serial.is_empty())
    {
        return Some(format!(
            "vkb-stecs:{:04x}:{:04x}:{}",
            device_info.vendor_id,
            device_info.product_id,
            serial.to_ascii_lowercase()
        ));
    }

    Some(format!(
        "vkb-stecs:path:{}",
        vkb_path_group_key(&device_info.device_path)
    ))
}

/// Compute STECS virtual-controller ordering metadata for a device set.
///
/// Interfaces are grouped by physical id and sorted by HID path to provide
/// deterministic indexing (`VC0`, `VC1`, ...).
pub fn vkb_stecs_interface_metadata<'a, I>(devices: I) -> Vec<VkbStecsInterfaceMetadata>
where
    I: IntoIterator<Item = &'a HidDeviceInfo>,
{
    let mut groups: BTreeMap<String, Vec<&HidDeviceInfo>> = BTreeMap::new();

    for device in devices {
        let Some(physical_id) = vkb_stecs_physical_id(device) else {
            continue;
        };
        groups.entry(physical_id).or_default().push(device);
    }

    let mut metadata = Vec::new();
    for (physical_id, mut interfaces) in groups {
        interfaces.sort_by(|lhs, rhs| lhs.device_path.cmp(&rhs.device_path));
        let interface_count = u8::try_from(interfaces.len()).unwrap_or(u8::MAX);

        for (index, interface) in interfaces.iter().enumerate() {
            metadata.push(VkbStecsInterfaceMetadata {
                device_path: interface.device_path.clone(),
                physical_id: physical_id.clone(),
                virtual_controller_index: u8::try_from(index).unwrap_or(u8::MAX),
                interface_count,
            });
        }
    }

    metadata.sort_by(|lhs, rhs| lhs.device_path.cmp(&rhs.device_path));
    metadata
}

pub fn vkb_stecs_control_map(variant: VkbStecsVariant) -> &'static DeviceControlMap {
    match variant {
        VkbStecsVariant::RightSpaceThrottleGripMini => &VKB_STECS_RIGHT_MINI_CONTROL_MAP,
        VkbStecsVariant::LeftSpaceThrottleGripMini => &VKB_STECS_LEFT_MINI_CONTROL_MAP,
        VkbStecsVariant::RightSpaceThrottleGripMiniPlus => &VKB_STECS_RIGHT_MINI_PLUS_CONTROL_MAP,
        VkbStecsVariant::LeftSpaceThrottleGripMiniPlus => &VKB_STECS_LEFT_MINI_PLUS_CONTROL_MAP,
        VkbStecsVariant::RightSpaceThrottleGripStandard => &VKB_STECS_RIGHT_STANDARD_CONTROL_MAP,
        VkbStecsVariant::LeftSpaceThrottleGripStandard => &VKB_STECS_LEFT_STANDARD_CONTROL_MAP,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn load_hex_fixture(name: &str) -> Vec<u8> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("fixtures");
        path.push(name);
        let content = std::fs::read_to_string(path).expect("fixture should exist");
        content
            .split_whitespace()
            .filter_map(|token| {
                let token = token.trim_start_matches("0x");
                u8::from_str_radix(token, 16).ok()
            })
            .collect()
    }

    fn vkb_device(product_id: u16) -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: VKB_VENDOR_ID,
            product_id,
            serial_number: None,
            manufacturer: None,
            product_name: None,
            device_path: "/dev/test-vkb".to_string(),
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_JOYSTICK,
            report_descriptor: None,
        }
    }

    fn tflight_device(product_id: u16) -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: THRUSTMASTER_VENDOR_ID,
            product_id,
            serial_number: None,
            manufacturer: None,
            product_name: None,
            device_path: "/dev/test-tflight".to_string(),
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_JOYSTICK,
            report_descriptor: None,
        }
    }

    fn tflight_device_with_name(product_id: u16, product_name: Option<&str>) -> HidDeviceInfo {
        let mut info = tflight_device(product_id);
        info.product_name = product_name.map(str::to_string);
        info
    }

    #[test]
    fn test_axis_mode_from_usages() {
        let usages = vec![
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_X,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_Y,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_RZ,
            },
        ];

        assert_eq!(axis_mode_from_usages(&usages), AxisMode::Merged);

        let usages = vec![
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_X,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_Y,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_RZ,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_SLIDER,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_SLIDER,
            },
        ];

        assert_eq!(axis_mode_from_usages(&usages), AxisMode::Separate);
    }

    #[test]
    fn test_descriptor_discovery_from_usages() {
        let usages = vec![
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_X,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_Y,
            },
            HidUsage {
                usage_page: USAGE_PAGE_GENERIC_DESKTOP,
                usage: USAGE_HAT_SWITCH,
            },
            HidUsage {
                usage_page: USAGE_PAGE_BUTTON,
                usage: 1,
            },
            HidUsage {
                usage_page: USAGE_PAGE_BUTTON,
                usage: 2,
            },
            HidUsage {
                usage_page: 0xFF00,
                usage: 1,
            },
        ];

        let discovery = descriptor_discovery_from_usages(&usages);
        assert_eq!(discovery.counts.axes, 2);
        assert_eq!(discovery.counts.hats, 1);
        assert_eq!(discovery.counts.buttons, 2);
        assert_eq!(discovery.counts.other, 1);
        assert_eq!(discovery.axes[0].label, "X");
        assert_eq!(discovery.hats[0].label, "Hat switch");
        assert_eq!(discovery.buttons[0].label, "Button 1");
    }

    #[test]
    fn test_axis_mode_from_descriptor_fixtures() {
        let merged = load_hex_fixture("tflight_merged.hex");
        let separate = load_hex_fixture("tflight_separate.hex");

        assert_eq!(axis_mode_from_descriptor(&merged), AxisMode::Merged);
        assert_eq!(axis_mode_from_descriptor(&separate), AxisMode::Separate);
    }

    #[test]
    fn test_default_mapping_hint() {
        let mapping = tflight_default_mapping(AxisMode::Separate);
        let hint = mapping.as_hint_string();
        assert!(hint.contains("X->Roll"));
        assert!(hint.contains("Slider0->Throttle"));
        assert!(hint.contains("RZ->Yaw"));
    }

    #[test]
    fn test_tflight_model_detection() {
        let device_info = tflight_device(TFLIGHT_HOTAS_ONE_PID);
        assert_eq!(tflight_model(&device_info), Some(TFlightModel::HotasOne));
    }

    #[test]
    fn test_tflight_model_fallback_from_product_name() {
        let hotas4 = tflight_device_with_name(0xFFFF, Some("T.Flight HOTAS 4"));
        let hotas_one = tflight_device_with_name(0xABCD, Some("T.Flight HOTAS One"));
        let unknown = tflight_device_with_name(0xABCD, Some("Thrustmaster Warthog"));

        assert_eq!(tflight_model(&hotas4), Some(TFlightModel::Hotas4));
        assert_eq!(tflight_model(&hotas_one), Some(TFlightModel::HotasOne));
        assert_eq!(tflight_model(&unknown), None);
    }

    #[test]
    fn test_tflight_model_fallback_requires_thrustmaster_vendor() {
        let mut info = tflight_device_with_name(0xABCD, Some("T.Flight HOTAS 4"));
        info.vendor_id = LOGITECH_VENDOR_ID;
        assert_eq!(tflight_model(&info), None);
    }

    #[test]
    fn test_hotas4_primary_and_legacy_pid_detection() {
        let primary = tflight_device(TFLIGHT_HOTAS_4_PID);
        let legacy = tflight_device(TFLIGHT_HOTAS_4_PID_LEGACY);

        assert_eq!(tflight_model(&primary), Some(TFlightModel::Hotas4));
        assert_eq!(tflight_model(&legacy), Some(TFlightModel::Hotas4));
        assert!(!is_hotas4_legacy_pid(&primary));
        assert!(is_hotas4_legacy_pid(&legacy));
    }

    #[test]
    fn test_vkb_stecs_variant_detection() {
        let device_info = vkb_device(VKB_STECS_RIGHT_SPACE_MINI_PID);
        assert_eq!(
            vkb_stecs_variant(&device_info),
            Some(VkbStecsVariant::RightSpaceThrottleGripMini)
        );
        assert!(is_vkb_stecs_device(&device_info));

        let device_info = vkb_device(VKB_STECS_LEFT_SPACE_MINI_PID);
        assert_eq!(
            vkb_stecs_variant(&device_info),
            Some(VkbStecsVariant::LeftSpaceThrottleGripMini)
        );

        let device_info = vkb_device(VKB_STECS_RIGHT_SPACE_MINI_PLUS_PID);
        assert_eq!(
            vkb_stecs_variant(&device_info),
            Some(VkbStecsVariant::RightSpaceThrottleGripMiniPlus)
        );

        let device_info = vkb_device(VKB_STECS_LEFT_SPACE_MINI_PLUS_PID);
        assert_eq!(
            vkb_stecs_variant(&device_info),
            Some(VkbStecsVariant::LeftSpaceThrottleGripMiniPlus)
        );

        let device_info = vkb_device(VKB_STECS_RIGHT_SPACE_STANDARD_PID);
        assert_eq!(
            vkb_stecs_variant(&device_info),
            Some(VkbStecsVariant::RightSpaceThrottleGripStandard)
        );

        let device_info = vkb_device(VKB_STECS_LEFT_SPACE_STANDARD_PID);
        assert_eq!(
            vkb_stecs_variant(&device_info),
            Some(VkbStecsVariant::LeftSpaceThrottleGripStandard)
        );
    }

    #[test]
    fn test_vkb_gladiator_variant_detection() {
        let device_info = vkb_device(VKB_GLADIATOR_NXT_EVO_RIGHT_PID);
        assert_eq!(
            vkb_gladiator_variant(&device_info),
            Some(VkbGladiatorVariant::NxtEvoRight)
        );
        assert!(is_vkb_gladiator_device(&device_info));

        let device_info = vkb_device(VKB_GLADIATOR_NXT_EVO_LEFT_PID);
        assert_eq!(
            vkb_gladiator_variant(&device_info),
            Some(VkbGladiatorVariant::NxtEvoLeft)
        );
    }

    #[test]
    fn test_vkb_gladiator_control_map_contents() {
        let control_map = vkb_gladiator_control_map(VkbGladiatorVariant::NxtEvoRight);
        assert_eq!(control_map.schema, "flight.device-map/1");
        assert_eq!(control_map.axes.len(), 8);
        assert!(
            control_map
                .axes
                .iter()
                .any(|axis| axis.usage == AxisUsage::Rx && axis.name.contains("Mini-stick X"))
        );
        assert!(
            control_map.axes.iter().any(
                |axis| axis.usage == AxisUsage::Slider0 && axis.name.contains("Throttle Wheel")
            )
        );
        assert!(control_map.buttons.is_empty());
        assert!(control_map.encoders.is_empty());
        assert!(
            control_map
                .notes
                .iter()
                .any(|note| note.contains("descriptor-first"))
        );

        let left_map = vkb_gladiator_control_map(VkbGladiatorVariant::NxtEvoLeft);
        assert!(
            left_map
                .axes
                .iter()
                .any(|axis| axis.name.starts_with("LSCG"))
        );
    }

    #[test]
    fn test_vkb_stecs_control_map_contents() {
        let control_map = vkb_stecs_control_map(VkbStecsVariant::LeftSpaceThrottleGripMiniPlus);
        assert_eq!(control_map.schema, "flight.device-map/1");
        assert!(
            control_map
                .axes
                .iter()
                .any(|axis| axis.usage == AxisUsage::Z && axis.name.contains("Throttle"))
        );
        assert!(
            control_map
                .buttons
                .iter()
                .any(|button| button.index == 8 && button.name.contains("Rot CCW"))
        );
        assert_eq!(control_map.encoders.len(), 1);
        assert_eq!(control_map.encoders[0].cw_button, 9);
        assert_eq!(control_map.encoders[0].ccw_button, 8);
        assert_eq!(control_map.encoders[0].press_button, Some(20));

        let control_map = vkb_stecs_control_map(VkbStecsVariant::RightSpaceThrottleGripStandard);
        assert_eq!(control_map.encoders.len(), 2);
        assert_eq!(control_map.encoders[0].cw_button, 47);
        assert_eq!(control_map.encoders[0].ccw_button, 46);
        assert_eq!(control_map.encoders[0].press_button, Some(50));
    }

    #[test]
    fn test_vkb_stecs_interface_metadata_groups_by_serial() {
        let mut vc0 = vkb_device(VKB_STECS_RIGHT_SPACE_STANDARD_PID);
        vc0.serial_number = Some("ABC123".to_string());
        vc0.device_path = r"\\?\hid#vid_231d&pid_013c&mi_00#7".to_string();

        let mut vc1 = vkb_device(VKB_STECS_RIGHT_SPACE_STANDARD_PID);
        vc1.serial_number = Some("ABC123".to_string());
        vc1.device_path = r"\\?\hid#vid_231d&pid_013c&mi_01#7".to_string();

        let metadata = vkb_stecs_interface_metadata([&vc1, &vc0]);
        assert_eq!(metadata.len(), 2);

        assert_eq!(metadata[0].virtual_controller_index, 0);
        assert_eq!(metadata[1].virtual_controller_index, 1);
        assert_eq!(metadata[0].interface_count, 2);
        assert_eq!(metadata[1].interface_count, 2);
        assert_eq!(metadata[0].physical_id, metadata[1].physical_id);
    }

    #[test]
    fn test_vkb_gladiator_interface_metadata_groups_by_serial() {
        let mut if0 = vkb_device(VKB_GLADIATOR_NXT_EVO_RIGHT_PID);
        if0.serial_number = Some("SCG-ABC123".to_string());
        if0.device_path = r"\\?\hid#vid_231d&pid_0200&mi_00#7".to_string();

        let mut if1 = vkb_device(VKB_GLADIATOR_NXT_EVO_RIGHT_PID);
        if1.serial_number = Some("SCG-ABC123".to_string());
        if1.device_path = r"\\?\hid#vid_231d&pid_0200&mi_01#7".to_string();

        let metadata = vkb_gladiator_interface_metadata([&if1, &if0]);
        assert_eq!(metadata.len(), 2);

        assert_eq!(metadata[0].interface_index, 0);
        assert_eq!(metadata[1].interface_index, 1);
        assert_eq!(metadata[0].interface_count, 2);
        assert_eq!(metadata[1].interface_count, 2);
        assert_eq!(metadata[0].physical_id, metadata[1].physical_id);
    }

    #[test]
    fn test_vkb_stecs_physical_id_falls_back_to_path_stem() {
        let mut device = vkb_device(VKB_STECS_LEFT_SPACE_MINI_PLUS_PID);
        device.serial_number = None;
        device.device_path = "/dev/hidraw3#if1".to_string();

        assert_eq!(
            vkb_stecs_physical_id(&device),
            Some("vkb-stecs:path:/dev/hidraw3".to_string())
        );
    }

    #[test]
    fn test_vkb_gladiator_physical_id_falls_back_to_path_stem() {
        let mut device = vkb_device(VKB_GLADIATOR_NXT_EVO_LEFT_PID);
        device.serial_number = None;
        device.device_path = r"\\?\hid#vid_231d&pid_0201&mi_01#7".to_string();

        assert_eq!(
            vkb_gladiator_physical_id(&device),
            Some(r"vkb-gladiator:path:\\?\hid#vid_231d&pid_0201#7".to_string())
        );
    }

    #[test]
    fn test_warning_and_notes() {
        assert_eq!(axis_mode_warning(AxisMode::Merged), Some(AXIS_MODE_WARNING));
        assert!(axis_mode_warning(AxisMode::Separate).is_none());
        assert!(driver_note().contains("Thrustmaster"));
        assert!(pc_mode_note(TFlightModel::Hotas4).contains("Share+Option+PS"));
        assert!(pc_mode_note(TFlightModel::HotasOne).contains("Guide"));
        assert_eq!(
            default_mapping_note(AxisMode::Unknown),
            Some(DEFAULT_MAPPING_NOTE_UNKNOWN)
        );
    }

    #[test]
    fn test_tca_boeing_device_detection() {
        assert!(is_tca_boeing_device(
            THRUSTMASTER_VENDOR_ID,
            TCA_YOKE_BOEING_PID
        ));
        assert!(is_tca_boeing_device(
            THRUSTMASTER_VENDOR_ID,
            TCA_QUADRANT_BOEING_ENG12_PID
        ));
        assert!(is_tca_boeing_device(
            THRUSTMASTER_VENDOR_ID,
            TCA_QUADRANT_BOEING_ENG34_PID
        ));
        // Wrong vendor must not match.
        assert!(!is_tca_boeing_device(0x1234, TCA_YOKE_BOEING_PID));
        // Airbus PIDs must not match Boeing detector.
        assert!(!is_tca_boeing_device(
            THRUSTMASTER_VENDOR_ID,
            TCA_QUADRANT_AIRBUS_ENG12_PID
        ));
    }

    #[test]
    fn test_tca_boeing_model_from_pid() {
        assert_eq!(
            tca_boeing_model(TCA_YOKE_BOEING_PID),
            Some(TcaBoeingModel::YokeBoeing)
        );
        assert_eq!(
            tca_boeing_model(TCA_QUADRANT_BOEING_ENG12_PID),
            Some(TcaBoeingModel::QuadrantBoeing12)
        );
        assert_eq!(
            tca_boeing_model(TCA_QUADRANT_BOEING_ENG34_PID),
            Some(TcaBoeingModel::QuadrantBoeing34)
        );
        assert_eq!(tca_boeing_model(0x9999), None);
    }

    #[test]
    fn test_tca_boeing_model_names() {
        assert_eq!(TcaBoeingModel::YokeBoeing.name(), "TCA Yoke Boeing Edition");
        assert_eq!(
            TcaBoeingModel::QuadrantBoeing12.name(),
            "TCA Quadrant Boeing Edition (Eng 1&2)"
        );
        assert_eq!(
            TcaBoeingModel::QuadrantBoeing34.name(),
            "TCA Quadrant Boeing Add-On (Eng 3&4)"
        );
    }

    #[test]
    fn test_is_extreme_3d_pro_match() {
        assert!(is_extreme_3d_pro(LOGITECH_VENDOR_ID, EXTREME_3D_PRO_PID));
    }

    #[test]
    fn test_is_extreme_3d_pro_wrong_pid() {
        assert!(!is_extreme_3d_pro(LOGITECH_VENDOR_ID, 0x0000));
    }

    #[test]
    fn test_is_extreme_3d_pro_wrong_vendor() {
        assert!(!is_extreme_3d_pro(0x1234, EXTREME_3D_PRO_PID));
    }

    #[test]
    fn test_is_g940_joystick_match() {
        assert!(is_g940_joystick(LOGITECH_VENDOR_ID, G940_FLIGHT_SYSTEM_PID));
    }

    #[test]
    fn test_is_g940_joystick_wrong_pid() {
        assert!(!is_g940_joystick(LOGITECH_VENDOR_ID, 0x0000));
    }

    #[test]
    fn test_is_g940_joystick_wrong_vendor() {
        assert!(!is_g940_joystick(0x1234, G940_FLIGHT_SYSTEM_PID));
    }

    #[test]
    fn test_is_g940_throttle_match() {
        assert!(is_g940_throttle(LOGITECH_VENDOR_ID, G940_THROTTLE_PID));
    }

    #[test]
    fn test_is_g940_throttle_wrong_pid() {
        assert!(!is_g940_throttle(LOGITECH_VENDOR_ID, 0x0000));
    }

    #[test]
    fn test_is_g940_throttle_wrong_vendor() {
        assert!(!is_g940_throttle(0x1234, G940_THROTTLE_PID));
    }

    #[test]
    fn test_virpil_vendor_id_value() {
        assert_eq!(VIRPIL_VENDOR_ID, 0x3344);
    }

    #[test]
    fn test_vkb_vendor_id_value() {
        assert_eq!(VKB_VENDOR_ID, 0x231D);
    }

    #[test]
    fn test_vkb_gladiator_device_detected() {
        let device = vkb_device(VKB_GLADIATOR_NXT_EVO_RIGHT_PID);
        assert!(is_vkb_gladiator_device(&device));
    }

    #[test]
    fn test_vkb_gladiator_device_wrong_vendor_not_detected() {
        let mut device = vkb_device(VKB_GLADIATOR_NXT_EVO_RIGHT_PID);
        device.vendor_id = 0x1234;
        assert!(!is_vkb_gladiator_device(&device));
    }

    #[test]
    fn test_vkb_stecs_device_detected() {
        let device = vkb_device(VKB_STECS_LEFT_SPACE_MINI_PID);
        assert!(is_vkb_stecs_device(&device));
    }

    #[test]
    fn test_vkb_stecs_device_wrong_vendor_not_detected() {
        let mut device = vkb_device(VKB_STECS_LEFT_SPACE_MINI_PID);
        device.vendor_id = 0xAAAA;
        assert!(!is_vkb_stecs_device(&device));
    }
}
