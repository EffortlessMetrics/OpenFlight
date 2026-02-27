// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! VR headset head tracking adapter for OpenFlight.
//!
//! Provides a unified interface for polling head pose data from VR hardware
//! via the [`VrBackend`] trait. Supports mock backends for testing.

// flight-core is pulled in for workspace dependency alignment; the dep is
// reserved for future integration with the profile/bus system.
#[allow(unused_extern_crates)]
extern crate flight_core;

pub mod adapter;
pub mod mock;
pub mod pose;

pub use adapter::{VrAdapter, VrBackend, VrError};
pub use mock::MockVrBackend;
pub use pose::{HeadPose, TrackingQuality, VrSnapshot};
