// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Client connection management

use flight_ipc::{ClientConfig, client::FlightClient};

pub struct ClientManager {
    config: ClientConfig,
    address: String,
}

impl ClientManager {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            address: "http://127.0.0.1:50051".to_string(),
        }
    }

    /// Create a new client manager with a custom server address.
    pub fn with_address(config: ClientConfig, address: String) -> Self {
        Self { config, address }
    }

    /// Return the configured server address.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Create a new client connection
    pub async fn get_client(&self) -> anyhow::Result<FlightClient> {
        FlightClient::connect_with_config(self.config.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to Flight Hub service: {}", e))
    }

    /// Create a new IpcClient connection (preferred over `get_client`).
    pub async fn get_ipc_client(&self) -> anyhow::Result<flight_ipc::client::IpcClient> {
        flight_ipc::client::IpcClient::connect_with_config(&self.address, self.config.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to Flight Hub service: {}", e))
    }
}
