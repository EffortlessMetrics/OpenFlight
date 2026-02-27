@REQ-148 @product
Feature: GoFlight panel integration  @AC-148.1
  Scenario: GF-MCP Pro autopilot panel buttons decoded
    Given a GF-MCP Pro panel connected via USB HID
    When a HID input report with autopilot button presses is received
    Then all autopilot button states SHALL be decoded correctly  @AC-148.2
  Scenario: GF-MCP Pro speed and heading rotary increments correctly
    Given a GF-MCP Pro panel with speed and heading encoder knobs
    When the speed encoder is rotated clockwise by one detent
    Then the speed value SHALL be incremented by the configured step  @AC-148.3
  Scenario: GF-EFIS buttons decoded
    Given a GF-EFIS panel connected via USB HID
    When a HID input report with EFIS button states is received
    Then all EFIS button states SHALL be decoded correctly  @AC-148.4
  Scenario: GF-RP48 rotary position read
    Given a GF-RP48 panel connected via USB HID
    When the rotary selector is moved to a new position
    Then the adapter SHALL report the new rotary position value  @AC-148.5
  Scenario: GF-T8 Plus toggle states decoded
    Given a GF-T8 Plus panel connected via USB HID
    When a HID input report with toggle switch states is received
    Then all toggle states SHALL be decoded and reported correctly  @AC-148.6
  Scenario: LED states updated via output report
    Given a connected GoFlight panel that supports LED output
    When an LED state update command is issued
    Then the adapter SHALL write the correct HID output report to the device  @AC-148.7
  Scenario: Multiple GoFlight panels on same USB hub
    Given two GoFlight panels of different types connected to the same USB hub
    When HID enumeration runs
    Then both panels SHALL be enumerated and each addressed independently  @AC-148.8
  Scenario: Panel enumeration returns correct device types
    Given one or more GoFlight panels connected
    When the panel enumeration API is queried
    Then each entry SHALL report the correct GoFlight device type identifier
