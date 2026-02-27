Feature: FFB Trim Effect
  As a flight simulation enthusiast
  I want ffb trim effect
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Shift centering to trim position
    Given the system is configured for ffb trim effect
    When the feature is exercised
    Then ffb engine shifts the centering force to match trim position

  Scenario: Smooth transition period
    Given the system is configured for ffb trim effect
    When the feature is exercised
    Then trim shift is smooth over a configurable transition period

  Scenario: Combine with other effects
    Given the system is configured for ffb trim effect
    When the feature is exercised
    Then trim effect combines correctly with other active effects

  Scenario: Respect safety envelope
    Given the system is configured for ffb trim effect
    When the feature is exercised
    Then trim effect respects ffb safety envelope limits
