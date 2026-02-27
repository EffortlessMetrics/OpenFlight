@REQ-385 @product
Feature: Axis Split by Sign into Positive and Negative Halves  @AC-385.1
  Scenario: Split mode divides one axis into two logical axes
    Given an axis configured in split mode
    When the axis produces output
    Then it SHALL be separated into a positive-half axis and a negative-half axis  @AC-385.2
  Scenario: Positive half maps center-to-max of source to 0 to 1
    Given an axis in split mode with center at 0.0 and max at 1.0
    When the source axis moves from center to max
    Then the positive half SHALL output values linearly from 0.0 to 1.0  @AC-385.3
  Scenario: Negative half maps center-to-min of source to 0 to 1
    Given an axis in split mode with center at 0.0 and min at -1.0
    When the source axis moves from center to min
    Then the negative half SHALL output values linearly from 0.0 to 1.0  @AC-385.4
  Scenario: Split mode is invertible via combine
    Given an axis that has been split into positive and negative halves
    When the halves are recombined using the combine operation
    Then the result SHALL equal the original source axis value  @AC-385.5
  Scenario: Property test confirms split then combine is identity within float epsilon
    Given a property test with arbitrary axis values in [-1, 1]
    When split and combine are applied in sequence
    Then the result SHALL be within float epsilon of the original input  @AC-385.6
  Scenario: Split is useful for a combined brake pedal axis to left and right brakes
    Given a single combined brake pedal axis
    When split mode is applied to produce two logical axes
    Then the two resulting axes SHALL independently represent left and right brake pressure
