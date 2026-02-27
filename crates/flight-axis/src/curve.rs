// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis response curve with multiple interpolation modes.
//!
//! Curves are defined by control points [(x0,y0), (x1,y1), ...] where x is input
//! and y is output, both in [0.0, 1.0]. Points must be sorted by x with x0=0.0, xn=1.0.

use thiserror::Error;

/// Errors returned when constructing a [`ResponseCurve`].
#[derive(Debug, Error, PartialEq)]
pub enum CurveError {
    /// At least two control points are required.
    #[error("curve requires at least 2 control points")]
    TooFewPoints,
    /// Control points must have strictly increasing x values.
    #[error("control points must be sorted by x in ascending order")]
    NotSortedByX,
    /// An x or y coordinate is outside [0.0, 1.0].
    #[error("out of range: {0}")]
    OutOfRange(String),
}

/// Interpolation strategy for [`ResponseCurve::evaluate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpolationMode {
    /// Piecewise linear interpolation between adjacent control points.
    Linear,
    /// Cubic Hermite spline with finite-difference tangents (Catmull-Rom style).
    CubicHermite,
    /// Monotone-preserving cubic spline (Fritsch-Carlson algorithm).
    MonotoneCubic,
}

/// A single control point on a response curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPoint {
    /// Input value in [0.0, 1.0].
    pub x: f32,
    /// Output value in [0.0, 1.0].
    pub y: f32,
}

impl ControlPoint {
    /// Create a new control point.
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Axis response curve mapping normalized input [0.0, 1.0] to normalized output [0.0, 1.0].
///
/// # Example
///
/// ```
/// use flight_axis::curve::{ResponseCurve, ControlPoint, InterpolationMode};
///
/// let curve = ResponseCurve::linear_identity();
/// assert!((curve.evaluate(0.5) - 0.5).abs() < 1e-5);
/// ```
#[derive(Debug, Clone)]
pub struct ResponseCurve {
    /// Control points sorted by x in ascending order.
    pub points: Vec<ControlPoint>,
    /// Interpolation mode used by [`Self::evaluate`].
    pub mode: InterpolationMode,
}

impl ResponseCurve {
    /// Identity curve using linear interpolation: maps input directly to output.
    #[must_use]
    pub fn linear_identity() -> Self {
        Self {
            points: vec![ControlPoint::new(0.0, 0.0), ControlPoint::new(1.0, 1.0)],
            mode: InterpolationMode::Linear,
        }
    }

    /// Construct a curve from a set of control points and an interpolation mode.
    ///
    /// # Errors
    ///
    /// Returns [`CurveError::TooFewPoints`] if fewer than 2 points are given,
    /// [`CurveError::NotSortedByX`] if x values are not strictly ascending, or
    /// [`CurveError::OutOfRange`] if any coordinate is outside [0.0, 1.0].
    pub fn from_points(
        points: Vec<ControlPoint>,
        mode: InterpolationMode,
    ) -> Result<Self, CurveError> {
        if points.len() < 2 {
            return Err(CurveError::TooFewPoints);
        }
        for (i, pt) in points.iter().enumerate() {
            if !(0.0..=1.0).contains(&pt.x) {
                return Err(CurveError::OutOfRange(format!(
                    "point[{i}].x = {} is out of [0.0, 1.0]",
                    pt.x
                )));
            }
            if !(0.0..=1.0).contains(&pt.y) {
                return Err(CurveError::OutOfRange(format!(
                    "point[{i}].y = {} is out of [0.0, 1.0]",
                    pt.y
                )));
            }
        }
        for i in 1..points.len() {
            if points[i].x <= points[i - 1].x {
                return Err(CurveError::NotSortedByX);
            }
        }
        Ok(Self { points, mode })
    }

    /// Evaluate the curve at `x`, which is clamped to [0.0, 1.0].
    ///
    /// The output is also clamped to [0.0, 1.0].
    #[must_use]
    pub fn evaluate(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        let pts = &self.points;
        let n = pts.len();

        // Find the leftmost segment window whose right endpoint is >= x.
        let seg = pts.windows(2).position(|w| x <= w[1].x).unwrap_or(n - 2);

        let p0 = pts[seg];
        let p1 = pts[seg + 1];
        let dx = p1.x - p0.x;

        if dx <= 0.0 {
            return p0.y.clamp(0.0, 1.0);
        }

        let t = (x - p0.x) / dx;

        let y = match self.mode {
            InterpolationMode::Linear => p0.y + t * (p1.y - p0.y),
            InterpolationMode::CubicHermite => {
                let tangents = Self::hermite_tangents(pts);
                hermite_eval(t, dx, p0.y, p1.y, tangents[seg], tangents[seg + 1])
            }
            InterpolationMode::MonotoneCubic => {
                let tangents = Self::monotone_tangents(pts);
                hermite_eval(t, dx, p0.y, p1.y, tangents[seg], tangents[seg + 1])
            }
        };

        y.clamp(0.0, 1.0)
    }

    /// Number of control points.
    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns `true` when there are no control points.
    ///
    /// In practice a valid [`ResponseCurve`] always has at least 2 points, so
    /// this will only be `true` for a default-constructed or otherwise empty curve.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Returns `true` if the output values are non-decreasing across all control points.
    #[must_use]
    pub fn is_monotone(&self) -> bool {
        self.points.windows(2).all(|w| w[1].y >= w[0].y)
    }

    /// Compute per-point tangents using central (interior) and one-sided (boundary)
    /// finite differences.
    fn hermite_tangents(pts: &[ControlPoint]) -> Vec<f32> {
        let n = pts.len();
        let mut tangents = vec![0.0f32; n];

        let slope = |i: usize| -> f32 { (pts[i + 1].y - pts[i].y) / (pts[i + 1].x - pts[i].x) };

        tangents[0] = slope(0);
        tangents[n - 1] = slope(n - 2);
        for (i, t) in tangents.iter_mut().enumerate().take(n - 1).skip(1) {
            *t = (slope(i - 1) + slope(i)) / 2.0;
        }
        tangents
    }

    /// Compute per-point tangents using the Fritsch-Carlson monotone cubic algorithm.
    fn monotone_tangents(pts: &[ControlPoint]) -> Vec<f32> {
        let n = pts.len();

        // Segment slopes.
        let delta: Vec<f32> = (0..n - 1)
            .map(|k| (pts[k + 1].y - pts[k].y) / (pts[k + 1].x - pts[k].x))
            .collect();

        // Initial tangent estimates.
        let mut tangents = vec![0.0f32; n];
        tangents[0] = delta[0];
        tangents[n - 1] = delta[n - 2];
        for k in 1..n - 1 {
            // Zero tangent at local extrema; average elsewhere.
            if delta[k - 1] * delta[k] <= 0.0 {
                tangents[k] = 0.0;
            } else {
                tangents[k] = (delta[k - 1] + delta[k]) / 2.0;
            }
        }

        // Fritsch-Carlson adjustment: rescale tangents so that α² + β² ≤ 9.
        for k in 0..n - 1 {
            if delta[k].abs() < f32::EPSILON {
                tangents[k] = 0.0;
                tangents[k + 1] = 0.0;
                continue;
            }
            let alpha = tangents[k] / delta[k];
            let beta = tangents[k + 1] / delta[k];
            let h = alpha.mul_add(alpha, beta * beta);
            if h > 9.0 {
                let tau = 3.0 / h.sqrt();
                tangents[k] = tau * alpha * delta[k];
                tangents[k + 1] = tau * beta * delta[k];
            }
        }

        tangents
    }
}

/// Evaluate a cubic Hermite polynomial on a single segment.
///
/// - `t`  – local parameter in [0, 1]
/// - `dx` – segment width (x1 − x0), used to scale tangents
/// - `y0`, `y1` – endpoint values
/// - `m0`, `m1` – endpoint tangents (dy/dx)
#[inline]
fn hermite_eval(t: f32, dx: f32, y0: f32, y1: f32, m0: f32, m1: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = t3.mul_add(2.0, t2.mul_add(-3.0, 1.0));
    let h10 = t3.mul_add(1.0, t2.mul_add(-2.0, t));
    let h01 = t3.mul_add(-2.0, 3.0 * t2);
    let h11 = t2.mul_add(-1.0, t3);
    h00 * y0 + h10 * dx * m0 + h01 * y1 + h11 * dx * m1
}

/// Exponential response curve.
///
/// Modifies axis sensitivity around center.
/// - expo = 0.0: linear
/// - expo > 0.0: reduced center sensitivity (typical flight sim "expo")
/// - expo < 0.0: increased center sensitivity
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExpoCurveConfig {
    /// Expo factor clamped to [-1.0, 1.0].
    pub expo: f32,
}

impl ExpoCurveConfig {
    /// Create a new expo config, clamping `expo` to [-1.0, 1.0].
    pub fn new(expo: f32) -> Self {
        Self {
            expo: expo.clamp(-1.0, 1.0),
        }
    }

    /// Linear response (no expo applied).
    pub fn linear() -> Self {
        Self { expo: 0.0 }
    }

    /// Apply expo curve to a value in [-1.0, 1.0].
    ///
    /// Uses the formula `expo * v³ + (1 − expo) * v`, clamped to [-1.0, 1.0].
    #[inline]
    pub fn apply(&self, value: f32) -> f32 {
        let v = value.clamp(-1.0, 1.0);
        let e = self.expo;
        (e * v * v * v + (1.0 - e) * v).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const TOL: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    // ── Unit tests ────────────────────────────────────────────────────────────

    #[test]
    fn test_linear_identity_evaluates_x() {
        let curve = ResponseCurve::linear_identity();
        assert!(approx_eq(curve.evaluate(0.5), 0.5, TOL));
    }

    #[test]
    fn test_linear_identity_zero() {
        let curve = ResponseCurve::linear_identity();
        assert!(approx_eq(curve.evaluate(0.0), 0.0, TOL));
    }

    #[test]
    fn test_linear_identity_one() {
        let curve = ResponseCurve::linear_identity();
        assert!(approx_eq(curve.evaluate(1.0), 1.0, TOL));
    }

    #[test]
    fn test_linear_interpolation_midpoint() {
        let curve = ResponseCurve::from_points(
            vec![ControlPoint::new(0.0, 0.0), ControlPoint::new(1.0, 0.5)],
            InterpolationMode::Linear,
        )
        .unwrap();
        assert!(approx_eq(curve.evaluate(0.5), 0.25, TOL));
    }

    #[test]
    fn test_too_few_points_error() {
        let result = ResponseCurve::from_points(
            vec![ControlPoint::new(0.0, 0.0)],
            InterpolationMode::Linear,
        );
        assert_eq!(result.unwrap_err(), CurveError::TooFewPoints);
    }

    #[test]
    fn test_unsorted_points_error() {
        let result = ResponseCurve::from_points(
            vec![ControlPoint::new(0.8, 0.8), ControlPoint::new(0.2, 0.2)],
            InterpolationMode::Linear,
        );
        assert_eq!(result.unwrap_err(), CurveError::NotSortedByX);
    }

    #[test]
    fn test_cubic_hermite_identity() {
        let curve = ResponseCurve::from_points(
            vec![ControlPoint::new(0.0, 0.0), ControlPoint::new(1.0, 1.0)],
            InterpolationMode::CubicHermite,
        )
        .unwrap();
        assert!(approx_eq(curve.evaluate(0.5), 0.5, 1e-4));
    }

    #[test]
    fn test_cubic_hermite_s_curve() {
        let curve = ResponseCurve::from_points(
            vec![
                ControlPoint::new(0.0, 0.0),
                ControlPoint::new(0.5, 0.3),
                ControlPoint::new(1.0, 1.0),
            ],
            InterpolationMode::CubicHermite,
        )
        .unwrap();
        let v = curve.evaluate(0.5);
        assert!(v >= 0.0 && v <= 1.0);
        assert!(approx_eq(v, 0.3, 1e-4));
    }

    #[test]
    fn test_monotone_cubic_identity() {
        let curve = ResponseCurve::from_points(
            vec![ControlPoint::new(0.0, 0.0), ControlPoint::new(1.0, 1.0)],
            InterpolationMode::MonotoneCubic,
        )
        .unwrap();
        assert!(approx_eq(curve.evaluate(0.3), 0.3, 1e-4));
    }

    #[test]
    fn test_monotone_cubic_preserves_monotone() {
        let curve = ResponseCurve::from_points(
            vec![
                ControlPoint::new(0.0, 0.0),
                ControlPoint::new(0.25, 0.2),
                ControlPoint::new(0.5, 0.5),
                ControlPoint::new(0.75, 0.8),
                ControlPoint::new(1.0, 1.0),
            ],
            InterpolationMode::MonotoneCubic,
        )
        .unwrap();
        let mut prev = 0.0f32;
        for i in 0..=100 {
            let x = i as f32 / 100.0;
            let y = curve.evaluate(x);
            assert!(
                y >= prev - 1e-5,
                "monotonicity violated at x={x}: {y} < {prev}"
            );
            prev = y;
        }
    }

    #[test]
    fn test_curve_evaluate_below_zero_clamped() {
        let curve = ResponseCurve::linear_identity();
        assert!(approx_eq(curve.evaluate(-0.1), 0.0, TOL));
    }

    #[test]
    fn test_curve_evaluate_above_one_clamped() {
        let curve = ResponseCurve::linear_identity();
        assert!(approx_eq(curve.evaluate(1.1), 1.0, TOL));
    }

    #[test]
    fn test_is_monotone_identity() {
        assert!(ResponseCurve::linear_identity().is_monotone());
    }

    #[test]
    fn test_linear_identity_constructor() {
        let curve = ResponseCurve::linear_identity();
        assert_eq!(curve.len(), 2);
        assert_eq!(curve.mode, InterpolationMode::Linear);
        assert!(approx_eq(curve.points[0].x, 0.0, TOL));
        assert!(approx_eq(curve.points[0].y, 0.0, TOL));
        assert!(approx_eq(curve.points[1].x, 1.0, TOL));
        assert!(approx_eq(curve.points[1].y, 1.0, TOL));
    }

    #[test]
    fn test_standard_deadzone_expo_curve() {
        // Flat deadzone [0, 0.05], then expo to full deflection.
        let curve = ResponseCurve::from_points(
            vec![
                ControlPoint::new(0.0, 0.0),
                ControlPoint::new(0.05, 0.0),
                ControlPoint::new(0.9, 0.8),
                ControlPoint::new(1.0, 1.0),
            ],
            InterpolationMode::Linear,
        )
        .unwrap();
        assert!(approx_eq(curve.evaluate(0.02), 0.0, TOL));
        assert!(curve.evaluate(0.5) > 0.0);
        assert!(approx_eq(curve.evaluate(1.0), 1.0, TOL));
    }

    // ── Property-based tests ──────────────────────────────────────────────────

    proptest! {
        /// Output is always within [0.0, 1.0] for any input in [0.0, 1.0].
        #[test]
        fn prop_output_always_in_range(x in 0.0f32..=1.0f32) {
            let curve = ResponseCurve::from_points(
                vec![
                    ControlPoint::new(0.0, 0.0),
                    ControlPoint::new(0.5, 0.7),
                    ControlPoint::new(1.0, 1.0),
                ],
                InterpolationMode::MonotoneCubic,
            )
            .unwrap();
            let y = curve.evaluate(x);
            prop_assert!(y >= 0.0 && y <= 1.0, "output {y} out of [0, 1] for x={x}");
        }

        /// Linear identity: evaluate(x) == x for all x in [0.0, 1.0].
        #[test]
        fn prop_linear_identity_eq_x(x in 0.0f32..=1.0f32) {
            let curve = ResponseCurve::linear_identity();
            let y = curve.evaluate(x);
            prop_assert!(
                (y - x).abs() < 1e-5,
                "linear identity: evaluate({x}) = {y} != {x}"
            );
        }

        /// Monotone cubic never decreases: evaluate(a) ≤ evaluate(b) when a ≤ b.
        #[test]
        fn prop_monotone_cubic_never_decreases(a in 0.0f32..=0.5f32, b in 0.5f32..=1.0f32) {
            let curve = ResponseCurve::from_points(
                vec![
                    ControlPoint::new(0.0, 0.0),
                    ControlPoint::new(0.5, 0.5),
                    ControlPoint::new(1.0, 1.0),
                ],
                InterpolationMode::MonotoneCubic,
            )
            .unwrap();
            let ya = curve.evaluate(a);
            let yb = curve.evaluate(b);
            prop_assert!(
                ya <= yb + 1e-5,
                "monotone violated: evaluate({a}) = {ya} > evaluate({b}) = {yb}"
            );
        }
    }

    // ── ExpoCurveConfig tests ─────────────────────────────────────────────────

    #[test]
    fn test_expo_zero_is_linear() {
        let expo = ExpoCurveConfig::linear();
        assert!((expo.apply(0.5) - 0.5).abs() < 1e-6);
        assert!((expo.apply(-0.3) - (-0.3)).abs() < 1e-6);
        assert!(expo.apply(0.0).abs() < 1e-6);
    }

    #[test]
    fn test_expo_positive_reduces_center_sensitivity() {
        let expo = ExpoCurveConfig::new(0.5);
        let out = expo.apply(0.5);
        assert!(out < 0.5, "expected output < 0.5, got {out}");
        let out_neg = expo.apply(-0.5);
        assert!(out_neg > -0.5, "expected output > -0.5, got {out_neg}");
    }

    #[test]
    fn test_expo_negative_increases_center_sensitivity() {
        let expo = ExpoCurveConfig::new(-0.5);
        let out = expo.apply(0.5);
        assert!(out > 0.5, "expected output > 0.5, got {out}");
        let out_neg = expo.apply(-0.5);
        assert!(out_neg < -0.5, "expected output < -0.5, got {out_neg}");
    }

    #[test]
    fn test_expo_extremes_clamp() {
        let expo_max = ExpoCurveConfig::new(1.0);
        let expo_min = ExpoCurveConfig::new(-1.0);
        for &v in &[-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
            let out_max = expo_max.apply(v);
            let out_min = expo_min.apply(v);
            assert!(
                (-1.0..=1.0).contains(&out_max),
                "expo=1.0, v={v}: output {out_max} out of range"
            );
            assert!(
                (-1.0..=1.0).contains(&out_min),
                "expo=-1.0, v={v}: output {out_min} out of range"
            );
        }
        assert!((expo_max.apply(1.0) - 1.0).abs() < 1e-5);
        assert!((expo_min.apply(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_expo_zero_input_maps_to_zero() {
        for &e in &[-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
            let expo = ExpoCurveConfig::new(e);
            assert!(expo.apply(0.0).abs() < 1e-6, "expo={e}: apply(0.0) != 0.0");
        }
    }

    #[test]
    fn test_expo_one_input_maps_to_one() {
        for &e in &[-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
            let expo = ExpoCurveConfig::new(e);
            assert!(
                (expo.apply(1.0) - 1.0).abs() < 1e-5,
                "expo={e}: apply(1.0) != 1.0"
            );
        }
    }

    #[test]
    fn test_expo_neg_one_input_maps_to_neg_one() {
        for &e in &[-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
            let expo = ExpoCurveConfig::new(e);
            assert!(
                (expo.apply(-1.0) + 1.0).abs() < 1e-5,
                "expo={e}: apply(-1.0) != -1.0"
            );
        }
    }

    #[test]
    fn test_expo_monotonic_for_positive_expo() {
        let expo = ExpoCurveConfig::new(0.7);
        let mut prev = -1.1_f32;
        for i in 0..=100 {
            let v = -1.0 + 2.0 * i as f32 / 100.0;
            let out = expo.apply(v);
            assert!(
                out >= prev - 1e-5,
                "monotonicity violated at v={v}: {out} < {prev}"
            );
            prev = out;
        }
    }
}
