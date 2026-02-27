@REQ-455 @product
Feature: Axis Trim System — Software Trim Adjustment for Axis Center Position

  @AC-455.1
  Scenario: Trim value shifts the axis center position
    Given an axis with trim set to +0.05
    When the physical input is at the hardware center (normalized 0.0)
    Then the axis output SHALL be 0.05

  @AC-455.2
  Scenario: Trim is adjustable via CLI and persisted in profile
    Given a running service and a loaded profile
    When the command "flightctl axis trim --axis pitch --value 0.03" is executed
    Then the trim SHALL take effect immediately and be persisted to the active profile

  @AC-455.3
  Scenario: Trim adjustment is clamped to configured maximum offset
    Given an axis with maximum trim offset configured at 0.1
    When a trim value of 0.5 is requested
    Then the applied trim SHALL be clamped to 0.1 and a warning SHALL be logged

  @AC-455.4
  Scenario: Trim reset command returns trim to zero
    Given an axis with trim currently set to 0.07
    When the command "flightctl axis trim --axis pitch --reset" is executed
    Then the trim SHALL return to 0.0 and the profile SHALL be updated accordingly
