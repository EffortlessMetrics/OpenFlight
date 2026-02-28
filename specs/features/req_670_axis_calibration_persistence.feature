@REQ-670
Feature: Axis Calibration Persistence
  @AC-670.1
  Scenario: Calibration data is saved to persistent storage on completion
    Given the system is configured for REQ-670
    When the feature condition is met
    Then calibration data is saved to persistent storage on completion

  @AC-670.2
  Scenario: Calibration is restored on service restart
    Given the system is configured for REQ-670
    When the feature condition is met
    Then calibration is restored on service restart

  @AC-670.3
  Scenario: Corrupt calibration file falls back to defaults with warning
    Given the system is configured for REQ-670
    When the feature condition is met
    Then corrupt calibration file falls back to defaults with warning

  @AC-670.4
  Scenario: Calibration file format is versioned for forward compatibility
    Given the system is configured for REQ-670
    When the feature condition is met
    Then calibration file format is versioned for forward compatibility
