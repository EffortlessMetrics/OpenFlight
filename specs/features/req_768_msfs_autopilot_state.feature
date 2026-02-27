Feature: MSFS Autopilot State
  As a flight simulation enthusiast
  I want msfs autopilot state
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Track autopilot master state
    Given the system is configured for msfs autopilot state
    When the feature is exercised
    Then simconnect adapter tracks autopilot master engagement state

  Scenario: Detect mode changes
    Given the system is configured for msfs autopilot state
    When the feature is exercised
    Then adapter detects autopilot mode changes (heading, altitude, nav)

  Scenario: Publish state changes on bus
    Given the system is configured for msfs autopilot state
    When the feature is exercised
    Then autopilot state changes are published on the event bus

  Scenario: FFB adjusts for autopilot
    Given the system is configured for msfs autopilot state
    When the feature is exercised
    Then ffb effects adjust when autopilot is engaged
