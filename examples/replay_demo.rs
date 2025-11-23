//! Simple replay demonstration using only flight-replay crate
//!
//! Run with: cargo run -p flight-hub-examples --bin replay_demo --features replay

#[cfg(feature = "replay")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use flight_replay::{BlackboxConfig, BlackboxWriter};
    use std::path::PathBuf;
    use tracing::{Level, info};

    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting replay demo");

    // Create blackbox configuration
    let config = BlackboxConfig {
        output_dir: PathBuf::from("./replay_output"),
        enable_compression: true,
        buffer_size: 1024 * 1024, // 1MB buffer
        max_recording_duration: Some(std::time::Duration::from_secs(60)),
        ..Default::default()
    };

    // Create blackbox writer
    let writer = BlackboxWriter::new(config);

    info!("Blackbox writer created successfully");
    info!("Demo completed");

    Ok(())
}

#[cfg(not(feature = "replay"))]
fn main() {
    println!("This example requires the 'replay' feature. Run with: cargo run --features replay");
}
