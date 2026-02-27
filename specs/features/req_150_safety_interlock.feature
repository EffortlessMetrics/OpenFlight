@REQ-150 @product
Feature: Safety interlock triggers  @AC-150.1
  Scenario: Maximum force limit prevents over-torque
    Given an FFB device with a configured maximum force limit
    When a force command exceeding the maximum limit is received
    Then the safety interlock SHALL clamp the output to the maximum permitted force  @AC-150.2
  Scenario: Temperature threshold triggers FFB reduction
    Given an FFB device reporting a temperature above the safety threshold
    When the thermal monitor detects the over-temperature condition
    Then the safety interlock SHALL reduce FFB output to protect the device  @AC-150.3
  Scenario: Voltage drop detected and FFB attenuated
    Given an FFB device reporting a supply voltage below the safe threshold
    When the voltage monitor detects the under-voltage condition
    Then the safety interlock SHALL attenuate FFB output proportionally  @AC-150.4
  Scenario: Physical stop detection halts motor command
    Given an FFB device with physical stop detection enabled
    When the axis reaches a hard stop position
    Then the safety interlock SHALL immediately halt the motor command  @AC-150.5
  Scenario: Emergency stop kills FFB immediately
    Given an FFB device with active force output
    When the emergency stop command is issued
    Then all force output SHALL be zeroed within one control cycle  @AC-150.6
  Scenario: Safe-mode profile activates on interlock trigger
    Given an FFB device with a configured safe-mode profile
    When any safety interlock condition is triggered
    Then the safe-mode profile SHALL become active for subsequent commands  @AC-150.7
  Scenario: Interlock event logged with timestamp
    Given the safety interlock logging is enabled
    When a safety interlock condition is triggered
    Then the event SHALL be recorded in the log with a UTC timestamp  @AC-150.8
  Scenario: Interlock cleared only by explicit reset command
    Given a safety interlock that has been triggered
    When the system restarts without an explicit reset command
    Then the interlock SHALL remain active until an explicit reset is issued
