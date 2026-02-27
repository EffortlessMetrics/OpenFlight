Feature: Profile Conditional Activation
  As a flight simulation enthusiast
  I want profile conditional activation
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Auto-activate based on sim state conditions
    Given the system is configured for profile conditional activation
    When the feature is exercised
    Then profiles activate automatically based on simulator state conditions

  Scenario: Match on aircraft, phase, and sim variables
    Given the system is configured for profile conditional activation
    When the feature is exercised
    Then conditions support matching on aircraft type, phase of flight, and sim variables

  Scenario: Priority ordering for multiple profiles
    Given the system is configured for profile conditional activation
    When the feature is exercised
    Then multiple profiles can have activation conditions with priority ordering

  Scenario: No impact on RT processing latency
    Given the system is configured for profile conditional activation
    When the feature is exercised
    Then condition evaluation does not impact RT spine processing latency
