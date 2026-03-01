// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Brunner Elektronik AG CLS-E Force Feedback driver for OpenFlight.
//!
//! Provides protocol types, FFB effect engine, safety envelope enforcement,
//! and per-aircraft FFB profiles for the Brunner CLS-E joystick base and
//! CLS-P FFB pedals.
//!
//! # Device identifiers
//!
//! | Device        | VID    | PID    |
//! |---------------|--------|--------|
//! | CLS-E Joystick | 0x25BB | 0x0063 |
//! | CLS-P Pedals   | 0x25BB | 0x0064 |
//!
//! # Architecture
//!
//! The Brunner CLS-E uses a two-layer communication model:
//!
//! 1. **USB HID** — standard joystick input reports (axes + buttons)
//! 2. **CLS2Sim protocol** — TCP/UDP command interface for force-feedback
//!    output, trim control, autopilot coupling, and device configuration.
//!
//! This crate models the CLS2Sim command protocol and provides a
//! device-independent effect engine with safety interlocks.

pub mod effects;
pub mod profiles;
pub mod protocol;
pub mod safety;

pub use effects::{
    BrunnerEffect, ConstantForceParams, DamperParams, EffectComposite, FrictionParams,
    PeriodicParams, PeriodicWaveform, SpringParams, compute_effect_force,
};
pub use profiles::{AircraftCategory, BrunnerProfile, default_cls_e_profile};
pub use protocol::{
    BRUNNER_VENDOR_ID, CLS_E_PID, CLS_P_PID, Cls2SimCommand, DeviceCapabilities, DeviceModel,
    ForceAxis,
};
pub use safety::{EmergencyStopReason, SafetyEnvelope, SafetyEvent, WatchdogState};
