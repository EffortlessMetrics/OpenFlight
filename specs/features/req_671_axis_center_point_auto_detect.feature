@REQ-671
Feature: Axis Center Point Auto-Detect
  @AC-671.1
  Scenario: Center point is detected from idle position during calibration
    Given the system is configured for REQ-671
    When the feature condition is met
    Then center point is detected from idle position during calibration

  @AC-671.2
  Scenario: Auto-detect tolerates small deviations from mechanical center
    Given the system is configured for REQ-671
    When the feature condition is met
    Then auto-detect tolerates small deviations from mechanical center

  @AC-671.3
  Scenario: Center point can be manually overridden in profile
    Given the system is configured for REQ-671
    When the feature condition is met
    Then center point can be manually overridden in profile

  @AC-671.4
  Scenario: Center offset is applied before deadzone processing
    Given the system is configured for REQ-671
    When the feature condition is met
    Then center offset is applied before deadzone processing
