//! Flight Hub UI implementation with integration documentation support

use std::path::PathBuf;

pub mod integration_docs;
pub mod settings;

pub struct FlightUi {
    pub integration_docs: integration_docs::IntegrationDocsManager,
}

impl FlightUi {
    pub fn new() -> Self {
        Self {
            integration_docs: integration_docs::IntegrationDocsManager::new(),
        }
    }
}

impl Default for FlightUi {
    fn default() -> Self {
        Self::new()
    }
}
