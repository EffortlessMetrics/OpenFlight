//! Flight Hub Core Library
//!
//! Provides core data structures, profile management, and shared utilities
//! for the Flight Hub flight simulation input management system.

pub mod error;
pub mod profile;
pub mod rules;
pub mod units;

pub use error::{FlightError, Result};
