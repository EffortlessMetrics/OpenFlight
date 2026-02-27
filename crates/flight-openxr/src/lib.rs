// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! OpenXR head tracking adapter for OpenFlight.
//!
//! Reads HMD pose data via an [`OpenXrRuntime`] and exposes it as
//! [`HeadPose`] snapshots (6DOF: x/y/z in metres, yaw/pitch/roll in
//! radians).
//!
//! ## Design
//!
//! The crate does **not** depend on the `openxr` crate directly.  Instead,
//! integration is done through the [`OpenXrRuntime`] trait, making it easy to
//! substitute a [`MockRuntime`] in unit tests without any hardware or OpenXR
//! loader.
//!
//! ## Quick start
//!
//! ```rust
//! use flight_openxr::{OpenXrAdapter, MockRuntime, HeadPose, SessionState};
//!
//! let poses = vec![HeadPose { x: 0.1, y: 0.0, z: 0.0, yaw: 0.5, pitch: 0.0, roll: 0.0 }];
//! let runtime = MockRuntime::new(poses);
//! let mut adapter = OpenXrAdapter::new(runtime);
//! adapter.initialize().unwrap();
//! assert_eq!(adapter.state(), SessionState::Running);
//! let pose = adapter.poll();
//! assert!((pose.x - 0.1).abs() < 1e-6);
//! ```

// flight-core is a workspace peer; pulled in for workspace dependency alignment.
#[allow(unused_extern_crates)]
extern crate flight_core;

pub mod adapter;
pub mod pose;
pub mod session;

pub use adapter::OpenXrAdapter;
pub use pose::HeadPose;
pub use session::{MockRuntime, OpenXrError, OpenXrRuntime, SessionState};
