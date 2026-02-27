Feature: FFB Wheel Shimmy
  As a flight simulation enthusiast
  I want ffb wheel shimmy
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Simulate shimmy during ground roll
    Given the system is configured for ffb wheel shimmy
    When the feature is exercised
    Then fFB simulates nose wheel shimmy vibration during ground roll

  Scenario: Intensity scales with ground speed
    Given the system is configured for ffb wheel shimmy
    When the feature is exercised
    Then shimmy intensity increases with ground speed above a configurable threshold

  Scenario: Suppress when steering centered
    Given the system is configured for ffb wheel shimmy
    When the feature is exercised
    Then shimmy effect is suppressed when nosewheel steering is centered

  Scenario: Respect FFB safety envelope
    Given the system is configured for ffb wheel shimmy
    When the feature is exercised
    Then effect respects FFB safety envelope limits
