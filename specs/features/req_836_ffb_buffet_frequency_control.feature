Feature: FFB Buffet Frequency Control
  As a flight simulation enthusiast
  I want ffb buffet frequency control
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Configurable buffet frequency per aircraft
    Given the system is configured for ffb buffet frequency control
    When the feature is exercised
    Then fFB buffet effect frequency is configurable in the aircraft profile

  Scenario: Validate frequency against device limits
    Given the system is configured for ffb buffet frequency control
    When the feature is exercised
    Then frequency range is validated against device capability limits

  Scenario: Smooth frequency transitions during effects
    Given the system is configured for ffb buffet frequency control
    When the feature is exercised
    Then frequency transitions smoothly when changed during active effect

  Scenario: Default frequency when unspecified
    Given the system is configured for ffb buffet frequency control
    When the feature is exercised
    Then default frequency is used when profile does not specify a value
