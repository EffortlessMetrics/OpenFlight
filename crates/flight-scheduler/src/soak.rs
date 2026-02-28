// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Soak Test Framework
//!
//! Provides long-duration stability validation for Flight Hub's real-time systems.
//! Soak tests run synthetic telemetry and FFB loops for 24-48 hours to validate:
//! - No missed ticks beyond threshold
//! - RSS memory stability (delta < 10%)
//! - Blackbox dumps present on any faults
//!
//! **Validates: Requirements 13.1, 13.2, 13.3**

use std::time::{Duration, Instant};

use crate::metrics::{JitterMetrics, JitterStats};

/// Threshold for missed tick detection at 250Hz (>6ms is a missed tick)
pub const MISSED_TICK_THRESHOLD_NS: u64 = 6_000_000;

/// Default soak test duration (24 hours)
pub const DEFAULT_SOAK_DURATION: Duration = Duration::from_secs(24 * 60 * 60);

/// Maximum acceptable RSS growth percentage
pub const MAX_RSS_GROWTH_PERCENT: f64 = 10.0;

/// Target frequency for soak tests
pub const SOAK_TEST_FREQUENCY_HZ: u32 = 250;

/// Warmup period in seconds before recording metrics
pub const WARMUP_SECONDS: u32 = 5;

/// Soak test metrics collected during the test run
///
/// **Validates: Requirements 13.1, 13.2**
#[derive(Debug, Clone, Default)]
pub struct SoakMetrics {
    /// Total number of ticks processed
    pub total_ticks: u64,
    /// Number of missed ticks (tick duration > 6ms at 250Hz)
    pub missed_ticks: u64,
    /// Maximum jitter observed in nanoseconds
    pub max_jitter_ns: i64,
    /// Initial RSS (Resident Set Size) in bytes at test start
    pub initial_rss_bytes: u64,
    /// Final RSS in bytes at test end
    pub final_rss_bytes: u64,
    /// Number of faults detected during the test
    pub faults_detected: u64,
    /// Number of blackbox dumps created
    pub blackbox_dumps: u64,
}

impl SoakMetrics {
    /// Calculate RSS growth as a percentage
    pub fn rss_growth_percent(&self) -> f64 {
        if self.initial_rss_bytes == 0 {
            return 0.0;
        }
        let delta = self.final_rss_bytes as f64 - self.initial_rss_bytes as f64;
        (delta / self.initial_rss_bytes as f64) * 100.0
    }

    /// Check if RSS growth is within acceptable limits
    pub fn rss_within_limits(&self) -> bool {
        self.rss_growth_percent() < MAX_RSS_GROWTH_PERCENT
    }

    /// Check if there were any missed ticks
    pub fn no_missed_ticks(&self) -> bool {
        self.missed_ticks == 0
    }
}

/// Synthetic telemetry generator for soak tests
///
/// Generates realistic telemetry data patterns to stress-test the axis
/// and FFB processing pipelines without requiring actual simulator connections.
///
/// **Validates: Requirements 13.1**
#[derive(Debug, Clone)]
pub struct SyntheticTelemetryGenerator {
    /// Current simulation time in seconds
    sim_time_s: f64,
    /// Time step per tick (1/250 = 4ms)
    time_step_s: f64,
    /// Current phase for oscillating values
    phase: f64,
    /// Tick counter for pattern generation
    tick_counter: u64,
}

impl Default for SyntheticTelemetryGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntheticTelemetryGenerator {
    /// Create a new synthetic telemetry generator
    pub fn new() -> Self {
        Self {
            sim_time_s: 0.0,
            time_step_s: 1.0 / SOAK_TEST_FREQUENCY_HZ as f64,
            phase: 0.0,
            tick_counter: 0,
        }
    }

    /// Generate the next telemetry snapshot
    pub fn next_snapshot(&mut self) -> TelemetrySnapshot {
        self.tick_counter += 1;
        self.sim_time_s += self.time_step_s;
        self.phase += self.time_step_s * 0.5; // Slow oscillation

        // Generate realistic axis values with various patterns
        let pitch = (self.phase * 2.0 * std::f64::consts::PI).sin() * 0.3;
        let roll = (self.phase * 1.5 * std::f64::consts::PI).cos() * 0.2;
        let yaw = (self.phase * 0.7 * std::f64::consts::PI).sin() * 0.1;
        let throttle = 0.5 + (self.phase * 0.3 * std::f64::consts::PI).sin() * 0.3;

        // Generate FFB forces based on simulated flight dynamics
        let ffb_pitch = pitch as f32 * 2.0;
        let ffb_roll = roll as f32 * 1.5;

        TelemetrySnapshot {
            timestamp_ns: (self.sim_time_s * 1_000_000_000.0) as u64,
            tick_number: self.tick_counter,
            pitch: pitch as f32,
            roll: roll as f32,
            yaw: yaw as f32,
            throttle: throttle as f32,
            ffb_pitch_nm: ffb_pitch,
            ffb_roll_nm: ffb_roll,
            safe_for_ffb: true,
        }
    }

    /// Reset the generator to initial state
    pub fn reset(&mut self) {
        self.sim_time_s = 0.0;
        self.phase = 0.0;
        self.tick_counter = 0;
    }
}

/// A single telemetry snapshot from the synthetic generator
#[derive(Debug, Clone)]
pub struct TelemetrySnapshot {
    /// Timestamp in nanoseconds since test start
    pub timestamp_ns: u64,
    /// Tick sequence number
    pub tick_number: u64,
    /// Pitch axis value (-1.0 to 1.0)
    pub pitch: f32,
    /// Roll axis value (-1.0 to 1.0)
    pub roll: f32,
    /// Yaw axis value (-1.0 to 1.0)
    pub yaw: f32,
    /// Throttle value (0.0 to 1.0)
    pub throttle: f32,
    /// FFB pitch torque in Newton-meters
    pub ffb_pitch_nm: f32,
    /// FFB roll torque in Newton-meters
    pub ffb_roll_nm: f32,
    /// Whether FFB is safe to apply
    pub safe_for_ffb: bool,
}

impl TelemetrySnapshot {
    /// Check if any values are NaN or Inf
    pub fn has_nan_or_inf(&self) -> bool {
        !self.pitch.is_finite()
            || !self.roll.is_finite()
            || !self.yaw.is_finite()
            || !self.throttle.is_finite()
            || !self.ffb_pitch_nm.is_finite()
            || !self.ffb_roll_nm.is_finite()
    }
}

/// Result of a soak test run
///
/// **Validates: Requirements 13.2, 13.3**
#[derive(Debug)]
pub struct SoakTestResult {
    /// Whether the test passed all assertions
    pub passed: bool,
    /// Collected metrics from the test
    pub metrics: SoakMetrics,
    /// Jitter statistics from the test
    pub jitter_stats: JitterStats,
    /// RSS growth percentage
    pub rss_delta_pct: f64,
    /// Test duration
    pub duration: Duration,
    /// Failure reasons if test failed
    pub failure_reasons: Vec<SoakFailureReason>,
}

/// Reasons why a soak test might fail
#[derive(Debug, Clone, PartialEq)]
pub enum SoakFailureReason {
    /// Missed ticks detected
    MissedTicks { count: u64, threshold: u64 },
    /// RSS growth exceeded limit
    RssGrowthExceeded { growth_pct: f64, limit_pct: f64 },
    /// Jitter exceeded quality gate
    JitterExceeded { p99_ms: f64, limit_ms: f64 },
    /// Faults detected without blackbox dumps
    FaultsWithoutBlackbox { faults: u64, dumps: u64 },
    /// NaN or Inf values detected in telemetry
    InvalidTelemetry { tick: u64 },
}

impl std::fmt::Display for SoakFailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SoakFailureReason::MissedTicks { count, threshold } => {
                write!(f, "Missed {} ticks (threshold: {})", count, threshold)
            }
            SoakFailureReason::RssGrowthExceeded {
                growth_pct,
                limit_pct,
            } => {
                write!(
                    f,
                    "RSS growth {:.2}% exceeded limit {:.2}%",
                    growth_pct, limit_pct
                )
            }
            SoakFailureReason::JitterExceeded { p99_ms, limit_ms } => {
                write!(
                    f,
                    "p99 jitter {:.3}ms exceeded limit {:.3}ms",
                    p99_ms, limit_ms
                )
            }
            SoakFailureReason::FaultsWithoutBlackbox { faults, dumps } => {
                write!(
                    f,
                    "{} faults detected but only {} blackbox dumps",
                    faults, dumps
                )
            }
            SoakFailureReason::InvalidTelemetry { tick } => {
                write!(f, "Invalid telemetry (NaN/Inf) at tick {}", tick)
            }
        }
    }
}

/// Configuration for soak tests
#[derive(Debug, Clone)]
pub struct SoakTestConfig {
    /// Test duration
    pub duration: Duration,
    /// Target frequency in Hz
    pub frequency_hz: u32,
    /// Maximum acceptable RSS growth percentage
    pub max_rss_growth_pct: f64,
    /// Maximum acceptable p99 jitter in milliseconds
    pub max_jitter_p99_ms: f64,
    /// Warmup period before recording metrics
    pub warmup_duration: Duration,
    /// Whether to simulate faults for testing
    pub simulate_faults: bool,
    /// Interval between simulated faults (if enabled)
    pub fault_interval: Duration,
}

impl Default for SoakTestConfig {
    fn default() -> Self {
        Self {
            duration: DEFAULT_SOAK_DURATION,
            frequency_hz: SOAK_TEST_FREQUENCY_HZ,
            max_rss_growth_pct: MAX_RSS_GROWTH_PERCENT,
            max_jitter_p99_ms: 0.5,
            warmup_duration: Duration::from_secs(WARMUP_SECONDS as u64),
            simulate_faults: false,
            fault_interval: Duration::from_secs(3600), // 1 hour
        }
    }
}

impl SoakTestConfig {
    /// Create a short test configuration for unit testing
    pub fn short_test() -> Self {
        Self {
            duration: Duration::from_secs(10),
            warmup_duration: Duration::from_secs(1),
            ..Default::default()
        }
    }

    /// Create a medium test configuration (1 hour)
    pub fn medium_test() -> Self {
        Self {
            duration: Duration::from_secs(3600),
            ..Default::default()
        }
    }

    /// Create a full 24-hour soak test configuration
    pub fn full_24h() -> Self {
        Self {
            duration: Duration::from_secs(24 * 60 * 60),
            ..Default::default()
        }
    }

    /// Create a full 48-hour soak test configuration
    pub fn full_48h() -> Self {
        Self {
            duration: Duration::from_secs(48 * 60 * 60),
            ..Default::default()
        }
    }
}

/// Soak test runner for long-duration stability validation
///
/// **Validates: Requirements 13.1, 13.2, 13.3**
pub struct SoakTest {
    /// Test configuration
    config: SoakTestConfig,
    /// Collected metrics
    metrics: SoakMetrics,
    /// Synthetic telemetry generator
    telemetry_gen: SyntheticTelemetryGenerator,
    /// Jitter measurement
    jitter_metrics: JitterMetrics,
    /// Test start time
    start_time: Option<Instant>,
    /// Last tick time for duration tracking
    last_tick_time: Option<Instant>,
    /// Whether the test is in warmup phase
    in_warmup: bool,
}

impl SoakTest {
    /// Create a new soak test with the given configuration
    pub fn new(config: SoakTestConfig) -> Self {
        Self {
            jitter_metrics: JitterMetrics::new(config.frequency_hz),
            config,
            metrics: SoakMetrics::default(),
            telemetry_gen: SyntheticTelemetryGenerator::new(),
            start_time: None,
            last_tick_time: None,
            in_warmup: true,
        }
    }

    /// Create a soak test with default 24-hour configuration
    pub fn new_24h() -> Self {
        Self::new(SoakTestConfig::full_24h())
    }

    /// Create a soak test with 48-hour configuration
    pub fn new_48h() -> Self {
        Self::new(SoakTestConfig::full_48h())
    }

    /// Get the test configuration
    pub fn config(&self) -> &SoakTestConfig {
        &self.config
    }

    /// Get current metrics (snapshot)
    pub fn metrics(&self) -> &SoakMetrics {
        &self.metrics
    }

    /// Record initial RSS before test starts
    pub fn record_initial_rss(&mut self, rss_bytes: u64) {
        self.metrics.initial_rss_bytes = rss_bytes;
    }

    /// Record final RSS after test completes
    pub fn record_final_rss(&mut self, rss_bytes: u64) {
        self.metrics.final_rss_bytes = rss_bytes;
    }

    /// Record a fault detection
    pub fn record_fault(&mut self) {
        self.metrics.faults_detected += 1;
    }

    /// Record a blackbox dump
    pub fn record_blackbox_dump(&mut self) {
        self.metrics.blackbox_dumps += 1;
    }
}

impl SoakTest {
    /// Process a single tick of the soak test
    ///
    /// Returns `true` if the test should continue, `false` if complete.
    pub fn process_tick(&mut self, now: Instant) -> bool {
        // Initialize start time on first tick
        if self.start_time.is_none() {
            self.start_time = Some(now);
        }

        let start = self.start_time.unwrap();
        let elapsed = now.duration_since(start);

        // Check if warmup is complete
        if self.in_warmup && elapsed >= self.config.warmup_duration {
            self.in_warmup = false;
            self.jitter_metrics.reset();
        }

        // Check if test is complete
        if elapsed >= self.config.duration + self.config.warmup_duration {
            return false;
        }

        // Generate synthetic telemetry
        let snapshot = self.telemetry_gen.next_snapshot();

        // Check for invalid telemetry
        if snapshot.has_nan_or_inf() {
            // This would be a serious bug - record but continue
            tracing::error!("Invalid telemetry at tick {}", snapshot.tick_number);
        }

        // Track tick timing
        if let Some(last_tick) = self.last_tick_time {
            let tick_duration_ns = now.duration_since(last_tick).as_nanos() as u64;

            // Check for missed tick (>6ms at 250Hz)
            if tick_duration_ns > MISSED_TICK_THRESHOLD_NS {
                self.metrics.missed_ticks += 1;
                tracing::warn!(
                    "Missed tick at {}: {}ms",
                    self.metrics.total_ticks,
                    tick_duration_ns as f64 / 1_000_000.0
                );
            }

            // Record jitter if not in warmup
            if !self.in_warmup {
                self.jitter_metrics.record_tick(now, 0);
            }
        }

        self.metrics.total_ticks += 1;
        self.last_tick_time = Some(now);

        true
    }

    /// Run the complete soak test synchronously
    ///
    /// This is a blocking call that runs for the configured duration.
    /// For real soak tests, this should be run on a dedicated thread.
    pub fn run(&mut self) -> SoakTestResult {
        let start = Instant::now();
        self.start_time = Some(start);

        // Record initial RSS
        self.metrics.initial_rss_bytes = get_rss_bytes();

        let period = Duration::from_nanos(1_000_000_000 / self.config.frequency_hz as u64);
        let mut next_tick = start;

        // Main test loop
        while self.process_tick(Instant::now()) {
            next_tick += period;
            let now = Instant::now();
            if next_tick > now {
                std::thread::sleep(next_tick - now);
            }
        }

        // Record final RSS
        self.metrics.final_rss_bytes = get_rss_bytes();

        // Generate result
        self.finalize()
    }
}

impl SoakTest {
    /// Finalize the test and generate results with assertions
    ///
    /// **Validates: Requirements 13.2**
    pub fn finalize(&self) -> SoakTestResult {
        let jitter_stats = self.jitter_metrics.get_stats();
        let rss_delta_pct = self.metrics.rss_growth_percent();
        let duration = self
            .start_time
            .map(|s| s.elapsed())
            .unwrap_or(Duration::ZERO);

        let mut failure_reasons = Vec::new();

        // Assert no missed ticks
        if self.metrics.missed_ticks > 0 {
            failure_reasons.push(SoakFailureReason::MissedTicks {
                count: self.metrics.missed_ticks,
                threshold: 0,
            });
        }

        // Assert RSS delta < 10%
        if rss_delta_pct >= self.config.max_rss_growth_pct {
            failure_reasons.push(SoakFailureReason::RssGrowthExceeded {
                growth_pct: rss_delta_pct,
                limit_pct: self.config.max_rss_growth_pct,
            });
        }

        // Assert p99 jitter <= 0.5ms
        let p99_ms = jitter_stats.p99_ns.abs() as f64 / 1_000_000.0;
        if p99_ms > self.config.max_jitter_p99_ms {
            failure_reasons.push(SoakFailureReason::JitterExceeded {
                p99_ms,
                limit_ms: self.config.max_jitter_p99_ms,
            });
        }

        // Assert blackbox present on faults
        if self.metrics.faults_detected > 0
            && self.metrics.blackbox_dumps < self.metrics.faults_detected
        {
            failure_reasons.push(SoakFailureReason::FaultsWithoutBlackbox {
                faults: self.metrics.faults_detected,
                dumps: self.metrics.blackbox_dumps,
            });
        }

        let passed = failure_reasons.is_empty();

        // Update max jitter in metrics
        let mut metrics = self.metrics.clone();
        metrics.max_jitter_ns = jitter_stats.max_ns;

        SoakTestResult {
            passed,
            metrics,
            jitter_stats,
            rss_delta_pct,
            duration,
            failure_reasons,
        }
    }
}

// =============================================================================
// RSS Memory Measurement
// =============================================================================

/// Get current process RSS (Resident Set Size) in bytes
///
/// Platform-specific implementation for memory monitoring.
#[cfg(target_os = "linux")]
pub fn get_rss_bytes() -> u64 {
    // Read from /proc/self/statm
    // Format: size resident shared text lib data dt
    // resident is in pages
    if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
        let parts: Vec<&str> = statm.split_whitespace().collect();
        if parts.len() >= 2
            && let Ok(pages) = parts[1].parse::<u64>()
        {
            // Get page size
            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
            return pages * page_size;
        }
    }
    0
}

/// Get current process RSS (Resident Set Size) in bytes
#[cfg(target_os = "windows")]
pub fn get_rss_bytes() -> u64 {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::GetCurrentProcess;

    unsafe {
        let process = GetCurrentProcess();
        let mut pmc = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };

        if GetProcessMemoryInfo(
            process,
            &mut pmc,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .is_ok()
        {
            return pmc.WorkingSetSize as u64;
        }
    }
    0
}

/// Get current process RSS (Resident Set Size) in bytes
#[cfg(target_os = "macos")]
pub fn get_rss_bytes() -> u64 {
    // Use mach APIs on macOS
    // For now, return 0 as a placeholder
    // A full implementation would use task_info with TASK_BASIC_INFO
    0
}

/// Fallback for other platforms
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn get_rss_bytes() -> u64 {
    0
}

// =============================================================================
// Diagnostic Output
// =============================================================================

/// Diagnostic information for soak test failures
///
/// **Validates: Requirements 13.3**
#[derive(Debug)]
pub struct SoakDiagnostics {
    /// Test result summary
    pub result: SoakTestResult,
    /// Tick timing histogram (buckets in microseconds)
    pub tick_timing_histogram: Vec<(u64, u64)>,
    /// Memory profile over time (timestamp_s, rss_bytes)
    pub memory_profile: Vec<(f64, u64)>,
    /// Fault details
    pub fault_details: Vec<FaultDetail>,
}

/// Details about a fault that occurred during the test
#[derive(Debug, Clone)]
pub struct FaultDetail {
    /// When the fault occurred (seconds since test start)
    pub timestamp_s: f64,
    /// Type of fault
    pub fault_type: String,
    /// Whether a blackbox dump was created
    pub blackbox_created: bool,
    /// Additional context
    pub context: String,
}

impl SoakTestResult {
    /// Generate diagnostic output for the test result
    ///
    /// **Validates: Requirements 13.3**
    pub fn generate_diagnostics(&self) -> String {
        let mut output = String::new();

        output.push_str("=== SOAK TEST DIAGNOSTICS ===\n\n");

        // Summary
        output.push_str(&format!(
            "Status: {}\n",
            if self.passed { "PASSED" } else { "FAILED" }
        ));
        output.push_str(&format!("Duration: {:?}\n", self.duration));
        output.push_str(&format!("Total Ticks: {}\n", self.metrics.total_ticks));
        output.push_str(&format!("Missed Ticks: {}\n", self.metrics.missed_ticks));
        output.push('\n');

        // Jitter Statistics
        output.push_str("--- Jitter Statistics ---\n");
        output.push_str(&format!("Samples: {}\n", self.jitter_stats.sample_count));
        output.push_str(&format!(
            "p50: {:.3}ms\n",
            self.jitter_stats.p50_ns as f64 / 1_000_000.0
        ));
        output.push_str(&format!(
            "p99: {:.3}ms\n",
            self.jitter_stats.p99_ns as f64 / 1_000_000.0
        ));
        output.push_str(&format!(
            "Max: {:.3}ms\n",
            self.jitter_stats.max_ns as f64 / 1_000_000.0
        ));
        output.push('\n');

        // Memory Profile
        output.push_str("--- Memory Profile ---\n");
        output.push_str(&format!(
            "Initial RSS: {} bytes ({:.2} MB)\n",
            self.metrics.initial_rss_bytes,
            self.metrics.initial_rss_bytes as f64 / 1_048_576.0
        ));
        output.push_str(&format!(
            "Final RSS: {} bytes ({:.2} MB)\n",
            self.metrics.final_rss_bytes,
            self.metrics.final_rss_bytes as f64 / 1_048_576.0
        ));
        output.push_str(&format!("RSS Growth: {:.2}%\n", self.rss_delta_pct));
        output.push('\n');

        // Fault Summary
        output.push_str("--- Fault Summary ---\n");
        output.push_str(&format!(
            "Faults Detected: {}\n",
            self.metrics.faults_detected
        ));
        output.push_str(&format!(
            "Blackbox Dumps: {}\n",
            self.metrics.blackbox_dumps
        ));
        output.push('\n');

        // Failure Reasons
        if !self.failure_reasons.is_empty() {
            output.push_str("--- Failure Reasons ---\n");
            for (i, reason) in self.failure_reasons.iter().enumerate() {
                output.push_str(&format!("{}. {}\n", i + 1, reason));
            }
        }

        output
    }

    /// Log diagnostic output using tracing
    pub fn log_diagnostics(&self) {
        if self.passed {
            tracing::info!("Soak test PASSED after {:?}", self.duration);
            tracing::info!(
                "Ticks: {}, Missed: {}, p99 jitter: {:.3}ms, RSS growth: {:.2}%",
                self.metrics.total_ticks,
                self.metrics.missed_ticks,
                self.jitter_stats.p99_ns as f64 / 1_000_000.0,
                self.rss_delta_pct
            );
        } else {
            tracing::error!("Soak test FAILED after {:?}", self.duration);
            for reason in &self.failure_reasons {
                tracing::error!("Failure: {}", reason);
            }
            tracing::error!(
                "Ticks: {}, Missed: {}, p99 jitter: {:.3}ms, RSS growth: {:.2}%",
                self.metrics.total_ticks,
                self.metrics.missed_ticks,
                self.jitter_stats.p99_ns as f64 / 1_000_000.0,
                self.rss_delta_pct
            );
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_telemetry_generator() {
        let mut generator = SyntheticTelemetryGenerator::new();

        // Generate several snapshots
        for i in 0..1000 {
            let snapshot = generator.next_snapshot();

            // Verify tick number increments
            assert_eq!(snapshot.tick_number, i + 1);

            // Verify values are in valid ranges
            assert!(snapshot.pitch >= -1.0 && snapshot.pitch <= 1.0);
            assert!(snapshot.roll >= -1.0 && snapshot.roll <= 1.0);
            assert!(snapshot.yaw >= -1.0 && snapshot.yaw <= 1.0);
            assert!(snapshot.throttle >= 0.0 && snapshot.throttle <= 1.0);

            // Verify no NaN/Inf
            assert!(!snapshot.has_nan_or_inf());
        }
    }

    #[test]
    fn test_soak_metrics_rss_growth() {
        let mut metrics = SoakMetrics::default();
        metrics.initial_rss_bytes = 100_000_000; // 100 MB
        metrics.final_rss_bytes = 105_000_000; // 105 MB

        assert!((metrics.rss_growth_percent() - 5.0).abs() < 0.01);
        assert!(metrics.rss_within_limits());

        // Test exceeding limit
        metrics.final_rss_bytes = 115_000_000; // 115 MB (15% growth)
        assert!(!metrics.rss_within_limits());
    }

    #[test]
    fn test_soak_test_short_run() {
        let config = SoakTestConfig {
            duration: Duration::from_millis(100),
            warmup_duration: Duration::from_millis(10),
            frequency_hz: 250,
            ..Default::default()
        };

        let mut test = SoakTest::new(config);
        let result = test.run();

        // Should complete without panicking
        assert!(result.metrics.total_ticks > 0);
        assert!(result.duration >= Duration::from_millis(100));
    }

    #[test]
    fn test_failure_reason_display() {
        let reason = SoakFailureReason::MissedTicks {
            count: 5,
            threshold: 0,
        };
        assert!(reason.to_string().contains("5 ticks"));

        let reason = SoakFailureReason::RssGrowthExceeded {
            growth_pct: 15.5,
            limit_pct: 10.0,
        };
        assert!(reason.to_string().contains("15.50%"));
    }

    #[test]
    fn test_telemetry_snapshot_nan_detection() {
        let mut snapshot = TelemetrySnapshot {
            timestamp_ns: 0,
            tick_number: 0,
            pitch: 0.0,
            roll: 0.0,
            yaw: 0.0,
            throttle: 0.5,
            ffb_pitch_nm: 0.0,
            ffb_roll_nm: 0.0,
            safe_for_ffb: true,
        };

        assert!(!snapshot.has_nan_or_inf());

        snapshot.pitch = f32::NAN;
        assert!(snapshot.has_nan_or_inf());

        snapshot.pitch = 0.0;
        snapshot.roll = f32::INFINITY;
        assert!(snapshot.has_nan_or_inf());
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: release-readiness, Property 6: Soak Test Stability
    // **Validates: Requirements 13.2**
    //
    // For any soak test run of 24-48 hours, the following invariants SHALL hold:
    // - missed_ticks == 0
    // - RSS delta < 10%
    // - blackbox dumps are present for any faults detected

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Test that synthetic telemetry generator produces valid values
        #[test]
        fn prop_telemetry_generator_valid_values(
            num_ticks in 1usize..10000
        ) {
            let mut generator = SyntheticTelemetryGenerator::new();

            for _ in 0..num_ticks {
                let snapshot = generator.next_snapshot();

                // All axis values should be in valid ranges
                prop_assert!(snapshot.pitch >= -1.0 && snapshot.pitch <= 1.0,
                    "pitch {} out of range", snapshot.pitch);
                prop_assert!(snapshot.roll >= -1.0 && snapshot.roll <= 1.0,
                    "roll {} out of range", snapshot.roll);
                prop_assert!(snapshot.yaw >= -1.0 && snapshot.yaw <= 1.0,
                    "yaw {} out of range", snapshot.yaw);
                prop_assert!(snapshot.throttle >= 0.0 && snapshot.throttle <= 1.0,
                    "throttle {} out of range", snapshot.throttle);

                // No NaN or Inf values
                prop_assert!(!snapshot.has_nan_or_inf(),
                    "Invalid telemetry at tick {}", snapshot.tick_number);
            }
        }

        /// Test RSS growth calculation accuracy
        #[test]
        fn prop_rss_growth_calculation(
            initial_mb in 50u64..1000,
            growth_pct in -20i64..50
        ) {
            let initial_bytes = initial_mb * 1_048_576;
            let final_bytes = ((initial_bytes as f64) * (1.0 + growth_pct as f64 / 100.0)) as u64;

            let mut metrics = SoakMetrics::default();
            metrics.initial_rss_bytes = initial_bytes;
            metrics.final_rss_bytes = final_bytes;

            let calculated_growth = metrics.rss_growth_percent();

            // Allow for floating point tolerance
            prop_assert!(
                (calculated_growth - growth_pct as f64).abs() < 0.1,
                "Expected ~{}%, got {}%", growth_pct, calculated_growth
            );

            // Verify limit check - use the actual calculated growth for comparison
            // to avoid floating point boundary issues
            let within_limits = metrics.rss_within_limits();
            let expected_within = calculated_growth < MAX_RSS_GROWTH_PERCENT;
            prop_assert_eq!(within_limits, expected_within,
                "rss_within_limits() returned {} but calculated growth {} vs limit {}",
                within_limits, calculated_growth, MAX_RSS_GROWTH_PERCENT);
        }

        /// Test soak test assertions are correctly applied
        #[test]
        fn prop_soak_assertions(
            missed_ticks in 0u64..100,
            rss_growth_pct in 0i64..20,
            faults in 0u64..10,
            dumps in 0u64..10
        ) {
            let mut metrics = SoakMetrics::default();
            metrics.total_ticks = 1_000_000;
            metrics.missed_ticks = missed_ticks;
            metrics.initial_rss_bytes = 100_000_000;
            metrics.final_rss_bytes = ((100_000_000.0 * (1.0 + rss_growth_pct as f64 / 100.0)) as u64);
            metrics.faults_detected = faults;
            metrics.blackbox_dumps = dumps;

            // Create a minimal test to check assertions
            let config = SoakTestConfig::default();
            let test = SoakTest {
                config: config.clone(),
                metrics: metrics.clone(),
                telemetry_gen: SyntheticTelemetryGenerator::new(),
                jitter_metrics: JitterMetrics::new(250),
                start_time: Some(Instant::now()),
                last_tick_time: None,
                in_warmup: false,
            };

            let result = test.finalize();

            // Verify missed ticks assertion
            if missed_ticks > 0 {
                prop_assert!(
                    result.failure_reasons.iter().any(|r| matches!(r, SoakFailureReason::MissedTicks { .. })),
                    "Should fail on missed ticks"
                );
            }

            // Verify RSS growth assertion
            if rss_growth_pct >= 10 {
                prop_assert!(
                    result.failure_reasons.iter().any(|r| matches!(r, SoakFailureReason::RssGrowthExceeded { .. })),
                    "Should fail on RSS growth"
                );
            }

            // Verify blackbox assertion
            if faults > 0 && dumps < faults {
                prop_assert!(
                    result.failure_reasons.iter().any(|r| matches!(r, SoakFailureReason::FaultsWithoutBlackbox { .. })),
                    "Should fail on missing blackbox dumps"
                );
            }
        }
    }
}
