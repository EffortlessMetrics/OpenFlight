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
        0 // Stateless node
    }

    unsafe fn init_state(&self, _state_ptr: *mut u8) {
        // No state to initialize
    }

    unsafe fn step_soa(&self, frame: &mut AxisFrame, _state_ptr: *mut u8) {
        // Delegate to regular step for stateless nodes
        let mut node = self.clone();
        node.step(frame);
    }

    fn node_type(&self) -> &'static str {
        "deadzone"
    }
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
            expo >= -1.0 && expo <= 1.0,
            "Exponential factor must be in range [-1.0, 1.0], got {}",
            expo
        );
        Self { expo }
    }

    /// Create exponential curve with validation
    pub fn exponential(expo: f32) -> Result<Self, &'static str> {
        if expo < -1.0 || expo > 1.0 {
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
        0 // Stateless node
    }

    unsafe fn init_state(&self, _state_ptr: *mut u8) {
        // No state to initialize
    }

    unsafe fn step_soa(&self, frame: &mut AxisFrame, _state_ptr: *mut u8) {
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

/// State for slew rate limiter (8 bytes aligned)
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct SlewState {
    pub last_output: f32,
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
        std::mem::size_of::<SlewState>()
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn init_state(&self, state_ptr: *mut u8) {
        let state = state_ptr as *mut SlewState;
        *state = SlewState {
            last_output: 0.0,
            last_time_ns: 0,
        };
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn step_soa(&self, frame: &mut AxisFrame, state_ptr: *mut u8) {
        let state = &mut *(state_ptr as *mut SlewState);

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
        
        let rate = if desired_change > 0.0 {
            self.attack_rate.unwrap_or(self.rate_limit)
        } else {
            self.rate_limit
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
        if new_detent_idx == u32::MAX {
            if let Some(idx) = self.find_entry_detent(position) {
                new_detent_idx = idx as u32;
            }
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
        debug_assert_eq!(inputs.len(), self.input_count, 
                        "Input count mismatch: expected {}, got {}", 
                        self.input_count, inputs.len());

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
        unimplemented!("MixerNode requires SoA state layout and external input management - use step_soa()");
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
