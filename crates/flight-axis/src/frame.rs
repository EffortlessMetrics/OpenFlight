// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis Frame Processing
//!
//! This module defines the core `AxisFrame` structure that flows through the
//! real-time processing pipeline. Each frame represents a single sample of
//! axis input data with associated metadata and processing results.
//!
//! # Design Principles
//!
//! - **Zero Allocation**: Frame processing never allocates memory
//! - **In-Place Processing**: All transformations modify the frame directly
//! - **Explicit Units**: All values have documented units and ranges
//! - **Monotonic Time**: Timestamps are monotonic and high-resolution
//!
//! # Examples
//!
//! ## Basic Frame Creation
//!
//! ```rust
//! use flight_axis::AxisFrame;
//!
//! // Create a frame with 50% input at timestamp 1000ns
//! let mut frame = AxisFrame::new(0.5, 1000);
//! assert_eq!(frame.in_raw, 0.5);
//! assert_eq!(frame.ts_mono_ns, 1000);
//! ```
//!
//! ## Frame Processing
//!
//! ```rust
//! use flight_axis::{AxisFrame, nodes::DeadzoneNode, Node};
//!
//! let mut frame = AxisFrame::new(0.02, 1000); // Small input
//! let mut deadzone = DeadzoneNode::new(0.03); // 3% deadzone
//!
//! // Process through deadzone - should zero out small input
//! deadzone.step(&mut frame);
//! assert_eq!(frame.out, 0.0);
//! ```

use std::fmt;

/// Core axis processing frame
///
/// Represents a single sample of axis input data flowing through the processing pipeline.
/// All values are normalized to [-1.0, 1.0] range unless otherwise specified.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)] // Ensure consistent memory layout for performance
pub struct AxisFrame {
    /// Raw input value from hardware [-1.0, 1.0]
    pub in_raw: f32,
    
    /// Processed output value [-1.0, 1.0]
    pub out: f32,
    
    /// Input derivative (rate of change) per second
    pub d_in_dt: f32,
    
    /// Monotonic timestamp in nanoseconds
    pub ts_mono_ns: u64,
}

impl AxisFrame {
    /// Create a new axis frame with the given input and timestamp
    ///
    /// # Arguments
    ///
    /// * `input` - Raw input value, should be in range [-1.0, 1.0]
    /// * `timestamp_ns` - Monotonic timestamp in nanoseconds
    ///
    /// # Examples
    ///
    /// ```rust
    /// use flight_axis::AxisFrame;
    ///
    /// let frame = AxisFrame::new(0.75, 1_000_000_000);
    /// assert_eq!(frame.in_raw, 0.75);
    /// assert_eq!(frame.out, 0.75); // Initially same as input
    /// ```
    pub fn new(input: f32, timestamp_ns: u64) -> Self {
        Self {
            in_raw: input,
            out: input, // Initially, output equals input
            d_in_dt: 0.0, // Will be calculated by derivative node
            ts_mono_ns: timestamp_ns,
        }
    }
    
    /// Create a frame with all fields specified
    ///
    /// This is primarily used for testing and replay scenarios where
    /// you need to recreate exact frame states.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use flight_axis::AxisFrame;
    ///
    /// let frame = AxisFrame::with_all_fields(0.5, 0.3, 0.1, 2_000_000_000);
    /// assert_eq!(frame.in_raw, 0.5);
    /// assert_eq!(frame.out, 0.3);
    /// assert_eq!(frame.d_in_dt, 0.1);
    /// ```
    pub fn with_all_fields(input: f32, output: f32, derivative: f32, timestamp_ns: u64) -> Self {
        Self {
            in_raw: input,
            out: output,
            d_in_dt: derivative,
            ts_mono_ns: timestamp_ns,
        }
    }
    
    /// Reset the frame to initial state (output = input, derivative = 0)
    ///
    /// This is useful when reprocessing a frame through a different pipeline.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use flight_axis::AxisFrame;
    ///
    /// let mut frame = AxisFrame::new(0.5, 1000);
    /// frame.out = 0.3; // Simulate processing
    /// frame.d_in_dt = 0.1;
    ///
    /// frame.reset();
    /// assert_eq!(frame.out, 0.5); // Back to input value
    /// assert_eq!(frame.d_in_dt, 0.0);
    /// ```
    pub fn reset(&mut self) {
        self.out = self.in_raw;
        self.d_in_dt = 0.0;
    }
    
    /// Check if the frame values are within valid ranges
    ///
    /// Returns `true` if all values are finite and within expected ranges:
    /// - Input and output should be in [-1.0, 1.0]
    /// - Derivative should be finite
    /// - Timestamp should be non-zero
    ///
    /// # Examples
    ///
    /// ```rust
    /// use flight_axis::AxisFrame;
    ///
    /// let valid_frame = AxisFrame::new(0.5, 1000);
    /// assert!(valid_frame.is_valid());
    ///
    /// let invalid_frame = AxisFrame::new(f32::NAN, 1000);
    /// assert!(!invalid_frame.is_valid());
    /// ```
    pub fn is_valid(&self) -> bool {
        self.in_raw.is_finite() &&
        self.out.is_finite() &&
        self.d_in_dt.is_finite() &&
        self.in_raw >= -1.0 && self.in_raw <= 1.0 &&
        self.out >= -1.0 && self.out <= 1.0 &&
        self.ts_mono_ns > 0
    }
    
    /// Get the processing latency if compared with another frame
    ///
    /// Calculates the time difference between this frame and another,
    /// typically used to measure processing delays.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use flight_axis::AxisFrame;
    ///
    /// let frame1 = AxisFrame::new(0.5, 1_000_000);
    /// let frame2 = AxisFrame::new(0.5, 1_500_000);
    ///
    /// assert_eq!(frame2.latency_from(&frame1), 500_000);
    /// ```
    pub fn latency_from(&self, other: &AxisFrame) -> u64 {
        self.ts_mono_ns.abs_diff(other.ts_mono_ns)
    }
    
    /// Calculate the time delta from the previous frame in seconds
    ///
    /// This is commonly used by nodes that need to know the time step
    /// for rate-based calculations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use flight_axis::AxisFrame;
    ///
    /// let frame1 = AxisFrame::new(0.5, 1_000_000_000); // 1 second
    /// let frame2 = AxisFrame::new(0.6, 1_004_000_000); // 1.004 seconds
    ///
    /// let dt = frame2.delta_time_from(&frame1);
    /// assert!((dt - 0.004).abs() < 1e-6); // 4ms delta
    /// ```
    pub fn delta_time_from(&self, previous: &AxisFrame) -> f32 {
        if self.ts_mono_ns > previous.ts_mono_ns {
            (self.ts_mono_ns - previous.ts_mono_ns) as f32 / 1_000_000_000.0
        } else {
            0.0
        }
    }
    
    /// Update the derivative based on the previous frame
    ///
    /// This calculates the rate of change of the input value and stores it
    /// in the d_in_dt field for use by other pipeline nodes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use flight_axis::AxisFrame;
    ///
    /// let frame1 = AxisFrame::new(0.5, 1_000_000_000);
    /// let mut frame2 = AxisFrame::new(0.6, 1_004_000_000);
    ///
    /// frame2.update_derivative(&frame1);
    /// assert!(frame2.d_in_dt > 0.0); // Positive derivative
    /// ```
    pub fn update_derivative(&mut self, previous: &AxisFrame) {
        let dt = self.delta_time_from(previous);
        if dt > 0.0 {
            self.d_in_dt = (self.in_raw - previous.in_raw) / dt;
        } else {
            self.d_in_dt = 0.0;
        }
    }
    
    /// Apply a simple linear transformation to the output
    ///
    /// This is a convenience method for basic scaling and offset operations.
    /// More complex transformations should use dedicated pipeline nodes.
    ///
    /// # Arguments
    ///
    /// * `scale` - Multiplication factor
    /// * `offset` - Addition offset (applied after scaling)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use flight_axis::AxisFrame;
    ///
    /// let mut frame = AxisFrame::new(0.5, 1000);
    /// frame.transform(2.0, 0.1); // Scale by 2, add 0.1
    /// assert_eq!(frame.out, 1.1);
    /// ```
    pub fn transform(&mut self, scale: f32, offset: f32) {
        self.out = self.out * scale + offset;
    }
    
    /// Clamp the output to the specified range
    ///
    /// Ensures the output value stays within the given bounds.
    /// This is commonly used for safety limits and capability enforcement.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use flight_axis::AxisFrame;
    ///
    /// let mut frame = AxisFrame::new(1.5, 1000); // Out of range
    /// frame.clamp(-1.0, 1.0);
    /// assert_eq!(frame.out, 1.0); // Clamped to maximum
    /// ```
    pub fn clamp(&mut self, min: f32, max: f32) {
        self.out = self.out.clamp(min, max);
    }
}

impl fmt::Display for AxisFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AxisFrame {{ in: {:.3}, out: {:.3}, d/dt: {:.3}, ts: {}ns }}", 
               self.in_raw, self.out, self.d_in_dt, self.ts_mono_ns)
    }
}

/// Frame validation error types
#[derive(Debug, Clone, PartialEq)]
pub enum FrameError {
    /// Input value is not finite (NaN or infinite)
    InvalidInput(f32),
    /// Input value is outside valid range [-1.0, 1.0]
    InputOutOfRange(f32),
    /// Output value is not finite
    InvalidOutput(f32),
    /// Output value is outside valid range [-1.0, 1.0]
    OutputOutOfRange(f32),
    /// Timestamp is zero or invalid
    InvalidTimestamp(u64),
    /// Derivative is not finite
    InvalidDerivative(f32),
}

impl AxisFrame {
    /// Validate frame and return detailed error information
    ///
    /// This provides more detailed validation than `is_valid()`,
    /// returning specific error information for debugging.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use flight_axis::{AxisFrame, FrameError};
    ///
    /// let invalid_frame = AxisFrame::new(2.0, 1000); // Out of range
    /// match invalid_frame.validate() {
    ///     Err(FrameError::InputOutOfRange(val)) => {
    ///         assert_eq!(val, 2.0);
    ///     }
    ///     _ => panic!("Expected InputOutOfRange error"),
    /// }
    /// ```
    pub fn validate(&self) -> Result<(), FrameError> {
        // Check input
        if !self.in_raw.is_finite() {
            return Err(FrameError::InvalidInput(self.in_raw));
        }
        if self.in_raw < -1.0 || self.in_raw > 1.0 {
            return Err(FrameError::InputOutOfRange(self.in_raw));
        }
        
        // Check output
        if !self.out.is_finite() {
            return Err(FrameError::InvalidOutput(self.out));
        }
        if self.out < -1.0 || self.out > 1.0 {
            return Err(FrameError::OutputOutOfRange(self.out));
        }
        
        // Check derivative
        if !self.d_in_dt.is_finite() {
            return Err(FrameError::InvalidDerivative(self.d_in_dt));
        }
        
        // Check timestamp
        if self.ts_mono_ns == 0 {
            return Err(FrameError::InvalidTimestamp(self.ts_mono_ns));
        }
        
        Ok(())
    }
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::InvalidInput(val) => write!(f, "Invalid input value: {}", val),
            FrameError::InputOutOfRange(val) => write!(f, "Input out of range [-1.0, 1.0]: {}", val),
            FrameError::InvalidOutput(val) => write!(f, "Invalid output value: {}", val),
            FrameError::OutputOutOfRange(val) => write!(f, "Output out of range [-1.0, 1.0]: {}", val),
            FrameError::InvalidTimestamp(ts) => write!(f, "Invalid timestamp: {}", ts),
            FrameError::InvalidDerivative(val) => write!(f, "Invalid derivative: {}", val),
        }
    }
}

impl std::error::Error for FrameError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_creation() {
        let frame = AxisFrame::new(0.5, 1000);
        assert_eq!(frame.in_raw, 0.5);
        assert_eq!(frame.out, 0.5);
        assert_eq!(frame.d_in_dt, 0.0);
        assert_eq!(frame.ts_mono_ns, 1000);
    }
    
    #[test]
    fn test_frame_validation() {
        let valid_frame = AxisFrame::new(0.5, 1000);
        assert!(valid_frame.is_valid());
        assert!(valid_frame.validate().is_ok());
        
        let invalid_frame = AxisFrame::new(2.0, 1000);
        assert!(!invalid_frame.is_valid());
        assert!(matches!(invalid_frame.validate(), Err(FrameError::InputOutOfRange(_))));
    }
    
    #[test]
    fn test_frame_reset() {
        let mut frame = AxisFrame::new(0.5, 1000);
        frame.out = 0.3;
        frame.d_in_dt = 0.1;
        
        frame.reset();
        assert_eq!(frame.out, 0.5);
        assert_eq!(frame.d_in_dt, 0.0);
    }
    
    #[test]
    fn test_frame_transform() {
        let mut frame = AxisFrame::new(0.5, 1000);
        frame.transform(2.0, 0.1);
        assert_eq!(frame.out, 1.1);
    }
    
    #[test]
    fn test_frame_clamp() {
        let mut frame = AxisFrame::new(1.5, 1000);
        frame.clamp(-1.0, 1.0);
        assert_eq!(frame.out, 1.0);
    }
    
    #[test]
    fn test_delta_time_calculation() {
        let frame1 = AxisFrame::new(0.5, 1_000_000_000);
        let frame2 = AxisFrame::new(0.6, 1_004_000_000);
        
        let dt = frame2.delta_time_from(&frame1);
        assert!((dt - 0.004).abs() < 1e-6);
    }
    
    #[test]
    fn test_latency_calculation() {
        let frame1 = AxisFrame::new(0.5, 1_000_000);
        let frame2 = AxisFrame::new(0.5, 1_500_000);
        
        assert_eq!(frame2.latency_from(&frame1), 500_000);
        assert_eq!(frame1.latency_from(&frame2), 500_000);
    }
}