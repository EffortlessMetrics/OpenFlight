Feature: MSFS Traffic Data
  As a flight simulation enthusiast
  I want msfs traffic data
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Expose AI traffic positions
    Given the system is configured for msfs traffic data
    When the feature is exercised
    Then simconnect adapter exposes ai traffic aircraft positions

  Scenario: Include callsign type distance
    Given the system is configured for msfs traffic data
    When the feature is exercised
    Then traffic data includes callsign, type, and distance

  Scenario: Configurable radius filter
    Given the system is configured for msfs traffic data
    When the feature is exercised
    Then only traffic within a configurable radius is reported

  Scenario: Publish traffic on event bus
    Given the system is configured for msfs traffic data
    When the feature is exercised
    Then traffic data is published on the event bus
