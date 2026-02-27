@REQ-675
Feature: Axis Multi-Point Calibration
  @AC-675.1
  Scenario: Calibration captures values at multiple reference points
    Given the system is configured for REQ-675
    When the feature condition is met
    Then calibration captures values at multiple reference points

  @AC-675.2
  Scenario: Interpolation between calibration points uses cubic spline
    Given the system is configured for REQ-675
    When the feature condition is met
    Then interpolation between calibration points uses cubic spline

  @AC-675.3
  Scenario: Minimum of 3 calibration points are required per axis
    Given the system is configured for REQ-675
    When the feature condition is met
    Then minimum of 3 calibration points are required per axis

  @AC-675.4
  Scenario: Calibration points can be added incrementally
    Given the system is configured for REQ-675
    When the feature condition is met
    Then calibration points can be added incrementally
