//! Example: Subscribe to health events
//!
//! This example demonstrates how to subscribe to health events from
//! the Flight Hub service and display them in real-time.

use flight_ipc::{client::FlightClient, proto::HealthEventType};
use tokio_stream::StreamExt;
use tracing::{error, info, warn};
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

    let mut health_stream = match client.subscribe_health().await {
        Ok(stream) => stream,
        Err(e) => {
            error!("Failed to subscribe to health events: {}", e);
            return Err(e.into());
        }
    };

    // Process health events
    let mut event_count = 0;

    while let Some(event_result) = health_stream.next().await {
        match event_result {
            Ok(event) => {
                event_count += 1;

                // Format timestamp
                let timestamp = chrono::DateTime::from_timestamp(event.timestamp, 0)
                    .map(|dt| dt.format("%H:%M:%S%.3f").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                // Format event based on type
                match event.r#type() {
                    HealthEventType::Info => {
                        info!("[{}] INFO: {}", timestamp, event.message);
                    }
                    HealthEventType::Warning => {
                        warn!("[{}] WARNING: {}", timestamp, event.message);
                    }
                    HealthEventType::Error => {
                        error!("[{}] ERROR: {}", timestamp, event.message);
                    }
                    HealthEventType::Fault => {
                        error!("[{}] FAULT: {}", timestamp, event.message);
                        if !event.error_code.is_empty() {
                            error!("  Error Code: {}", event.error_code);
                        }
                    }
                    HealthEventType::Performance => {
                        if let Some(perf) = &event.performance {
                            info!(
                                "[{}] PERFORMANCE: Jitter: {:.3}ms, HID Latency: {:.1}μs, Missed Ticks: {}, CPU: {:.1}%",
                                timestamp,
                                perf.jitter_p99_ms,
                                perf.hid_latency_p99_us,
                                perf.missed_ticks,
                                perf.cpu_usage_percent
                            );
                        } else {
                            info!("[{}] PERFORMANCE: {}", timestamp, event.message);
                        }
                    }
                    _ => {
                        info!("[{}] UNKNOWN: {}", timestamp, event.message);
                    }
                }

                // Display device information if available
                if !event.device_id.is_empty() {
                    info!("  Device: {}", event.device_id);
                }

                // Display metadata if available
                if !event.metadata.is_empty() {
                    info!("  Metadata:");
                    for (key, value) in &event.metadata {
                        info!("    {}: {}", key, value);
                    }
                }

                // Show progress every 10 events
                if event_count % 10 == 0 {
                    info!("Processed {} health events", event_count);
                }
            }
            Err(e) => {
                error!("Error receiving health event: {}", e);
                break;
            }
        }
    }

    info!(
        "Health subscription ended. Total events processed: {}",
        event_count
    );

    Ok(())
}
