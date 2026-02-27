Feature: Axis Mode Switching
  As a flight simulation enthusiast
  I want axis mode switching
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Switch between normal, precision, and fast modes
    Given the system is configured for axis mode switching
    When the feature is exercised
    Then axis engine supports runtime switching between normal, precision, and fast modes

  Scenario: Smooth transitions on mode switch
    Given the system is configured for axis mode switching
    When the feature is exercised
    Then mode switch transitions apply smoothing to avoid output discontinuities

  Scenario: Expose current mode as state variable
    Given the system is configured for axis mode switching
    When the feature is exercised
    Then current mode is exposed as a readable state variable

  Scenario: Configure mode parameters per-axis in profile
    Given the system is configured for axis mode switching
    When the feature is exercised
    Then mode configuration parameters are defined per-axis in the profile
