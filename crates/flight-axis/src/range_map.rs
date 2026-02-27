// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Output range mapping for axis values.
//!
//! Maps processed axis values from `[-1.0, 1.0]` to a custom `[out_min, out_max]` range
//! using linear interpolation. Input is clamped before mapping.

use thiserror::Error;

/// Errors returned by [`RangeMapper::new`] and [`RangeMapConfig::validate`].
#[derive(Debug, Error, PartialEq)]
pub enum RangeMapError {
    /// `out_min` or `out_max` is not finite (NaN or infinite).
    #[error("out_min and out_max must be finite")]
    NonFiniteRange,
}

/// Configuration for output range mapping.
///
/// Maps the standard axis range `[-1.0, 1.0]` to `[out_min, out_max]`.
/// `out_min` and `out_max` may be equal (constant output).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeMapConfig {
    /// Lower bound of the output range.
    pub out_min: f32,
    /// Upper bound of the output range.
    pub out_max: f32,
}

impl Default for RangeMapConfig {
    fn default() -> Self {
        Self {
            out_min: -1.0,
            out_max: 1.0,
        }
    }
}

impl RangeMapConfig {
    /// Validates that both `out_min` and `out_max` are finite.
    ///
    /// # Errors
    ///
    /// Returns [`RangeMapError::NonFiniteRange`] if either value is NaN or infinite.
    pub fn validate(&self) -> Result<(), RangeMapError> {
        if !self.out_min.is_finite() || !self.out_max.is_finite() {
            return Err(RangeMapError::NonFiniteRange);
        }
        Ok(())
    }

    /// Returns `true` if this config is the identity mapping `[-1.0, 1.0]`.
    pub fn is_identity(&self) -> bool {
        self.out_min == -1.0 && self.out_max == 1.0
    }
}

/// Maps axis values from `[-1.0, 1.0]` to a custom output range.
#[derive(Debug, PartialEq)]
pub struct RangeMapper {
    config: RangeMapConfig,
}

impl RangeMapper {
    /// Creates a new `RangeMapper` with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns [`RangeMapError::NonFiniteRange`] if either bound is NaN or infinite.
    pub fn new(config: RangeMapConfig) -> Result<Self, RangeMapError> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Maps `input` from `[-1.0, 1.0]` to `[out_min, out_max]`.
    ///
    /// Input is clamped to `[-1.0, 1.0]` before mapping.
    #[inline]
    pub fn map(&self, input: f32) -> f32 {
        let clamped = input.clamp(-1.0, 1.0);
        self.config.out_min + (clamped + 1.0) / 2.0 * (self.config.out_max - self.config.out_min)
    }
}

/// Fixed-size bank of N range mappers, one per axis.
///
/// All axes in the bank share the same `RangeMapConfig`.
pub struct RangeMapBank<const N: usize> {
    mappers: [RangeMapper; N],
}

impl<const N: usize> RangeMapBank<N> {
    /// Creates a bank of `N` mappers all using the given configuration.
    ///
    /// # Errors
    ///
    /// Returns [`RangeMapError::NonFiniteRange`] if the config is invalid.
    pub fn new(config: RangeMapConfig) -> Result<Self, RangeMapError> {
        config.validate()?;
        Ok(Self {
            mappers: std::array::from_fn(|_| RangeMapper { config }),
        })
    }

    /// Applies range mapping to all `N` inputs, writing results into `outputs`.
    #[inline]
    pub fn process(&self, inputs: &[f32; N], outputs: &mut [f32; N]) {
        for i in 0..N {
            outputs[i] = self.mappers[i].map(inputs[i]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_range_no_change() {
        let mapper = RangeMapper::new(RangeMapConfig::default()).unwrap();
        assert!((mapper.map(0.5) - 0.5).abs() < 1e-6);
        assert!((mapper.map(-0.5) - (-0.5)).abs() < 1e-6);
        assert!((mapper.map(0.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_map_minus_one_to_out_min() {
        let config = RangeMapConfig {
            out_min: -2.0,
            out_max: 4.0,
        };
        let mapper = RangeMapper::new(config).unwrap();
        assert!((mapper.map(-1.0) - (-2.0)).abs() < 1e-6);
    }

    #[test]
    fn test_map_plus_one_to_out_max() {
        let config = RangeMapConfig {
            out_min: -2.0,
            out_max: 4.0,
        };
        let mapper = RangeMapper::new(config).unwrap();
        assert!((mapper.map(1.0) - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_map_zero_to_midpoint() {
        let config = RangeMapConfig {
            out_min: 0.0,
            out_max: 100.0,
        };
        let mapper = RangeMapper::new(config).unwrap();
        assert!((mapper.map(0.0) - 50.0).abs() < 1e-4);
    }

    #[test]
    fn test_map_positive_range_0_to_1() {
        let config = RangeMapConfig {
            out_min: 0.0,
            out_max: 1.0,
        };
        let mapper = RangeMapper::new(config).unwrap();
        assert!((mapper.map(-1.0) - 0.0).abs() < 1e-6);
        assert!((mapper.map(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_input_clamped_before_mapping() {
        let config = RangeMapConfig {
            out_min: 0.0,
            out_max: 1.0,
        };
        let mapper = RangeMapper::new(config).unwrap();
        let at_one = mapper.map(1.0);
        let over_one = mapper.map(2.0);
        assert!((at_one - over_one).abs() < 1e-6);
    }

    #[test]
    fn test_equal_out_min_out_max() {
        let config = RangeMapConfig {
            out_min: 0.5,
            out_max: 0.5,
        };
        let mapper = RangeMapper::new(config).unwrap();
        assert!((mapper.map(-1.0) - 0.5).abs() < 1e-6);
        assert!((mapper.map(0.0) - 0.5).abs() < 1e-6);
        assert!((mapper.map(1.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_nan_out_min_invalid() {
        let config = RangeMapConfig {
            out_min: f32::NAN,
            out_max: 1.0,
        };
        assert_eq!(config.validate(), Err(RangeMapError::NonFiniteRange));
    }

    #[test]
    fn test_inf_out_max_invalid() {
        let config = RangeMapConfig {
            out_min: 0.0,
            out_max: f32::INFINITY,
        };
        assert_eq!(RangeMapper::new(config), Err(RangeMapError::NonFiniteRange));
    }

    #[test]
    fn test_bank_maps_multiple_axes() {
        let config = RangeMapConfig {
            out_min: 0.0,
            out_max: 1.0,
        };
        let bank = RangeMapBank::<3>::new(config).unwrap();
        let inputs = [-1.0f32, 0.0, 1.0];
        let mut outputs = [0.0f32; 3];
        bank.process(&inputs, &mut outputs);
        assert!((outputs[0] - 0.0).abs() < 1e-6);
        assert!((outputs[1] - 0.5).abs() < 1e-6);
        assert!((outputs[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_is_identity_for_default_config() {
        let config = RangeMapConfig::default();
        assert!(config.is_identity());
        let non_identity = RangeMapConfig {
            out_min: 0.0,
            out_max: 1.0,
        };
        assert!(!non_identity.is_identity());
    }
}
