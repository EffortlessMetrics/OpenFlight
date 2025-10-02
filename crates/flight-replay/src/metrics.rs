//! Replay metrics and performance tracking

use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// Comprehensive replay metrics
pub struct ReplayMetrics {
    enabled: bool,
    start_time: Option<Instant>,
    frames_processed: u64,
    axis_outputs: Vec<f32>,
    ffb_outputs: Vec<f32>,
    processing_times: Vec<Duration>,
    timestamps: Vec<u64>,
}

/// Performance metrics for replay execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Total execution time
    pub total_duration: Duration,
    /// Average frame processing time
    pub avg_frame_time: Duration,
    /// Maximum frame processing time
    pub max_frame_time: Duration,
    /// Minimum frame processing time
    pub min_frame_time: Duration,
    /// Frames processed per second
    pub frames_per_second: f64,
    /// Memory usage statistics
    pub memory_stats: MemoryStats,
}

/// Accuracy metrics for output validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyMetrics {
    /// Number of frames analyzed
    pub frames_analyzed: u64,
    /// Axis output statistics
    pub axis_stats: OutputStats,
    /// FFB output statistics
    pub ffb_stats: OutputStats,
    /// Timing accuracy statistics
    pub timing_stats: TimingAccuracyStats,
}

/// Statistics for a specific output type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputStats {
    /// Minimum value observed
    pub min_value: f32,
    /// Maximum value observed
    pub max_value: f32,
    /// Average value
    pub avg_value: f32,
    /// Standard deviation
    pub std_deviation: f32,
    /// Range (max - min)
    pub range: f32,
}

/// Memory usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Peak memory usage in bytes
    pub peak_memory_bytes: u64,
    /// Average memory usage in bytes
    pub avg_memory_bytes: u64,
    /// Number of allocations tracked
    pub allocation_count: u64,
}

/// Timing accuracy statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingAccuracyStats {
    /// Average timing error in nanoseconds
    pub avg_timing_error_ns: u64,
    /// Maximum timing error in nanoseconds
    pub max_timing_error_ns: u64,
    /// Standard deviation of timing errors
    pub timing_error_std_ns: u64,
    /// Percentage of frames within timing tolerance
    pub frames_within_tolerance_pct: f32,
}

impl ReplayMetrics {
    /// Create new replay metrics
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            start_time: None,
            frames_processed: 0,
            axis_outputs: Vec::new(),
            ffb_outputs: Vec::new(),
            processing_times: Vec::new(),
            timestamps: Vec::new(),
        }
    }

    /// Start metrics collection
    pub fn start(&mut self) {
        if self.enabled {
            self.start_time = Some(Instant::now());
        }
    }

    /// Record a processed frame
    pub fn record_frame_processed(&mut self, timestamp_ns: u64, axis_output: f32, ffb_output: f32) {
        if !self.enabled {
            return;
        }

        let frame_start = Instant::now();
        
        self.frames_processed += 1;
        self.axis_outputs.push(axis_output);
        self.ffb_outputs.push(ffb_output);
        self.timestamps.push(timestamp_ns);
        
        // Record processing time (simulated)
        let processing_time = frame_start.elapsed();
        self.processing_times.push(processing_time);
    }

    /// Reset metrics for new replay
    pub fn reset(&mut self) {
        self.start_time = None;
        self.frames_processed = 0;
        self.axis_outputs.clear();
        self.ffb_outputs.clear();
        self.processing_times.clear();
        self.timestamps.clear();
    }

    /// Get number of frames processed
    pub fn get_frames_processed(&self) -> u64 {
        self.frames_processed
    }

    /// Get performance metrics
    pub fn get_performance_metrics(&self) -> PerformanceMetrics {
        let total_duration = self.start_time
            .map(|start| start.elapsed())
            .unwrap_or(Duration::from_secs(0));

        let (avg_frame_time, max_frame_time, min_frame_time) = if !self.processing_times.is_empty() {
            let total_processing: Duration = self.processing_times.iter().sum();
            let avg = total_processing / self.processing_times.len() as u32;
            let max = self.processing_times.iter().max().copied().unwrap_or(Duration::from_secs(0));
            let min = self.processing_times.iter().min().copied().unwrap_or(Duration::from_secs(0));
            (avg, max, min)
        } else {
            (Duration::from_secs(0), Duration::from_secs(0), Duration::from_secs(0))
        };

        let frames_per_second = if total_duration.as_secs_f64() > 0.0 {
            self.frames_processed as f64 / total_duration.as_secs_f64()
        } else {
            0.0
        };

        PerformanceMetrics {
            total_duration,
            avg_frame_time,
            max_frame_time,
            min_frame_time,
            frames_per_second,
            memory_stats: self.get_memory_stats(),
        }
    }

    /// Get accuracy metrics
    pub fn get_accuracy_metrics(&self) -> AccuracyMetrics {
        AccuracyMetrics {
            frames_analyzed: self.frames_processed,
            axis_stats: self.calculate_output_stats(&self.axis_outputs),
            ffb_stats: self.calculate_output_stats(&self.ffb_outputs),
            timing_stats: self.calculate_timing_stats(),
        }
    }

    /// Calculate statistics for output values
    fn calculate_output_stats(&self, values: &[f32]) -> OutputStats {
        if values.is_empty() {
            return OutputStats {
                min_value: 0.0,
                max_value: 0.0,
                avg_value: 0.0,
                std_deviation: 0.0,
                range: 0.0,
            };
        }

        let min_value = values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_value = values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let sum: f32 = values.iter().sum();
        let avg_value = sum / values.len() as f32;
        
        let variance: f32 = values.iter()
            .map(|&x| (x - avg_value).powi(2))
            .sum::<f32>() / values.len() as f32;
        let std_deviation = variance.sqrt();
        
        let range = max_value - min_value;

        OutputStats {
            min_value,
            max_value,
            avg_value,
            std_deviation,
            range,
        }
    }

    /// Calculate timing accuracy statistics
    fn calculate_timing_stats(&self) -> TimingAccuracyStats {
        if self.timestamps.len() < 2 {
            return TimingAccuracyStats {
                avg_timing_error_ns: 0,
                max_timing_error_ns: 0,
                timing_error_std_ns: 0,
                frames_within_tolerance_pct: 100.0,
            };
        }

        // Calculate timing errors (difference from expected 4ms intervals)
        let expected_interval_ns = 4_000_000u64; // 4ms = 250Hz
        let mut timing_errors = Vec::new();
        
        for i in 1..self.timestamps.len() {
            let actual_interval = self.timestamps[i] - self.timestamps[i-1];
            let error = if actual_interval > expected_interval_ns {
                actual_interval - expected_interval_ns
            } else {
                expected_interval_ns - actual_interval
            };
            timing_errors.push(error);
        }

        let avg_timing_error_ns = if !timing_errors.is_empty() {
            timing_errors.iter().sum::<u64>() / timing_errors.len() as u64
        } else {
            0
        };

        let max_timing_error_ns = timing_errors.iter().max().copied().unwrap_or(0);

        // Calculate standard deviation of timing errors
        let avg_error_f64 = avg_timing_error_ns as f64;
        let variance = timing_errors.iter()
            .map(|&error| (error as f64 - avg_error_f64).powi(2))
            .sum::<f64>() / timing_errors.len() as f64;
        let timing_error_std_ns = variance.sqrt() as u64;

        // Calculate percentage within tolerance (500μs = 500,000ns)
        let tolerance_ns = 500_000u64;
        let within_tolerance = timing_errors.iter()
            .filter(|&&error| error <= tolerance_ns)
            .count();
        let frames_within_tolerance_pct = if !timing_errors.is_empty() {
            (within_tolerance as f32 / timing_errors.len() as f32) * 100.0
        } else {
            100.0
        };

        TimingAccuracyStats {
            avg_timing_error_ns,
            max_timing_error_ns,
            timing_error_std_ns,
            frames_within_tolerance_pct,
        }
    }

    /// Get memory usage statistics (simplified implementation)
    fn get_memory_stats(&self) -> MemoryStats {
        // In a real implementation, this would track actual memory usage
        // For now, estimate based on collected data
        let estimated_memory = (self.axis_outputs.len() + self.ffb_outputs.len()) * 4 + // f32 values
                              self.processing_times.len() * 16 + // Duration values
                              self.timestamps.len() * 8; // u64 values

        MemoryStats {
            peak_memory_bytes: estimated_memory as u64,
            avg_memory_bytes: estimated_memory as u64,
            allocation_count: self.frames_processed,
        }
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            total_duration: Duration::from_secs(0),
            avg_frame_time: Duration::from_secs(0),
            max_frame_time: Duration::from_secs(0),
            min_frame_time: Duration::from_secs(0),
            frames_per_second: 0.0,
            memory_stats: MemoryStats::default(),
        }
    }
}

impl Default for AccuracyMetrics {
    fn default() -> Self {
        Self {
            frames_analyzed: 0,
            axis_stats: OutputStats::default(),
            ffb_stats: OutputStats::default(),
            timing_stats: TimingAccuracyStats::default(),
        }
    }
}

impl Default for OutputStats {
    fn default() -> Self {
        Self {
            min_value: 0.0,
            max_value: 0.0,
            avg_value: 0.0,
            std_deviation: 0.0,
            range: 0.0,
        }
    }
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            peak_memory_bytes: 0,
            avg_memory_bytes: 0,
            allocation_count: 0,
        }
    }
}

impl Default for TimingAccuracyStats {
    fn default() -> Self {
        Self {
            avg_timing_error_ns: 0,
            max_timing_error_ns: 0,
            timing_error_std_ns: 0,
            frames_within_tolerance_pct: 100.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = ReplayMetrics::new(true);
        assert_eq!(metrics.get_frames_processed(), 0);
    }

    #[test]
    fn test_frame_recording() {
        let mut metrics = ReplayMetrics::new(true);
        metrics.start();
        
        metrics.record_frame_processed(1000000, 0.5, 2.0);
        metrics.record_frame_processed(2000000, 0.6, 2.1);
        
        assert_eq!(metrics.get_frames_processed(), 2);
        
        let accuracy = metrics.get_accuracy_metrics();
        assert_eq!(accuracy.frames_analyzed, 2);
        assert_eq!(accuracy.axis_stats.min_value, 0.5);
        assert_eq!(accuracy.axis_stats.max_value, 0.6);
    }

    #[test]
    fn test_performance_metrics() {
        let mut metrics = ReplayMetrics::new(true);
        metrics.start();
        
        // Record some frames
        for i in 0..100 {
            metrics.record_frame_processed(i * 4_000_000, 0.5, 2.0);
        }
        
        let performance = metrics.get_performance_metrics();
        assert_eq!(performance.frames_per_second > 0.0, true);
        assert!(performance.total_duration > Duration::from_secs(0));
    }

    #[test]
    fn test_output_statistics() {
        let mut metrics = ReplayMetrics::new(true);
        
        // Record frames with known values
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        for (i, &value) in values.iter().enumerate() {
            metrics.record_frame_processed(i as u64 * 4_000_000, value, value * 2.0);
        }
        
        let accuracy = metrics.get_accuracy_metrics();
        assert_eq!(accuracy.axis_stats.min_value, 1.0);
        assert_eq!(accuracy.axis_stats.max_value, 5.0);
        assert_eq!(accuracy.axis_stats.avg_value, 3.0);
        assert_eq!(accuracy.axis_stats.range, 4.0);
    }

    #[test]
    fn test_timing_statistics() {
        let mut metrics = ReplayMetrics::new(true);
        
        // Record frames with regular 4ms intervals
        for i in 0..10 {
            metrics.record_frame_processed(i * 4_000_000, 0.5, 2.0);
        }
        
        let accuracy = metrics.get_accuracy_metrics();
        assert_eq!(accuracy.timing_stats.avg_timing_error_ns, 0);
        assert_eq!(accuracy.timing_stats.frames_within_tolerance_pct, 100.0);
    }

    #[test]
    fn test_timing_with_jitter() {
        let mut metrics = ReplayMetrics::new(true);
        
        // Record frames with some jitter
        let timestamps = vec![0, 4_100_000, 8_050_000, 12_200_000]; // Some timing variation
        for (i, &timestamp) in timestamps.iter().enumerate() {
            metrics.record_frame_processed(timestamp, 0.5, 2.0);
        }
        
        let accuracy = metrics.get_accuracy_metrics();
        assert!(accuracy.timing_stats.avg_timing_error_ns > 0);
        assert!(accuracy.timing_stats.max_timing_error_ns > 0);
    }

    #[test]
    fn test_metrics_reset() {
        let mut metrics = ReplayMetrics::new(true);
        metrics.start();
        
        metrics.record_frame_processed(1000000, 0.5, 2.0);
        assert_eq!(metrics.get_frames_processed(), 1);
        
        metrics.reset();
        assert_eq!(metrics.get_frames_processed(), 0);
        
        let performance = metrics.get_performance_metrics();
        assert_eq!(performance.total_duration, Duration::from_secs(0));
    }

    #[test]
    fn test_disabled_metrics() {
        let mut metrics = ReplayMetrics::new(false);
        
        metrics.record_frame_processed(1000000, 0.5, 2.0);
        assert_eq!(metrics.get_frames_processed(), 0);
        
        let performance = metrics.get_performance_metrics();
        assert_eq!(performance.frames_per_second, 0.0);
    }
}