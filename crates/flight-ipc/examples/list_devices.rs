//! Example: Get service information
//!
//! This example demonstrates how to connect to the Flight Hub service
//! and retrieve service information including version, status, and capabilities.

use flight_ipc::client::FlightClient;
use tracing::{error, info};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Connecting to Flight Hub service...");

    // Create client with default configuration
    let mut client = match FlightClient::connect().await {
        Ok(client) => {
            info!("Successfully connected to Flight Hub service");
            client
        }
        Err(e) => {
            error!("Failed to connect to Flight Hub service: {}", e);
            return Err(e.into());
        }
    };

    // Display negotiation result
    if let Some(negotiation) = client.negotiation_result() {
        info!("Server version: {}", negotiation.server_version);
        info!("Enabled features: {:?}", negotiation.enabled_features);
        info!("Transport type: {:?}", negotiation.transport_type);
    }

    // Get service information
    info!("Getting service information...");

    match client.get_service_info().await {
        Ok(service_info) => {
            info!("Service Information:");
            info!("  Version: {}", service_info.version);
            info!("  Uptime: {} seconds", service_info.uptime_seconds);
            info!("  Status: {:?}", service_info.status());

            if !service_info.capabilities.is_empty() {
                info!("  Capabilities:");
                for (key, value) in &service_info.capabilities {
                    info!("    {}: {}", key, value);
                }
            } else {
                info!("  No capabilities reported");
            }
        }
        Err(e) => {
            error!("Failed to get service info: {}", e);
            return Err(e.into());
        }
    }

    info!("Example completed successfully");

    Ok(())
}
