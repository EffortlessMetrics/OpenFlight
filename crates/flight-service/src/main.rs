//! Flight Hub Service

use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting Flight Hub service");

    // TODO: Initialize service components

    Ok(())
}
