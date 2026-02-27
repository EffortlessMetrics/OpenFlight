Feature: FFB Wind Effect
  As a flight simulation enthusiast
  I want ffb wind effect
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Generate wind buffet from airspeed
    Given the system is configured for ffb wind effect
    When the feature is exercised
    Then ffb engine generates wind buffet effects from telemetry airspeed

  Scenario: Intensity scales with airspeed
    Given the system is configured for ffb wind effect
    When the feature is exercised
    Then buffet intensity scales with indicated airspeed

  Scenario: Direction influences force vector
    Given the system is configured for ffb wind effect
    When the feature is exercised
    Then wind direction influences force vector orientation

  Scenario: Respect safety envelope
    Given the system is configured for ffb wind effect
    When the feature is exercised
    Then effect respects ffb safety envelope limits
