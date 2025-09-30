//! Flight Hub Core Library
//!
//! Provides core data structures, profile management, and shared utilities
//! for the Flight Hub flight simulation input management system.

pub mod error;
pub mod profile;
pub mod rules;
pub mod units;
pub mod writers;

pub use error::{FlightError, Result};
pub use writers::{CurveConflictWriter, WritersConfig, WriteResult, VerificationResult, BackupInfo};
pub use profile::{CapabilityMode, CapabilityLimits, CapabilityContext};
