@REQ-273 @product
Feature: Axis curve validation rejects malformed control points and falls back to linear identity  @AC-273.1
  Scenario: Curve with fewer than two points is rejected on profile load
    Given a profile containing an axis curve with only one control point
    When the profile is loaded
    Then the service SHALL reject the profile with a validation error  @AC-273.2
  Scenario: Non-monotone curve with unsorted x values is rejected
    Given a profile containing an axis curve where x values are not strictly increasing
    When the profile is loaded
    Then the service SHALL reject the curve with a non-monotone validation error  @AC-273.3
  Scenario: Control point outside unit range is rejected
    Given a profile containing an axis curve with a control point whose x or y value is outside [0.0, 1.0]
    When the profile is loaded
    Then the service SHALL reject the curve with an out-of-range validation error  @AC-273.4
  Scenario: Valid curve passes validation without modification
    Given a profile containing an axis curve with at least two points all within [0.0, 1.0] and monotonically increasing x values
    When the profile is loaded
    Then the service SHALL accept the curve and apply it without alteration  @AC-273.5
  Scenario: Validation error identifies the offending point index and value
    Given a profile containing an axis curve with an invalid control point
    When the profile is loaded and validation fails
    Then the validation error SHALL include the index and coordinate value of the offending point  @AC-273.6
  Scenario: Profile with invalid curve falls back to linear identity curve
    Given a profile containing an axis curve that fails validation
    When the service loads the profile
    Then the axis SHALL operate with a linear identity curve instead of the invalid one
