// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Client connection management

use flight_ipc::{client::FlightClient, ClientConfig};

pub struct ClientManager {
    config: ClientConfig,
}

impl ClientManager {
    pub fn new(config: ClientConfig) -> Self {
        Self { config }
    }
    
    /// Create a new client connection
    pub async fn get_client(&self) -> anyhow::Result<FlightClient> {
        FlightClient::connect_with_config(self.config.clone()).await
            .map_err(|e| anyhow::anyhow!("Failed to connect to Flight Hub service: {}", e))
    }
}