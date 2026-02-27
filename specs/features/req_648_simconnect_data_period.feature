Feature: SimConnect Data Period Tuning
  As a flight simulation enthusiast
  I want SimConnect subscription data period to be tunable
  So that I can balance update frequency against CPU load per variable

  Background:
    Given the OpenFlight service is running and connected to MSFS via SimConnect

  Scenario: Data period can be set to SIM_FRAME, SECOND, or custom count
    Given a SimConnect variable subscription is configured with period "SIM_FRAME"
    When the subscription is registered
    Then data is received every simulator frame

  Scenario: Per-variable period is configurable in profile
    Given a profile configures "PLANE_ALTITUDE" with period "SECOND"
    And a profile configures "ELEVATOR_POSITION" with period "SIM_FRAME"
    When the SimConnect adapter connects
    Then each variable is subscribed with its individually configured period

  Scenario: Period changes take effect on next SimConnect reconnect
    Given a variable period is changed in the profile
    When the SimConnect adapter reconnects
    Then the variable is re-subscribed with the updated period

  Scenario: Current periods are visible in SimConnect adapter diagnostics
    When the command "flightctl diagnostics simconnect" is run
    Then the output lists each subscribed variable with its current data period
