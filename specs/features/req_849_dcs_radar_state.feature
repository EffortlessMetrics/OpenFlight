Feature: DCS Radar State
  As a flight simulation enthusiast
  I want dcs radar state
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Expose radar power state and mode
    Given the system is configured for dcs radar state
    When the feature is exercised
    Then dCS adapter exposes radar power state and operating mode

  Scenario: Provide target count when active
    Given the system is configured for dcs radar state
    When the feature is exercised
    Then radar target count is available when radar is active

  Scenario: Publish state changes to bus
    Given the system is configured for dcs radar state
    When the feature is exercised
    Then radar state changes are published to the event bus

  Scenario: Handle aircraft without radar gracefully
    Given the system is configured for dcs radar state
    When the feature is exercised
    Then adapter gracefully handles aircraft without radar systems
