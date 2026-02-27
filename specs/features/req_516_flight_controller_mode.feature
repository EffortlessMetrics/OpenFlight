@REQ-516 @product
Feature: Flight Controller Mode Detection

  @AC-516.1 @AC-516.2
  Scenario: Controller mode is detected from active profile and devices
    Given a profile configured for helicopter operation and a collective axis device
    When the service evaluates the active profile and connected devices
    Then the controller mode SHALL be set to "helicopter"

  @AC-516.3
  Scenario: Mode change triggers a profile phase transition
    Given the controller mode is currently "standard"
    When the active profile changes to one defining "spaceflight" mode
    Then a profile phase transition SHALL be triggered for the new mode

  @AC-516.4
  Scenario: Mode is reported in service status and IPC
    Given the service is running with a detected controller mode
    When the user runs flightctl service status or queries the IPC status endpoint
    Then the response SHALL include the current controller mode
