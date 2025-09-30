//! Error types for Flight Hub

use thiserror::Error;

/// Flight Hub error types
#[derive(Error, Debug)]
pub enum FlightError {
    #[error("Profile validation error: {0}")]
    ProfileValidation(String),

    #[error("Rules validation error: {0}")]
    RulesValidation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Configuration(String),
}

/// Result type alias for Flight Hub operations
pub type Result<T> = std::result::Result<T, FlightError>;
