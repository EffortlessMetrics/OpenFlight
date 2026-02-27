Feature: MSFS ATC Event Integration
  As a flight simulation enthusiast
  I want to bind MSFS ATC events to physical buttons
  So that I can control radio and ATC functions from my hardware

  Background:
    Given the OpenFlight service is running
    And the SimConnect adapter is connected to MSFS

  Scenario: ATC events can be bound to buttons
    Given a profile rule binds button "COM_RADIO_UP" to the ATC event "COM_RADIO_WHOLE_INC"
    When the button is pressed
    Then the "COM_RADIO_WHOLE_INC" ATC event is dispatched to MSFS via SimConnect

  Scenario: ATC event bindings are configurable in profile rules
    Given a profile TOML file containing an ATC event binding section
    When the profile is loaded by the service
    Then the ATC event bindings are registered in the rules engine

  Scenario: ATC state changes are received from MSFS via SimConnect
    Given the adapter subscribes to the COM1 frequency SimVar
    When MSFS reports a frequency change to 122.800
    Then the flight-bus receives an ATC state update with frequency 122.800

  Scenario: ATC event bus snapshot includes current frequency
    When the diagnostics endpoint returns the ATC event bus snapshot
    Then the snapshot includes a "com1_frequency_mhz" field with the current value
