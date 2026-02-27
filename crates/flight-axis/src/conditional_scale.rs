// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Conditional axis scaling (REQ-809).
//!
//! Applies a scale factor chosen by the first matching [`ScaleCondition`].
//! Conditions are evaluated in order from a fixed-size array so the hot path
//! is allocation-free.
//!
//! RT-safe: no heap allocation.

/// Maximum number of conditions stored in a [`ConditionalScale`].
pub const MAX_CONDITIONS: usize = 8;

/// A condition that determines when a scale factor applies.
///
/// RT-safe: no heap allocation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScaleCondition {
    /// Matches when the absolute input value is within `[min, max]`.
    InputRange {
        /// Lower bound (inclusive).
        min: f64,
        /// Upper bound (inclusive).
        max: f64,
        /// Scale factor to apply when matched.
        factor: f64,
    },
    /// Always matches.  Typically used as a fallback at the end of the list.
    Always {
        /// Scale factor to apply.
        factor: f64,
    },
}

impl ScaleCondition {
    /// Returns `true` if the condition matches `value`.
    #[inline]
    #[must_use]
    fn matches(&self, value: f64) -> bool {
        match self {
            ScaleCondition::InputRange { min, max, .. } => {
                let abs = value.abs();
                abs >= *min && abs <= *max
            }
            ScaleCondition::Always { .. } => true,
        }
    }

    /// Returns the scale factor carried by this condition.
    #[inline]
    #[must_use]
    const fn factor(&self) -> f64 {
        match self {
            ScaleCondition::InputRange { factor, .. } | ScaleCondition::Always { factor } => {
                *factor
            }
        }
    }
}

/// Conditional scaler backed by a fixed-size condition array.
///
/// During [`apply`](Self::apply), conditions are evaluated in order and the
/// first match determines the output scale factor.  If no condition matches
/// the input passes through unscaled.
///
/// RT-safe: no heap allocation.
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionalScale {
    conditions: [Option<ScaleCondition>; MAX_CONDITIONS],
    len: usize,
}

impl Default for ConditionalScale {
    fn default() -> Self {
        Self::new()
    }
}

impl ConditionalScale {
    /// Creates an empty `ConditionalScale` (no conditions → passthrough).
    ///
    /// RT-safe: no heap allocation.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            conditions: [None; MAX_CONDITIONS],
            len: 0,
        }
    }

    /// Appends a condition.  Returns `false` if the array is full.
    pub fn add_condition(&mut self, cond: ScaleCondition) -> bool {
        if self.len >= MAX_CONDITIONS {
            return false;
        }
        self.conditions[self.len] = Some(cond);
        self.len += 1;
        true
    }

    /// Returns the number of conditions currently stored.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if no conditions are stored.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Applies the first matching condition's scale factor to `value`.
    ///
    /// If no condition matches, `value` is returned unchanged.
    ///
    /// RT-safe: no heap allocation.
    #[inline]
    #[must_use]
    pub fn apply(&self, value: f64) -> f64 {
        for slot in self.conditions[..self.len].iter().flatten() {
            if slot.matches(value) {
                return value * slot.factor();
            }
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_passthrough() {
        let cs = ConditionalScale::new();
        assert!(cs.is_empty());
        assert!((cs.apply(0.7) - 0.7).abs() < 1e-12);
    }

    #[test]
    fn always_condition_scales() {
        let mut cs = ConditionalScale::new();
        cs.add_condition(ScaleCondition::Always { factor: 0.5 });
        assert!((cs.apply(1.0) - 0.5).abs() < 1e-12);
        assert!((cs.apply(-0.8) - (-0.4)).abs() < 1e-12);
    }

    #[test]
    fn input_range_matches_absolute() {
        let mut cs = ConditionalScale::new();
        cs.add_condition(ScaleCondition::InputRange {
            min: 0.0,
            max: 0.5,
            factor: 2.0,
        });
        // |0.3| in [0, 0.5] → matched
        assert!((cs.apply(0.3) - 0.6).abs() < 1e-12);
        // |-0.4| in [0, 0.5] → matched
        assert!((cs.apply(-0.4) - (-0.8)).abs() < 1e-12);
        // |0.8| not in [0, 0.5] → passthrough
        assert!((cs.apply(0.8) - 0.8).abs() < 1e-12);
    }

    #[test]
    fn first_match_wins() {
        let mut cs = ConditionalScale::new();
        cs.add_condition(ScaleCondition::InputRange {
            min: 0.0,
            max: 0.3,
            factor: 3.0,
        });
        cs.add_condition(ScaleCondition::Always { factor: 0.5 });
        // |0.2| in [0, 0.3] → first condition wins (factor 3.0)
        assert!((cs.apply(0.2) - 0.6).abs() < 1e-12);
        // |0.5| not in [0, 0.3] → falls through to Always (factor 0.5)
        assert!((cs.apply(0.5) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn add_beyond_capacity_returns_false() {
        let mut cs = ConditionalScale::new();
        for _ in 0..MAX_CONDITIONS {
            assert!(cs.add_condition(ScaleCondition::Always { factor: 1.0 }));
        }
        assert!(!cs.add_condition(ScaleCondition::Always { factor: 1.0 }));
        assert_eq!(cs.len(), MAX_CONDITIONS);
    }

    #[test]
    fn zero_input_stays_zero() {
        let mut cs = ConditionalScale::new();
        cs.add_condition(ScaleCondition::Always { factor: 5.0 });
        assert_eq!(cs.apply(0.0), 0.0);
    }

    #[test]
    fn boundary_values_of_range() {
        let mut cs = ConditionalScale::new();
        cs.add_condition(ScaleCondition::InputRange {
            min: 0.5,
            max: 0.5,
            factor: 10.0,
        });
        // |0.5| == 0.5 → matches
        assert!((cs.apply(0.5) - 5.0).abs() < 1e-12);
        assert!((cs.apply(-0.5) - (-5.0)).abs() < 1e-12);
        // |0.49| < 0.5 → no match
        assert!((cs.apply(0.49) - 0.49).abs() < 1e-12);
    }
}
