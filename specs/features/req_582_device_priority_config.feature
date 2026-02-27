@REQ-582 @product
Feature: Device Priority Configuration — Service should support priority ordering for device inputs  @AC-582.1
  Scenario: Device priority list is configurable in profile
    Given a profile with a device_priority list specifying two devices
    When the profile is loaded
    Then the service SHALL apply the priority ordering defined in the profile  @AC-582.2
  Scenario: Higher priority device input overrides lower priority on conflict
    Given two devices are active and both provide input for the same axis
    When a conflict occurs
    Then the axis value SHALL be taken from the higher priority device  @AC-582.3
  Scenario: Priority tiebreaking uses most-recently-moved device
    Given two devices have equal priority and both provide input for the same axis
    When both devices report movement in the same tick
    Then the axis value SHALL be taken from the most-recently-moved device  @AC-582.4
  Scenario: Priority state is visible in axis diagnostics
    Given the axis engine is running with device priority configured
    When axis diagnostics are queried
    Then the diagnostics SHALL include the current active device and its priority rank for each axis
