//! Integration demonstration using multiple flight crates
//!
//! Run with: cargo run -p flight-hub-examples --bin integration_demo --features integration

#[cfg(feature = "integration")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use flight_axis::{AxisEngine, ConflictDetectorConfig, EngineConfig};
    use flight_replay::{BlackboxConfig, BlackboxWriter};
    use std::path::PathBuf;
    use tracing::{Level, info};

    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting integration demo");

    // Create axis engine
    let axis_config = EngineConfig {
        enable_rt_checks: true,
        max_frame_time_us: 500,
        enable_counters: true,
        enable_conflict_detection: true,
        conflict_detector_config: ConflictDetectorConfig::default(),
    };

    let axis_engine = AxisEngine::with_config("integration_demo".to_string(), axis_config);
    info!("Axis engine created");

    // Create blackbox writer
    let replay_config = BlackboxConfig {
        output_dir: PathBuf::from("./integration_output"),
        enable_compression: true,
        buffer_size: 1024 * 1024,
        max_recording_duration: Some(std::time::Duration::from_secs(60)),
        ..Default::default()
    };

    let blackbox_writer = BlackboxWriter::new(replay_config);
    info!("Blackbox writer created");

    info!("Integration demo completed successfully");

    Ok(())
}

#[cfg(not(feature = "integration"))]
fn main() {
    println!(
        "This example requires the 'integration' feature. Run with: cargo run --features integration"
    );
}
