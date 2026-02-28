// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Post-recording analysis utilities for blackbox data.
//!
//! Provides anomaly detection, per-axis statistics, and chronological event
//! timelines over a [`BlackboxRecorder`] snapshot.

use crate::recorder::{BlackboxRecorder, RecordEntry};

// ── Anomaly Detection ────────────────────────────────────────────────

/// Thresholds for anomaly detection.
#[derive(Debug, Clone)]
pub struct AnomalyThresholds {
    /// Maximum acceptable inter-sample jitter in nanoseconds.
    pub max_jitter_ns: u64,
    /// An axis value whose absolute value exceeds this is considered saturated.
    pub saturation_threshold: f64,
    /// Maximum gap between consecutive axis samples (ns) before flagging a
    /// disconnect.
    pub max_gap_ns: u64,
}

impl Default for AnomalyThresholds {
    fn default() -> Self {
        Self {
            // 0.5 ms jitter threshold (matches QG-RT-JITTER gate)
            max_jitter_ns: 500_000,
            saturation_threshold: 0.999,
            // 20 ms gap (5 missed ticks at 250 Hz)
            max_gap_ns: 20_000_000,
        }
    }
}

/// A detected anomaly in the recording.
#[derive(Debug, Clone, PartialEq)]
pub enum Anomaly {
    /// Jitter between two consecutive axis samples exceeded the threshold.
    JitterSpike {
        axis_id: u16,
        timestamp_ns: u64,
        delta_ns: u64,
        expected_ns: u64,
    },
    /// An axis value hit the saturation limit.
    Saturation {
        axis_id: u16,
        timestamp_ns: u64,
        value: f64,
    },
    /// A gap in axis data suggests a device disconnect or scheduling stall.
    Disconnect {
        axis_id: u16,
        from_ns: u64,
        to_ns: u64,
        gap_ns: u64,
    },
}

/// Detect anomalies in the recording using the given thresholds.
pub fn detect_anomalies(
    recorder: &BlackboxRecorder,
    thresholds: &AnomalyThresholds,
) -> Vec<Anomaly> {
    let mut anomalies = Vec::new();

    // Track the last timestamp per axis to compute deltas.
    // Use a small inline map — most setups have <16 axes.
    let mut last_ts: Vec<(u16, u64)> = Vec::new();

    // Collect per-axis intervals to estimate expected cadence.
    let mut intervals: Vec<(u16, Vec<u64>)> = Vec::new();

    for entry in recorder.iter() {
        if let RecordEntry::Axis(a) = entry {
            // Check saturation
            if a.processed.abs() >= thresholds.saturation_threshold {
                anomalies.push(Anomaly::Saturation {
                    axis_id: a.axis_id,
                    timestamp_ns: a.timestamp_ns,
                    value: a.processed,
                });
            }

            // Find or insert last timestamp for this axis
            let prev = last_ts.iter_mut().find(|(id, _)| *id == a.axis_id);
            if let Some((_, prev_ts)) = prev {
                let delta = a.timestamp_ns.saturating_sub(*prev_ts);

                // Collect interval for expected-cadence estimation
                let ivl = intervals.iter_mut().find(|(id, _)| *id == a.axis_id);
                if let Some((_, v)) = ivl {
                    v.push(delta);
                } else {
                    intervals.push((a.axis_id, vec![delta]));
                }

                // Check for disconnect (large gap)
                if delta > thresholds.max_gap_ns {
                    anomalies.push(Anomaly::Disconnect {
                        axis_id: a.axis_id,
                        from_ns: *prev_ts,
                        to_ns: a.timestamp_ns,
                        gap_ns: delta,
                    });
                }

                *prev_ts = a.timestamp_ns;
            } else {
                last_ts.push((a.axis_id, a.timestamp_ns));
            }
        }
    }

    // Second pass: check for jitter spikes relative to median interval.
    for (axis_id, ivls) in &intervals {
        if ivls.is_empty() {
            continue;
        }
        let mut sorted = ivls.clone();
        sorted.sort_unstable();
        let median = sorted[sorted.len() / 2];

        // Re-walk axis entries to flag jitter relative to median
        let mut prev_ts: Option<u64> = None;
        for entry in recorder.iter() {
            if let RecordEntry::Axis(a) = entry {
                if a.axis_id != *axis_id {
                    continue;
                }
                if let Some(pt) = prev_ts {
                    let delta = a.timestamp_ns.saturating_sub(pt);
                    let jitter = delta.abs_diff(median);
                    if jitter > thresholds.max_jitter_ns {
                        // Avoid duplicating disconnect anomalies
                        let is_disconnect = anomalies.iter().any(|an| {
                            matches!(an, Anomaly::Disconnect { axis_id: id, from_ns, .. }
                                if *id == a.axis_id && *from_ns == pt)
                        });
                        if !is_disconnect {
                            anomalies.push(Anomaly::JitterSpike {
                                axis_id: a.axis_id,
                                timestamp_ns: a.timestamp_ns,
                                delta_ns: delta,
                                expected_ns: median,
                            });
                        }
                    }
                }
                prev_ts = Some(a.timestamp_ns);
            }
        }
    }

    anomalies
}

// ── Axis Statistics ──────────────────────────────────────────────────

/// Descriptive statistics for a single axis.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisStatistics {
    pub axis_id: u16,
    pub count: u64,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub stddev: f64,
    /// 99th-percentile of *processed* values.
    pub p99: f64,
}

/// Compute descriptive statistics for a given axis.
///
/// Returns `None` if no samples exist for `axis_id`.
pub fn axis_statistics(recorder: &BlackboxRecorder, axis_id: u16) -> Option<AxisStatistics> {
    let mut values: Vec<f64> = Vec::new();

    for entry in recorder.iter() {
        if let RecordEntry::Axis(a) = entry
            && a.axis_id == axis_id
        {
            values.push(a.processed);
        }
    }

    if values.is_empty() {
        return None;
    }

    let count = values.len() as u64;
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let sum: f64 = values.iter().sum();
    let mean = sum / values.len() as f64;

    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    let stddev = variance.sqrt();

    // p99
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p99_idx = ((values.len() as f64) * 0.99).ceil() as usize;
    let p99 = values[p99_idx.min(values.len() - 1)];

    Some(AxisStatistics {
        axis_id,
        count,
        min,
        max,
        mean,
        stddev,
        p99,
    })
}

// ── Event Timeline ───────────────────────────────────────────────────

/// A single entry in a chronological event timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineEntry {
    pub timestamp_ns: u64,
    pub kind: TimelineKind,
}

/// The kind of timeline entry.
#[derive(Debug, Clone, PartialEq)]
pub enum TimelineKind {
    Event { event_type: u16, source: String },
    Telemetry { sim: String },
    Ffb { effect_type: u16, magnitude: f64 },
}

/// Build a chronological event timeline from the recording.
///
/// Axis samples are excluded (they are high-frequency data points, not events).
/// The returned list is sorted by timestamp.
pub fn event_timeline(recorder: &BlackboxRecorder) -> Vec<TimelineEntry> {
    let mut timeline = Vec::new();

    for entry in recorder.iter() {
        match entry {
            RecordEntry::Event(e) => {
                timeline.push(TimelineEntry {
                    timestamp_ns: e.timestamp_ns,
                    kind: TimelineKind::Event {
                        event_type: e.event_type,
                        source: e.source_str().to_string(),
                    },
                });
            }
            RecordEntry::Telemetry(t) => {
                timeline.push(TimelineEntry {
                    timestamp_ns: t.timestamp_ns,
                    kind: TimelineKind::Telemetry {
                        sim: t.sim_str().to_string(),
                    },
                });
            }
            RecordEntry::Ffb(f) => {
                timeline.push(TimelineEntry {
                    timestamp_ns: f.timestamp_ns,
                    kind: TimelineKind::Ffb {
                        effect_type: f.effect_type,
                        magnitude: f.magnitude,
                    },
                });
            }
            RecordEntry::Axis(_) | RecordEntry::Empty => {}
        }
    }

    timeline.sort_by_key(|e| e.timestamp_ns);
    timeline
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::RecorderConfig;

    fn make(cap: usize) -> BlackboxRecorder {
        BlackboxRecorder::new(RecorderConfig { capacity: cap })
    }

    // ── Anomaly detection ────────────────────────────────────────

    #[test]
    fn detect_saturation_anomaly() {
        let mut rec = make(32);
        rec.record_axis(1, 1.0, 1.0, 1_000_000);
        rec.record_axis(1, 0.5, 0.5, 2_000_000);
        rec.record_axis(1, -1.0, -1.0, 3_000_000);

        let anomalies = detect_anomalies(&rec, &AnomalyThresholds::default());
        let saturations: Vec<_> = anomalies
            .iter()
            .filter(|a| matches!(a, Anomaly::Saturation { .. }))
            .collect();
        // Values 1.0 and -1.0 both exceed 0.999
        assert_eq!(saturations.len(), 2);
    }

    #[test]
    fn detect_disconnect_anomaly() {
        let mut rec = make(32);
        // Normal 4 ms cadence, then a 50 ms gap
        rec.record_axis(1, 0.0, 0.0, 4_000_000);
        rec.record_axis(1, 0.1, 0.1, 8_000_000);
        rec.record_axis(1, 0.2, 0.2, 12_000_000);
        rec.record_axis(1, 0.3, 0.3, 62_000_000); // 50 ms gap

        let thresholds = AnomalyThresholds {
            max_gap_ns: 20_000_000,
            ..Default::default()
        };
        let anomalies = detect_anomalies(&rec, &thresholds);
        let disconnects: Vec<_> = anomalies
            .iter()
            .filter(|a| matches!(a, Anomaly::Disconnect { .. }))
            .collect();
        assert_eq!(disconnects.len(), 1);
        if let Anomaly::Disconnect { gap_ns, .. } = disconnects[0] {
            assert_eq!(*gap_ns, 50_000_000);
        }
    }

    #[test]
    fn detect_jitter_spike() {
        let mut rec = make(64);
        // Regular 4ms cadence, then one sample arrives 2ms late (jitter = 2ms)
        let cadence = 4_000_000u64;
        for i in 0..10 {
            rec.record_axis(1, 0.0, 0.0, i * cadence);
        }
        // Inject a jitter spike: next sample at +6ms instead of +4ms
        rec.record_axis(1, 0.0, 0.0, 10 * cadence + 2_000_000);
        // Resume normal
        rec.record_axis(1, 0.0, 0.0, 11 * cadence + 2_000_000);

        let thresholds = AnomalyThresholds {
            max_jitter_ns: 500_000, // 0.5 ms
            max_gap_ns: 100_000_000,
            saturation_threshold: 2.0, // disable saturation checks
        };
        let anomalies = detect_anomalies(&rec, &thresholds);
        let jitters: Vec<_> = anomalies
            .iter()
            .filter(|a| matches!(a, Anomaly::JitterSpike { .. }))
            .collect();
        assert!(
            !jitters.is_empty(),
            "should detect at least one jitter spike"
        );
    }

    #[test]
    fn no_anomalies_in_clean_data() {
        let mut rec = make(64);
        let cadence = 4_000_000u64;
        for i in 0..20 {
            rec.record_axis(1, 0.0, 0.5, i * cadence);
        }
        let thresholds = AnomalyThresholds {
            max_jitter_ns: 1_000_000,
            saturation_threshold: 0.999,
            max_gap_ns: 20_000_000,
        };
        let anomalies = detect_anomalies(&rec, &thresholds);
        assert!(anomalies.is_empty(), "clean data should have no anomalies");
    }

    // ── Statistics ───────────────────────────────────────────────

    #[test]
    fn statistics_basic() {
        let mut rec = make(64);
        // Values: 1, 2, 3, 4, 5
        for i in 1..=5 {
            rec.record_axis(1, i as f64, i as f64, i as u64 * 1000);
        }

        let stats = axis_statistics(&rec, 1).unwrap();
        assert_eq!(stats.count, 5);
        assert!((stats.min - 1.0).abs() < f64::EPSILON);
        assert!((stats.max - 5.0).abs() < f64::EPSILON);
        assert!((stats.mean - 3.0).abs() < f64::EPSILON);
        // stddev of [1,2,3,4,5] = sqrt(2) ≈ 1.4142
        assert!((stats.stddev - std::f64::consts::SQRT_2).abs() < 0.001);
        // p99 should be 5.0
        assert!((stats.p99 - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn statistics_single_value() {
        let mut rec = make(16);
        rec.record_axis(7, 0.42, 0.42, 100);

        let stats = axis_statistics(&rec, 7).unwrap();
        assert_eq!(stats.count, 1);
        assert!((stats.min - 0.42).abs() < f64::EPSILON);
        assert!((stats.max - 0.42).abs() < f64::EPSILON);
        assert!((stats.mean - 0.42).abs() < f64::EPSILON);
        assert!((stats.stddev - 0.0).abs() < f64::EPSILON);
        assert!((stats.p99 - 0.42).abs() < f64::EPSILON);
    }

    #[test]
    fn statistics_missing_axis_returns_none() {
        let mut rec = make(16);
        rec.record_axis(1, 0.0, 0.0, 100);
        assert!(axis_statistics(&rec, 99).is_none());
    }

    #[test]
    fn statistics_ignores_other_entry_types() {
        let mut rec = make(32);
        rec.record_axis(1, 0.5, 0.5, 1000);
        rec.record_event(10, "src", &[]);
        rec.record_ffb(2, 0.8);
        rec.record_axis(1, 0.7, 0.7, 2000);

        let stats = axis_statistics(&rec, 1).unwrap();
        assert_eq!(stats.count, 2);
    }

    #[test]
    fn statistics_correctness_known_values() {
        let mut rec = make(64);
        // Known data set: 10 identical values
        for i in 0..10 {
            rec.record_axis(3, 0.0, 0.5, i * 1000);
        }
        let stats = axis_statistics(&rec, 3).unwrap();
        assert_eq!(stats.count, 10);
        assert!((stats.min - 0.5).abs() < f64::EPSILON);
        assert!((stats.max - 0.5).abs() < f64::EPSILON);
        assert!((stats.mean - 0.5).abs() < f64::EPSILON);
        assert!((stats.stddev - 0.0).abs() < f64::EPSILON);
    }

    // ── Event timeline ───────────────────────────────────────────

    #[test]
    fn timeline_excludes_axis_data() {
        let mut rec = make(32);
        rec.record_axis(1, 0.0, 0.0, 1000);
        rec.record_event(1, "hid", &[]);
        rec.record_ffb(2, 0.5);

        let tl = event_timeline(&rec);
        assert_eq!(tl.len(), 2);
        assert!(matches!(tl[0].kind, TimelineKind::Event { .. }));
        assert!(matches!(tl[1].kind, TimelineKind::Ffb { .. }));
    }

    #[test]
    fn timeline_sorted_by_timestamp() {
        let mut rec = make(32);
        // Insert events out of order via telemetry/FFB (they use monotonic_now_ns
        // internally, so they will be naturally ordered, but let's verify sort).
        rec.record_ffb(1, 0.5);
        rec.record_event(1, "a", &[]);
        rec.record_telemetry("DCS", &[]);

        let tl = event_timeline(&rec);
        for window in tl.windows(2) {
            assert!(window[0].timestamp_ns <= window[1].timestamp_ns);
        }
    }

    #[test]
    fn timeline_empty_recording() {
        let rec = make(16);
        let tl = event_timeline(&rec);
        assert!(tl.is_empty());
    }

    #[test]
    fn timeline_includes_telemetry() {
        let mut rec = make(16);
        rec.record_telemetry("MSFS", &[0x01]);

        let tl = event_timeline(&rec);
        assert_eq!(tl.len(), 1);
        match &tl[0].kind {
            TimelineKind::Telemetry { sim } => assert_eq!(sim, "MSFS"),
            _ => panic!("expected Telemetry"),
        }
    }
}
