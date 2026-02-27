@REQ-678
Feature: Axis Calibration Export
  @AC-678.1
  Scenario: Calibration data can be exported to a portable file format
    Given the system is configured for REQ-678
    When the feature condition is met
    Then calibration data can be exported to a portable file format

  @AC-678.2
  Scenario: Exported calibration can be imported on another installation
    Given the system is configured for REQ-678
    When the feature condition is met
    Then exported calibration can be imported on another installation

  @AC-678.3
  Scenario: Export includes device identifier for matching verification
    Given the system is configured for REQ-678
    When the feature condition is met
    Then export includes device identifier for matching verification

  @AC-678.4
  Scenario: Import validates compatibility before applying calibration
    Given the system is configured for REQ-678
    When the feature condition is met
    Then import validates compatibility before applying calibration
