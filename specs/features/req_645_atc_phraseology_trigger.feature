Feature: ATC Phraseology Axis Trigger
  As a flight simulation enthusiast
  I want axis positions to trigger ATC phraseology events
  So that ATC communications can be automated from physical controls

  Background:
    Given the OpenFlight service is running and connected to MSFS via SimConnect

  Scenario: ATC trigger fires when axis crosses configurable threshold
    Given an axis trigger is configured with a threshold of 0.9
    When the axis value crosses 0.9
    Then the ATC trigger fires

  Scenario: Trigger maps to SimConnect ATC event
    Given an axis ATC trigger is configured to send "ATC_MENU_OPEN"
    When the trigger fires
    Then the SimConnect ATC event "ATC_MENU_OPEN" is transmitted

  Scenario: Trigger includes configurable hysteresis
    Given an axis ATC trigger has a threshold of 0.9 and hysteresis of 0.05
    When the axis rises above 0.9 firing the trigger
    Then the trigger does not re-arm until the axis falls below 0.85

  Scenario: Trigger state is included in axis diagnostics
    Given an ATC axis trigger is configured
    When axis diagnostics are retrieved via CLI
    Then the output includes the trigger threshold, hysteresis, and current armed state
