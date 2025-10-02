//! Flight Hub Update System
//! 
//! Provides secure, reliable updates with channels, rollback, and delta compression.
//! Implements signed updates with automatic rollback on startup crashes.

pub mod channels;
pub mod delta;
pub mod integration_docs;
pub mod rollback;
pub mod signature;
pub mod updater;

pub use channels::{Channel, ChannelConfig};
pub use delta::{DeltaApplier, DeltaPatch};
pub use integration_docs::{IntegrationDocsManager, SimIntegrationDocs, ValidationReport};
pub use rollback::{RollbackManager, VersionInfo};
pub use signature::{SignatureVerifier, UpdateSignature};
pub use updater::{UpdateManager, UpdateConfig, UpdateResult};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum UpdateError {
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Delta patch error: {0}")]
    DeltaPatch(String),
    
    #[error("Rollback error: {0}")]
    Rollback(String),
    
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),
    
    #[error("Version validation failed: {0}")]
    VersionValidation(String),
}

pub type Result<T> = std::result::Result<T, UpdateError>;