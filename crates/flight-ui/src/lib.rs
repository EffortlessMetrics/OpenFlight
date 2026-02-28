// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight Hub UI implementation with dashboard, REST API, and WebSocket support.

pub mod api;
pub mod dashboard;
pub mod integration_docs;
pub mod settings;
pub mod websocket;

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
