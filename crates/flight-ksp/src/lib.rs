//! # flight-ksp
//!
//! Kerbal Space Program adapter for OpenFlight.
//!
//! Connects to a running KSP instance via the [kRPC] mod and streams vessel
//! telemetry into the Flight Hub bus.  The kRPC mod must be installed in KSP
//! and the server must be running before this adapter can connect.
//!
//! ## Minimal example
//!
//! ```no_run
//! use flight_ksp::{KspAdapter, KspConfig};
//!
//! #[tokio::main]
//! async fn main() {
//!     let adapter = KspAdapter::new(KspConfig::default());
//!     adapter.start().await;
//!
//!     // snapshot() returns None until the adapter connects and a vessel is active
//!     if let Some(snap) = adapter.current_snapshot().await {
//!         println!("altitude: {} ft", snap.environment.altitude);
//!     }
//!
//!     adapter.stop().await;
//! }
//! ```
//!
//! [kRPC]: https://krpc.github.io/krpc/

pub mod adapter;
pub mod connection;
pub mod controls;
pub mod error;
pub mod mapping;
pub mod protocol;

pub use adapter::{KspAdapter, KspConfig};
pub use controls::KspControls;
pub use error::KspError;
