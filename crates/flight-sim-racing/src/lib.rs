// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Sim racing game adapter for OpenFlight.
//!
//! Provides normalised telemetry ingestion and force-feedback translation for
//! racing simulators such as Assetto Corsa and iRacing via a generic UDP bridge.
//!
//! ## Quick start
//!
//! ```rust
//! use flight_sim_racing::{parse_generic_udp, RacingFfbTranslator};
//!
//! // Build a minimal valid packet (in practice, received over UDP).
//! let mut pkt = vec![0u8; flight_sim_racing::MIN_PACKET_SIZE];
//! pkt[0..4].copy_from_slice(&flight_sim_racing::RACING_MAGIC.to_le_bytes());
//! pkt[4] = 0x01; // version
//!
//! let telemetry = parse_generic_udp(&pkt).expect("valid packet");
//! let ffb = RacingFfbTranslator::new().translate(&telemetry);
//! assert_eq!(ffb.lateral_force, 0.0);
//! ```

pub mod ffb_translator;
pub mod generic_udp;
pub mod telemetry;

pub use ffb_translator::{FfbOutput, RacingFfbTranslator};
pub use generic_udp::{
    MIN_PACKET_SIZE, RACING_MAGIC, RACING_UDP_PORT, RacingError, parse_generic_udp,
};
pub use telemetry::RacingTelemetry;
