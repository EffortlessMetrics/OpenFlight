Feature: DCS Weapon Release Detection
  As a flight simulation enthusiast
  I want dcs weapon release detection
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Detect weapon release events
    Given the system is configured for dcs weapon release detection
    When the feature is exercised
    Then dcs adapter detects weapon release events from export data

  Scenario: Identify weapon type
    Given the system is configured for dcs weapon release detection
    When the feature is exercised
    Then weapon type is identified in the release event

  Scenario: Publish release on event bus
    Given the system is configured for dcs weapon release detection
    When the feature is exercised
    Then release events are published on the event bus

  Scenario: Track rapid sequential releases
    Given the system is configured for dcs weapon release detection
    When the feature is exercised
    Then multiple rapid releases are individually tracked
