// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Trace recording and deterministic replay for axis regression testing.
//!
//! This module provides a "wind tunnel" approach to axis pipeline testing:
//! record real device/sim telemetry traces, then replay them deterministically
//! through the pipeline to detect regressions.
//!
//! - [`AxisTrace`] — a serialisable sequence of timestamped axis I/O samples.
//! - [`TraceRecorder`] — captures input/output pairs during live operation.
//! - [`TraceReplayer`] — feeds recorded inputs through an [`AxisEngine`] and
//!   collects outputs.
//! - [`assert_trace_matches`] — compares replay output against a golden
//!   reference within a configurable tolerance.

use crate::{AxisEngine, AxisFrame};

/// A single timestamped axis input/output observation.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct TraceSample {
    /// Monotonic timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// Axis identifier (index).
    pub axis_id: u8,
    /// Raw input value fed into the pipeline.
    pub raw_input: f32,
    /// Output value produced by the pipeline.
    pub output: f32,
}

/// An ordered sequence of [`TraceSample`]s forming a complete trace.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
pub struct AxisTrace {
    samples: Vec<TraceSample>,
}

impl AxisTrace {
    /// Create an empty trace.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a sample to the trace.
    pub fn push(&mut self, sample: TraceSample) {
        self.samples.push(sample);
    }

    /// Number of samples.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Returns `true` when there are no samples.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Borrow the underlying sample slice.
    pub fn samples(&self) -> &[TraceSample] {
        &self.samples
    }

    /// Iterate over the inputs only (timestamp, axis_id, raw_input).
    pub fn inputs(&self) -> impl Iterator<Item = (u64, u8, f32)> + '_ {
        self.samples
            .iter()
            .map(|s| (s.timestamp_ns, s.axis_id, s.raw_input))
    }

    /// Iterate over the outputs only.
    pub fn outputs(&self) -> impl Iterator<Item = f32> + '_ {
        self.samples.iter().map(|s| s.output)
    }

    /// Extract only the output values as a `Vec<f32>`.
    pub fn output_vec(&self) -> Vec<f32> {
        self.outputs().collect()
    }
}

/// Records axis input/output pairs from live processing.
///
/// Attach a `TraceRecorder` alongside the pipeline: after each
/// [`AxisEngine::process`] call, feed the resulting [`AxisFrame`] into
/// [`record_frame`](Self::record_frame) to capture the I/O pair.
pub struct TraceRecorder {
    trace: AxisTrace,
    axis_id: u8,
}

impl TraceRecorder {
    /// Create a recorder for the given axis.
    pub fn new(axis_id: u8) -> Self {
        Self {
            trace: AxisTrace::new(),
            axis_id,
        }
    }

    /// Record a single frame's input and output.
    pub fn record_frame(&mut self, frame: &AxisFrame) {
        self.trace.push(TraceSample {
            timestamp_ns: frame.ts_mono_ns,
            axis_id: self.axis_id,
            raw_input: frame.in_raw,
            output: frame.out,
        });
    }

    /// Consume the recorder and return the captured trace.
    pub fn finish(self) -> AxisTrace {
        self.trace
    }

    /// Borrow the trace captured so far.
    pub fn trace(&self) -> &AxisTrace {
        &self.trace
    }
}

/// Replays a recorded trace through an [`AxisEngine`], collecting outputs.
///
/// Each sample's `raw_input` and `timestamp_ns` are used to build an
/// [`AxisFrame`] which is then processed through the engine. The resulting
/// output is stored in a new [`AxisTrace`].
pub struct TraceReplayer<'a> {
    engine: &'a AxisEngine,
}

impl<'a> TraceReplayer<'a> {
    /// Create a replayer bound to the given engine.
    pub fn new(engine: &'a AxisEngine) -> Self {
        Self { engine }
    }

    /// Replay every sample in `trace` and return a new trace with actual
    /// pipeline outputs.
    pub fn replay(&self, trace: &AxisTrace) -> AxisTrace {
        let mut result = AxisTrace::new();
        for sample in trace.samples() {
            let mut frame = AxisFrame::new(sample.raw_input, sample.timestamp_ns);
            let _ = self.engine.process(&mut frame);
            result.push(TraceSample {
                timestamp_ns: sample.timestamp_ns,
                axis_id: sample.axis_id,
                raw_input: sample.raw_input,
                output: frame.out,
            });
        }
        result
    }
}

/// Maximum absolute difference allowed when comparing two output values.
const DEFAULT_TOLERANCE: f32 = 1e-6;

/// Compare two traces sample-by-sample and panic with diagnostics on mismatch.
///
/// `tolerance` controls the maximum allowed absolute difference between
/// corresponding output values. Pass `None` for the default (`1e-6`).
///
/// # Panics
///
/// Panics if the traces differ in length or if any output pair exceeds the
/// tolerance.
pub fn assert_trace_matches(expected: &AxisTrace, actual: &AxisTrace, tolerance: Option<f32>) {
    let tol = tolerance.unwrap_or(DEFAULT_TOLERANCE);

    assert_eq!(
        expected.len(),
        actual.len(),
        "trace length mismatch: expected {} samples, got {}",
        expected.len(),
        actual.len(),
    );

    for (i, (exp, act)) in expected
        .samples()
        .iter()
        .zip(actual.samples())
        .enumerate()
    {
        assert_eq!(
            exp.timestamp_ns, act.timestamp_ns,
            "sample {i}: timestamp mismatch (expected {}ns, got {}ns)",
            exp.timestamp_ns, act.timestamp_ns,
        );
        assert_eq!(
            exp.axis_id, act.axis_id,
            "sample {i}: axis_id mismatch (expected {}, got {})",
            exp.axis_id, act.axis_id,
        );
        let diff = (exp.output - act.output).abs();
        assert!(
            diff <= tol,
            "sample {i} @ {}ns: output mismatch — expected {:.8}, got {:.8} (diff {:.8} > tol {:.8})",
            exp.timestamp_ns,
            exp.output,
            act.output,
            diff,
            tol,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sample(ts: u64, axis: u8, input: f32, output: f32) -> TraceSample {
        TraceSample {
            timestamp_ns: ts,
            axis_id: axis,
            raw_input: input,
            output,
        }
    }

    // ── AxisTrace basics ──────────────────────────────────────────────────

    #[test]
    fn test_trace_push_and_len() {
        let mut trace = AxisTrace::new();
        assert!(trace.is_empty());
        trace.push(make_sample(1000, 0, 0.5, 0.5));
        assert_eq!(trace.len(), 1);
        assert!(!trace.is_empty());
    }

    #[test]
    fn test_trace_output_vec() {
        let mut trace = AxisTrace::new();
        trace.push(make_sample(1000, 0, 0.1, 0.2));
        trace.push(make_sample(2000, 0, 0.3, 0.4));
        assert_eq!(trace.output_vec(), vec![0.2, 0.4]);
    }

    // ── TraceRecorder ─────────────────────────────────────────────────────

    #[test]
    fn test_recorder_captures_frames() {
        let mut recorder = TraceRecorder::new(0);
        let mut frame = AxisFrame::new(0.5, 4_000_000);
        frame.out = 0.45;
        recorder.record_frame(&frame);

        let trace = recorder.finish();
        assert_eq!(trace.len(), 1);
        let s = &trace.samples()[0];
        assert_eq!(s.raw_input, 0.5);
        assert_eq!(s.output, 0.45);
        assert_eq!(s.timestamp_ns, 4_000_000);
    }

    // ── TraceReplayer ─────────────────────────────────────────────────────

    #[test]
    fn test_replayer_passthrough() {
        // Engine with no pipeline → output == input (pass-through)
        let engine = AxisEngine::new();
        let replayer = TraceReplayer::new(&engine);

        let mut trace = AxisTrace::new();
        trace.push(make_sample(4_000_000, 0, 0.5, 999.0)); // output ignored on input
        trace.push(make_sample(8_000_000, 0, -0.3, 999.0));

        let result = replayer.replay(&trace);
        assert_eq!(result.len(), 2);
        assert!((result.samples()[0].output - 0.5).abs() < 1e-6);
        assert!((result.samples()[1].output - (-0.3)).abs() < 1e-6);
    }

    // ── Roundtrip serialisation ───────────────────────────────────────────

    #[test]
    fn test_trace_roundtrip() {
        let mut trace = AxisTrace::new();
        for i in 0..10 {
            let t = (i + 1) as u64 * 4_000_000;
            let v = (i as f32) * 0.1 - 0.5;
            trace.push(make_sample(t, 0, v, v));
        }

        let json = serde_json::to_string_pretty(&trace).expect("serialise");
        let restored: AxisTrace = serde_json::from_str(&json).expect("deserialise");

        assert_trace_matches(&trace, &restored, None);
    }

    // ── assert_trace_matches ──────────────────────────────────────────────

    #[test]
    fn test_assert_trace_matches_identical() {
        let mut t = AxisTrace::new();
        t.push(make_sample(1000, 0, 0.5, 0.5));
        assert_trace_matches(&t, &t, None);
    }

    #[test]
    #[should_panic(expected = "trace length mismatch")]
    fn test_assert_trace_matches_length_mismatch() {
        let mut a = AxisTrace::new();
        a.push(make_sample(1000, 0, 0.5, 0.5));
        let b = AxisTrace::new();
        assert_trace_matches(&a, &b, None);
    }

    #[test]
    #[should_panic(expected = "output mismatch")]
    fn test_assert_trace_matches_value_mismatch() {
        let mut a = AxisTrace::new();
        a.push(make_sample(1000, 0, 0.5, 0.5));
        let mut b = AxisTrace::new();
        b.push(make_sample(1000, 0, 0.5, 0.6));
        assert_trace_matches(&a, &b, None);
    }

    #[test]
    fn test_assert_trace_matches_within_tolerance() {
        let mut a = AxisTrace::new();
        a.push(make_sample(1000, 0, 0.5, 0.500_000_0));
        let mut b = AxisTrace::new();
        b.push(make_sample(1000, 0, 0.5, 0.500_000_9));
        assert_trace_matches(&a, &b, Some(1e-5));
    }

    // ── Golden-trace regression ───────────────────────────────────────────

    #[test]
    fn test_golden_trace_regression() {
        let golden_json = include_str!("../tests/fixtures/golden_trace.json");
        let golden: AxisTrace = serde_json::from_str(golden_json).expect("parse golden trace");

        // Replay through a default (pass-through) engine
        let engine = AxisEngine::new();
        let replayer = TraceReplayer::new(&engine);
        let actual = replayer.replay(&golden);

        assert_trace_matches(&golden, &actual, Some(1e-6));
    }
}
