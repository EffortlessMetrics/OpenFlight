// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Demonstration of the synthetic telemetry harness
//!
//! This example shows how to use the synthetic harness to feed
//! synthetic BusSnapshot data into the FFB engine for testing
//! without requiring a real simulator.

use std::time::Duration;

use flight_ffb::{FfbConfig, FfbMode, TelemetrySynthConfig};
use flight_replay::{SyntheticHarness, SyntheticHarnessConfig, TelemetryPattern};

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Synthetic Telemetry Harness Demo ===\n");

    // Test different telemetry patterns
    let patterns = vec![
        (TelemetryPattern::SteadyFlight, "Steady Level Flight"),
        (TelemetryPattern::GentleBank, "Gentle Banking Maneuver"),
        (
            TelemetryPattern::PitchOscillation,
            "Pitch Oscillation Pattern",
        ),
        (
            TelemetryPattern::CombinedManeuver,
            "Combined Roll and Pitch",
        ),
        (TelemetryPattern::HighGTurn, "High-G Turn"),
    ];

    for (pattern, description) in patterns {
        println!("Testing pattern: {}", description);
        println!("----------------------------------------");

        // Configure the harness
        let config = SyntheticHarnessConfig {
            update_rate_hz: 60,
            duration: Duration::from_secs(2),
            ffb_config: FfbConfig {
                max_torque_nm: 15.0,
                fault_timeout_ms: 50,
                interlock_required: false,
                mode: FfbMode::TelemetrySynth,
                device_path: None,
            },
            pattern,
        };

        // Create and configure the harness
        let mut harness = SyntheticHarness::new(config)?;

        // Enable telemetry synthesis
        let synth_config = TelemetrySynthConfig::default();
        harness
            .get_ffb_engine_mut()
            .enable_telemetry_synthesis(synth_config)?;

        // Run the harness
        let results = harness.run()?;

        // Display results
        println!("  Total frames:    {}", results.total_frames);
        println!("  Success count:   {}", results.success_count);
        println!("  Error count:     {}", results.error_count);
        println!("  Missed frames:   {}", results.missed_frames);
        println!("  Duration:        {:?}", results.duration);
        println!("  Success rate:    {:.2}%", results.success_rate() * 100.0);
        println!("  Actual FPS:      {:.2}", results.actual_frame_rate());
        println!("  Safety state:    {:?}", results.final_safety_state);
        println!(
            "  Status:          {}",
            if results.is_successful() {
                "✓ PASSED"
            } else {
                "✗ FAILED"
            }
        );
        println!();
    }

    println!("=== Demo Complete ===");
    Ok(())
}
