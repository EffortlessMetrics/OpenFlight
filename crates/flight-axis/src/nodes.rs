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
struct SlewState {
    last_output: f32,
    last_time_ns: u64,
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

    unsafe fn init_state(&self, state_ptr: *mut u8) {
        let state = state_ptr as *mut SlewState;
        *state = SlewState {
            last_output: 0.0,
            last_time_ns: 0,
        };
    }

    unsafe fn step_soa(&self, frame: &mut AxisFrame, state_ptr: *mut u8) {
        let state = &mut *(state_ptr as *mut SlewState);

        if state.last_time_ns == 0 {
            state.last_output = frame.out;
            state.last_time_ns = frame.ts_mono_ns;
            return;
        }

        let dt_s = (frame.ts_mono_ns - state.last_time_ns) as f32 / 1_000_000_000.0;
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
