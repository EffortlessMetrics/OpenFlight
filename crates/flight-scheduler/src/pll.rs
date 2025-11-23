// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Phase-Locked Loop for scheduler timing correction
//!
//! Implements a simple PLL to correct for systematic timing drift
//! and maintain phase lock with the target frequency.

/// Phase-Locked Loop for timing correction
#[derive(Debug, Clone)]
pub struct Pll {
    /// PLL gain (typically 0.001 for 0.1%/s correction)
    gain: f64,
    /// Nominal period in nanoseconds
    nominal_period_ns: f64,
    /// Current corrected period
    corrected_period_ns: f64,
    /// Accumulated phase error
    phase_error: f64,
}

impl Pll {
    /// Create new PLL
    pub fn new(gain: f64, nominal_period_ns: f64) -> Self {
        Self {
            gain,
            nominal_period_ns,
            corrected_period_ns: nominal_period_ns,
            phase_error: 0.0,
        }
    }

    /// Update PLL with timing error and return corrected period
    pub fn update(&mut self, error_ns: f64) -> f64 {
        // Accumulate phase error
        self.phase_error += error_ns;

        // Apply proportional correction to period
        let correction = -self.gain * self.phase_error;
        self.corrected_period_ns = self.nominal_period_ns + correction;

        // Clamp correction to reasonable bounds (±1%)
        let max_correction = self.nominal_period_ns * 0.01;
        self.corrected_period_ns = self
            .corrected_period_ns
            .max(self.nominal_period_ns - max_correction)
            .min(self.nominal_period_ns + max_correction);

        self.corrected_period_ns
    }

    /// Get current phase error
    pub fn phase_error(&self) -> f64 {
        self.phase_error
    }

    /// Get current period correction
    pub fn period_correction(&self) -> f64 {
        self.corrected_period_ns - self.nominal_period_ns
    }

    /// Reset PLL state
    pub fn reset(&mut self) {
        self.phase_error = 0.0;
        self.corrected_period_ns = self.nominal_period_ns;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pll_correction() {
        let mut pll = Pll::new(0.001, 4_000_000.0); // 250Hz = 4ms period

        // Simulate consistent late timing
        for _ in 0..100 {
            let corrected = pll.update(1000.0); // 1μs late each time
            // Should gradually reduce period to compensate
        }

        // Should have negative correction (shorter period)
        assert!(pll.period_correction() < 0.0);

        // Should be bounded
        assert!(pll.period_correction().abs() < 40_000.0); // <1% of 4ms
    }

    #[test]
    fn test_pll_bounds() {
        let mut pll = Pll::new(0.1, 4_000_000.0); // High gain for testing

        // Large error should be clamped
        let corrected = pll.update(1_000_000.0); // 1ms error

        // Should be clamped to ±1%
        assert!(pll.period_correction().abs() <= 40_000.0);
    }
}
