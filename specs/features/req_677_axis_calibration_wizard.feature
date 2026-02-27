@REQ-677
Feature: Axis Calibration Wizard
  @AC-677.1
  Scenario: Step-by-step wizard guides user through calibration process
    Given the system is configured for REQ-677
    When the feature condition is met
    Then step-by-step wizard guides user through calibration process

  @AC-677.2
  Scenario: Wizard detects when axis is at center and full deflection
    Given the system is configured for REQ-677
    When the feature condition is met
    Then wizard detects when axis is at center and full deflection

  @AC-677.3
  Scenario: Progress is shown visually during calibration
    Given the system is configured for REQ-677
    When the feature condition is met
    Then progress is shown visually during calibration

  @AC-677.4
  Scenario: Wizard can be cancelled without affecting existing calibration
    Given the system is configured for REQ-677
    When the feature condition is met
    Then wizard can be cancelled without affecting existing calibration
