@REQ-446 @product
Feature: Axis Calibration Wizard — CLI-Guided Per-Axis Calibration Procedure

  @AC-446.1
  Scenario: Calibration wizard prompts user to move axis to minimum then maximum
    Given an uncalibrated axis is selected for calibration
    When the wizard starts
    Then it SHALL prompt the user to move the axis to its minimum position and then to its maximum position

  @AC-446.2
  Scenario: Wizard records min/max and center values and saves to calibration store
    Given the user has moved the axis through its full range as directed
    When the wizard completes data collection
    Then it SHALL save the observed minimum, maximum, and center values to the calibration store

  @AC-446.3
  Scenario: Calibration results are applied immediately without service restart
    Given calibration data has been saved for an axis
    When the wizard finalises the calibration
    Then the axis engine SHALL use the new calibration data on the next tick without requiring a service restart

  @AC-446.4
  Scenario: Wizard can be cancelled without overwriting existing calibration
    Given a previously calibrated axis
    When the user starts the wizard and then cancels before completion
    Then the existing calibration data SHALL remain unchanged
