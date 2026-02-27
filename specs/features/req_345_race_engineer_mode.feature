@REQ-345 @product
Feature: Race Engineer Mode (Sim Racing)  @AC-345.1
  Scenario: Pit strategy data is displayed on StreamDeck panel
    Given the service is in race engineer mode and pit strategy data is available
    When the StreamDeck panel is active
    Then the panel SHALL display current pit strategy information  @AC-345.2
  Scenario: Lap count and position drive panel button colors
    Given the race is in progress with telemetry providing lap count and position
    When the StreamDeck panel renders
    Then button colors SHALL reflect the current lap count and race position  @AC-345.3
  Scenario: Pit limiter button maps to in-game pit limiter toggle
    Given a StreamDeck button is mapped to the pit limiter action
    When the user presses that button
    Then the service SHALL send the pit limiter toggle command to the simulator  @AC-345.4
  Scenario: DRS button maps to in-game DRS when available
    Given telemetry indicates DRS is available for the current lap
    When the user presses the DRS button
    Then the service SHALL send the DRS activation command to the simulator  @AC-345.5
  Scenario: Race mode profiles are auto-loaded when racing sim is detected
    Given a racing simulator is detected by the service
    When the service connects to that simulator
    Then the service SHALL automatically load the race mode profile  @AC-345.6
  Scenario: Race data is cleared when sim disconnects
    Given the service is in race engineer mode with active telemetry data
    When the racing simulator disconnects
    Then the service SHALL clear all race telemetry data from the panel and internal state
