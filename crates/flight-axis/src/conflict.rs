//! Curve conflict detection and analysis
//!
//! Detects when both simulator and profile curves are active on the same axis,
//! creating "double-curve" situations that can lead to poor control response.

use crate::{AxisFrame, Node};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, warn, info};

/// Configuration for curve conflict detection
#[derive(Debug, Clone)]
pub struct ConflictDetectorConfig {
    /// Number of test points to use for linearity analysis
    pub test_points: usize,
    /// Minimum non-linearity threshold to trigger detection (0.0-1.0)
    pub nonlinearity_threshold: f32,
    /// Time window for collecting samples (milliseconds)
    pub sample_window_ms: u64,
    /// Minimum samples needed for reliable detection
    pub min_samples: usize,
    /// Enable continuous monitoring vs one-shot detection
    pub continuous_monitoring: bool,
}

impl Default for ConflictDetectorConfig {
    fn default() -> Self {
        Self {
            test_points: 11, // 0.0, 0.1, 0.2, ... 1.0
            nonlinearity_threshold: 0.15, // 15% deviation from linear
            sample_window_ms: 5000, // 5 seconds
            min_samples: 50, // At least 50 samples
            continuous_monitoring: true,
        }
    }
}

/// Detected curve conflict information
#[derive(Debug, Clone)]
pub struct CurveConflict {
    pub axis_name: String,
    pub conflict_type: ConflictType,
    pub severity: ConflictSeverity,
    pub description: String,
    pub metadata: ConflictMetadata,
    pub suggested_resolutions: Vec<ConflictResolution>,
    pub detected_at: Instant,
}

/// Type of curve conflict detected
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictType {
    DoubleCurve,
    ExcessiveNonlinearity,
    OpposingCurves,
}

/// Severity of the detected conflict
#[derive(Debug, Clone, PartialEq, Ord, PartialOrd, Eq)]
pub enum ConflictSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Metadata about the detected conflict
#[derive(Debug, Clone)]
pub struct ConflictMetadata {
    pub sim_curve_strength: f32,
    pub profile_curve_strength: f32,
    pub combined_nonlinearity: f32,
    pub test_inputs: Vec<f32>,
    pub expected_outputs: Vec<f32>,
    pub actual_outputs: Vec<f32>,
    pub detection_timestamp: Instant,
}

/// Suggested resolution for a conflict
#[derive(Debug, Clone)]
pub struct ConflictResolution {
    pub resolution_type: ResolutionType,
    pub description: String,
    pub estimated_improvement: f32,
    pub requires_sim_restart: bool,
    pub parameters: HashMap<String, String>,
}

/// Type of resolution that can be applied
#[derive(Debug, Clone, PartialEq)]
pub enum ResolutionType {
    DisableSimCurve,
    DisableProfileCurve,
    ApplyGainCompensation,
    ReduceCurveStrength,
}

/// Sample data point for analysis
#[derive(Debug, Clone)]
struct SamplePoint {
    input: f32,
    output: f32,
    timestamp: Instant,
}

/// Curve conflict detector with real-time analysis
pub struct CurveConflictDetector {
    config: ConflictDetectorConfig,
    sample_buffer: HashMap<String, Vec<SamplePoint>>,
    detected_conflicts: HashMap<String, CurveConflict>,
    last_analysis: HashMap<String, Instant>,
    analysis_in_progress: HashMap<String, bool>,
}

impl CurveConflictDetector {
    /// Create new conflict detector with default configuration
    pub fn new() -> Self {
        Self::with_config(ConflictDetectorConfig::default())
    }

    /// Create new conflict detector with custom configuration
    pub fn with_config(config: ConflictDetectorConfig) -> Self {
        Self {
            config,
            sample_buffer: HashMap::new(),
            detected_conflicts: HashMap::new(),
            last_analysis: HashMap::new(),
            analysis_in_progress: HashMap::new(),
        }
    }

    /// Add sample point for analysis (called from RT thread)
    /// 
    /// This function must be fast and non-blocking to maintain RT guarantees.
    #[inline(always)]
    pub fn add_sample(&mut self, axis_name: &str, frame: &AxisFrame) {
        let now = Instant::now();
        
        // Get or create sample buffer for this axis
        let samples = self.sample_buffer.entry(axis_name.to_string()).or_insert_with(Vec::new);
        
        // Add new sample
        samples.push(SamplePoint {
            input: frame.in_raw,
            output: frame.out,
            timestamp: now,
        });

        // Trim old samples to maintain window size (keep it fast)
        let window_duration = Duration::from_millis(self.config.sample_window_ms);
        let cutoff_time = now - window_duration;
        
        // Only trim if we have too many samples (avoid O(n) operation every time)
        if samples.len() > self.config.min_samples * 2 {
            samples.retain(|sample| sample.timestamp > cutoff_time);
        }

        // Check if we should trigger analysis (non-blocking)
        if samples.len() >= self.config.min_samples {
            let should_analyze = self.last_analysis
                .get(axis_name)
                .map(|last| now.duration_since(*last) > Duration::from_millis(1000))
                .unwrap_or(true);

            if should_analyze && !self.analysis_in_progress.get(axis_name).unwrap_or(&false) {
                // Mark analysis as in progress to prevent concurrent analysis
                self.analysis_in_progress.insert(axis_name.to_string(), true);
                
                // Clone samples for analysis to avoid borrowing issues
                let samples_clone = samples.clone();
                
                // Trigger analysis (this should be done off RT thread in practice)
                if let Some(conflict) = self.analyze_samples(axis_name, &samples_clone) {
                    self.detected_conflicts.insert(axis_name.to_string(), conflict);
                }
                
                self.last_analysis.insert(axis_name.to_string(), now);
                self.analysis_in_progress.insert(axis_name.to_string(), false);
            }
        }
    }

    /// Get detected conflicts for an axis
    pub fn get_conflicts(&self, axis_name: &str) -> Option<&CurveConflict> {
        self.detected_conflicts.get(axis_name)
    }

    /// Get all detected conflicts
    pub fn get_all_conflicts(&self) -> &HashMap<String, CurveConflict> {
        &self.detected_conflicts
    }

    /// Clear conflicts for an axis (after resolution)
    pub fn clear_conflicts(&mut self, axis_name: &str) {
        self.detected_conflicts.remove(axis_name);
        self.sample_buffer.remove(axis_name);
        self.last_analysis.remove(axis_name);
    }

    /// Perform one-shot conflict detection with test inputs
    pub fn detect_conflicts_with_test_inputs(
        &mut self,
        axis_name: &str,
        test_node: &dyn Node,
    ) -> Option<CurveConflict> {
        let mut test_samples = Vec::new();
        let now = Instant::now();

        // Generate test inputs from 0.0 to 1.0
        for i in 0..self.config.test_points {
            let input = i as f32 / (self.config.test_points - 1) as f32;
            
            // Create test frame
            let mut frame = AxisFrame::new(input, now.elapsed().as_nanos() as u64);
            
            // Process through node (this simulates the pipeline)
            // Note: In practice, this would need to be done more carefully
            // to avoid affecting the real RT state
            
            test_samples.push(SamplePoint {
                input,
                output: frame.out,
                timestamp: now,
            });
        }

        self.analyze_samples(axis_name, &test_samples)
    }

    /// Analyze samples for curve conflicts (non-RT thread)
    fn analyze_samples(&self, axis_name: &str, samples: &[SamplePoint]) -> Option<CurveConflict> {
        if samples.len() < self.config.min_samples {
            return None;
        }

        debug!("Analyzing {} samples for axis '{}'", samples.len(), axis_name);

        // Calculate linearity metrics
        let linearity_analysis = self.analyze_linearity(samples);
        
        // Detect conflict type and severity
        let conflict_type = self.classify_conflict(&linearity_analysis);
        let severity = self.assess_severity(&linearity_analysis);

        // Only report conflicts above threshold
        if linearity_analysis.combined_nonlinearity < self.config.nonlinearity_threshold {
            return None;
        }

        let description = self.generate_description(&conflict_type, &linearity_analysis);
        let suggested_resolutions = self.generate_resolutions(&conflict_type, &linearity_analysis);

        let metadata = ConflictMetadata {
            sim_curve_strength: linearity_analysis.sim_curve_strength,
            profile_curve_strength: linearity_analysis.profile_curve_strength,
            combined_nonlinearity: linearity_analysis.combined_nonlinearity,
            test_inputs: linearity_analysis.test_inputs,
            expected_outputs: linearity_analysis.expected_outputs,
            actual_outputs: linearity_analysis.actual_outputs,
            detection_timestamp: Instant::now(),
        };

        let conflict = CurveConflict {
            axis_name: axis_name.to_string(),
            conflict_type,
            severity,
            description,
            metadata,
            suggested_resolutions,
            detected_at: Instant::now(),
        };

        info!("Detected curve conflict on axis '{}': {:?}", axis_name, conflict.conflict_type);
        
        Some(conflict)
    }

    /// Analyze linearity of sample data
    fn analyze_linearity(&self, samples: &[SamplePoint]) -> LinearityAnalysis {
        // Sort samples by input for analysis
        let mut sorted_samples: Vec<_> = samples.iter().collect();
        sorted_samples.sort_by(|a, b| a.input.partial_cmp(&b.input).unwrap());

        // Calculate expected linear response
        let min_input = sorted_samples.first().unwrap().input;
        let max_input = sorted_samples.last().unwrap().input;
        let min_output = sorted_samples.first().unwrap().output;
        let max_output = sorted_samples.last().unwrap().output;

        let input_range = max_input - min_input;
        let output_range = max_output - min_output;

        let mut test_inputs = Vec::new();
        let mut expected_outputs = Vec::new();
        let mut actual_outputs = Vec::new();
        let mut deviations = Vec::new();

        // Analyze deviation from linear response
        for sample in &sorted_samples {
            let normalized_input = if input_range > 0.0 {
                (sample.input - min_input) / input_range
            } else {
                0.0
            };

            let expected_output = min_output + normalized_input * output_range;
            let deviation = (sample.output - expected_output).abs();
            let relative_deviation = if output_range > 0.0 {
                deviation / output_range
            } else {
                0.0
            };

            test_inputs.push(sample.input);
            expected_outputs.push(expected_output);
            actual_outputs.push(sample.output);
            deviations.push(relative_deviation);
        }

        // Calculate metrics
        let mean_deviation = deviations.iter().sum::<f32>() / deviations.len() as f32;
        let max_deviation = deviations.iter().fold(0.0f32, |acc, &x| acc.max(x));

        // Estimate curve strengths (simplified heuristic)
        let sim_curve_strength = self.estimate_sim_curve_strength(&sorted_samples);
        let profile_curve_strength = self.estimate_profile_curve_strength(&sorted_samples);

        LinearityAnalysis {
            mean_deviation,
            max_deviation,
            combined_nonlinearity: max_deviation,
            sim_curve_strength,
            profile_curve_strength,
            test_inputs,
            expected_outputs,
            actual_outputs,
        }
    }

    /// Estimate simulator curve strength (heuristic)
    fn estimate_sim_curve_strength(&self, _samples: &[&SamplePoint]) -> f32 {
        // This is a simplified heuristic. In practice, this would require
        // more sophisticated analysis, possibly comparing with known
        // linear baseline or using machine learning techniques.
        0.3 // Placeholder value
    }

    /// Estimate profile curve strength (heuristic)
    fn estimate_profile_curve_strength(&self, _samples: &[&SamplePoint]) -> f32 {
        // This would analyze the profile configuration to determine
        // the strength of any applied curves
        0.2 // Placeholder value
    }

    /// Classify the type of conflict
    fn classify_conflict(&self, analysis: &LinearityAnalysis) -> ConflictType {
        if analysis.sim_curve_strength > 0.1 && analysis.profile_curve_strength > 0.1 {
            ConflictType::DoubleCurve
        } else if analysis.combined_nonlinearity > 0.4 {
            ConflictType::ExcessiveNonlinearity
        } else {
            ConflictType::OpposingCurves
        }
    }

    /// Assess the severity of the conflict
    fn assess_severity(&self, analysis: &LinearityAnalysis) -> ConflictSeverity {
        match analysis.combined_nonlinearity {
            x if x > 0.6 => ConflictSeverity::Critical,
            x if x > 0.4 => ConflictSeverity::High,
            x if x > 0.25 => ConflictSeverity::Medium,
            _ => ConflictSeverity::Low,
        }
    }

    /// Generate human-readable description
    fn generate_description(&self, conflict_type: &ConflictType, analysis: &LinearityAnalysis) -> String {
        match conflict_type {
            ConflictType::DoubleCurve => {
                format!(
                    "Both simulator and profile curves are active, creating {:.1}% non-linear response. \
                     This can make controls feel unpredictable or overly sensitive.",
                    analysis.combined_nonlinearity * 100.0
                )
            }
            ConflictType::ExcessiveNonlinearity => {
                format!(
                    "Combined curve effects create {:.1}% non-linear response, \
                     which may make precise control difficult.",
                    analysis.combined_nonlinearity * 100.0
                )
            }
            ConflictType::OpposingCurves => {
                format!(
                    "Simulator and profile curves appear to work against each other, \
                     creating {:.1}% deviation from expected response.",
                    analysis.combined_nonlinearity * 100.0
                )
            }
        }
    }

    /// Generate suggested resolutions
    fn generate_resolutions(&self, conflict_type: &ConflictType, analysis: &LinearityAnalysis) -> Vec<ConflictResolution> {
        let mut resolutions = Vec::new();

        match conflict_type {
            ConflictType::DoubleCurve => {
                // Suggest disabling sim curve first (usually easier)
                resolutions.push(ConflictResolution {
                    resolution_type: ResolutionType::DisableSimCurve,
                    description: "Disable simulator's built-in curve and use only profile curve".to_string(),
                    estimated_improvement: 0.8,
                    requires_sim_restart: true,
                    parameters: HashMap::new(),
                });

                // Alternative: disable profile curve
                resolutions.push(ConflictResolution {
                    resolution_type: ResolutionType::DisableProfileCurve,
                    description: "Remove curve from profile and use simulator's curve".to_string(),
                    estimated_improvement: 0.7,
                    requires_sim_restart: false,
                    parameters: HashMap::new(),
                });
            }
            ConflictType::ExcessiveNonlinearity => {
                // Suggest reducing curve strength
                resolutions.push(ConflictResolution {
                    resolution_type: ResolutionType::ReduceCurveStrength,
                    description: "Reduce profile curve strength to balance with simulator".to_string(),
                    estimated_improvement: 0.6,
                    requires_sim_restart: false,
                    parameters: {
                        let mut params = HashMap::new();
                        params.insert("target_strength".to_string(), "0.5".to_string());
                        params
                    },
                });
            }
            ConflictType::OpposingCurves => {
                // Suggest gain compensation
                resolutions.push(ConflictResolution {
                    resolution_type: ResolutionType::ApplyGainCompensation,
                    description: "Apply gain compensation to balance opposing effects".to_string(),
                    estimated_improvement: 0.5,
                    requires_sim_restart: false,
                    parameters: {
                        let mut params = HashMap::new();
                        let compensation = 1.0 / (1.0 + analysis.combined_nonlinearity);
                        params.insert("gain_factor".to_string(), compensation.to_string());
                        params
                    },
                });
            }
        }

        resolutions
    }
}

impl Default for CurveConflictDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal analysis results
struct LinearityAnalysis {
    mean_deviation: f32,
    max_deviation: f32,
    combined_nonlinearity: f32,
    sim_curve_strength: f32,
    profile_curve_strength: f32,
    test_inputs: Vec<f32>,
    expected_outputs: Vec<f32>,
    actual_outputs: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_detector_creation() {
        let detector = CurveConflictDetector::new();
        assert_eq!(detector.get_all_conflicts().len(), 0);
    }

    #[test]
    fn test_sample_collection() {
        let mut detector = CurveConflictDetector::new();
        let frame = AxisFrame::new(0.5, 1000);
        
        detector.add_sample("test_axis", &frame);
        
        // Should have collected the sample
        assert!(detector.sample_buffer.contains_key("test_axis"));
        assert_eq!(detector.sample_buffer["test_axis"].len(), 1);
    }

    #[test]
    fn test_conflict_severity_ordering() {
        assert!(ConflictSeverity::Critical > ConflictSeverity::High);
        assert!(ConflictSeverity::High > ConflictSeverity::Medium);
        assert!(ConflictSeverity::Medium > ConflictSeverity::Low);
    }

    #[test]
    fn test_resolution_generation() {
        let detector = CurveConflictDetector::new();
        let analysis = LinearityAnalysis {
            mean_deviation: 0.3,
            max_deviation: 0.5,
            combined_nonlinearity: 0.5,
            sim_curve_strength: 0.4,
            profile_curve_strength: 0.3,
            test_inputs: vec![0.0, 0.5, 1.0],
            expected_outputs: vec![0.0, 0.5, 1.0],
            actual_outputs: vec![0.0, 0.3, 1.0],
        };

        let resolutions = detector.generate_resolutions(&ConflictType::DoubleCurve, &analysis);
        assert!(!resolutions.is_empty());
        assert!(resolutions.iter().any(|r| r.resolution_type == ResolutionType::DisableSimCurve));
    }
}