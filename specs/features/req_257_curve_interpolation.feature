@REQ-257 @product
Feature: Axis curve interpolation preserves monotonicity and smoothness  @AC-257.1
  Scenario: Linear interpolation available as option
    Given a curve with multiple control points and interpolation mode set to linear
    When the curve is evaluated at a point between two control points
    Then the result SHALL be the linearly interpolated value between the surrounding control points  @AC-257.2
  Scenario: Cubic Hermite spline interpolation available as option
    Given a curve with multiple control points and interpolation mode set to cubic-hermite
    When the curve is evaluated at a point between two control points
    Then the result SHALL be computed using a cubic Hermite spline producing a smooth C1 curve  @AC-257.3
  Scenario: Monotone cubic spline preserves monotonicity
    Given a monotonically increasing set of curve control points and interpolation mode set to monotone-cubic
    When the curve is evaluated across the full input range
    Then the output SHALL be monotonically non-decreasing throughout the range (Fritsch-Carlson guarantee)  @AC-257.4
  Scenario: Output of any valid curve is always in range zero to one
    Given any valid curve configuration with input in [0.0, 1.0]
    When the curve is evaluated across 10000 uniformly spaced input samples
    Then every output value SHALL be in the range [0.0, 1.0]  @AC-257.5
  Scenario: Curve with only two endpoints returns linear mapping
    Given a curve defined by only two points at (0.0, 0.0) and (1.0, 1.0)
    When the curve is evaluated at any input value in [0.0, 1.0]
    Then the output SHALL equal the input value (identity/linear mapping)  @AC-257.6
  Scenario: Curve evaluation benchmarked under 100ns per point
    Given a curve with ten control points compiled into the axis pipeline
    When the benchmark evaluates a single curve lookup
    Then the p99 evaluation time SHALL be below 100 nanoseconds per point
