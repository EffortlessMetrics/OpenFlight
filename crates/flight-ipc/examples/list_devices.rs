//! Example: List connected devices
//!
//! This example demonstrates how to connect to the Flight Hub service
//! and list all connected devices.

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
    
    // List devices
    info!("Requesting device list...");
    
    match client.list_devices().await {
        Ok(devices) => {
            info!("Found {} devices:", devices.len());
            
            for (i, device) in devices.iter().enumerate() {
                info!("Device {}: {} ({})", i + 1, device.name, device.id);
                info!("  Type: {:?}", device.r#type());
                info!("  Status: {:?}", device.status());
                
                if let Some(capabilities) = &device.capabilities {
                    info!("  Capabilities:");
                    info!("    Force Feedback: {}", capabilities.supports_force_feedback);
                    info!("    Raw Torque: {}", capabilities.supports_raw_torque);
                    if capabilities.max_torque_nm > 0 {
                        info!("    Max Torque: {} Nm", capabilities.max_torque_nm);
                    }
                    info!("    Health Stream: {}", capabilities.has_health_stream);
                }
                
                if let Some(health) = &device.health {
                    info!("  Health:");
                    if health.temperature_celsius > 0.0 {
                        info!("    Temperature: {:.1}°C", health.temperature_celsius);
                    }
                    if health.current_amperes > 0.0 {
                        info!("    Current: {:.2}A", health.current_amperes);
                    }
                    if health.packet_loss_count > 0 {
                        info!("    Packet Loss: {}", health.packet_loss_count);
                    }
                    if !health.active_faults.is_empty() {
                        info!("    Active Faults: {:?}", health.active_faults);
                    }
                }
                
                info!(""); // Empty line for readability
            }
            
            if devices.is_empty() {
                info!("No devices currently connected.");
                info!("Make sure your flight controls are connected and recognized by the system.");
            }
        }
        Err(e) => {
            error!("Failed to list devices: {}", e);
            return Err(e.into());
        }
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
            }
        }
        Err(e) => {
            error!("Failed to get service info: {}", e);
        }
    }
    
    info!("Example completed successfully");
    
    Ok(())
}