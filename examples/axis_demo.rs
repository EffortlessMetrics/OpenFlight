//! Simple axis demonstration using only flight-axis crate
//!
//! Run with: cargo run -p flight-hub-examples --bin axis_demo --features axis

#[cfg(feature = "axis")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use flight_axis::{AxisEngine, ConflictDetectorConfig, EngineConfig};
    use tokio::time::{Duration, sleep};
    use tracing::{Level, info};

    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting axis demo");

    // Create engine configuration
    let config = EngineConfig {
        enable_rt_checks: true,
        max_frame_time_us: 500,
        enable_counters: true,
        enable_conflict_detection: true,
        conflict_detector_config: ConflictDetectorConfig::default(),
    };

    // Create axis engine
    let engine = AxisEngine::with_config("demo".to_string(), config);

    info!("Axis engine created successfully");
    info!("Demo completed");

    Ok(())
}

#[cfg(not(feature = "axis"))]
fn main() {
    println!("This example requires the 'axis' feature. Run with: cargo run --features axis");
}
