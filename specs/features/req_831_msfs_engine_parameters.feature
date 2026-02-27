Feature: MSFS Engine Parameters
  As a flight simulation enthusiast
  I want msfs engine parameters
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Expose RPM, manifold pressure, and EGT
    Given the system is configured for msfs engine parameters
    When the feature is exercised
    Then simConnect adapter exposes RPM, manifold pressure, and EGT per engine

  Scenario: Publish engine parameter changes to bus
    Given the system is configured for msfs engine parameters
    When the feature is exercised
    Then engine parameter changes are published to the event bus

  Scenario: Support turboprop and piston parameters
    Given the system is configured for msfs engine parameters
    When the feature is exercised
    Then adapter supports turboprop and piston engine parameter sets

  Scenario: Refresh rate matches polling interval
    Given the system is configured for msfs engine parameters
    When the feature is exercised
    Then parameter refresh rate matches the configured polling interval
