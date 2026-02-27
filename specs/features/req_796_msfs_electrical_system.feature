Feature: MSFS Electrical System
  As a flight simulation enthusiast
  I want msfs electrical system
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Expose bus voltage and battery state
    Given the system is configured for msfs electrical system
    When the feature is exercised
    Then simconnect adapter exposes main bus voltage and battery state

  Scenario: Generator and alternator status
    Given the system is configured for msfs electrical system
    When the feature is exercised
    Then electrical data includes generator and alternator status

  Scenario: Publish changes on event bus
    Given the system is configured for msfs electrical system
    When the feature is exercised
    Then electrical state changes are published on the event bus

  Scenario: Update rate at least 1Hz
    Given the system is configured for msfs electrical system
    When the feature is exercised
    Then electrical data is updated at least once per second
