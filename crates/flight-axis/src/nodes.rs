//! Pipeline processing nodes

use crate::AxisFrame;

/// Pipeline node trait for zero-allocation processing
pub trait Node {
    /// Process axis frame in-place
    fn step(&mut self, frame: &mut AxisFrame);
}

/// Deadzone processing node
#[derive(Debug, Clone)]
pub struct DeadzoneNode {
    pub threshold: f32,
}

impl Node for DeadzoneNode {
    #[inline(always)]
    fn step(&mut self, frame: &mut AxisFrame) {
        if frame.out.abs() < self.threshold {
            frame.out = 0.0;
        } else {
            let sign = frame.out.signum();
            frame.out = sign * ((frame.out.abs() - self.threshold) / (1.0 - self.threshold));
        }
    }
}

/// Exponential curve node
#[derive(Debug, Clone)]
pub struct CurveNode {
    pub expo: f32,
}

impl Node for CurveNode {
    #[inline(always)]
    fn step(&mut self, frame: &mut AxisFrame) {
        let sign = frame.out.signum();
        let abs_val = frame.out.abs();
        frame.out = sign * (abs_val.powf(1.0 + self.expo));
    }
}

/// Slew rate limiter node
#[derive(Debug, Clone)]
pub struct SlewNode {
    pub rate_limit: f32, // units per second
    last_output: f32,
    last_time_ns: u64,
}

impl SlewNode {
    pub fn new(rate_limit: f32) -> Self {
        Self {
            rate_limit,
            last_output: 0.0,
            last_time_ns: 0,
        }
    }
}

impl Node for SlewNode {
    #[inline(always)]
    fn step(&mut self, frame: &mut AxisFrame) {
        if self.last_time_ns == 0 {
            self.last_output = frame.out;
            self.last_time_ns = frame.ts_mono_ns;
            return;
        }

        let dt_s = (frame.ts_mono_ns - self.last_time_ns) as f32 / 1_000_000_000.0;
        let max_change = self.rate_limit * dt_s;
        let desired_change = frame.out - self.last_output;

        if desired_change.abs() > max_change {
            frame.out = self.last_output + desired_change.signum() * max_change;
        }

        self.last_output = frame.out;
        self.last_time_ns = frame.ts_mono_ns;
    }
}
