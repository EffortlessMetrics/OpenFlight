@REQ-270 @product
Feature: Axis output supports configurable scale factor and configurable output range  @AC-270.1
  Scenario: Axis scaled by constant factor
    Given a profile that specifies a scale factor of 0.5 for an axis
    When a raw input value of 1.0 is processed
    Then the scaled output SHALL be 0.5  @AC-270.2
  Scenario: Scaled output clamped to configured range
    Given an axis with scale factor 2.0 and output range [-1.0, 1.0]
    When a raw input value of 0.8 is processed
    Then the output SHALL be clamped to 1.0 rather than 1.6  @AC-270.3
  Scenario: Output range configurable per axis
    Given two axes in the same profile with different output ranges
    When both axes process the same raw input
    Then each axis SHALL produce output within its own individually configured range  @AC-270.4
  Scenario: Default output range is negative one to one
    Given an axis profile entry with no output range specified
    When the axis processes input values at the extremes
    Then the output range SHALL default to [-1.0, 1.0]  @AC-270.5
  Scenario: Scale applied after deadzone and curve
    Given an axis with a 10% deadzone, a cubic curve, and a scale factor of 0.75
    When an input value is processed
    Then the processing order SHALL be deadzone then curve then scale  @AC-270.6
  Scenario: Range violation produces warning not panic
    Given a profile that configures an output range where min is greater than max
    When the profile is loaded
    Then the service SHALL emit a warning log entry and apply a safe default range rather than panicking
