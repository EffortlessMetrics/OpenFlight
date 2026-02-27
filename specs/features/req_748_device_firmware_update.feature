Feature: Device Firmware Update
  As a flight simulation enthusiast
  I want the service to support device firmware updates
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Firmware updates supported
    Given a device supports firmware updates
    When a new firmware version is available
    Then the service can apply the update

  Scenario: Update requires confirmation
    Given a firmware update is available
    When the update is initiated
    Then explicit user confirmation is required

  Scenario: Progress reported
    Given a firmware update is in progress
    When the update progresses
    Then progress is reported via IPC and CLI

  Scenario: Failed update rolls back
    Given a firmware update fails
    When the failure is detected
    Then the device rolls back to the previous firmware
