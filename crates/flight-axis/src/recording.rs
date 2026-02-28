// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis input recording and playback for deterministic test replay.
//!
//! [`AxisRecording`] captures a time-series of axis values keyed by microsecond
//! timestamps.  [`AxisPlayback`] replays them deterministically, advancing a
//! cursor through the timeline and returning the samples that fall within each
//! time window into a fixed-size 32-slot output array (zero allocation).
//!
//! Samples **must be appended in ascending `timestamp_us` order** for
//! [`AxisPlayback`] to replay them correctly.

/// A single recorded axis sample.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisSample {
    /// Time offset from recording start, in microseconds.
    pub timestamp_us: u64,
    /// Axis index.
    pub axis_index: u8,
    /// Normalized axis value in `[-1.0, 1.0]`.
    pub value: f32,
}

/// A recording of axis input over time.
#[derive(Debug, Clone, Default)]
pub struct AxisRecording {
    samples: Vec<AxisSample>,
    duration_us: u64,
}

impl AxisRecording {
    /// Create an empty recording.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a sample at the given timestamp.
    ///
    /// Samples should be appended in ascending `timestamp_us` order for
    /// [`AxisPlayback`] to replay them correctly.
    pub fn record(&mut self, timestamp_us: u64, axis_index: u8, value: f32) {
        self.samples.push(AxisSample {
            timestamp_us,
            axis_index,
            value,
        });
        if timestamp_us > self.duration_us {
            self.duration_us = timestamp_us;
        }
    }

    /// Total duration of the recording in microseconds (the largest timestamp seen).
    pub fn duration_us(&self) -> u64 {
        self.duration_us
    }

    /// Number of samples in the recording.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Returns `true` if there are no samples.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// All samples in insertion order.
    pub fn samples(&self) -> &[AxisSample] {
        &self.samples
    }

    /// Iterator over samples whose timestamp falls within `[start_us, end_us]` (inclusive).
    pub fn samples_in_range(
        &self,
        start_us: u64,
        end_us: u64,
    ) -> impl Iterator<Item = &AxisSample> {
        self.samples
            .iter()
            .filter(move |s| (start_us..=end_us).contains(&s.timestamp_us))
    }

    /// Iterator over all samples for a specific axis index.
    pub fn axis_samples(&self, axis_index: u8) -> impl Iterator<Item = &AxisSample> {
        self.samples
            .iter()
            .filter(move |s| s.axis_index == axis_index)
    }

    /// Remove all samples and reset the duration to zero.
    pub fn clear(&mut self) {
        self.samples.clear();
        self.duration_us = 0;
    }
}

/// Replays a recording at its original timestamps.
///
/// Call [`advance`](AxisPlayback::advance) each tick to obtain the samples that
/// fell within that tick's time window.  The output is written into a
/// caller-provided fixed-size `[AxisSample; 32]` array so that no heap
/// allocation occurs on the hot path.
pub struct AxisPlayback {
    recording: AxisRecording,
    current_us: u64,
    sample_index: usize,
    looping: bool,
}

impl AxisPlayback {
    /// Create a new playback cursor for `recording`.
    ///
    /// If `looping` is `true`, playback wraps back to the beginning when it
    /// reaches the end of the recording and [`is_finished`](Self::is_finished)
    /// always returns `false`.
    pub fn new(recording: AxisRecording, looping: bool) -> Self {
        Self {
            recording,
            current_us: 0,
            sample_index: 0,
            looping,
        }
    }

    /// Advance playback by `delta_us` microseconds.
    ///
    /// Copies samples that fall within `[current_us, current_us + delta_us)`
    /// into `output` (at most 32 samples).  Returns the number of samples
    /// written.
    ///
    /// When `looping` is enabled and the window crosses the recording boundary,
    /// the cursor wraps and samples from the wrapped portion are also included
    /// (subject to the 32-sample cap).
    pub fn advance(&mut self, delta_us: u64, output: &mut [AxisSample; 32]) -> usize {
        let mut count = 0usize;
        let window_end = self.current_us.saturating_add(delta_us);
        let duration = self.recording.duration_us;
        let sample_count = self.recording.samples.len();

        if self.looping && duration > 0 && window_end >= duration {
            // Part 1: drain the tail of the recording up to (and including) the
            // duration boundary sample.
            while count < 32 && self.sample_index < sample_count {
                let ts = self.recording.samples[self.sample_index].timestamp_us;
                if ts > duration {
                    break;
                }
                output[count] = self.recording.samples[self.sample_index].clone();
                count += 1;
                self.sample_index += 1;
            }

            // Wrap the cursor back to the start.
            let new_pos = window_end % duration;
            self.current_us = 0;
            self.sample_index = 0;

            // Part 2: collect samples from the start up to the wrapped position.
            while count < 32 && self.sample_index < sample_count {
                let ts = self.recording.samples[self.sample_index].timestamp_us;
                if ts >= new_pos {
                    break;
                }
                output[count] = self.recording.samples[self.sample_index].clone();
                count += 1;
                self.sample_index += 1;
            }
            self.current_us = new_pos;
        } else {
            // Normal (non-wrapping) advance.
            while count < 32 && self.sample_index < sample_count {
                let ts = self.recording.samples[self.sample_index].timestamp_us;
                if ts >= window_end {
                    break;
                }
                output[count] = self.recording.samples[self.sample_index].clone();
                count += 1;
                self.sample_index += 1;
            }
            self.current_us = window_end;
        }

        count
    }

    /// Current playback position in microseconds.
    pub fn position_us(&self) -> u64 {
        self.current_us
    }

    /// Returns `true` when playback has reached the end **and** `looping` is
    /// `false`.  Always returns `false` for looping playback.
    pub fn is_finished(&self) -> bool {
        !self.looping && self.current_us >= self.recording.duration_us
    }

    /// Reset the playback cursor to the beginning.
    pub fn rewind(&mut self) {
        self.current_us = 0;
        self.sample_index = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn s(ts: u64, axis: u8, val: f32) -> AxisSample {
        AxisSample {
            timestamp_us: ts,
            axis_index: axis,
            value: val,
        }
    }

    fn blank_output() -> [AxisSample; 32] {
        std::array::from_fn(|_| s(0, 0, 0.0))
    }

    // ── unit tests ────────────────────────────────────────────────────────────

    #[test]
    fn test_recording_empty_by_default() {
        let r = AxisRecording::new();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert_eq!(r.duration_us(), 0);
    }

    #[test]
    fn test_record_single_sample() {
        let mut r = AxisRecording::new();
        r.record(1000, 0, 0.5);
        assert_eq!(r.len(), 1);
        assert_eq!(r.samples()[0], s(1000, 0, 0.5));
        assert_eq!(r.duration_us(), 1000);
    }

    #[test]
    fn test_record_multiple_samples() {
        let mut r = AxisRecording::new();
        r.record(0, 0, 0.1);
        r.record(1000, 1, 0.2);
        r.record(2000, 0, 0.3);
        assert_eq!(r.len(), 3);
        assert_eq!(r.duration_us(), 2000);
    }

    #[test]
    fn test_recording_duration() {
        let mut r = AxisRecording::new();
        r.record(500, 0, 0.0);
        r.record(3000, 0, 0.5);
        r.record(1500, 0, 0.25);
        // duration tracks the max timestamp seen
        assert_eq!(r.duration_us(), 3000);
    }

    #[test]
    fn test_samples_in_range() {
        let mut r = AxisRecording::new();
        r.record(0, 0, 0.0);
        r.record(1000, 0, 0.1);
        r.record(2000, 0, 0.2);
        r.record(3000, 0, 0.3);
        let got: Vec<_> = r.samples_in_range(1000, 2000).collect();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].timestamp_us, 1000);
        assert_eq!(got[1].timestamp_us, 2000);
    }

    #[test]
    fn test_axis_samples_filter() {
        let mut r = AxisRecording::new();
        r.record(0, 0, 0.0);
        r.record(100, 1, 0.1);
        r.record(200, 0, 0.2);
        r.record(300, 2, 0.3);

        let axis0: Vec<_> = r.axis_samples(0).collect();
        assert_eq!(axis0.len(), 2);
        assert!(axis0.iter().all(|s| s.axis_index == 0));

        let axis1: Vec<_> = r.axis_samples(1).collect();
        assert_eq!(axis1.len(), 1);

        let axis3: Vec<_> = r.axis_samples(3).collect();
        assert!(axis3.is_empty());
    }

    #[test]
    fn test_playback_advance_returns_samples_in_window() {
        let mut r = AxisRecording::new();
        r.record(500, 0, 0.5);
        r.record(1500, 0, -0.5);
        let mut pb = AxisPlayback::new(r, false);
        let mut out = blank_output();

        // Window [0, 1000): should yield the sample at t=500.
        let n = pb.advance(1000, &mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0].timestamp_us, 500);

        // Window [1000, 2000): should yield the sample at t=1500.
        let n = pb.advance(1000, &mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0].timestamp_us, 1500);
    }

    #[test]
    fn test_playback_advance_no_samples_beyond_end() {
        let mut r = AxisRecording::new();
        r.record(100, 0, 0.1);
        let mut pb = AxisPlayback::new(r, false);
        let mut out = blank_output();

        pb.advance(1000, &mut out); // consumes the only sample
        let n = pb.advance(1000, &mut out); // nothing left
        assert_eq!(n, 0);
    }

    #[test]
    fn test_playback_is_finished_at_end() {
        let mut r = AxisRecording::new();
        r.record(500, 0, 0.5);
        let mut pb = AxisPlayback::new(r, false);
        let mut out = blank_output();

        assert!(!pb.is_finished());
        pb.advance(1000, &mut out);
        assert!(pb.is_finished());
    }

    #[test]
    fn test_playback_looping_wraps_around() {
        let mut r = AxisRecording::new();
        r.record(0, 0, 0.1);
        r.record(1000, 0, 0.9);
        // duration_us == 1000
        let mut pb = AxisPlayback::new(r, true);
        let mut out = blank_output();

        // First full pass: should see both samples.
        let n1 = pb.advance(2000, &mut out);
        assert!(n1 >= 1, "first pass yielded no samples");

        // Second full pass after wrap: samples should appear again.
        let n2 = pb.advance(2000, &mut out);
        assert!(n2 >= 1, "looping should re-yield samples, got {n2}");
        assert!(!pb.is_finished());
    }

    #[test]
    fn test_playback_rewind() {
        let mut r = AxisRecording::new();
        r.record(500, 0, 0.5);
        let mut pb = AxisPlayback::new(r, false);
        let mut out = blank_output();

        pb.advance(1000, &mut out);
        assert!(pb.is_finished());

        pb.rewind();
        assert_eq!(pb.position_us(), 0);
        assert!(!pb.is_finished());

        // After rewind, samples are available again.
        let n = pb.advance(1000, &mut out);
        assert_eq!(n, 1);
    }

    #[test]
    fn test_advance_large_delta_catches_up() {
        let mut r = AxisRecording::new();
        for i in 0u64..20 {
            r.record(i * 100, 0, i as f32 * 0.05);
        }
        let mut pb = AxisPlayback::new(r, false);
        let mut out = blank_output();

        // A very large delta should collect all 20 samples in one call.
        let n = pb.advance(u64::MAX / 2, &mut out);
        assert!(n <= 32);
        assert_eq!(n, 20);
    }

    // ── property-based tests ──────────────────────────────────────────────────

    proptest! {
        /// The output slice has only 32 slots; advance must never write more.
        #[test]
        fn prop_advance_count_never_exceeds_32(
            timestamps in proptest::collection::vec(0u64..1_000_000u64, 0..64),
            delta in 0u64..2_000_000u64,
        ) {
            let mut r = AxisRecording::new();
            let mut sorted = timestamps.clone();
            sorted.sort_unstable();
            for ts in sorted {
                r.record(ts, 0, 0.5);
            }
            let mut pb = AxisPlayback::new(r, false);
            let mut out: [AxisSample; 32] =
                std::array::from_fn(|_| AxisSample { timestamp_us: 0, axis_index: 0, value: 0.0 });
            let n = pb.advance(delta, &mut out);
            prop_assert!(n <= 32, "advance returned {n} > 32");
        }

        /// The playback position must be non-decreasing between advances
        /// (rewind is not called in this test).
        #[test]
        fn prop_position_non_decreasing(
            timestamps in proptest::collection::vec(0u64..1_000_000u64, 0..32),
            deltas in proptest::collection::vec(0u64..100_000u64, 1..20),
        ) {
            let mut r = AxisRecording::new();
            let mut sorted = timestamps.clone();
            sorted.sort_unstable();
            for ts in sorted {
                r.record(ts, 0, 0.5);
            }
            let mut pb = AxisPlayback::new(r, false);
            let mut prev = pb.position_us();
            let mut out: [AxisSample; 32] =
                std::array::from_fn(|_| AxisSample { timestamp_us: 0, axis_index: 0, value: 0.0 });
            for delta in deltas {
                pb.advance(delta, &mut out);
                let pos = pb.position_us();
                prop_assert!(pos >= prev, "position went backwards: {prev} -> {pos}");
                prev = pos;
            }
        }
    }
}
