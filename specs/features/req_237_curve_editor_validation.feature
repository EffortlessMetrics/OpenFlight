@REQ-237 @product
Feature: Axis response curves validated for mathematical correctness  @AC-237.1
  Scenario: Monotone curve requires strictly increasing X values
    Given a curve definition with control points
    When a control point has an X value not strictly greater than the previous point's X value
    Then the curve validation SHALL reject it as non-monotone  @AC-237.2
  Scenario: Control point X values must be in range 0.0 to 1.0
    Given a curve definition with control points
    When any control point has an X value outside the closed interval [0.0, 1.0]
    Then the curve validation SHALL reject it with an out-of-range error  @AC-237.3
  Scenario: Control point Y values must be in range 0.0 to 1.0
    Given a curve definition with control points
    When any control point has a Y value outside the closed interval [0.0, 1.0]
    Then the curve validation SHALL reject it with an out-of-range error  @AC-237.4
  Scenario: Interpolation between control points uses cubic Hermite spline
    Given a validated curve with at least two control points
    When the axis engine evaluates a position between two control points
    Then the interpolated output SHALL follow cubic Hermite spline mathematics  @AC-237.5
  Scenario: Symmetric curve option mirrors first half to second half automatically
    Given a curve with the symmetric option enabled
    When control points are defined for the first half of the range
    Then the curve SHALL automatically mirror those points to produce a symmetric second half  @AC-237.6
  Scenario: Curve with zero control points returns linear identity mapping
    Given a curve definition containing zero control points
    When the axis engine evaluates any input position
    Then the output SHALL equal the input unchanged as a linear identity mapping
