Feature: DCS Cockpit Controls
  As a flight simulation enthusiast
  I want dcs cockpit controls
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Expose clickable cockpit control states
    Given the system is configured for dcs cockpit controls
    When the feature is exercised
    Then dCS adapter exposes the state of clickable cockpit controls

  Scenario: Publish control state changes to bus
    Given the system is configured for dcs cockpit controls
    When the feature is exercised
    Then control state changes are published to the event bus

  Scenario: Map argument numbers to readable names
    Given the system is configured for dcs cockpit controls
    When the feature is exercised
    Then adapter maps DCS argument numbers to human-readable control names

  Scenario: Skip unsupported controls with debug log
    Given the system is configured for dcs cockpit controls
    When the feature is exercised
    Then unsupported controls are skipped with a debug-level log entry
