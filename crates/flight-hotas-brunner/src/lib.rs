// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Brunner Elektronik AG CLS-E Force Feedback Yoke device support for OpenFlight.
//!
//! # VID/PID
//!
//! - **VID 0x25BB** (Brunner Elektronik AG): confirmed from
//!   [linux-usb.org USB ID registry](https://www.linux-usb.org/usb-ids.html).
//! - **PID 0x0063** (PRT.5105 [Yoke]): confirmed from the-sz.com USB ID database,
//!   listed as "PRT.5105 [Yoke]" — the Brunner part number for the CLS-E USB interface.
//!
//! # HID report format
//!
//! Based on Brunner SDK documentation and the USB PID/part mapping.
//! Hardware-in-the-loop (HIL) validation has not been performed.
//!
//! ```text
//! byte  0     : report_id (0x01)
//! bytes 1–2   : roll  / X axis (i16 LE, bipolar: –32768…+32767)
//! bytes 3–4   : pitch / Y axis (i16 LE, bipolar: –32768…+32767)
//! bytes 5–8   : button bytes (32 buttons, LSB-first)
//! total: 9 bytes minimum
//! ```

pub mod cls_e;

pub use flight_hid_support::device_support::{
    BRUNNER_CLS_E_JOYSTICK_PID, BRUNNER_CLS_E_NG_YOKE_PID, BRUNNER_CLS_E_RUDDER_PID,
    BRUNNER_CLS_E_YOKE_PID, BRUNNER_VENDOR_ID, BrunnerModel, brunner_model, is_brunner_device,
};

pub use cls_e::{
    CLS_E_MIN_REPORT_BYTES, ClsEAxes, ClsEButtons, ClsEInputState, ClsEParseError,
    parse_cls_e_report,
};
