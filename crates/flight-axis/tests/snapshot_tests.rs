// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for deterministic axis processing outputs.
//!
//! These tests fix the exact numeric output of curve and pipeline nodes at
//! specific representative inputs.  Any regression that changes the processing
//! formula will be caught here before it reaches users.

use flight_axis::{AxisFrame, CurveNode, Node, Pipeline, PipelineBuilder, PipelineState};

// ── helper ────────────────────────────────────────────────────────────────────

fn step(pipeline: &Pipeline, state: &mut PipelineState, input: f32, ts_ns: u64) -> f32 {
    let mut frame = AxisFrame::new(input, ts_ns);
    frame.out = frame.in_raw;
    pipeline.process(&mut frame, state);
    frame.out
}

// ── snapshot: curve node output values ────────────────────────────────────────

/// Fix the output of `CurveNode::new(0.4)` at nine representative input points.
///
/// These values encode the current expo formula; any change to the curve maths
/// will surface as a snapshot diff rather than a silent regression.
#[test]
fn snapshot_curve_node_expo_0_4_outputs() {
    let mut node = CurveNode::new(0.4);
    let inputs: &[f32] = &[-1.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0];

    let outputs: Vec<String> = inputs
        .iter()
        .map(|&x| {
            let mut frame = AxisFrame::new(x, 1_000_000);
            node.step(&mut frame);
            format!("in={x:.2} out={:.6}", frame.out)
        })
        .collect();

    insta::assert_debug_snapshot!("curve_node_expo_0_4_outputs", outputs);
}

// ── snapshot: deadzone + expo pipeline metadata ───────────────────────────────

/// Fix the node-type sequence, state offsets, and state sizes for a
/// `deadzone(0.05) → curve(0.3)` pipeline.
///
/// Regressions in node ordering, naming, or state layout will fail this test.
#[test]
fn snapshot_deadzone_expo_pipeline_metadata() {
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .expect("expo 0.3 is valid")
        .compile()
        .expect("pipeline should compile");

    let meta: Vec<String> = pipeline
        .metadata()
        .iter()
        .map(|m| {
            format!(
                "id={} type={} state_offset={} state_size={}",
                m.node_id.0, m.node_type, m.state_offset, m.state_size
            )
        })
        .collect();

    insta::assert_debug_snapshot!("deadzone_expo_pipeline_metadata", meta);
}

// ── snapshot: deadzone + expo pipeline output values ──────────────────────────

/// Fix the end-to-end output of `deadzone(0.1) → curve(0.3)` at key inputs.
///
/// Tests both the boundary behaviour (inputs inside / at / outside the deadzone)
/// and the shape of the curve in the live range.
#[test]
fn snapshot_deadzone_expo_pipeline_output_values() {
    let pipeline = PipelineBuilder::new()
        .deadzone(0.1)
        .curve(0.3)
        .expect("expo 0.3 is valid")
        .compile()
        .expect("pipeline should compile");

    let inputs: &[f32] = &[0.0, 0.05, 0.10, 0.15, 0.30, 0.50, 0.70, 0.90, 1.0];

    let outputs: Vec<String> = inputs
        .iter()
        .map(|&x| {
            let mut state = pipeline.create_state();
            let out = step(&pipeline, &mut state, x, 1_000_000);
            format!("in={x:.2} out={out:.6}")
        })
        .collect();

    insta::assert_debug_snapshot!("deadzone_expo_pipeline_output_values", outputs);
}
