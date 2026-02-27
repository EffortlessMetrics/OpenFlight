Feature: DCS Mission State
  As a flight simulation enthusiast
  I want dcs mission state
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Expose mission phase state
    Given the system is configured for dcs mission state
    When the feature is exercised
    Then dCS adapter exposes current mission phase (briefing, flying, debriefing)

  Scenario: Publish state transitions to bus
    Given the system is configured for dcs mission state
    When the feature is exercised
    Then mission state transitions are published to the event bus

  Scenario: Detect restart and reset state
    Given the system is configured for dcs mission state
    When the feature is exercised
    Then adapter detects mission restart and resets state accordingly

  Scenario: Provide mission time elapsed
    Given the system is configured for dcs mission state
    When the feature is exercised
    Then mission time elapsed is available as a readable variable
