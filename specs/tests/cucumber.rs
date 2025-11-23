// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! BDD test runner for Flight Hub specifications
//!
//! This test harness executes Gherkin scenarios defined in specs/features/
//! and validates them against the implementation.

use cucumber::World;

mod steps;

#[derive(Debug, Default, World)]
pub struct FlightWorld {
    // Axis processing state
    pub axis_pipeline: Option<AxisPipelineState>,
    pub scheduler_state: Option<SchedulerState>,
    pub latency_measurements: Vec<f64>,
    pub jitter_measurements: Vec<f64>,

    // Documentation validation state
    pub doc_path: Option<String>,
    pub doc_content: Option<String>,
    pub validation_errors: Vec<String>,
    pub doc_ids: Vec<String>,
    pub front_matter: Option<FrontMatter>,
}

#[derive(Debug)]
pub struct AxisPipelineState {
    pub num_axes: usize,
    pub telemetry_rate_hz: u32,
    pub processing_duration_secs: u64,
}

#[derive(Debug)]
pub struct SchedulerState {
    pub rate_hz: u32,
    pub measurement_duration_secs: u64,
    pub warmup_secs: u64,
}

#[derive(Debug, Clone)]
pub struct FrontMatter {
    pub doc_id: String,
    pub kind: String,
    pub area: String,
    pub status: String,
    pub links: Links,
}

#[derive(Debug, Clone, Default)]
pub struct Links {
    pub requirements: Vec<String>,
    pub tasks: Vec<String>,
    pub adrs: Vec<String>,
}

#[tokio::main]
async fn main() {
    // Determine the features path - when running from workspace root
    let features_path = if std::path::Path::new("specs/features").exists() {
        "specs/features/"
    } else {
        // Fallback for when running from specs directory
        "features/"
    };

    FlightWorld::cucumber().run_and_exit(features_path).await;
}
