Feature: Configurable HID Report Descriptor Parsing
  As a flight simulation enthusiast
  I want to provide custom HID report descriptor overrides
  So that devices with non-standard descriptors work correctly

  Background:
    Given the OpenFlight service is running

  Scenario: Custom descriptor override loaded from compat manifest quirks
    Given the compat manifest contains a descriptor override for VID 0x0483 PID 0x5720
    When the service enumerates that device
    Then the custom descriptor override is applied instead of the native descriptor
    And the device reports the overridden axis count and range

  Scenario: Override applied only to matching VID/PID
    Given a descriptor override is defined for VID 0x0483 PID 0x5720
    When a different device with VID 0x044F PID 0xB10A is enumerated
    Then no override is applied to that device
    And the device uses its native HID report descriptor

  Scenario: Override error logged with context
    Given the compat manifest contains a malformed descriptor override
    When the service attempts to load the override
    Then an error is logged including the VID, PID, and the nature of the parsing failure
