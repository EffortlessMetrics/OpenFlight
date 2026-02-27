Feature: FFB Stall Buffet Effect
  As a flight simulation enthusiast
  I want ffb stall buffet effect
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Generate buffet from AoA telemetry
    Given the system is configured for ffb stall buffet effect
    When the feature is exercised
    Then ffb engine generates stall buffet from angle of attack telemetry

  Scenario: Intensity increases near critical angle
    Given the system is configured for ffb stall buffet effect
    When the feature is exercised
    Then buffet intensity increases as aoa approaches critical angle

  Scenario: Frequency matches aero characteristics
    Given the system is configured for ffb stall buffet effect
    When the feature is exercised
    Then buffet frequency matches aerodynamic characteristics

  Scenario: Respect safety envelope
    Given the system is configured for ffb stall buffet effect
    When the feature is exercised
    Then effect respects ffb safety envelope limits
