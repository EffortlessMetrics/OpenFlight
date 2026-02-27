Feature: Hardware Capability Detection
  As a flight simulation enthusiast
  I want the service to detect hardware capabilities on enumeration
  So that devices are configured correctly without manual specification

  Background:
    Given the OpenFlight service is running
    And a new HID device is connected

  Scenario: Axis resolution is detected from HID descriptor
    Given the HID descriptor reports a logical maximum of 65535 for the X axis
    When the device is enumerated
    Then the axis resolution is recorded as 16-bit for that device

  Scenario: FFB capability presence is detected and cached
    Given the HID descriptor contains force feedback usage pages
    When the device is enumerated
    Then FFB capability is marked as present in the device capability cache

  Scenario: Capability cache is invalidated on firmware version change
    Given a device has a cached capability entry for firmware version "1.0.0"
    When the device reconnects reporting firmware version "1.1.0"
    Then the capability cache entry for that device is invalidated and re-detected

  Scenario: Capabilities are exposed in device diagnostics JSON
    When the operator runs "flightctl device diagnostics --device STICK_LEFT"
    Then the diagnostics JSON output contains an "axis_resolution_bits" field
    And a "ffb_capable" boolean field
