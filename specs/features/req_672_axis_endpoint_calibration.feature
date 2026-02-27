@REQ-672
Feature: Axis Endpoint Calibration
  @AC-672.1
  Scenario: Full deflection endpoints are captured during calibration sweep
    Given the system is configured for REQ-672
    When the feature condition is met
    Then full deflection endpoints are captured during calibration sweep

  @AC-672.2
  Scenario: Endpoint values define the axis normalization range
    Given the system is configured for REQ-672
    When the feature condition is met
    Then endpoint values define the axis normalization range

  @AC-672.3
  Scenario: Asymmetric endpoints are supported for worn hardware
    Given the system is configured for REQ-672
    When the feature condition is met
    Then asymmetric endpoints are supported for worn hardware

  @AC-672.4
  Scenario: Endpoint calibration validates that range exceeds minimum threshold
    Given the system is configured for REQ-672
    When the feature condition is met
    Then endpoint calibration validates that range exceeds minimum threshold
