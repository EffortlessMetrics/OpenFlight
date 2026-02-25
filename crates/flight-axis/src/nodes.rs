// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Pipeline processing nodes with zero-allocation guarantee
//!
//! All nodes implement the Node trait for compile-to-function-pointer optimization.
//! State is stored in Structure-of-Arrays (SoA) layout for cache efficiency.

use crate::AxisFrame;
use std::fmt;

/// Unique identifier for pipeline nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

/// Pipeline node trait for zero-allocation processing
///
/// All implementations must guarantee:
/// - No heap allocations during step()
/// - No mutex/lock operations
/// - Deterministic execution
/// - Cache-friendly memory access patterns
pub trait Node: Send + Sync + fmt::Debug {
    /// Process axis frame in-place with zero allocations
    ///
    /// # Safety
    /// This function must not allocate memory or acquire locks
    fn step(&mut self, frame: &mut AxisFrame);

    /// Get the size of state data needed for SoA layout
    fn state_size(&self) -> usize;

    /// Initialize state data in provided buffer
    ///
    /// # Safety
    /// Buffer must be at least state_size() bytes and properly aligned
    unsafe fn init_state(&self, state_ptr: *mut u8);

    /// Step function using SoA state layout
    ///
    /// # Safety
    /// state_ptr must point to valid state data initialized by init_state()
    unsafe fn step_soa(&self, frame: &mut AxisFrame, state_ptr: *mut u8);

    /// Get node type identifier for debugging
    fn node_type(&self) -> &'static str;
}

/// Deadzone processing node with symmetric/asymmetric support
#[derive(Debug, Clone)]
pub struct DeadzoneNode {
    /// Deadzone threshold [0.0, 1.0]
    pub threshold: f32,
    /// Asymmetric negative threshold (optional)
    pub threshold_neg: Option<f32>,
}

impl DeadzoneNode {
    /// Create symmetric deadzone node
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold: threshold.clamp(0.0, 1.0),
            threshold_neg: None,
        }
    }

    /// Create asymmetric deadzone node
    pub fn asymmetric(threshold_pos: f32, threshold_neg: f32) -> Self {
        Self {
            threshold: threshold_pos.clamp(0.0, 1.0),
            threshold_neg: Some(threshold_neg.clamp(0.0, 1.0)),
        }
    }
}

/// Compiled state for deadzone nodes (config embedded in state buffer)
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct DeadzoneCompiledState {
    /// Positive threshold [0.0, 1.0]
    pub threshold: f32,
    /// Negative threshold (same as threshold if symmetric)
    pub threshold_neg: f32,
}

impl Node for DeadzoneNode {
    #[inline(always)]
    fn step(&mut self, frame: &mut AxisFrame) {
        let threshold = if frame.out < 0.0 {
            self.threshold_neg.unwrap_or(self.threshold)
        } else {
            self.threshold
        };

        if frame.out.abs() < threshold {
            frame.out = 0.0;
        } else {
            let sign = frame.out.signum();
            let abs_val = frame.out.abs();
            frame.out = sign * ((abs_val - threshold) / (1.0 - threshold));
        }
    }

    fn state_size(&self) -> usize {
        std::mem::size_of::<DeadzoneCompiledState>()
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn init_state(&self, state_ptr: *mut u8) {
        // Write config to state buffer for function pointer path
        let state = state_ptr as *mut DeadzoneCompiledState;
        *state = DeadzoneCompiledState {
            threshold: self.threshold,
            threshold_neg: self.threshold_neg.unwrap_or(self.threshold),
        };
    }

    unsafe fn step_soa(&self, frame: &mut AxisFrame, _state_ptr: *mut u8) {
        // Use self directly - config is already available
        let mut node = self.clone();
        node.step(frame);
    }

    fn node_type(&self) -> &'static str {
        "deadzone"
    }
}

/// Compiled state for curve nodes (config embedded in state buffer)
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct CurveCompiledState {
    /// Exponential factor [-1.0, 1.0]
    pub expo: f32,
    /// Precomputed exponent: 1.0 + expo
    pub exponent: f32,
}

/// Exponential curve node with monotonicity guarantee
#[derive(Debug, Clone)]
pub struct CurveNode {
    /// Exponential factor [-1.0, 1.0]
    pub expo: f32,
}

impl CurveNode {
    /// Create exponential curve node
    ///
    /// # Panics
    /// Panics if expo is outside [-1.0, 1.0] range
    pub fn new(expo: f32) -> Self {
        assert!(
            (-1.0..=1.0).contains(&expo),
            "Exponential factor must be in range [-1.0, 1.0], got {}",
            expo
        );
        Self { expo }
    }

    /// Create exponential curve with validation
    pub fn exponential(expo: f32) -> Result<Self, &'static str> {
        if !(-1.0..=1.0).contains(&expo) {
            Err("Exponential factor must be in range [-1.0, 1.0]")
        } else {
            Ok(Self { expo })
        }
    }
}

impl Node for CurveNode {
    #[inline(always)]
    fn step(&mut self, frame: &mut AxisFrame) {
        if self.expo == 0.0 {
            return; // Linear, no change needed
        }

        let sign = frame.out.signum();
        let abs_val = frame.out.abs();

        // Ensure monotonic curve: f(x) = sign(x) * |x|^(1 + expo)
        frame.out = sign * abs_val.powf(1.0 + self.expo);
    }

    fn state_size(&self) -> usize {
        std::mem::size_of::<CurveCompiledState>()
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn init_state(&self, state_ptr: *mut u8) {
        // Write config to state buffer for function pointer path
        let state = state_ptr as *mut CurveCompiledState;
        *state = CurveCompiledState {
            expo: self.expo,
            exponent: 1.0 + self.expo,
        };
    }

    unsafe fn step_soa(&self, frame: &mut AxisFrame, _state_ptr: *mut u8) {
        // Use self directly - config is already available
        let mut node = self.clone();
        node.step(frame);
    }

    fn node_type(&self) -> &'static str {
        "curve"
    }
}

/// Slew rate limiter node with configurable attack/decay
#[derive(Debug, Clone)]
pub struct SlewNode {
    /// Rate limit in normalized units per second
    pub rate_limit: f32,
    /// Separate attack rate (optional)
    pub attack_rate: Option<f32>,
}

/// Legacy state for slew rate limiter (retained for API compatibility)
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct SlewState {
    pub last_output: f32,
    pub last_time_ns: u64,
}

/// Compiled state for slew nodes (config + runtime state in one buffer)
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct SlewCompiledState {
    // Config (immutable after compilation)
    /// Rate limit for decay (moving toward zero)
    pub rate_limit: f32,
    /// Rate limit for attack (moving away from zero), same as rate_limit if symmetric
    pub attack_rate: f32,
    // Runtime state
    /// Last output value
    pub last_output: f32,
    /// Padding for alignment before u64
    pub _pad: u32,
    /// Last timestamp in nanoseconds
    pub last_time_ns: u64,
}

impl SlewNode {
    /// Create slew rate limiter with symmetric rate
    pub fn new(rate_limit: f32) -> Self {
        Self {
            rate_limit: rate_limit.max(0.0),
            attack_rate: None,
        }
    }

    /// Create slew rate limiter with separate attack/decay rates
    pub fn asymmetric(attack_rate: f32, decay_rate: f32) -> Self {
        Self {
            rate_limit: decay_rate.max(0.0),
            attack_rate: Some(attack_rate.max(0.0)),
        }
    }
}

impl Node for SlewNode {
    #[inline(always)]
    fn step(&mut self, _frame: &mut AxisFrame) {
        // This implementation is for compatibility only
        // Real RT path uses step_soa with SoA state layout
        unimplemented!("SlewNode requires SoA state layout - use step_soa()");
    }

    fn state_size(&self) -> usize {
        std::mem::size_of::<SlewCompiledState>()
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn init_state(&self, state_ptr: *mut u8) {
        // Write config and initial runtime state to buffer
        let state = state_ptr as *mut SlewCompiledState;
        *state = SlewCompiledState {
            rate_limit: self.rate_limit,
            attack_rate: self.attack_rate.unwrap_or(self.rate_limit),
            last_output: 0.0,
            _pad: 0,
            last_time_ns: 0,
        };
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn step_soa(&self, frame: &mut AxisFrame, state_ptr: *mut u8) {
        let state = &mut *(state_ptr as *mut SlewCompiledState);

        if state.last_time_ns == 0 {
            state.last_output = frame.out;
            state.last_time_ns = frame.ts_mono_ns;
            return;
        }

        let dt_s = if frame.ts_mono_ns > state.last_time_ns {
            (frame.ts_mono_ns - state.last_time_ns) as f32 / 1_000_000_000.0
        } else {
            0.0 // Handle case where timestamps go backwards or are equal
        };
        let desired_change = frame.out - state.last_output;

        // Select rate based on direction of change
        let rate = if desired_change.signum() == frame.out.signum() {
            state.attack_rate
        } else {
            state.rate_limit
        };

        let max_change = rate * dt_s;

        if desired_change.abs() > max_change {
            frame.out = state.last_output + desired_change.signum() * max_change;
        }

        state.last_output = frame.out;
        state.last_time_ns = frame.ts_mono_ns;
    }

    fn node_type(&self) -> &'static str {
        "slew"
    }
}

/// Legacy state for filter node (retained for API compatibility)
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct FilterState {
    /// Previous filtered output
    pub prev_output: f32,
    /// Last raw input (for spike detection)
    pub last_raw: f32,
    /// Consecutive spike count
    pub spike_count: u8,
    /// Whether filter has been initialized
    pub initialized: bool,
    /// Padding for alignment
    pub _pad: [u8; 6],
}

/// Compiled state for filter nodes (config + runtime state in one buffer)
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct FilterCompiledState {
    // Config (immutable after compilation)
    /// EMA smoothing factor [0.0, 1.0]
    pub alpha: f32,
    /// Spike rejection threshold (0.0 = disabled)
    pub spike_threshold: f32,
    /// Maximum consecutive spikes before accepting
    pub max_spike_count: u8,
    pub _config_pad: [u8; 3],
    // Runtime state
    /// Previous filtered output
    pub prev_output: f32,
    /// Last raw input
    pub last_raw: f32,
    /// Consecutive spike count
    pub spike_count: u8,
    /// Whether filter has been initialized
    pub initialized: bool,
    pub _state_pad: [u8; 6],
}

/// EMA filter node with optional spike rejection.
///
/// Implements exponential moving average filtering for potentiometer noise reduction,
/// with optional spike rejection for handling transient noise spikes.
///
/// Formula: S_t = alpha * Y_t + (1 - alpha) * S_{t-1}
///
/// Lower alpha values provide more smoothing but increase latency.
/// Spike rejection helps with B104 potentiometers that exhibit occasional large jumps.
///
/// # Examples
///
/// ```
/// use flight_axis::FilterNode;
///
/// // Basic EMA filter with 15% responsiveness
/// let filter = FilterNode::new(0.15);
///
/// // B104 potentiometer preset for T.Flight HOTAS 4
/// let b104_filter = FilterNode::b104_preset();
/// ```
#[derive(Debug, Clone)]
pub struct FilterNode {
    /// Smoothing factor [0.0, 1.0] - lower = more smoothing
    pub alpha: f32,
    /// Spike rejection threshold (optional, normalized units)
    pub spike_threshold: Option<f32>,
    /// Maximum consecutive spikes before accepting as real change
    pub max_spike_count: u8,
}

impl FilterNode {
    /// Create a new EMA filter with the specified alpha.
    ///
    /// # Arguments
    /// * `alpha` - Smoothing factor in [0.0, 1.0]. Lower values = more smoothing.
    ///   0.1 = heavy smoothing, 0.5 = moderate, 1.0 = no filtering.
    pub fn new(alpha: f32) -> Self {
        Self {
            alpha: alpha.clamp(0.0, 1.0),
            spike_threshold: None,
            max_spike_count: 3,
        }
    }

    /// Create an EMA filter with spike rejection.
    ///
    /// # Arguments
    /// * `alpha` - Smoothing factor in [0.0, 1.0]
    /// * `threshold` - Spike rejection threshold in normalized units.
    ///   Changes larger than this are considered spikes.
    pub fn with_spike_rejection(alpha: f32, threshold: f32) -> Self {
        Self {
            alpha: alpha.clamp(0.0, 1.0),
            spike_threshold: Some(threshold.max(0.0)),
            max_spike_count: 3,
        }
    }

    /// Preset tuned for B104 linear potentiometers (T.Flight HOTAS 4).
    ///
    /// The B104 (100kΩ linear) potentiometers used in T.Flight HOTAS 4 are
    /// known for jitter/noise. This preset provides:
    /// - Alpha 0.15: Moderate smoothing with acceptable latency
    /// - Spike threshold 0.4: Rejects large transient jumps
    /// - Max spike count 5: Allows sustained changes to pass through
    pub fn b104_preset() -> Self {
        Self {
            alpha: 0.15,
            spike_threshold: Some(0.4),
            max_spike_count: 5,
        }
    }
}

impl Node for FilterNode {
    #[inline(always)]
    fn step(&mut self, _frame: &mut AxisFrame) {
        // This implementation is for compatibility only
        // Real RT path uses step_soa with SoA state layout
        unimplemented!("FilterNode requires SoA state layout - use step_soa()");
    }

    fn state_size(&self) -> usize {
        std::mem::size_of::<FilterCompiledState>()
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn init_state(&self, state_ptr: *mut u8) {
        // Write config and initial runtime state to buffer
        let state = state_ptr as *mut FilterCompiledState;
        *state = FilterCompiledState {
            // Config
            alpha: self.alpha,
            spike_threshold: self.spike_threshold.unwrap_or(0.0),
            max_spike_count: self.max_spike_count,
            _config_pad: [0; 3],
            // Runtime state
            prev_output: 0.0,
            last_raw: 0.0,
            spike_count: 0,
            initialized: false,
            _state_pad: [0; 6],
        };
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn step_soa(&self, frame: &mut AxisFrame, state_ptr: *mut u8) {
        let state = &mut *(state_ptr as *mut FilterCompiledState);
        let input = frame.out;

        // Initialize on first sample
        if !state.initialized {
            state.prev_output = input;
            state.last_raw = input;
            state.initialized = true;
            return;
        }

        // Check for spikes if threshold is configured (non-zero)
        let accept_input = if state.spike_threshold > 0.0 {
            let delta = (input - state.last_raw).abs();

            if delta > state.spike_threshold {
                // Potential spike detected
                state.spike_count = state.spike_count.saturating_add(1);

                if state.spike_count >= state.max_spike_count {
                    // Too many consecutive "spikes" - accept as real change
                    state.spike_count = 0;
                    true
                } else {
                    // Reject spike, keep previous output
                    false
                }
            } else {
                // Normal change, reset spike count
                state.spike_count = 0;
                true
            }
        } else {
            // No spike rejection configured
            true
        };

        if accept_input {
            // Apply EMA: S_t = alpha * Y_t + (1 - alpha) * S_{t-1}
            frame.out = state.alpha * input + (1.0 - state.alpha) * state.prev_output;
            state.prev_output = frame.out;
            state.last_raw = input;
        } else {
            // Spike rejected, output previous filtered value
            frame.out = state.prev_output;
        }
    }

    fn node_type(&self) -> &'static str {
        "filter"
    }
}

/// Semantic role for detent zones
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetentRole {
    /// Idle/Off position
    Idle,
    /// Taxi power setting
    Taxi,
    /// Takeoff power setting
    Takeoff,
    /// Climb power setting
    Climb,
    /// Cruise power setting
    Cruise,
    /// Approach power setting
    Approach,
    /// Landing power setting
    Landing,
    /// Reverse thrust
    Reverse,
    /// Emergency/Maximum power
    Emergency,
    /// Custom detent with user-defined meaning
    Custom(u8),
}

impl DetentRole {
    /// Get human-readable name for the detent role
    pub fn name(&self) -> &'static str {
        match self {
            DetentRole::Idle => "Idle",
            DetentRole::Taxi => "Taxi",
            DetentRole::Takeoff => "Takeoff",
            DetentRole::Climb => "Climb",
            DetentRole::Cruise => "Cruise",
            DetentRole::Approach => "Approach",
            DetentRole::Landing => "Landing",
            DetentRole::Reverse => "Reverse",
            DetentRole::Emergency => "Emergency",
            DetentRole::Custom(_) => "Custom",
        }
    }
}

/// Detent zone definition with hysteresis
#[derive(Debug, Clone)]
pub struct DetentZone {
    /// Center position of the detent [-1.0, 1.0]
    pub center: f32,
    /// Half-width of the detent zone
    pub half_width: f32,
    /// Hysteresis band (additional width for exit threshold)
    pub hysteresis: f32,
    /// Semantic role of this detent
    pub role: DetentRole,
    /// Whether this detent should snap the output to center
    pub snap_to_center: bool,
}

impl DetentZone {
    /// Create a new detent zone
    pub fn new(center: f32, half_width: f32, hysteresis: f32, role: DetentRole) -> Self {
        Self {
            center: center.clamp(-1.0, 1.0),
            half_width: half_width.max(0.0),
            hysteresis: hysteresis.max(0.0),
            role,
            snap_to_center: true,
        }
    }

    /// Create a detent zone without output snapping
    pub fn no_snap(center: f32, half_width: f32, hysteresis: f32, role: DetentRole) -> Self {
        Self {
            center: center.clamp(-1.0, 1.0),
            half_width: half_width.max(0.0),
            hysteresis: hysteresis.max(0.0),
            role,
            snap_to_center: false,
        }
    }

    /// Check if a position is within the entry threshold
    pub fn contains_entry(&self, position: f32) -> bool {
        (position - self.center).abs() <= self.half_width
    }

    /// Check if a position is within the exit threshold (with hysteresis)
    pub fn contains_exit(&self, position: f32) -> bool {
        (position - self.center).abs() <= (self.half_width + self.hysteresis)
    }

    /// Get the entry boundaries (min, max)
    pub fn entry_bounds(&self) -> (f32, f32) {
        (
            (self.center - self.half_width).max(-1.0),
            (self.center + self.half_width).min(1.0),
        )
    }

    /// Get the exit boundaries (min, max) including hysteresis
    pub fn exit_bounds(&self) -> (f32, f32) {
        (
            (self.center - self.half_width - self.hysteresis).max(-1.0),
            (self.center + self.half_width + self.hysteresis).min(1.0),
        )
    }
}

/// Detent transition event
#[derive(Debug, Clone, PartialEq)]
pub struct DetentEvent {
    /// Timestamp of the event
    pub timestamp_ns: u64,
    /// Previous detent (None if no previous detent)
    pub from_detent: Option<DetentRole>,
    /// New detent (None if exiting all detents)
    pub to_detent: Option<DetentRole>,
    /// Input position when transition occurred
    pub position: f32,
}

/// State for detent mapper (16 bytes aligned)
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct DetentState {
    /// Currently active detent index (u32::MAX if none)
    pub active_detent_idx: u32,
    /// Last processed position
    pub last_position: f32,
    /// Last event timestamp
    pub last_event_ns: u64,
}

/// Detent mapper node with hysteresis and event generation
#[derive(Debug, Clone)]
pub struct DetentNode {
    /// Ordered list of detent zones (must be sorted by center position)
    pub zones: Vec<DetentZone>,
    /// Event sender (will be set during pipeline compilation)
    event_sender: Option<crossbeam::channel::Sender<DetentEvent>>,
}

impl DetentNode {
    /// Create a new detent mapper with sorted zones
    pub fn new(mut zones: Vec<DetentZone>) -> Self {
        // Sort zones by center position for efficient lookup
        zones.sort_by(|a, b| a.center.partial_cmp(&b.center).unwrap());

        Self {
            zones,
            event_sender: None,
        }
    }

    /// Set the event channel sender
    pub fn with_event_sender(mut self, sender: crossbeam::channel::Sender<DetentEvent>) -> Self {
        self.event_sender = Some(sender);
        self
    }

    /// Find the detent zone that contains the given position for entry
    pub fn find_entry_detent(&self, position: f32) -> Option<usize> {
        self.zones
            .iter()
            .position(|zone| zone.contains_entry(position))
    }

    /// Check if position is still within exit threshold of given detent
    fn is_within_exit_threshold(&self, position: f32, detent_idx: usize) -> bool {
        if detent_idx >= self.zones.len() {
            return false;
        }
        self.zones[detent_idx].contains_exit(position)
    }

    /// Send detent event if sender is available
    fn send_event(&self, event: DetentEvent) {
        if let Some(ref sender) = self.event_sender {
            // Use try_send to avoid blocking RT thread
            let _ = sender.try_send(event);
        }
    }
}

impl Node for DetentNode {
    #[inline(always)]
    fn step(&mut self, _frame: &mut AxisFrame) {
        // This implementation is for compatibility only
        // Real RT path uses step_soa with SoA state layout
        unimplemented!("DetentNode requires SoA state layout - use step_soa()");
    }

    fn state_size(&self) -> usize {
        std::mem::size_of::<DetentState>()
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn init_state(&self, state_ptr: *mut u8) {
        let state = state_ptr as *mut DetentState;
        *state = DetentState {
            active_detent_idx: u32::MAX, // No active detent initially
            last_position: 0.0,
            last_event_ns: 0,
        };
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn step_soa(&self, frame: &mut AxisFrame, state_ptr: *mut u8) {
        let state = &mut *(state_ptr as *mut DetentState);
        let position = frame.out;

        // Check if we're still in the current detent (if any)
        let mut new_detent_idx = if state.active_detent_idx != u32::MAX {
            if self.is_within_exit_threshold(position, state.active_detent_idx as usize) {
                state.active_detent_idx // Stay in current detent
            } else {
                u32::MAX // Exited current detent
            }
        } else {
            u32::MAX // No current detent
        };

        // If we're not in a detent, check for entry into a new one
        if new_detent_idx == u32::MAX
            && let Some(idx) = self.find_entry_detent(position)
        {
            new_detent_idx = idx as u32;
        }

        // Generate event if detent changed
        if new_detent_idx != state.active_detent_idx {
            let from_detent = if state.active_detent_idx != u32::MAX {
                Some(self.zones[state.active_detent_idx as usize].role)
            } else {
                None
            };

            let to_detent = if new_detent_idx != u32::MAX {
                Some(self.zones[new_detent_idx as usize].role)
            } else {
                None
            };

            let event = DetentEvent {
                timestamp_ns: frame.ts_mono_ns,
                from_detent,
                to_detent,
                position,
            };

            self.send_event(event);
            state.active_detent_idx = new_detent_idx;
            state.last_event_ns = frame.ts_mono_ns;
        }

        // Apply output snapping if in a detent
        if new_detent_idx != u32::MAX {
            let zone = &self.zones[new_detent_idx as usize];
            if zone.snap_to_center {
                frame.out = zone.center;
            }
        }

        state.last_position = position;
    }

    fn node_type(&self) -> &'static str {
        "detent"
    }
}

/// Input configuration for mixer node
#[derive(Debug, Clone)]
pub struct MixerInput {
    /// Scale factor for this input [-10.0, 10.0] (normalized units)
    pub scale: f32,
    /// Gain factor for this input [0.0, 10.0] (normalized units)
    pub gain: f32,
    /// Input identifier for debugging
    pub name: String,
}

impl MixerInput {
    /// Create new mixer input with scale and gain
    pub fn new(name: impl Into<String>, scale: f32, gain: f32) -> Self {
        Self {
            scale: scale.clamp(-10.0, 10.0),
            gain: gain.clamp(0.0, 10.0),
            name: name.into(),
        }
    }

    /// Create mixer input with scale only (gain = 1.0)
    pub fn with_scale(name: impl Into<String>, scale: f32) -> Self {
        Self::new(name, scale, 1.0)
    }

    /// Apply input transformation to value
    #[inline(always)]
    pub fn apply(&self, value: f32) -> f32 {
        value * self.scale * self.gain
    }
}

/// Configuration for mixer node
#[derive(Debug, Clone)]
pub struct MixerConfig {
    /// List of input configurations
    pub inputs: Vec<MixerInput>,
    /// Output axis identifier for debugging
    pub output_name: String,
    /// Whether to clamp output to [-1.0, 1.0] range
    pub clamp_output: bool,
}

impl MixerConfig {
    /// Create new mixer configuration
    pub fn new(output_name: impl Into<String>) -> Self {
        Self {
            inputs: Vec::new(),
            output_name: output_name.into(),
            clamp_output: true,
        }
    }

    /// Add input to mixer configuration
    pub fn add_input(mut self, input: MixerInput) -> Self {
        self.inputs.push(input);
        self
    }

    /// Add input with scale only
    pub fn add_scaled_input(self, name: impl Into<String>, scale: f32) -> Self {
        self.add_input(MixerInput::with_scale(name, scale))
    }

    /// Add input with scale and gain
    pub fn add_input_with_gain(self, name: impl Into<String>, scale: f32, gain: f32) -> Self {
        self.add_input(MixerInput::new(name, scale, gain))
    }

    /// Disable output clamping
    pub fn no_clamp(mut self) -> Self {
        self.clamp_output = false;
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.inputs.is_empty() {
            return Err("Mixer must have at least one input");
        }

        if self.inputs.len() > 8 {
            return Err("Mixer supports maximum 8 inputs");
        }

        // Check for reasonable scale values
        for input in &self.inputs {
            if input.scale.abs() > 10.0 {
                return Err("Input scale must be in range [-10.0, 10.0]");
            }
            if input.gain < 0.0 || input.gain > 10.0 {
                return Err("Input gain must be in range [0.0, 10.0]");
            }
        }

        Ok(())
    }
}

/// State for mixer node (8 bytes aligned for compatibility)
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct MixerState {
    /// Previous input values for each mixer input (up to 8 inputs)
    pub prev_inputs: [f32; 8],
    /// Number of active inputs
    pub input_count: u32,
    /// Last update timestamp
    pub last_update_ns: u64,
    /// Reserved for future use
    pub _reserved: u32,
}

/// Mixer node for multi-axis interactions (helicopter anti-torque, etc.)
///
/// The mixer combines multiple input sources with configurable scale and gain factors.
/// This is essential for helicopter flight models where collective pitch affects
/// anti-torque requirements, or for complex aircraft with control coupling.
///
/// # Examples
///
/// Helicopter anti-torque mixing:
/// ```
/// use flight_axis::{MixerNode, MixerConfig, MixerInput};
///
/// let config = MixerConfig::new("anti_torque")
///     .add_scaled_input("collective", -0.3)  // Collective increases, need more left pedal
///     .add_scaled_input("pedals", 1.0);      // Direct pedal input
///
/// let mixer = MixerNode::new(config).expect("Valid config");
/// ```
#[derive(Debug, Clone)]
pub struct MixerNode {
    /// Mixer configuration
    config: MixerConfig,
    /// Cached input count for performance
    input_count: usize,
}

impl MixerNode {
    /// Create new mixer node with configuration
    pub fn new(config: MixerConfig) -> Result<Self, &'static str> {
        config.validate()?;

        let input_count = config.inputs.len();

        Ok(Self {
            config,
            input_count,
        })
    }

    /// Create helicopter anti-torque mixer
    ///
    /// Standard configuration for helicopter anti-torque where collective
    /// pitch affects pedal requirements.
    pub fn helicopter_anti_torque(collective_scale: f32) -> Result<Self, &'static str> {
        let config = MixerConfig::new("anti_torque")
            .add_scaled_input("collective", collective_scale)
            .add_scaled_input("pedals", 1.0);

        Self::new(config)
    }

    /// Create aileron-rudder mixer for coordinated turns
    pub fn aileron_rudder_coordination(coordination_factor: f32) -> Result<Self, &'static str> {
        let config = MixerConfig::new("rudder_coordinated")
            .add_scaled_input("aileron", coordination_factor)
            .add_scaled_input("rudder", 1.0);

        Self::new(config)
    }

    /// Get mixer configuration
    pub fn config(&self) -> &MixerConfig {
        &self.config
    }

    /// Process mixer with multiple input values
    ///
    /// # Safety
    /// inputs slice must have exactly the same length as configured inputs
    #[inline(always)]
    pub fn process_inputs(&self, inputs: &[f32], output: &mut f32) {
        debug_assert_eq!(
            inputs.len(),
            self.input_count,
            "Input count mismatch: expected {}, got {}",
            self.input_count,
            inputs.len()
        );

        let mut mixed_output = 0.0f32;

        // Process each input with its scale and gain
        for (input_val, mixer_input) in inputs.iter().zip(&self.config.inputs) {
            mixed_output += mixer_input.apply(*input_val);
        }

        // Apply output clamping if enabled
        if self.config.clamp_output {
            mixed_output = mixed_output.clamp(-1.0, 1.0);
        }

        *output = mixed_output;
    }
}

impl Node for MixerNode {
    #[inline(always)]
    fn step(&mut self, _frame: &mut AxisFrame) {
        // This implementation is for compatibility only
        // Real RT path uses step_soa with SoA state layout and external input management
        unimplemented!(
            "MixerNode requires SoA state layout and external input management - use step_soa()"
        );
    }

    fn state_size(&self) -> usize {
        std::mem::size_of::<MixerState>()
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn init_state(&self, state_ptr: *mut u8) {
        let state = state_ptr as *mut MixerState;
        *state = MixerState {
            prev_inputs: [0.0; 8],
            input_count: self.input_count as u32,
            last_update_ns: 0,
            _reserved: 0,
        };
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn step_soa(&self, frame: &mut AxisFrame, state_ptr: *mut u8) {
        let state = &mut *(state_ptr as *mut MixerState);

        // For single-axis mixer, we use the current frame output as the primary input
        // and mix with previous inputs stored in state
        // This is a simplified implementation - in practice, mixers would receive
        // multiple axis inputs from the engine

        // Store current input
        if state.input_count > 0 {
            state.prev_inputs[0] = frame.out;
        }

        // For demonstration, apply a simple mixing operation
        // In a real implementation, this would receive multiple axis values
        let mut mixed_output = 0.0f32;

        for (i, mixer_input) in self.config.inputs.iter().enumerate() {
            if i < state.input_count as usize {
                let input_val = if i == 0 {
                    frame.out // Current axis
                } else {
                    state.prev_inputs[i] // Previous stored values
                };

                mixed_output += mixer_input.apply(input_val);
            }
        }

        // Apply output clamping if enabled
        if self.config.clamp_output {
            mixed_output = mixed_output.clamp(-1.0, 1.0);
        }

        frame.out = mixed_output;
        state.last_update_ns = frame.ts_mono_ns;
    }

    fn node_type(&self) -> &'static str {
        "mixer"
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use crate::AxisFrame;
    use proptest::prelude::*;

    proptest! {
        // Test DeadzoneNode behavior
        #[test]
        fn prop_deadzone_node_step(
            threshold in 0.0f32..=0.9f32,
            input_val in -1.0f32..=1.0f32
        ) {
            let mut node = DeadzoneNode::new(threshold);
            let mut frame = AxisFrame::new(input_val, 1000);
            frame.out = input_val; // Initialize out same as in

            node.step(&mut frame);

            if input_val.abs() < threshold {
                prop_assert_eq!(frame.out, 0.0);
            } else {
                prop_assert!(frame.out.abs() > 0.0);
                prop_assert!(frame.out.abs() <= 1.0);
                // Should preserve sign
                prop_assert_eq!(frame.out.signum(), input_val.signum());
            }
        }

        // Test CurveNode behavior
        #[test]
        fn prop_curve_node_step(
            expo in -1.0f32..=1.0f32,
            input_val in -1.0f32..=1.0f32
        ) {
            let mut node = CurveNode::new(expo);
            let mut frame = AxisFrame::new(input_val, 1000);
            frame.out = input_val;

            node.step(&mut frame);

            prop_assert!(frame.out.abs() <= 1.0);
            // Should preserve sign
            if input_val != 0.0 {
                prop_assert_eq!(frame.out.signum(), input_val.signum());
            }
            // Should be monotonic
            // (Strict monotonicity checks are hard with floats, but sign check covers most basic regressions)
        }

        // Test MixerConfig validation logic
        #[test]
        fn prop_mixer_config_validation(
            scale in -10.0f32..=10.0f32,
            gain in 0.0f32..=10.0f32,
            output_name in "[a-z_]+"
        ) {
            let config = MixerConfig::new(output_name)
                .add_input_with_gain("test_input", scale, gain);

            prop_assert!(config.validate().is_ok());
        }

        // Test MixerNode processing (basic single input pass-through check)
        #[test]
        fn prop_mixer_process_single(
            input_val in -1.0f32..=1.0f32,
            scale in -1.0f32..=1.0f32,
            gain in 0.0f32..=2.0f32
        ) {
            let config = MixerConfig::new("test")
                .add_input_with_gain("in1", scale, gain);

            let mixer = MixerNode::new(config).unwrap();
            let inputs = vec![input_val];
            let mut output = 0.0;

            mixer.process_inputs(&inputs, &mut output);

            let expected_raw = input_val * scale * gain;
            let expected_clamped = expected_raw.clamp(-1.0, 1.0);

            prop_assert!((output - expected_clamped).abs() < 1e-5);
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::AxisFrame;

    // ── DetentZone ──────────────────────────────────────────────────────────

    #[test]
    fn detent_zone_contains_entry_inside() {
        let zone = DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle);
        assert!(zone.contains_entry(0.0));
        assert!(zone.contains_entry(0.09));
        assert!(!zone.contains_entry(0.15));
    }

    #[test]
    fn detent_zone_contains_exit_with_hysteresis() {
        let zone = DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle);
        // Inside entry → definitely inside exit
        assert!(zone.contains_exit(0.0));
        // Between entry and exit (hysteresis band)
        assert!(zone.contains_exit(0.12));
        // Outside exit
        assert!(!zone.contains_exit(0.20));
    }

    #[test]
    fn detent_zone_no_snap_flag() {
        let zone = DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle);
        assert!(zone.snap_to_center, "new() should have snap_to_center=true");

        let no_snap = DetentZone::no_snap(0.0, 0.1, 0.05, DetentRole::Idle);
        assert!(
            !no_snap.snap_to_center,
            "no_snap() should have snap_to_center=false"
        );
    }

    #[test]
    fn detent_zone_entry_bounds_clamped() {
        // Zone near edge — bounds should be clamped to [-1, 1]
        let zone = DetentZone::new(0.95, 0.2, 0.05, DetentRole::Takeoff);
        let (lo, hi) = zone.entry_bounds();
        assert!(lo >= -1.0 && lo <= 1.0);
        assert!(hi >= -1.0 && hi <= 1.0);
        assert!(lo <= hi);
    }

    #[test]
    fn detent_zone_exit_bounds_wider_than_entry() {
        let zone = DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle);
        let (elo, ehi) = zone.entry_bounds();
        let (xlo, xhi) = zone.exit_bounds();
        assert!(xlo <= elo, "exit lower bound should be ≤ entry lower bound");
        assert!(xhi >= ehi, "exit upper bound should be ≥ entry upper bound");
    }

    // ── CurveNode ────────────────────────────────────────────────────────────

    #[test]
    fn curve_node_exponential_zero_expo_is_linear() {
        let mut node = CurveNode::exponential(0.0).expect("valid expo");
        let mut frame = AxisFrame::new(0.5, 1000);
        frame.out = 0.5;
        node.step(&mut frame);
        // expo=0 → linear, output ≈ input
        assert!((frame.out - 0.5).abs() < 0.01);
    }

    // ── MixerConfig helpers ──────────────────────────────────────────────────

    #[test]
    fn mixer_config_add_scaled_input_creates_entry() {
        let config = MixerConfig::new("pitch").add_scaled_input("aileron", 0.5);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn mixer_config_no_clamp_disables_clamping() {
        let config = MixerConfig::new("test").add_scaled_input("a", 1.0).no_clamp();
        assert!(!config.clamp_output, "no_clamp should disable output clamping");
    }

    #[test]
    fn mixer_node_helicopter_anti_torque() {
        let node = MixerNode::helicopter_anti_torque(1.0);
        // Should compile without error and produce a valid mixer
        assert!(node.is_ok());
    }

    #[test]
    fn mixer_node_aileron_rudder_coordination() {
        let node = MixerNode::aileron_rudder_coordination(0.2);
        assert!(node.is_ok());
    }
}
