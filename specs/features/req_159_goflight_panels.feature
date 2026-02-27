@REQ-159 @product
Feature: GoFlight avionics panels

  @AC-159.1
  Scenario: GF-MCP Pro speed knob CW increments value by 1 kts
    Given a GF-MCP Pro panel is connected and bound
    When the speed knob is rotated one step clockwise
    Then the speed value SHALL increment by 1 knot

  @AC-159.2
  Scenario: GF-MCP Pro heading knob CW increments by 1 degree
    Given a GF-MCP Pro panel is connected and bound
    When the heading knob is rotated one step clockwise
    Then the heading value SHALL increment by 1 degree

  @AC-159.3
  Scenario: GF-EFIS BARO setting knob decoded
    Given a GF-EFIS panel is connected and bound
    When the BARO setting knob is rotated
    Then the corresponding barometric pressure event SHALL be emitted with the correct delta

  @AC-159.4
  Scenario: GF-RP48 16-position rotary decoded
    Given a GF-RP48 panel is connected and bound
    When the 16-position rotary switch is set to any valid position
    Then the decoded position value SHALL match the physical knob position

  @AC-159.5
  Scenario: GF-T8 Plus toggle states decoded
    Given a GF-T8 Plus panel is connected and bound
    When any toggle switch changes state
    Then the toggle event SHALL report the correct on or off state

  @AC-159.6
  Scenario: LED output updated on all panel types
    Given a GoFlight panel supporting LED output is connected and bound
    When the host writes an LED state update
    Then the panel LEDs SHALL reflect the requested state within one processing cycle

  @AC-159.7
  Scenario: Panel HID connection enumerated correctly
    Given the HID subsystem is running
    When a GoFlight panel is physically connected
    Then the panel SHALL be enumerated and its type identified from the USB VID/PID pair
