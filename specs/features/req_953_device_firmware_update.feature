Feature: Device Firmware Update
  As a flight simulation enthusiast
  I want device firmware update
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Firmware update files can be flashed to supported devices through OpenFlight
    Given the system is configured for device firmware update
    When the feature is exercised
    Then firmware update files can be flashed to supported devices through OpenFlight

  Scenario: Firmware update process validates file integrity before flashing
    Given the system is configured for device firmware update
    When the feature is exercised
    Then firmware update process validates file integrity before flashing

  Scenario: Update progress is reported with percentage and estimated time remaining
    Given the system is configured for device firmware update
    When the feature is exercised
    Then update progress is reported with percentage and estimated time remaining

  Scenario: Failed firmware update triggers automatic rollback to previous version
    Given the system is configured for device firmware update
    When the feature is exercised
    Then failed firmware update triggers automatic rollback to previous version