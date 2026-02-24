// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! OpenFlight Reference Hardware — HID protocol definitions.
//!
//! This crate defines the USB HID report format for the **OpenFlight Reference
//! FFB Stick**: an open-hardware force-feedback joystick design intended to
//! serve as a first-party reference device for the Flight Hub ecosystem.
//!
//! # Design goals
//!
//! * `#![no_std]` — usable in embedded firmware (STM32/RP2350) and on the host.
//! * Single source of truth for the wire format shared between firmware and
//!   the Flight Hub host driver.
//! * Plain `[u8; N]` serialisation; no allocator required.
//!
//! # HID descriptor summary
//!
//! | Report ID | Direction | Purpose              | Length  |
//! |-----------|-----------|----------------------|---------|
//! | 0x01      | IN (device→host) | Axis + button state | 16 bytes |
//! | 0x10      | OUT (host→device) | FFB force command  |  8 bytes |
//! | 0x20      | OUT (host→device) | LED / mode control |  4 bytes |
//! | 0xF0      | IN  (device→host) | Firmware version   |  8 bytes |
//!
//! # USB identifiers (placeholder — pending allocation)
//!
//! VID: `0x1209` (pid.codes open allocation), PID: `0xF170` (OpenFlight, provisional)
//!
//! # Reference design
//!
//! Schematics, PCB layout, BOM, and firmware source live in
//! `docs/reference/open-hardware/`. The MCU target is STM32G0B1 with a
//! dedicated USB-FS peripheral; fallback target is RP2350.

#![no_std]

pub mod firmware_version;
pub mod input_report;
pub mod led_report;
pub mod output_report;

pub use firmware_version::{FirmwareVersionReport, FIRMWARE_REPORT_ID};
pub use input_report::{InputReport, INPUT_REPORT_ID, INPUT_REPORT_LEN};
pub use led_report::{LedReport, LED_REPORT_ID, LED_REPORT_LEN};
pub use output_report::{FfbOutputReport, FFB_REPORT_ID, FFB_REPORT_LEN};

/// USB Vendor ID (pid.codes open allocation).
pub const VENDOR_ID: u16 = 0x1209;

/// USB Product ID for the OpenFlight Reference Stick (provisional).
pub const PRODUCT_ID: u16 = 0xF170;
