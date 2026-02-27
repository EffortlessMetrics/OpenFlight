//! RT timing headroom tests for the axis processing pipeline.
//!
//! Verifies that processing 8 axes with typical filter configs completes
//! within the 4ms RT budget (ADR-001). The target is 50% headroom (≤2ms
//! average) on developer hardware; the full 4ms budget is allowed in CI.

use flight_axis::{AxisEngine, AxisFrame, PipelineBuilder};
use std::time::Instant;

fn build_engine(name: &str) -> AxisEngine {
    let engine = AxisEngine::new_for_axis(name.to_string());
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.2)
        .expect("valid curve")
        .slew(1.5)
        .compile()
        .expect("pipeline compiles");
    engine.update_pipeline(pipeline);
    // Prime the pending pipeline swap.
    let mut frame = AxisFrame::new(0.0, 0);
    let _ = engine.process(&mut frame);
    engine
}

/// Verify the average 8-axis tick time fits within the RT budget.
///
/// Target: ≤2ms on dev hardware (50% headroom), ≤4ms in CI.
#[test]
fn axis_pipeline_fits_in_rt_budget() {
    let engines: Vec<AxisEngine> = (0..8).map(|i| build_engine(&format!("axis_{i}"))).collect();

    // Warm up – not measured.
    for i in 0..100u64 {
        let ts = i * 4_000_000;
        for (j, engine) in engines.iter().enumerate() {
            let mut frame = AxisFrame::new((j as f32 + 1.0) * 0.1, ts);
            let _ = engine.process(&mut frame);
        }
    }

    // Measure 1 000 complete ticks.
    let start = Instant::now();
    for i in 0..1_000u64 {
        let ts = std::hint::black_box(1_000_000_000u64 + i * 4_000_000);
        for (j, engine) in engines.iter().enumerate() {
            let input = std::hint::black_box((j as f32 + 1.0) * 0.1);
            let mut frame = AxisFrame::new(input, ts);
            std::hint::black_box(engine.process(&mut frame).ok());
        }
    }
    let avg_us = start.elapsed().as_micros() / 1_000;

    // 4 000μs full budget; require 2 000μs (50% headroom) outside CI.
    let budget_us: u128 = if std::env::var_os("CI").is_some() {
        4_000
    } else {
        2_000
    };

    assert!(
        avg_us < budget_us,
        "Average 8-axis tick {avg_us}μs exceeds {budget_us}μs RT budget target"
    );
    println!("✓ 8-axis tick average: {avg_us}μs (budget: {budget_us}μs)");
}

/// Verify the p99 8-axis tick time fits within the 4ms RT budget.
#[test]
fn axis_pipeline_p99_within_4ms_budget() {
    let engines: Vec<AxisEngine> = (0..8)
        .map(|i| build_engine(&format!("axis_p99_{i}")))
        .collect();

    // Warm up.
    for i in 0..50u64 {
        let ts = i * 4_000_000;
        for (j, engine) in engines.iter().enumerate() {
            let mut frame = AxisFrame::new((j as f32 + 1.0) * 0.1, ts);
            let _ = engine.process(&mut frame);
        }
    }

    const ITERATIONS: usize = 500;
    let mut tick_times = Vec::with_capacity(ITERATIONS);

    for i in 0..ITERATIONS as u64 {
        let ts = std::hint::black_box(1_000_000_000u64 + i * 4_000_000);
        let tick_start = Instant::now();
        for (j, engine) in engines.iter().enumerate() {
            let input = std::hint::black_box((j as f32 + 1.0) * 0.1);
            let mut frame = AxisFrame::new(input, ts);
            std::hint::black_box(engine.process(&mut frame).ok());
        }
        tick_times.push(tick_start.elapsed());
    }

    tick_times.sort();
    let p99_idx = ((ITERATIONS as f32) * 0.99) as usize;
    let p99_us = tick_times[p99_idx.min(tick_times.len() - 1)].as_micros();

    assert!(
        p99_us < 4_000,
        "p99 8-axis tick time {p99_us}μs exceeds 4000μs RT budget"
    );
    println!("✓ 8-axis tick p99: {p99_us}μs (budget: 4000μs)");
}
