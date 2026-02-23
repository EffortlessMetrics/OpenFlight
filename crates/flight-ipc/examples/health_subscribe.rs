//! Example: Subscribe to health events
//!
//! This example demonstrates how to subscribe to health events from
//! the Flight Hub service and display them in real-time.

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
    }

    // Subscribe to health events
    info!("Subscribing to health events...");
    info!("Press Ctrl+C to stop");

    let _subscription = match client.subscribe_health().await {
        Ok(subscription) => subscription,
        Err(e) => {
            error!("Failed to subscribe to health events: {}", e);
            return Err(e.into());
        }
    };

    // The current client has a placeholder subscribe call and does not
    // yet expose a streaming interface in this example binary.
    info!("Health subscription request sent successfully.");
    info!("Streaming consumption will be enabled once client stream APIs are wired.");

    Ok(())
}
