Feature: FFB Runway Rumble Effect
  As a flight simulation enthusiast
  I want ffb runway rumble effect
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Generate runway surface rumble
    Given the system is configured for ffb runway rumble effect
    When the feature is exercised
    Then ffb engine generates runway surface rumble during ground roll

  Scenario: Intensity scales with ground speed
    Given the system is configured for ffb runway rumble effect
    When the feature is exercised
    Then rumble intensity scales with ground speed

  Scenario: Surface type affects characteristics
    Given the system is configured for ffb runway rumble effect
    When the feature is exercised
    Then surface type affects rumble characteristics

  Scenario: Respect safety envelope
    Given the system is configured for ffb runway rumble effect
    When the feature is exercised
    Then effect respects ffb safety envelope limits
