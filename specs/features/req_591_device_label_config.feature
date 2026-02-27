Feature: Device Label Configuration
  As a flight simulation enthusiast
  I want to assign user-defined labels to devices in my profile
  So that I can identify my hardware easily in the CLI and diagnostics

  Background:
    Given the OpenFlight service is running
    And at least one HID device is connected

  Scenario: User-defined labels can be assigned to device VID/PID in profile
    Given a profile entry assigns the label "HOTAS Throttle" to VID 0x044F PID 0xB10A
    When the profile is loaded
    Then the device with VID 0x044F PID 0xB10A is associated with the label "HOTAS Throttle"

  Scenario: Labels appear in CLI device list and diagnostics
    Given the device "HOTAS Throttle" label is configured
    When the user runs "flightctl devices list"
    Then the output includes the label "HOTAS Throttle" next to the device entry

  Scenario: Labels survive device reconnection
    Given the device labelled "HOTAS Throttle" disconnects and reconnects
    When the device is re-enumerated
    Then the label "HOTAS Throttle" is still associated with the device

  Scenario: Labels are validated to be non-empty strings up to 64 chars
    Given a profile entry assigns an empty label to a device
    When the profile is loaded
    Then the load fails with a validation error indicating the label must be non-empty
    Given a profile entry assigns a label of 65 characters to a device
    When the profile is loaded
    Then the load fails with a validation error indicating the label exceeds 64 characters
