Feature: FFB Ground Effect
  As a flight simulation enthusiast
  I want ffb ground effect
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Generate force cushion below ground effect altitude
    Given the system is configured for ffb ground effect
    When the feature is exercised
    Then fFB generates increasing force cushion as aircraft descends below ground effect altitude

  Scenario: Scale intensity with wing config and airspeed
    Given the system is configured for ffb ground effect
    When the feature is exercised
    Then ground effect intensity scales with wing configuration and airspeed

  Scenario: Smooth transition between ground effect and free air
    Given the system is configured for ffb ground effect
    When the feature is exercised
    Then effect transitions smoothly between ground effect and free air

  Scenario: Respect FFB safety envelope limits
    Given the system is configured for ffb ground effect
    When the feature is exercised
    Then ground effect respects FFB safety envelope limits
