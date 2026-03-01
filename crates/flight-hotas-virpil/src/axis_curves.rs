// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Virpil-specific axis response curve reading and application.
//!
//! VIRPIL VPC devices allow firmware-level response curves to be configured
//! via the VPC Configuration Tool. These curves are defined by a set of control
//! points with linear interpolation between them.
//!
//! This module provides:
//!
//! - [`AxisCurve`] — a response curve defined by up to 11 control points
//! - [`read_axis_curve`] — parse a curve definition from a raw byte buffer
//! - [`apply_curve`] — apply a curve to a normalised `[0.0, 1.0]` input value
//!
//! # Wire format
//!
//! A curve definition is stored as a sequence of `(x, y)` pairs encoded as
//! unsigned bytes, where each value represents a percentage (0–100). The
//! encoding is:
//!
//! ```text
//! byte 0    : number of control points (N, 2..=11)
//! bytes 1..=2N : N × (x_percent: u8, y_percent: u8) pairs
//! ```
//!
//! Control points must be sorted by ascending X value. Duplicate X values
//! are rejected.

use thiserror::Error;

/// Maximum number of control points in an axis curve.
pub const MAX_CONTROL_POINTS: usize = 11;

/// Minimum number of control points (must have at least start and end).
pub const MIN_CONTROL_POINTS: usize = 2;

/// A single control point in a response curve.
///
/// Both `x` and `y` are normalised to the `[0.0, 1.0]` range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPoint {
    /// Input value (normalised 0.0–1.0).
    pub x: f64,
    /// Output value (normalised 0.0–1.0).
    pub y: f64,
}

impl ControlPoint {
    /// Create a new control point from normalised values.
    ///
    /// Values are clamped to `[0.0, 1.0]`.
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0),
        }
    }
}

/// Error returned when parsing or validating an axis curve.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AxisCurveError {
    #[error("axis curve data too short: got {0} bytes, need at least 1 + 2×N")]
    TooShort(usize),
    #[error("invalid point count {0} (must be {MIN_CONTROL_POINTS}..={MAX_CONTROL_POINTS})")]
    InvalidPointCount(u8),
    #[error("control points not sorted by X: point {index} has x={x}, previous x={prev_x}")]
    NotSorted { index: usize, x: u8, prev_x: u8 },
    #[error("duplicate X value {x} at index {index}")]
    DuplicateX { index: usize, x: u8 },
}

/// A response curve for a VIRPIL axis, defined by control points with
/// linear interpolation.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisCurve {
    points: Vec<ControlPoint>,
}

impl AxisCurve {
    /// Create a linear (identity) curve: output equals input.
    pub fn linear() -> Self {
        Self {
            points: vec![ControlPoint::new(0.0, 0.0), ControlPoint::new(1.0, 1.0)],
        }
    }

    /// Create a curve from a sorted slice of control points.
    ///
    /// The points must be sorted by ascending X. At least [`MIN_CONTROL_POINTS`]
    /// and at most [`MAX_CONTROL_POINTS`] points are required.
    pub fn from_points(points: &[ControlPoint]) -> Result<Self, AxisCurveError> {
        let n = points.len();
        if !(MIN_CONTROL_POINTS..=MAX_CONTROL_POINTS).contains(&n) {
            return Err(AxisCurveError::InvalidPointCount(n as u8));
        }
        for i in 1..n {
            if points[i].x < points[i - 1].x {
                // Use approximate integer representation for error message
                let x = (points[i].x * 100.0).round() as u8;
                let prev_x = (points[i - 1].x * 100.0).round() as u8;
                return Err(AxisCurveError::NotSorted {
                    index: i,
                    x,
                    prev_x,
                });
            }
            if (points[i].x - points[i - 1].x).abs() < 1e-9 {
                let x = (points[i].x * 100.0).round() as u8;
                return Err(AxisCurveError::DuplicateX { index: i, x });
            }
        }
        Ok(Self {
            points: points.to_vec(),
        })
    }

    /// Return the control points of this curve.
    pub fn points(&self) -> &[ControlPoint] {
        &self.points
    }

    /// Number of control points.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether the curve has no points (always false for valid curves).
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// Parse an axis curve from a raw byte buffer.
///
/// See the [module documentation](self) for the wire format.
pub fn read_axis_curve(data: &[u8]) -> Result<AxisCurve, AxisCurveError> {
    if data.is_empty() {
        return Err(AxisCurveError::TooShort(0));
    }

    let point_count = data[0] as usize;
    if !(MIN_CONTROL_POINTS..=MAX_CONTROL_POINTS).contains(&point_count) {
        return Err(AxisCurveError::InvalidPointCount(data[0]));
    }

    let needed = 1 + point_count * 2;
    if data.len() < needed {
        return Err(AxisCurveError::TooShort(data.len()));
    }

    let mut points = Vec::with_capacity(point_count);
    for i in 0..point_count {
        let x_pct = data[1 + i * 2];
        let y_pct = data[2 + i * 2];
        points.push(ControlPoint::new(x_pct as f64 / 100.0, y_pct as f64 / 100.0));
    }

    // Validate sorted order
    for i in 1..points.len() {
        let x_raw = data[1 + i * 2];
        let prev_raw = data[1 + (i - 1) * 2];
        if x_raw < prev_raw {
            return Err(AxisCurveError::NotSorted {
                index: i,
                x: x_raw,
                prev_x: prev_raw,
            });
        }
        if x_raw == prev_raw {
            return Err(AxisCurveError::DuplicateX { index: i, x: x_raw });
        }
    }

    Ok(AxisCurve { points })
}

/// Apply a response curve to a normalised input value.
///
/// The input is clamped to `[0.0, 1.0]`. Linear interpolation is used between
/// control points. Values below the first control point or above the last are
/// extrapolated from the nearest segment (but the result is still clamped to
/// `[0.0, 1.0]`).
pub fn apply_curve(input: f64, curve: &AxisCurve) -> f64 {
    let input = input.clamp(0.0, 1.0);
    let points = &curve.points;

    if points.is_empty() {
        return input;
    }
    if points.len() == 1 {
        return points[0].y;
    }

    // Below the first point
    if input <= points[0].x {
        return points[0].y;
    }
    // Above the last point
    if input >= points[points.len() - 1].x {
        return points[points.len() - 1].y;
    }

    // Find the segment containing the input
    for i in 1..points.len() {
        if input <= points[i].x {
            let p0 = &points[i - 1];
            let p1 = &points[i];
            let dx = p1.x - p0.x;
            if dx.abs() < 1e-12 {
                return p0.y;
            }
            let t = (input - p0.x) / dx;
            let result = p0.y + t * (p1.y - p0.y);
            return result.clamp(0.0, 1.0);
        }
    }

    points[points.len() - 1].y
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ControlPoint ──────────────────────────────────────────────────────

    #[test]
    fn control_point_clamps() {
        let p = ControlPoint::new(-0.5, 1.5);
        assert_eq!(p.x, 0.0);
        assert_eq!(p.y, 1.0);
    }

    #[test]
    fn control_point_identity() {
        let p = ControlPoint::new(0.5, 0.7);
        assert!((p.x - 0.5).abs() < 1e-9);
        assert!((p.y - 0.7).abs() < 1e-9);
    }

    // ── AxisCurve::linear ─────────────────────────────────────────────────

    #[test]
    fn linear_curve_is_identity() {
        let curve = AxisCurve::linear();
        assert_eq!(curve.len(), 2);
        assert!((apply_curve(0.0, &curve)).abs() < 1e-9);
        assert!((apply_curve(0.5, &curve) - 0.5).abs() < 1e-9);
        assert!((apply_curve(1.0, &curve) - 1.0).abs() < 1e-9);
    }

    // ── AxisCurve::from_points ────────────────────────────────────────────

    #[test]
    fn from_points_valid() {
        let points = [
            ControlPoint::new(0.0, 0.0),
            ControlPoint::new(0.5, 0.8),
            ControlPoint::new(1.0, 1.0),
        ];
        let curve = AxisCurve::from_points(&points).unwrap();
        assert_eq!(curve.len(), 3);
    }

    #[test]
    fn from_points_too_few() {
        let points = [ControlPoint::new(0.5, 0.5)];
        assert!(AxisCurve::from_points(&points).is_err());
    }

    #[test]
    fn from_points_unsorted() {
        let points = [
            ControlPoint::new(0.5, 0.5),
            ControlPoint::new(0.3, 0.3),
        ];
        assert!(AxisCurve::from_points(&points).is_err());
    }

    #[test]
    fn from_points_duplicate_x() {
        let points = [
            ControlPoint::new(0.0, 0.0),
            ControlPoint::new(0.5, 0.3),
            ControlPoint::new(0.5, 0.7),
            ControlPoint::new(1.0, 1.0),
        ];
        assert!(AxisCurve::from_points(&points).is_err());
    }

    // ── read_axis_curve ───────────────────────────────────────────────────

    #[test]
    fn read_linear_curve() {
        let data = [2, 0, 0, 100, 100];
        let curve = read_axis_curve(&data).unwrap();
        assert_eq!(curve.len(), 2);
        assert!((curve.points()[0].x).abs() < 1e-9);
        assert!((curve.points()[0].y).abs() < 1e-9);
        assert!((curve.points()[1].x - 1.0).abs() < 1e-9);
        assert!((curve.points()[1].y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn read_three_point_curve() {
        let data = [3, 0, 0, 50, 80, 100, 100];
        let curve = read_axis_curve(&data).unwrap();
        assert_eq!(curve.len(), 3);
        assert!((curve.points()[1].x - 0.5).abs() < 1e-9);
        assert!((curve.points()[1].y - 0.8).abs() < 1e-9);
    }

    #[test]
    fn read_empty_is_error() {
        assert!(read_axis_curve(&[]).is_err());
    }

    #[test]
    fn read_too_few_points() {
        assert!(read_axis_curve(&[1, 50, 50]).is_err());
    }

    #[test]
    fn read_too_many_points() {
        assert!(read_axis_curve(&[12]).is_err());
    }

    #[test]
    fn read_truncated_data() {
        // Claims 3 points but only provides 2
        let data = [3, 0, 0, 50, 50];
        assert!(read_axis_curve(&data).is_err());
    }

    #[test]
    fn read_unsorted_x_values() {
        let data = [2, 80, 80, 20, 20];
        assert!(read_axis_curve(&data).is_err());
    }

    #[test]
    fn read_duplicate_x_values() {
        let data = [3, 0, 0, 50, 50, 50, 80];
        assert!(read_axis_curve(&data).is_err());
    }

    // ── apply_curve ───────────────────────────────────────────────────────

    #[test]
    fn apply_linear() {
        let curve = AxisCurve::linear();
        for i in 0..=10 {
            let v = i as f64 / 10.0;
            assert!((apply_curve(v, &curve) - v).abs() < 1e-9, "failed at {v}");
        }
    }

    #[test]
    fn apply_aggressive_curve() {
        // Curve that maps 50% input to 80% output
        let points = [
            ControlPoint::new(0.0, 0.0),
            ControlPoint::new(0.5, 0.8),
            ControlPoint::new(1.0, 1.0),
        ];
        let curve = AxisCurve::from_points(&points).unwrap();
        assert!((apply_curve(0.0, &curve)).abs() < 1e-9);
        assert!((apply_curve(0.5, &curve) - 0.8).abs() < 1e-9);
        assert!((apply_curve(1.0, &curve) - 1.0).abs() < 1e-9);
        // 25% input → linear interpolation between (0,0) and (0.5,0.8) = 0.4
        assert!((apply_curve(0.25, &curve) - 0.4).abs() < 1e-9);
    }

    #[test]
    fn apply_clamps_input() {
        let curve = AxisCurve::linear();
        assert!((apply_curve(-1.0, &curve)).abs() < 1e-9);
        assert!((apply_curve(2.0, &curve) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn apply_below_first_point() {
        let points = [
            ControlPoint::new(0.2, 0.3),
            ControlPoint::new(0.8, 0.9),
        ];
        let curve = AxisCurve::from_points(&points).unwrap();
        // Input below first point → returns first point's y
        assert!((apply_curve(0.1, &curve) - 0.3).abs() < 1e-9);
    }

    #[test]
    fn apply_above_last_point() {
        let points = [
            ControlPoint::new(0.2, 0.3),
            ControlPoint::new(0.8, 0.9),
        ];
        let curve = AxisCurve::from_points(&points).unwrap();
        assert!((apply_curve(0.9, &curve) - 0.9).abs() < 1e-9);
    }

    #[test]
    fn roundtrip_read_and_apply() {
        let data = [3, 0, 0, 50, 80, 100, 100];
        let curve = read_axis_curve(&data).unwrap();
        // At 0% → 0%
        assert!((apply_curve(0.0, &curve)).abs() < 1e-9);
        // At 50% → 80%
        assert!((apply_curve(0.5, &curve) - 0.8).abs() < 1e-9);
        // At 100% → 100%
        assert!((apply_curve(1.0, &curve) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn curve_is_not_empty() {
        let curve = AxisCurve::linear();
        assert!(!curve.is_empty());
    }

    #[test]
    fn curve_points_accessor() {
        let curve = AxisCurve::linear();
        assert_eq!(curve.points().len(), 2);
    }
}
