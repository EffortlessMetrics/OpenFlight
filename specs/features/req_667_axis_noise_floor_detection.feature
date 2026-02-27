@REQ-667
Feature: Axis Noise Floor Detection
  @AC-667.1
  Scenario: Noise floor is measured during calibration idle period
    Given the system is configured for REQ-667
    When the feature condition is met
    Then noise floor is measured during calibration idle period

  @AC-667.2
  Scenario: Noise floor threshold is stored per-axis in calibration data
    Given the system is configured for REQ-667
    When the feature condition is met
    Then noise floor threshold is stored per-axis in calibration data

  @AC-667.3
  Scenario: Inputs below noise floor are suppressed to zero
    Given the system is configured for REQ-667
    When the feature condition is met
    Then inputs below noise floor are suppressed to zero

  @AC-667.4
  Scenario: Noise floor recalibration can be triggered manually
    Given the system is configured for REQ-667
    When the feature condition is met
    Then noise floor recalibration can be triggered manually
