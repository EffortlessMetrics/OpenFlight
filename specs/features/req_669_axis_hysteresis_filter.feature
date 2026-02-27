@REQ-669
Feature: Axis Hysteresis Filter
  @AC-669.1
  Scenario: Hysteresis band prevents oscillation at decision boundaries
    Given the system is configured for REQ-669
    When the feature condition is met
    Then hysteresis band prevents oscillation at decision boundaries

  @AC-669.2
  Scenario: Band width is configurable per axis in profile
    Given the system is configured for REQ-669
    When the feature condition is met
    Then band width is configurable per axis in profile

  @AC-669.3
  Scenario: Filter does not add latency beyond one sample period
    Given the system is configured for REQ-669
    When the feature condition is met
    Then filter does not add latency beyond one sample period

  @AC-669.4
  Scenario: Hysteresis resets on axis recalibration
    Given the system is configured for REQ-669
    When the feature condition is met
    Then hysteresis resets on axis recalibration
