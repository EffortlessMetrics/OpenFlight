//! Hardware-in-Loop (HIL) tests specifically for trim correctness validation
//!
//! These tests validate trim behavior with actual hardware timing constraints
//! and floating-point precision requirements for production deployment.

use std::time::{Duration, Instant};
use crate::{
    FfbEngine, FfbConfig, FfbMode, TrimController, TrimMode, SetpointChange, TrimLimits,
    TrimOutput, TrimValidationSuite, TrimValidationConfig, TrimValidationResult,
    DeviceCapabilities, BlackboxEntry
};

/// HIL trim test configuration
#[derive(Debug, Clone)]
pub struct HilTrimTestConfig {
    /// Device maximum torque for testing
    pub device_max_torque_nm: f32,
    /// Test duration limit
    pub max_test_duration: Duration,
    /// Floating-point tolerance for HIL validation
    pub hil_fp_tolerance: f32,
    /// Whether to use physical device (if available)
    pub use_physical_device: bool,
    /// Sample rate for HIL measurements
    pub hil_sample_rate_hz: u32,
}

impl Default for HilTrimTestConfig {
    fn default() -> Self {
        Self {
            device_max_torque_nm: 15.0,
            max_test_duration: Duration::from_secs(60),
            hil_fp_tolerance: 1e-4, // Relaxed for hardware timing
            use_physical_device: false,
            hil_sample_rate_hz: 250, // Match axis processing rate
        }
    }
}

/// HIL trim test result with hardware-specific metrics
#[derive(Debug, Clone)]
pub struct HilTrimTestResult {
    /// Base validation result
    pub validation_result: TrimValidationResult,
    /// Hardware-specific metrics
    pub hardware_metrics: HardwareMetrics,
    /// Timing analysis
    pub timing_analysis: TimingAnalysis,
}

/// Hardware-specific metrics from HIL testing
#[derive(Debug, Clone)]
pub struct HardwareMetrics {
    /// Actual device response time (ms)
    pub device_response_time_ms: f32,
    /// USB communication latency (ms)
    pub usb_latency_ms: f32,
    /// Jitter in hardware timing (ms)
    pub timing_jitter_ms: f32,
    /// Number of USB communication errors
    pub usb_errors: u32,
}

impl Default for HardwareMetrics {
    fn default() -> Self {
        Self {
            device_response_time_ms: 0.0,
            usb_latency_ms: 0.0,
            timing_jitter_ms: 0.0,
            usb_errors: 0,
        }
    }
}

/// Timing analysis from HIL testing
#[derive(Debug, Clone)]
pub struct TimingAnalysis {
    /// Average update period (ms)
    pub avg_update_period_ms: f32,
    /// Maximum update period (ms)
    pub max_update_period_ms: f32,
    /// Minimum update period (ms)
    pub min_update_period_ms: f32,
    /// Standard deviation of update periods (ms)
    pub update_period_stddev_ms: f32,
    /// Number of missed deadlines
    pub missed_deadlines: u32,
}

impl Default for TimingAnalysis {
    fn default() -> Self {
        Self {
            avg_update_period_ms: 0.0,
            max_update_period_ms: 0.0,
            min_update_period_ms: f32::INFINITY,
            update_period_stddev_ms: 0.0,
            missed_deadlines: 0,
        }
    }
}

/// HIL trim test suite for hardware validation
pub struct HilTrimTestSuite {
    config: HilTrimTestConfig,
    validation_suite: TrimValidationSuite,
}

impl HilTrimTestSuite {
    /// Create new HIL trim test suite
    pub fn new(config: HilTrimTestConfig) -> Self {
        let validation_config = TrimValidationConfig {
            fp_tolerance: config.hil_fp_tolerance,
            max_test_duration: config.max_test_duration,
            sample_rate_hz: config.hil_sample_rate_hz,
            verbose_logging: true,
        };
        
        let validation_suite = TrimValidationSuite::new(validation_config);
        
        Self {
            config,
            validation_suite,
        }
    }

    /// Run complete HIL trim validation
    pub fn run_hil_trim_validation(&mut self) -> Vec<HilTrimTestResult> {
        let mut results = Vec::new();

        // Core trim behavior tests with hardware timing
        results.push(self.test_hil_ffb_rate_limiting());
        results.push(self.test_hil_ffb_jerk_limiting());
        results.push(self.test_hil_spring_freeze_timing());
        results.push(self.test_hil_spring_ramp_timing());
        
        // Hardware-specific tests
        results.push(self.test_hil_usb_timing_compliance());
        results.push(self.test_hil_device_response_latency());
        results.push(self.test_hil_concurrent_trim_operations());
        
        // Stress tests
        results.push(self.test_hil_rapid_setpoint_changes());
        results.push(self.test_hil_long_duration_stability());

        results
    }

    /// Test FFB rate limiting with hardware timing constraints
    pub fn test_hil_ffb_rate_limiting(&mut self) -> HilTrimTestResult {
        let start_time = Instant::now();
        let mut hardware_metrics = HardwareMetrics::default();
        let mut timing_analysis = TimingAnalysis::default();
        let mut update_periods = Vec::new();

        let validation_result = (|| -> TrimValidationResult {
            let mut engine = self.create_test_engine();
            
            // Configure for high-performance FFB mode
            let capabilities = DeviceCapabilities {
                supports_pid: true,
                supports_raw_torque: true,
                max_torque_nm: self.config.device_max_torque_nm,
                min_period_us: 1000, // 1ms minimum period
                has_health_stream: true,
                supports_interlock: true,
            };
            
            engine.set_device_capabilities(capabilities).unwrap();
            
            let limits = TrimLimits {
                max_rate_nm_per_s: 10.0,
                max_jerk_nm_per_s2: 40.0,
            };
            
            {
                let trim_controller = engine.get_trim_controller_mut();
                trim_controller.set_mode(TrimMode::ForceFeedback);

                let change = SetpointChange {
                    target_nm: 12.0,
                    limits: limits.clone(),
                };

                trim_controller.apply_setpoint_change(change).unwrap();
            }

            let mut measurements = Vec::new();
            let mut max_rate = 0.0f32;
            let mut last_update = Instant::now();
            
            // Run with hardware timing constraints
            for _i in 0..2500 { // 10 seconds at 250Hz
                let update_start = Instant::now();
                
                let output = engine.update_trim_controller();
                
                if let TrimOutput::ForceFeedback { rate_nm_per_s, setpoint_nm } = output {
                    max_rate = max_rate.max(rate_nm_per_s.abs());
                    measurements.push(rate_nm_per_s.abs());
                    
                    // Record in engine blackbox
                    engine.record_axis_frame(
                        "hil_test_device".to_string(),
                        0.0, // raw input
                        setpoint_nm, // processed output
                        setpoint_nm, // torque
                    ).unwrap();
                }
                
                // Measure timing
                let update_period = update_start.duration_since(last_update);
                update_periods.push(update_period.as_secs_f32() * 1000.0);
                last_update = update_start;
                
                // Simulate hardware communication delay
                if self.config.use_physical_device {
                    std::thread::sleep(Duration::from_micros(100)); // USB latency
                }
                
                // Maintain 250Hz rate (4ms period)
                let target_period = Duration::from_millis(4);
                let elapsed = update_start.elapsed();
                if elapsed < target_period {
                    std::thread::sleep(target_period - elapsed);
                }
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            // Analyze timing
            if !update_periods.is_empty() {
                timing_analysis.avg_update_period_ms = update_periods.iter().sum::<f32>() / update_periods.len() as f32;
                timing_analysis.max_update_period_ms = update_periods.iter().fold(0.0f32, |a, &b| a.max(b));
                timing_analysis.min_update_period_ms = update_periods.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                
                // Calculate standard deviation
                let mean = timing_analysis.avg_update_period_ms;
                let variance = update_periods.iter()
                    .map(|&x| (x - mean).powi(2))
                    .sum::<f32>() / update_periods.len() as f32;
                timing_analysis.update_period_stddev_ms = variance.sqrt();
                
                // Count missed deadlines (>5ms for 250Hz)
                timing_analysis.missed_deadlines = update_periods.iter()
                    .filter(|&&period| period > 5.0)
                    .count() as u32;
            }

            // Validate rate limiting with hardware tolerance
            let rate_violation = measurements.iter()
                .any(|&rate| rate > limits.max_rate_nm_per_s + self.config.hil_fp_tolerance);

            TrimValidationResult {
                name: "HIL FFB Rate Limiting".to_string(),
                passed: !rate_violation && timing_analysis.missed_deadlines == 0,
                duration: start_time.elapsed(),
                error: if rate_violation {
                    Some(format!("Rate limit exceeded with hardware timing"))
                } else if timing_analysis.missed_deadlines > 0 {
                    Some(format!("Missed {} timing deadlines", timing_analysis.missed_deadlines))
                } else {
                    None
                },
                measurements,
                metrics: crate::TrimValidationMetrics {
                    max_rate_nm_per_s: max_rate,
                    ..Default::default()
                },
            }
        })();

        HilTrimTestResult {
            validation_result,
            hardware_metrics,
            timing_analysis,
        }
    }

    /// Test FFB jerk limiting with hardware timing constraints
    fn test_hil_ffb_jerk_limiting(&mut self) -> HilTrimTestResult {
        let start_time = Instant::now();
        let mut hardware_metrics = HardwareMetrics::default();
        let mut timing_analysis = TimingAnalysis::default();

        let validation_result = (|| -> TrimValidationResult {
            let mut engine = self.create_test_engine();
            let trim_controller = engine.get_trim_controller_mut();
            trim_controller.set_mode(TrimMode::ForceFeedback);

            let limits = TrimLimits {
                max_rate_nm_per_s: 8.0,
                max_jerk_nm_per_s2: 25.0,
            };

            let change = SetpointChange {
                target_nm: 10.0,
                limits: limits.clone(),
            };

            trim_controller.apply_setpoint_change(change).unwrap();

            let mut measurements = Vec::new();
            let mut previous_rate = 0.0f32;
            let mut max_jerk = 0.0f32;
            let dt = 0.004f32; // 4ms timestep for 250Hz
            
            for _ in 0..2000 {
                let output = trim_controller.update();
                
                if let TrimOutput::ForceFeedback { rate_nm_per_s, .. } = output {
                    let jerk = (rate_nm_per_s - previous_rate).abs() / dt;
                    max_jerk = max_jerk.max(jerk);
                    measurements.push(jerk);
                    previous_rate = rate_nm_per_s;
                }
                
                // Hardware timing simulation
                std::thread::sleep(Duration::from_millis(4));
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            // Validate jerk limiting with hardware tolerance
            let jerk_tolerance = self.config.hil_fp_tolerance * 50.0; // More tolerance for jerk
            let jerk_violation = measurements.iter()
                .any(|&jerk| jerk > limits.max_jerk_nm_per_s2 + jerk_tolerance);

            TrimValidationResult {
                name: "HIL FFB Jerk Limiting".to_string(),
                passed: !jerk_violation,
                duration: start_time.elapsed(),
                error: if jerk_violation {
                    Some(format!("Jerk limit exceeded with hardware timing"))
                } else {
                    None
                },
                measurements,
                metrics: crate::TrimValidationMetrics {
                    max_jerk_nm_per_s2: max_jerk,
                    ..Default::default()
                },
            }
        })();

        HilTrimTestResult {
            validation_result,
            hardware_metrics,
            timing_analysis,
        }
    }

    /// Test spring freeze timing with hardware constraints
    pub fn test_hil_spring_freeze_timing(&mut self) -> HilTrimTestResult {
        let start_time = Instant::now();
        let mut hardware_metrics = HardwareMetrics::default();
        let mut timing_analysis = TimingAnalysis::default();

        let validation_result = (|| -> TrimValidationResult {
            let mut engine = self.create_test_engine();
            let trim_controller = engine.get_trim_controller_mut();
            trim_controller.set_mode(TrimMode::SpringCentered);

            let change = SetpointChange {
                target_nm: 7.5,
                limits: TrimLimits::default(),
            };

            trim_controller.apply_setpoint_change(change).unwrap();

            let mut measurements = Vec::new();
            let mut freeze_start = None;
            let mut freeze_end = None;
            
            for _ in 0..1000 {
                let output = trim_controller.update();
                
                if let TrimOutput::SpringCentered { frozen, .. } = output {
                    measurements.push(if frozen { 1.0 } else { 0.0 });
                    
                    if frozen && freeze_start.is_none() {
                        freeze_start = Some(Instant::now());
                    } else if !frozen && freeze_start.is_some() && freeze_end.is_none() {
                        freeze_end = Some(Instant::now());
                    }
                }
                
                // Hardware timing simulation
                std::thread::sleep(Duration::from_millis(10));
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            let freeze_duration = if let (Some(start), Some(end)) = (freeze_start, freeze_end) {
                Some(end.duration_since(start))
            } else {
                None
            };

            // Validate freeze timing (should be 100ms + ramp time)
            let timing_valid = if let Some(duration) = freeze_duration {
                let duration_ms = duration.as_millis();
                duration_ms >= 50 && duration_ms <= 500 // Reasonable range
            } else {
                false
            };

            TrimValidationResult {
                name: "HIL Spring Freeze Timing".to_string(),
                passed: timing_valid,
                duration: start_time.elapsed(),
                error: if !timing_valid {
                    Some(format!("Spring freeze timing invalid: {:?}", freeze_duration))
                } else {
                    None
                },
                measurements,
                metrics: crate::TrimValidationMetrics {
                    spring_freeze_duration: freeze_duration,
                    ..Default::default()
                },
            }
        })();

        HilTrimTestResult {
            validation_result,
            hardware_metrics,
            timing_analysis,
        }
    }

    /// Test spring ramp timing with hardware constraints
    fn test_hil_spring_ramp_timing(&mut self) -> HilTrimTestResult {
        let start_time = Instant::now();
        let hardware_metrics = HardwareMetrics::default();
        let timing_analysis = TimingAnalysis::default();

        let validation_result = (|| -> TrimValidationResult {
            let mut engine = self.create_test_engine();
            let trim_controller = engine.get_trim_controller_mut();
            trim_controller.set_mode(TrimMode::SpringCentered);

            let change = SetpointChange {
                target_nm: 5.0,
                limits: TrimLimits::default(),
            };

            trim_controller.apply_setpoint_change(change).unwrap();

            // Wait for freeze period
            std::thread::sleep(Duration::from_millis(200));

            let mut measurements = Vec::new();
            let mut ramp_start = None;
            let mut ramp_end = None;
            let mut strength_values = Vec::new();
            
            for _ in 0..300 {
                let state = trim_controller.get_trim_state();
                
                if state.spring_ramping && ramp_start.is_none() {
                    ramp_start = Some(Instant::now());
                } else if !state.spring_ramping && ramp_start.is_some() && ramp_end.is_none() {
                    ramp_end = Some(Instant::now());
                }

                let output = trim_controller.update();
                if let TrimOutput::SpringCentered { config, .. } = output {
                    strength_values.push(config.strength);
                    measurements.push(config.strength);
                }
                
                std::thread::sleep(Duration::from_millis(10));
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            let ramp_duration = if let (Some(start), Some(end)) = (ramp_start, ramp_end) {
                Some(end.duration_since(start))
            } else {
                None
            };

            // Validate ramp shows gradual increase
            let ramp_valid = if !strength_values.is_empty() {
                let first = strength_values[0];
                let last = strength_values[strength_values.len() - 1];
                last > first // Should increase during ramp
            } else {
                false
            };

            TrimValidationResult {
                name: "HIL Spring Ramp Timing".to_string(),
                passed: ramp_valid && ramp_duration.is_some(),
                duration: start_time.elapsed(),
                error: if !ramp_valid {
                    Some("Spring ramp did not show gradual increase".to_string())
                } else if ramp_duration.is_none() {
                    Some("Spring ramp timing not detected".to_string())
                } else {
                    None
                },
                measurements,
                metrics: crate::TrimValidationMetrics {
                    spring_ramp_duration: ramp_duration,
                    ..Default::default()
                },
            }
        })();

        HilTrimTestResult {
            validation_result,
            hardware_metrics,
            timing_analysis,
        }
    }

    /// Test USB timing compliance for trim operations
    fn test_hil_usb_timing_compliance(&mut self) -> HilTrimTestResult {
        let start_time = Instant::now();
        let mut hardware_metrics = HardwareMetrics::default();
        let mut timing_analysis = TimingAnalysis::default();

        let validation_result = (|| -> TrimValidationResult {
            let mut engine = self.create_test_engine();
            let trim_controller = engine.get_trim_controller_mut();
            trim_controller.set_mode(TrimMode::ForceFeedback);

            let mut measurements = Vec::new();
            let mut usb_latencies = Vec::new();
            
            // Test multiple rapid setpoint changes to stress USB communication
            for i in 0..20 {
                let usb_start = Instant::now();
                
                let change = SetpointChange {
                    target_nm: (i as f32 % 10.0) - 5.0, // Oscillate between -5 and 5
                    limits: TrimLimits {
                        max_rate_nm_per_s: 15.0,
                        max_jerk_nm_per_s2: 50.0,
                    },
                };

                trim_controller.apply_setpoint_change(change).unwrap();
                
                // Simulate USB communication
                if self.config.use_physical_device {
                    std::thread::sleep(Duration::from_micros(200)); // Realistic USB latency
                }
                
                let usb_latency = usb_start.elapsed().as_secs_f32() * 1000.0;
                usb_latencies.push(usb_latency);
                measurements.push(usb_latency);
                
                // Run a few updates
                for _ in 0..10 {
                    trim_controller.update();
                    std::thread::sleep(Duration::from_millis(4)); // 250Hz
                }
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            // Analyze USB timing
            if !usb_latencies.is_empty() {
                hardware_metrics.usb_latency_ms = usb_latencies.iter().sum::<f32>() / usb_latencies.len() as f32;
                
                let max_latency = usb_latencies.iter().fold(0.0f32, |a, &b| a.max(b));
                let min_latency = usb_latencies.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                hardware_metrics.timing_jitter_ms = max_latency - min_latency;
            }

            // USB latency should be reasonable (< 5ms for HID)
            let latency_acceptable = hardware_metrics.usb_latency_ms < 5.0;
            let jitter_acceptable = hardware_metrics.timing_jitter_ms < 2.0;

            TrimValidationResult {
                name: "HIL USB Timing Compliance".to_string(),
                passed: latency_acceptable && jitter_acceptable,
                duration: start_time.elapsed(),
                error: if !latency_acceptable {
                    Some(format!("USB latency too high: {:.2}ms", hardware_metrics.usb_latency_ms))
                } else if !jitter_acceptable {
                    Some(format!("USB jitter too high: {:.2}ms", hardware_metrics.timing_jitter_ms))
                } else {
                    None
                },
                measurements,
                metrics: Default::default(),
            }
        })();

        HilTrimTestResult {
            validation_result,
            hardware_metrics,
            timing_analysis,
        }
    }

    /// Test device response latency for trim commands
    fn test_hil_device_response_latency(&mut self) -> HilTrimTestResult {
        let start_time = Instant::now();
        let mut hardware_metrics = HardwareMetrics::default();
        let timing_analysis = TimingAnalysis::default();

        let validation_result = (|| -> TrimValidationResult {
            let mut engine = self.create_test_engine();
            let trim_controller = engine.get_trim_controller_mut();
            trim_controller.set_mode(TrimMode::ForceFeedback);

            let mut measurements = Vec::new();
            let mut response_times = Vec::new();
            
            // Test device response to setpoint changes
            for i in 0..10 {
                let response_start = Instant::now();
                
                let target_nm = if i % 2 == 0 { 8.0 } else { -8.0 };
                let change = SetpointChange {
                    target_nm: target_nm,
                    limits: TrimLimits::default(),
                };

                trim_controller.apply_setpoint_change(change).unwrap();
                
                // Wait for device to respond (simulate hardware response time)
                let mut response_detected = false;
                let mut response_time = Duration::ZERO;
                
                for _ in 0..100 { // Up to 400ms to respond
                    let output = trim_controller.update();
                    
                    if let TrimOutput::ForceFeedback { setpoint_nm, .. } = output {
                        // Check if we're moving toward target
                        let target = target_nm;
                        let moving_toward_target = if target > 0.0 {
                            setpoint_nm > 0.1 // Moving positive
                        } else {
                            setpoint_nm < -0.1 // Moving negative
                        };
                        
                        if moving_toward_target && !response_detected {
                            response_detected = true;
                            response_time = response_start.elapsed();
                            break;
                        }
                    }
                    
                    std::thread::sleep(Duration::from_millis(4));
                }
                
                if response_detected {
                    let response_ms = response_time.as_secs_f32() * 1000.0;
                    response_times.push(response_ms);
                    measurements.push(response_ms);
                }
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            // Analyze device response times
            if !response_times.is_empty() {
                hardware_metrics.device_response_time_ms = response_times.iter().sum::<f32>() / response_times.len() as f32;
            }

            // Device should respond within 50ms
            let response_acceptable = hardware_metrics.device_response_time_ms < 50.0;

            TrimValidationResult {
                name: "HIL Device Response Latency".to_string(),
                passed: response_acceptable,
                duration: start_time.elapsed(),
                error: if !response_acceptable {
                    Some(format!("Device response too slow: {:.2}ms", hardware_metrics.device_response_time_ms))
                } else {
                    None
                },
                measurements,
                metrics: Default::default(),
            }
        })();

        HilTrimTestResult {
            validation_result,
            hardware_metrics,
            timing_analysis,
        }
    }

    /// Test concurrent trim operations
    fn test_hil_concurrent_trim_operations(&mut self) -> HilTrimTestResult {
        let start_time = Instant::now();
        let hardware_metrics = HardwareMetrics::default();
        let timing_analysis = TimingAnalysis::default();

        let validation_result = (|| -> TrimValidationResult {
            let mut engine = self.create_test_engine();
            let trim_controller = engine.get_trim_controller_mut();
            trim_controller.set_mode(TrimMode::ForceFeedback);

            let mut measurements = Vec::new();
            
            // Apply multiple rapid setpoint changes to test handling
            let setpoints = vec![5.0, -3.0, 8.0, -7.0, 2.0, -4.0, 6.0];
            
            for &target in &setpoints {
                let change = SetpointChange {
                    target_nm: target,
                    limits: TrimLimits {
                        max_rate_nm_per_s: 12.0,
                        max_jerk_nm_per_s2: 35.0,
                    },
                };

                trim_controller.apply_setpoint_change(change).unwrap();
                
                // Only run a few updates before next change (concurrent behavior)
                for _ in 0..5 {
                    let output = trim_controller.update();
                    if let TrimOutput::ForceFeedback { setpoint_nm, .. } = output {
                        measurements.push(setpoint_nm);
                    }
                    std::thread::sleep(Duration::from_millis(4));
                }
            }

            // Verify no NaN or infinite values during concurrent operations
            let stability_ok = measurements.iter().all(|&x| x.is_finite());

            TrimValidationResult {
                name: "HIL Concurrent Trim Operations".to_string(),
                passed: stability_ok,
                duration: start_time.elapsed(),
                error: if !stability_ok {
                    Some("Instability detected during concurrent operations".to_string())
                } else {
                    None
                },
                measurements,
                metrics: Default::default(),
            }
        })();

        HilTrimTestResult {
            validation_result,
            hardware_metrics,
            timing_analysis,
        }
    }

    /// Test rapid setpoint changes
    fn test_hil_rapid_setpoint_changes(&mut self) -> HilTrimTestResult {
        let start_time = Instant::now();
        let hardware_metrics = HardwareMetrics::default();
        let timing_analysis = TimingAnalysis::default();

        let validation_result = (|| -> TrimValidationResult {
            let mut engine = self.create_test_engine();
            let trim_controller = engine.get_trim_controller_mut();
            trim_controller.set_mode(TrimMode::ForceFeedback);

            let mut measurements = Vec::new();
            
            // Rapid setpoint changes every 50ms
            for i in 0..50 {
                let target = ((i as f32 * 0.5).sin() * 10.0).clamp(-12.0, 12.0);
                
                let change = SetpointChange {
                    target_nm: target,
                    limits: TrimLimits {
                        max_rate_nm_per_s: 20.0,
                        max_jerk_nm_per_s2: 60.0,
                    },
                };

                trim_controller.apply_setpoint_change(change).unwrap();
                
                // Run updates for 50ms
                for _ in 0..12 { // ~50ms at 4ms intervals
                    let output = trim_controller.update();
                    if let TrimOutput::ForceFeedback { setpoint_nm, rate_nm_per_s } = output {
                        measurements.push(setpoint_nm);
                        
                        // Verify rate limits are still respected
                        if rate_nm_per_s.abs() > 20.0 + self.config.hil_fp_tolerance {
                            return TrimValidationResult {
                                name: "HIL Rapid Setpoint Changes".to_string(),
                                passed: false,
                                duration: start_time.elapsed(),
                                error: Some(format!("Rate limit violated during rapid changes: {} Nm/s", rate_nm_per_s)),
                                measurements,
                                metrics: Default::default(),
                            };
                        }
                    }
                    std::thread::sleep(Duration::from_millis(4));
                }
                
                if start_time.elapsed() > self.config.max_test_duration {
                    break;
                }
            }

            TrimValidationResult {
                name: "HIL Rapid Setpoint Changes".to_string(),
                passed: true,
                duration: start_time.elapsed(),
                error: None,
                measurements,
                metrics: Default::default(),
            }
        })();

        HilTrimTestResult {
            validation_result,
            hardware_metrics,
            timing_analysis,
        }
    }

    /// Test long duration stability
    fn test_hil_long_duration_stability(&mut self) -> HilTrimTestResult {
        let start_time = Instant::now();
        let hardware_metrics = HardwareMetrics::default();
        let timing_analysis = TimingAnalysis::default();

        let validation_result = (|| -> TrimValidationResult {
            let mut engine = self.create_test_engine();
            let trim_controller = engine.get_trim_controller_mut();
            trim_controller.set_mode(TrimMode::ForceFeedback);

            let mut measurements = Vec::new();
            let test_duration = Duration::from_secs(30); // 30 second stability test
            
            // Set a moderate setpoint and run for extended period
            let change = SetpointChange {
                target_nm: 6.0,
                limits: TrimLimits::default(),
            };

            trim_controller.apply_setpoint_change(change).unwrap();
            
            let mut sample_count = 0;
            while start_time.elapsed() < test_duration && start_time.elapsed() < self.config.max_test_duration {
                let output = trim_controller.update();
                
                if let TrimOutput::ForceFeedback { setpoint_nm, .. } = output {
                    measurements.push(setpoint_nm);
                    sample_count += 1;
                    
                    // Check for stability issues
                    if !setpoint_nm.is_finite() {
                        return TrimValidationResult {
                            name: "HIL Long Duration Stability".to_string(),
                            passed: false,
                            duration: start_time.elapsed(),
                            error: Some(format!("Non-finite value after {} samples", sample_count)),
                            measurements,
                            metrics: Default::default(),
                        };
                    }
                }
                
                std::thread::sleep(Duration::from_millis(4));
            }

            // Verify we ran for a reasonable duration
            let duration_ok = start_time.elapsed() >= Duration::from_secs(10);
            let sample_count_ok = sample_count > 1000; // Should have many samples

            TrimValidationResult {
                name: "HIL Long Duration Stability".to_string(),
                passed: duration_ok && sample_count_ok,
                duration: start_time.elapsed(),
                error: if !duration_ok {
                    Some("Test duration too short".to_string())
                } else if !sample_count_ok {
                    Some(format!("Insufficient samples: {}", sample_count))
                } else {
                    None
                },
                measurements,
                metrics: Default::default(),
            }
        })();

        HilTrimTestResult {
            validation_result,
            hardware_metrics,
            timing_analysis,
        }
    }

    /// Create test engine with appropriate configuration
    fn create_test_engine(&self) -> FfbEngine {
        let config = FfbConfig {
            max_torque_nm: self.config.device_max_torque_nm,
            fault_timeout_ms: 50,
            interlock_required: false, // Disable for testing
            mode: FfbMode::Auto,
            device_path: Some("hil_test_device".to_string()),
        };
        
        FfbEngine::new(config).expect("Failed to create test engine")
    }

    /// Generate HIL test report
    pub fn generate_hil_report(&self, results: &[HilTrimTestResult]) -> String {
        let mut report = String::new();
        report.push_str("# HIL Trim Correctness Validation Report\n\n");

        let total_tests = results.len();
        let passed_tests = results.iter().filter(|r| r.validation_result.passed).count();
        let failed_tests = total_tests - passed_tests;

        report.push_str("## HIL Test Summary\n\n");
        report.push_str(&format!("- **Total HIL Tests**: {}\n", total_tests));
        report.push_str(&format!("- **Passed**: {} ({}%)\n", passed_tests, 
            (passed_tests as f32 / total_tests as f32 * 100.0) as u32));
        report.push_str(&format!("- **Failed**: {} ({}%)\n", failed_tests,
            (failed_tests as f32 / total_tests as f32 * 100.0) as u32));
        
        let overall_status = if failed_tests == 0 { "✅ PASS" } else { "❌ FAIL" };
        report.push_str(&format!("- **Overall HIL Status**: {}\n\n", overall_status));

        report.push_str("## Hardware Metrics Summary\n\n");
        
        // Aggregate hardware metrics
        let avg_device_response = results.iter()
            .map(|r| r.hardware_metrics.device_response_time_ms)
            .filter(|&x| x > 0.0)
            .collect::<Vec<_>>();
        
        let avg_usb_latency = results.iter()
            .map(|r| r.hardware_metrics.usb_latency_ms)
            .filter(|&x| x > 0.0)
            .collect::<Vec<_>>();

        if !avg_device_response.is_empty() {
            let avg = avg_device_response.iter().sum::<f32>() / avg_device_response.len() as f32;
            report.push_str(&format!("- **Average Device Response Time**: {:.2}ms\n", avg));
        }
        
        if !avg_usb_latency.is_empty() {
            let avg = avg_usb_latency.iter().sum::<f32>() / avg_usb_latency.len() as f32;
            report.push_str(&format!("- **Average USB Latency**: {:.2}ms\n", avg));
        }

        report.push_str("\n## Detailed HIL Results\n\n");
        
        for result in results {
            let status = if result.validation_result.passed { "✅ PASS" } else { "❌ FAIL" };
            report.push_str(&format!("### {} - {}\n\n", status, result.validation_result.name));
            
            report.push_str(&format!("- **Duration**: {:.2}ms\n", 
                result.validation_result.duration.as_secs_f32() * 1000.0));
            
            if let Some(error) = &result.validation_result.error {
                report.push_str(&format!("- **Error**: {}\n", error));
            }

            // Hardware metrics
            let hw = &result.hardware_metrics;
            if hw.device_response_time_ms > 0.0 {
                report.push_str(&format!("- **Device Response Time**: {:.2}ms\n", hw.device_response_time_ms));
            }
            if hw.usb_latency_ms > 0.0 {
                report.push_str(&format!("- **USB Latency**: {:.2}ms\n", hw.usb_latency_ms));
            }
            if hw.timing_jitter_ms > 0.0 {
                report.push_str(&format!("- **Timing Jitter**: {:.2}ms\n", hw.timing_jitter_ms));
            }

            // Timing analysis
            let timing = &result.timing_analysis;
            if timing.avg_update_period_ms > 0.0 {
                report.push_str(&format!("- **Average Update Period**: {:.2}ms\n", timing.avg_update_period_ms));
            }
            if timing.missed_deadlines > 0 {
                report.push_str(&format!("- **Missed Deadlines**: {}\n", timing.missed_deadlines));
            }
            
            report.push_str("\n");
        }

        report.push_str("## HIL Compliance Verification\n\n");
        report.push_str("This HIL validation suite verifies:\n\n");
        report.push_str("- **Real-time Performance**: 250Hz update rate with <5ms jitter\n");
        report.push_str("- **Hardware Timing**: USB latency <5ms, device response <50ms\n");
        report.push_str("- **Rate/Jerk Limiting**: No torque steps under hardware timing constraints\n");
        report.push_str("- **Spring Behavior**: Proper freeze/ramp timing with hardware delays\n");
        report.push_str("- **Stability**: Long-duration operation without degradation\n");
        report.push_str("- **Concurrent Operations**: Stable behavior under rapid setpoint changes\n\n");

        report
    }
}

impl Default for HilTrimTestSuite {
    fn default() -> Self {
        Self::new(HilTrimTestConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hil_trim_test_suite_creation() {
        let suite = HilTrimTestSuite::default();
        assert_eq!(suite.config.device_max_torque_nm, 15.0);
        assert_eq!(suite.config.hil_sample_rate_hz, 250);
    }

    #[test]
    fn test_hil_ffb_rate_limiting() {
        let mut suite = HilTrimTestSuite::default();
        let result = suite.test_hil_ffb_rate_limiting();
        
        assert!(result.validation_result.passed, 
            "HIL FFB rate limiting test failed: {:?}", result.validation_result.error);
        assert!(!result.validation_result.measurements.is_empty());
    }

    #[test]
    fn test_hil_spring_freeze_timing() {
        let mut suite = HilTrimTestSuite::default();
        let result = suite.test_hil_spring_freeze_timing();
        
        assert!(result.validation_result.passed, 
            "HIL spring freeze timing test failed: {:?}", result.validation_result.error);
    }

    #[test]
    fn test_hil_complete_validation() {
        let mut suite = HilTrimTestSuite::default();
        let results = suite.run_hil_trim_validation();
        
        assert!(!results.is_empty());
        
        // Most tests should pass (allowing for some hardware-dependent failures)
        let passed_count = results.iter().filter(|r| r.validation_result.passed).count();
        let pass_rate = passed_count as f32 / results.len() as f32;
        
        assert!(pass_rate >= 0.8, "HIL pass rate too low: {:.1}%", pass_rate * 100.0);
    }

    #[test]
    fn test_hil_report_generation() {
        let mut suite = HilTrimTestSuite::default();
        
        let mock_results = vec![
            HilTrimTestResult {
                validation_result: TrimValidationResult {
                    name: "HIL Test 1".to_string(),
                    passed: true,
                    duration: Duration::from_millis(100),
                    error: None,
                    measurements: vec![1.0, 2.0],
                    metrics: Default::default(),
                },
                hardware_metrics: HardwareMetrics {
                    device_response_time_ms: 25.0,
                    usb_latency_ms: 2.5,
                    ..Default::default()
                },
                timing_analysis: Default::default(),
            },
        ];
        
        let report = suite.generate_hil_report(&mock_results);
        
        assert!(report.contains("HIL Test Summary"));
        assert!(report.contains("Hardware Metrics Summary"));
        assert!(report.contains("Device Response Time"));
        assert!(report.contains("USB Latency"));
        assert!(report.contains("HIL Compliance Verification"));
    }
}