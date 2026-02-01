// SPDX-License-Identifier: MIT OR Apache-2.0

//! X52 Pro MFD (Multi-Function Display) support.
//!
//! **Protocol Status: UNVERIFIED**
//!
//! This implementation is based on community documentation and hypothesis.
//! The protocol has not been verified via USB capture analysis.
//!
//! Enable the `x52-mfd-experimental` feature to use this module.

mod x52_pro;

pub use x52_pro::X52ProMfd;
