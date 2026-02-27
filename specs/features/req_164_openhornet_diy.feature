@REQ-164 @product
Feature: OpenHornet DIY cockpit pit support

  @AC-164.1
  Scenario: UFC keypad buttons decoded
    Given an OpenHornet UFC panel is connected and bound
    When a UFC keypad button is pressed
    Then the corresponding button event SHALL be emitted with the correct identifier

  @AC-164.2
  Scenario: IFEI display panel buttons decoded
    Given an OpenHornet IFEI display panel is connected and bound
    When an IFEI button is pressed
    Then the corresponding button event SHALL be emitted with the correct identifier

  @AC-164.3
  Scenario: Custom USB HID report parsed generically
    Given an OpenHornet panel with a custom USB HID report descriptor is connected
    When the panel sends a HID report
    Then the report SHALL be parsed using the generic HID report parser and events emitted for each changed control

  @AC-164.4
  Scenario: Tier 3 support acknowledged in health report
    Given an OpenHornet panel is connected and active
    When a health report is requested
    Then the health report SHALL indicate Tier 3 community support for this device

  @AC-164.5
  Scenario: Unknown PID graceful fallback
    Given the HID subsystem is running
    When a device with the OpenHornet VID but an unrecognised PID is connected
    Then the device SHALL fall back to generic HID parsing without raising a fatal error

  @AC-164.6
  Scenario: Panel events routed to DCS export
    Given an OpenHornet panel is connected and bound
    And the DCS export adapter is active
    When a panel button is pressed
    Then the corresponding event SHALL be forwarded to the DCS export adapter
