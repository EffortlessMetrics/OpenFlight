//! Axis frame data structure

/// Real-time axis processing frame
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AxisFrame {
    /// Raw input value [-1.0, 1.0]
    pub in_raw: f32,
    /// Processed output value [-1.0, 1.0]
    pub out: f32,
    /// Input derivative (units per second)
    pub d_in_dt: f32,
    /// Monotonic timestamp in nanoseconds
    pub ts_mono_ns: u64,
}

impl AxisFrame {
    /// Create new axis frame
    pub fn new(in_raw: f32, ts_mono_ns: u64) -> Self {
        Self {
            in_raw,
            out: in_raw,
            d_in_dt: 0.0,
            ts_mono_ns,
        }
    }
}
