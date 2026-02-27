Feature: Device Firmware Version Detection
  As a flight simulation enthusiast
  I want the service to detect and report HID device firmware versions
  So that firmware-related issues can be diagnosed and flagged automatically

  Background:
    Given the OpenFlight service is running
    And a compatible HID device is connected

  Scenario: Firmware version queried on enumeration
    When the service enumerates connected HID devices
    Then the firmware version is queried for each device
    And the firmware version is stored in the device record

  Scenario: Firmware version included in diagnostics
    When I request device diagnostics via "flightctl devices --diagnostics"
    Then the output includes the firmware version for each device
    And the firmware version is formatted as a semver string

  Scenario: Known buggy firmware flagged from compatibility manifest
    Given the compatibility manifest contains a known-buggy firmware entry for VID 0x044F PID 0xB10A version "1.2.3"
    When the service enumerates a device with that VID, PID, and firmware version
    Then the device diagnostics include a firmware bug warning
    And the warning references the compatibility manifest entry
