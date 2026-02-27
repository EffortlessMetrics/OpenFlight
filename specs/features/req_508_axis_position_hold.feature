@REQ-508 @product
Feature: Axis Position Hold Mode — Freeze Axis Output at Current Value  @AC-508.1
  Scenario: Position hold mode freezes axis output at current value
    Given an axis is outputting 0.5
    When position hold mode is activated for that axis
    Then subsequent physical axis movements SHALL not change the output value  @AC-508.2
  Scenario: Hold is activatable via IPC command or hardware button
    Given the service is running with an axis configured for position hold
    When an IPC hold-axis command is sent for that axis
    Then position hold SHALL be activated and confirmed in the IPC response  @AC-508.3
  Scenario: Hold is automatically released when physical axis moves beyond threshold
    Given position hold is active on an axis frozen at 0.5
    When the physical axis is moved to 0.8 exceeding the release threshold
    Then position hold SHALL be automatically released and the output SHALL follow the physical position  @AC-508.4
  Scenario: Hold state is visible in axis diagnostics
    Given position hold is active on an axis
    When the axis diagnostics are queried via IPC
    Then the diagnostics response SHALL include a hold-active flag set to true
