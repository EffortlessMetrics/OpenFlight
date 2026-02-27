Feature: Device Firmware Update Check
  As a flight simulation enthusiast
  I want device firmware update check
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Check for firmware updates on devices
    Given the system is configured for device firmware update check
    When the feature is exercised
    Then service checks for available firmware updates for connected devices

  Scenario: Report availability via CLI
    Given the system is configured for device firmware update check
    When the feature is exercised
    Then update availability is reported via the CLI device info command

  Scenario: Configurable check interval
    Given the system is configured for device firmware update check
    When the feature is exercised
    Then firmware check respects a configurable check interval

  Scenario: Failures do not affect device operation
    Given the system is configured for device firmware update check
    When the feature is exercised
    Then firmware check failures are logged without affecting device operation
