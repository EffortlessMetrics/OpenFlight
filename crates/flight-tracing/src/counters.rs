// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Performance counters for CI quality gates
//!
//! Provides atomic counters for tracking key performance metrics that are
//! consumed by CI systems for regression detection and quality gates.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use crate::events::{TraceEvent, EventData};

/// Atomic performance counters
pub struct PerfCounters {
    // Tick counters
    total_ticks: AtomicU64,
    deadline_misses: AtomicU64,
    
    // HID counters
    total_hid_writes: AtomicU64,
    hid_write_time_ns: AtomicU64,
    
    // Writer counters
    writer_drops: AtomicU64,
    
    // Jitter tracking
    jitter_samples: Mutex<JitterTracker>,
    
    // Session tracking
    session_start: Instant,
}

/// Jitter sample tracking for percentile calculation
struct JitterTracker {
    samples: Vec<i64>,
    max_samples: usize,
}

impl JitterTracker {
    fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::with_capacity(max_samples),
            max_samples,
        }
    }
    
    fn add_sample(&mut self, jitter_ns: i64) {
        if self.samples.len() >= self.max_samples {
            // Remove oldest sample (FIFO)
            self.samples.remove(0);
        }
        self.samples.push(jitter_ns);
    }
    
    fn calculate_percentiles(&self) -> JitterStats {
        if self.samples.is_empty() {
            return JitterStats::default();
        }
        
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        
        let len = sorted.len();
        let p50_idx = len / 2;
        let p99_idx = (len * 99) / 100;
        
        JitterStats {
            p50_ns: sorted[p50_idx],
            p99_ns: sorted[p99_idx.min(len - 1)],
            max_ns: *sorted.last().unwrap(),
            sample_count: len,
        }
    }
    
    fn reset(&mut self) {
        self.samples.clear();
    }
}

/// Jitter statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JitterStats {
    pub p50_ns: i64,
    pub p99_ns: i64,
    pub max_ns: i64,
    pub sample_count: usize,
}



/// HID write statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HidStats {
    pub total_writes: u64,
    pub total_time_ns: u64,
    pub avg_time_ns: u64,
    pub p99_time_ns: u64,
}



/// Complete counter snapshot for CI consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterSnapshot {
    /// Total RT ticks processed
    pub total_ticks: u64,
    
    /// Number of deadline misses
    pub deadline_misses: u64,
    
    /// Deadline miss rate (0.0 to 1.0)
    pub miss_rate: f64,
    
    /// Total HID writes
    pub total_hid_writes: u64,
    
    /// Writer buffer drops
    pub writer_drops: u64,
    
    /// Jitter statistics
    pub jitter: JitterStats,
    
    /// HID write statistics
    pub hid: HidStats,
    
    /// Session duration
    pub session_duration_ms: u64,
    
    /// Timestamp of snapshot
    pub timestamp_ns: u64,
}

impl Default for PerfCounters {
    fn default() -> Self {
        Self::new()
    }
}

impl PerfCounters {
    /// Create new performance counters
    pub fn new() -> Self {
        Self {
            total_ticks: AtomicU64::new(0),
            deadline_misses: AtomicU64::new(0),
            total_hid_writes: AtomicU64::new(0),
            hid_write_time_ns: AtomicU64::new(0),
            writer_drops: AtomicU64::new(0),
            jitter_samples: Mutex::new(JitterTracker::new(10000)), // ~40s at 250Hz
            session_start: Instant::now(),
        }
    }
    
    /// Record a trace event and update counters
    pub fn record_event(&self, event: &TraceEvent) {
        match &event.data {
            EventData::TickStart { .. } => {
                self.total_ticks.fetch_add(1, Ordering::Relaxed);
            }
            
            EventData::TickEnd { jitter_ns, .. } => {
                // Record jitter sample
                let mut tracker = self.jitter_samples.lock();
                tracker.add_sample(*jitter_ns);
            }
            
            EventData::HidWrite { duration_ns, .. } => {
                self.total_hid_writes.fetch_add(1, Ordering::Relaxed);
                self.hid_write_time_ns.fetch_add(*duration_ns, Ordering::Relaxed);
            }
            
            EventData::DeadlineMiss { .. } => {
                self.deadline_misses.fetch_add(1, Ordering::Relaxed);
            }
            
            EventData::WriterDrop { dropped_count, .. } => {
                self.writer_drops.fetch_add(*dropped_count, Ordering::Relaxed);
            }
            
            EventData::Custom { .. } => {
                // Custom events don't update standard counters
            }
        }
    }
    
    /// Get current counter snapshot
    pub fn snapshot(&self) -> CounterSnapshot {
        let total_ticks = self.total_ticks.load(Ordering::Relaxed);
        let deadline_misses = self.deadline_misses.load(Ordering::Relaxed);
        let total_hid_writes = self.total_hid_writes.load(Ordering::Relaxed);
        let hid_write_time_ns = self.hid_write_time_ns.load(Ordering::Relaxed);
        let writer_drops = self.writer_drops.load(Ordering::Relaxed);
        
        let miss_rate = if total_ticks > 0 {
            deadline_misses as f64 / total_ticks as f64
        } else {
            0.0
        };
        
        let jitter = {
            let tracker = self.jitter_samples.lock();
            tracker.calculate_percentiles()
        };
        
        let hid = HidStats {
            total_writes: total_hid_writes,
            total_time_ns: hid_write_time_ns,
            avg_time_ns: if total_hid_writes > 0 {
                hid_write_time_ns / total_hid_writes
            } else {
                0
            },
            p99_time_ns: 0, // TODO: Track HID write time percentiles
        };
        
        let session_duration_ms = self.session_start.elapsed().as_millis() as u64;
        
        CounterSnapshot {
            total_ticks,
            deadline_misses,
            miss_rate,
            total_hid_writes,
            writer_drops,
            jitter,
            hid,
            session_duration_ms,
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
        }
    }
    
    /// Reset all counters
    pub fn reset(&self) {
        self.total_ticks.store(0, Ordering::Relaxed);
        self.deadline_misses.store(0, Ordering::Relaxed);
        self.total_hid_writes.store(0, Ordering::Relaxed);
        self.hid_write_time_ns.store(0, Ordering::Relaxed);
        self.writer_drops.store(0, Ordering::Relaxed);
        
        let mut tracker = self.jitter_samples.lock();
        tracker.reset();
    }
    
    /// Check if counters exceed quality gates
    pub fn check_quality_gates(&self) -> QualityGateResult {
        let snapshot = self.snapshot();
        
        let mut violations = Vec::new();
        
        // QG-AX-Jitter: p99 jitter ≤ 0.5ms
        if snapshot.jitter.sample_count >= 1000 && snapshot.jitter.p99_ns.abs() > 500_000 {
            violations.push(QualityGateViolation {
                gate: "QG-AX-Jitter".to_string(),
                threshold: "p99 ≤ 0.5ms".to_string(),
                actual: format!("p99 = {:.3}ms", snapshot.jitter.p99_ns as f64 / 1_000_000.0),
            });
        }
        
        // QG-HID-Latency: p99 ≤ 300μs
        if snapshot.hid.p99_time_ns > 300_000 {
            violations.push(QualityGateViolation {
                gate: "QG-HID-Latency".to_string(),
                threshold: "p99 ≤ 300μs".to_string(),
                actual: format!("p99 = {:.1}μs", snapshot.hid.p99_time_ns as f64 / 1_000.0),
            });
        }
        
        // Check for excessive deadline misses (>1% is concerning)
        if snapshot.miss_rate > 0.01 {
            violations.push(QualityGateViolation {
                gate: "Deadline-Miss-Rate".to_string(),
                threshold: "≤ 1%".to_string(),
                actual: format!("{:.2}%", snapshot.miss_rate * 100.0),
            });
        }
        
        QualityGateResult {
            passed: violations.is_empty(),
            violations,
            snapshot,
        }
    }
}

/// Quality gate check result
#[derive(Debug, Clone)]
pub struct QualityGateResult {
    pub passed: bool,
    pub violations: Vec<QualityGateViolation>,
    pub snapshot: CounterSnapshot,
}

/// Quality gate violation details
#[derive(Debug, Clone)]
pub struct QualityGateViolation {
    pub gate: String,
    pub threshold: String,
    pub actual: String,
}

impl std::fmt::Display for QualityGateViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: expected {}, got {}", self.gate, self.threshold, self.actual)
    }
}

/// CI-friendly counter export
impl CounterSnapshot {
    /// Export as JSON for CI consumption
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
    
    /// Export as key-value pairs for CI metrics
    pub fn to_kv_pairs(&self) -> Vec<(String, String)> {
        vec![
            ("total_ticks".to_string(), self.total_ticks.to_string()),
            ("deadline_misses".to_string(), self.deadline_misses.to_string()),
            ("miss_rate_percent".to_string(), format!("{:.4}", self.miss_rate * 100.0)),
            ("jitter_p50_us".to_string(), format!("{:.1}", self.jitter.p50_ns as f64 / 1_000.0)),
            ("jitter_p99_us".to_string(), format!("{:.1}", self.jitter.p99_ns as f64 / 1_000.0)),
            ("hid_writes".to_string(), self.total_hid_writes.to_string()),
            ("hid_avg_us".to_string(), format!("{:.1}", self.hid.avg_time_ns as f64 / 1_000.0)),
            ("writer_drops".to_string(), self.writer_drops.to_string()),
            ("session_duration_s".to_string(), format!("{:.1}", self.session_duration_ms as f64 / 1_000.0)),
        ]
    }
    
    /// Check if this snapshot indicates a performance regression
    pub fn is_regression(&self, baseline: &CounterSnapshot) -> bool {
        // Jitter regression: >20% increase in p99
        let jitter_regression = if baseline.jitter.p99_ns != 0 {
            let jitter_increase = (self.jitter.p99_ns - baseline.jitter.p99_ns) as f64 / baseline.jitter.p99_ns as f64;
            jitter_increase > 0.20
        } else {
            false
        };
        
        // HID latency regression: >20% increase in average
        let hid_regression = if baseline.hid.avg_time_ns != 0 {
            let hid_increase = (self.hid.avg_time_ns as f64 - baseline.hid.avg_time_ns as f64) / baseline.hid.avg_time_ns as f64;
            hid_increase > 0.20
        } else {
            false
        };
        
        // Miss rate regression: >50% increase
        let miss_regression = if baseline.miss_rate > 0.0 {
            let miss_increase = (self.miss_rate - baseline.miss_rate) / baseline.miss_rate;
            miss_increase > 0.50
        } else {
            self.miss_rate > 0.005 // New misses above 0.5%
        };
        
        jitter_regression || hid_regression || miss_regression
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::TraceEvent;

    #[test]
    fn test_counter_recording() {
        let counters = PerfCounters::new();
        
        // Record some events
        counters.record_event(&TraceEvent::tick_start(1));
        counters.record_event(&TraceEvent::tick_end(1, 4000000, 1500));
        counters.record_event(&TraceEvent::hid_write(0x1234, 64, 250000));
        counters.record_event(&TraceEvent::deadline_miss(2, 2000000));
        counters.record_event(&TraceEvent::writer_drop("axis", 5));
        
        let snapshot = counters.snapshot();
        
        assert_eq!(snapshot.total_ticks, 1);
        assert_eq!(snapshot.deadline_misses, 1);
        assert_eq!(snapshot.total_hid_writes, 1);
        assert_eq!(snapshot.writer_drops, 5);
        assert_eq!(snapshot.jitter.sample_count, 1);
        assert_eq!(snapshot.jitter.p50_ns, 1500);
    }

    #[test]
    fn test_jitter_percentiles() {
        let counters = PerfCounters::new();
        
        // Record jitter samples
        for i in 0..1000 {
            let jitter = if i < 990 { 1000 } else { 10000 }; // 99% at 1μs, 1% at 10μs
            counters.record_event(&TraceEvent::tick_end(i, 4000000, jitter));
        }
        
        let snapshot = counters.snapshot();
        
        assert_eq!(snapshot.jitter.sample_count, 1000);
        assert_eq!(snapshot.jitter.p50_ns, 1000); // Median should be 1μs
        assert!(snapshot.jitter.p99_ns >= 10000); // p99 should be ~10μs
    }

    #[test]
    fn test_quality_gates() {
        let counters = PerfCounters::new();
        
        // Record good performance
        for i in 0..2000 {
            counters.record_event(&TraceEvent::tick_end(i, 4000000, 100)); // 100ns jitter
        }
        
        let result = counters.check_quality_gates();
        assert!(result.passed);
        assert!(result.violations.is_empty());
        
        // Reset and record bad performance
        counters.reset();
        for i in 0..2000 {
            counters.record_event(&TraceEvent::tick_end(i, 4000000, 1_000_000)); // 1ms jitter
        }
        
        let result = counters.check_quality_gates();
        assert!(!result.passed);
        assert!(!result.violations.is_empty());
    }

    #[test]
    fn test_regression_detection() {
        let baseline = CounterSnapshot {
            total_ticks: 1000,
            deadline_misses: 0,
            miss_rate: 0.0,
            total_hid_writes: 100,
            writer_drops: 0,
            jitter: JitterStats {
                p50_ns: 1000,
                p99_ns: 5000,
                max_ns: 10000,
                sample_count: 1000,
            },
            hid: HidStats {
                total_writes: 100,
                total_time_ns: 25_000_000, // 250μs average
                avg_time_ns: 250_000,
                p99_time_ns: 300_000,
            },
            session_duration_ms: 4000,
            timestamp_ns: 0,
        };
        
        // Good performance - no regression
        let good = CounterSnapshot {
            jitter: JitterStats { p99_ns: 5100, ..baseline.jitter.clone() },
            hid: HidStats { avg_time_ns: 260_000, ..baseline.hid.clone() },
            ..baseline.clone()
        };
        assert!(!good.is_regression(&baseline));
        
        // Jitter regression - >20% increase
        let jitter_regression = CounterSnapshot {
            jitter: JitterStats { p99_ns: 7000, ..baseline.jitter.clone() }, // 40% increase
            ..baseline.clone()
        };
        assert!(jitter_regression.is_regression(&baseline));
        
        // HID regression - >20% increase
        let hid_regression = CounterSnapshot {
            hid: HidStats { avg_time_ns: 320_000, ..baseline.hid.clone() }, // 28% increase
            ..baseline.clone()
        };
        assert!(hid_regression.is_regression(&baseline));
    }

    #[test]
    fn test_kv_export() {
        let snapshot = CounterSnapshot {
            total_ticks: 1000,
            deadline_misses: 5,
            miss_rate: 0.005,
            total_hid_writes: 100,
            writer_drops: 2,
            jitter: JitterStats {
                p50_ns: 1500,
                p99_ns: 4500,
                max_ns: 8000,
                sample_count: 1000,
            },
            hid: HidStats {
                total_writes: 100,
                total_time_ns: 25_000_000,
                avg_time_ns: 250_000,
                p99_time_ns: 300_000,
            },
            session_duration_ms: 4000,
            timestamp_ns: 0,
        };
        
        let kv_pairs = snapshot.to_kv_pairs();
        
        // Check key metrics are present
        assert!(kv_pairs.iter().any(|(k, v)| k == "jitter_p99_us" && v == "4.5"));
        assert!(kv_pairs.iter().any(|(k, v)| k == "miss_rate_percent" && v == "0.5000"));
        assert!(kv_pairs.iter().any(|(k, v)| k == "hid_avg_us" && v == "250.0"));
    }
}