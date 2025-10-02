//! Axis frame data structure

/// Real-time axis processing frame with explicit units
#[repr(C)]
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct AxisFrame {
    /// Raw input value [-1.0, 1.0] (normalized units)
    pub in_raw: f32,
    /// Processed output value [-1.0, 1.0] (normalized units)
    pub out: f32,
    /// Input derivative (normalized units per second)
    pub d_in_dt: f32,
    /// Monotonic timestamp in nanoseconds (CLOCK_MONOTONIC)
    pub ts_mono_ns: u64,
}

impl AxisFrame {
    /// Create new axis frame with explicit units
    pub fn new(in_raw: f32, ts_mono_ns: u64) -> Self {
        Self {
            in_raw,
            out: in_raw,
            d_in_dt: 0.0,
            ts_mono_ns,
        }
    }

    /// Reset frame for new input while preserving timestamp
    #[inline(always)]
    pub fn reset_with_input(&mut self, in_raw: f32) {
        self.in_raw = in_raw;
        self.out = in_raw;
        self.d_in_dt = 0.0;
    }

    /// Calculate derivative from previous frame
    #[inline(always)]
    pub fn update_derivative(&mut self, prev_frame: &AxisFrame) {
        if prev_frame.ts_mono_ns > 0 && self.ts_mono_ns > prev_frame.ts_mono_ns {
            let dt_s = (self.ts_mono_ns - prev_frame.ts_mono_ns) as f32 / 1_000_000_000.0;
            self.d_in_dt = (self.in_raw - prev_frame.in_raw) / dt_s;
        }
    }
}
