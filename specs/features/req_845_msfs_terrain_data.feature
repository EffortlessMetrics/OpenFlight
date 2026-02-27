Feature: MSFS Terrain Data
  As a flight simulation enthusiast
  I want msfs terrain data
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Expose terrain elevation at aircraft position
    Given the system is configured for msfs terrain data
    When the feature is exercised
    Then simConnect adapter exposes terrain elevation at aircraft position

  Scenario: Provide terrain type classification
    Given the system is configured for msfs terrain data
    When the feature is exercised
    Then terrain type classification is available as a readable variable

  Scenario: Update at configured polling rate
    Given the system is configured for msfs terrain data
    When the feature is exercised
    Then terrain data updates at the configured SimConnect polling rate

  Scenario: Return sentinel for unavailable data
    Given the system is configured for msfs terrain data
    When the feature is exercised
    Then unavailable terrain data returns a sentinel value without error
